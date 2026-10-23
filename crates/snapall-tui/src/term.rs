// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The terminal: setup and restore, key translation, and the event loop.

use std::io;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::DefaultTerminal;
use signal_hook::consts::{SIGHUP, SIGINT, SIGTERM};
use signal_hook::flag;
use snapall_core::SnapshotBackend;

use crate::model::Key;
use crate::session::Session;
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
/// Twelve-factor disposability: a process must stop gracefully on SIGTERM. Without this, systemd
/// stopping a unit or an ssh session dropping would leave the user's terminal in raw mode on the
/// alternate screen.
pub fn run(backend: &dyn SnapshotBackend) -> io::Result<Exit> {
    // Registered before the terminal is touched, so there is no window where a signal kills a
    // process that has already changed the terminal.
    let received = Arc::new(AtomicUsize::new(0));
    for signal in [SIGINT, SIGTERM, SIGHUP] {
        flag::register_usize(signal, Arc::clone(&received), signal as usize)?;
    }
    let mut terminal = ratatui::try_init()?;
    let result = event_loop(&mut terminal, Session::new(backend), &received);
    ratatui::restore();
    result
}

fn event_loop(
    terminal: &mut DefaultTerminal,
    mut session: Session<'_>,
    received: &AtomicUsize,
) -> io::Result<Exit> {
    terminal.draw(|frame| view::render(&session.state, frame))?;
    loop {
        if session.state.quit {
            return Ok(Exit::Quit);
        }
        if let Ok(signal) = i32::try_from(received.load(Ordering::Relaxed)) {
            if signal != 0 {
                return Ok(Exit::Signal(signal));
            }
        }
        if !event::poll(TICK)? {
            continue;
        }
        // Resize and other events simply fall through to a redraw.
        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                if let Some(key) = translate(key) {
                    session.press(key);
                }
            }
        }
        terminal.draw(|frame| view::render(&session.state, frame))?;
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
