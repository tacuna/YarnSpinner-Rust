//! Adapted from <https://github.com/YarnSpinnerTool/YarnSpinner/blob/3a5b7343f715e4e9a3705fa4224e7fa510b92f1c/YarnSpinner.Tests/TestPlan.cs>

use crate::prelude::*;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TestPlan {
    pub next_expected_step: ExpectedStepType,
    pub next_expected_options: Vec<ProcessedOption>,
    pub next_step_value: Option<StepValue>,
    steps: Vec<Step>,
    current_test_plan_step: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessedOption {
    pub line: String,
    pub enabled: bool,
}

impl TestPlan {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn read(path: impl AsRef<Path>) -> Self {
        let steps = fs::read_to_string(path)
            .unwrap()
            .lines()
            // Skip commented lines
            .filter(|line| !line.trim_start().starts_with('#'))
            // Skip empty or blank lines
            .filter(|line| !line.trim().is_empty())
            // Filter out standalone 'd' tokens (detour-done markers used in some v3.0 testplans)
            .filter(|line| line.trim() != "d")
            .map(|line| {
                // v3.0 testplan: `---` (or `--`) means restart dialogue
                if line.trim() == "---" || line.trim() == "--" {
                    Step::with_expected_step_type(ExpectedStepType::Restart)
                } else if Step::has_known_prefix(line) {
                    Step::read(line)
                } else {
                    // v3.0: bare text lines without a step prefix are treated as expected lines
                    Step::read(&format!("line: {}", line.trim()))
                }
            })
            .collect();
        Self { steps, ..Default::default() }
    }

    /// Process all non-blocking steps starting from the current position,
    /// stopping just before the first blocking step. Does not consume the
    /// first blocking step.
    pub fn process_non_blocking(&mut self, dialogue: &mut Dialogue) {
        while let Some(step) = self.steps.get(self.current_test_plan_step) {
            if step.expected_step_type.is_blocking() {
                break;
            }
            // Process this non-blocking step
            self.current_test_plan_step += 1;
            match step.expected_step_type {
                ExpectedStepType::Set => {
                    let Some(StepValue::StringPair(var, value)) = step.value.clone() else {
                        panic!("Expected set to be a pair of strings");
                    };
                    let current_value = dialogue.variable_storage().get(&var).unwrap();
                    let new_value = match current_value {
                        YarnValue::Number(_) => YarnValue::Number(value.parse::<f32>().unwrap()),
                        YarnValue::String(_) => YarnValue::String(value),
                        YarnValue::Boolean(_) => YarnValue::Boolean(value.parse::<bool>().unwrap()),
                    };
                    println!("INFO: Variable {} set to {}", var, new_value);
                    dialogue.variable_storage_mut().set(var, new_value).unwrap();
                }
                ExpectedStepType::Run => {
                    let Some(StepValue::String(next_node)) = step.value.clone() else {
                        panic!("Expected run to be a string");
                    };
                    println!("INFO: Jumped to node {}", next_node);
                    let _ = dialogue.set_node(next_node);
                }
                ExpectedStepType::Restart => {
                    println!("INFO: Restarting dialogue");
                    let _ = dialogue.set_node("Start");
                }
                ExpectedStepType::Saliency => {
                    // Saliency mode change -- skip
                }
                ExpectedStepType::Node => {
                    // v3.0: override the start node after a restart
                    if let Some(StepValue::String(next_node)) = step.value.clone() {
                        println!("INFO: Starting at node {}", next_node);
                        let _ = dialogue.set_node(next_node);
                    }
                }
                _ => {}
            }
        }
    }

    pub fn next(&mut self, dialogue: &mut Dialogue) {
        // step through the test plan until we hit an expectation to
        // see a line, option, or command. specifically, we're waiting
        // to see if we got a Line, Select, Command or Assert step
        // type.
        if self.next_expected_step == ExpectedStepType::Select {
            // our previously-notified task was to select an option.
            // we've now moved past that, so clear the list of expected
            // options.
            self.next_expected_options.clear();
            self.next_step_value = Some(StepValue::Number(0));
        }

        for current_step in self.steps.iter().skip(self.current_test_plan_step) {
            self.current_test_plan_step += 1;

            match current_step.expected_step_type {
                ExpectedStepType::Option => {
                    let Some(StepValue::String(line)) = current_step.value.clone() else {
                        panic!("Expected option line to be a string");
                    };

                    self.next_expected_options.push(ProcessedOption {
                        line,
                        enabled: current_step.expect_option_enabled,
                    });
                }
                ExpectedStepType::Line | ExpectedStepType::Command | ExpectedStepType::Select => {
                    self.next_expected_step = current_step.expected_step_type;
                    self.next_step_value.clone_from(&current_step.value);
                    return;
                }
                ExpectedStepType::Stop => {
                    self.next_expected_step = current_step.expected_step_type;
                    return;
                }
                ExpectedStepType::Set => {
                    let Some(StepValue::StringPair(var, value)) = current_step.value.clone() else {
                        panic!("Expected run line to be a pair of strings");
                    };

                    // .unwrap() used to panic on error as this is used only in Test
                    let current_value = dialogue.variable_storage().get(&var).unwrap();
                    let new_value = match current_value {
                        YarnValue::Number(_) => YarnValue::Number(value.parse::<f32>().unwrap()),
                        YarnValue::String(_) => YarnValue::String(value),
                        YarnValue::Boolean(_) => YarnValue::Boolean(value.parse::<bool>().unwrap()),
                    };

                    println!("INFO: Variable {} set to {}", var, new_value);
                    dialogue.variable_storage_mut().set(var, new_value).unwrap();
                }
                ExpectedStepType::Run => {
                    let Some(StepValue::String(next_node)) = current_step.value.clone() else {
                        panic!("Expected run line to be a string");
                    };

                    println!("INFO: Jumped to node {}", next_node);
                    let _ = dialogue.set_node(next_node);
                }
                ExpectedStepType::Saliency => {
                    // Saliency mode change -- skip
                }
                ExpectedStepType::Node => {
                    // v3.0: override the start node
                    if let Some(StepValue::String(next_node)) = current_step.value.clone() {
                        println!("INFO: Starting at node {}", next_node);
                        let _ = dialogue.set_node(next_node);
                    }
                }
                ExpectedStepType::Restart => {
                    // v3.0: restart dialogue from the Start node
                    println!("INFO: Restarting dialogue");
                    let _ = dialogue.set_node("Start");
                }
            }
        }

        // We've fallen off the end of the test plan step list. We
        // expect a stop here.
        self.next_expected_step = ExpectedStepType::Stop;
    }

    pub fn current_step(&self) -> Option<Step> {
        self.steps.get(self.current_test_plan_step).cloned()
    }

    /// Check if there's a Restart step among the upcoming non-blocking steps.
    pub fn has_pending_restart(&self) -> bool {
        let mut i = self.current_test_plan_step;
        while i < self.steps.len() && !self.steps[i].expected_step_type.is_blocking() {
            if self.steps[i].expected_step_type == ExpectedStepType::Restart {
                return true;
            }
            i += 1;
        }
        false
    }

    /// Check if the current run (steps before the next Restart or end)
    /// has any more blocking steps remaining. Used to mimic C#'s pull-based
    /// test runner: when no more blocking steps remain, we stop driving
    /// the dialogue instead of eagerly calling continue_().
    pub fn has_more_blocking_steps_in_current_run(&self) -> bool {
        let mut i = self.current_test_plan_step;
        while i < self.steps.len() {
            let step_type = self.steps[i].expected_step_type;
            if step_type == ExpectedStepType::Restart {
                return false; // Hit run boundary without finding a blocking step
            }
            if step_type.is_blocking() {
                return true;
            }
            i += 1;
        }
        false
    }

    pub fn expect_line(mut self, line: impl Into<String>) -> Self {
        self.steps.push(Step::from_line(line));
        self
    }

    pub fn expect_option(mut self, line: impl Into<String>) -> Self {
        self.steps.push(Step::from_option(line));
        self
    }

    pub fn expect_command(mut self, line: impl Into<String>) -> Self {
        self.steps.push(Step::from_command(line));
        self
    }

    pub fn then_select(mut self, selection: usize) -> Self {
        self.steps.push(Step::from_select(selection));
        self
    }

    pub fn then_set(mut self, variable_name: impl Into<String>, value: impl Into<String>) -> Self {
        self.steps.push(Step::from_set(variable_name, value));
        self
    }

    pub fn then_run(mut self, node_name: impl Into<String>) -> Self {
        self.steps.push(Step::from_run(node_name));
        self
    }

    pub fn expect_stop(mut self) -> Self {
        self.steps.push(Step::from_stop());
        self
    }
}
