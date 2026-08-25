//! Workload-neutral validation shared by compiler-module handoff producers.

use crate::kernel_ir_codegen::InertCompilerModuleTextV1;
use fe2o3_compiler_ffi::{
    CompilerFfiEnvelopeV1, CompilerModuleSymbolManifestErrorV1, CompilerModuleSymbolManifestV1,
    CompilerModuleSymbolRoleV1, DeviceTargetV1,
};
use fe2o3_kernel_ir::{
    AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE, AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME,
    Module, TargetCapability,
};
use std::collections::BTreeSet;

pub(crate) fn construct_symbol_manifest(
    module: &InertCompilerModuleTextV1,
) -> Result<CompilerModuleSymbolManifestV1, CompilerModuleSymbolManifestErrorV1> {
    use CompilerModuleSymbolRoleV1 as Role;

    let mut entries = Vec::new();
    entries.extend(
        module
            .kernel_entries()
            .iter()
            .cloned()
            .map(|symbol| (Role::KernelEntry, symbol)),
    );
    entries.extend(
        module
            .kernel_entries()
            .iter()
            .map(|symbol| (Role::KernelDescriptor, format!("{symbol}.kd"))),
    );
    entries.extend(
        module
            .device_ffi_exports()
            .iter()
            .cloned()
            .map(|symbol| (Role::DeviceFfiExport, symbol)),
    );
    entries.extend(
        module
            .internal_helpers()
            .iter()
            .cloned()
            .map(|symbol| (Role::InternalHelper, symbol)),
    );
    entries.extend(
        module
            .external_declarations()
            .iter()
            .cloned()
            .map(|symbol| (Role::UnresolvedExternalImport, symbol)),
    );
    CompilerModuleSymbolManifestV1::new(entries)
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct ExactTargetBindingError {
    pub(crate) module: Vec<String>,
    pub(crate) envelope: String,
}

pub(crate) fn validate_exact_target_binding(
    envelope_target: DeviceTargetV1,
    module: &Module,
) -> Result<(), ExactTargetBindingError> {
    let bindings = module
        .required_capabilities
        .iter()
        .chain(
            module
                .functions
                .iter()
                .flat_map(|function| &function.required_capabilities),
        )
        .chain(
            module
                .kernels
                .iter()
                .flat_map(|kernel| &kernel.required_capabilities),
        )
        .filter_map(|capability| match capability {
            TargetCapability::Extension { namespace, name }
                if namespace == AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE =>
            {
                Some(name.clone())
            }
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    if bindings.is_empty() {
        return Ok(());
    }

    let envelope_target = envelope_target.to_string();
    if bindings.len() == 1
        && bindings.contains(AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME)
        && envelope_target == AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME
    {
        return Ok(());
    }

    Err(ExactTargetBindingError {
        module: bindings.into_iter().collect(),
        envelope: envelope_target,
    })
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum CompilerModuleRoleError {
    MissingExternalDeclaration(String),
    MissingCompilerDefinition(String),
}

pub(crate) fn validate_envelope_module_roles(
    envelope: &CompilerFfiEnvelopeV1,
    module: &InertCompilerModuleTextV1,
) -> Result<(), CompilerModuleRoleError> {
    let symbols = envelope.directional_symbols();
    for symbol in symbols.imports() {
        if module
            .external_declarations()
            .binary_search_by(|candidate| candidate.as_str().cmp(symbol))
            .is_err()
        {
            return Err(CompilerModuleRoleError::MissingExternalDeclaration(
                symbol.to_owned(),
            ));
        }
    }
    for symbol in symbols.exports() {
        if module
            .device_ffi_exports()
            .binary_search_by(|candidate| candidate.as_str().cmp(symbol))
            .is_err()
        {
            return Err(CompilerModuleRoleError::MissingCompilerDefinition(
                symbol.to_owned(),
            ));
        }
    }
    Ok(())
}
