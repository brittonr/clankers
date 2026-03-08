//! Screenshot tool — captures the terminal/screen for visual validation
//!
//! Supports multiple capture methods:
//! - `screen`: Full screen capture via grim (Wayland) or import (X11)
//! - `window`: Focused window capture
//! - `region`: Specific pixel region
//!
//! Returns the image as base64 in the tool result, which the model can see.

use std::path::Path;
use std::path::PathBuf;

use async_trait::async_trait;
use base64::Engine as _;
use base64::engine::general_purpose;
use serde_json::Value;
use serde_json::json;
use tokio::process::Command;

use crate::tools::Tool;
use crate::tools::ToolContext;
use crate::tools::ToolDefinition;
use crate::tools::ToolResult;
use crate::tools::ToolResultContent;

pub struct ScreenshotTool {
    definition: ToolDefinition,
}

impl ScreenshotTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                name: "screenshot".to_string(),
                description: "Capture a screenshot of the screen or terminal window. Returns the image for visual inspection. Useful for validating TUI rendering, checking command output appearance, or visual debugging.\n\nThe image is automatically scaled down for efficiency.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "target": {
                            "type": "string",
                            "enum": ["screen", "window", "region"],
                            "description": "What to capture. 'screen' = full screen, 'window' = focused window, 'region' = specific coordinates. Defaults to 'screen'."
                        },
                        "region": {
                            "type": "string",
                            "description": "Region to capture as 'X,Y WxH' (e.g. '100,200 800x600'). Only used with target='region'."
                        },
                        "scale": {
                            "type": "number",
                            "description": "Scale factor 0.0-1.0 (default 0.5). Smaller = faster but less detail."
                        },
                        "delay": {
                            "type": "number",
                            "description": "Delay in seconds before capturing (default 0). Useful to let animations settle."
                        }
                    }
                }),
            },
        }
    }
}

impl Default for ScreenshotTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for ScreenshotTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, ctx: &ToolContext, params: Value) -> ToolResult {
        let target = params.get("target").and_then(|v| v.as_str()).unwrap_or("screen");
        let region = params.get("region").and_then(|v| v.as_str());
        let scale = params.get("scale").and_then(|v| v.as_f64()).unwrap_or(0.5);
        let delay = params.get("delay").and_then(|v| v.as_f64()).unwrap_or(0.0);

        // Clamp scale
        let scale = scale.clamp(0.1, 1.0);

        // Optional delay
        if delay > 0.0 {
            tokio::select! {
                () = tokio::time::sleep(std::time::Duration::from_secs_f64(delay)) => {}
                () = ctx.signal.cancelled() => {
                    return ToolResult::error("Cancelled during delay");
                }
            }
        }

        // Detect capture backend
        let backend = detect_backend().await;
        ctx.emit_progress(&format!("capturing {} via {:?}", target, backend));

        // Capture to temp file
        let tmp_path = std::env::temp_dir().join(format!("clankers-screenshot-{}.png", std::process::id()));

        let capture_result = match backend {
            Backend::Grim => capture_grim(&tmp_path, target, region).await,
            Backend::Import => capture_import(&tmp_path, target, region).await,
            Backend::None => Err("No screenshot tool found. Install 'grim' (Wayland) or 'imagemagick' (X11).".into()),
        };

        if let Err(e) = capture_result {
            let _ = tokio::fs::remove_file(&tmp_path).await;
            return ToolResult::error(format!("Screenshot capture failed: {}", e));
        }

        if ctx.signal.is_cancelled() {
            let _ = tokio::fs::remove_file(&tmp_path).await;
            return ToolResult::error("Cancelled");
        }

        // Scale down if needed
        ctx.emit_progress(&format!("scaling to {:.0}%", scale * 100.0));
        let output_path = if (scale - 1.0).abs() > 0.01 {
            let scaled_path =
                std::env::temp_dir().join(format!("clankers-screenshot-{}-scaled.jpg", std::process::id()));
            match scale_image(&tmp_path, &scaled_path, scale).await {
                Ok(()) => {
                    let _ = tokio::fs::remove_file(&tmp_path).await;
                    scaled_path
                }
                Err(_) => {
                    // Scaling failed, use original
                    tmp_path.clone()
                }
            }
        } else {
            tmp_path.clone()
        };

        // Read and base64-encode
        let image_data = match tokio::fs::read(&output_path).await {
            Ok(data) => data,
            Err(e) => {
                let _ = tokio::fs::remove_file(&output_path).await;
                let _ = tokio::fs::remove_file(&tmp_path).await;
                return ToolResult::error(format!("Failed to read screenshot: {}", e));
            }
        };

        let _ = tokio::fs::remove_file(&output_path).await;
        if output_path != tmp_path {
            let _ = tokio::fs::remove_file(&tmp_path).await;
        }

        let media_type = if output_path.extension().and_then(|e| e.to_str()) == Some("jpg") {
            "image/jpeg"
        } else {
            "image/png"
        };

        let b64 = general_purpose::STANDARD.encode(&image_data);
        let size_kb = image_data.len() / 1024;

        ToolResult {
            content: vec![
                ToolResultContent::Text {
                    text: format!("Screenshot captured ({} KB, {}, scale {:.0}%)", size_kb, target, scale * 100.0),
                },
                ToolResultContent::Image {
                    media_type: media_type.to_string(),
                    data: b64,
                },
            ],
            is_error: false,
            details: None,
            full_output_path: None,
        }
    }
}

// ── Backend detection ───────────────────────────────────────────────

#[derive(Debug)]
enum Backend {
    Grim,
    Import,
    None,
}

async fn detect_backend() -> Backend {
    // Prefer grim on Wayland
    if std::env::var("WAYLAND_DISPLAY").is_ok() && which("grim").await {
        return Backend::Grim;
    }

    // Fall back to ImageMagick import (works on X11)
    if which("import").await {
        return Backend::Import;
    }

    // Try grim anyway (might work via XWayland)
    if which("grim").await {
        return Backend::Grim;
    }

    Backend::None
}

async fn which(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await
        .map(|s| s.success())
        .unwrap_or(false)
}

// ── Capture backends ────────────────────────────────────────────────

async fn capture_grim(path: &PathBuf, target: &str, region: Option<&str>) -> Result<(), String> {
    let mut cmd = Command::new("grim");

    match target {
        "region" => {
            if let Some(r) = region {
                cmd.arg("-g").arg(r);
            } else {
                return Err("'region' target requires a 'region' parameter (e.g. '100,200 800x600')".into());
            }
        }
        "window" => {
            // Try to get focused window geometry from niri or sway
            if let Some(geom) = get_focused_window_geometry().await {
                cmd.arg("-g").arg(&geom);
            }
            // If we can't get geometry, just capture the full screen
        }
        _ => {
            // "screen" — capture the whole output
        }
    }

    cmd.arg(path);

    let output = cmd.output().await.map_err(|e| format!("grim failed to start: {}", e))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("grim failed: {}", stderr));
    }
    Ok(())
}

async fn capture_import(path: &PathBuf, target: &str, region: Option<&str>) -> Result<(), String> {
    let mut cmd = Command::new("import");

    match target {
        "window" => {
            // import -window root captures full screen; -window "$(xdotool getactivewindow)" for focused
            cmd.arg("-window").arg("root");
        }
        "region" => {
            if let Some(r) = region {
                // import uses -crop WxH+X+Y format
                cmd.arg("-crop").arg(r);
            }
            cmd.arg("-window").arg("root");
        }
        _ => {
            cmd.arg("-window").arg("root");
        }
    }

    cmd.arg(path);

    let output = cmd.output().await.map_err(|e| format!("import failed to start: {}", e))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("import failed: {}", stderr));
    }
    Ok(())
}

// ── Window geometry helpers ─────────────────────────────────────────

async fn get_focused_window_geometry() -> Option<String> {
    // Try niri first
    if let Some(geom) = get_niri_focused_geometry().await {
        return Some(geom);
    }
    // Try swaymsg
    if let Some(geom) = get_sway_focused_geometry().await {
        return Some(geom);
    }
    None
}

async fn get_niri_focused_geometry() -> Option<String> {
    let output = Command::new("niri").args(["msg", "-j", "focused-window"]).output().await.ok()?;

    if !output.status.success() {
        return None;
    }

    let json: Value = serde_json::from_slice(&output.stdout).ok()?;

    // We need the window position on screen. niri gives us tile size but not absolute position.
    // Use niri msg -j windows to find the focused window's position,
    // or fall back to tile_pos_in_workspace_view if available.
    let layout = json.get("layout")?;

    // tile_pos_in_workspace_view gives [x, y] when visible
    if let Some(pos) = layout.get("tile_pos_in_workspace_view").and_then(|v| v.as_array())
        && pos.len() == 2
    {
        let x = pos[0].as_f64()?;
        let y = pos[1].as_f64()?;
        let size = layout.get("tile_size")?.as_array()?;
        let w = size[0].as_f64()?;
        let h = size[1].as_f64()?;
        return Some(format!("{},{} {}x{}", x as i64, y as i64, w as i64, h as i64));
    }

    // Fallback: just use tile size from origin (not ideal but better than full screen)
    let size = layout.get("tile_size")?.as_array()?;
    let w = size[0].as_f64()?;
    let h = size[1].as_f64()?;
    Some(format!("0,0 {}x{}", w as i64, h as i64))
}

async fn get_sway_focused_geometry() -> Option<String> {
    let output = Command::new("swaymsg").args(["-t", "get_tree"]).output().await.ok()?;

    if !output.status.success() {
        return None;
    }

    // Find focused window in the tree
    let tree: Value = serde_json::from_slice(&output.stdout).ok()?;
    find_sway_focused(&tree)
}

fn find_sway_focused(node: &Value) -> Option<String> {
    if node.get("focused").and_then(|v| v.as_bool()) == Some(true) {
        let rect = node.get("rect")?;
        let x = rect.get("x")?.as_i64()?;
        let y = rect.get("y")?.as_i64()?;
        let w = rect.get("width")?.as_i64()?;
        let h = rect.get("height")?.as_i64()?;
        return Some(format!("{},{} {}x{}", x, y, w, h));
    }

    if let Some(nodes) = node.get("nodes").and_then(|v| v.as_array()) {
        for n in nodes {
            if let Some(geom) = find_sway_focused(n) {
                return Some(geom);
            }
        }
    }
    if let Some(nodes) = node.get("floating_nodes").and_then(|v| v.as_array()) {
        for n in nodes {
            if let Some(geom) = find_sway_focused(n) {
                return Some(geom);
            }
        }
    }
    None
}

// ── Image scaling ───────────────────────────────────────────────────

async fn scale_image(input: &Path, output: &Path, scale: f64) -> Result<(), String> {
    let pct = format!("{}%", (scale * 100.0) as u32);

    // Try magick (ImageMagick v7) first, fall back to convert (v6)
    let result = Command::new("magick")
        .args([
            input.to_str().unwrap_or(""),
            "-resize",
            &pct,
            "-quality",
            "85",
            output.to_str().unwrap_or(""),
        ])
        .output()
        .await;

    match result {
        Ok(o) if o.status.success() => return Ok(()),
        _ => {}
    }

    // Fall back to convert
    let result = Command::new("convert")
        .args([
            input.to_str().unwrap_or(""),
            "-resize",
            &pct,
            "-quality",
            "85",
            output.to_str().unwrap_or(""),
        ])
        .output()
        .await;

    match result {
        Ok(o) if o.status.success() => Ok(()),
        Ok(o) => Err(format!("convert failed: {}", String::from_utf8_lossy(&o.stderr))),
        Err(e) => Err(format!("No image scaling tool available: {}", e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_definition_is_valid() {
        let tool = ScreenshotTool::new();
        let def = tool.definition();
        assert_eq!(def.name, "screenshot");
        assert!(def.description.contains("screenshot"));
    }

    #[tokio::test]
    async fn detect_backend_finds_something() {
        // On this system we should find grim or import
        let backend = detect_backend().await;
        assert!(!matches!(backend, Backend::None), "Should find a screenshot backend");
    }
}
