//! Adapted from <https://github.com/YarnSpinnerTool/YarnSpinner/blob/3a5b7343f715e4e9a3705fa4224e7fa510b92f1c/YarnSpinner.Tests/LanguageTests.cs>
//!
//! ## Implementation notes
//!
//! Because Rust has no concept of a current global culture setting, the test `TestCompilationShouldNotBeCultureDependent` was omitted.
//! The test `TestNumberPlurals` was moved to a unit test in the `runtime` crate because it fits better there.

#[cfg(feature = "bevy")]
use bevy::prelude::World;
use std::collections::HashMap;
use test_base::prelude::*;
use yarnspinner::compiler::*;
use yarnspinner::core::*;
use yarnspinner::runtime::*;

mod test_base;

#[test]
fn test_example_script() {
    let path = test_data_path().join("Example.yarn");
    let test_plan = path.with_extension("testplan");

    let result = Compiler::default().read_file(path).compile().unwrap();

    TestBase::default()
        .with_runtime_errors_do_not_cause_failure()
        .with_compilation(result)
        .read_test_plan(test_plan)
        .run_standard_testcase();
}

#[test]
fn can_compile_space_demo() {
    let test_base = TestBase::default();
    let sally_path = space_demo_scripts_path().join("Sally.yarn");
    let ship_path = space_demo_scripts_path().join("Ship.yarn");

    let _result_sally = Compiler::new()
        .read_file(&sally_path)
        .extend_library(test_base.dialogue.library().clone())
        .compile()
        .unwrap();
    let _result_sally_and_ship = Compiler::new()
        .read_file(&sally_path)
        .read_file(ship_path)
        .extend_library(test_base.dialogue.library().clone())
        .compile()
        .unwrap();
}

#[test]
#[should_panic]
fn test_merging_nodes() {
    let test_base = TestBase::default();
    let sally_path = space_demo_scripts_path().join("Sally.yarn");
    let ship_path = space_demo_scripts_path().join("Ship.yarn");

    let result_sally = Compiler::default()
        .read_file(&sally_path)
        .extend_library(test_base.dialogue.library().clone())
        .compile()
        .unwrap();
    let result_sally_and_ship = Compiler::default()
        .read_file(&sally_path)
        .read_file(ship_path)
        .extend_library(test_base.dialogue.library().clone())
        .compile()
        .unwrap();

    // Loading code with the same contents should throw
    let _combined_not_working = Program::combine(vec![result_sally.program.unwrap(), result_sally_and_ship.program.unwrap()]);
}

#[test]
fn test_end_of_notes_with_options_not_added() {
    let path = test_data_path().join("SkippedOptions.yarn");
    let result = Compiler::default().read_file(path).compile().unwrap();

    let mut dialogue = TestBase::default().with_compilation(result).dialogue;
    dialogue.set_node("Start").unwrap();

    #[cfg(feature = "bevy")]
    let mut world = World::default();

    let mut has_options = false;
    while dialogue.can_continue() {
        #[cfg(feature = "bevy")]
        let events = dialogue.continue_with_world(&mut world);
        #[cfg(not(feature = "bevy"))]
        let events = dialogue.continue_();
        let events = events.unwrap_or_else(|e| panic!("Encountered error while running dialogue: {e}"));
        if events.iter().any(|event| matches!(event, DialogueEvent::Options(_))) {
            has_options = true;
            break;
        }
    }
    assert!(!has_options);
}

#[test]
fn test_node_headers() {
    let path = test_data_path().join("Headers.yarn");
    let result = Compiler::default().read_file(&path).compile().unwrap();
    let program = result.program.as_ref().unwrap();
    assert_eq!(program.nodes.len(), 8);

    for tag in &["one", "two", "three"].map(|s| s.to_owned()) {
        assert!(program.nodes["Tags"].tags.contains(tag));
    }

    let headers: HashMap<_, _> = vec![
        ("EmptyTags", vec![("title", "EmptyTags"), ("tags", "")]),
        (
            "ArbitraryHeaderWithValue",
            vec![("title", "ArbitraryHeaderWithValue"), ("arbitraryheader", "some-arbitrary-text")],
        ),
        ("Tags", vec![("title", "Tags"), ("tags", "one two three")]),
        ("SingleTagOnly", vec![("title", "SingleTagOnly")]),
        ("Comments", vec![("title", "Comments"), ("tags", "one two three")]),
        (
            "LotsOfHeaders",
            vec![
                ("contains", "lots"),
                ("title", "LotsOfHeaders"),
                ("this", "node"),
                ("of", ""),
                ("blank", ""),
                ("others", "are"),
                ("headers", ""),
                ("some", "are"),
                ("not", ""),
            ],
        ),
        (
            "HeaderWith.Comments",
            vec![
                ("title", "HeaderWith"),
                ("subtitle", "Comments"),
                ("when", "always"),
                ("another", "header"),
                ("also", "/"),
            ],
        ),
        // Synthetic hub node generated for the node group; has no yarn headers.
        ("HeaderWith", vec![]),
    ]
    .into_iter()
    .collect();
    assert_eq!(program.nodes.len(), headers.len());
    for (node_name, expected_headers) in headers {
        let node = &program.nodes[node_name];
        assert_eq!(node.headers.len(), expected_headers.len());
        for header in &node.headers {
            let expected_header = expected_headers.iter().find(|(k, _)| k == &header.key).unwrap();
            assert_eq!(header.value, expected_header.1);
        }
    }

    let path = path.to_string_lossy().to_string();

    assert!(result.file_tags.contains_key(&path));
    assert_eq!(1, result.file_tags.len());
    assert!(result.file_tags[&path].contains(&"file_header".to_owned()));
    assert_eq!(1, result.file_tags[&path].len());
}

#[test]
fn test_invalid_characters_in_node_title() {
    let path = test_data_path().join("InvalidNodeTitle.yarn");
    let result = Compiler::default().read_file(path).compile();
    assert!(result.is_err());
}

#[test]
fn test_sources() {
    for file in [
        "TestCases",
        "TestCases/ParseFailures",
        // ## Implementation note: this directory does not exist
        // "Issues"
    ]
    .iter()
    .flat_map(TestBase::file_sources)
    {
        let path = test_data_path().join(&file);
        let test_plan = path.with_extension("testplan");

        let test_base = TestBase::default().extend_library(|library| {
            library
                .add_function("add_three_operands", |a: i32, b: i32, c: i32| a + b + c)
                .add_function("set_objective_complete", |_s: String| true)
                .add_function("is_objective_active", |_s: String| true)
                .add_function("get_quest_status", |_s: String| "InProgress".to_owned());
        });
        let result = Compiler::default()
            .read_file(&path)
            .extend_library(test_base.dialogue.library().clone())
            .compile();

        if !test_plan.exists() {
            // No test plan for this file exists, which indicates that
            // the file is not expected to compile. We'll actually make
            // it a test failure if it _does_ compile.
            assert!(result.is_err(), "{} is expected to have compile errors", file.display());
        } else {
            let compilation = result.unwrap();

            let mut test_base = test_base
                .read_test_plan(test_plan)
                .with_compilation(compilation)
                .extend_library(|library| {
                    library
                        .add_function("dummy_bool", || true)
                        .add_function("dummy_number", || 1)
                        .add_function("dummy_string", || "string".to_owned());
                });

            // If this file contains a Start node, run the test case
            // (otherwise, we're just testing its parseability, which
            // we did in the last line)
            if test_base.dialogue.node_exists("Start") {
                test_base.run_standard_testcase();
            }
        }
    }
}

#[test]
#[should_panic]
fn crashes_on_command_expression_evaluating_whitespace() {
    let result = Compiler::from_test_source("<<{\"\"}{\"   \"}>>").compile().unwrap();
    TestBase::new().with_compilation(result).run_standard_testcase();
}

// ---------------------------------------------------------------------------
// TestIdentifiersMayContainValidCharacters
// ---------------------------------------------------------------------------

/// Adapted from `TestIdentifiersMayContainValidCharacters`.
///
/// Verifies that the Yarn grammar accepts identifiers containing Unicode
/// characters from scripts outside the Basic Multilingual Plane, including
/// CJK, Cyrillic, and Emoji ranges.
#[test]
fn test_identifiers_may_contain_valid_characters() {
    let source = r#"
title: Start
---
<<declare $实验 = 1 as number>>
<<declare $эксперимент = 1 as number>>
<<declare $🧶 = 1 as number>>
===
"#;

    let result = Compiler::new()
        .add_file(File {
            file_name: "<input>".to_owned(),
            source: source.to_owned(),
        })
        .compile()
        .unwrap();

    // compile().unwrap() already asserts no errors; additionally verify no warnings are errors
    assert!(
        result.warnings.is_empty() || result.warnings.iter().all(|d| d.severity != DiagnosticSeverity::Error),
        "Unicode identifiers should compile without errors: {:?}",
        result.warnings
    );
}

// ---------------------------------------------------------------------------
// TestNumberPlurals — ported to runtime unit tests
// ---------------------------------------------------------------------------

/// # Why ignored
///
/// `TestNumberPlurals` tests the pluralisation rules for each supported
/// locale.  In the Rust port this was moved to a unit test inside the
/// `yarnspinner_runtime` crate where the relevant code lives.
#[test]
#[ignore = "Ported to unit tests in the runtime crate — not applicable as an integration test"]
fn test_number_plurals() {}

// ---------------------------------------------------------------------------
// TestCompilationShouldNotBeCultureDependent
// ---------------------------------------------------------------------------

/// # Why ignored
///
/// Rust has no concept of a global culture/locale setting, so this test is
/// not applicable.
#[test]
#[ignore = "Rust has no global culture setting — not applicable"]
fn test_compilation_should_not_be_culture_dependent() {}

// ---------------------------------------------------------------------------
// TestBasicBlockExtraction
// ---------------------------------------------------------------------------

/// Adapted from `TestBasicBlockExtraction`.
///
/// Compiles a multi-node Yarn script and verifies that every node can be
/// decomposed into at least one non-empty basic block.
#[test]
fn test_basic_block_extraction() {
    let source = concat!(
        "title: NodeA\n---\n",
        "Hello world!\n",
        "<<if true>>\n",
        "  Option A\n",
        "<<else>>\n",
        "  Option B\n",
        "<<endif>>\n",
        "===\n",
        "title: NodeB\n---\n",
        "Another line.\n",
        "===\n",
    );
    let result = Compiler::new()
        .add_file(yarnspinner::compiler::File {
            file_name: "<input>".to_owned(),
            source: source.to_owned(),
        })
        .compile()
        .unwrap();

    let program = result.program.unwrap();
    assert!(!program.nodes.is_empty());

    for node in program.nodes.values() {
        let blocks = node.get_basic_blocks();
        assert!(!blocks.is_empty(), "node '{}' should have at least one block", node.name);
        for block in &blocks {
            assert!(
                !block.instructions.is_empty(),
                "block in node '{}' should have at least one instruction",
                node.name
            );
            assert_eq!(block.node_name, node.name, "block.node_name should equal the node name");
            assert!(!block.to_string().is_empty(), "block.to_string() should not be empty");
        }
    }
}

// ---------------------------------------------------------------------------
// TestBasicBlockDetours
// ---------------------------------------------------------------------------

/// Adapted from `TestBasicBlockDetours`.
///
/// Compiles a script where NodeA detoures to NodeB and checks that the
/// basic-block analysis correctly identifies the detour destination and its
/// return-to block.
#[test]
fn test_basic_block_detours() {
    let source = concat!(
        "title: NodeA\n---\n",
        "Line 1\n",
        "<<detour NodeB>>\n",
        "Line 3\n",
        "===\n",
        "title: NodeB\n---\n",
        "Line 2\n",
        "===\n",
    );
    let result = Compiler::new()
        .add_file(yarnspinner::compiler::File {
            file_name: "<input>".to_owned(),
            source: source.to_owned(),
        })
        .compile()
        .unwrap();

    let program = result.program.unwrap();
    let node_a = &program.nodes["NodeA"];
    let blocks = node_a.get_basic_blocks();

    assert_eq!(blocks.len(), 2, "NodeA should decompose into exactly 2 basic blocks");

    let first_block = &blocks[0];
    assert_eq!(first_block.destinations.len(), 1, "first block should have exactly one destination");

    let Destination::Node(ref dest) = first_block.destinations[0] else {
        panic!("expected NodeDestination, got {:?}", first_block.destinations[0]);
    };
    assert_eq!(dest.node_name, "NodeB");
    let return_to = dest.return_to.as_ref().expect("detour should have a return_to");
    assert_eq!(return_to.node_name, "NodeA");
}

// ---------------------------------------------------------------------------
// TestCompletelyInvalidTokenStreamDoesNotCrashCompiler
// ---------------------------------------------------------------------------

/// Adapted from `TestCompletelyInvalidTokenStreamDoesNotCrashCompiler`.
///
/// Verifies that attempting to compile a completely invalid Yarn script does
/// not cause a panic.  The compiler may return an error result, but must not
/// crash.
#[test]
fn test_completely_invalid_token_stream_does_not_crash_compiler() {
    let input = "This is invalid yarn script, and will not compile.";
    let _ = Compiler::new()
        .add_file(File {
            file_name: "<input>".to_owned(),
            source: input.to_owned(),
        })
        .compile();
    // If we reach this point without panicking, the test passes.
}

// ---------------------------------------------------------------------------
// TestWhenClauseValuesParseAsExpressions
// ---------------------------------------------------------------------------

/// Adapted from `TestWhenClauseValuesParseAsExpressions`.
///
/// Verifies that `when:` header values are treated as boolean expressions by
/// the compiler (enabling saliency conditions), while non-`when` headers with
/// the same expression text store the raw source string verbatim.
///
/// C# inspects the ANTLR parse tree directly via
/// `YarnSpinnerParser.NodeContext` internals (`GetWhenHeaders`,
/// `GetHeaders`, `header_when_expression().expression()`).  Rust does not
/// expose parser internals, so we observe the same guarantees through the
/// compiled program's node headers and successful compilation:
///
/// - Successful compilation proves that `when: $a` and `when: $a || false`
///   were parsed as boolean expressions (not plain text) — if they were
///   stored as raw strings the saliency compilation step would fail.
/// - `some_other_header1: $a` and `some_other_header2: $a || false` are plain
///   headers, so they appear in the compiled node's `.headers` with their raw
///   source text preserved exactly.
#[test]
fn test_when_clause_values_parse_as_expressions() {
    // Given – a node with two `when:` headers (simple and compound boolean
    // expressions) plus two regular headers whose values use the same text.
    // `$a` is declared in the body; declarations are collected globally before
    // type-checking, so the `when:` expression in the header can reference it.
    let source = r#"title: Start
when: $a
some_other_header1: $a
when: $a || false
some_other_header2: $a || false
---
<<declare $a = false>>
==="#;

    // When – compile; must succeed.
    let result = Compiler::new()
        .add_file(File {
            file_name: "<input>".to_owned(),
            source: source.to_owned(),
        })
        .compile()
        .unwrap();

    let program = result.program.unwrap();

    // Node-group compilation renames the member from "Start" to an
    // auto-generated key (e.g. "Start.0"), so locate it by header presence.
    let content_node = program
        .nodes
        .values()
        .find(|n| n.headers.iter().any(|h| h.key == "some_other_header1"))
        .expect("expected a compiled node with 'some_other_header1' header");

    // Then – non-`when` headers preserve the raw source text.
    let hdr1 = content_node.headers.iter().find(|h| h.key == "some_other_header1").unwrap();
    assert_eq!(hdr1.value, "$a", "non-when header stores raw expression text verbatim");

    let hdr2 = content_node.headers.iter().find(|h| h.key == "some_other_header2").unwrap();
    assert_eq!(hdr2.value, "$a || false", "non-when header stores compound expression text verbatim");

    // `when:` headers are present and preserved on the compiled node.
    let when_count = content_node.headers.iter().filter(|h| h.key == "when").count();
    assert_eq!(when_count, 2, "both when headers are present on the compiled node");

    // Non-`when`, non-`title` headers: exactly the two declared above.
    let other_count = content_node.headers.iter().filter(|h| h.key != "when" && h.key != "title").count();
    assert_eq!(other_count, 2, "exactly two non-when, non-title headers");
}

/// Equivalent to `TestParsingStucturedCommands` in `LanguageTests.cs`.
///
/// Verifies that `parse_structured_command` correctly extracts the command name
/// and argument count from a valid structured command, and produces diagnostics
/// for invalid input (old-style brace expressions).
#[test]
fn test_parsing_structured_commands() {
    // Given
    let valid_command_text = r#"walk mae $var 2.3 "string" false true SomeArbitraryID function_call(2,"three") EnumA.Member .Member"#;
    let invalid_command_text = "walk mae {$myVar}"; // an old-style 'plain text' command

    // When
    let parsed_valid = parse_structured_command(valid_command_text);
    let parsed_invalid = parse_structured_command(invalid_command_text);

    // Then
    assert_eq!(parsed_valid.command_name.as_deref(), Some("walk"));
    assert_eq!(parsed_valid.argument_count, 10, "the command has this many parameters");
    assert!(parsed_valid.diagnostics.is_empty(), "a valid structured command has no errors");

    assert!(!parsed_invalid.diagnostics.is_empty(), "an invalid structured command has errors");

    // Even if a structured command fails to parse, we can still recover the
    // command name.  This helps decide whether the parse error should be
    // surfaced to the user (i.e. this is meant to be a structured command).
    assert_eq!(parsed_invalid.command_name.as_deref(), Some("walk"));
}
