//! Adapted from <https://github.com/YarnSpinnerTool/YarnSpinner/blob/3a5b7343f715e4e9a3705fa4224e7fa510b92f1c/YarnSpinner.Tests/TestPlan.cs>

use reader::*;
use std::fmt::Debug;
use std::str::FromStr;

mod reader;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Step {
    pub expected_step_type: ExpectedStepType,
    pub value: Option<StepValue>,
    pub expect_option_enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepValue {
    String(String),
    Number(usize),
    StringPair(String, String),
}

impl Step {
    /// Returns true if the line starts with a known step type prefix.
    pub(crate) fn has_known_prefix(line: &str) -> bool {
        let trimmed = line.trim();
        let prefixes = [
            "line:",
            "line ",
            "lind:",
            "lind ",
            "option:",
            "option ",
            "select:",
            "select ",
            "command:",
            "command ",
            "stop",
            "set:",
            "set ",
            "run:",
            "run ",
            "saliency:",
            "saliency ",
            "node:",
            "node ",
        ];
        prefixes.iter().any(|p| trimmed.starts_with(p))
    }

    pub(crate) fn read(string: &str) -> Self {
        let mut reader = Reader::new(string);
        let expected_step_type = reader.read_next::<ExpectedStepType>();

        // v3.0 testplan format: `stop` has no colon (no value follows)
        if expected_step_type == ExpectedStepType::Stop {
            return Self::with_expected_step_type(expected_step_type);
        }

        let delimiter: String = reader.read_next();
        assert_eq!(":", delimiter, "Expected ':' after step type");

        match expected_step_type {
            ExpectedStepType::Line | ExpectedStepType::Option | ExpectedStepType::Command => {
                let buf = reader.read_to_end();
                let value = buf.trim().to_owned();

                // Options whose text ends with " [disabled]"
                // are expected to be present, but have their
                // 'allowed' flag set to false.
                // Handle [disabled] BEFORE stripping backtick delimiters.
                if expected_step_type == ExpectedStepType::Option && value.ends_with("[disabled]") {
                    let stripped = value.replace(" [disabled]", "").replace("[disabled]", "");
                    let stripped = extract_backtick_value(&stripped);
                    Self {
                        expected_step_type,
                        value: Some(stripped.into()),
                        expect_option_enabled: false,
                    }
                } else {
                    // v3.0 testplans wrap line/option/command values in backticks
                    // as delimiters (e.g. `line: \`Success\``). Strip them.
                    // Hashtag metadata (e.g. `#line:source`) may follow the
                    // closing backtick and should be ignored.
                    let value = extract_backtick_value(&value);

                    if value == "*" {
                        Self::with_expected_step_type(expected_step_type)
                    } else {
                        Self::with_value_and_type(value, expected_step_type)
                    }
                }
            }
            ExpectedStepType::Select => {
                let value = reader.read_next::<usize>();
                Self::with_value_and_type(value, expected_step_type)
            }
            ExpectedStepType::Stop => Self::with_expected_step_type(expected_step_type),
            ExpectedStepType::Set => {
                let variable_name = reader.read_next::<String>();

                // v3.0 testplan format: `set: $var = value`
                // Skip the `=` separator if present.
                let next_token = reader.read_next::<String>();
                let value = if next_token == "=" { reader.read_next::<String>() } else { next_token };

                assert!(variable_name.starts_with("$"), "Variables must start with $");

                Self::with_value_and_type(StepValue::StringPair(variable_name, value), expected_step_type)
            }
            ExpectedStepType::Run => {
                let node_name = reader.read_next::<String>();
                Self::with_value_and_type(node_name, expected_step_type)
            }
            ExpectedStepType::Saliency | ExpectedStepType::Node => {
                // v3.0 directives: consume the value but treat as non-blocking no-op
                let value = reader.read_next::<String>();
                Self::with_value_and_type(value, expected_step_type)
            }
            ExpectedStepType::Restart => {
                // `---` already handled before Step::read is called
                Self::with_expected_step_type(expected_step_type)
            }
        }
    }

    pub(crate) fn from_line(line: impl Into<String>) -> Self {
        Self::with_value_and_type(line.into(), ExpectedStepType::Line)
    }

    pub(crate) fn from_option(line: impl Into<String>) -> Self {
        Self::with_value_and_type(line.into(), ExpectedStepType::Option)
    }

    pub(crate) fn from_command(line: impl Into<String>) -> Self {
        Self::with_value_and_type(line.into(), ExpectedStepType::Command)
    }

    pub(crate) fn from_select(selection: impl Into<usize>) -> Self {
        Self::with_value_and_type(selection.into(), ExpectedStepType::Select)
    }

    pub(crate) fn from_stop() -> Self {
        Self::with_expected_step_type(ExpectedStepType::Stop)
    }

    pub(crate) fn from_set(variable_name: impl Into<String>, value: impl Into<String>) -> Self {
        Self::with_value_and_type(StepValue::StringPair(variable_name.into(), value.into()), ExpectedStepType::Set)
    }

    pub(crate) fn from_run(node_name: impl Into<String>) -> Self {
        Self::with_value_and_type(node_name.into(), ExpectedStepType::Run)
    }

    fn with_value_and_type(value: impl Into<StepValue>, expected_step_type: ExpectedStepType) -> Self {
        Self {
            expected_step_type,
            value: Some(value.into()),
            expect_option_enabled: true,
        }
    }
    pub(crate) fn with_expected_step_type(expected_step_type: ExpectedStepType) -> Self {
        Self {
            expected_step_type,
            value: None,
            expect_option_enabled: true,
        }
    }
}

impl From<String> for StepValue {
    fn from(value: String) -> Self {
        Self::from(value.as_str())
    }
}

impl From<&str> for StepValue {
    fn from(value: &str) -> Self {
        Self::String(to_rust_serialization(value))
    }
}

impl From<usize> for StepValue {
    fn from(value: usize) -> Self {
        Self::Number(value)
    }
}

impl TryInto<String> for StepValue {
    type Error = ();

    fn try_into(self) -> Result<String, Self::Error> {
        match self {
            Self::String(value) => Ok(value),
            _ => Err(()),
        }
    }
}

impl TryInto<usize> for StepValue {
    type Error = ();

    fn try_into(self) -> Result<usize, Self::Error> {
        match self {
            Self::Number(value) => Ok(value),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Default)]
pub enum ExpectedStepType {
    // expecting to see this specific line
    #[default]
    Line,

    // expecting to see this specific option (if '*' is given,
    // means 'see an option, don't care about text')
    Option,

    // expecting options to have been presented; value = the
    // index to select
    Select,

    // expecting to see this specific command
    Command,

    // expecting to stop the test here (this is optional - a
    // 'stop' at the end of a test plan is assumed)
    Stop,

    /// sets a variable to a value
    Set,

    /// runs a new node.
    Run,

    /// sets the saliency mode (v3.0, currently ignored)
    Saliency,

    /// expects to be in a specific node
    Node,

    /// restart the dialogue from the start node (v3.0 `---` separator)
    Restart,
}

impl ExpectedStepType {
    pub(crate) fn is_blocking(&self) -> bool {
        !matches!(self, Self::Set | Self::Run | Self::Saliency | Self::Node | Self::Restart)
    }
}

impl FromStr for ExpectedStepType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "line" => Ok(Self::Line),
            "lind" => Ok(Self::Line), // typo in VisitTracking.testplan
            "option" => Ok(Self::Option),
            "select" => Ok(Self::Select),
            "command" => Ok(Self::Command),
            "stop" => Ok(Self::Stop),
            "set" => Ok(Self::Set),
            "run" => Ok(Self::Run),
            "saliency" => Ok(Self::Saliency),
            "node" => Ok(Self::Node),
            _ => Err(()),
        }
    }
}

fn to_rust_serialization(line: &str) -> String {
    // Need to do this because in Rust, booleans are not capitalized when converted to strings.
    // But in C# and hence our test plans, they are: https://stackoverflow.com/questions/491334/why-does-boolean-tostring-output-true-and-not-true
    line.replace("True", "true").replace("False", "false")
}

/// Extracts content from backtick-delimited values in v3.0 testplans.
/// If the value starts with a backtick, extracts text between the first
/// and second backtick, ignoring any hashtag metadata that follows.
/// Otherwise falls back to `trim_matches('`')`.
fn extract_backtick_value(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.starts_with('`') {
        // Find the closing backtick (second occurrence)
        if let Some(end) = trimmed[1..].find('`') {
            return trimmed[1..1 + end].to_owned();
        }
    }
    // Fallback: strip leading/trailing backticks
    trimmed.trim_matches('`').to_owned()
}
