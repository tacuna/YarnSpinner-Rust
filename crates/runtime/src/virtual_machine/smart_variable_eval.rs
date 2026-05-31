use crate::prelude::*;
use crate::saliency::{ContentSaliencyOption, ContentSaliencyStrategy, ContentType};
use yarnspinner_core::prelude::*;

/// The tag on synthetic nodes that represent smart variable evaluators.
const SMART_VARIABLE_NODE_TAG: &str = "Yarn.SmartVariable";

/// The tag on synthetic nodes that represent node group hubs.
const NODE_GROUP_HUB_TAG: &str = "yarn_internal_node_group";

/// Checks whether the given variable name refers to a smart variable in the program.
pub(crate) fn is_smart_variable(program: &Program, variable_name: &str) -> bool {
    if let Some(node) = program.nodes.get(variable_name) {
        node.tags.iter().any(|t| t == SMART_VARIABLE_NODE_TAG)
    } else {
        false
    }
}

/// Evaluates a smart variable by executing the bytecode in its synthetic node.
///
/// Returns the computed `InternalValue`. Panics if the node doesn't exist,
/// the stack is empty after evaluation, or an unsupported opcode is encountered.
pub(crate) fn evaluate_smart_variable(
    variable_name: &str,
    program: &Program,
    variable_storage: &dyn VariableStorage,
    library: &Library,
    strategy: &dyn ContentSaliencyStrategy,
    function_call_fn: &mut impl FnMut(&dyn UntypedYarnFn, Vec<YarnValue>) -> YarnValue,
) -> InternalValue {
    let node = program
        .nodes
        .get(variable_name)
        .unwrap_or_else(|| panic!("No smart variable node found for {variable_name}"));

    let mut stack: Vec<InternalValue> = Vec::new();
    let mut pc: usize = 0;

    while pc < node.instructions.len() {
        let instruction = &node.instructions[pc];
        let opcode: OpCode = instruction.opcode.try_into().unwrap();

        match opcode {
            OpCode::PushString => {
                let s: String = instruction.read_operand(0);
                stack.push(s.into());
                pc += 1;
            }
            OpCode::PushFloat => {
                let f: f32 = instruction.read_operand(0);
                stack.push(f.into());
                pc += 1;
            }
            OpCode::PushBool => {
                let b: bool = instruction.read_operand(0);
                stack.push(b.into());
                pc += 1;
            }
            OpCode::Pop => {
                stack.pop().expect("Stack underflow in smart variable evaluation");
                pc += 1;
            }
            OpCode::PushVariable => {
                let name: String = instruction.read_operand(0);
                let value = resolve_variable(&name, program, variable_storage, library, strategy, function_call_fn);
                stack.push(value);
                pc += 1;
            }
            OpCode::CallFunc => {
                let actual_param_count: usize = stack
                    .pop()
                    .expect("Stack underflow reading param count")
                    .try_into()
                    .unwrap_or_else(|e: YarnValueCastError| panic!("Failed to convert param count: {e:?}"));

                let mut parameters: Vec<YarnValue> = (0..actual_param_count)
                    .map(|_| stack.pop().expect("Stack underflow reading function parameters").raw_value)
                    .collect();
                parameters.reverse();

                let function_name: String = instruction.read_operand(0);

                // has_any_content needs program access; handle it specially.
                if function_name == "has_any_content" {
                    let node_group: String = parameters.into_iter().next().expect("has_any_content requires 1 argument").into();
                    let result = has_any_content(&node_group, program, variable_storage, library, strategy, function_call_fn);
                    stack.push(InternalValue {
                        raw_value: YarnValue::Boolean(result),
                        r#type: Type::Boolean,
                    });
                } else {
                    let function = library
                        .get(&function_name)
                        .unwrap_or_else(|| panic!("Function {function_name} not found in library during smart variable evaluation"));

                    let return_value = function_call_fn(function, parameters);
                    let return_type: Type = function
                        .return_type()
                        .try_into()
                        .unwrap_or_else(|e| panic!("Failed to get return type for {function_name}: {e:?}"));
                    stack.push(InternalValue {
                        raw_value: return_value,
                        r#type: return_type,
                    });
                }
                pc += 1;
            }
            OpCode::JumpIfFalse => {
                let top: bool = (&stack.last().expect("Stack underflow in JumpIfFalse").raw_value)
                    .try_into()
                    .unwrap_or_else(|e: YarnValueCastError| panic!("Failed to convert to bool in JumpIfFalse: {e:?}"));
                if !top {
                    let label_name: String = instruction.read_operand(0);
                    pc = *node.labels.get(&label_name).unwrap_or_else(|| panic!("Label {label_name} not found")) as usize;
                } else {
                    pc += 1;
                }
            }
            OpCode::Stop => {
                break;
            }
            other => {
                panic!("Unsupported opcode {other:?} in smart variable evaluation for {variable_name}");
            }
        }
    }

    assert!(!stack.is_empty(), "Stack was empty after evaluating smart variable {variable_name}");
    stack.pop().unwrap()
}

/// Resolves a variable value: first from storage, then initial_values, then smart variable evaluation.
fn resolve_variable(
    name: &str,
    program: &Program,
    variable_storage: &dyn VariableStorage,
    library: &Library,
    strategy: &dyn ContentSaliencyStrategy,
    function_call_fn: &mut impl FnMut(&dyn UntypedYarnFn, Vec<YarnValue>) -> YarnValue,
) -> InternalValue {
    // Try variable storage first
    if let Ok(value) = variable_storage.get(name) {
        return value.into();
    }

    // Try initial values
    if let Some(initial) = program.initial_values.get(name) {
        let yarn_value: YarnValue = initial.clone().into();
        return yarn_value.into();
    }

    // Try smart variable (recursive)
    if is_smart_variable(program, name) {
        return evaluate_smart_variable(name, program, variable_storage, library, strategy, function_call_fn);
    }

    // Internal variable defaults
    if name.starts_with("$Yarn.Internal.Once.") {
        return InternalValue::from(false);
    }
    if name.starts_with("$Yarn.Internal.Content.ViewCount.") {
        return InternalValue::from(0.0f32);
    }

    panic!("Failed to fetch any value for {name} when evaluating a smart variable");
}

/// Executes a node group hub's bytecode and collects all content saliency candidates.
///
/// Returns `None` if the node does not exist or is not a node group hub.
/// Returns `Some(candidates)` with one entry per member node, each with passing/failing
/// condition counts and complexity scores based on the current variable storage.
///
/// Unlike [`has_any_content`], this function does **not** invoke the saliency strategy —
/// it simply evaluates all conditions and returns the full candidate list for the caller
/// to inspect or pass to a strategy.
pub(crate) fn collect_hub_saliency_candidates(
    node_group: &str,
    program: &Program,
    variable_storage: &dyn VariableStorage,
    library: &Library,
    strategy: &dyn ContentSaliencyStrategy,
    function_call_fn: &mut impl FnMut(&dyn UntypedYarnFn, Vec<YarnValue>) -> YarnValue,
) -> Option<Vec<ContentSaliencyOption>> {
    let node = program.nodes.get(node_group)?;

    if !node.tags.iter().any(|t| t == NODE_GROUP_HUB_TAG) {
        // Not a hub node — callers should handle non-hubs separately.
        return None;
    }

    let mut stack: Vec<InternalValue> = Vec::new();
    let mut pc: usize = 0;
    let mut candidates: Vec<ContentSaliencyOption> = Vec::new();

    while pc < node.instructions.len() {
        let instruction = &node.instructions[pc];
        let opcode: OpCode = instruction.opcode.try_into().unwrap();

        match opcode {
            OpCode::PushString => {
                let s: String = instruction.read_operand(0);
                stack.push(s.into());
                pc += 1;
            }
            OpCode::PushFloat => {
                let f: f32 = instruction.read_operand(0);
                stack.push(f.into());
                pc += 1;
            }
            OpCode::PushBool => {
                let b: bool = instruction.read_operand(0);
                stack.push(b.into());
                pc += 1;
            }
            OpCode::Pop => {
                stack.pop().expect("Stack underflow in hub saliency evaluation");
                pc += 1;
            }
            OpCode::PushVariable => {
                let name: String = instruction.read_operand(0);
                let value = resolve_variable(&name, program, variable_storage, library, strategy, function_call_fn);
                stack.push(value);
                pc += 1;
            }
            OpCode::StoreVariable => {
                // Read-only evaluation: skip the store.
                // Hub nodes may have visit-tracking code; we do not modify state here.
                pc += 1;
            }
            OpCode::CallFunc => {
                let actual_param_count: usize = stack
                    .pop()
                    .expect("Stack underflow reading param count")
                    .try_into()
                    .unwrap_or_else(|e: YarnValueCastError| panic!("Failed to convert param count: {e:?}"));

                let mut parameters: Vec<YarnValue> = (0..actual_param_count)
                    .map(|_| stack.pop().expect("Stack underflow reading function parameters").raw_value)
                    .collect();
                parameters.reverse();

                let function_name: String = instruction.read_operand(0);

                if function_name == "has_any_content" {
                    // Condition uses has_any_content for a nested node group —
                    // evaluate it as a boolean (does the group have runnable content?).
                    let nested_group: String = parameters.into_iter().next().expect("has_any_content requires 1 argument").into();
                    let result = has_any_content(&nested_group, program, variable_storage, library, strategy, function_call_fn);
                    stack.push(InternalValue {
                        raw_value: YarnValue::Boolean(result),
                        r#type: Type::Boolean,
                    });
                } else {
                    let function = library
                        .get(&function_name)
                        .unwrap_or_else(|| panic!("Function {function_name} not found in library during hub saliency evaluation"));

                    let return_value = function_call_fn(function, parameters);
                    let return_type: Type = function
                        .return_type()
                        .try_into()
                        .unwrap_or_else(|e| panic!("Failed to get return type for {function_name}: {e:?}"));
                    stack.push(InternalValue {
                        raw_value: return_value,
                        r#type: return_type,
                    });
                }
                pc += 1;
            }
            OpCode::JumpIfFalse => {
                let top: bool = (&stack.last().expect("Stack underflow in JumpIfFalse").raw_value)
                    .try_into()
                    .unwrap_or_else(|e: YarnValueCastError| panic!("Failed to convert to bool in JumpIfFalse: {e:?}"));
                if !top {
                    let label_name: String = instruction.read_operand(0);
                    pc = *node.labels.get(&label_name).unwrap_or_else(|| panic!("Label {label_name} not found")) as usize;
                } else {
                    pc += 1;
                }
            }
            OpCode::AddSaliencyCandidate => {
                // The condition result is on top of the stack.
                let condition_passed: bool = stack
                    .pop()
                    .expect("Stack underflow in AddSaliencyCandidate")
                    .try_into()
                    .unwrap_or_else(|e: YarnValueCastError| panic!("Failed to convert condition to bool: {e:?}"));
                let content_id: String = instruction.read_operand(0);
                let complexity_score: f32 = instruction.read_operand(1);
                candidates.push(ContentSaliencyOption {
                    content_id,
                    passing_condition_count: if condition_passed { 1 } else { 0 },
                    failing_condition_count: if condition_passed { 0 } else { 1 },
                    complexity_score: complexity_score as i32,
                    content_type: ContentType::Node,
                    destination: 0,
                });
                pc += 1;
            }
            OpCode::SelectSaliencyCandidate => {
                // All candidates have been collected — return them.
                return Some(candidates);
            }
            _ => {
                // Skip opcodes not relevant to condition evaluation
                // (e.g. PeekAndJump, DetourToNode, Return, etc.)
                pc += 1;
            }
        }
    }

    // Reached end of instructions without SelectSaliencyCandidate.
    // Return whatever candidates were collected (handles unusual hub shapes).
    Some(candidates)
}

/// Checks whether a node group has any content that could currently run.
///
/// - If the node doesn't exist in the program, returns `false`.
/// - If the node is not a group hub, returns `true` (plain nodes always have content).
/// - If the node is a group hub, evaluates all `when:` conditions via
///   [`collect_hub_saliency_candidates`] and queries the strategy to determine if
///   any content would be selected.
pub(crate) fn has_any_content(
    node_group: &str,
    program: &Program,
    variable_storage: &dyn VariableStorage,
    library: &Library,
    strategy: &dyn ContentSaliencyStrategy,
    function_call_fn: &mut impl FnMut(&dyn UntypedYarnFn, Vec<YarnValue>) -> YarnValue,
) -> bool {
    let Some(node) = program.nodes.get(node_group) else {
        return false;
    };

    if !node.tags.iter().any(|t| t == NODE_GROUP_HUB_TAG) {
        // Not a hub — plain nodes always have content.
        return true;
    }

    // Collect all candidates, then ask the strategy whether any are selectable.
    match collect_hub_saliency_candidates(node_group, program, variable_storage, library, strategy, function_call_fn) {
        Some(candidates) => strategy.query_best_content(&candidates, variable_storage).is_some(),
        None => false,
    }
}
