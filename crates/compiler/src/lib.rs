//! The compiler components of Yarn Spinner. These mostly follow the same structure as the original Yarn Spinner compiler.
//!
//! You probably don't want to use this crate directly, except if you're coming from another language than Rust and want to call Yarn Spinner via FFI.
//! Otherwise:
//! - If you're a game developer, you'll want to use a crate that is already designed for your game engine of choice,
//!   such as [`bevy_yarnspinner`](https://crates.io/crates/bevy_yarnspinner) for the [Bevy engine](https://bevyengine.org/).
//! - If you wish to write an adapter crate for an engine yourself, use the [`yarnspinner`](https://crates.io/crates/yarnspinner) crate.
//!
#![warn(missing_docs, missing_debug_implementations)]

pub mod analysis;
mod collections;
pub(crate) mod compilation_steps;
pub(crate) mod compiler;
pub(crate) mod error_strategy;
mod file_parse_result;
pub(crate) mod listeners;
mod output;
mod parser;
pub(crate) mod parser_rule_context_ext;
#[cfg(feature = "serde")]
pub mod project;
pub mod project_version;
mod string_table_manager;
pub(crate) mod token_ext;
pub(crate) mod visitors;

pub use crate::compiler::Result;
pub use crate::compiler::descriptive_line_tag_generator::DescriptiveLineTagGenerator;
pub use crate::compiler::line_tag_generator::{LineTagContext, LineTagGenerator, LineTaggingError, RandomLineTagGenerator, TagAbortBehaviour};

pub mod prelude {
    //! Everything you need to get started with the Yarn Spinner compiler.
    pub use crate::analysis::{
        AnyNodeDestination,
        BasicBlock,
        BlockDestination,
        Condition,
        Destination,
        NodeBasicBlocksExt,
        NodeDestination,
        OptionDestination,
        ReturnToDestination,
        get_basic_blocks,
    };
    pub(crate) use crate::compiler::antlr_rust_ext::*;
    pub(crate) use crate::compiler::run_compilation::*;
    pub use crate::compiler::structured_command_parser::{StructuredCommandParseResult, parse_structured_command};
    pub use crate::compiler::utils::generate_yarn_file_with_declarations;
    pub(crate) use crate::compiler::utils::*;
    pub use crate::compiler::{CompilationType, Compiler, File};
    pub(crate) use crate::file_parse_result::*;
    pub use crate::listeners::{Diagnostic, DiagnosticDescriptor, DiagnosticSeverity, DiagnosticVec};
    pub use crate::output::*;
    pub(crate) use crate::parser::*;
    pub(crate) use crate::parser_rule_context_ext::*;
    #[cfg(feature = "serde")]
    pub use crate::project::*;
    pub use crate::project_version::*;
    pub(crate) use crate::string_table_manager::*;
    pub(crate) use crate::token_ext::*;
    pub(crate) use yarnspinner_core::prelude::*;
    pub(crate) use yarnspinner_internal_shared::prelude::*;
}
