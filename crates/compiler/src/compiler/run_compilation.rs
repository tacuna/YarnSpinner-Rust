use crate::Result;
use crate::compilation_steps::*;
use crate::output::*;
use crate::prelude::*;
use crate::string_table_manager::StringTableManager;
use crate::visitors::*;
use std::collections::{HashMap, HashSet, VecDeque};

/// Compile Yarn code, as specified by a compilation job.
pub(crate) fn compile(compiler: &Compiler) -> Result<Compilation> {
    let compiler_steps: Vec<&CompilationStep> = vec![
        &register_initial_variables,
        &parse_files,
        &check_preview_features,
        &validate_syntax,
        &register_strings,
        &validate_shadow_lines,
        &find_node_groups,
        &declare_when_expression_variables,
        &validate_unique_node_names,
        &validate_unique_node_subtitles,
        &validate_when_headers,
        &break_on_job_with_only_strings,
        &get_declarations,
        &check_types,
        &type_check_when_headers,
        &check_unused_variables,
        &detect_smart_variable_loops,
        &find_tracking_nodes,
        &create_declarations_for_tracking_nodes,
        &add_tracking_declarations,
        &resolve_deferred_type_diagnostic,
        &add_error_for_empty_nodes,
        &break_on_job_with_only_declarations,
        &generate_code,
        &detect_unreachable_code,
        &compile_node_groups,
        &validate_undefined_node_references,
        &detect_unreferenced_nodes,
        &detect_cyclic_nodes,
        &compile_smart_variables,
        &add_initial_value_registrations,
        &collect_user_defined_types,
    ];

    let chars: Vec<Vec<u32>> = compiler
        .files
        .iter()
        .map(|file| {
            // Strip the BOM from the source string if it is present before compiling.
            // Rust does not do this by default
            // https://github.com/rust-lang/rfcs/issues/2428
            let source = match file.source.strip_prefix('\u{feff}') {
                None => file.source.as_str(),
                Some(sanitized_string) => sanitized_string,
            };
            source.chars().map(|c| c as u32).collect()
        })
        .collect();
    let chars: Vec<_> = chars.iter().map(|c| c.as_slice()).collect();
    let initial = CompilationIntermediate::from_job(compiler, chars);
    let mut intermediate = compiler_steps
        .into_iter()
        .fold(initial, |state, step| if state.early_break { state } else { step(state) });
    // Cleaning up diagnostics doesn't change the state but makes sure
    // that diagnostics are unique, there are no errors in the warnings, etc.
    // So we execute it even if we've had early breaks.
    intermediate = clean_up_diagnostics(intermediate);

    // Apply any user-supplied severity overrides.
    if !compiler.diagnostic_severities.is_empty() {
        for diag in &mut intermediate.diagnostics {
            if let Some(code) = &diag.code
                && let Some(&new_severity) = compiler.diagnostic_severities.get(code.as_str())
            {
                diag.severity = new_severity;
            }
        }
        // Remove suppressed (None-severity) diagnostics.
        intermediate.diagnostics.retain(|d| d.severity != DiagnosticSeverity::None);
        // Re-split: errors → Err, warnings-only → Ok
        use crate::listeners::DiagnosticVec;
        if intermediate.diagnostics.has_errors() {
            intermediate.result = Some(Err(CompilerError(intermediate.diagnostics.clone())));
        } else if let Some(Ok(compilation)) = intermediate.result.as_mut() {
            compilation.warnings.clone_from(&intermediate.diagnostics);
        } else if !intermediate.diagnostics.is_empty() {
            // Build a minimal Ok result if the original was an Err but is now warnings-only.
            // Only reachable when overrides downgrade errors to warnings.
            intermediate.result = Some(Ok(Compilation {
                warnings: intermediate.diagnostics.clone(),
                ..Compilation::default()
            }));
        }
    }

    intermediate.result.unwrap()
}

/// Metadata about a single member node within a node group.
#[derive(Debug, Clone)]
pub(crate) struct NodeGroupMember {
    /// All raw `when:` expression strings for this node, in source order.
    /// Most nodes have one header, but multiple `when:` headers are valid.
    pub(crate) when_expressions: Vec<String>,
    /// The unique internal name for this member node (e.g. "NodeGroup.0").
    pub(crate) unique_name: String,
}

type CompilationStep = dyn Fn(CompilationIntermediate) -> CompilationIntermediate;

pub(crate) struct CompilationIntermediate<'input> {
    pub(crate) job: &'input Compiler,
    pub(crate) file_chars: Vec<&'input [u32]>,
    pub(crate) result: Option<Result<Compilation>>,
    /// All variable declarations that we've encountered, PLUS the ones we knew about before
    pub(crate) known_variable_declarations: Vec<Declaration>,
    /// All variable declarations that we've encountered during this compilation job
    pub(crate) derived_variable_declarations: Vec<Declaration>,
    pub(crate) potential_issues: Vec<DeferredTypeDiagnostic>,
    pub(crate) parsed_files: Vec<(FileParseResult<'input>, KnownTypes)>,
    pub(crate) tracking_nodes: HashSet<String>,
    pub(crate) skip_nodes: HashSet<String>,
    pub(crate) string_table: StringTableManager,
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) file_tags: HashMap<String, Vec<String>>,
    pub(crate) early_break: bool,
    /// Node groups found during compilation: group_title → ordered member list.
    pub(crate) node_groups: HashMap<String, Vec<NodeGroupMember>>,
    /// Per-file name overrides for node group members:
    /// file_name → (original_title → queue of unique names in source order).
    pub(crate) node_group_name_overrides: HashMap<String, HashMap<String, VecDeque<String>>>,
}

impl<'input> CompilationIntermediate<'input> {
    pub(crate) fn from_job(compiler: &'input Compiler, chars: Vec<&'input [u32]>) -> Self {
        Self {
            job: compiler,
            file_chars: chars,
            result: Default::default(),
            known_variable_declarations: Default::default(),
            derived_variable_declarations: Default::default(),
            potential_issues: Default::default(),
            parsed_files: Default::default(),
            tracking_nodes: Default::default(),
            skip_nodes: Default::default(),
            string_table: Default::default(),
            diagnostics: Default::default(),
            file_tags: Default::default(),
            early_break: Default::default(),
            node_groups: Default::default(),
            node_group_name_overrides: Default::default(),
        }
    }
}
