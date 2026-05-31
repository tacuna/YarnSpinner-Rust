//! Adapted from <https://github.com/YarnSpinnerTool/YarnSpinner/blob/3a5b7343f715e4e9a3705fa4224e7fa510b92f1c/YarnSpinner.Tests/TagTests.cs>

use test_base::prelude::*;
use yarnspinner::compiler::*;

mod test_base;

#[test]
fn test_no_options_line_not_tagged() {
    let result = Compiler::from_test_source("title:Start\n---\nline without options #line:1\n===\n")
        .compile()
        .unwrap();

    let info = &result.string_table[&"line:1".into()];
    assert!(!contains_last_line_tag(info));
}

#[test]
fn test_line_before_options_tagged_last_line() {
    let result = Compiler::from_test_source("title:Start\n---\nline before options #line:1\n-> option 1\n-> option 2\n===\n")
        .compile()
        .unwrap();

    let info = &result.string_table[&"line:1".into()];
    assert!(contains_last_line_tag(info));
}

#[test]
fn test_line_not_before_options_not_tagged_last_line() {
    let result =
        Compiler::from_test_source("title:Start\n---\nline not before options #line:0\nline before options #line:1\n-> option 1\n-> option 2\n===\n")
            .compile()
            .unwrap();

    let info = &result.string_table[&"line:0".into()];
    assert!(!contains_last_line_tag(info));
}

#[test]
fn test_line_after_options_not_tagged_last_line() {
    let result =
        Compiler::from_test_source("title:Start\n---\nline before options #line:1\n-> option 1\n-> option 2\nline after options #line:2\n===\n")
            .compile()
            .unwrap();

    let info = &result.string_table[&"line:2".into()];
    assert!(!contains_last_line_tag(info));
}

#[test]
fn test_nested_option_lines_tagged_last_line() {
    let result = Compiler::from_test_source(
        "
line before options #line:1
-> option 1
    line 1a #line:1a
    line 1b #line:1b
    -> option 1a
    -> option 1b
-> option 2
-> option 3
",
    )
    .compile()
    .unwrap();

    let info = &result.string_table[&"line:1".into()];
    assert!(contains_last_line_tag(info));

    let info = &result.string_table[&"line:1b".into()];
    assert!(contains_last_line_tag(info));
}

#[test]
fn test_if_interior_lines_tagged_last_line() {
    let result = Compiler::from_test_source(
        "
<<if true>>
line before options #line:0
-> option 1
-> option 2
<<endif>>
            ",
    )
    .compile()
    .unwrap();

    let info = &result.string_table[&"line:0".into()];
    assert!(contains_last_line_tag(info));
}

#[test]
fn test_if_interior_lines_not_tagged_last_line() {
    let result = Compiler::from_test_source(
        "
<<if true>>
line before options #line:0
<<endif>>
-> option 1
-> option 2
",
    )
    .compile()
    .unwrap();

    let info = &result.string_table[&"line:0".into()];
    assert!(!contains_last_line_tag(info));
}

#[test]
fn test_nested_option_lines_not_tagged() {
    let result = Compiler::from_test_source(
        "
-> option 1
    inside options #line:1a
-> option 2
-> option 3
",
    )
    .compile()
    .unwrap();

    let info = &result.string_table[&"line:1a".into()];
    assert!(!contains_last_line_tag(info));
}

#[test]
fn test_interrupted_lines_not_tagged() {
    let result = Compiler::from_test_source(
        "
line before command #line:0
<<custom command>>
-> option 1
line before declare #line:1
<<declare $value = 0>>
-> option 1
line before set #line:2
<<set $value = 0>>
-> option 1
line before jump #line:3
<<jump nodename>>
line before call #line:4
<<call function()>>
            ",
    )
    .compile()
    .unwrap();

    let info = &result.string_table[&"line:0".into()];
    assert!(!contains_last_line_tag(info));
    let info = &result.string_table[&"line:1".into()];
    assert!(!contains_last_line_tag(info));
    let info = &result.string_table[&"line:2".into()];
    assert!(!contains_last_line_tag(info));
    let info = &result.string_table[&"line:3".into()];
    assert!(!contains_last_line_tag(info));
    let info = &result.string_table[&"line:4".into()];
    assert!(!contains_last_line_tag(info));
}

#[test]
fn test_line_is_last_before_another_node_not_tagged() {
    let result = Compiler::from_test_source("title: Start\n---\nlast line #line:0\n===\ntitle: Second\n---\n-> option 1\n===\n")
        .compile()
        .unwrap();

    let info = &result.string_table[&"line:0".into()];
    assert!(!contains_last_line_tag(info));
}

fn contains_last_line_tag(info: &StringInfo) -> bool {
    info.metadata.contains(&"lastline".to_owned())
}

#[test]
fn test_comments_arent_tagged() {
    let source = "title: Start\n---\n\\\\\n===";

    // ensuring the base text compiles fine as is
    let result = Compiler::from_test_source(source)
        .with_compilation_type(CompilationType::StringsOnly)
        .compile();

    assert!(result.is_ok());

    // Tagging the lines
    let (tagged_version, _) = Compiler::tag_lines(source, vec![]).unwrap().unwrap();

    // recompiling, we should have no errors
    let result = Compiler::from_test_source(&tagged_version)
        .with_compilation_type(CompilationType::StringsOnly)
        .compile();

    assert!(result.is_ok());
}

// ---------------------------------------------------------------------------
// TestShadowLinesReflectSourceLines
// Divergence from C#: Rust keeps `text: String` (non-empty) on shadow lines
// rather than setting it to null, to avoid breaking the non-optional field type.
// Callers should use `shadow_line_id.is_some()` to detect shadow lines.
// ---------------------------------------------------------------------------

#[test]
fn test_shadow_lines_reflect_source_lines() {
    let source = "title: Start\n---\nThis is a line. #line:source #apple\nThis is a line. #shadow:source #banana\n===\n";
    let file = File {
        file_name: "input".to_owned(),
        source: source.to_owned(),
    };
    let result = Compiler::new().add_file(file).compile().unwrap();

    assert_eq!(result.string_table.len(), 2, "there are two lines in the string table");

    let source_line = result.string_table.get(&"line:source".into()).expect("source line not found");
    let shadow_line = result
        .string_table
        .iter()
        .find(|(k, _)| k.0 != "line:source")
        .map(|(_, v)| v)
        .expect("shadow line not found");

    assert_eq!(source_line.text, "This is a line.");
    assert!(source_line.shadow_line_id.is_none(), "source lines should not have shadow_line_id");
    assert!(source_line.metadata.contains(&"apple".to_owned()));
    assert!(!source_line.metadata.contains(&"banana".to_owned()));

    // Divergence: text is not null/empty in Rust (kept as the source text for runtime use)
    assert!(shadow_line.shadow_line_id.is_some(), "shadow lines should have shadow_line_id set");
    assert_eq!(shadow_line.shadow_line_id.as_deref(), Some("line:source"));
    assert!(
        !shadow_line.metadata.contains(&"apple".to_owned()),
        "shadow lines have their own metadata"
    );
    assert!(
        shadow_line.metadata.contains(&"banana".to_owned()),
        "shadow lines have their own metadata"
    );
}
