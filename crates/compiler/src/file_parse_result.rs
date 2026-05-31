//! Adapted from <https://github.com/YarnSpinnerTool/YarnSpinner/blob/3a5b7343f715e4e9a3705fa4224e7fa510b92f1c/YarnSpinner.Compiler/FileParseResult.cs>

use crate::parser::enum_registry::EnumRegistry;
use crate::prelude::generated::yarnspinnerparser::*;
use crate::prelude::*;
use std::collections::HashSet;
use std::rc::Rc;

/// Metadata about v3.0 line groups detected during lexing.
/// Used by the code generation visitor to emit saliency instructions
/// instead of option instructions for `=>` line group items.
#[derive(Clone, Default, Debug)]
pub(crate) struct LineGroupMetadata {
    /// Character start positions of SHORTCUT_ARROW tokens that were
    /// converted from `=>` (line group arrows) during lexing.
    pub(crate) converted_arrows: HashSet<isize>,
    /// Character start positions of converted arrows that are inside
    /// `<<once>>` blocks.
    pub(crate) once_arrows: HashSet<isize>,
    /// Character start positions of COMMAND_START tokens for inline
    /// `<<once>>` conditions (bare once, no expression). The lexer
    /// transforms these to `<<if true>>` and records the position here
    /// so that code generation can emit NOT($once_var) instead.
    pub(crate) once_lines: HashSet<isize>,
    /// Character start positions of COMMAND_START tokens for inline
    /// `<<once if expr>>` conditions. The lexer transforms these to
    /// `<<if expr>>` (dropping "once") and records the position here
    /// so that code generation can prepend NOT($once_var) AND.
    pub(crate) once_if_lines: HashSet<isize>,
}

/// Contains the result of parsing a single file of source code.
///
/// This struct provides only syntactic information about a parse — that is,
/// access to the parse tree and the token stream used to produce it.
///
/// # Why this is `pub(crate)` and not part of [`Compilation`]
///
/// In C#, `FileParseResult` holds `IParseTree` / `CommonTokenStream` as GC-managed
/// objects that survive indefinitely, allowing `CompilationResult.ParseResults` to
/// carry them back to callers for reuse in a subsequent compilation job.
///
/// In Rust this is structurally impossible without unsafe code: the `'input` lifetime
/// is tied to the UTF-32 input buffer (`&'input [u32]`) that is a local temporary inside
/// `compile()`. Making the buffer `Arc<Vec<u32>>` and threading that ownership through
/// the ANTLR parser and all generated node types would be a significant refactor.
/// Since the primary beneficiary of that feature is language-server tooling (avoiding
/// re-lex/re-parse of unchanged files), and not game-runtime use, it is deferred.
#[derive(Clone)]
pub(crate) struct FileParseResult<'input> {
    pub name: String,

    pub tree: Rc<DialogueContextAll<'input>>,

    /// This was not in the original, but in Rust we need to actually store
    /// the parser itself somewhere, which is why we store it here.
    /// We also end up leading the `ErrorStrategy` into the public interface, but using generics here makes
    /// the code a lot more complicated without actually providing much benefit.
    pub parser: Rc<ActualYarnSpinnerParser<'input>>,

    /// Metadata about v3.0 line groups detected during lexing.
    pub line_group_metadata: LineGroupMetadata,

    /// A snapshot of the enum registry that was built for this file during
    /// lexing. Used after code generation to populate
    /// [`Compilation::user_defined_types`].
    pub enum_registry: EnumRegistry,
}

impl<'input> FileParseResult<'input> {
    pub(crate) fn new(
        name: String,
        tree: Rc<DialogueContextAll<'input>>,
        parser: Rc<ActualYarnSpinnerParser<'input>>,
        line_group_metadata: LineGroupMetadata,
        enum_registry: EnumRegistry,
    ) -> Self {
        Self {
            name,
            tree,
            parser,
            line_group_metadata,
            enum_registry,
        }
    }

    pub(crate) fn tokens(&self) -> &ActualTokenStream<'input> {
        &self.parser.input
    }
}
