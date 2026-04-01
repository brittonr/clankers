//! Terminal event handling

use std::time::Duration;

use crossterm::event::Event;
use crossterm::event::KeyEvent;
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
    /// Terminal gained focus
    FocusGained,
    /// Terminal lost focus
    FocusLost,
}

/// Poll for terminal events with timeout
#[cfg_attr(dylint_lib = "tigerstyle", allow(catch_all_on_enum, reason = "default handler covers many variants uniformly"))]
pub fn poll_event(timeout: Duration) -> Option<AppEvent> {
    if event::poll(timeout).ok()? {
        match event::read().ok()? {
            Event::Key(key) => Some(AppEvent::Key(key)),
            Event::Paste(text) => Some(AppEvent::Paste(text)),
            Event::FocusGained => Some(AppEvent::FocusGained),
            Event::FocusLost => Some(AppEvent::FocusLost),
            Event::Resize(w, h) => Some(AppEvent::Resize(w, h)),
            Event::Mouse(mouse) => match mouse.kind {
                MouseEventKind::ScrollUp => Some(AppEvent::ScrollUp(mouse.column, mouse.row, 3)),
                MouseEventKind::ScrollDown => Some(AppEvent::ScrollDown(mouse.column, mouse.row, 3)),
                MouseEventKind::Down(btn) => Some(AppEvent::MouseDown(btn.into(), mouse.column, mouse.row)),
                MouseEventKind::Drag(btn) => Some(AppEvent::MouseDrag(btn.into(), mouse.column, mouse.row)),
                MouseEventKind::Up(btn) => Some(AppEvent::MouseUp(btn.into(), mouse.column, mouse.row)),
                _ => Some(AppEvent::Mouse),
            },
            #[allow(unreachable_patterns)]
            _ => None,
        }
    } else {
        None
    }
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
