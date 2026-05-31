//! Adapted from <https://github.com/YarnSpinnerTool/YarnSpinner/blob/3a5b7343f715e4e9a3705fa4224e7fa510b92f1c/YarnSpinner.Tests/DialogueTests.cs>

#[cfg(feature = "bevy")]
use bevy::prelude::World;
use bevy_platform::collections::HashMap;
use test_base::prelude::*;
use yarnspinner::compiler::*;
use yarnspinner::runtime::*;

mod test_base;

#[test]
fn test_node_exists() {
    let path = space_demo_scripts_path().join("Sally.yarn");
    let test_base = TestBase::new();

    let result = Compiler::new()
        .read_file(path)
        .extend_library(test_base.dialogue.library().clone())
        .compile()
        .unwrap();

    let mut dialogue = test_base.dialogue;
    dialogue.replace_program(result.program.unwrap());
    assert!(dialogue.node_exists("Sally"));

    // Test clearing everything
    dialogue.unload_all();
    assert!(!dialogue.node_exists("Sally"));
}

#[test]
fn test_analysis() {
    let mut context = Context::default_analysers();
    let test_base = TestBase::new();

    // this script has the following variables:
    // $foo is read from and written to
    // $bar is written to but never read
    // this means that there should be one diagnosis result
    let path = test_data_path().join("AnalysisTest.yarn");
    let result = Compiler::new()
        .read_file(path)
        .extend_library(test_base.dialogue.library().clone())
        .compile()
        .unwrap();

    test_base.with_compilation(result).dialogue.analyse(&mut context);

    let diagnoses: Vec<_> = context
        .finish_analysis()
        .into_iter()
        .filter(|d| d.severity == DiagnosisSeverity::Warning)
        .collect();
    println!("{diagnoses:#?}");

    assert_eq!(1, diagnoses.len());
    assert!(diagnoses[0].message.contains("Variable $bar is assigned, but never read from"));
}

/// Split off from `test_analysis`
#[test]
fn test_analysis_has_no_false_positives() {
    let test_base = TestBase::new();
    let result = Compiler::new()
        .read_file(space_demo_scripts_path().join("Sally.yarn"))
        .read_file(space_demo_scripts_path().join("Ship.yarn"))
        .extend_library(test_base.dialogue.library().clone())
        .compile()
        .unwrap();
    let mut context = Context::default_analysers();
    test_base.with_compilation(result).dialogue.analyse(&mut context);

    let diagnoses: Vec<_> = context
        .finish_analysis()
        .into_iter()
        .filter(|d| d.severity == DiagnosisSeverity::Warning)
        .collect();
    println!("{diagnoses:#?}");
    assert!(diagnoses.is_empty());
}

#[test]
fn test_missing_node() {
    let path = test_data_path().join("TestCases").join("Smileys.yarn");

    let result = Compiler::new().read_file(path).compile().unwrap();

    let mut test_base = TestBase::new()
        .with_program(result.program.unwrap())
        .with_runtime_errors_do_not_cause_failure();
    let result = test_base.dialogue.set_node("THIS NODE DOES NOT EXIST");
    assert!(result.is_err());
}

#[test]
fn test_getting_current_node_name() {
    let path = space_demo_scripts_path().join("Sally.yarn");
    let test_base = TestBase::new();

    let result = Compiler::new()
        .read_file(path)
        .extend_library(test_base.dialogue.library().clone())
        .compile()
        .unwrap();

    let mut dialogue = test_base.dialogue;
    dialogue.replace_program(result.program.unwrap());

    // dialogue should not be running yet
    assert!(dialogue.current_node().is_none());

    dialogue.set_node("Sally").unwrap();
    assert_eq!(dialogue.current_node(), Some("Sally".to_string()));

    let stop_events = dialogue.stop();
    assert_eq!(Some(DialogueEvent::DialogueComplete), stop_events.into_iter().next_back());

    // Current node should now be none
    assert!(dialogue.current_node().is_none());
}

#[test]
fn test_getting_raw_source() {
    let path = test_data_path().join("Example.yarn");
    let mut test_base = TestBase::new();

    let result = Compiler::new().read_file(path).compile().unwrap();

    test_base = test_base.with_compilation(result);
    let dialogue = &test_base.dialogue;

    let source_id = dialogue.get_line_id_for_node("LearnMore").unwrap();
    let source = test_base.string_table.get_text(&source_id).unwrap();

    assert_eq!(source, "A: HAHAHA\n");
}

#[test]
fn test_getting_tags() {
    let path = test_data_path().join("Example.yarn");
    let mut test_base = TestBase::new();

    let result = Compiler::new().read_file(path).compile().unwrap();

    test_base = test_base.with_program(result.program.unwrap());
    let dialogue = &test_base.dialogue;

    let tags: Vec<String> = dialogue
        .get_header_value("LearnMore", "tags")
        .unwrap()
        .split_whitespace()
        .map(str::to_owned)
        .collect();

    assert_eq!(tags, vec!["rawText"]);
}

#[test]
fn test_getting_headers() {
    let path = test_data_path().join("Example.yarn");
    let mut test_base = TestBase::new();

    let result = Compiler::new().read_file(path).compile().unwrap();

    test_base = test_base.with_program(result.program.unwrap());
    let dialogue = &test_base.dialogue;

    let headers = dialogue.get_headers_for_node("LearnMore").unwrap();

    let mut expected_headers = HashMap::default();
    expected_headers.insert("title".to_string(), "LearnMore".to_string());
    expected_headers.insert("tags".to_string(), "rawText".to_string());
    expected_headers.insert("colorID".to_string(), "0".to_string());
    expected_headers.insert("position".to_string(), "763,472".to_string());

    assert_eq!(headers, expected_headers);
}

/// ## Implementation note
/// Corresponds to `TestPrepareForLine`
#[test]
fn test_line_hints() {
    let path = test_data_path().join("TaggedLines.yarn");

    let result = Compiler::new().read_file(path).compile().unwrap();

    let mut dialogue = TestBase::new().with_compilation(result).dialogue;
    dialogue.set_line_hints_enabled(true).set_node("Start").unwrap();

    let mut line_hints_were_sent = false;

    #[cfg(feature = "bevy")]
    let events = dialogue.continue_with_world(&mut World::default());
    #[cfg(not(feature = "bevy"))]
    let events = dialogue.continue_();
    let events = events.unwrap_or_else(|e| panic!("Encountered error while running dialogue: {e}"));
    for event in events {
        if let DialogueEvent::LineHints(lines) = event {
            // When the Dialogue realises it's about to run the Start
            // node, it will tell us that it's about to run these two line IDs
            assert_eq!(lines.len(), 2);
            println!("{lines:?}");
            assert!(lines.contains(&"line:test1".into()));
            assert!(lines.contains(&"line:test2".into()));

            // Ensure that these asserts were actually called
            line_hints_were_sent = true;
        }
    }

    assert!(line_hints_were_sent);
}

#[test]
fn test_function_argument_type_inference() {
    let test_base = TestBase::new().extend_library(|library| {
        // Register some functions
        library
            .add_function("ConcatString", |a: &str, b: &str| format!("{a}{b}"))
            .add_function("AddInt", |a: i32, b: i32| a + b)
            .add_function("AddFloat", |a: f32, b: f32| a + b)
            .add_function("NegateBool", |a: bool| !a);
    });

    // Run some code to exercise these functions
    let source = "\
    <<declare $str = \"\">>
    <<declare $int = 0>>
    <<declare $float = 0.0>>
    <<declare $bool = false>>

    <<set $str = ConcatString(\"a\", \"b\")>>
    <<set $int = AddInt(1,2)>>
    <<set $float = AddFloat(1,2)>>
    <<set $bool = NegateBool(true)>>
    ";

    let result = Compiler::from_test_source(source)
        .extend_library(test_base.dialogue.library().clone())
        .compile()
        .unwrap();

    let storage = test_base
        .with_compilation(result)
        .run_standard_testcase()
        .variable_storage
        .clone_shallow();

    // The values should be of the right type and value
    let str_value: String = storage.get("$str").unwrap().into();
    assert_eq!("ab", &str_value);

    let int_value: i32 = storage.get("$int").unwrap().try_into().unwrap();
    assert_eq!(3, int_value);

    let float_value: f32 = storage.get("$float").unwrap().try_into().unwrap();
    assert_eq!(3.0, float_value);

    let bool_value: bool = storage.get("$bool").unwrap().try_into().unwrap();
    assert!(!bool_value);
}

#[test]
fn test_selecting_option_from_inside_option_callback() {
    let result = Compiler::from_test_source("-> option 1\n->option 2\nfinal line\n").compile().unwrap();

    let mut test_base = TestBase::new()
        .with_test_plan(
            TestPlan::new()
                .expect_option("option 1")
                .expect_option("option 2")
                .then_select(0)
                .expect_line("final line"),
        )
        .with_compilation(result);
    test_base.dialogue.set_node("Start").unwrap();

    #[cfg(feature = "bevy")]
    let mut world = World::default();

    while test_base.dialogue.can_continue() {
        #[cfg(feature = "bevy")]
        let events = test_base.dialogue.continue_with_world(&mut world);
        #[cfg(not(feature = "bevy"))]
        let events = test_base.dialogue.continue_();
        let events = events.unwrap_or_else(|e| panic!("Encountered error while running dialogue: {e}"));
        for event in events {
            match event {
                DialogueEvent::Line(line) => {
                    let test_plan = test_base.test_plan.as_mut().unwrap();
                    test_plan.next(&mut test_base.dialogue);

                    let expected_step = test_plan.next_expected_step;
                    let expected_value = test_plan.next_step_value.clone().unwrap();
                    assert_eq!(ExpectedStepType::Line, expected_step);
                    assert_eq!(StepValue::String(line.text), expected_value);
                }
                DialogueEvent::Options(options) => {
                    test_base.test_plan.as_mut().unwrap().next(&mut test_base.dialogue);
                    let actual_options: Vec<_> = options
                        .into_iter()
                        .map(|o| ProcessedOption {
                            line: o.line.text,
                            enabled: o.is_available,
                        })
                        .collect();
                    let test_plan = test_base.test_plan.as_ref().unwrap();
                    let next_expected_options = test_plan.next_expected_options.clone();
                    assert_eq!(next_expected_options, actual_options);

                    let expected_step = test_plan.next_expected_step;
                    assert_eq!(ExpectedStepType::Select, expected_step);
                    test_base.dialogue.set_selected_option(OptionId(0)).unwrap();
                }
                DialogueEvent::DialogueComplete => {
                    let test_plan = test_base.test_plan.as_mut().unwrap();
                    test_plan.next(&mut test_base.dialogue);
                    let expected_step = test_plan.next_expected_step;
                    assert_eq!(ExpectedStepType::Stop, expected_step);
                }
                DialogueEvent::Command(_) | DialogueEvent::NodeComplete(_) | DialogueEvent::NodeStart(_) | DialogueEvent::LineHints(_) => {}
            }
        }
    }
}

// ---------------------------------------------------------------------------
// TestDialogueStorageCanRetrieveValues
// ---------------------------------------------------------------------------

/// Adapted from `TestDialogueStorageCanRetrieveValues`.
///
/// After compiling a program with declared variables, the dialogue's variable
/// storage should contain the declared initial values.
#[test]
fn test_dialogue_storage_can_retrieve_values() {
    use yarnspinner::core::YarnValue;

    let source = "<<declare $numVar = 42>>\n<<declare $stringVar = \"hello\">>\n<<declare $boolVar = true>>";
    let result = Compiler::from_test_source(source).compile().unwrap();
    // compile().unwrap() already asserts no errors

    let test_base = TestBase::default().with_compilation(result);
    let storage = test_base.dialogue.variable_storage();

    let num = storage.get("$numVar").expect("$numVar should be in storage");
    assert!(
        matches!(num, YarnValue::Number(n) if (n - 42.0_f32).abs() < f32::EPSILON),
        "Expected $numVar == 42, got {num:?}"
    );

    let s = storage.get("$stringVar").expect("$stringVar should be in storage");
    assert_eq!(String::from(s), "hello");

    let b = storage.get("$boolVar").expect("$boolVar should be in storage");
    assert_eq!(bool::try_from(b).unwrap(), true);
}

// ---------------------------------------------------------------------------
// TestDumpingCode
// ---------------------------------------------------------------------------

/// # Why ignored
///
/// `GetByteCode` is now implemented as `Compilation::dump_program()`.
#[test]
fn test_dumping_code() {
    let path = test_data_path().join("Example.yarn");
    let result = Compiler::new().read_file(path).compile().unwrap();
    let byte_code = result.dump_program();
    assert!(!byte_code.is_empty());
    assert_ne!(byte_code, "<no program>");
}

// ---------------------------------------------------------------------------
// TestVariadicFunctions
// ---------------------------------------------------------------------------

/// Tests that variadic functions (those accepting a variable number of arguments
/// after fixed parameters) can be registered, compiled, and executed correctly.
///
/// This is a partial port of the C# `TestVariadicFunctions` test.  The Rust
/// type system does not support C#-style `params T[]` parameters natively, so
/// variadic functions must be registered explicitly via
/// [`Library::add_variadic_function`].  Variable-type inference in call sites
/// is not implemented (the C# version uses a constraint solver), so that part
/// of the test is omitted here.
#[test]
fn test_variadic_functions() {
    use core::any::TypeId;
    use yarnspinner::core::YarnValue;

    let mut test_base = TestBase::new();

    // fn variadic_add(params f32[]) -> f32  { args.sum() }
    test_base.dialogue.library_mut().add_variadic_function(
        "variadic_add",
        vec![],              // no fixed params
        TypeId::of::<f32>(), // variadic param type = Number
        TypeId::of::<f32>(), // return type = Number
        |args: Vec<YarnValue>| {
            // Use explicit fold to guarantee +0.0 for empty args (nightly f32::sum() can yield -0.0).
            let sum: f32 = args.iter().filter_map(|v| f32::try_from(v.clone()).ok()).fold(0.0_f32, |acc, x| acc + x);
            YarnValue::Number(sum)
        },
    );

    // fn variadic_string_add(s: String, params f32[]) -> String  { s + args.sum() }
    test_base.dialogue.library_mut().add_variadic_function(
        "variadic_string_add",
        vec![TypeId::of::<String>()], // one fixed param: String
        TypeId::of::<f32>(),          // variadic param type = Number
        TypeId::of::<String>(),       // return type = String
        |args: Vec<YarnValue>| {
            let s = String::try_from(args[0].clone()).unwrap_or_default();
            let sum: f32 = args[1..].iter().filter_map(|v| f32::try_from(v.clone()).ok()).sum();
            YarnValue::String(format!("{s}{}", sum as i32))
        },
    );

    // Script exercises 0-arg, 3-arg, and mixed-type calls.
    let source = "title: Start\n---\n\
        {variadic_add(1,2,3)}\n\
        {variadic_string_add(\"s\",1,2,3)}\n\
        {variadic_add()}\n\
        {variadic_string_add(\"s\")}\n\
        ===";

    let result = Compiler::new()
        .add_file(File {
            file_name: "input.yarn".to_owned(),
            source: source.to_owned(),
        })
        .extend_library(test_base.dialogue.library().clone())
        .compile()
        .unwrap();

    // Runtime output verification.
    let test_plan = TestPlan::new()
        .expect_line("6")
        .expect_line("s6")
        .expect_line("0")
        .expect_line("s0")
        .expect_stop();
    test_base.with_compilation(result).with_test_plan(test_plan).run_standard_testcase();
}

// ---------------------------------------------------------------------------
// TestVariadicFunctionsMustAllBeSameType
// ---------------------------------------------------------------------------

/// Tests that passing an argument of the wrong type to a variadic parameter
/// produces a compile-time type error.
#[test]
fn test_variadic_functions_must_all_be_same_type() {
    use core::any::TypeId;
    use yarnspinner::core::YarnValue;

    let mut test_base = TestBase::new();

    test_base.dialogue.library_mut().add_variadic_function(
        "variadic_add",
        vec![],
        TypeId::of::<f32>(),
        TypeId::of::<f32>(),
        |args: Vec<YarnValue>| {
            let sum: f32 = args.iter().filter_map(|v| f32::try_from(v.clone()).ok()).sum();
            YarnValue::Number(sum)
        },
    );

    test_base.dialogue.library_mut().add_variadic_function(
        "variadic_string_add",
        vec![TypeId::of::<String>()],
        TypeId::of::<f32>(),
        TypeId::of::<String>(),
        |args: Vec<YarnValue>| {
            let s = String::try_from(args[0].clone()).unwrap_or_default();
            let sum: f32 = args[1..].iter().filter_map(|v| f32::try_from(v.clone()).ok()).sum();
            YarnValue::String(format!("{s}{}", sum as i32))
        },
    );

    // Both calls mix Number and Boolean — each should produce a type error.
    let source = "title: Start\n---\n\
        {variadic_add(1,true,3)}\n\
        {variadic_string_add(\"s\",1,true,3)}\n\
        ===";

    let error = Compiler::new()
        .add_file(File {
            file_name: "input.yarn".to_owned(),
            source: source.to_owned(),
        })
        .extend_library(test_base.dialogue.library().clone())
        .compile()
        .unwrap_err();

    let error_diagnostics: Vec<_> = error.0.iter().filter(|d| matches!(d.severity, DiagnosticSeverity::Error)).collect();
    assert_eq!(
        2,
        error_diagnostics.len(),
        "Expected exactly 2 error diagnostics, got: {error_diagnostics:#?}"
    );

    // Each error should mention that a Bool is not convertible to Number.
    for diag in &error_diagnostics {
        assert!(
            diag.message.contains("Bool") || diag.message.contains("bool") || diag.message.contains("true"),
            "Error should mention the mismatched Bool argument: {}",
            diag.message
        );
    }
}
