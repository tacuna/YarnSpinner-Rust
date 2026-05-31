//! Adapted from <https://github.com/YarnSpinnerTool/YarnSpinner/blob/3a5b7343f715e4e9a3705fa4224e7fa510b92f1c/YarnSpinner/Analyser.cs>

pub use self::context::*;
pub(crate) use self::default_analysers::*;
pub use self::diagnosis::*;
use crate::prelude::*;
use core::fmt::Debug;

mod context;
pub(crate) mod default_analysers;
mod diagnosis;

/// A trait for analysing a compiled Yarn program. Can be used by adding them to a [`Context`] with [`Context::add_analyser`] and then applied to a
/// compiled Yarn program with [`Dialogue::analyse`](crate::prelude::Dialogue).
pub trait CompiledProgramAnalyser: Debug {
    /// Reads data from the provided program that is later used in [`CompiledProgramAnalyser::collect_diagnoses`].
    fn diagnose(&mut self, program: &Program);

    /// Takes the data collected by [`CompiledProgramAnalyser::diagnose`], analyzes it and returns the resulting [`Diagnosis`] instances.
    ///
    /// ## Implementation note
    /// Corresponds to the original `GatherDiagnoses`, but was renamed to `collect_diagnoses` because that terminology is more idiomatic in Rust.
    fn collect_diagnoses(&self) -> Vec<Diagnosis>;
}
