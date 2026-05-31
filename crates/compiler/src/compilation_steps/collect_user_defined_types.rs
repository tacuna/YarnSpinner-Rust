use crate::output::enum_type::{EnumCase, EnumType};
use crate::parser::enum_registry::{EnumCaseValue, EnumRawType};
use crate::prelude::*;
use std::collections::HashMap;
use yarnspinner_core::prelude::*;

/// Collects enum type definitions from all parsed files and stores them as
/// [`EnumType`] values in [`Compilation::user_defined_types`].
///
/// This step runs after [`generate_code`] so that the result is already present.
/// Emits YS0040 if the same enum type name is declared in more than one file.
pub(crate) fn collect_user_defined_types(mut state: CompilationIntermediate) -> CompilationIntermediate {
    // Track which file first declared each enum name so we can emit YS0040
    // if a second declaration is encountered.
    let mut first_declared_in: HashMap<String, String> = HashMap::new();

    // Collect all enum types, emitting YS0040 for duplicates.
    let mut new_types: Vec<EnumType> = Vec::new();
    let mut redecl_diagnostics: Vec<Diagnostic> = Vec::new();

    for (file_result, _) in &state.parsed_files {
        for (enum_name, def) in &file_result.enum_registry.enums {
            if let Some(first_file) = first_declared_in.get(enum_name.as_str()) {
                // Already declared in a different file — emit YS0040.
                redecl_diagnostics.push(DiagnosticDescriptor::REDECLARATION_OF_EXISTING_TYPE.create(
                    &file_result.name,
                    format!("Enum type '{}' is already declared in '{}'", enum_name, first_file),
                ));
                continue;
            }
            first_declared_in.insert(enum_name.clone(), file_result.name.clone());

            let raw_type = match def.raw_type {
                EnumRawType::Number => Type::Number,
                EnumRawType::String => Type::String,
            };
            let cases = def
                .cases
                .iter()
                .map(|(case_name, case_val)| EnumCase {
                    name: case_name.clone(),
                    raw_value: match case_val {
                        EnumCaseValue::Number(n) => YarnValue::from(*n as f32),
                        EnumCaseValue::Str(s) => YarnValue::from(s.as_str()),
                    },
                    description: String::new(),
                })
                .collect();
            new_types.push(EnumType {
                name: enum_name.clone(),
                description: String::new(),
                raw_type,
                cases,
            });
        }
    }

    state.diagnostics.extend(redecl_diagnostics);
    if let Some(Ok(compilation)) = state.result.as_mut() {
        compilation.user_defined_types.extend(new_types);
    }
    state
}
