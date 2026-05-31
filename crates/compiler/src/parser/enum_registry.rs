//! Pre-scanner that builds an enum registry from Yarn source text before tokenisation.
//!
//! This is needed because the v2.5 ANTLR grammar has no rules for `<<enum>>`,
//! `<<case>>`, or `<<endenum>>`, so we handle those commands as COMMAND_TEXT
//! no-ops.  However, the *expressions* inside `<<if>>` / `<<set>>` /
//! `<<declare>>` blocks may reference enum members (e.g. `Food.Apple` or the
//! shorthand `.Apple`), which the lexer must convert to their raw values
//! *before* the parser sees them.  The registry supplies those raw values.

use std::collections::HashMap;

/// A source range for an enum block error: (start_line_0based, start_col, end_line_0based, end_col).
pub(crate) type EnumBlockRange = (usize, usize, usize, usize);

/// A value that a single enum case resolves to.
#[derive(Debug, Clone)]
pub(crate) enum EnumCaseValue {
    Number(f64),
    Str(String),
}

/// Whether an enum's raw values are all numbers or all strings.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum EnumRawType {
    Number,
    String,
}

/// A single enum definition.
#[derive(Debug, Clone)]
pub(crate) struct EnumDef {
    #[allow(dead_code)]
    pub raw_type: EnumRawType,
    /// (case_name, resolved_value) in declaration order.
    pub cases: Vec<(String, EnumCaseValue)>,
}

/// Mapping from enum type name → definition.
#[derive(Debug, Clone, Default)]
pub(crate) struct EnumRegistry {
    pub enums: HashMap<String, EnumDef>,
}

impl EnumRegistry {
    /// Scan `source` for `<<enum>>` / `<<case>>` / `<<endenum>>` blocks and
    /// build the registry.  Any structural errors (empty enum, duplicate
    /// cases, mixed raw-value types, etc.) are returned as `(message, range)`
    /// pairs where `range` is `(start_line, start_col, end_line, end_col)` in
    /// 0-based coordinates so that the caller can emit proper `Diagnostic` objects.
    ///
    /// For blocks that contain non-constant raw case values, a **single** error
    /// is emitted covering the entire `<<enum>>…<<endenum>>` block, with
    /// the message `"Expected a constant type"`.
    pub fn from_source(source: &str) -> (Self, Vec<(String, EnumBlockRange)>) {
        let mut registry = EnumRegistry::default();
        let mut errors: Vec<(String, EnumBlockRange)> = Vec::new();

        struct InProgress {
            name: String,
            cases: Vec<(String, Option<EnumCaseValue>)>,
            /// True when at least one case had a non-constant raw value.
            had_non_constant: bool,
            /// 0-based line index of the `<<enum ...>>` opener.
            start_line_0: usize,
            /// Column of the `<<enum ...>>` opener (always 0 for trimmed lines).
            start_col: usize,
        }

        let mut current: Option<InProgress> = None;

        for (idx, line) in source.lines().enumerate() {
            let line_num_1 = idx + 1; // 1-based (kept for legacy compat where needed)
            let trimmed = line.trim();

            // Extract the contents of the first <<...>> on this line.
            let Some(content) = trimmed.strip_prefix("<<").and_then(|s| s.split_once(">>").map(|(c, _)| c.trim())) else {
                continue;
            };

            if let Some(name) = content.strip_prefix("enum ") {
                let name = name.trim().to_string();
                // Find the column where "<<" starts on this line.
                let start_col = line.find("<<").unwrap_or(0);
                current = Some(InProgress {
                    name,
                    cases: Vec::new(),
                    had_non_constant: false,
                    start_line_0: idx,
                    start_col,
                });
            } else if content == "endenum" {
                let Some(en) = current.take() else {
                    continue;
                };
                // End position: end of `<<endenum>>` on this line.
                let endenum_start = line.find("<<").unwrap_or(0);
                // "<<endenum>>" is 11 chars; end col is exclusive.
                let end_col = endenum_start + "<<endenum>>".len();

                // Non-constant raw value → emit ONE error for the whole block.
                if en.had_non_constant {
                    errors.push(("Expected a constant type".to_string(), (en.start_line_0, en.start_col, idx, end_col)));
                    continue;
                }

                if en.cases.is_empty() {
                    errors.push((
                        format!("Enum '{}' must not be empty", en.name),
                        (en.start_line_0, en.start_col, idx, end_col),
                    ));
                    continue;
                }

                // Categorise the explicit raw value types present.
                let has_string = en.cases.iter().any(|(_, v)| matches!(v, Some(EnumCaseValue::Str(_))));
                let has_number = en.cases.iter().any(|(_, v)| matches!(v, Some(EnumCaseValue::Number(_))));

                if has_string && has_number {
                    errors.push((
                        format!("Enum '{}': raw values must all be of the same type", en.name),
                        (en.start_line_0, en.start_col, idx, end_col),
                    ));
                    continue;
                }

                // If any case has a string raw value, ALL must have one.
                if has_string && en.cases.iter().any(|(_, v)| v.is_none()) {
                    errors.push((
                        format!("Enum '{}': all cases must have a string raw value if any do", en.name),
                        (en.start_line_0, en.start_col, idx, end_col),
                    ));
                    continue;
                }

                // Duplicate case names.
                let mut seen_names: std::collections::HashSet<&str> = std::collections::HashSet::new();
                let mut dup_name = false;
                for (n, _) in &en.cases {
                    if !seen_names.insert(n.as_str()) {
                        errors.push((
                            format!("Enum '{}': duplicate case name '{}'", en.name, n),
                            (en.start_line_0, en.start_col, idx, end_col),
                        ));
                        dup_name = true;
                        break;
                    }
                }
                if dup_name {
                    continue;
                }

                // Duplicate raw values.
                let mut seen_nums: Vec<f64> = Vec::new();
                let mut seen_strs: Vec<String> = Vec::new();
                let mut dup_val = false;
                for (_, v) in &en.cases {
                    match v {
                        Some(EnumCaseValue::Number(n)) => {
                            if seen_nums.contains(n) {
                                errors.push((
                                    format!("Enum '{}': duplicate raw value '{}'", en.name, n),
                                    (en.start_line_0, en.start_col, idx, end_col),
                                ));
                                dup_val = true;
                                break;
                            }
                            seen_nums.push(*n);
                        }
                        Some(EnumCaseValue::Str(s)) => {
                            if seen_strs.contains(s) {
                                errors.push((
                                    format!("Enum '{}': duplicate raw value '{}'", en.name, s),
                                    (en.start_line_0, en.start_col, idx, end_col),
                                ));
                                dup_val = true;
                                break;
                            }
                            seen_strs.push(s.clone());
                        }
                        None => {}
                    }
                }
                if dup_val {
                    continue;
                }

                // Resolve auto-indexed values.
                let raw_type = if has_string { EnumRawType::String } else { EnumRawType::Number };
                let mut next_idx = 0f64;
                let mut resolved_cases: Vec<(String, EnumCaseValue)> = Vec::new();
                for (name, val) in &en.cases {
                    let rv = match val {
                        Some(EnumCaseValue::Number(n)) => {
                            next_idx = *n + 1.0;
                            EnumCaseValue::Number(*n)
                        }
                        Some(EnumCaseValue::Str(s)) => {
                            next_idx += 1.0;
                            EnumCaseValue::Str(s.clone())
                        }
                        None => {
                            let v = EnumCaseValue::Number(next_idx);
                            next_idx += 1.0;
                            v
                        }
                    };
                    resolved_cases.push((name.clone(), rv));
                }
                registry.enums.insert(
                    en.name,
                    EnumDef {
                        raw_type,
                        cases: resolved_cases,
                    },
                );
            } else if let Some(rest) = content.strip_prefix("case ") {
                let Some(ref mut en) = current else {
                    continue;
                };
                let rest = rest.trim();
                if let Some((name, raw)) = rest.split_once('=') {
                    let name = name.trim().to_string();
                    let raw = raw.trim();
                    if let Some(s) = raw.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
                        en.cases.push((name, Some(EnumCaseValue::Str(s.to_string()))));
                    } else if let Ok(n) = raw.parse::<f64>() {
                        en.cases.push((name, Some(EnumCaseValue::Number(n))));
                    } else {
                        // Non-constant raw value (enum ref, function call, …)
                        // Mark the enum as poisoned; the single block-level error
                        // is emitted when we reach <<endenum>>.
                        en.had_non_constant = true;
                        // Keep the case (with None) so that later duplicate
                        // checks still see the case name.
                        en.cases.push((name, None));
                    }
                } else {
                    // Auto-indexed case.
                    en.cases.push((rest.to_string(), None));
                }
            }

            let _ = line_num_1; // suppress unused warning
        }

        (registry, errors)
    }

    /// Look up the raw value of `enum_name::member_name`.
    pub fn lookup(&self, enum_name: &str, member_name: &str) -> Option<&EnumCaseValue> {
        self.enums.get(enum_name)?.cases.iter().find(|(n, _)| n == member_name).map(|(_, v)| v)
    }

    /// Look up a member name across ALL enums, returning the value only when
    /// exactly one enum contains a case with that name.  Returns `None` if
    /// the name is unknown or ambiguous.
    pub fn lookup_member_unique(&self, member_name: &str) -> Option<&EnumCaseValue> {
        let mut found: Option<&EnumCaseValue> = None;
        let mut count = 0usize;
        for def in self.enums.values() {
            if let Some((_, v)) = def.cases.iter().find(|(n, _)| n == member_name) {
                found = Some(v);
                count += 1;
                if count > 1 {
                    return None;
                }
            }
        }
        found
    }

    /// Returns `true` if `type_name` is the name of a registered enum.
    #[allow(dead_code)]
    pub fn is_enum_type(&self, type_name: &str) -> bool {
        self.enums.contains_key(type_name)
    }
}

/// Format an f64 raw value as a Yarn NUMBER literal string.
/// Integers are formatted without a decimal point ("3", not "3.0").
pub(crate) fn format_enum_number(n: f64) -> String {
    if n.is_finite() && n == n.floor() {
        format!("{}", n as i64)
    } else {
        format!("{}", n)
    }
}
