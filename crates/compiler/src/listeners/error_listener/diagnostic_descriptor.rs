//! Stable diagnostic error codes for the Yarn Spinner compiler.
//!
//! Adapted from <https://github.com/YarnSpinnerTool/YarnSpinner/blob/v3.2.0/YarnSpinner.Compiler/DiagnosticDescriptor.cs>

use crate::listeners::{Diagnostic, DiagnosticSeverity};
use std::ops::Range;
use yarnspinner_core::prelude::Position;

/// Describes a class of compiler diagnostic, providing a stable error code,
/// a human-readable message template, and a default severity.
///
/// Use the predefined static instances (e.g. [`DiagnosticDescriptor::SYNTAX_ERROR`]) to
/// create [`Diagnostic`] values with the appropriate code already set.
#[derive(Debug, Clone)]
pub struct DiagnosticDescriptor {
    /// The stable error code (e.g. `"YS0005"`).
    pub code: &'static str,
    /// A human-readable template for the diagnostic message.
    pub message_template: &'static str,
    /// The default severity of this diagnostic.
    pub default_severity: DiagnosticSeverity,
    /// A human-readable description of what the diagnostic means.
    pub description: &'static str,
}

impl DiagnosticDescriptor {
    const fn new(code: &'static str, message_template: &'static str, default_severity: DiagnosticSeverity, description: &'static str) -> Self {
        Self {
            code,
            message_template,
            default_severity,
            description,
        }
    }

    /// Creates a [`Diagnostic`] with this descriptor's code and severity,
    /// using the provided message and file name.
    #[allow(dead_code)]
    pub(crate) fn create(&self, file_name: impl Into<String>, message: impl Into<String>) -> Diagnostic {
        Diagnostic::from_message(message)
            .with_file_name(file_name)
            .with_severity(self.default_severity)
            .with_code(self.code)
    }

    /// Creates a [`Diagnostic`] with this descriptor's code and severity,
    /// a file name, and a source range.
    #[allow(dead_code)]
    pub(crate) fn create_with_range(&self, file_name: impl Into<String>, range: Range<Position>, message: impl Into<String>) -> Diagnostic {
        Diagnostic::from_message(message)
            .with_file_name(file_name)
            .with_severity(self.default_severity)
            .with_range(range)
            .with_code(self.code)
    }

    // -----------------------------------------------------------------------
    // Predefined descriptors — one per stable error code
    // -----------------------------------------------------------------------

    /// YS0001 — Implicit variable type conflict
    pub const IMPLICIT_VARIABLE_TYPE_CONFLICT: DiagnosticDescriptor = DiagnosticDescriptor::new(
        "YS0001",
        "Variable {0} cannot be inferred as type {1}: it's already inferred as {2}",
        DiagnosticSeverity::Error,
        "A variable's type was inferred from context in incompatible ways.",
    );

    /// YS0002 — Type mismatch
    pub const TYPE_MISMATCH: DiagnosticDescriptor = DiagnosticDescriptor::new(
        "YS0002",
        "{0}",
        DiagnosticSeverity::Error,
        "A value was used in an incompatible type context.",
    );

    /// YS0003 — Undefined variable
    pub const UNDEFINED_VARIABLE: DiagnosticDescriptor = DiagnosticDescriptor::new(
        "YS0003",
        "{0} is used but never declared",
        DiagnosticSeverity::Warning,
        "A variable was used without a corresponding <<declare>> statement.",
    );

    /// YS0004 — Missing delimiter
    pub const MISSING_DELIMITER: DiagnosticDescriptor =
        DiagnosticDescriptor::new("YS0004", "{0}", DiagnosticSeverity::Error, "A required delimiter was missing.");

    /// YS0005 — Syntax error
    pub const SYNTAX_ERROR: DiagnosticDescriptor = DiagnosticDescriptor::new(
        "YS0005",
        "{0}",
        DiagnosticSeverity::Error,
        "A syntax error was encountered while parsing the script.",
    );

    /// YS0006 — Unclosed command
    pub const UNCLOSED_COMMAND: DiagnosticDescriptor = DiagnosticDescriptor::new(
        "YS0006",
        "Unclosed '{0}' command",
        DiagnosticSeverity::Error,
        "A command block was opened but never closed.",
    );

    /// YS0007 — Unclosed scope
    pub const UNCLOSED_SCOPE: DiagnosticDescriptor = DiagnosticDescriptor::new(
        "YS0007",
        "Unclosed scope",
        DiagnosticSeverity::Error,
        "An indented scope was opened but never closed.",
    );

    /// YS0008 — Unreachable code
    pub const UNREACHABLE_CODE: DiagnosticDescriptor = DiagnosticDescriptor::new(
        "YS0008",
        "Unreachable code",
        DiagnosticSeverity::Warning,
        "Code after a jump or return statement can never be reached.",
    );

    /// YS0009 — Unreferenced node
    pub const UNREFERENCED_NODE: DiagnosticDescriptor = DiagnosticDescriptor::new(
        "YS0009",
        "Node '{0}' is never referenced",
        DiagnosticSeverity::Warning,
        "A node is defined but never referenced by any other node.",
    );

    /// YS0010 — Unused variable
    pub const UNUSED_VARIABLE: DiagnosticDescriptor = DiagnosticDescriptor::new(
        "YS0010",
        "{0} is declared but never used",
        DiagnosticSeverity::Warning,
        "A variable was declared but never read or written.",
    );

    /// YS0011 — Duplicate node title
    pub const DUPLICATE_NODE_TITLE: DiagnosticDescriptor = DiagnosticDescriptor::new(
        "YS0011",
        "More than one node is named {0}",
        DiagnosticSeverity::Error,
        "Two or more nodes share the same title.",
    );

    /// YS0012 — Undefined node
    pub const UNDEFINED_NODE: DiagnosticDescriptor = DiagnosticDescriptor::new(
        "YS0012",
        "Jump to undefined node: '{0}'",
        DiagnosticSeverity::Error,
        "A <<jump>> or option points to a node that does not exist.",
    );

    /// YS0013 — Invalid function call
    pub const INVALID_FUNCTION_CALL: DiagnosticDescriptor =
        DiagnosticDescriptor::new("YS0013", "{0}", DiagnosticSeverity::Error, "A function call was invalid.");

    /// YS0014 — Invalid command
    pub const INVALID_COMMAND: DiagnosticDescriptor =
        DiagnosticDescriptor::new("YS0014", "{0}", DiagnosticSeverity::Error, "A command was used incorrectly.");

    /// YS0015 — Cyclic dependency
    pub const CYCLIC_DEPENDENCY: DiagnosticDescriptor = DiagnosticDescriptor::new(
        "YS0015",
        "Cyclic dependency detected",
        DiagnosticSeverity::Warning,
        "A cyclic dependency was detected between variables.",
    );

    /// YS0016 — Unknown character
    pub const UNKNOWN_CHARACTER: DiagnosticDescriptor = DiagnosticDescriptor::new(
        "YS0016",
        "Unknown character: '{0}'",
        DiagnosticSeverity::Warning,
        "An unrecognised character was encountered.",
    );

    /// YS0017 — Lines can't have both a line tag and a shadow tag
    pub const LINES_CANT_HAVE_LINE_AND_SHADOW_TAG: DiagnosticDescriptor = DiagnosticDescriptor::new(
        "YS0017",
        "Lines can't have both a line tag (#line:) and a shadow tag (#shadow:)",
        DiagnosticSeverity::Error,
        "A line has both a line ID tag and a shadow tag.",
    );

    /// YS0018 — Duplicate line ID
    pub const DUPLICATE_LINE_ID: DiagnosticDescriptor = DiagnosticDescriptor::new(
        "YS0018",
        "Duplicate line ID: '{0}'",
        DiagnosticSeverity::Error,
        "Two or more lines share the same line ID.",
    );

    /// YS0019 — Line content after command (warning)
    pub const LINE_CONTENT_AFTER_COMMAND: DiagnosticDescriptor = DiagnosticDescriptor::new(
        "YS0019",
        "Dialogue \"{0}\" content found following a command. Commands should be on their own line.",
        DiagnosticSeverity::Warning,
        "A line has text content after a command, which will be ignored.",
    );

    /// YS0020 — Command following a line of dialogue (error)
    pub const COMMAND_FOLLOWING_LINE: DiagnosticDescriptor = DiagnosticDescriptor::new(
        "YS0020",
        "Command \"{0}\" found following a line of dialogue. Commands should start on a new line.",
        DiagnosticSeverity::Error,
        "A line has text content before a command.",
    );

    /// YS0021 — Stray command end marker (warning)
    pub const STRAY_COMMAND_END: DiagnosticDescriptor = DiagnosticDescriptor::new(
        "YS0021",
        "Stray '>>' without matching '<<'. Did you forget to open the command?",
        DiagnosticSeverity::Warning,
        "A '>>' was found without a preceding '<<'.",
    );

    /// YS0022 — Unenclosed command keyword (warning)
    pub const UNENCLOSED_COMMAND: DiagnosticDescriptor = DiagnosticDescriptor::new(
        "YS0022",
        "'{0}' command must be enclosed in '<<' and '>>'. Did you mean '<<{0} ...'?",
        DiagnosticSeverity::Warning,
        "A command keyword was found outside of a command block.",
    );

    /// YS0027 — Invalid node name
    pub const INVALID_NODE_NAME: DiagnosticDescriptor = DiagnosticDescriptor::new(
        "YS0027",
        "Invalid node name: '{0}'",
        DiagnosticSeverity::Error,
        "A node title contains invalid characters.",
    );

    /// YS0028 — Type inference failure
    pub const TYPE_INFERENCE_FAILURE: DiagnosticDescriptor = DiagnosticDescriptor::new(
        "YS0028",
        "Type inference failed for {0}",
        DiagnosticSeverity::Error,
        "The type of an expression could not be determined.",
    );

    /// YS0029 — Expression type undetermined
    pub const EXPRESSION_TYPE_UNDETERMINED: DiagnosticDescriptor = DiagnosticDescriptor::new(
        "YS0029",
        "Cannot determine the type of expression {0}",
        DiagnosticSeverity::Error,
        "The type of an expression was left undetermined after type checking.",
    );

    /// YS0030 — Smart variable is read-only
    pub const SMART_VARIABLE_READ_ONLY: DiagnosticDescriptor = DiagnosticDescriptor::new(
        "YS0030",
        "Smart variable '{0}' is read-only",
        DiagnosticSeverity::Error,
        "An attempt was made to assign a value to a smart variable.",
    );

    /// YS0031 — Node group member missing `when` header
    pub const NODE_GROUP_MISSING_WHEN: DiagnosticDescriptor = DiagnosticDescriptor::new(
        "YS0031",
        "Node '{0}' is part of a node group but has no 'when' header",
        DiagnosticSeverity::Error,
        "A node group member is missing its saliency condition.",
    );

    /// YS0032 — Duplicate subtitle
    pub const DUPLICATE_SUBTITLE: DiagnosticDescriptor = DiagnosticDescriptor::new(
        "YS0032",
        "Duplicate subtitle '{0}' in node group '{1}'",
        DiagnosticSeverity::Error,
        "Two nodes in the same group share the same subtitle.",
    );

    /// YS0033 — Empty node (warning)
    pub const EMPTY_NODE: DiagnosticDescriptor = DiagnosticDescriptor::new(
        "YS0033",
        "Node \"{0}\" is empty and will not be included in the compiled output.",
        DiagnosticSeverity::Warning,
        "A node has no content and will be excluded from compilation.",
    );

    /// YS0034 — Invalid library function
    pub const INVALID_LIBRARY_FUNCTION: DiagnosticDescriptor = DiagnosticDescriptor::new(
        "YS0034",
        "{0}",
        DiagnosticSeverity::Error,
        "A function registered in the function library is invalid.",
    );

    /// YS0035 — Enum declaration error
    pub const ENUM_DECLARATION_ERROR: DiagnosticDescriptor =
        DiagnosticDescriptor::new("YS0035", "{0}", DiagnosticSeverity::Error, "An error occurred in an enum declaration.");

    /// YS0036 — Language version too low
    pub const LANGUAGE_VERSION_TOO_LOW: DiagnosticDescriptor = DiagnosticDescriptor::new(
        "YS0036",
        "This file requires Yarn Spinner language version {0} or higher.",
        DiagnosticSeverity::Error,
        "The file uses features not supported by the target language version.",
    );

    /// YS0037 — Invalid literal value
    pub const INVALID_LITERAL_VALUE: DiagnosticDescriptor = DiagnosticDescriptor::new(
        "YS0037",
        "Invalid literal value: '{0}'",
        DiagnosticSeverity::Error,
        "A literal value could not be parsed.",
    );

    /// YS0038 — Invalid member access
    pub const INVALID_MEMBER_ACCESS: DiagnosticDescriptor = DiagnosticDescriptor::new(
        "YS0038",
        "'{0}' does not have a member named '{1}'",
        DiagnosticSeverity::Error,
        "An attempt was made to access a member that doesn't exist.",
    );

    /// YS0039 — Redeclaration of existing variable
    pub const REDECLARATION_OF_EXISTING_VARIABLE: DiagnosticDescriptor = DiagnosticDescriptor::new(
        "YS0039",
        "Variable {0} has already been declared elsewhere",
        DiagnosticSeverity::Error,
        "A variable was declared more than once.",
    );

    /// YS0040 — Redeclaration of existing type
    pub const REDECLARATION_OF_EXISTING_TYPE: DiagnosticDescriptor = DiagnosticDescriptor::new(
        "YS0040",
        "Type {0} has already been declared elsewhere",
        DiagnosticSeverity::Error,
        "A type (e.g. an enum) was declared more than once.",
    );

    /// YS0041 — Internal error
    pub const INTERNAL_ERROR: DiagnosticDescriptor = DiagnosticDescriptor::new(
        "YS0041",
        "Internal error: {0}",
        DiagnosticSeverity::Error,
        "An unexpected internal compiler error occurred.",
    );

    /// YS0042 — Unknown line ID for shadow line
    pub const UNKNOWN_LINE_ID_FOR_SHADOW_LINE: DiagnosticDescriptor = DiagnosticDescriptor::new(
        "YS0042",
        "\"{0}\" is not a known line ID.",
        DiagnosticSeverity::Error,
        "A shadow line references an unknown line ID.",
    );

    /// YS0043 — Shadow lines can't have expressions
    pub const SHADOW_LINES_CANT_HAVE_EXPRESSIONS: DiagnosticDescriptor = DiagnosticDescriptor::new(
        "YS0043",
        "Shadow lines must not have expressions",
        DiagnosticSeverity::Error,
        "A shadow line contains an inline expression, which is not allowed.",
    );

    /// YS0044 — Shadow lines must have same text as source
    pub const SHADOW_LINES_MUST_HAVE_SAME_TEXT_AS_SOURCE: DiagnosticDescriptor = DiagnosticDescriptor::new(
        "YS0044",
        "Shadow lines must have the same text as their source lines",
        DiagnosticSeverity::Error,
        "A shadow line's text does not match the text of its source line.",
    );

    /// YS0045 — Smart variable loop
    pub const SMART_VARIABLE_LOOP: DiagnosticDescriptor = DiagnosticDescriptor::new(
        "YS0045",
        "Smart variable loop detected: {0}",
        DiagnosticSeverity::Error,
        "A cycle was detected in smart variable dependencies.",
    );

    /// YS0046 — Null default value
    pub const NULL_DEFAULT_VALUE: DiagnosticDescriptor = DiagnosticDescriptor::new(
        "YS0046",
        "Null is not a permitted type in Yarn Spinner 2.0 and later",
        DiagnosticSeverity::Error,
        "A variable or expression was assigned a null value.",
    );

    /// YS0047 — Type solver timeout
    pub const TYPE_SOLVER_TIMEOUT: DiagnosticDescriptor = DiagnosticDescriptor::new(
        "YS0047",
        "Type solver timed out while trying to determine the type of {0}",
        DiagnosticSeverity::Error,
        "The type solver exceeded its time limit.",
    );

    /// YS0048 — Dialogue wrapped in single angle brackets (warning)
    pub const SINGULAR_COMMAND_WRAP: DiagnosticDescriptor = DiagnosticDescriptor::new(
        "YS0048",
        "Line {0} has single '<' and '>' wrapping it. Did you mean to make this a command?",
        DiagnosticSeverity::Warning,
        "Dialogue begins and ends with single chevrons.",
    );

    /// YS0050 — Type checker error
    pub const TYPE_CHECKER_ERROR: DiagnosticDescriptor = DiagnosticDescriptor::new(
        "YS0050",
        "{0}",
        DiagnosticSeverity::Error,
        "An error occurred in resolving this expression's type.",
    );

    /// YS0051 — Node is missing its title
    pub const NODE_MISSING_TITLE: DiagnosticDescriptor = DiagnosticDescriptor::new(
        "YS0051",
        "Nodes must have a title",
        DiagnosticSeverity::Error,
        "A node is missing its title header.",
    );

    /// YS0052 — Node has more than one title
    pub const NODE_HAS_MORE_THAN_ONE_TITLE: DiagnosticDescriptor = DiagnosticDescriptor::new(
        "YS0052",
        "Nodes must have a single title header",
        DiagnosticSeverity::Error,
        "A node has more than one title.",
    );

    /// YS0053 — Variable's declared type doesn't match its initial value's type
    pub const DECLARATION_VALUE_DOESNT_MATCH_TYPE: DiagnosticDescriptor = DiagnosticDescriptor::new(
        "YS0053",
        "{0} is declared to be a {1}, but its initial value '{2}' is a {3}",
        DiagnosticSeverity::Error,
        "Variable's declared type doesn't match the type of its initial value.",
    );

    /// YS0060 — Unknown command
    pub const UNKNOWN_COMMAND: DiagnosticDescriptor = DiagnosticDescriptor::new(
        "YS0060",
        "Unknown command: {0}",
        DiagnosticSeverity::Error,
        "Command is not recognized or has invalid syntax.",
    );

    /// YS0061 — Command called with wrong number of parameters
    pub const WRONG_COMMAND_PARAMETER_COUNT: DiagnosticDescriptor = DiagnosticDescriptor::new(
        "YS0061",
        "Command {0} was called with {1} parameters, but expected {2}",
        DiagnosticSeverity::Error,
        "Command was called with the wrong number of parameters.",
    );

    /// YS0062 — Dialogue has multiple '#line' or '#shadow' IDs
    pub const MULTIPLE_LINE_OR_SHADOW_IDS_ON_A_LINE: DiagnosticDescriptor = DiagnosticDescriptor::new(
        "YS0062",
        "Dialogue has multiple '#line' or '#shadow' IDs.",
        DiagnosticSeverity::Error,
        "Lines of dialogue can only have a single line ID or shadow line.",
    );

    /// YS0063 — Dialogue has malformed or invalid markup
    pub const MARKUP_FAILED_TO_PARSE: DiagnosticDescriptor = DiagnosticDescriptor::new(
        "YS0063",
        "Dialogue has malformed or invalid markup. {0}",
        DiagnosticSeverity::Warning,
        "Dialogue has malformed or invalid markup.",
    );

    /// YS0064 — Extra chevrons on the same line as a command (warning)
    pub const ROGUE_CHEVRON_WITH_COMMAND: DiagnosticDescriptor = DiagnosticDescriptor::new(
        "YS0064",
        "Command \"<<{0}>>\" has extra chevrons on the same line as it.",
        DiagnosticSeverity::Warning,
        "Extra chevrons on the same line as a command.",
    );
}
