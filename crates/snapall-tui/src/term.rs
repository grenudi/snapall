//! The terminal: setup and restore, key translation, and the event loop.

use std::io;

use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::DefaultTerminal;
use snapall_core::SnapshotBackend;

use crate::model::Key;
use crate::session::Session;
use crate::view;

/// Run the UI until the user quits. The terminal is restored on every exit path, including
/// panics (ratatui installs a panic hook that restores it first).
pub fn run(backend: &dyn SnapshotBackend) -> io::Result<()> {
    let mut terminal = ratatui::try_init()?;
    let result = event_loop(&mut terminal, Session::new(backend));
    ratatui::restore();
    result
}

fn event_loop(terminal: &mut DefaultTerminal, mut session: Session<'_>) -> io::Result<()> {
    loop {
        terminal.draw(|frame| view::render(&session.state, frame))?;
        if session.state.quit {
            return Ok(());
        }
        // Resize and other events simply fall through to a redraw.
        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                if let Some(key) = translate(key) {
                    session.press(key);
                }
            }
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
