//! Adapted from <https://github.com/YarnSpinnerTool/YarnSpinner/blob/3a5b7343f715e4e9a3705fa4224e7fa510b92f1c/YarnSpinner/Analyser.cs>,
//! which was split into multiple files.

use crate::prelude::*;
use bevy_platform::collections::HashSet;

#[derive(Debug, Default)]
pub(crate) struct VariableLister {
    variables: HashSet<String>,
}

impl VariableLister {
    pub(crate) fn new() -> Self {
        Self::default()
    }
}

impl CompiledProgramAnalyser for VariableLister {
    fn diagnose(&mut self, program: &Program) {
        // In each node, find all reads and writes to variables
        let new_variables = program.nodes.values().flat_map(|node| {
            node.instructions
                .iter()
                .filter_map(|instruction| match instruction.opcode() {
                    OpCode::PushVariable | OpCode::StoreVariable => Some(instruction.operands[0].clone()),
                    _ => None,
                })
                .map(|operand| operand.try_into().unwrap())
        });
        self.variables.extend(new_variables);
    }

    fn collect_diagnoses(&self) -> Vec<Diagnosis> {
        self.variables
            .iter()
            .map(|variable| Diagnosis::new(DiagnosisSeverity::Note, format!("Script uses variable {variable}")))
            .collect()
    }
}
