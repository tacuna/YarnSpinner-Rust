//! Adapted from <https://github.com/YarnSpinnerTool/YarnSpinner/blob/3a5b7343f715e4e9a3705fa4224e7fa510b92f1c/YarnSpinner.Tests/SmartVariableTests.cs>
//!
//! ## Implementation notes
//!
//! The following tests require either `SmartVariableEvaluationVirtualMachine.TryGetSmartVariable`
//! or `VariableStorage.TryGetValue` evaluating smart variables directly, which are not in the
//! Rust public API. They are replaced with equivalent dialogue-output checks:
//! - `TestSmartVariablesCanBeEvaluatedExternally`  → `test_smart_variables_can_be_evaluated_externally`
//! - `TestCanEvaluateSmartVariable`                → `test_can_evaluate_smart_variable`
//! - `TestSmartVariablesCanCallFunctions`          → `test_smart_variables_can_call_functions`

use test_base::prelude::*;
use yarnspinner::compiler::*;
use yarnspinner::core::Type;

mod test_base;

/// Tag attached to synthetic nodes emitted for smart variables.
const SMART_VARIABLE_NODE_TAG: &str = "Yarn.SmartVariable";

#[test]
fn test_smart_variables_can_be_declared() {
    let result = Compiler::from_test_source("<<declare $smart_var = 1 + 1>>").compile().unwrap();

    // .unwrap() above already guarantees no errors.
    let smart_vars: Vec<_> = result.declarations.iter().filter(|d| d.is_inline_expansion).collect();
    assert_eq!(1, smart_vars.len());
    assert_eq!("$smart_var", smart_vars[0].name);
    assert_eq!(Type::Number, smart_vars[0].r#type);
}

#[test]
fn test_smart_variables_can_take_any_valid_expression_type() {
    let result = Compiler::from_test_source(
        r#"
        <<declare $smart_var_number = 1 + 1>>
        <<declare $smart_var_string = "hello" + " yes">>
        <<declare $smart_var_bool = true || false>>
        "#,
    )
    .compile()
    .unwrap();

    // .unwrap() above already guarantees no errors.
    let smart_vars: Vec<_> = result.declarations.iter().filter(|d| d.is_inline_expansion).collect();
    assert_eq!(3, smart_vars.len());

    assert!(smart_vars.iter().any(|d| d.name == "$smart_var_number" && d.r#type == Type::Number));
    assert!(smart_vars.iter().any(|d| d.name == "$smart_var_string" && d.r#type == Type::String));
    assert!(smart_vars.iter().any(|d| d.name == "$smart_var_bool" && d.r#type == Type::Boolean));
}

#[test]
fn test_smart_variables_can_reference_other_smart_variables() {
    let test_base = TestBase::new().with_test_plan(
        TestPlan::new()
            // The smart variable $smart_var_bool == ($smart_var_number == 2), and
            // $smart_var_number == 1+1 == 2, so we expect "true".
            // Note: Rust's default bool Display is lowercase "true".
            .expect_line("true")
            .expect_stop(),
    );

    let result = Compiler::from_test_source(
        r#"
        <<declare $smart_var_number = 1 + 1>>
        <<declare $smart_var_bool = $smart_var_number == 2>>
        {$smart_var_bool}
        "#,
    )
    .compile()
    .unwrap();

    test_base.with_compilation(result).run_standard_testcase();
}

#[test]
fn test_smart_variables_are_dynamic_content() {
    for smart_var_expression in ["1 + 1", "number(\"2\")", "$some_other_int"] {
        let source = format!(
            r#"
            <<declare $some_other_int = 2>>
            <<declare $smart_var = {smart_var_expression}>>
            "#
        );

        let result = Compiler::from_test_source(&source).compile().unwrap();

        // .unwrap() above already guarantees no errors.
        let smart_vars: Vec<_> = result.declarations.iter().filter(|d| d.is_inline_expansion).collect();
        assert!(
            smart_vars.iter().any(|d| d.name == "$smart_var"),
            "$smart_var should be a smart variable for expression `{smart_var_expression}`"
        );
    }
}

#[test]
fn test_smart_variables_can_be_evaluated_in_script() {
    let test_base = TestBase::new().with_test_plan(TestPlan::new().expect_line("2").expect_line("pass").expect_stop());

    let result = Compiler::from_test_source(
        r#"
        <<declare $smart_var = 1 + 1>>
        {$smart_var}
        <<if string($smart_var) == "2">>
        pass
        <<endif>>
        "#,
    )
    .compile()
    .unwrap();

    test_base.with_compilation(result).run_standard_testcase();
}

#[test]
fn test_smart_variables_compile_to_nodes() {
    let result = Compiler::from_test_source(
        r#"
        <<declare $smart_var_number = 1 + 1>>
        <<declare $smart_var_string = "hello" + " yes">>
        <<declare $smart_var_bool = true || false>>
        "#,
    )
    .compile()
    .unwrap();

    let program = result.program.unwrap();
    let smart_var_nodes: Vec<_> = program
        .nodes
        .values()
        .filter(|n| n.tags.iter().any(|t| t == SMART_VARIABLE_NODE_TAG))
        .collect();

    assert_eq!(3, smart_var_nodes.len());

    let names: Vec<&str> = smart_var_nodes.iter().map(|n| n.name.as_str()).collect();
    assert!(names.contains(&"$smart_var_number"));
    assert!(names.contains(&"$smart_var_string"));
    assert!(names.contains(&"$smart_var_bool"));
}

/// Equivalent of C#'s `TestSmartVariablesCanBeEvaluatedExternally`.
///
/// The C# test gets the smart variable value directly from `VariableStorage.TryGetValue`.
/// In the Rust port, smart variable evaluation happens inside the VM, so we test this
/// by running the dialogue and verifying the output.
#[test]
fn test_smart_variables_can_be_evaluated_externally() {
    let test_base = TestBase::new().with_test_plan(TestPlan::new().expect_line("2").expect_stop());

    let result = Compiler::from_test_source(
        r#"
        <<declare $smart_var = 1 + 1>>
        {$smart_var}
        "#,
    )
    .compile()
    .unwrap();

    test_base.with_compilation(result).run_standard_testcase();
}

#[test]
fn test_smart_variables_cannot_contain_dependency_loops() {
    // Create a dependency loop: $smart_var_1 → $smart_var_2 → $smart_var_3 → $smart_var_1.
    // Adding +1 forces the type checker to see all vars as Numbers instead of raising
    // a type error before the loop detection runs.
    let result = Compiler::from_test_source(
        r#"
        <<declare $smart_var_1 = $smart_var_2 + 1>>
        <<declare $smart_var_2 = $smart_var_3>>
        <<declare $smart_var_3 = $smart_var_1>>
        "#,
    )
    .compile()
    .unwrap_err();

    assert!(
        result.0.iter().any(|d| d.message.contains("$smart_var_1") && d.message.contains("loop")),
        "Expected a loop diagnostic mentioning $smart_var_1, got: {:?}",
        result.0.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
}

#[test]
fn test_smart_variables_can_be_chained() {
    let test_base = TestBase::new().with_test_plan(TestPlan::new().expect_line("2").expect_stop());

    let result = Compiler::from_test_source(
        r#"
        <<declare $smart_var_1 = $smart_var_2>>
        <<declare $smart_var_2 = $smart_var_3>>
        <<declare $smart_var_3 = 1 + 1>>
        {$smart_var_1}
        "#,
    )
    .compile()
    .unwrap();

    test_base.with_compilation(result).run_standard_testcase();
}

#[test]
fn test_smart_variables_cannot_be_written_to() {
    let result = Compiler::from_test_source(
        r#"
        <<declare $smart_var = 1 + 1>>
        <<set $smart_var to 3>>
        "#,
    )
    .compile()
    .unwrap_err();

    assert!(
        result
            .0
            .iter()
            .any(|d| d.message.contains("$smart_var") && d.message.contains("smart variable")),
        "Expected a read-only diagnostic for $smart_var, got: {:?}",
        result.0.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
}

/// Equivalent of C#'s `TestCanEvaluateSmartVariable`.
///
/// C# uses `SmartVariableEvaluationVirtualMachine.TryGetSmartVariable` directly.
/// In the Rust port, evaluation is done inside the dialogue VM.
#[test]
fn test_can_evaluate_smart_variable() {
    let test_base = TestBase::new().with_test_plan(TestPlan::new().expect_line("5").expect_stop());

    let result = Compiler::from_test_source(
        r#"
        <<declare $smart_var = 3 + 2>>
        {$smart_var}
        "#,
    )
    .compile()
    .unwrap();

    test_base.with_compilation(result).run_standard_testcase();
}

/// Equivalent of C#'s `TestSmartVariablesCanCallFunctions`.
///
/// C# gets the value directly from `VariableStorage.TryGetValue`.
/// In the Rust port, we verify the evaluated value via dialogue output.
#[test]
fn test_smart_variables_can_call_functions() {
    let mut test_base = TestBase::new().with_test_plan(TestPlan::new().expect_line("6").expect_stop());
    test_base
        .dialogue
        .library_mut()
        .add_function("add_three_operands", |a: f32, b: f32, c: f32| a + b + c);

    let result = Compiler::from_test_source(
        r#"
        <<declare $smart_var = add_three_operands(1,2,3)>>
        {$smart_var}
        "#,
    )
    .extend_library(test_base.dialogue.library().clone())
    .compile()
    .unwrap();

    let mut test_base = test_base.with_compilation(result);
    test_base.run_standard_testcase();
}

// ---------------------------------------------------------------------------
// TestSmartVariablesHaveDependencies
// ---------------------------------------------------------------------------

#[test]
fn test_smart_variables_have_dependencies() {
    let result = Compiler::from_test_source(
        "<<declare $smart_var_1 = $smart_var_2 or $stored_var_1>>\n\
         <<declare $smart_var_2 = $stored_var_2 > 5>>",
    )
    .compile()
    .unwrap();

    let decl = |name: &str| {
        result
            .declarations
            .iter()
            .find(|d| d.name == name)
            .unwrap_or_else(|| panic!("declaration '{name}' not found"))
    };

    let smart_var_1 = decl("$smart_var_1");
    let smart_var_2 = decl("$smart_var_2");
    let stored_var_1 = decl("$stored_var_1");
    let stored_var_2 = decl("$stored_var_2");

    assert!(smart_var_1.is_inline_expansion, "$smart_var_1 should be a smart variable");
    assert!(smart_var_2.is_inline_expansion, "$smart_var_2 should be a smart variable");
    assert!(!stored_var_1.is_inline_expansion, "$stored_var_1 should not be a smart variable");
    assert!(!stored_var_2.is_inline_expansion, "$stored_var_2 should not be a smart variable");

    // smart_var_1 depends on smart_var_2, stored_var_1, and (transitively) stored_var_2
    assert!(smart_var_1.dependents.is_empty(), "$smart_var_1 should have no dependents");
    assert!(
        smart_var_1.dependencies.contains(&"$smart_var_2".to_string()),
        "$smart_var_1 should depend on $smart_var_2"
    );
    assert!(
        smart_var_1.dependencies.contains(&"$stored_var_1".to_string()),
        "$smart_var_1 should depend on $stored_var_1"
    );
    assert!(
        smart_var_1.dependencies.contains(&"$stored_var_2".to_string()),
        "$smart_var_1 should transitively depend on $stored_var_2"
    );

    // smart_var_2 depends on stored_var_2; smart_var_1 depends on smart_var_2
    assert!(
        smart_var_2.dependents.contains(&"$smart_var_1".to_string()),
        "$smart_var_2 should be depended on by $smart_var_1"
    );
    assert!(
        smart_var_2.dependencies.contains(&"$stored_var_2".to_string()),
        "$smart_var_2 should depend on $stored_var_2"
    );

    // stored_var_1 is depended on only by smart_var_1
    assert!(stored_var_1.dependencies.is_empty(), "$stored_var_1 should have no dependencies");
    assert!(
        stored_var_1.dependents.contains(&"$smart_var_1".to_string()),
        "$stored_var_1 should be depended on by $smart_var_1"
    );

    // stored_var_2 is depended on by both smart vars
    assert!(stored_var_2.dependencies.is_empty(), "$stored_var_2 should have no dependencies");
    assert!(
        stored_var_2.dependents.contains(&"$smart_var_1".to_string()),
        "$stored_var_2 should be depended on by $smart_var_1"
    );
    assert!(
        stored_var_2.dependents.contains(&"$smart_var_2".to_string()),
        "$stored_var_2 should be depended on by $smart_var_2"
    );
}
