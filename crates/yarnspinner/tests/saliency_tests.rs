//! Adapted from <https://github.com/YarnSpinnerTool/YarnSpinner/blob/3a5b7343f715e4e9a3705fa4224e7fa510b92f1c/YarnSpinner.Tests/SaliencyTests.cs>

use test_base::prelude::*;
use yarnspinner::compiler::*;
use yarnspinner::core::YarnValue;
use yarnspinner::runtime::*;

mod test_base;

/// Tag used on hub nodes that the compiler emits to represent a node group.
const NODE_GROUP_HUB_TAG: &str = "yarn_internal_node_group";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Compiles `source` as a raw Yarn file (multiple nodes), loads the program
/// into the test dialogue, and returns a `TestBase` ready for running.
fn compile_and_prepare(source: &str, start_node: &str) -> TestBase {
    let result = Compiler::new()
        .add_file(File {
            file_name: "<input>".to_owned(),
            source: source.to_owned(),
        })
        .compile()
        .expect("compilation should succeed without errors");

    // compile().expect() already asserts no errors; warnings are permitted
    assert!(
        result.warnings.iter().all(|d| d.severity != DiagnosticSeverity::Error),
        "source produced errors in warnings: {:?}",
        result.warnings
    );

    let mut test_base = TestBase::default().with_compilation(result);
    test_base.dialogue.set_node(start_node).unwrap();
    test_base
}

/// Drives the dialogue to completion, selecting the first option at any
/// choice point.  This is the equivalent of C#'s basic `Continue()` loop.
fn run_to_completion(dialogue: &mut yarnspinner::runtime::Dialogue) {
    while dialogue.can_continue() {
        let events = dialogue.continue_().expect("dialogue continued without error");
        for event in events {
            if let DialogueEvent::Options(options) = event {
                if !options.is_empty() {
                    dialogue.set_selected_option(OptionId(0)).unwrap();
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Custom strategy used by `test_mocking`
// ---------------------------------------------------------------------------

/// A simple strategy that always returns the **first** option whose
/// `failing_condition_count == 0`, or `None` if all options are failing.
///
/// This mirrors the C# `FirstContentStrategy` used in the mock test.
#[derive(Debug, Clone, Default)]
struct AlwaysFirstStrategy;

impl ContentSaliencyStrategy for AlwaysFirstStrategy {
    fn clone_box(&self) -> Box<dyn ContentSaliencyStrategy> {
        Box::new(self.clone())
    }

    fn query_best_content(&self, content: &[ContentSaliencyOption], _variable_storage: &dyn VariableStorage) -> Option<usize> {
        content.iter().position(|c| c.failing_condition_count == 0)
    }

    fn content_was_selected(&self, _content: &ContentSaliencyOption, _variable_storage: &mut dyn VariableStorage) {}
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Adapted from `TestMocking`.
///
/// Verifies that a custom `ContentSaliencyStrategy` implementation can be
/// constructed, installed on the dialogue, and used to query the saliency
/// candidates returned by `get_saliency_options_for_node_group`.
#[test]
fn test_mocking() {
    let source = r#"
title: Start
when: always
---
Hello from the node group!
===
"#;

    let mut test_base = compile_and_prepare(source, "Start");
    test_base.dialogue.set_content_saliency_strategy(Box::new(AlwaysFirstStrategy));

    let options = test_base
        .dialogue
        .get_saliency_options_for_node_group("Start")
        .expect("Start is a valid node group");

    assert_eq!(options.len(), 1, "one member in the group");

    let storage = test_base.dialogue.variable_storage();
    let chosen = AlwaysFirstStrategy.query_best_content(&options, storage);
    assert_eq!(chosen, Some(0), "strategy should select the first (and only) passing option");
}

/// Adapted from `TestQueryingCandidates`.
///
/// A node group with three members — `when: $condition1`, `when: $condition2`,
/// `when: always` — should return three candidates when queried.
/// With `$condition1 = true` and `$condition2 = false`, exactly two candidates
/// should have `failing_condition_count == 0` and one should have it > 0.
#[test]
fn test_querying_candidates() {
    let source = r#"
title: NodeGroup
when: $condition1
---
<<stop>>
===
title: NodeGroup
when: $condition2
---
<<stop>>
===
title: NodeGroup
when: always
---
<<stop>>
===
"#;

    let mut test_base = compile_and_prepare(source, "NodeGroup");

    // Set the condition variables explicitly so the hub can evaluate them.
    // ($condition1 = true → passes, $condition2 = false → fails)
    test_base
        .variable_storage
        .set("$condition1".to_string(), YarnValue::Boolean(true))
        .unwrap();
    test_base
        .variable_storage
        .set("$condition2".to_string(), YarnValue::Boolean(false))
        .unwrap();

    let options = test_base
        .dialogue
        .get_saliency_options_for_node_group("NodeGroup")
        .expect("NodeGroup is a valid node group");

    assert_eq!(options.len(), 3, "all 3 members should be returned");

    let passing = options.iter().filter(|o| o.failing_condition_count == 0).count();
    let failing = options.iter().filter(|o| o.failing_condition_count > 0).count();
    assert_eq!(passing, 2, "2 nodes are passing ($condition1 and always)");
    assert_eq!(failing, 1, "1 node is failing ($condition2)");
}

/// Adapted from `TestNodesWithOnceHeaderOnlyAppearOnce`.
///
/// A node group with a single `when: once` member should have one passing
/// candidate before it has been run, and zero passing candidates after.
#[test]
fn test_nodes_with_once_header_only_appear_once() {
    let source = r#"
title: Start
when: once
---
This content is only seen once.
===
"#;

    let mut test_base = compile_and_prepare(source, "Start");
    // NOTE: do NOT use FirstSaliencyStrategy here. Its content_was_selected is
    // a no-op and never increments the ViewCount variable. The when: once
    // condition in Rust checks $Yarn.Internal.Content.ViewCount.*, which is
    // only updated by strategies that implement content_was_selected properly
    // (e.g. BestLeastRecentlyViewedSaliencyStrategy, the default).
    test_base
        .dialogue
        .set_content_saliency_strategy(Box::new(BestLeastRecentlyViewedSaliencyStrategy));

    // Before running: the once-condition has never fired, so it passes.
    let before = test_base
        .dialogue
        .get_saliency_options_for_node_group("Start")
        .expect("Start is a valid node group");

    assert_eq!(
        before.iter().filter(|o| o.failing_condition_count == 0).count(),
        1,
        "the once-member should be passing before it runs"
    );

    // Run the dialogue to completion (selects and fires the member).
    run_to_completion(&mut test_base.dialogue);

    // After running: the view-count for the member is > 0 → once-condition fails.
    let after = test_base
        .dialogue
        .get_saliency_options_for_node_group("Start")
        .expect("Start is still a valid node group");

    assert_eq!(
        after.iter().filter(|o| o.failing_condition_count == 0).count(),
        0,
        "the once-member should be failing after it has run"
    );
}

/// Adapted from `TestDialogueCanBeQueriedForNodeGroups`.
///
/// Verifies `is_node_group`, `node_exists`, and
/// `get_saliency_options_for_node_group` on a program that contains one
/// node group (`Start`, 2 members) and one plain node (`NotAGroup`).
#[test]
fn test_dialogue_can_be_queried_for_node_groups() {
    let source = r#"
title: Start
when: once
---
This content is only seen once.
===
title: Start
when: $a == 2
---
This content is only seen when a is 2.
===
title: NotAGroup
---
This node is not part of a node group.
===
"#;

    let mut test_base = compile_and_prepare(source, "Start");

    // Set $a so the when: $a == 2 condition can be evaluated without panicking.
    test_base.variable_storage.set("$a".to_string(), YarnValue::Number(0.0)).unwrap();

    // Node existence and group membership
    assert!(!test_base.dialogue.node_exists("DoesntExist"), "non-existent node");
    assert!(test_base.dialogue.node_exists("Start"), "Start should exist");
    assert!(test_base.dialogue.is_node_group("Start"), "Start is a node group hub");
    assert!(test_base.dialogue.node_exists("NotAGroup"), "NotAGroup should exist");
    assert!(!test_base.dialogue.is_node_group("NotAGroup"), "NotAGroup is a plain node");

    // Querying an invalid node name should return an error.
    assert!(
        test_base.dialogue.get_saliency_options_for_node_group("DoesntExist").is_err(),
        "querying a non-existent node should return Err"
    );

    // The node group hub should report 2 candidates (one per member).
    let start_options = test_base
        .dialogue
        .get_saliency_options_for_node_group("Start")
        .expect("Start is a valid node group");
    assert_eq!(start_options.len(), 2, "Start has 2 member nodes");

    // A plain node should report exactly 1 candidate representing itself.
    let plain_options = test_base
        .dialogue
        .get_saliency_options_for_node_group("NotAGroup")
        .expect("NotAGroup is a valid node");
    assert_eq!(plain_options.len(), 1, "NotAGroup has 1 option (the node itself)");
}

/// Adapted from `TestConditionCounts`.
///
/// Verifies that each node-group member's `complexity_score` matches the
/// expected integer value specified via an `expected:` header.
///
/// The C# complexity scoring is:
///   - `always`          → 0
///   - `once`            → 1
///   - `once if <expr>`  → 1 + bool_ops(expr) + 1
///   - `<expr>`          → bool_ops(expr) + 1
/// Where `bool_ops` counts `&&`/`||`/`^` binary boolean operators.
#[test]
fn test_condition_counts() {
    use std::sync::{Arc, Mutex};

    let source = r#"
title: Start
---
<<set $condition = true>>
<<jump NodeGroup>>
===
title: NodeGroup
when: $condition
expected: 1
---
<<stop>>
===
title: NodeGroup
when: true
expected: 1
---
<<stop>>
===
title: NodeGroup
when: !false
expected: 1
---
<<stop>>
===
title: NodeGroup
when: $condition is true
expected: 1
---
<<stop>>
===
title: NodeGroup
when: once
expected: 1
---
<<stop>>
===
title: NodeGroup
when: once if $condition
expected: 2
---
<<stop>>
===
title: NodeGroup
when: once if $condition && true
expected: 3
---
<<stop>>
===
title: NodeGroup
when: once if $condition && true
when: always
expected: 3
---
<<stop>>
===
title: NodeGroup
when: always
expected: 0
---
<<stop>>
===
title: NodeGroup
when: $condition && ($condition || false)
expected: 3
---
<<stop>>
===
"#;

    // Compile, extracting expected-complexity map from node headers.
    let result = Compiler::new()
        .add_file(File {
            file_name: "<input>".to_owned(),
            source: source.to_owned(),
        })
        .compile()
        .expect("compilation should succeed");

    // Build expected-complexity map from the compiled node headers.
    let program = result.program.as_ref().expect("program present");
    let mut expected_complexities: std::collections::HashMap<String, i32> = std::collections::HashMap::new();
    for (node_name, node) in &program.nodes {
        if let Some(hdr) = node.headers.iter().find(|h| h.key == "expected") {
            let val: i32 = hdr.value.trim().parse().expect("expected: should be an int");
            expected_complexities.insert(node_name.clone(), val);
        }
    }
    assert!(!expected_complexities.is_empty(), "no nodes with expected: header found");

    // Strategy that captures the options passed to it.
    let captured: Arc<Mutex<Vec<ContentSaliencyOption>>> = Arc::new(Mutex::new(Vec::new()));
    let captured_clone = captured.clone();

    #[derive(Clone)]
    struct CapturingStrategy {
        captured: Arc<Mutex<Vec<ContentSaliencyOption>>>,
    }
    impl std::fmt::Debug for CapturingStrategy {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("CapturingStrategy")
        }
    }
    impl ContentSaliencyStrategy for CapturingStrategy {
        fn clone_box(&self) -> Box<dyn ContentSaliencyStrategy> {
            Box::new(self.clone())
        }
        fn query_best_content(&self, content: &[ContentSaliencyOption], _storage: &dyn VariableStorage) -> Option<usize> {
            let mut guard = self.captured.lock().unwrap();
            guard.extend_from_slice(content);
            // Return first passing option (failing_condition_count == 0)
            content.iter().position(|c| c.failing_condition_count == 0)
        }
        fn content_was_selected(&self, _content: &ContentSaliencyOption, _storage: &mut dyn VariableStorage) {}
    }

    let mut test_base = TestBase::default().with_compilation(result);
    test_base
        .dialogue
        .set_content_saliency_strategy(Box::new(CapturingStrategy { captured: captured_clone }));
    test_base.dialogue.set_node("Start").unwrap();

    // Set $condition = true so conditions that require it can pass.
    test_base
        .variable_storage
        .set("$condition".to_string(), YarnValue::Boolean(true))
        .unwrap();

    // Run to let the hub node evaluate candidates.
    run_to_completion(&mut test_base.dialogue);

    let options = captured.lock().unwrap();
    assert!(!options.is_empty(), "saliency strategy should have been invoked with at least one option");

    for opt in options.iter() {
        if let Some(&expected) = expected_complexities.get(&opt.content_id) {
            assert_eq!(
                opt.complexity_score, expected,
                "complexity for {} should be {} but was {}",
                opt.content_id, expected, opt.complexity_score
            );
        }
        // All passing options must have at least one passing condition.
        if opt.failing_condition_count == 0 {
            assert!(
                opt.passing_condition_count > 0,
                "{} should have at least one passing condition",
                opt.content_id
            );
        }
    }
}

/// Adapted from `TestNodeGroupWithSparseSubtitles`.
///
/// Verifies that when a node group has a mix of nodes with and without
/// subtitle headers, the compiled program contains exactly the expected
/// number of nodes whose names start with the group name followed by a dot.
#[test]
fn test_node_group_with_sparse_subtitles() {
    let source = r#"
title: Start
subtitle: Special
when: always
---
This is a special start node which should get a subtitle name.
===
title: Start
when: always
---
This is a random start node which should get a UUID name.
===
title: Start
when: always
---
This is a random start node which should get a UUID name.
===
"#;

    let result = Compiler::new()
        .add_file(yarnspinner::compiler::File {
            file_name: "<input>".to_owned(),
            source: source.to_owned(),
        })
        .compile()
        .unwrap();

    // .compile().unwrap() above already guarantees no errors.

    let program = result.program.unwrap();

    // The hub node itself is named "Start" (tagged yarn_internal_node_group).
    // The member nodes are named "Start.Special", "Start.<UUID>", "Start.<UUID2>".
    assert!(program.nodes.contains_key("Start"), "Expected a hub node named 'Start'");
    assert!(
        program.nodes["Start"].tags.iter().any(|t| t == NODE_GROUP_HUB_TAG),
        "Hub node 'Start' should be tagged '{NODE_GROUP_HUB_TAG}'"
    );
    assert!(
        program.nodes.contains_key("Start.Special"),
        "Expected a member node named 'Start.Special'"
    );

    // Three member nodes should exist (one named, two UUID-suffixed).
    let member_count = program.nodes.keys().filter(|k| k.starts_with("Start.")).count();
    assert_eq!(
        3,
        member_count,
        "Expected 3 member nodes (Start.Special + 2 UUID), found: {:?}",
        program.nodes.keys().filter(|k| k.starts_with("Start.")).collect::<Vec<_>>()
    );
}
