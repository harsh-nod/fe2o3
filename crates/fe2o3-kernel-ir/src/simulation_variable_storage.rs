use std::collections::{BTreeMap, BTreeSet};

use crate::{
    DebugSourceMapDocumentV2, FunctionRole, Module, SemanticKernelStorageV1,
    SemanticKirStorageRepresentationV1 as Representation, SemanticStorageBindingV1 as Storage,
    SemanticVariableStorageV1, Type,
};

#[derive(Debug)]
pub(crate) struct VariableStorageMismatch;

/// Join whole-variable storage, never aggregate components, against the exact
/// defined function. Helper associations must be consistent and injective.
pub(crate) fn validate_variable_storage(
    kernels: &[SemanticKernelStorageV1],
    variables: &[SemanticVariableStorageV1],
    source: &DebugSourceMapDocumentV2,
    module: &Module,
) -> Result<(), VariableStorageMismatch> {
    if variables.len() != source.variables().len() {
        return Err(VariableStorageMismatch);
    }
    let mut semantic_functions = BTreeMap::new();
    let mut physical_functions = BTreeMap::new();
    let roots = kernels
        .iter()
        .map(SemanticKernelStorageV1::semantic_root)
        .collect::<BTreeSet<_>>();
    for kernel in kernels {
        let ordinal = u64::from(kernel.kir_function_ordinal());
        let function = module
            .functions
            .get(kernel.kir_function_ordinal() as usize)
            .ok_or(VariableStorageMismatch)?;
        if function.role != FunctionRole::KernelEntry
            || function.body.is_none()
            || semantic_functions
                .insert(kernel.semantic_body(), ordinal)
                .is_some()
            || physical_functions
                .insert(ordinal, kernel.semantic_body())
                .is_some()
        {
            return Err(VariableStorageMismatch);
        }
    }
    for binding in variables {
        let index = source
            .variables()
            .binary_search_by_key(&binding.variable_identity(), |variable| variable.identity())
            .map_err(|_| VariableStorageMismatch)?;
        let variable = &source.variables()[index];
        let ordinal = variable.function_ordinal();
        let function = module
            .functions
            .get(usize::try_from(ordinal).map_err(|_| VariableStorageMismatch)?)
            .ok_or(VariableStorageMismatch)?;
        let body = function.body.as_ref().ok_or(VariableStorageMismatch)?;
        match semantic_functions.get(&binding.semantic_function()) {
            Some(expected) if *expected != ordinal => return Err(VariableStorageMismatch),
            Some(_) => {}
            None if function.role != FunctionRole::InternalHelper
                || roots.contains(&binding.semantic_function()) =>
            {
                return Err(VariableStorageMismatch);
            }
            None => {
                semantic_functions.insert(binding.semantic_function(), ordinal);
            }
        }
        if let Some(previous) = physical_functions.insert(ordinal, binding.semantic_function())
            && previous != binding.semantic_function()
        {
            return Err(VariableStorageMismatch);
        }
        match (binding.storage(), variable.function_binding()) {
            (
                Storage::ExactKirParameter {
                    kir_parameter_ordinal,
                    kir_value_ordinal,
                    representation,
                },
                Some(value),
            ) => {
                let slot = *kir_parameter_ordinal as usize;
                let ty = function
                    .signature
                    .parameters
                    .get(slot)
                    .ok_or(VariableStorageMismatch)?;
                if body.parameters.get(slot).map(|value| value.0) != Some(*kir_value_ordinal)
                    || value.generation() != 1
                    || value.value_ordinal() != u64::from(*kir_value_ordinal)
                    || !matches!(
                        (representation, ty),
                        (Representation::Scalar, Type::Scalar(_))
                            | (Representation::RegionSlice, Type::Slice(_))
                            | (Representation::RegionPointer, Type::Pointer(_))
                            | (Representation::OpaqueFlattened, _)
                    )
                {
                    return Err(VariableStorageMismatch);
                }
            }
            (Storage::ExactKirParameter { .. }, _) | (_, Some(_)) => {
                return Err(VariableStorageMismatch);
            }
            (_, None) => {}
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "simulation_variable_storage_tests.rs"]
mod tests;
