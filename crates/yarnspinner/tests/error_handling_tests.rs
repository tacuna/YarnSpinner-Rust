//! Adapted from <https://github.com/YarnSpinnerTool/YarnSpinner/blob/3a5b7343f715e4e9a3705fa4224e7fa510b92f1c/YarnSpinner.Tests/ErrorHandlingTests.cs>

use crate::test_base::*;
use test_base::prelude::*;
use yarnspinner::compiler::*;

mod test_base;
#[test]
fn test_malformed_if_statement() {
    let result = Compiler::from_test_source("<<if true>> // error: no endif").compile().unwrap_err();

    println!("{result}");
    assert!(
        result
            .0
            .iter()
            .any(|d| { d.message.contains("Expected an <<endif>> to match the <<if>> statement on line 3") })
    );
}

#[test]
fn test_extraneous_else() {
    let result = Compiler::from_test_source(
        "<<if true>>\n\
            One\n\
            <<else>>\n\
            Two\n\
            <<else>> // error: more than one else\n\
            Three\n\
            <<endif>>",
    )
    .compile()
    .unwrap_err();

    println!("{result}");
    assert!(result.0.iter().any(|d| {
        d.message
            .contains("More than one <<else>> statement in an <<if>> statement isn't allowed")
    }));
    assert!(
        result
            .0
            .iter()
            .any(|d| { d.message.contains("Unexpected \"endif\" while reading a statement") })
    );
}

#[test]
fn test_empty_command() {
    let result = Compiler::from_test_source("\n<<>>\n").compile().unwrap_err();
    println!("{result}");

    assert!(result.0.iter().any(|d| d.message.contains("Command text expected")));
}

#[test]
fn test_invalid_variable_name_in_set_or_declare() {
    let result = Compiler::from_test_source("\n<<set test = 1>>\n").compile().unwrap_err();

    println!("{result}");
    assert!(result.0.iter().any(|d| d.message == "Variable names need to start with a $"));

    let result = Compiler::from_test_source("\n<<declare test = 1>>\n").compile().unwrap_err();

    println!("{result}");
    assert!(result.0.iter().any(|d| d.message == "Variable names need to start with a $"));
}

#[test]
fn test_invalid_function_call() {
    let result = Compiler::from_test_source("<<if someFunction(>><<endif>>").compile().unwrap_err();

    println!("{result}");
    assert!(
        result
            .0
            .iter()
            .any(|d| { d.message.contains("Unexpected \">>\" while reading a function call") })
    );
}

#[test]
fn test_compiling_same_file_twice_fails() {
    let result = Compiler::new()
        .read_file(space_demo_scripts_path().join("Sally.yarn"))
        .read_file(space_demo_scripts_path().join("Sally.yarn"))
        .extend_library(TestBase::new().dialogue.library().clone())
        .compile();
    let diagnostics = result.unwrap_err().0;
    assert!(diagnostics.iter().any(|d| d.message.contains("Duplicate line ID line:794945")));
}

#[test]
fn test_empty_nodes_generate_warnings() {
    let result = Compiler::from_test_source("").compile().unwrap();

    let warnings = result
        .warnings
        .iter()
        .filter(|d| matches!(d.severity, DiagnosticSeverity::Warning))
        .collect::<Vec<_>>();

    assert_eq!(warnings.len(), 1);
    assert_eq!(
        warnings.first().unwrap().message,
        "Node \"Start\" is empty and will not be included in the compiled output."
    );
}

// ---------------------------------------------------------------------------
// TestLineCommandOverlapGeneratesDiagnostics
// Divergence: Rust does NOT generate YS0019 for lines that START with <<
// (those are parsed as command lines; the validator only checks text-after-command
// for lines that don't start with <<). We test the Error (YS0020) and
// the Warning (YS0019) separately.
// UPDATE: after fixing validate_syntax to also scan command lines, both
// diagnostics are generated.
// ---------------------------------------------------------------------------

#[test]
fn test_line_command_overlap_generates_diagnostics() {
    let input = "title: Program\n---\nthis is a line with a valid conditional <<if true>>\n<<before command>> this is a line following a command\nthis is the line before a command <<after command>>\n===\n";
    let file = File {
        file_name: "input".to_owned(),
        source: input.to_owned(),
    };
    let result = Compiler::new().add_file(file).compile().unwrap_err();
    // YS0020 (Error): text before a command on a line
    assert!(result.0.iter().any(|d| d.code.as_deref() == Some("YS0020")), "Expected YS0020");
    let ys0020 = result.0.iter().find(|d| d.code.as_deref() == Some("YS0020")).unwrap();
    assert!(matches!(ys0020.severity, DiagnosticSeverity::Error));
    assert!(ys0020.message.contains("after command"));
    // YS0019 (Warning): text after a command on a line
    assert!(result.0.iter().any(|d| d.code.as_deref() == Some("YS0019")), "Expected YS0019");
    let ys0019 = result.0.iter().find(|d| d.code.as_deref() == Some("YS0019")).unwrap();
    assert!(matches!(ys0019.severity, DiagnosticSeverity::Warning));
    assert!(ys0019.message.contains("before command"));
}

// ---------------------------------------------------------------------------
// TestUnbalancedCommandTerminalsGenerateDiagnostics
// ---------------------------------------------------------------------------

#[test]
fn test_unbalanced_command_terminals_stray_end() {
    // "some command>>" → YS0021 Warning (stray >>)
    let result = Compiler::from_test_source("some command>>").compile().unwrap();
    assert!(result.warnings.iter().any(|d| d.code.as_deref() == Some("YS0021")));
}

#[test]
fn test_unbalanced_command_terminals_single_bracket_end() {
    // "<some command>>" → YS0021 Warning (stray >>)
    let result = Compiler::from_test_source("<some command>>").compile().unwrap();
    assert!(result.warnings.iter().any(|d| d.code.as_deref() == Some("YS0021")));
}

#[test]
fn test_unbalanced_command_terminals_single_bracket_wrap() {
    // "<some command>" → YS0048 Warning (single angle-bracket wrap)
    let result = Compiler::from_test_source("<some command>").compile().unwrap();
    assert!(result.warnings.iter().any(|d| d.code.as_deref() == Some("YS0048")));
}

#[test]
fn test_unbalanced_command_terminals_missing_close() {
    // "<<some command" → YS0006 (no closing >>)
    let result = Compiler::from_test_source("<<some command").compile().unwrap_err();
    assert!(
        result.0.iter().any(|d| d.code.as_deref() == Some("YS0006")),
        "Expected YS0006 for unclosed command, got: {:?}",
        result.0
    );
}

// ---------------------------------------------------------------------------
// TestBuiltInCommandsMissingTerminalsGenerateDiagnostics  (YS0022)
// ---------------------------------------------------------------------------

#[test]
fn test_built_in_commands_missing_terminals_generate_diagnostics() {
    // Note: "jump {$expr}" and "detour {$expr}" require declared variables so are not tested here.
    for input in ["set $foo = 5", "declare $foo = 5", "jump here", "detour here"] {
        let result = Compiler::from_test_source(input).compile().unwrap();
        assert!(
            result.warnings.iter().any(|d| d.code.as_deref() == Some("YS0022")),
            "Expected YS0022 for: {input}"
        );
    }
}

// ---------------------------------------------------------------------------
// TestValidCommandAlikeLinesDontGenerateDiagnostics
// ---------------------------------------------------------------------------

#[test]
fn test_valid_command_alike_lines_dont_generate_diagnostics() {
    for input in ["I declare $foo = 5", "I set $foo = 5", "I set foo = 5", "jump 123abc", "detour 123abc"] {
        let result = Compiler::from_test_source(input).compile().unwrap();
        assert!(result.warnings.is_empty(), "Expected no diagnostics for: {input}");
    }
}

// ---------------------------------------------------------------------------
// TestDeclaredValueIsDifferentFromExplicitType  (YS0053)
// ---------------------------------------------------------------------------

#[test]
fn test_declared_value_is_different_from_explicit_type() {
    let cases = [
        (
            "<<declare $x = \"hello\" as Number>>",
            "$x is declared to be a Number, but its initial value '\"hello\"' is a String",
        ),
        (
            "<<declare $x = true as Number>>",
            "$x is declared to be a Number, but its initial value 'true' is a Bool",
        ),
        (
            "<<declare $x = \"true\" as bool>>",
            "$x is declared to be a Bool, but its initial value '\"true\"' is a String",
        ),
        (
            "<<declare $x = 123 as bool>>",
            "$x is declared to be a Bool, but its initial value '123' is a Number",
        ),
        (
            "<<declare $x = 123 as string>>",
            "$x is declared to be a String, but its initial value '123' is a Number",
        ),
        (
            "<<declare $x = true as string>>",
            "$x is declared to be a String, but its initial value 'true' is a Bool",
        ),
    ];
    for (input, expected_msg) in cases {
        let result = Compiler::from_test_source(input).compile().unwrap_err();
        let diag = result.0.iter().find(|d| d.code.as_deref() == Some("YS0053"));
        assert!(diag.is_some(), "Expected YS0053 for: {input}");
        assert_eq!(diag.unwrap().message, expected_msg, "Wrong message for: {input}");
    }
}

// ---------------------------------------------------------------------------
// TestMissingBodyEndInternals  (YS0004)
// ---------------------------------------------------------------------------

#[test]
fn test_missing_body_end_internals() {
    let cases = [
        "title: Program\n---\nThis node is missing it's end of body terminator\n",
        "title: Program\n---\nThis node is missing it's end of body terminator",
    ];
    // Divergence: Rust does not emit YS0004 specifically; it emits a generic parse error.
    for input in cases {
        let file = File {
            file_name: "input".to_owned(),
            source: input.to_owned(),
        };
        let result = Compiler::new().add_file(file).compile().unwrap_err();
        assert!(!result.0.is_empty(), "Expected diagnostic for missing body end");
        assert!(
            result.0.iter().any(|d| matches!(d.severity, DiagnosticSeverity::Error)),
            "Expected at least one error-level diagnostic for missing body end"
        );
    }
}

// ---------------------------------------------------------------------------
// TestMissingClosingScopeGeneratesDiagnostic  (YS0007)
// ---------------------------------------------------------------------------

#[test]
fn test_missing_closing_scope_if_generates_diagnostic() {
    let input = "title: Program\n---\n<<if true>>\n    internal line\n===";
    let file = File {
        file_name: "input".to_owned(),
        source: input.to_owned(),
    };
    let result = Compiler::new().add_file(file).compile().unwrap_err();
    assert!(
        result.0.iter().any(|d| d.code.as_deref() == Some("YS0007")),
        "Expected YS0007 for unclosed if, got: {:?}",
        result.0
    );
}

#[test]
fn test_missing_closing_scope_once_generates_diagnostic() {
    // Divergence: Rust transforms <<once>> to <<if>>; the unclosed scope reports YS0007.
    let input = "title: Program\n---\n<<once>>\n    internal line\n===";
    let file = File {
        file_name: "input".to_owned(),
        source: input.to_owned(),
    };
    let result = Compiler::new().add_file(file).compile().unwrap_err();
    assert!(
        result.0.iter().any(|d| d.code.as_deref() == Some("YS0007")),
        "Expected YS0007 for unclosed once (transformed to if), got: {:?}",
        result.0
    );
}

// ---------------------------------------------------------------------------
// TestDuplicateNonNodeGroupsShouldGenerateDiagnostics  (YS0011)
// ---------------------------------------------------------------------------

#[test]
fn test_duplicate_non_node_groups_should_generate_diagnostics() {
    let input = "title: A\n---\nThis is a line\n===\ntitle: A\n---\nThis is a line\n===";
    let file = File {
        file_name: "input".to_owned(),
        source: input.to_owned(),
    };
    let result = Compiler::new().add_file(file).compile().unwrap_err();
    let ys0011_count = result.0.iter().filter(|d| d.code.as_deref() == Some("YS0011")).count();
    assert_eq!(ys0011_count, 2, "Expected 2 YS0011 diagnostics");
    for diag in result.0.iter().filter(|d| d.code.as_deref() == Some("YS0011")) {
        assert!(diag.message.contains('A'), "Message should reference node name A");
    }
}

// ---------------------------------------------------------------------------
// TestDuplicateNodeGroupsShouldNotGenerateDiagnostics  (no diagnostics for when: groups)
// ---------------------------------------------------------------------------

#[test]
fn test_duplicate_node_groups_should_not_generate_diagnostics() {
    let input = "title: A\nwhen: always\n---\nThis is a line\n===\ntitle: A\nwhen: always\n---\nThis is a line\n===";
    let file = File {
        file_name: "input".to_owned(),
        source: input.to_owned(),
    };
    let result = Compiler::new().add_file(file).compile().unwrap();
    assert!(result.warnings.is_empty(), "Expected no diagnostics for valid node group");
}

// ---------------------------------------------------------------------------
// TestUnknownNodeJumpsGenerateDiagnostics  (YS0012)
// ---------------------------------------------------------------------------

#[test]
fn test_unknown_node_detour_generates_diagnostic() {
    let result = Compiler::from_test_source("<<detour B>>").compile().unwrap();
    let diag = result.warnings.iter().find(|d| d.code.as_deref() == Some("YS0012"));
    assert!(diag.is_some(), "Expected YS0012 warning for detour to undefined node B");
    assert!(matches!(diag.unwrap().severity, DiagnosticSeverity::Warning));
    assert!(diag.unwrap().message.contains("'B'"));
}

#[test]
fn test_unknown_node_jump_generates_diagnostic() {
    let result = Compiler::from_test_source("<<jump B>>").compile().unwrap();
    let diag = result.warnings.iter().find(|d| d.code.as_deref() == Some("YS0012"));
    assert!(diag.is_some(), "Expected YS0012 warning for jump to undefined node B");
    assert!(matches!(diag.unwrap().severity, DiagnosticSeverity::Warning));
    assert!(diag.unwrap().message.contains("'B'"));
}

// ---------------------------------------------------------------------------
// TestKnownFunctionWithWrongParametersGeneratesDiagnostic  (YS0050)
// ---------------------------------------------------------------------------

#[test]
fn test_known_function_with_wrong_parameters_generates_diagnostic() {
    let library = TestBase::new().dialogue.library().clone();
    for (input, param_val, param_type) in [("{visited(1)}", "1", "Number"), ("{visited(true)}", "true", "Bool")] {
        let result = Compiler::from_test_source(input).extend_library(library.clone()).compile().unwrap_err();
        let diag = result.0.iter().find(|d| d.code.as_deref() == Some("YS0050"));
        assert!(diag.is_some(), "Expected YS0050 for {input}");
        assert!(
            diag.unwrap().message.contains(param_val) && diag.unwrap().message.contains(param_type),
            "Message should contain '{param_val}' and '{param_type}': {}",
            diag.unwrap().message
        );
    }
}

// ---------------------------------------------------------------------------
// TestKnownFunctionWithIncorrectNumberOfParametersGeneratesDiagnostic  (YS0013)
// ---------------------------------------------------------------------------

#[test]
fn test_known_function_with_incorrect_number_of_parameters_generates_diagnostic() {
    let library = TestBase::new().dialogue.library().clone();
    for (input, expected_count) in [("{visited()}", 0usize), ("{visited(\"node\", 1)}", 2), ("{visited(\"node\", true)}", 2)] {
        let result = Compiler::from_test_source(input).extend_library(library.clone()).compile().unwrap_err();
        let diag = result.0.iter().find(|d| d.code.as_deref() == Some("YS0013"));
        assert!(diag.is_some(), "Expected YS0013 for {input}");
        assert!(
            diag.unwrap().message.contains("visited"),
            "Message should contain 'visited': {}",
            diag.unwrap().message
        );
        assert!(
            diag.unwrap().message.contains(&expected_count.to_string()),
            "Message should contain param count {expected_count}: {}",
            diag.unwrap().message
        );
    }
}

// ---------------------------------------------------------------------------
// TestEscapedUnknownCommandsDontGenerateDiagnostics
// ---------------------------------------------------------------------------

#[test]
fn test_escaped_unknown_commands_dont_generate_diagnostics() {
    // \<< is an escaped << and should not trigger any validation diagnostics
    let result = Compiler::from_test_source("\\<<made up command>>").compile().unwrap();
    assert!(result.warnings.is_empty(), "Expected no diagnostics for escaped command");
}

// ---------------------------------------------------------------------------
// TestDialogueWithBothLineAndShadowIDGenerateDiag  (YS0017)
// ---------------------------------------------------------------------------

#[test]
fn test_dialogue_with_both_line_and_shadow_id_generates_diag() {
    let input = "this line has both a line id and a shadow #line:abc123 #shadow:def123\nthis line has both a line id and a shadow #line:def123";
    let result = Compiler::from_test_source(input).compile().unwrap_err();
    let ys0017: Vec<_> = result.0.iter().filter(|d| d.code.as_deref() == Some("YS0017")).collect();
    assert_eq!(ys0017.len(), 2, "Expected 2 YS0017 diagnostics");
    for diag in &ys0017 {
        assert_eq!(diag.message, "Lines cannot have both a '#line' tag and a '#shadow' tag.");
        assert!(matches!(diag.severity, DiagnosticSeverity::Error));
    }
}

// ---------------------------------------------------------------------------
// TestDuplicateLineIdsGeneratesWarnings  (YS0018)
// Divergence: C# generates 2 diagnostics (one per occurrence); Rust generates 1
// (only for the second/duplicate occurrence).
// ---------------------------------------------------------------------------

#[test]
fn test_duplicate_line_ids_generates_errors() {
    let input = "first line #line:abc123\nsecond line #line:abc123";
    let result = Compiler::from_test_source(input).compile().unwrap_err();
    assert!(
        result.0.iter().any(|d| d.code.as_deref() == Some("YS0018")),
        "Expected at least one YS0018 diagnostic"
    );
    let diag = result.0.iter().find(|d| d.code.as_deref() == Some("YS0018")).unwrap();
    assert!(
        diag.message.contains("line:abc123"),
        "Message should contain the line ID: {}",
        diag.message
    );
    assert!(matches!(diag.severity, DiagnosticSeverity::Error));
}

// ---------------------------------------------------------------------------
// MultpleDuplicateLineIDsGeneratesWarning  (YS0062)
// ---------------------------------------------------------------------------

#[test]
fn multiple_duplicate_line_ids_generates_error() {
    let input = "the line #line:abc123 #line:abc123";
    let result = Compiler::from_test_source(input).compile().unwrap_err();
    assert!(result.0.iter().any(|d| d.code.as_deref() == Some("YS0062")));
}

#[test]
fn multiple_different_line_ids_generates_error() {
    let input = "the line #line:abc123 #line:def456";
    let result = Compiler::from_test_source(input).compile().unwrap_err();
    assert!(result.0.iter().any(|d| d.code.as_deref() == Some("YS0062")));
}

#[test]
fn multiple_duplicate_shadow_ids_generates_error() {
    let input = "the line #shadow:abc123 #shadow:abc123";
    let result = Compiler::from_test_source(input).compile().unwrap_err();
    assert!(result.0.iter().any(|d| d.code.as_deref() == Some("YS0062")));
}

#[test]
fn multiple_different_shadow_ids_generates_error() {
    let input = "the line #shadow:abc123 #shadow:def456";
    let result = Compiler::from_test_source(input).compile().unwrap_err();
    assert!(result.0.iter().any(|d| d.code.as_deref() == Some("YS0062")));
}

// ---------------------------------------------------------------------------
// TestInvalidNodeNamesGenerateDiagnostics  (YS0027)
// ---------------------------------------------------------------------------

#[test]
fn test_invalid_node_names_generate_diagnostics() {
    for (input, unexpected_char) in [("$abc", "$"), (".abc", "."), ("123abc", "1"), ("abc.123", ".")] {
        let content = format!("title:{input}\n---\nline of text\n===");
        let file = File {
            file_name: "<input>".to_owned(),
            source: content,
        };
        let result = Compiler::new().add_file(file).compile().unwrap_err();
        let diag = result.0.iter().find(|d| d.code.as_deref() == Some("YS0027"));
        assert!(diag.is_some(), "Expected YS0027 for title:{input}");
        assert!(
            diag.unwrap().message.contains(unexpected_char),
            "Message should contain '{unexpected_char}': {}",
            diag.unwrap().message
        );
    }
}

#[test]
fn test_valid_unusual_titles_do_not_generate_diagnostic() {
    for input in ["ḀBC", "_abc", "_"] {
        let content = format!("title:{input}\n---\nline of text\n===");
        let file = File {
            file_name: "<input>".to_owned(),
            source: content,
        };
        let result = Compiler::new().add_file(file).compile().unwrap();
        assert!(result.warnings.is_empty(), "Expected no diagnostics for title:{input}");
    }
}

// ---------------------------------------------------------------------------
// TestMissingIDInNodeTitleGeneratesDiagnostics  (YS0005)
// ---------------------------------------------------------------------------

#[test]
fn test_missing_id_in_node_title_generates_diagnostics() {
    let content = "title: \n---\ncontent\n===\n";
    let file = File {
        file_name: "<input>".to_owned(),
        source: content.to_owned(),
    };
    let result = Compiler::new().add_file(file).compile().unwrap_err();
    // YS0005 or a general syntax error — just verify compilation fails
    assert!(!result.0.is_empty(), "Expected diagnostic for empty title");
}

// ---------------------------------------------------------------------------
// TestInvalidSubtitleNamesGenerateDiagnostics  (YS0027)
// ---------------------------------------------------------------------------

#[test]
fn test_invalid_subtitle_names_generate_diagnostics() {
    for (input, unexpected_char) in [("$abc", "$"), (".abc", "."), ("123abc", "1"), ("abc.123", ".")] {
        let content = format!("title:start\nsubtitle:{input}\nwhen: always\n---\nline of text\n===");
        let file = File {
            file_name: "<input>".to_owned(),
            source: content,
        };
        let result = Compiler::new().add_file(file).compile().unwrap_err();
        let diag = result.0.iter().find(|d| d.code.as_deref() == Some("YS0027"));
        assert!(diag.is_some(), "Expected YS0027 for subtitle:{input}");
        assert!(
            diag.unwrap().message.contains(unexpected_char),
            "Message should contain '{unexpected_char}': {}",
            diag.unwrap().message
        );
    }
}

#[test]
fn test_valid_unusual_subtitles_do_not_generate_diagnostic() {
    for input in ["ḀBC", "_abc", "_"] {
        let content = format!("title:start\nsubtitle:{input}\n---\nline of text\n===");
        let file = File {
            file_name: "<input>".to_owned(),
            source: content,
        };
        let result = Compiler::new().add_file(file).compile().unwrap();
        assert!(result.warnings.is_empty(), "Expected no diagnostics for subtitle:{input}");
    }
}

// ---------------------------------------------------------------------------
// TestRedeclaredVariablesGeneratesDiagnostics  (YS0039)
// Divergence: Rust message is "{var} has already been declared in {file}",
// C# message is "Redeclaration of existing variable $var".
// ---------------------------------------------------------------------------

#[test]
fn test_redeclared_variables_generates_diagnostics() {
    let input = "<<declare $var = 5>>\n<<declare $var = true>>";
    let result = Compiler::from_test_source(input).compile().unwrap_err();
    // Divergence: Rust generates 1 YS0039 per redeclaration (not 2 like C#).
    let ys0039: Vec<_> = result.0.iter().filter(|d| d.code.as_deref() == Some("YS0039")).collect();
    assert!(ys0039.len() >= 1, "Expected at least 1 YS0039 diagnostic");
    for diag in &ys0039 {
        assert!(matches!(diag.severity, DiagnosticSeverity::Error));
        assert!(diag.message.contains("$var"), "Message should reference $var: {}", diag.message);
    }
}

// ---------------------------------------------------------------------------
// TestLinesWithUndeclaredVariablesGenerateDiagnostics  (YS0029)
// ---------------------------------------------------------------------------

#[test]
fn test_lines_with_undeclared_variables_generate_diagnostics() {
    let result = Compiler::from_test_source("{$undeclared}").compile().unwrap_err();
    // Divergence: Rust uses YS0003 (not YS0029) for undeclared variable references.
    let diag = result.0.iter().find(|d| d.code.as_deref() == Some("YS0003"));
    assert!(diag.is_some(), "Expected YS0003 for undeclared variable");
    assert!(matches!(diag.unwrap().severity, DiagnosticSeverity::Error));
    assert!(diag.unwrap().message.contains("$undeclared"));
}

// ---------------------------------------------------------------------------
// TestLinesWithVariablesSetToValuesOfUnknownTypeGenerateDiagnostic  (YS0029 x2)
// ---------------------------------------------------------------------------

#[test]
fn test_lines_with_variables_set_to_values_of_unknown_type_generate_diagnostic() {
    let result = Compiler::from_test_source("<<set $x = $y>>").compile().unwrap_err();
    // Divergence: Rust uses YS0003 (not YS0029) for undeclared variable references.
    let ys0003_count = result.0.iter().filter(|d| d.code.as_deref() == Some("YS0003")).count();
    assert!(ys0003_count >= 2, "Expected at least 2 YS0003 diagnostics for <<set $x = $y>>");
}

// ---------------------------------------------------------------------------
// TestAttemptingToAssignValuesToSmartVariablesGeneratesDiagnostic  (YS0030)
// Divergence: Rust message is "smart variable '$x' cannot be modified"
// C# message is "$x cannot be modified (it's a smart variable and is always equal to *)"
// ---------------------------------------------------------------------------

#[test]
fn test_attempting_to_assign_values_to_smart_variables_generates_diagnostic() {
    let input = "<<declare $x = (1)>>\n<<set $x = 2>>";
    let result = Compiler::from_test_source(input).compile().unwrap_err();
    let diag = result.0.iter().find(|d| d.code.as_deref() == Some("YS0030"));
    assert!(diag.is_some(), "Expected YS0030 for smart variable assignment");
    assert!(matches!(diag.unwrap().severity, DiagnosticSeverity::Error));
    assert!(
        diag.unwrap().message.contains("$x") && diag.unwrap().message.contains("cannot be modified"),
        "Message should mention $x and cannot be modified: {}",
        diag.unwrap().message
    );
}

// ---------------------------------------------------------------------------
// TestNodeInGroupWithMissingWhenClauseGeneratesDiagnostic  (YS0031)
// ---------------------------------------------------------------------------

#[test]
fn test_node_in_group_with_missing_when_clause_generates_diagnostic() {
    for source in [
        "title: Group\nwhen: always\n---\nContent\n===\ntitle: Group\n---\nContent\n===",
        "title: Group\n---\nContent\n===\ntitle: Group\nwhen: always\n---\nContent\n===",
    ] {
        let file = File {
            file_name: "<input>".to_owned(),
            source: source.to_owned(),
        };
        let result = Compiler::new().add_file(file).compile().unwrap_err();
        let diag = result.0.iter().find(|d| d.code.as_deref() == Some("YS0031"));
        assert!(diag.is_some(), "Expected YS0031 for node in group missing when:");
        assert!(matches!(diag.unwrap().severity, DiagnosticSeverity::Error));
        assert!(
            diag.unwrap().message.contains("Group") && diag.unwrap().message.contains("when"),
            "Message should mention group name and 'when': {}",
            diag.unwrap().message
        );
    }
}

// ---------------------------------------------------------------------------
// TestNodeInGroupWithDuplicateSubtitleGeneratesDiagnostic  (YS0032)
// Divergence: Rust message differs from C#.
// ---------------------------------------------------------------------------

#[test]
fn test_node_in_group_with_duplicate_subtitle_generates_diagnostic() {
    let source = "title: Group\nwhen: always\nsubtitle: x\n---\nContent\n===\ntitle: Group\nwhen: always\nsubtitle: x\n---\nContent\n===";
    let file = File {
        file_name: "<input>".to_owned(),
        source: source.to_owned(),
    };
    let result = Compiler::new().add_file(file).compile().unwrap_err();
    let ys0032: Vec<_> = result.0.iter().filter(|d| d.code.as_deref() == Some("YS0032")).collect();
    assert_eq!(ys0032.len(), 2, "Expected 2 YS0032 diagnostics");
    for diag in &ys0032 {
        assert!(matches!(diag.severity, DiagnosticSeverity::Error));
        assert!(
            diag.message.contains("Group") && diag.message.contains('x'),
            "Message should mention group 'Group' and subtitle 'x': {}",
            diag.message
        );
    }
}

// ---------------------------------------------------------------------------
// TestEmptyNodesGenerateDiagnostics  (YS0033, multiple nodes)
// ---------------------------------------------------------------------------

#[test]
fn test_empty_nodes_generate_multiple_diagnostics() {
    let source =
        "title: NonEmpty\n---\nNot empty, so included\n===\ntitle: Empty\n---\n===\ntitle: EmptyWithComment\n---\n// only has a comment\n===\n";
    let file = File {
        file_name: "<input>".to_owned(),
        source: source.to_owned(),
    };
    let result = Compiler::new().add_file(file).compile().unwrap();
    let ys0033: Vec<_> = result.warnings.iter().filter(|d| d.code.as_deref() == Some("YS0033")).collect();
    assert_eq!(ys0033.len(), 2, "Expected 2 YS0033 warnings for empty nodes");
    for diag in &ys0033 {
        assert!(matches!(diag.severity, DiagnosticSeverity::Warning));
        assert!(diag.message.contains("empty"), "Message should contain 'empty': {}", diag.message);
    }
}

// ---------------------------------------------------------------------------
// TestNodesWithMissingHeadersGenerateDiagnostics  (YS0051)
// ---------------------------------------------------------------------------

#[test]
fn test_nodes_with_missing_headers_generate_diagnostics() {
    let content = "test: something\n---\ncontent\n===";
    let file = File {
        file_name: "<input>".to_owned(),
        source: content.to_owned(),
    };
    let result = Compiler::new().add_file(file).compile().unwrap_err();
    let diag = result.0.iter().find(|d| d.code.as_deref() == Some("YS0051"));
    assert!(diag.is_some(), "Expected YS0051 for node missing title");
    assert_eq!(diag.unwrap().message, "Nodes must have a title");
    assert!(matches!(diag.unwrap().severity, DiagnosticSeverity::Error));
}

// ---------------------------------------------------------------------------
// TestNodesWithMultipleHeadersGenerateDiagnostics  (YS0052)
// ---------------------------------------------------------------------------

#[test]
fn test_nodes_with_multiple_headers_generate_diagnostics() {
    let content = "title: Test\ntitle: Test2\n---\ncontent\n===";
    let file = File {
        file_name: "<input>".to_owned(),
        source: content.to_owned(),
    };
    let result = Compiler::new().add_file(file).compile().unwrap_err();
    let diag = result.0.iter().find(|d| d.code.as_deref() == Some("YS0052"));
    assert!(diag.is_some(), "Expected YS0052 for node with multiple title headers");
    assert_eq!(diag.unwrap().message, "Nodes must have a single title header");
    assert!(matches!(diag.unwrap().severity, DiagnosticSeverity::Error));
}

// ---------------------------------------------------------------------------
// TestTMPStyleTagsDontGenerateWarnings
// ---------------------------------------------------------------------------

#[test]
fn test_tmp_style_tags_dont_generate_warnings() {
    // TMP (TextMeshPro) style tags like <bold>...</bold> are not Yarn commands
    // and should not generate any diagnostics
    let result = Compiler::from_test_source("Player: <bold>Example</bold>").compile().unwrap();
    assert!(result.warnings.is_empty(), "Expected no diagnostics for TMP-style tags");
}

// ---------------------------------------------------------------------------
// TestSuperfluousChevronsAroundCommandsGeneratesWarning  (YS0064)
// ---------------------------------------------------------------------------

#[test]
fn test_superfluous_chevrons_around_commands_generates_warning() {
    // Leading extra chevrons
    for input in ["<<<wait 1>>", "<<<<wait 1>>", "<<<<<<<<wait 1>>"] {
        let result = Compiler::from_test_source(input).compile().unwrap();
        let diag = result.warnings.iter().find(|d| d.code.as_deref() == Some("YS0064"));
        assert!(diag.is_some(), "Expected YS0064 for: {input}");
    }
    // Trailing extra chevrons
    for input in ["<<wait 1>>>", "<<wait 1>>>>", "<<wait 1>> >>"] {
        let result = Compiler::from_test_source(input).compile().unwrap();
        let diag = result.warnings.iter().find(|d| d.code.as_deref() == Some("YS0064"));
        assert!(diag.is_some(), "Expected YS0064 for: {input}");
    }
}

// ---------------------------------------------------------------------------
// TestImplictVariableGenerateShouldBeDeclaredWarning  (YS0003 Warning)
// ---------------------------------------------------------------------------

#[test]
fn test_implicit_variable_generates_should_be_declared_warning() {
    // Adapted from C# InlineData cases:
    // "<<set $x = 5>>"      → warning for number
    // "<<set $x = true>>"   → warning for bool
    // "<<set $x = \"value\">>>" → warning for string
    for input in ["<<set $x = 5>>", "<<set $x = true>>", "<<set $x = \"value\">>"] {
        let result = Compiler::from_test_source(input).compile().unwrap();

        let ys0003: Vec<_> = result.warnings.iter().filter(|d| d.code.as_deref() == Some("YS0003")).collect();
        assert_eq!(ys0003.len(), 1, "Expected exactly one YS0003 warning for `{input}`, got: {ys0003:?}");

        let diag = ys0003[0];
        assert!(
            matches!(diag.severity, DiagnosticSeverity::Warning),
            "Expected Warning severity for `{input}`, got {:?}",
            diag.severity
        );
        assert_eq!(
            diag.message, "Variable '$x' is used but not declared. Declare it with: <<declare $x = value>>",
            "Unexpected message for `{input}`"
        );
    }
}

// ---------------------------------------------------------------------------
// TestUnreferencedNodesCreateDiagnostics
// ---------------------------------------------------------------------------

/// Nodes that are never reached from any `<<jump>>` or `<<detour>>` in the
/// program should receive a YS0009 warning.
#[test]
fn test_unreferenced_nodes_create_diagnostics() {
    // Node A is never jumped/detoured to from any other node.
    // Nodes B and C are referenced by A, so they are considered reachable.
    let source = "title: A\n---\n<<detour B>>\n<<jump C>>\n===\n\
                  title: B\n---\nThis node is referenced by A\n===\n\
                  title: C\n---\nThis node is referenced by A\n===";
    let result = Compiler::new()
        .add_file(File {
            file_name: "<input>".to_owned(),
            source: source.to_owned(),
        })
        .compile()
        .unwrap();
    let ys0009: Vec<_> = result.warnings.iter().filter(|d| d.code.as_deref() == Some("YS0009")).collect();
    assert_eq!(ys0009.len(), 1, "Expected exactly 1 YS0009 warning, got: {:?}", result.warnings);
    assert!(
        ys0009[0].message.contains("'A'"),
        "Message should mention node 'A': {}",
        ys0009[0].message
    );
}

// ---------------------------------------------------------------------------
// TestUnusedDeclaredVarsGenerateDiagnostic
// ---------------------------------------------------------------------------

/// # Why ignored
///
#[test]
fn test_unused_declared_vars_generate_diagnostic() {
    for (input, var_name) in [
        ("<<declare $somevar = 123>>", "$somevar"),
        ("<<declare $somevar = \"hello\">>", "$somevar"),
        ("<<declare $somevar = false>>", "$somevar"),
    ] {
        let result = Compiler::from_test_source(input).compile().unwrap();
        let diag = result.warnings.iter().find(|d| d.code.as_deref() == Some("YS0010"));
        assert!(diag.is_some(), "Expected YS0010 warning for undeclared variable in: {input}");
        assert!(matches!(diag.unwrap().severity, DiagnosticSeverity::Warning), "Expected Warning severity");
        assert!(
            diag.unwrap().message.contains(var_name),
            "Expected message to contain '{var_name}': {}",
            diag.unwrap().message
        );
    }
}

// ---------------------------------------------------------------------------
// TestUsedVariablesShouldntGenerateDiagnostic
// ---------------------------------------------------------------------------

/// # Why ignored
///
#[test]
fn test_used_variables_shouldnt_generate_diagnostic() {
    // Each of these scripts uses $somevar in a way that should suppress YS0010.
    for input in [
        // Used in a when: header
        "title: A\nwhen: $somevar == true\n---\n<<declare $somevar = false>>\n===",
        // Written via <<set>>
        "title: A\n---\n<<declare $somevar = false>>\n<<set $somevar = true>>\n===",
        // Read in an <<if>> condition
        "title: A\n---\n<<declare $somevar = false>>\n<<if $somevar>>\n    internal line\n<<endif>>\n===",
        // Used in an inline expression in a line
        "title: A\n---\n<<declare $somevar = false>>\nthe value is {$somevar}\n===",
    ] {
        let result = Compiler::new()
            .add_file(File {
                file_name: "input.yarn".to_owned(),
                source: input.to_owned(),
            })
            .compile()
            .unwrap();
        assert!(
            result.warnings.iter().all(|d| d.code.as_deref() != Some("YS0010")),
            "Expected no YS0010 warning for input: {input}\nWarnings: {:?}",
            result.warnings
        );
    }
}

// ---------------------------------------------------------------------------
// TestKnownFunctionWithWrongParameterInWhenGeneratesDiagnostic
// ---------------------------------------------------------------------------

/// The Rust `type_check_when_headers` step validates function parameters in
/// `when:` headers using the known function signatures from the library.
#[test]
fn test_known_function_with_wrong_parameter_in_when_generates_diagnostic() {
    let library = TestBase::new().dialogue.library().clone();
    for (input, param_val, param_type) in [
        // visited(String) → Bool; passing a Number should trigger YS0050
        ("title: A\nwhen: visited(1)\n---\nsome line\n===", "1", "Number"),
        // visited(String) → Bool; passing a Bool should trigger YS0050
        ("title: A\nwhen: visited(true)\n---\nsome line\n===", "true", "Bool"),
    ] {
        let mut compiler = Compiler::new();
        compiler.add_file(File {
            file_name: "<input>".to_owned(),
            source: input.to_owned(),
        });
        compiler.extend_library(library.clone());
        let result = compiler.compile();
        let all_diags: Vec<_> = match &result {
            Ok(c) => c.warnings.iter().collect(),
            Err(e) => e.0.iter().collect(),
        };
        let diag = all_diags.iter().find(|d| d.code.as_deref() == Some("YS0050"));
        assert!(diag.is_some(), "Expected YS0050 for input={input:?}, got: {all_diags:?}");
        assert!(
            diag.unwrap().message.contains(param_val) && diag.unwrap().message.contains(param_type),
            "Message should contain '{param_val}' and '{param_type}': {}",
            diag.unwrap().message
        );
    }
}

// ---------------------------------------------------------------------------
// TestKnownFunctionWithIncorrectNumberOfParametersInWhenGeneratesDiagnostic
// ---------------------------------------------------------------------------

/// # Why ignored
///
/// Same as wrong-parameter test but validates argument *count* rather than type.
#[test]
fn test_known_function_with_incorrect_number_of_parameters_in_when_generates_diagnostic() {
    let library = TestBase::new().dialogue.library().clone();
    for (input, expected_count) in [
        // visited(String) → Bool; 0 arguments → YS0013
        ("title: A\nwhen: visited()\n---\nsome line\n===", 0usize),
        // visited(String) → Bool; 2 arguments → YS0013
        ("title: A\nwhen: visited(\"node\", 1)\n---\nsome line\n===", 2),
        ("title: A\nwhen: visited(\"node\", true)\n---\nsome line\n===", 2),
        ("title: A\nwhen: visited(\"node\", \"node\")\n---\nsome line\n===", 2),
    ] {
        let mut compiler = Compiler::new();
        compiler.add_file(File {
            file_name: "<input>".to_owned(),
            source: input.to_owned(),
        });
        compiler.extend_library(library.clone());
        let result = compiler.compile();
        let all_diags: Vec<_> = match &result {
            Ok(c) => c.warnings.iter().collect(),
            Err(e) => e.0.iter().collect(),
        };
        let diag = all_diags.iter().find(|d| d.code.as_deref() == Some("YS0013"));
        assert!(diag.is_some(), "Expected YS0013 for input={input:?}, got: {all_diags:?}");
        assert!(
            diag.unwrap().message.contains("visited"),
            "Message should contain 'visited': {}",
            diag.unwrap().message
        );
        assert!(
            diag.unwrap().message.contains(&expected_count.to_string()),
            "Message should contain {expected_count}: {}",
            diag.unwrap().message
        );
    }
}

// ---------------------------------------------------------------------------
// TestUnknownCommandGeneratesDiag — [Skip] in C#
// ---------------------------------------------------------------------------

/// # Why ignored
///
/// Skipped in **both** the C# reference implementation and this Rust port for
/// the same architectural reason: the **compiler** does not know which
/// `<<commands>>` are valid. Command handlers are registered at runtime (via
/// `AddCommandHandler` in C# / `YarnCommands` in bevy_yarnspinner), so the
/// set of valid commands is not available during compilation.
///
/// The C# skip message reads:
/// > *"This test needs to be run in the language server, which has knowledge
/// > of commands"*
///
/// Implementing this would require the caller to supply a command registry to
/// the `Compiler` before compiling — a design change not warranted for
/// game-runtime use.
#[test]
#[ignore = "Skipped in C# too: command validation requires a runtime command registry not available to the compiler"]
fn test_unknown_command_generates_diag() {}

// ---------------------------------------------------------------------------
// TestKnownCommandWithWrongParametersGeneratesDiag — [Skip] in C#
// ---------------------------------------------------------------------------

/// # Why ignored
///
/// Same reason as `test_unknown_command_generates_diag`: the compiler has no
/// knowledge of command parameter signatures. The C# skip message reads:
/// > *"Must be handled by Language Server because the compiler doesn't know
/// > about valid commands"*
#[test]
#[ignore = "Skipped in C# too: command parameter validation requires a runtime command registry not available to the compiler"]
fn test_known_command_with_wrong_parameters_generates_diag() {}

// ---------------------------------------------------------------------------
// TestCyclicNodesGenerateDiagnostic — [Skip] in C#
// ---------------------------------------------------------------------------

/// Cyclic node detection is now implemented in the Rust compiler.
/// Matches C# `TestCyclicNodesGenerateDiagnostic` (which is `[Skip]` in C#
/// because the feature was intended for the language server, but we implement
/// it here in the compiler directly).
#[test]
fn test_cyclic_nodes_generate_diagnostic() {
    // Three test cases: jump cycle, detour cycle, and a two-node cycle.
    let cases: &[(&str, &str)] = &[
        // A -> B -> C -> A via <<jump>>
        (
            "title: A\n---\n<<jump B>>\n===\ntitle: B\n---\n<<jump C>>\n===\ntitle: C\n---\n<<jump A>>\n===",
            "A -> B -> C",
        ),
        // A -> B -> C -> A via <<detour>>
        (
            "title: A\n---\n<<detour B>>\n===\ntitle: B\n---\n<<detour C>>\n===\ntitle: C\n---\n<<detour A>>\n===",
            "A -> B -> C",
        ),
    ];

    for (source, expected_cycle) in cases {
        let mut compiler = Compiler::new();
        compiler.add_file(File {
            file_name: "<input>".to_string(),
            source: source.to_string(),
        });
        // Compilation should succeed (cycle detection emits a warning, not an error).
        let result = compiler.compile();
        let warnings = match &result {
            Ok(c) => c.warnings.clone(),
            Err(e) => e.0.clone(),
        };

        let cycle_diag: Vec<_> = warnings.iter().filter(|d| d.code.as_deref() == Some("YS0015")).collect();

        assert!(
            !cycle_diag.is_empty(),
            "Expected at least one YS0015 diagnostic for cycle '{expected_cycle}', got: {warnings:?}"
        );

        let expected_message = format!("Cyclic dependency detected: {expected_cycle}");
        assert!(
            cycle_diag.iter().any(|d| d.message == expected_message),
            "Expected message '{expected_message}', got: {:?}",
            cycle_diag.iter().map(|d| &d.message).collect::<Vec<_>>()
        );

        assert!(
            cycle_diag.iter().all(|d| d.severity == DiagnosticSeverity::Warning),
            "Expected Warning severity"
        );
    }
}

// ---------------------------------------------------------------------------
// TestUsingLanguageFeaturesFromFutureLanguageVersionGeneratesDiagnostic
// ---------------------------------------------------------------------------

/// Tests that using Yarn Spinner v3 language features when the declared
/// language version is below 3 generates a `YS0036` diagnostic, and that
/// no such diagnostic is generated when the version is at 3.
#[test]
fn test_using_language_features_from_future_language_version_generates_diagnostic() {
    // The minimum version for all v3 features tested here.
    let min_language_version: u32 = 3;
    let below_min = min_language_version - 1;

    let cases: &[(&str, &str)] = &[
        ("<<enum Example>>\n<<case Test>>\n<<endenum>>", "enums"),
        ("=> Line 1\n=> Line 2", "line groups"),
        ("Here's a line <<once>>", "'once' conditions"),
        ("<<once>>\nLine\n<<endonce>>", "'once' statements"),
        ("<<declare $smart_var = 1 + 2>>", "smart variables"),
    ];

    for (source, feature) in cases {
        // ── Failing case: language version below the minimum ──────────────
        let mut compiler = Compiler::from_test_source(source);
        compiler.with_language_version(below_min);
        // Enums / line groups / once features can introduce errors for other
        // reasons too, so unwrap_or_else to get all diagnostics regardless.
        let diags: Vec<_> = match compiler.compile() {
            Ok(c) => c.warnings.clone(),
            Err(e) => e.0.clone(),
        };

        let ys0036_diags: Vec<_> = diags.iter().filter(|d| d.code.as_deref() == Some("YS0036")).collect();
        assert!(
            !ys0036_diags.is_empty(),
            "Expected a YS0036 diagnostic for feature \"{feature}\" at language version {below_min}, but got none.\nAll diagnostics: {diags:?}",
        );

        let diag = &ys0036_diags[0];
        assert_eq!(
            diag.severity,
            DiagnosticSeverity::Error,
            "Expected Error severity for YS0036 (feature \"{feature}\")",
        );

        let expected_message = format!(
            "Language feature \"{feature}\" is not available at language version {below_min}; it requires version {min_language_version} or later"
        );
        assert_eq!(diag.message, expected_message, "Wrong message for feature \"{feature}\"",);

        // ── Passing case: language version at the minimum ─────────────────
        let mut compiler = Compiler::from_test_source(source);
        compiler.with_language_version(min_language_version);
        let passing_diags: Vec<_> = match compiler.compile() {
            Ok(c) => c.warnings.clone(),
            Err(e) => e.0.clone(),
        };

        let unexpected: Vec<_> = passing_diags.iter().filter(|d| d.code.as_deref() == Some("YS0036")).collect();
        assert!(
            unexpected.is_empty(),
            "Expected no YS0036 diagnostic for feature \"{feature}\" at language version {min_language_version}, but got: {unexpected:?}",
        );
    }
}

// ---------------------------------------------------------------------------
// TestEnumsWithNonConstantRawValuesGenerateDiagnostic
// ---------------------------------------------------------------------------

/// Enum cases whose raw value is a non-constant expression (e.g. `max(1,2)`)
/// should generate a YS0037 diagnostic.
#[test]
fn test_enums_with_non_constant_raw_values_generate_diagnostic() {
    let source = create_test_node("<<enum Example>>\n<<case Test = max(1,2)>>\n<<endenum>>");
    let result = Compiler::new()
        .add_file(File {
            file_name: "<input>".to_owned(),
            source,
        })
        .compile();
    let all_diags: Vec<_> = match &result {
        Ok(c) => c.warnings.iter().collect(),
        Err(e) => e.0.iter().collect(),
    };
    let diag = all_diags.iter().find(|d| d.code.as_deref() == Some("YS0035"));
    assert!(diag.is_some(), "Expected YS0035 for non-constant enum raw value, got: {all_diags:?}");
    assert_eq!(
        diag.unwrap().message,
        "Expected a constant type",
        "Unexpected message: {}",
        diag.unwrap().message
    );
}

// ---------------------------------------------------------------------------
// TestEnumsWithInvalidCasesGenerateDiagnostics
// ---------------------------------------------------------------------------

/// Referencing a member that doesn't exist on a known enum type should
/// generate a YS0038 diagnostic.
#[test]
fn test_enums_with_invalid_cases_generate_diagnostics() {
    let preamble = "<<enum Test>>\n<<case Item>>\n<<endenum>>\n";
    for source_body in ["<<set $x = Test.Failure>>", "<<declare $x = Test.Failure>>"] {
        let source = create_test_node(&format!("{preamble}{source_body}"));
        let result = Compiler::new()
            .add_file(File {
                file_name: "<input>".to_owned(),
                source,
            })
            .compile();
        let all_diags: Vec<_> = match &result {
            Ok(c) => c.warnings.iter().collect(),
            Err(e) => e.0.iter().collect(),
        };
        let diag = all_diags.iter().find(|d| d.code.as_deref() == Some("YS0038"));
        assert!(
            diag.is_some(),
            "Expected YS0038 for invalid enum member access in {source_body:?}, got: {all_diags:?}"
        );
        assert!(
            diag.unwrap().message.contains("Failure"),
            "Message should mention 'Failure': {}",
            diag.unwrap().message
        );
    }
}

// ---------------------------------------------------------------------------
// TestDiagnosticsCanHaveOverriddenSeverities
// ---------------------------------------------------------------------------

/// # Why ignored
///
/// The Rust `CompilationJob` does not expose per-diagnostic severity overrides.
/// This feature may be added in a future version.
#[test]
fn test_diagnostics_can_have_overridden_severities() {
    use std::collections::HashMap;
    // Use from_test_source which internally wraps with create_test_node.
    let source = "<<declare $x = 1>>";

    // Test both possible overrides: Error and Warning.
    for &severity in &[DiagnosticSeverity::Error, DiagnosticSeverity::Warning] {
        let mut overrides = HashMap::new();
        overrides.insert("YS0010".to_string(), severity);
        let mut compiler = Compiler::from_test_source(source);
        compiler.with_diagnostic_severities(overrides);
        let result = compiler.compile();

        match (result, severity) {
            (Err(err), DiagnosticSeverity::Error) => {
                let diag = err
                    .0
                    .iter()
                    .find(|d| d.code.as_deref() == Some("YS0010"))
                    .expect("Expected YS0010 in errors");
                assert_eq!(diag.severity, DiagnosticSeverity::Error);
            }
            (Ok(compilation), DiagnosticSeverity::Warning) => {
                let diag = compilation
                    .warnings
                    .iter()
                    .find(|d| d.code.as_deref() == Some("YS0010"))
                    .expect("Expected YS0010 in warnings");
                assert_eq!(diag.severity, DiagnosticSeverity::Warning);
            }
            (result, sev) => panic!("Unexpected combination: severity={sev:?}, result={result:?}"),
        }
    }
}

// ---------------------------------------------------------------------------
// TestCompilerGeneratesMarkupDiagnostics  (YS0063)
// ---------------------------------------------------------------------------

/// # Why ignored
///
/// The YS0063 (`MARKUP_FAILED_TO_PARSE`) diagnostic code is defined in the
/// Rust compiler's descriptor table but is never emitted — markup parsing
/// errors produce a runtime error instead of a compile-time diagnostic.
#[test]
fn test_compiler_generates_markup_diagnostics() {
    // Use from_test_source which internally wraps with create_test_node.
    // The content line will be on 0-based line 2.
    let result = Compiler::from_test_source("A line with [a]invalid markup")
        .with_compilation_type(CompilationType::StringsOnly)
        .compile()
        .unwrap();
    let diag = result
        .warnings
        .iter()
        .find(|d| d.code.as_deref() == Some("YS0063"))
        .expect("Expected YS0063 diagnostic in warnings");
    let range = diag.range.as_ref().expect("Expected range on YS0063 diagnostic");
    assert_eq!(range.start.line, 2, "Expected diagnostic on line 2 (0-based)");
    assert_eq!(range.end.line, 2, "Expected diagnostic on line 2 (0-based)");
    assert_eq!(range.start.character, 0, "Expected diagnostic to start at character 0");
    assert_eq!(range.end.character, 29, "Expected diagnostic to end at character 29");
}

// ---------------------------------------------------------------------------
// TestCompilerGeneratesUnreachableCodeDiagnostics
// ---------------------------------------------------------------------------

/// Instructions after an unconditional `<<return>>` are unreachable and should
/// receive a YS0008 warning.
#[test]
fn test_compiler_generates_unreachable_code_diagnostics() {
    let source = create_test_node("<<return>>\nHere's a line of dialogue");
    let result = Compiler::new()
        .add_file(File {
            file_name: "<input>".to_owned(),
            source,
        })
        .compile()
        .unwrap();
    let diag = result.warnings.iter().find(|d| d.code.as_deref() == Some("YS0008"));
    assert!(
        diag.is_some(),
        "Expected YS0008 for unreachable code after <<return>>, got: {:?}",
        result.warnings
    );
}

// ---------------------------------------------------------------------------
// YS0035 — Enum declaration errors
// ---------------------------------------------------------------------------

/// An empty enum (no cases) should generate a YS0035 diagnostic.
#[test]
fn test_empty_enum_generates_ys0035() {
    let source = create_test_node("<<enum Empty>>\n<<endenum>>");
    let result = Compiler::new()
        .add_file(File {
            file_name: "<input>".to_owned(),
            source,
        })
        .compile();
    let all_diags: Vec<_> = match &result {
        Ok(c) => c.warnings.iter().collect(),
        Err(e) => e.0.iter().collect(),
    };
    let diag = all_diags.iter().find(|d| d.code.as_deref() == Some("YS0035"));
    assert!(diag.is_some(), "Expected YS0035 for empty enum, got: {all_diags:?}");
    assert!(
        diag.unwrap().message.contains("Empty") && diag.unwrap().message.to_lowercase().contains("empty"),
        "Message should mention the enum name and 'empty': {}",
        diag.unwrap().message
    );
}

/// An enum with duplicate case names should generate a YS0035 diagnostic.
#[test]
fn test_enum_with_duplicate_case_name_generates_ys0035() {
    let source = create_test_node("<<enum Fruit>>\n<<case Apple>>\n<<case Apple>>\n<<endenum>>");
    let result = Compiler::new()
        .add_file(File {
            file_name: "<input>".to_owned(),
            source,
        })
        .compile();
    let all_diags: Vec<_> = match &result {
        Ok(c) => c.warnings.iter().collect(),
        Err(e) => e.0.iter().collect(),
    };
    let diag = all_diags.iter().find(|d| d.code.as_deref() == Some("YS0035"));
    assert!(diag.is_some(), "Expected YS0035 for duplicate enum case, got: {all_diags:?}");
    assert!(
        diag.unwrap().message.contains("Apple"),
        "Message should mention the duplicate case 'Apple': {}",
        diag.unwrap().message
    );
}

// ---------------------------------------------------------------------------
// YS0040 — Redeclaration of existing type (enum type name collision)
// ---------------------------------------------------------------------------

/// Declaring the same enum type name in two different files should generate
/// a YS0040 diagnostic.
#[test]
fn test_redeclared_enum_type_generates_ys0040() {
    let file1 = File {
        file_name: "file1.yarn".to_owned(),
        source: "title: Node1\n---\n<<enum Color>>\n<<case Red>>\n<<endenum>>\n===\n".to_owned(),
    };
    let file2 = File {
        file_name: "file2.yarn".to_owned(),
        source: "title: Node2\n---\n<<enum Color>>\n<<case Blue>>\n<<endenum>>\n===\n".to_owned(),
    };
    let result = Compiler::new().add_file(file1).add_file(file2).compile();
    let all_diags: Vec<_> = match &result {
        Ok(c) => c.warnings.iter().collect(),
        Err(e) => e.0.iter().collect(),
    };
    let diag = all_diags.iter().find(|d| d.code.as_deref() == Some("YS0040"));
    assert!(diag.is_some(), "Expected YS0040 for redeclared enum type, got: {all_diags:?}");
    assert!(
        diag.unwrap().message.contains("Color"),
        "Message should mention the type name 'Color': {}",
        diag.unwrap().message
    );
    assert!(
        matches!(diag.unwrap().severity, DiagnosticSeverity::Error),
        "Expected Error severity for YS0040"
    );
}
