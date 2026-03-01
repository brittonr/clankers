//! Terminal event handling

use std::time::Duration;

use crossterm::event::Event;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use crossterm::event::MouseButton;
use crossterm::event::MouseEventKind;
use crossterm::event::{self};

/// Mouse button classification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    Left,
    Right,
    Middle,
}

impl From<MouseButton> for Button {
    fn from(btn: MouseButton) -> Self {
        match btn {
            MouseButton::Left => Button::Left,
            MouseButton::Right => Button::Right,
            MouseButton::Middle => Button::Middle,
        }
    }
}

/// Application events
#[derive(Debug, Clone)]
pub enum AppEvent {
    /// Key press
    Key(KeyEvent),
    /// Bracketed paste text
    Paste(String),
    /// Terminal resize
    Resize(u16, u16),
    /// Mouse scroll up at (col, row, lines)
    ScrollUp(u16, u16, u16),
    /// Mouse scroll down at (col, row, lines)
    ScrollDown(u16, u16, u16),
    /// Mouse button pressed at (button, col, row)
    MouseDown(Button, u16, u16),
    /// Mouse dragged to (button, col, row)
    MouseDrag(Button, u16, u16),
    /// Mouse button released at (button, col, row)
    MouseUp(Button, u16, u16),
    /// Other mouse event
    Mouse,
}

/// Poll for terminal events with timeout
pub fn poll_event(timeout: Duration) -> Option<AppEvent> {
    if event::poll(timeout).ok()? {
        match event::read().ok()? {
            Event::Key(key) => Some(AppEvent::Key(key)),
            Event::Paste(text) => Some(AppEvent::Paste(text)),
            Event::Resize(w, h) => Some(AppEvent::Resize(w, h)),
            Event::Mouse(mouse) => match mouse.kind {
                MouseEventKind::ScrollUp => Some(AppEvent::ScrollUp(mouse.column, mouse.row, 3)),
                MouseEventKind::ScrollDown => Some(AppEvent::ScrollDown(mouse.column, mouse.row, 3)),
                MouseEventKind::Down(btn) => Some(AppEvent::MouseDown(btn.into(), mouse.column, mouse.row)),
                MouseEventKind::Drag(btn) => Some(AppEvent::MouseDrag(btn.into(), mouse.column, mouse.row)),
                MouseEventKind::Up(btn) => Some(AppEvent::MouseUp(btn.into(), mouse.column, mouse.row)),
                _ => Some(AppEvent::Mouse),
            },
            _ => None,
        }
    } else {
        None
    }
}

/// Check if key event is a quit command (Ctrl+C or Ctrl+D)
pub fn is_quit(key: &KeyEvent) -> bool {
    matches!(key, KeyEvent {
        code: KeyCode::Char('c' | 'd'),
        modifiers: KeyModifiers::CONTROL,
        ..
    })
}

/// Check if key event is submit (Enter without modifiers)
pub fn is_submit(key: &KeyEvent) -> bool {
    matches!(key, KeyEvent {
        code: KeyCode::Enter,
        modifiers: KeyModifiers::NONE,
        ..
    })
}

/// Check if key event is Alt+Enter (newline in input)
pub fn is_alt_enter(key: &KeyEvent) -> bool {
    matches!(key, KeyEvent {
        code: KeyCode::Enter,
        modifiers: KeyModifiers::ALT,
        ..
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_button_from_mouse_button() {
        assert_eq!(Button::from(MouseButton::Left), Button::Left);
        assert_eq!(Button::from(MouseButton::Right), Button::Right);
        assert_eq!(Button::from(MouseButton::Middle), Button::Middle);
    }

    #[test]
    fn test_button_equality() {
        assert_eq!(Button::Left, Button::Left);
        assert_ne!(Button::Left, Button::Right);
        assert_ne!(Button::Left, Button::Middle);
    }
}
