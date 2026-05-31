//! Syntax validation pass — warns about patterns that are legal to parse but semantically dubious.
//!
//! Adapted from `SyntaxValidationListener.cs` in
//! <https://github.com/YarnSpinnerTool/YarnSpinner/blob/v3.2.1/YarnSpinner.Compiler/SyntaxValidationListener.cs>
//!
//! Implements the following diagnostic codes:
//! - **YS0021** `StrayCommandEnd` — trailing `>>` without a matching `<<` on a text line
//! - **YS0022** `UnenclosedCommand` — `set`/`declare`/`jump`/`detour` keyword outside `<< >>`
//! - **YS0048** `SingularCommandWrap` — text wrapped in single `<>` angle brackets
//! - **YS0020** `CommandFollowingLine` — text before a non-flow-control command
//! - **YS0019** `LineContentAfterCommand`  — text after a non-flow-control command

use crate::prelude::*;
use yarnspinner_core::prelude::Position;

/// Command names that control program flow.  Lines where these are the only
/// command on the line are *not* flagged for mixed text content.
const FLOW_CONTROL_COMMANDS: &[&str] = &["if", "elseif", "else", "endif", "once", "endonce"];

pub(crate) fn validate_syntax(mut state: CompilationIntermediate) -> CompilationIntermediate {
    for file in &state.job.files {
        let new_diagnostics = check_file_syntax(&file.source, &file.file_name);
        state.diagnostics.extend(new_diagnostics);
    }
    state
}

// ---------------------------------------------------------------------------
// File-level entry point
// ---------------------------------------------------------------------------

fn check_file_syntax(source: &str, file_name: &str) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut in_node_body = false;

    for (line_num, line) in source.lines().enumerate() {
        let trimmed = line.trim();

        // Track node body boundaries
        if trimmed == "---" {
            in_node_body = true;
            continue;
        }
        if trimmed == "===" {
            in_node_body = false;
            continue;
        }

        if !in_node_body {
            continue;
        }

        // Skip blank lines, comments, and option lines
        if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with("->") {
            continue;
        }

        if trimmed.starts_with("<<") {
            // Command lines: check for extra leading chevrons (<<<command>>)  → YS0064
            check_rogue_leading_chevrons(trimmed, line_num, file_name, &mut diagnostics);
            // Also check for text content after the command (<<cmd>> text) → YS0019,
            // and rogue trailing chevrons (<<cmd>>>) → YS0064.
            check_text_mixed_with_command(line, line_num, file_name, &mut diagnostics);
        } else if trimmed.contains("<<") {
            // Text line with embedded command blocks — check for text mixed with commands
            // (YS0019/YS0020). The `<<` is NOT at the start, so this is genuine mixed content.
            check_text_mixed_with_command(line, line_num, file_name, &mut diagnostics);
        } else {
            // Pure text line (no `<<` blocks) — check for malformed chevron patterns
            // (YS0021/YS0048) and unenclosed command keywords (YS0022).
            check_malformed_text_line(line, trimmed, line_num, file_name, &mut diagnostics);
        }
    }

    diagnostics
}

// ---------------------------------------------------------------------------
// YS0021 / YS0048 / YS0022 — Malformed patterns on pure-text lines
// ---------------------------------------------------------------------------

/// Checks a line that contains no `<<` blocks for chevron-pattern diagnostics
/// and unenclosed command keywords.  Matches `CheckMalformedCommandsInText` from
/// the C# `SyntaxValidationListener`.
fn check_malformed_text_line(line: &str, trimmed: &str, line_num: usize, file_name: &str, diagnostics: &mut Vec<Diagnostic>) {
    // The column at which `trimmed` starts within `line` (leading whitespace length).
    let trim_start_col = line.len() - line.trim_start().len();

    let leading = trimmed.chars().take_while(|&c| c == '<').count();
    let trailing = trimmed.chars().rev().take_while(|&c| c == '>').count();

    // YS0048 — `<text>` wrapped in single angle brackets.
    if leading == 1 && trailing == 1 {
        diagnostics.push(
            Diagnostic::from_message(format!(
                "Line {trimmed} has single '<' and '>' wrapping it. Did you mean to make this a command?"
            ))
            .with_file_name(file_name)
            .with_range(
                Position {
                    line: line_num,
                    character: trim_start_col,
                }..Position {
                    line: line_num,
                    character: trim_start_col + trimmed.len(),
                },
            )
            .with_severity(DiagnosticSeverity::Warning)
            .with_code("YS0048"),
        );
        return;
    }

    // YS0021 — trailing `>>` on a text line (`text>>` or `<text>>`).
    if (leading == 0 || leading == 1) && trailing == 2 {
        let end_col = trim_start_col + trimmed.len();
        let start_col = end_col - 2;
        diagnostics.push(
            Diagnostic::from_message("Stray '>>' without matching '<<'. Did you forget to open the command?")
                .with_file_name(file_name)
                .with_range(
                    Position {
                        line: line_num,
                        character: start_col,
                    }..Position {
                        line: line_num,
                        character: end_col,
                    },
                )
                .with_severity(DiagnosticSeverity::Warning)
                .with_code("YS0021"),
        );
        return;
    }

    // No chevron anomalies — check for unenclosed command keywords (YS0022).
    if leading + trailing == 0 {
        check_unenclosed_command(trimmed, line_num, file_name, diagnostics);
    }
}

// ---------------------------------------------------------------------------
// YS0022 — Unenclosed `set`/`declare`/`jump`/`detour` keyword
// ---------------------------------------------------------------------------

fn check_unenclosed_command(trimmed: &str, line_num: usize, file_name: &str, diagnostics: &mut Vec<Diagnostic>) {
    // The line must start exactly with the keyword (not embedded in text like "I declare …").
    for &keyword in &["set", "declare", "jump", "detour"] {
        let Some(rest) = trimmed.strip_prefix(keyword) else {
            continue;
        };
        // Keyword must be followed by whitespace (distinguishes "set" from "setter", etc.)
        if !rest.starts_with(|c: char| c.is_whitespace()) {
            continue;
        }
        let after = rest.trim_start();

        let is_valid_target = match keyword {
            // `set`/`declare` expect a variable starting with `$`
            "set" | "declare" => after.starts_with('$'),
            // `jump`/`detour` expect a valid identifier (letter or `_`, not a raw number)
            "jump" | "detour" => after.starts_with(|c: char| c.is_alphabetic() || c == '_'),
            _ => false,
        };

        if is_valid_target {
            diagnostics.push(
                Diagnostic::from_message(format!(
                    "'{keyword}' command must be enclosed in '<<' and '>>'. Did you mean '<<{keyword} ...'?"
                ))
                .with_file_name(file_name)
                .with_range(
                    Position {
                        line: line_num,
                        character: 0,
                    }..Position {
                        line: line_num,
                        character: keyword.len(),
                    },
                )
                .with_severity(DiagnosticSeverity::Warning)
                .with_code("YS0022"),
            );
            // Only report one keyword per line.
            break;
        }
    }
}

// ---------------------------------------------------------------------------
// YS0019 / YS0020 — Text mixed with a non-flow-control command
// ---------------------------------------------------------------------------

fn check_text_mixed_with_command(line: &str, line_num: usize, file_name: &str, diagnostics: &mut Vec<Diagnostic>) {
    // Walk the line tracking whether non-whitespace text appears before/after
    // a non-flow-control command block (`<< ... >>`).
    let mut has_text_before = false;
    let mut has_text_after = false;
    // Track what kind of non-whitespace chars follow the last command:
    // `after_only_chevrons` is true while every non-whitespace char seen after
    // the last command has been `>`.  Used to distinguish YS0064 from YS0019.
    let mut after_only_chevrons = true;
    let mut first_cmd: Option<String> = None;
    let mut last_cmd: Option<String> = None;

    let mut chars = line.char_indices().peekable();
    while let Some((_i, ch)) = chars.next() {
        if ch == '\\' {
            // Escape: the next character is escaped (e.g. \<< becomes literal text).
            // Consume both the backslash and the escaped char, treating them as text.
            chars.next();
            continue;
        }
        if ch == '<' && matches!(chars.peek(), Some((_, '<'))) {
            // Consume second `<`
            chars.next();

            // Read full command text up to `>>` (including spaces — e.g. "after command")
            let mut buf = String::new();
            loop {
                match chars.peek() {
                    Some((_, '>')) | None => break,
                    Some(&(_j, c)) => {
                        buf.push(c);
                        chars.next();
                    }
                }
            }

            // Skip to closing `>>`
            loop {
                match chars.peek() {
                    None => break,
                    Some((_, '>')) => {
                        chars.next();
                        if matches!(chars.peek(), Some((_, '>'))) {
                            chars.next(); // consume second `>`
                            break;
                        }
                        // Single `>` inside command (e.g. comparison operator) — keep scanning.
                    }
                    Some(_) => {
                        chars.next();
                    }
                }
            }

            let cmd_name = buf.trim().to_owned();
            // Use only the first word for flow-control checks; the full name goes into messages.
            let cmd_first_word = cmd_name.split_whitespace().next().unwrap_or(cmd_name.as_str());
            if cmd_name.is_empty() || FLOW_CONTROL_COMMANDS.contains(&cmd_first_word) {
                continue;
            }

            if first_cmd.is_none() {
                first_cmd = Some(cmd_name.clone());
            }
            last_cmd = Some(cmd_name);
            has_text_after = false; // reset: track text after the *last* command
            after_only_chevrons = true; // reset for each new command
        } else if !ch.is_whitespace() {
            if first_cmd.is_none() {
                has_text_before = true;
            } else {
                has_text_after = true;
                if ch != '>' {
                    after_only_chevrons = false;
                }
            }
        }
    }

    let Some(first_cmd_name) = first_cmd else {
        return;
    };
    let last_cmd_name = last_cmd.unwrap_or_else(|| first_cmd_name.clone());

    let full_range = Position {
        line: line_num,
        character: 0,
    }..Position {
        line: line_num,
        character: line.len(),
    };

    // YS0020: text content BEFORE first non-flow command (error)
    if has_text_before {
        diagnostics.push(
            Diagnostic::from_message(format!(
                "Command \"{first_cmd_name}\" found following a line of dialogue. Commands should start on a new line."
            ))
            .with_file_name(file_name)
            .with_range(full_range.clone())
            .with_severity(DiagnosticSeverity::Error)
            .with_code("YS0020"),
        );
    }

    // YS0019 / YS0064: text content AFTER last non-flow command
    if has_text_after {
        // If all non-whitespace chars after the command are `>`, it's YS0064
        // (rogue chevrons), not YS0019.
        if after_only_chevrons {
            diagnostics.push(
                Diagnostic::from_message(format!(
                    "Stray '>' found following command '{last_cmd_name}'. Did you accidentally add extra chevrons?"
                ))
                .with_file_name(file_name)
                .with_range(full_range)
                .with_severity(DiagnosticSeverity::Warning)
                .with_code("YS0064"),
            );
        } else {
            diagnostics.push(
                Diagnostic::from_message(format!(
                    "Dialogue \"{last_cmd_name}\" content found following a command. Commands should be on their own line."
                ))
                .with_file_name(file_name)
                .with_range(full_range)
                .with_severity(DiagnosticSeverity::Warning)
                .with_code("YS0019"),
            );
        }
    }
}

// ---------------------------------------------------------------------------
// YS0064 — Extra leading chevrons before a command (e.g. `<<<set $x to 1>>`)
// ---------------------------------------------------------------------------

fn check_rogue_leading_chevrons(trimmed: &str, line_num: usize, file_name: &str, diagnostics: &mut Vec<Diagnostic>) {
    // Count leading `<` characters; a valid command starts with exactly `<<`.
    let leading = trimmed.chars().take_while(|&c| c == '<').count();
    if leading > 2 {
        // The command text is everything after the leading `<`s, trimmed of enclosing `<< >>`
        let inner = trimmed.trim_start_matches('<');
        let cmd_name = inner.split_whitespace().next().unwrap_or("").trim_end_matches('>');
        diagnostics.push(
            Diagnostic::from_message(format!(
                "Stray '<' found before command '{cmd_name}'. Did you accidentally add extra chevrons?"
            ))
            .with_file_name(file_name)
            .with_range(
                Position {
                    line: line_num,
                    character: 0,
                }..Position {
                    line: line_num,
                    character: leading - 2,
                },
            )
            .with_severity(DiagnosticSeverity::Warning)
            .with_code("YS0064"),
        );
    }
}
