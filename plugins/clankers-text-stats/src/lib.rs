//! clankers-text-stats plugin — Text analysis utilities.
//!
//! Provides two tools for the clankers agent:
//!
//! - **`text_stats`** — Compute word/char/line/sentence counts and readability.
//! - **`char_freq`** — Character frequency distribution analysis.

use clanker_plugin_sdk::prelude::*;
use std::collections::BTreeMap;

// ═══════════════════════════════════════════════════════════════════════
//  Extism guest functions
// ═══════════════════════════════════════════════════════════════════════

#[plugin_fn]
pub fn handle_tool_call(input: String) -> FnResult<String> {
    dispatch_tools(&input, &[
        ("text_stats", handle_text_stats),
        ("char_freq", handle_char_freq),
    ])
}

#[plugin_fn]
pub fn on_event(input: String) -> FnResult<String> {
    dispatch_events(&input, "clankers-text-stats", &[
        ("agent_start", |_| "clankers-text-stats plugin initialized".to_string()),
        ("agent_end", |_| "clankers-text-stats plugin shutting down".to_string()),
    ])
}

#[plugin_fn]
pub fn describe(Json(_): Json<()>) -> FnResult<Json<PluginMeta>> {
    Ok(Json(PluginMeta::new("clankers-text-stats", "0.1.0", &[
        ("text_stats", "Compute text statistics (words, chars, lines, readability)"),
        ("char_freq", "Character frequency distribution analysis"),
    ], &[])))
}

// ═══════════════════════════════════════════════════════════════════════
//  Tool implementations
// ═══════════════════════════════════════════════════════════════════════

#[derive(Serialize)]
struct TextStats {
    characters: usize,
    characters_no_spaces: usize,
    words: usize,
    lines: usize,
    sentences: usize,
    paragraphs: usize,
    avg_word_length: f64,
    avg_sentence_length: f64,
    flesch_kincaid_grade: f64,
    flesch_reading_ease: f64,
}

fn handle_text_stats(args: &Value) -> Result<String, String> {
    let text = args.require_str("text")?;

    let characters = text.chars().count();
    let characters_no_spaces = text.chars().filter(|c| !c.is_whitespace()).count();
    let words = count_words(text);
    let lines = if text.is_empty() { 0 } else { text.lines().count() };
    let sentences = count_sentences(text);
    let paragraphs = count_paragraphs(text);
    let syllables = count_syllables_total(text);

    let avg_word_length = if words > 0 {
        characters_no_spaces as f64 / words as f64
    } else {
        0.0
    };

    let avg_sentence_length = if sentences > 0 {
        words as f64 / sentences as f64
    } else {
        0.0
    };

    let avg_syllables_per_word = if words > 0 {
        syllables as f64 / words as f64
    } else {
        0.0
    };

    let flesch_kincaid_grade = if words > 0 && sentences > 0 {
        0.39 * avg_sentence_length + 11.8 * avg_syllables_per_word - 15.59
    } else {
        0.0
    };

    let flesch_reading_ease = if words > 0 && sentences > 0 {
        206.835 - 1.015 * avg_sentence_length - 84.6 * avg_syllables_per_word
    } else {
        0.0
    };

    let stats = TextStats {
        characters,
        characters_no_spaces,
        words,
        lines,
        sentences,
        paragraphs,
        avg_word_length: round2(avg_word_length),
        avg_sentence_length: round2(avg_sentence_length),
        flesch_kincaid_grade: round2(flesch_kincaid_grade),
        flesch_reading_ease: round2(flesch_reading_ease),
    };

    clanker_plugin_sdk::serde_json::to_string_pretty(&stats)
        .map_err(|e| format!("serialization error: {e}"))
}

#[derive(Serialize)]
struct CharFreqEntry {
    character: String,
    display: String,
    count: usize,
    percentage: f64,
}

fn handle_char_freq(args: &Value) -> Result<String, String> {
    let text = args.require_str("text")?;
    let top_n = args.get_u64_or("top_n", 10) as usize;
    let ignore_whitespace = args.get_bool_or("ignore_whitespace", false);

    let mut freq: BTreeMap<char, usize> = BTreeMap::new();
    let mut total: usize = 0;

    for ch in text.chars() {
        if ignore_whitespace && ch.is_whitespace() {
            continue;
        }
        *freq.entry(ch).or_insert(0) += 1;
        total += 1;
    }

    let mut entries: Vec<(char, usize)> = freq.into_iter().collect();
    entries.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    entries.truncate(top_n);

    let result: Vec<CharFreqEntry> = entries
        .into_iter()
        .map(|(ch, count)| {
            let percentage = if total > 0 {
                round2(count as f64 / total as f64 * 100.0)
            } else {
                0.0
            };
            let display = match ch {
                ' ' => "SPACE".to_string(),
                '\n' => "NEWLINE".to_string(),
                '\t' => "TAB".to_string(),
                '\r' => "CR".to_string(),
                c => c.to_string(),
            };
            CharFreqEntry {
                character: ch.to_string(),
                display,
                count,
                percentage,
            }
        })
        .collect();

    clanker_plugin_sdk::serde_json::to_string_pretty(&result)
        .map_err(|e| format!("serialization error: {e}"))
}

// ═══════════════════════════════════════════════════════════════════════
//  Text analysis helpers
// ═══════════════════════════════════════════════════════════════════════

fn count_words(text: &str) -> usize {
    text.split_whitespace().count()
}

fn count_sentences(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }
    let mut count = 0;
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '.' || chars[i] == '!' || chars[i] == '?' {
            while i + 1 < chars.len()
                && (chars[i + 1] == '.' || chars[i + 1] == '!' || chars[i + 1] == '?')
            {
                i += 1;
            }
            count += 1;
        }
        i += 1;
    }
    if count == 0 && count_words(text) > 0 {
        count = 1;
    }
    count
}

fn count_paragraphs(text: &str) -> usize {
    if text.trim().is_empty() {
        return 0;
    }
    let mut count = 0;
    let mut in_paragraph = false;
    for line in text.lines() {
        if line.trim().is_empty() {
            if in_paragraph {
                in_paragraph = false;
            }
        } else if !in_paragraph {
            in_paragraph = true;
            count += 1;
        }
    }
    count
}

fn count_syllables_total(text: &str) -> usize {
    text.split_whitespace()
        .map(count_syllables_word)
        .sum()
}

fn count_syllables_word(word: &str) -> usize {
    let word = word.to_lowercase();
    let chars: Vec<char> = word.chars().filter(|c| c.is_alphabetic()).collect();
    if chars.is_empty() {
        return 0;
    }
    if chars.len() <= 3 {
        return 1;
    }
    let vowels = ['a', 'e', 'i', 'o', 'u', 'y'];
    let mut count = 0;
    let mut prev_vowel = false;
    for &ch in &chars {
        let is_vowel = vowels.contains(&ch);
        if is_vowel && !prev_vowel {
            count += 1;
        }
        prev_vowel = is_vowel;
    }
    if chars.len() > 2 && chars[chars.len() - 1] == 'e' {
        let prev = chars[chars.len() - 2];
        if prev != 'l' && !vowels.contains(&prev) && count > 1 {
            count -= 1;
        }
    }
    if count == 0 {
        count = 1;
    }
    count
}

fn round2(val: f64) -> f64 {
    (val * 100.0).round() / 100.0
}
