//! Markdown-to-HTML conversion and long-response chunking for Matrix messages.

use pulldown_cmark::Options;
use pulldown_cmark::Parser;
use pulldown_cmark::html;

/// Matrix practical message size limit (32 KB).
pub const MAX_MESSAGE_BYTES: usize = 32_768;

/// Convert markdown text to HTML.
///
/// Enables tables, strikethrough, and task lists for rich formatting.
pub fn md_to_html(text: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);

    let parser = Parser::new_ext(text, options);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    html_output
}

/// Split a long markdown response into chunks that fit under `max_bytes`.
///
/// Splits at paragraph boundaries (double newlines) and preserves code blocks.
/// Never splits inside fenced code blocks — a code block stays with its
/// surrounding context. If a single block exceeds `max_bytes`, falls back
/// to splitting at single newline boundaries.
pub fn chunk_response(text: &str, max_bytes: usize) -> Vec<String> {
    if text.len() <= max_bytes {
        return vec![text.to_string()];
    }

    let mut chunks = Vec::new();
    let mut current_chunk = String::new();

    // First pass: split into blocks (paragraphs or code blocks)
    let blocks = split_into_blocks(text);

    for block in blocks {
        let block_size = block.len();

        // If adding this block would exceed the limit, finalize current chunk
        if !current_chunk.is_empty() && current_chunk.len() + block_size + 2 > max_bytes {
            chunks.push(current_chunk.trim().to_string());
            current_chunk.clear();
        }

        // If this single block exceeds the limit, handle it specially
        if block_size > max_bytes {
            // Try to split at line boundaries
            let lines: Vec<&str> = block.split('\n').collect();
            let mut temp = String::new();

            for line in lines {
                if !temp.is_empty() && temp.len() + line.len() + 1 > max_bytes {
                    chunks.push(temp.trim().to_string());
                    temp.clear();
                }

                if line.len() > max_bytes {
                    // Single line too long — truncate it
                    chunks.push(line[..max_bytes].to_string());
                } else {
                    if !temp.is_empty() {
                        temp.push('\n');
                    }
                    temp.push_str(line);
                }
            }

            if !temp.is_empty() {
                if current_chunk.is_empty() {
                    current_chunk = temp;
                } else {
                    chunks.push(current_chunk.trim().to_string());
                    current_chunk = temp;
                }
            }
        } else {
            // Normal case: add block to current chunk
            if !current_chunk.is_empty() {
                current_chunk.push_str("\n\n");
            }
            current_chunk.push_str(&block);
        }
    }

    if !current_chunk.is_empty() {
        chunks.push(current_chunk.trim().to_string());
    }

    if chunks.is_empty() {
        chunks.push(text.to_string());
    }

    chunks
}

/// Split text into blocks (paragraphs or code blocks).
///
/// A code block is a fenced block delimited by triple backticks.
/// Paragraphs are separated by double newlines.
fn split_into_blocks(text: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut current = String::new();
    let mut is_in_code_block = false;
    let mut lines = text.lines().peekable();

    while let Some(line) = lines.next() {
        let trimmed = line.trim();

        // Detect code fence
        if trimmed.starts_with("```") {
            is_in_code_block = !is_in_code_block;
            if !current.is_empty() {
                current.push('\n');
            }
            current.push_str(line);

            // If we just closed a code block, finalize it as its own block
            if !is_in_code_block {
                // Consume a trailing blank line after closing the code block, if any.
                if let Some(next_line) = lines.peek()
                    && next_line.trim().is_empty()
                {
                    lines.next();
                }
                blocks.push(current.trim().to_string());
                current.clear();
            }
            continue;
        }

        if is_in_code_block {
            // Inside code block, keep all lines together
            if !current.is_empty() {
                current.push('\n');
            }
            current.push_str(line);
        } else {
            // Outside code block
            if line.trim().is_empty() {
                // Blank line — check if next line is also blank (paragraph boundary)
                if let Some(next) = lines.peek()
                    && next.trim().is_empty()
                {
                    // Double blank → finalize current block
                    if !current.is_empty() {
                        blocks.push(current.trim().to_string());
                        current.clear();
                    }
                    continue;
                }
                // Single blank within a paragraph
                if !current.is_empty() {
                    current.push('\n');
                }
                current.push_str(line);
            } else {
                // Regular line
                if !current.is_empty() {
                    current.push('\n');
                }
                current.push_str(line);
            }
        }
    }

    if !current.is_empty() {
        blocks.push(current.trim().to_string());
    }

    blocks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_markdown() {
        let md = "# Hello\n\nThis is a paragraph.";
        let html = md_to_html(md);
        assert!(html.contains("<h1>"));
        assert!(html.contains("Hello"));
        assert!(html.contains("<p>"));
        assert!(html.contains("paragraph"));
    }

    #[test]
    fn test_code_block() {
        let md = "```rust\nfn main() {}\n```";
        let html = md_to_html(md);
        assert!(html.contains("<pre>"));
        assert!(html.contains("<code"));
        assert!(html.contains("fn main()"));
    }

    #[test]
    fn test_table() {
        let md = "| A | B |\n|---|---|\n| 1 | 2 |";
        let html = md_to_html(md);
        assert!(html.contains("<table>"));
        assert!(html.contains("<th>"));
        assert!(html.contains("<td>"));
    }

    #[test]
    fn test_chunk_short() {
        let text = "Short message";
        let chunks = chunk_response(text, 1000);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], text);
    }

    #[test]
    fn test_chunk_long() {
        // Create a long text with multiple paragraphs
        let paragraphs: Vec<String> =
            (0..20).map(|i| format!("This is paragraph number {}. It has some text in it.", i)).collect();
        let text = paragraphs.join("\n\n");

        let chunks = chunk_response(&text, 200);
        assert!(chunks.len() > 1);

        // Verify each chunk is under the limit
        for chunk in &chunks {
            assert!(chunk.len() <= 200);
        }

        // Verify we didn't lose content
        let rejoined = chunks.join("\n\n");
        assert!(rejoined.contains("paragraph number 0"));
        assert!(rejoined.contains("paragraph number 19"));
    }

    #[test]
    fn test_chunk_preserves_code_blocks() {
        let text = "Before code\n\n```rust\nfn main() {\n    println!(\"hello\");\n}\n```\n\nAfter code";
        let chunks = chunk_response(text, 100);

        // The code block should not be split
        let rejoined = chunks.join("\n\n");
        assert!(rejoined.contains("```rust"));
        assert!(rejoined.contains("fn main()"));
        assert!(rejoined.contains("```"));

        // Ensure the code fence markers appear together in one chunk
        let code_block_intact = chunks
            .iter()
            .any(|chunk| chunk.contains("```rust") && chunk.contains("fn main()") && chunk.matches("```").count() == 2);
        assert!(code_block_intact, "Code block should remain intact in a single chunk");
    }

    #[test]
    fn test_strikethrough() {
        let md = "This is ~~deleted~~ text.";
        let html = md_to_html(md);
        assert!(html.contains("<del>") || html.contains("<s>"));
    }

    #[test]
    fn test_task_list() {
        let md = "- [ ] Todo\n- [x] Done";
        let html = md_to_html(md);
        assert!(html.contains("checkbox"));
    }
}
