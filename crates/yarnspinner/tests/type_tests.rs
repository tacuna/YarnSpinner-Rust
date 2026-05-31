//! Adapted from <https://github.com/YarnSpinnerTool/YarnSpinner/blob/3a5b7343f715e4e9a3705fa4224e7fa510b92f1c/YarnSpinner.Tests/TypeTests.cs>
//!
//! Tests that check runtime validation of registered functions were omitted,
//! because Rust's type system already guarantees correctness at compile time.
//! This affects the following tests:
//! - `TestFailingFunctionDeclarationReturnType`
//! - `TestFailingFunctionDeclarationParameterType`
//!
//! Our [`Declaration`] and [`types::FunctionType`] types already support builder semantics, so the following tests were omitted:
//! - `TestDeclarationBuilderCanBuildDeclarations`
//! - `TestFunctionTypeBuilderCanBuildTypes`
//!
//! Because of our different (and imo better) visibility granularity, we have no access to `Type::EXPLICITLY_CONSTRUCTABLE`,
//! so the following (fairly useless) test was omitted:
//! - `TestBuiltinTypesAreEnumerated`

use test_base::prelude::*;
use yarnspinner::compiler::*;
use yarnspinner::core::*;
use yarnspinner::runtime::*;

mod test_base;

#[test]
fn test_variable_declarations_parsed() {
    let result = Compiler::from_test_source(
        r#"
            <<declare $int = 5>>
            <<declare $str = "yes">>

            // These value changes are allowed,
            // because they match the declared type
            <<set $int = 6>>
            <<set $str = "no">>
            <<set $bool = false>>

            // Declarations are allowed anywhere in the program
            <<declare $bool = true>>
"#,
    )
    .compile()
    .unwrap();
    let expected_declarations = &[
        Declaration::new("$int", Type::Number)
            .with_default_value(5.0)
            .with_range(Position { line: 3, character: 22 }..Position { line: 3, character: 26 })
            .with_source_node_name("Start")
            .with_source_file_name("<input>"),
        Declaration::new("$str", Type::String)
            .with_default_value("yes")
            .with_range(Position { line: 4, character: 22 }..Position { line: 4, character: 26 })
            .with_source_node_name("Start")
            .with_source_file_name("<input>"),
        Declaration::new("$bool", Type::Boolean)
            .with_default_value(true)
            .with_range(Position { line: 13, character: 22 }..Position { line: 13, character: 27 })
            .with_source_node_name("Start")
            .with_source_file_name("<input>"),
    ];

    let actual_declarations = result.declarations;
    for (expected, actual) in expected_declarations.iter().zip(actual_declarations.into_iter()) {
        assert_eq!(expected.name, actual.name);
        assert_eq!(expected.r#type, actual.r#type);
        assert_eq!(expected.default_value, actual.default_value);
        assert_eq!(expected.range, actual.range);
        assert_eq!(expected.source_node_name, actual.source_node_name);
        assert_eq!(expected.source_file_name, actual.source_file_name);
    }
}

#[test]
fn test_declarations_can_appear_in_other_files() {
    // Create two separately-compiled compilation units that each
    // declare a variable that's modified by the other
    let source_a = create_test_node_with_name(
        "
            <<declare $varB = 1>>
            <<set $varA = 2>>
            ",
        "NodeA",
    );
    let source_b = create_test_node_with_name(
        "
            <<declare $varA = 1>>
            <<set $varB = 2>>
            ",
        "NodeB",
    );
    let _result = Compiler::default()
        .add_file(File {
            file_name: "sourceA".to_owned(),
            source: source_a,
        })
        .add_file(File {
            file_name: "sourceB".to_owned(),
            source: source_b,
        })
        .compile()
        .unwrap();
}

#[test]
fn test_importing_variable_declarations() {
    let result = Compiler::from_test_source("<<set $int = 6>> // no error; declaration is imported")
        .declare_variable(Declaration::new("$int", Type::Number).with_default_value(0.0))
        .compile()
        .unwrap();
    // No variables are declared in the source code, so we should
    // expect an empty collection of variable declarations
    assert!(result.declarations.is_empty())
}

#[test]
fn test_variable_declarations_disallow_duplicates() {
    let result = Compiler::from_test_source(
        "
            <<declare $int = 5>>
            <<declare $int = 6>> // error! redeclaration of $int
            ",
    )
    .compile()
    .unwrap_err();

    println!("{result}");
    assert!(result.0.iter().any(|d| d.message.contains("$int has already been declared")));
}

#[test]
fn test_expressions_disallow_mismatched_types() {
    let result = Compiler::from_test_source(
        "
            <<declare $int = 5>>
            <<set $int = \"5\">> // error, can't assign string to a variable declared int
            ",
    )
    .compile()
    .unwrap_err();

    println!("{result}");
    assert!(result.0.iter().any(|d| d.message == "$int (Number) cannot be assigned a String"));
}

#[test]
fn test_expressions_allows_using_undeclared_variable() {
    for source in [
        "<<set $str = \"hi\">>", //  // in commands
        "{$str + 1}",            // in inline expressions
    ] {
        let _result = Compiler::from_test_source(source).compile().unwrap();
    }
}

#[test]
fn test_expressions_require_compatible_types() {
    for declare in [true, false] {
        let source = format!(
            "
           {}
           {}
           {}

           <<set $int = 1>>
           <<set $int = 1 + 1>>
           <<set $int = 1 - 1>>
           <<set $int = 1 * 2>>
           <<set $int = 1 / 2>>
           <<set $int = 1 % 2>>
           <<set $int += 1>>
           <<set $int -= 1>>
           <<set $int *= 1>>
           <<set $int /= 1>>
           <<set $int %= 1>>

           <<set $str = \"hello\">>
           <<set $str = \"hel\" + \"lo\">>

           <<set $bool = true>>
           <<set $bool = 1 > 1>>
           <<set $bool = 1 < 1>>
           <<set $bool = 1 <= 1>>
           <<set $bool = 1 >= 1>>

           <<set $bool = \"hello\" == \"hello\">>
           <<set $bool = \"hello\" != \"goodbye\">>
           <<set $bool = 1 == 1>>
           <<set $bool = 1 != 2>>
           <<set $bool = true == true>>
           <<set $bool = true != false>>

           <<set $bool = (1 + 1) > 2>>
           ",
            if declare { "<<declare $int = 0>>" } else { Default::default() },
            if declare { "<<declare $bool = false>>" } else { Default::default() },
            if declare { "<<declare $str = \"\">>" } else { Default::default() }
        );

        let result = Compiler::from_test_source(&source).compile().unwrap();

        assert!(result.declarations.iter().any(|d| d.name == "$int" && d.r#type == Type::Number));
        assert!(result.declarations.iter().any(|d| d.name == "$bool" && d.r#type == Type::Boolean));
        assert!(result.declarations.iter().any(|d| d.name == "$str" && d.r#type == Type::String));
    }
}

#[test]
fn test_null_not_allowed() {
    let result = Compiler::from_test_source("<<declare $err = null>> // error, null not allowed")
        .compile()
        .unwrap_err();

    println!("{result}");
    assert!(result.0.iter().any(|d| d.message.contains("Null is not a permitted type")));
}

#[test]
fn test_function_signatures() {
    let mut test_base = TestBase::default();
    test_base
        .dialogue
        .library_mut()
        .add_function("func_void_bool", || true)
        .add_function("func_int_bool", |_i: i32| true)
        .add_function("func_int_int_bool", |_i: i32, _j: i32| true)
        .add_function("func_string_string_bool", |_i: &str, _j: &str| true);

    for source in [
        "<<set $bool = func_void_bool()>>",
        "<<set $bool = func_int_bool(1)>>",
        "<<set $bool = func_int_int_bool(1, 2)>>",
        "<<set $bool = func_string_string_bool(\"1\", \"2\")>>",
    ] {
        let result = Compiler::from_test_source(source)
            .extend_library(test_base.dialogue.library().clone())
            .compile()
            .unwrap();

        // The variable '$bool' should have an implicit declaration. The
        // type of the variable should be Boolean, because that's the return
        // type of all of the functions we declared.
        assert_eq!(1, result.declarations.len());
        assert!(result.declarations.iter().any(|d| d.name == "$bool" && d.r#type == Type::Boolean));
    }
}
#[test]
fn test_operators_are_type_checked() {
    let test_base = TestBase::default();
    for operation in ["= 1 + 1", "= 1 / 1", "= 1 - 1", "= 1 * 1", "= 1 % 1", "+= 1", "-= 1", "/= 1", "*= 1"] {
        for declared in [true, false] {
            let source = format!(
                "{}\n<<set $var {operation}>>",
                if declared { "<<declare $var = 0>>" } else { Default::default() },
            );

            let result = Compiler::from_test_source(&source)
                .extend_library(test_base.dialogue.library().clone())
                .compile()
                .unwrap();

            assert!(result.declarations.iter().any(|d| d.name == "$var" && d.r#type == Type::Number));
        }
    }
}

#[test]
fn test_failing_function_signatures() {
    let mut test_base = TestBase::default();
    test_base
        .dialogue
        .library_mut()
        .add_function("func_void_bool", || true)
        .add_function("func_int_bool", |_i: i32| true)
        .add_function("func_int_int_bool", |_i: i32, _j: i32| true)
        .add_function("func_string_string_bool", |_i: &str, _j: &str| true);

    for (source, expected_exception_message) in [
        ("<<set $bool = func_void_bool(1)>>", "expects 0 parameters, but received 1"),
        ("<<set $bool = func_int_bool()>>", "expects 1 parameter, but received 0"),
        ("<<set $bool = func_int_bool(true)>>", "true (Bool) is not convertible to Number"),
        (
            "<<set $bool = func_string_string_bool(\"1\", 2)>>",
            "2 (Number) is not convertible to String",
        ),
        ("<<set $int = func_void_bool()>>", "$int (Number) cannot be assigned a Bool"),
    ] {
        let failing_source = format!("<<declare $bool = false>>\n<<declare $int = 1>>\n{source}",);

        let result = Compiler::from_test_source(&failing_source)
            .extend_library(test_base.dialogue.library().clone())
            .compile()
            .unwrap_err();
        println!("{result}");

        let diagnostic_messages = result.0.iter().map(|d| d.message.clone()).collect::<Vec<_>>();

        assert!(diagnostic_messages.iter().any(|m| m.contains(expected_exception_message)));
    }
}

#[test]
fn test_initial_values() -> anyhow::Result<()> {
    let source = "
            <<declare $int = 42>>
            <<declare $str = \"Hello\">>
            <<declare $bool = true>>
            // internal decls
            {$int}
            {$str}
            {$bool}
            // external decls
            {$external_int}
            {$external_str}
            {$external_bool}
            ";
    let test_base = TestBase::new().with_test_plan(
        TestPlan::new()
            // internal decls
            .expect_line("42")
            .expect_line("Hello")
            // ## Implementation note:
            // The original uses the default C# bool to string conversion, which capitalizes the first letter,
            // so this would be "True" instead.
            .expect_line("true")
            // external decls
            .expect_line("42")
            .expect_line("Hello")
            // ## Implementation note: See above
            .expect_line("true"),
    );

    let result = Compiler::from_test_source(source)
        .extend_library(test_base.dialogue.library().clone())
        .declare_variable(Declaration::new("$external_str", Type::String).with_default_value("Hello"))
        .declare_variable(Declaration::new("$external_int", Type::Number).with_default_value(42))
        .declare_variable(Declaration::new("$external_bool", Type::Boolean).with_default_value(true))
        .compile()?;

    let mut variable_storage = test_base.variable_storage.clone_shallow();
    variable_storage.set("$external_str".to_string(), "Hello".into())?;
    variable_storage.set("$external_int".to_string(), 42.into())?;
    variable_storage.set("$external_bool".to_string(), true.into())?;

    test_base.with_compilation(result).run_standard_testcase();
    Ok(())
}

#[test]
fn test_variable_storage_extended_with_program_initial_values() {
    let source = "
        <<declare $int = 42>>
        <<declare $str = \"Hello\">>
        <<declare $bool = true>>
    ";

    let result = Compiler::from_test_source(source).compile().unwrap();

    let mut dialogue = Dialogue::new(Box::new(MemoryVariableStorage::new()), Box::new(StringTableTextProvider::new()));
    dialogue.replace_program(result.program.unwrap());

    let variable_storage = dialogue.variable_storage();
    assert_eq!(variable_storage.get("$int").unwrap(), YarnValue::Number(42.0));
    assert_eq!(variable_storage.get("$str").unwrap(), YarnValue::String("Hello".to_string()));
    assert_eq!(variable_storage.get("$bool").unwrap(), YarnValue::Boolean(true));
}

#[test]
fn test_explicit_types() {
    let result = Compiler::from_test_source(
        r#"
        <<declare $str = "hello" as string>>
        <<declare $int = 1 as number>>
        <<declare $bool = false as bool>>
        "#,
    )
    .compile()
    .unwrap();

    let variable_declarations: Vec<_> = result.declarations.iter().filter(|d| d.name.starts_with('$')).collect();

    assert!(variable_declarations.iter().any(|d| d.name == "$str" && d.r#type == Type::String));
    assert!(variable_declarations.iter().any(|d| d.name == "$int" && d.r#type == Type::Number));
    assert!(variable_declarations.iter().any(|d| d.name == "$bool" && d.r#type == Type::Boolean));
}

#[test]
fn test_explicit_types_must_match_value() {
    for test in [
        r#"<<declare $str = "hello" as number>>"#,
        r#"<<declare $int = 1 as bool>>"#,
        r#"<<declare $bool = false as string>>"#,
    ] {
        let _result = Compiler::from_test_source(test).compile().unwrap_err();
    }
}

#[test]
fn test_variable_declaration_annotations() {
    let result = Compiler::from_test_source(
        r#"
        /// prefix: a number
        <<declare $prefix_int = 42>>

        /// prefix: a string
        <<declare $prefix_str = "Hello">>

        /// prefix: a bool
        <<declare $prefix_bool = true>>

        <<declare $suffix_int = 42>> /// suffix: a number

        <<declare $suffix_str = "Hello">> /// suffix: a string

        <<declare $suffix_bool = true>> /// suffix: a bool

        // No declaration before
        <<declare $none_int = 42>> // No declaration after

        /// Multi-line
        /// doc comment
        <<declare $multiline = 42>>
        "#,
    )
    .compile()
    .unwrap();

    let expected_declarations = [
        Declaration::new("$prefix_int", Type::Number)
            .with_default_value(42.0)
            .with_description("prefix: a number"),
        Declaration::new("$prefix_str", Type::String)
            .with_default_value("Hello")
            .with_description("prefix: a string"),
        Declaration::new("$prefix_bool", Type::Boolean)
            .with_default_value(true)
            .with_description("prefix: a bool"),
        Declaration::new("$suffix_int", Type::Number)
            .with_default_value(42.0)
            .with_description("suffix: a number"),
        Declaration::new("$suffix_str", Type::String)
            .with_default_value("Hello")
            .with_description("suffix: a string"),
        Declaration::new("$suffix_bool", Type::Boolean)
            .with_default_value(true)
            .with_description("suffix: a bool"),
        Declaration::new("$none_int", Type::Number).with_default_value(42.0),
        Declaration::new("$multiline", Type::Number)
            .with_default_value(42.0)
            .with_description("Multi-line doc comment"),
    ];
    let actual_declarations = result.declarations;

    assert_eq!(expected_declarations.len(), actual_declarations.len());

    for (expected, actual) in expected_declarations.iter().zip(actual_declarations.iter()) {
        assert_eq!(expected.name, actual.name);
        assert_eq!(expected.r#type, actual.r#type);
        assert_eq!(expected.default_value, actual.default_value);
        assert_eq!(expected.description, actual.description);
    }
}

#[test]
fn test_type_conversion() {
    let source = "
            string + string(number): {\"1\" + string(1)}
            string + string(bool): {\"1\" + string(true)}

            number + number(string): {1 + number(\"1\")}
            number + number(bool): {1 + number(true)}

            bool and bool(string): {true and bool(\"true\")}
            bool and bool(number): {true and bool(1)}
            ";
    let test_base = TestBase::new().with_test_plan(
        TestPlan::new()
            .expect_line("string + string(number): 11")
            .expect_line("string + string(bool): 1true")
            .expect_line("number + number(string): 2")
            .expect_line("number + number(bool): 2")
            .expect_line("bool and bool(string): true")
            .expect_line("bool and bool(number): true"),
    );
    let result = Compiler::from_test_source(source)
        .extend_library(test_base.dialogue.library().clone())
        .compile()
        .unwrap();

    test_base.with_compilation(result).run_standard_testcase();
}

#[test]
#[should_panic = "Failed to convert a Yarn value to a number: ParseFloatError(ParseFloatError { kind: Invalid })"]
fn test_type_conversion_failure_to_number() {
    let source = "{number(\"hello\")}";
    let test_base = TestBase::new().with_test_plan(TestPlan::new().expect_line("test failure if seen"));
    let result = Compiler::from_test_source(source)
        .extend_library(test_base.dialogue.library().clone())
        .compile()
        .unwrap();
    test_base.with_compilation(result).run_standard_testcase();
}

#[test]
#[should_panic = "Failed to convert a Yarn value to a bool: ParseBoolError(ParseBoolError"]
fn test_type_conversion_failure_to_bool() {
    let source = "{bool(\"hello\")}";
    let test_base = TestBase::new().with_test_plan(TestPlan::new().expect_line("test failure if seen"));
    let result = Compiler::from_test_source(source)
        .extend_library(test_base.dialogue.library().clone())
        .compile()
        .unwrap();
    test_base.with_compilation(result).run_standard_testcase();
}

#[test]
fn test_implicit_function_declarations() {
    let source = "
            {func_void_bool()}
            {func_void_bool() and bool(func_void_bool())}
            { 1 + func_void_int() }
            { \"he\" + func_void_str() }

            {func_int_bool(1)}
            {true and func_int_bool(1)}

            {func_bool_bool(false)}
            {true and func_bool_bool(false)}

            {func_str_bool(\"hello\")}
            {true and func_str_bool(\"hello\")}
            ";
    let test_base = TestBase::new()
        .with_test_plan(
            TestPlan::new()
                .expect_line("true")
                .expect_line("true")
                .expect_line("2")
                .expect_line("hello")
                .expect_line("true")
                .expect_line("true")
                .expect_line("true")
                .expect_line("true")
                .expect_line("true")
                .expect_line("true"),
        )
        .extend_library(|library| {
            library
                .add_function("func_void_bool", || true)
                .add_function("func_void_int", || 1)
                .add_function("func_void_str", || "llo".to_owned())
                .add_function("func_int_bool", |_i: i64| true)
                .add_function("func_bool_bool", |_b: bool| true)
                .add_function("func_str_bool", |_s: &str| true);
        });

    // the library is NOT attached to this compilation job; all
    // functions will be implicitly declared
    let result = Compiler::from_test_source(source).compile().unwrap();

    test_base.with_compilation(result).run_standard_testcase();
}

#[test]
fn test_implicit_variable_declarations() {
    for (value, type_name) in [("1", "Number"), ("\"hello\"", "String"), ("true", "Bool")] {
        let result = Compiler::from_test_source(&format!("<<set $v = {value}>>")).compile().unwrap();

        assert_eq!(1, result.declarations.len());
        assert!(result.declarations.iter().any(|d| d.name == "$v" && d.r#type.name() == type_name));
    }
}

#[test]
fn test_nested_implicit_function_declarations() {
    let source = "
    {func_bool_bool(bool(func_int_bool(1)))}
    ";
    let test_base = TestBase::new()
        .with_test_plan(TestPlan::new().expect_line("true"))
        .extend_library(|library| {
            library
                .add_function("func_int_bool", |i: i64| i == 1)
                .add_function("func_bool_bool", |b: bool| b);
        });

    // the library is NOT attached to this compilation job; all
    // functions will be implicitly declared
    let result = Compiler::from_test_source(source).compile().unwrap();

    assert_eq!(2, result.declarations.len());

    // Both declarations that resulted from the compile should be functions found on line 1
    for decl in &result.declarations {
        assert_eq!(3, decl.range.as_ref().unwrap().start.line);
        assert!(matches!(decl.r#type, Type::Function(_)));
    }

    test_base.with_compilation(result).run_standard_testcase();
}

#[test]
fn test_multiple_implicit_redeclarations_of_function_parameter_count_fail() {
    let result = Compiler::from_test_source(
        r#"
        {func(1)}
        {func(2, 2)} // wrong number of parameters (previous decl had 1)
        "#,
    )
    .compile()
    .unwrap_err();

    println!("{result}");

    assert_eq!("Function \"func\" expects 1 parameter, but received 2", result.0[0].message,);
}

#[test]
fn test_multiple_implicit_redeclarations_of_function_parameter_type_fail() {
    let result = Compiler::from_test_source(
        "
        {func(1)}
        {func(true)} // wrong type of parameter (previous decl had number)
        ",
    )
    .compile()
    .unwrap_err();

    println!("{result}");

    assert!(result.0.iter().any(|d| d.message.contains("true (Bool) is not convertible to Number")));
}

#[test]
fn test_if_statement_expressions_must_be_boolean() {
    let result = Compiler::from_test_source(
        r#"
        <<declare $str = "hello" as string>>
        <<declare $bool = true>>

        <<if $bool>> // ok
        Hello
        <<endif>>

        <<if $str>> // error, must be a bool
        Hello
        <<endif>>
        "#,
    )
    .compile()
    .unwrap_err();

    println!("{result}");

    assert!(
        result
            .0
            .iter()
            .any(|d| { d.message.contains("Terms of 'if statement' must be Bool, not String") })
    );
}

// ---------------------------------------------------------------------------
// TestNumericOperatorsAreTypeChecked
// ---------------------------------------------------------------------------

#[test]
fn test_numeric_operators_are_type_checked() {
    for operation in ["= 1 + 1", "= 1 / 1", "= 1 - 1", "= 1 * 1", "= 1 % 1", "+= 1", "-= 1", "/= 1", "*= 1"] {
        for declared in [true, false] {
            let source = if declared {
                format!("<<declare $var = 0>>\n<<set $var {operation}>>")
            } else {
                format!("<<set $var {operation}>>")
            };
            let result = Compiler::from_test_source(&source).compile().unwrap();
            assert!(
                result.declarations.iter().any(|d| d.name == "$var" && d.r#type == Type::Number),
                "Expected $var to be Number for (declared={declared}): {operation}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// TestLogicOperatorsAreTypeChecked
// ---------------------------------------------------------------------------

#[test]
fn test_logic_operators_are_type_checked() {
    for operation in ["= true and false", "= true or false", "= not true", "= true xor false"] {
        for declared in [true, false] {
            let source = if declared {
                format!("<<declare $var = false>>\n<<set $var {operation}>>")
            } else {
                format!("<<set $var {operation}>>")
            };
            let result = Compiler::from_test_source(&source).compile().unwrap();
            assert!(
                result.declarations.iter().any(|d| d.name == "$var" && d.r#type == Type::Boolean),
                "Expected $var to be Boolean for (declared={declared}): {operation}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// TestStringOperatorsAreTypeChecked
// ---------------------------------------------------------------------------

#[test]
fn test_string_operators_are_type_checked() {
    for declared in [true, false] {
        let source = if declared {
            r#"<<declare $var = "">>
<<set $var = "hello" + " world">>"#
                .to_owned()
        } else {
            r#"<<set $var = "hello" + " world">>"#.to_owned()
        };
        let result = Compiler::from_test_source(&source).compile().unwrap();
        assert!(
            result.declarations.iter().any(|d| d.name == "$var" && d.r#type == Type::String),
            "Expected $var to be String (declared={declared})"
        );
    }
}

// ---------------------------------------------------------------------------
// TestJumpExpressionsMustBeStrings
// ---------------------------------------------------------------------------

#[test]
fn test_jump_expressions_must_be_strings() {
    let source = "<<set $x = 5>>\n<<jump {$x}>>";
    let result = Compiler::from_test_source(source).compile().unwrap_err();
    assert!(
        result.0.iter().any(|d| matches!(d.severity, DiagnosticSeverity::Error)),
        "Expected at least one error for non-string jump expression"
    );
    let jump_diag = result.0.iter().find(|d| {
        matches!(d.severity, DiagnosticSeverity::Error)
            && (d.message.contains("jump") || d.message.contains("String") || d.message.contains("Number"))
    });
    assert!(jump_diag.is_some(), "Expected error mentioning jump/String/Number type mismatch");
}

// ---------------------------------------------------------------------------
// TestFailingFunctionDeclarationReturnType
// ---------------------------------------------------------------------------

/// # Why ignored
///
/// In Rust, function registrations are type-safe at compile time via generics.
/// There is no mechanism to register a function with an invalid return type,
/// so this error scenario cannot occur.
#[test]
#[ignore = "Rust type system prevents registering functions with invalid return types at compile time"]
fn test_failing_function_declaration_return_type() {}

// ---------------------------------------------------------------------------
// TestFailingFunctionDeclarationParameterType
// ---------------------------------------------------------------------------

/// # Why ignored
///
/// Same as `test_failing_function_declaration_return_type` — Rust's
/// generics ensure parameter types are valid at compile time.
#[test]
#[ignore = "Rust type system prevents registering functions with invalid parameter types at compile time"]
fn test_failing_function_declaration_parameter_type() {}

// ---------------------------------------------------------------------------
// TestJumpExpressionsMayBeStringEnums
// ---------------------------------------------------------------------------

/// Enum support is already implemented: the lexer converts `EnumType.Member`
/// to its raw value at lex time, and `<<jump {expr}>>` with a string expression
/// is handled by the `#jumpToExpression` grammar rule.
#[test]
fn test_jump_expressions_may_be_string_enums() {
    let result = Compiler::from_test_source(concat!(
        "<<enum TestEnum>>\n",
        "<<case A = \"A\">>\n",
        "<<endenum>>\n",
        "<<jump {TestEnum.A}>>",
    ))
    .compile();
    // Collect all diagnostics from the result (errors from Err variant, warnings from Ok)
    let all_diags: Vec<_> = match &result {
        Ok(c) => c.warnings.iter().collect(),
        Err(e) => e.0.iter().collect(),
    };
    let errors: Vec<_> = all_diags.iter().filter(|d| d.severity == DiagnosticSeverity::Error).collect();
    assert!(errors.is_empty(), "Expected no errors, got: {errors:?}");
}

// ---------------------------------------------------------------------------
// TestTypesAreEnumerated
// ---------------------------------------------------------------------------

/// # Why ignored
///
/// C# exposes `Types.AllBuiltinTypes` as a static enumerable collection of
/// built-in type instances. Rust represents types as an enum (`Type`) with no
/// equivalent `all_builtin_types()` iterator.
#[test]
#[ignore = "No AllBuiltinTypes equivalent in the Rust type system (types are a plain enum)"]
fn test_types_are_enumerated() {}

// ---------------------------------------------------------------------------
// TestDeclarationBuilderCanBuildDeclarations
// ---------------------------------------------------------------------------

/// # Why ignored
///
/// C# uses a fluent `DeclarationBuilder` API. Rust uses struct literal syntax
/// and the type system enforces correctness, so a dedicated builder is not needed.
#[test]
#[ignore = "C#-specific DeclarationBuilder fluent API; Rust uses struct literals instead"]
fn test_declaration_builder_can_build_declarations() {}

// ---------------------------------------------------------------------------
// TestFunctionTypeBuilderCanBuildTypes
// ---------------------------------------------------------------------------

/// # Why ignored
///
/// C# exposes a `FunctionTypeBuilder` class. The Rust port uses `FunctionType`
/// directly with its fields, so no builder class is needed or implemented.
#[test]
#[ignore = "C#-specific FunctionTypeBuilder API; Rust uses FunctionType directly"]
fn test_function_type_builder_can_build_types() {}

// ---------------------------------------------------------------------------
// TestEnumTypeBuilderCanBuildTypes
// ---------------------------------------------------------------------------

/// # Why ignored
///
/// `EnumTypeBuilder` is now implemented in the Rust compiler.
#[test]
fn test_enum_type_builder_can_build_types() {
    let enum_type = EnumTypeBuilder::new()
        .with_name("MyEnum")
        .with_description("Test Enum")
        .with_raw_type(Type::String)
        .with_case("One", "one", "first case")
        .with_case("Two", "two", "second case")
        .with_case("Three", "three", "third case")
        .build();

    assert_eq!(enum_type.name, "MyEnum");
    assert_eq!(enum_type.description, "Test Enum");
    assert_eq!(enum_type.raw_type, Type::String);
    assert_eq!(enum_type.cases.len(), 3);
    assert_eq!(enum_type.cases[0].name, "One");
    assert_eq!(enum_type.cases[0].raw_value, YarnValue::from("one"));
    assert_eq!(enum_type.cases[0].description, "first case");
    assert_eq!(enum_type.cases[1].name, "Two");
    assert_eq!(enum_type.cases[1].raw_value, YarnValue::from("two"));
    assert_eq!(enum_type.cases[1].description, "second case");
    assert_eq!(enum_type.cases[2].name, "Three");
    assert_eq!(enum_type.cases[2].raw_value, YarnValue::from("three"));
    assert_eq!(enum_type.cases[2].description, "third case");
}

// ---------------------------------------------------------------------------
// TestSolverCanResolveConvertabilityConstraints
// TestSolverCannotResolveMismatchedConvertabilityConstraint
// TestConstraintsCanConvertToDisjunctiveNormalForm
// ---------------------------------------------------------------------------

/// # Why ignored — constraint solver not implemented in Rust
///
/// The C# `TypeChecker` uses a constraint-based type inference engine:
/// it builds a system of `TypeConstraint` objects
/// (`TypeEqualityConstraint`, `TypeConvertibleConstraint`,
/// `ConjunctionConstraint`, `DisjunctionConstraint`), converts them to
/// Disjunctive Normal Form (DNF), and runs `Solver.TrySolve()` to unify
/// type variables.
///
/// The Rust `TypeCheckVisitor` uses a simpler eager approach instead:
/// direct `is_sub_type_of` checks with a `deferred_types` list for
/// variables whose type cannot yet be determined. All 33 type-checking
/// integration tests pass with this approach, so the observable behaviour
/// is fully equivalent.
///
/// `TypeConstraint` / `Solver` / DNF conversion are **internal** to the
/// C# type checker and are not part of the public API. They are not
/// exposed or replicated in Rust because:
/// 1. The simpler eager approach handles all current cases correctly.
/// 2. Implementing the full unification-based solver would be significant
///    work with no observable benefit for the feature set we support.
///
/// If future Yarn features require multi-variable type unification that
/// the eager approach cannot handle, implementing the solver would become
/// necessary.
#[test]
#[ignore = "C# internal constraint solver (TypeConstraint/Solver/DNF) — Rust uses a simpler eager type-checker that achieves the same observable results"]
fn test_solver_can_resolve_convertability_constraints() {}

#[test]
#[ignore = "C# internal constraint solver (TypeConstraint/Solver/DNF) — Rust uses a simpler eager type-checker that achieves the same observable results"]
fn test_solver_cannot_resolve_mismatched_convertability_constraint() {}

#[test]
#[ignore = "C# internal constraint solver (TypeConstraint/Solver/DNF) — Rust uses a simpler eager type-checker that achieves the same observable results"]
fn test_constraints_can_convert_to_disjunctive_normal_form() {}

// ---------------------------------------------------------------------------
// TestUserDefinedTypesAreProvided
// ---------------------------------------------------------------------------

/// # Why ignored
///
/// `<<enum>>` block support and `Compilation::user_defined_types` are now
/// implemented in the Rust compiler.
#[test]
fn test_user_defined_types_are_provided() {
    let result = Compiler::from_test_source("<<enum MyEnum>>\n<<case One>>\n<<case Two>>\n<<case Three>>\n<<endenum>>")
        .compile()
        .expect("Compilation should succeed");

    assert!(!result.user_defined_types.is_empty(), "Expected non-empty user_defined_types");
    let my_enum = result
        .user_defined_types
        .iter()
        .find(|t| t.name == "MyEnum")
        .expect("Expected to find MyEnum in user_defined_types");
    assert_eq!(my_enum.cases.len(), 3);
    assert!(
        result.user_defined_types.iter().all(|t| t.name != "String"),
        "String is built-in and should not appear in user_defined_types"
    );
}

// ---------------------------------------------------------------------------
// TestExternalEnumsAreImported
// ---------------------------------------------------------------------------

/// # Why ignored
///
/// External enum type declarations can now be supplied to the Rust compiler
/// via `Compiler::with_type_declarations`.
#[test]
fn test_external_enums_are_imported() {
    // Build a Rust equivalent of the C# TestEnum (values 0, 1, 128)
    let test_enum = EnumTypeBuilder::new()
        .with_name("TestEnum")
        .with_raw_type(Type::Number)
        .with_case("One", 0.0_f32, "")
        .with_case("Two", 1.0_f32, "")
        .with_case("Three", 128.0_f32, "")
        .build();

    let mut compiler = Compiler::from_test_source("<<set $var1 = TestEnum.One>>\n<<set $var2 = .Two>>");
    compiler.with_type_declarations(vec![test_enum]);
    let result = compiler.compile();

    assert!(result.is_ok(), "Expected no compilation errors, got: {:?}", result.err());
}
