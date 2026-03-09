//! Clipboard and external editor support.

use std::io;

use crossterm::event::DisableBracketedPaste;
use crossterm::event::DisableMouseCapture;
use crossterm::event::EnableBracketedPaste;
use crossterm::event::EnableMouseCapture;
use crossterm::execute;
use crossterm::terminal::EnterAlternateScreen;
use crossterm::terminal::LeaveAlternateScreen;
use crossterm::terminal::{self};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use crate::config::keybindings::InputMode;
use crate::tui::app::App;

/// Result of a background clipboard read.
pub enum ClipboardResult {
    /// Text was found in the clipboard.
    Text(String),
    /// An image was found: base64 PNG, mime type, raw size, width, height.
    Image {
        encoded: String,
        mime: String,
        raw_size: usize,
        width: u32,
        height: u32,
    },
    /// Nothing useful in clipboard.
    Empty(String),
    /// Error accessing the clipboard.
    Error(String),
}

/// Read from the system clipboard on a background thread. Tries text first,
/// then image. This avoids freezing the TUI when another application (e.g. a
/// browser) holds the Wayland clipboard selection.
pub(crate) fn paste_from_clipboard(app: &mut App) {
    if app.clipboard_pending {
        return;
    }
    app.clipboard_pending = true;

    let (tx, rx) = std::sync::mpsc::channel::<ClipboardResult>();

    std::thread::spawn(move || {
        let result = (|| -> Result<ClipboardResult, ClipboardResult> {
            let mut clipboard =
                arboard::Clipboard::new().map_err(|e| ClipboardResult::Error(format!("Clipboard error: {e}")))?;

            // Try text first — this is what the user almost always wants with Ctrl+V
            if let Ok(text) = clipboard.get_text()
                && !text.is_empty()
            {
                return Ok(ClipboardResult::Text(text));
            }

            // Fall back to image
            match clipboard.get_image() {
                Ok(img_data) => {
                    use base64::Engine;
                    use base64::engine::general_purpose::STANDARD as BASE64;

                    let width = img_data.width as u32;
                    let height = img_data.height as u32;
                    let rgba: Vec<u8> = img_data.bytes.into_owned();

                    let img = image::RgbaImage::from_raw(width, height, rgba)
                        .ok_or_else(|| ClipboardResult::Error("Failed to decode clipboard image data.".to_string()))?;

                    let mut png_buf: Vec<u8> = Vec::new();
                    let mut cursor = std::io::Cursor::new(&mut png_buf);
                    img.write_to(&mut cursor, image::ImageFormat::Png)
                        .map_err(|e| ClipboardResult::Error(format!("Failed to encode image as PNG: {e}")))?;

                    let raw_size = png_buf.len();
                    let encoded = BASE64.encode(&png_buf);

                    Ok(ClipboardResult::Image {
                        encoded,
                        mime: "image/png".to_string(),
                        raw_size,
                        width,
                        height,
                    })
                }
                Err(_) => Err(ClipboardResult::Empty("Clipboard is empty.".to_string())),
            }
        })();

        let _ = tx.send(result.unwrap_or_else(|e| e));
    });

    app.clipboard_rx = Some(rx);
}

/// Poll for a completed clipboard read (non-blocking).
pub(crate) fn poll_clipboard_result(app: &mut App) {
    let result = if let Some(ref rx) = app.clipboard_rx {
        match rx.try_recv() {
            Ok(result) => Some(result),
            Err(std::sync::mpsc::TryRecvError::Empty) => return,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                Some(ClipboardResult::Error("Clipboard thread crashed.".to_string()))
            }
        }
    } else {
        return;
    };

    app.clipboard_rx = None;
    app.clipboard_pending = false;

    if let Some(result) = result {
        match result {
            ClipboardResult::Text(text) => {
                app.input_mode = InputMode::Insert;
                app.selection = None;
                app.editor.insert_str(&text);
                app.update_slash_menu();
            }
            ClipboardResult::Image {
                encoded,
                mime,
                raw_size,
                width,
                height,
            } => {
                app.attach_image(encoded, mime, raw_size);

                let size_str = if raw_size >= 1024 * 1024 {
                    format!("{:.1} MB", raw_size as f64 / (1024.0 * 1024.0))
                } else if raw_size >= 1024 {
                    format!("{:.1} KB", raw_size as f64 / 1024.0)
                } else {
                    format!("{raw_size} bytes")
                };

                let count = app.pending_images.len();
                app.push_system(
                    format!(
                        "📎 Image attached ({width}×{height}, {size_str}). {count} image{} pending.",
                        if count == 1 { "" } else { "s" }
                    ),
                    false,
                );
            }
            ClipboardResult::Empty(_) => {
                // Nothing to paste — silently ignore
            }
            ClipboardResult::Error(msg) => {
                app.push_system(msg, true);
            }
        }
    }
}

/// Suspend the TUI, open $EDITOR with the current editor content, and load
/// the result back. Falls back to $VISUAL, then `vi`.
pub(crate) fn open_external_editor(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) {
    // Determine which editor to use
    let editor_cmd = std::env::var("EDITOR").or_else(|_| std::env::var("VISUAL")).unwrap_or_else(|_| "vi".to_string());

    // Write current editor content to a temp file
    let current_content = app.editor.content().join("\n");
    let tmp_dir = std::env::temp_dir();
    let tmp_path = tmp_dir.join(format!("clankers-edit-{}.md", std::process::id()));

    if let Err(e) = std::fs::write(&tmp_path, &current_content) {
        app.push_system(format!("Failed to create temp file: {}", e), true);
        return;
    }

    // Suspend the TUI: leave alternate screen, disable raw mode
    execute!(terminal.backend_mut(), DisableBracketedPaste, DisableMouseCapture, LeaveAlternateScreen).ok();
    terminal::disable_raw_mode().ok();

    // Parse the editor command (supports args like "code --wait")
    let mut parts = editor_cmd.split_whitespace();
    let program = parts.next().unwrap_or("vi");
    let extra_args: Vec<&str> = parts.collect();

    // Run the editor
    let result = std::process::Command::new(program)
        .args(&extra_args)
        .arg(&tmp_path)
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .current_dir(&app.cwd)
        .status();

    // Restore the TUI: re-enable raw mode, enter alternate screen
    terminal::enable_raw_mode().ok();
    execute!(terminal.backend_mut(), EnterAlternateScreen, EnableMouseCapture, EnableBracketedPaste).ok();

    // Force a full redraw after returning from the editor
    terminal.clear().ok();

    match result {
        Ok(status) if status.success() => {
            // Read back the edited content
            match std::fs::read_to_string(&tmp_path) {
                Ok(new_content) => {
                    let new_content = new_content.trim_end_matches('\n').to_string();
                    if new_content.is_empty() {
                        app.push_system("Editor returned empty content — input cleared.".to_string(), false);
                        app.editor.clear();
                    } else if new_content == current_content {
                        // No changes — don't bother updating
                    } else {
                        app.editor.clear();
                        for c in new_content.chars() {
                            app.editor.insert_char(c);
                        }
                        app.input_mode = InputMode::Insert;
                    }
                }
                Err(e) => {
                    app.push_system(format!("Failed to read editor output: {}", e), true);
                }
            }
        }
        Ok(status) => {
            app.push_system(format!("Editor exited with status {} — changes discarded.", status), true);
        }
        Err(e) => {
            app.push_system(format!("Failed to launch '{}': {}", editor_cmd, e), true);
        }
    }

    // Clean up temp file
    let _ = std::fs::remove_file(&tmp_path);
}
