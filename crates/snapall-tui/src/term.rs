// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The terminal: setup and restore, key translation, and the event loop.

use std::io;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::Duration;

use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::DefaultTerminal;
use signal_hook::consts::{SIGHUP, SIGINT, SIGTERM};
use signal_hook::flag;
use snapall_core::SnapshotBackend;

use crate::model::{update, Effect, Key, Msg, Notice, State};
use crate::session::Executor;
use crate::view;

/// How the UI ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    /// The user quit (`q`, Ctrl-C).
    Quit,
    /// A termination signal arrived (SIGINT, SIGTERM or SIGHUP) and the terminal was restored
    /// before returning. The caller should exit as a signalled process would: `128 + signal`.
    Signal(i32),
}

/// How long to wait for a key before checking whether a signal arrived.
const TICK: Duration = Duration::from_millis(100);

/// Run the UI until the user quits or the process is asked to stop. The terminal is restored on
/// every exit path: quitting, a signal, an error, and a panic (ratatui installs a hook).
///
/// Snapper calls run on a worker thread, so a slow one (a delete on a large subvolume, a busy
/// disk) never freezes the screen: the loop keeps drawing, keeps reading keys, and shows a
/// "working" indicator until the result comes back. The model decides what is allowed meanwhile
/// (reading and moving: yes; starting more work: no).
///
/// Twelve-factor disposability: a process must stop gracefully on SIGTERM. Without this, systemd
/// stopping a unit or an ssh session dropping would leave the user's terminal in raw mode on the
/// alternate screen. A signal does not wait for work in flight; leaving the scope does join the
/// worker, which finishes only the single snapper call it is in.
pub fn run(backend: &(dyn SnapshotBackend + Sync)) -> io::Result<Exit> {
    // Registered before the terminal is touched, so there is no window where a signal kills a
    // process that has already changed the terminal.
    let received = Arc::new(AtomicUsize::new(0));
    for signal in [SIGINT, SIGTERM, SIGHUP] {
        flag::register_usize(signal, Arc::clone(&received), signal as usize)?;
    }
    let mut terminal = ratatui::try_init()?;
    let result = thread::scope(|scope| {
        let (effect_tx, effect_rx) = mpsc::channel::<Effect>();
        let (result_tx, result_rx) = mpsc::channel::<Msg>();
        let executor = Executor::new(backend);
        scope.spawn(move || {
            for effect in effect_rx {
                if result_tx.send(executor.execute(effect)).is_err() {
                    break;
                }
            }
        });
        // `effect_tx` is dropped when this closure returns, which ends the worker's loop.
        event_loop(&mut terminal, &effect_tx, &result_rx, &received)
    });
    ratatui::restore();
    result
}

/// Feed a message to the model and hand the effects it asks for to the worker.
fn step(state: &mut State, msg: Msg, effects: &mpsc::Sender<Effect>) {
    for effect in update(state, msg) {
        if effects.send(effect).is_err() {
            // The worker is gone: nothing will ever answer, so do not wait for it.
            state.pending = state.pending.saturating_sub(1);
            state.notice = Some(Notice::Error("the background worker stopped".into()));
        }
    }
}

fn pending_signal(received: &AtomicUsize) -> Option<i32> {
    i32::try_from(received.load(Ordering::Relaxed))
        .ok()
        .filter(|signal| *signal != 0)
}

fn event_loop(
    terminal: &mut DefaultTerminal,
    effects: &mpsc::Sender<Effect>,
    results: &mpsc::Receiver<Msg>,
    received: &AtomicUsize,
) -> io::Result<Exit> {
    let mut state = State::default();
    step(&mut state, Msg::Start, effects);
    terminal.draw(|frame| view::render(&state, frame))?;
    loop {
        let mut changed = false;
        loop {
            match results.try_recv() {
                Ok(msg) => {
                    step(&mut state, msg, effects);
                    changed = true;
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    if state.pending > 0 {
                        state.pending = 0;
                        state.notice = Some(Notice::Error("the background worker stopped".into()));
                        changed = true;
                    }
                    break;
                }
            }
        }
        // Quitting waits for the operation in flight: a half-finished delete is worse than a
        // short wait. The header says "finishing" meanwhile.
        if state.quit && state.pending == 0 {
            return Ok(Exit::Quit);
        }
        if let Some(signal) = pending_signal(received) {
            return Ok(Exit::Signal(signal));
        }
        if event::poll(TICK)? {
            // Resize and other events simply fall through to a redraw.
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    if let Some(key) = translate(key) {
                        step(&mut state, Msg::Key(key), effects);
                    }
                }
            }
            changed = true;
        } else if state.pending > 0 {
            step(&mut state, Msg::Tick, effects);
            changed = true;
        }
        if changed {
            terminal.draw(|frame| view::render(&state, frame))?;
        }
    }
}

/// Map a terminal key event to the model's [`Key`]. Unmapped keys are ignored.
pub fn translate(event: KeyEvent) -> Option<Key> {
    let ctrl = event.modifiers.contains(KeyModifiers::CONTROL);
    Some(match event.code {
        KeyCode::Char('c') if ctrl => Key::CtrlC,
        KeyCode::Char(_) if ctrl => return None,
        KeyCode::Char(c) => Key::Char(c),
        KeyCode::Up => Key::Up,
        KeyCode::Down => Key::Down,
        KeyCode::Left => Key::Left,
        KeyCode::Right => Key::Right,
        KeyCode::Home => Key::Home,
        KeyCode::End => Key::End,
        KeyCode::Tab | KeyCode::BackTab => Key::Tab,
        KeyCode::Enter => Key::Enter,
        KeyCode::Esc => Key::Esc,
        KeyCode::Backspace => Key::Backspace,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    #[test]
    fn translates_the_keys_the_ui_uses() {
        let none = KeyModifiers::NONE;
        assert_eq!(
            translate(key(KeyCode::Char('c'), none)),
            Some(Key::Char('c'))
        );
        assert_eq!(
            translate(key(KeyCode::Char('G'), KeyModifiers::SHIFT)),
            Some(Key::Char('G'))
        );
        assert_eq!(translate(key(KeyCode::Down, none)), Some(Key::Down));
        assert_eq!(
            translate(key(KeyCode::BackTab, KeyModifiers::SHIFT)),
            Some(Key::Tab)
        );
        assert_eq!(translate(key(KeyCode::Enter, none)), Some(Key::Enter));
    }

    #[test]
    fn ctrl_c_quits_and_other_control_chords_do_nothing() {
        let ctrl = KeyModifiers::CONTROL;
        assert_eq!(translate(key(KeyCode::Char('c'), ctrl)), Some(Key::CtrlC));
        assert_eq!(translate(key(KeyCode::Char('d'), ctrl)), None);
        assert_eq!(translate(key(KeyCode::F(5), KeyModifiers::NONE)), None);
    }
}
