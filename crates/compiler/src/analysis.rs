//! Static analysis utilities for compiled Yarn programs.
//!
//! Adapted from
//! <https://github.com/YarnSpinnerTool/YarnSpinner/blob/3a5b7343f715e4e9a3705fa4224e7fa510b92f1c/YarnSpinner.Compiler/Analysis/BasicBlock.cs>
//! and the `GetBasicBlocks` extension method in `InstructionCollectionExtensions.cs`.

use crate::prelude::*;
use std::collections::{BTreeMap, BTreeSet, HashMap};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// A maximal straight-line sequence of instructions in a compiled Yarn node
/// — a *basic block*.  Control flow only enters a block at its first
/// instruction and leaves at the last.
///
/// The [`Vec<BasicBlock>`] returned by [`get_basic_blocks`] is ordered by
/// [`first_instruction_index`](BasicBlock::first_instruction_index); index 0
/// is always the entry block.
///
/// Adapted from the C# `BasicBlock` class in `YarnSpinner.Compiler`.
#[derive(Debug, Clone)]
pub struct BasicBlock {
    /// The label present at the start of this block, if any.
    ///
    /// When multiple labels share the same instruction index the
    /// lexicographically smallest one is stored.
    pub label_name: Option<String>,
    /// Index of the first instruction in this block within the containing
    /// node's instruction list.
    pub first_instruction_index: usize,
    /// Name of the [`Node`] this block belongs to.
    pub node_name: String,
    /// The instructions that make up this block.
    pub instructions: Vec<Instruction>,
    /// Possible successor destinations after this block executes.
    pub destinations: Vec<Destination>,
    /// Indices (into the same [`Vec<BasicBlock>`]) of blocks that have this
    /// block as a successor (i.e. predecessors).
    pub ancestor_indices: Vec<usize>,
}

impl core::fmt::Display for BasicBlock {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let end = self.first_instruction_index + self.instructions.len().saturating_sub(1);
        write!(f, "Block[{}..{}] in {}", self.first_instruction_index, end, self.node_name)
    }
}

/// A possible destination after a [`BasicBlock`] ends.
#[derive(Debug, Clone)]
pub enum Destination {
    /// Jumps to another block inside the same node.
    Block(BlockDestination),
    /// Jumps to (or detours to) a node whose name is known at compile time.
    Node(NodeDestination),
    /// Jumps to a node whose name is not known at compile time
    /// (e.g. the node name was pushed from a variable).
    AnyNode(AnyNodeDestination),
    /// Follows a player-visible dialogue option to another block.
    Option(OptionDestination),
}

/// Describes *why* control flow moves from one block to another.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Condition {
    /// Control falls through from the previous block (no explicit branch).
    Fallthrough,
    /// Unconditional jump.
    DirectJump,
    /// Taken when the condition expression evaluates to `true`.
    ExpressionIsTrue,
    /// Taken when the condition expression evaluates to `false`.
    ExpressionIsFalse,
    /// Taken when the player selects the associated dialogue option.
    Option,
}

/// An intra-node jump to a specific basic block.
#[derive(Debug, Clone)]
pub struct BlockDestination {
    /// Index of the target block in the enclosing [`Vec<BasicBlock>`].
    pub block_index: usize,
    /// Why this edge is taken.
    pub condition: Condition,
}

/// A jump (or `<<detour>>`) to a named Yarn node.
#[derive(Debug, Clone)]
pub struct NodeDestination {
    /// Name of the target node.
    pub node_name: String,
    /// Why this edge is taken.
    pub condition: Condition,
    /// For `<<detour>>` instructions: the block to resume at when the
    /// detoured node finishes.  `None` for plain `<<jump>>`.
    pub return_to: Option<ReturnToDestination>,
}

/// A jump to a node whose name is not known statically.
#[derive(Debug, Clone)]
pub struct AnyNodeDestination {
    /// For `<<detour>>` instructions: the block to resume at when the
    /// detoured node finishes.
    pub return_to: Option<ReturnToDestination>,
}

/// A player-visible option that, when selected, transfers control to a block.
#[derive(Debug, Clone)]
pub struct OptionDestination {
    /// The string-table ID for this option's display text.
    pub line_id: String,
    /// Index of the target block in the enclosing [`Vec<BasicBlock>`].
    pub block_index: usize,
}

/// Identifies the block that execution should resume at after a `<<detour>>`
/// finishes.
#[derive(Debug, Clone)]
pub struct ReturnToDestination {
    /// Name of the node that contains the return-to block.
    pub node_name: String,
    /// Index of the return-to block within that node's basic block list.
    pub block_index: usize,
}

// ---------------------------------------------------------------------------
// Extension trait
// ---------------------------------------------------------------------------

/// Adds basic-block analysis to a compiled [`Node`].
pub trait NodeBasicBlocksExt {
    /// Computes and returns the basic-block decomposition of this node.
    ///
    /// The returned [`Vec`] is ordered by
    /// [`BasicBlock::first_instruction_index`]; element 0 is always the
    /// entry block.  Returns an empty `Vec` if the node has no instructions.
    fn get_basic_blocks(&self) -> Vec<BasicBlock>;
}

impl NodeBasicBlocksExt for Node {
    fn get_basic_blocks(&self) -> Vec<BasicBlock> {
        get_basic_blocks(self)
    }
}

// ---------------------------------------------------------------------------
// Algorithm
// ---------------------------------------------------------------------------

/// Computes the basic-block decomposition of `node`.
///
/// See [`NodeBasicBlocksExt::get_basic_blocks`] for the trait-method form.
pub fn get_basic_blocks(node: &Node) -> Vec<BasicBlock> {
    if node.instructions.is_empty() {
        return Vec::new();
    }

    // Build a reverse label map: instruction index → label names
    let mut index_to_labels: HashMap<usize, Vec<String>> = HashMap::new();
    for (label, &idx) in &node.labels {
        index_to_labels.entry(idx as usize).or_default().push(label.clone());
    }

    // -----------------------------------------------------------------------
    // Step 1 – Identify "leader" instruction indices.
    //
    // A leader is the first instruction of a basic block:
    //   - Instruction 0 is always a leader.
    //   - The instruction *after* any terminator / branch is a leader.
    //   - Any instruction that is a label target is a leader.
    // -----------------------------------------------------------------------
    let mut leaders: BTreeSet<usize> = BTreeSet::new();
    leaders.insert(0);

    let n = node.instructions.len();
    for (i, instr) in node.instructions.iter().enumerate() {
        let Ok(opcode) = OpCode::try_from(instr.opcode) else {
            continue;
        };
        let next = i + 1;
        // These opcodes end a straight-line sequence; the very next
        // instruction (if any) begins a new block.
        let terminates_block = matches!(
            opcode,
            OpCode::JumpTo
                | OpCode::Jump
                | OpCode::JumpIfFalse
                | OpCode::Stop
                | OpCode::Return
                | OpCode::RunNode
                | OpCode::DetourToNode
                | OpCode::PeekAndJump
        );
        if terminates_block && next < n {
            leaders.insert(next);
        }
    }

    // Label targets are always leaders (jump destinations must start a block).
    for &idx in index_to_labels.keys() {
        if idx < n {
            leaders.insert(idx);
        }
    }

    // -----------------------------------------------------------------------
    // Step 2 – Slice the instruction list into blocks.
    // -----------------------------------------------------------------------
    let leader_list: Vec<usize> = leaders.into_iter().collect(); // sorted (BTreeSet)

    let mut blocks: Vec<BasicBlock> = Vec::with_capacity(leader_list.len());

    for (slot, &start) in leader_list.iter().enumerate() {
        let end = leader_list.get(slot + 1).copied().unwrap_or(n);
        let label_name = index_to_labels.get(&start).and_then(|labels| labels.iter().min().cloned());
        blocks.push(BasicBlock {
            label_name,
            first_instruction_index: start,
            node_name: node.name.clone(),
            instructions: node.instructions[start..end].to_vec(),
            destinations: Vec::new(),
            ancestor_indices: Vec::new(),
        });
    }

    // -----------------------------------------------------------------------
    // Step 3 – Build control-flow edges.
    // -----------------------------------------------------------------------

    // Collect edges in separate Vecs to avoid aliasing `blocks` mutably while
    // iterating it immutably.
    let mut dests_to_add: Vec<(usize, Destination)> = Vec::new();
    let mut ancestors_to_add: Vec<(usize /*dest_block*/, usize /*pred_block*/)> = Vec::new();

    for (block_idx, block) in blocks.iter().enumerate() {
        let mut current_string_at_tos: Option<String> = None;
        // Accumulates (line_id, dest_label) from AddOption / AddSaliencyCandidate
        // until a Jump (option dispatch) consumes them.
        let mut peek_jump_dests: Vec<(String, String)> = Vec::new();

        for (local_i, instr) in block.instructions.iter().enumerate() {
            let global_i = block.first_instruction_index + local_i;

            let Ok(opcode) = OpCode::try_from(instr.opcode) else {
                current_string_at_tos = None;
                continue;
            };

            match opcode {
                // --- String TOS tracking ---
                OpCode::PushString => {
                    current_string_at_tos = Some(instr.read_operand(0));
                }
                // These opcodes overwrite or consume the TOS, so the string
                // we were tracking is no longer valid.
                OpCode::CallFunc
                | OpCode::Pop
                | OpCode::PushBool
                | OpCode::PushFloat
                | OpCode::PushNull
                | OpCode::PushVariable
                | OpCode::SelectSaliencyCandidate => {
                    current_string_at_tos = None;
                }

                // --- Option / saliency accumulation ---
                OpCode::AddOption => {
                    // opA = string ID (line_id), opB = destination label
                    let line_id: String = instr.read_operand(0);
                    let dest_label: String = instr.read_operand(1);
                    peek_jump_dests.push((line_id, dest_label));
                }
                OpCode::AddSaliencyCandidate => {
                    // opA = content ID, opB = complexity, opC = dest label
                    let content_id: String = instr.read_operand(0);
                    let dest_label: String = instr.read_operand(2);
                    peek_jump_dests.push((content_id, dest_label));
                }

                // --- Option dispatch: consume accumulated peek-jump targets ---
                OpCode::Jump => {
                    // Peeks the selected option's label (a string) from TOS.
                    for (line_id, dest_label) in &peek_jump_dests {
                        if let Some(dest_block) = block_for_label(&leader_list, &node.labels, dest_label) {
                            dests_to_add.push((
                                block_idx,
                                Destination::Option(OptionDestination {
                                    line_id: line_id.clone(),
                                    block_index: dest_block,
                                }),
                            ));
                            ancestors_to_add.push((dest_block, block_idx));
                        }
                    }
                    peek_jump_dests.clear();
                }

                // --- Unconditional intra-node jump ---
                OpCode::JumpTo => {
                    let label: String = instr.read_operand(0);
                    if let Some(dest_block) = block_for_label(&leader_list, &node.labels, &label) {
                        dests_to_add.push((
                            block_idx,
                            Destination::Block(BlockDestination {
                                block_index: dest_block,
                                condition: Condition::DirectJump,
                            }),
                        ));
                        ancestors_to_add.push((dest_block, block_idx));
                    }
                }

                // --- Conditional jump (false branch jumps, true falls through) ---
                OpCode::JumpIfFalse => {
                    let false_label: String = instr.read_operand(0);

                    // False branch → jump to label
                    if let Some(false_block) = block_for_label(&leader_list, &node.labels, &false_label) {
                        dests_to_add.push((
                            block_idx,
                            Destination::Block(BlockDestination {
                                block_index: false_block,
                                condition: Condition::ExpressionIsFalse,
                            }),
                        ));
                        ancestors_to_add.push((false_block, block_idx));
                    }

                    // True branch → fall through to the next instruction
                    if let Some(true_block) = block_for_instr(&leader_list, global_i + 1)
                        && true_block != block_idx
                    {
                        dests_to_add.push((
                            block_idx,
                            Destination::Block(BlockDestination {
                                block_index: true_block,
                                condition: Condition::ExpressionIsTrue,
                            }),
                        ));
                        ancestors_to_add.push((true_block, block_idx));
                    }
                    current_string_at_tos = None;
                }

                // --- Cross-node jumps ---
                OpCode::RunNode => {
                    // Node name was PushString'd onto the stack.
                    let dest = match &current_string_at_tos {
                        Some(name) => Destination::Node(NodeDestination {
                            node_name: name.clone(),
                            condition: Condition::DirectJump,
                            return_to: None,
                        }),
                        None => Destination::AnyNode(AnyNodeDestination { return_to: None }),
                    };
                    dests_to_add.push((block_idx, dest));
                    current_string_at_tos = None;
                }

                OpCode::DetourToNode => {
                    // Node name was PushString'd; execution resumes at the
                    // instruction immediately after this one when the detoured
                    // node finishes.
                    let return_to_instr = global_i + 1;
                    let return_to = block_for_instr(&leader_list, return_to_instr).map(|ret_block| ReturnToDestination {
                        node_name: node.name.clone(),
                        block_index: ret_block,
                    });
                    let dest = match &current_string_at_tos {
                        Some(name) => Destination::Node(NodeDestination {
                            node_name: name.clone(),
                            condition: Condition::DirectJump,
                            return_to,
                        }),
                        None => Destination::AnyNode(AnyNodeDestination { return_to }),
                    };
                    dests_to_add.push((block_idx, dest));
                    current_string_at_tos = None;
                }

                // --- Terminator instructions: no outgoing edge ---
                OpCode::Stop | OpCode::Return => {
                    // Execution stops here; no destination to add.
                }

                // --- Saliency jump (integer instruction index from TOS) ---
                OpCode::PeekAndJump => {
                    // The destination is an integer pushed by SelectSaliencyCandidate;
                    // we cannot resolve it to a specific block statically.
                }

                // All other opcodes do not affect control flow.
                _ => {}
            }
        }

        // Fallthrough: if this block has no explicit destinations and its last
        // instruction is not a terminal, it falls through to the next block.
        let has_any_dest = dests_to_add.iter().any(|(idx, _)| *idx == block_idx);
        if !has_any_dest {
            let last_op = block.instructions.last().and_then(|i| OpCode::try_from(i.opcode).ok());
            let is_terminal = matches!(last_op, Some(OpCode::Stop) | Some(OpCode::Return));
            if !is_terminal {
                let next_instr = block.first_instruction_index + block.instructions.len();
                if let Some(next_block) = block_for_instr(&leader_list, next_instr) {
                    dests_to_add.push((
                        block_idx,
                        Destination::Block(BlockDestination {
                            block_index: next_block,
                            condition: Condition::Fallthrough,
                        }),
                    ));
                    ancestors_to_add.push((next_block, block_idx));
                }
            }
        }
    }

    // Apply the collected edges.
    for (block_idx, dest) in dests_to_add {
        blocks[block_idx].destinations.push(dest);
    }
    for (dest_block, ancestor_block) in ancestors_to_add {
        let ancestors = &mut blocks[dest_block].ancestor_indices;
        if !ancestors.contains(&ancestor_block) {
            ancestors.push(ancestor_block);
        }
    }

    blocks
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Returns the block index (into a contiguous `leader_list`) whose range
/// contains `instr_idx`.  Returns `None` if `instr_idx` is out of range.
fn block_for_instr(leader_list: &[usize], instr_idx: usize) -> Option<usize> {
    let pos = leader_list.partition_point(|&s| s <= instr_idx);
    pos.checked_sub(1)
}

/// Looks up a label name in `labels`, then maps the resulting instruction
/// index to a block index.
fn block_for_label(leader_list: &[usize], labels: &BTreeMap<String, i32>, label: &str) -> Option<usize> {
    labels.get(label).and_then(|&idx| block_for_instr(leader_list, idx as usize))
}
