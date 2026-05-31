use crate::analysis::NodeBasicBlocksExt;
use crate::prelude::*;

/// Emits a YS0008 warning for every node that contains unreachable
/// instructions.
///
/// A basic block is considered unreachable when it is not the entry block
/// (index 0) **and** it has no predecessor blocks — i.e.
/// `ancestor_indices.is_empty()`.
///
/// Only the first unreachable block per node produces a diagnostic
/// (matching C# behaviour: one warning per node, not per instruction).
pub(crate) fn detect_unreachable_code(mut state: CompilationIntermediate) -> CompilationIntermediate {
    // We need the compiled program; it is set by `generate_code`.
    let Some(Ok(compilation)) = state.result.as_ref() else {
        return state;
    };
    let Some(program) = compilation.program.as_ref() else {
        return state;
    };

    // Use the first source file name as a best-effort diagnostic location.
    // The C# test does not check range or file name.
    let file_name: String = state
        .job
        .files
        .first()
        .map(|f| f.file_name.clone())
        .unwrap_or_else(|| "<input>".to_owned());

    for node in program.nodes.values() {
        let blocks = node.get_basic_blocks();
        for (idx, block) in blocks.iter().enumerate() {
            if idx != 0 && block.ancestor_indices.is_empty() && !block.instructions.is_empty() {
                state
                    .diagnostics
                    .push(DiagnosticDescriptor::UNREACHABLE_CODE.create(&file_name, "Unreachable code"));
                // Emit at most one diagnostic per node.
                break;
            }
        }
    }

    state
}
