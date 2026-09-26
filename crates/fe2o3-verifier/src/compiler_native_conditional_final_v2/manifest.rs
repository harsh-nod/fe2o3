//! Exact emitted symbol closure, not source authentication or LLVM semantics.
use super::{Budget, Relation, Resource};
use crate::compiler_native_symbol_manifest_v1::{contains, count_matches};
use fe2o3_compiler_ffi::{
    CodeObjectVersion, CompilerModuleHandoffV2, CompilerModuleKindV1,
    CompilerModuleSymbolManifestV1 as Manifest, CompilerModuleSymbolRoleV1 as Role, DeviceTargetV1,
};
use fe2o3_kernel_descriptor::{
    DESCRIPTOR_QUERY_STORAGE_V5, DescriptorWireErrorV5, DeviceDescriptorTableV5,
};
use fe2o3_kernel_ir::{
    AmdGpuDiagnosticOperation, F32MathFunction, F32MathImplementation, FloatOperation, Function,
    FunctionRole, Module, ValueId,
};
use std::{fmt, mem::size_of};

#[derive(Debug)]
pub(crate) enum ManifestErrorV5 {
    Resource(Resource),
    Descriptor(DescriptorWireErrorV5<Resource>),
    Mismatch(&'static str),
}
type E = ManifestErrorV5;
type R<T> = Result<T, E>;
impl From<Resource> for E {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl fmt::Display for E {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "conditional native manifest: {self:?}")
    }
}
impl std::error::Error for E {}

const ROLES: [Role; 5] = [
    Role::KernelEntry,
    Role::InternalHelper,
    Role::DeviceFfiExport,
    Role::UnresolvedExternalImport,
    Role::KernelDescriptor,
];
const SCRATCH: usize = size_of::<[usize; 5]>() + DESCRIPTOR_QUERY_STORAGE_V5;
const FLOAT_STORAGE: usize = size_of::<Option<FloatOperation>>()
    + size_of::<Option<AmdGpuDiagnosticOperation>>()
    + 3 * size_of::<ValueId>();

/// Called only in the same paid text-relation visit. The enclosing account
/// performs postchecks; any refusal/unwind keeps this check's reservations.
pub(crate) fn check_conditional_native_manifest_v5(
    relation: &Relation<'_, '_, '_, '_, '_>,
    native: &CompilerModuleHandoffV2,
    budget: &mut Budget<'_>,
) -> R<()> {
    budget.charge_work(
        native
            .module_bytes()
            .len()
            .checked_add(relation.final_llvm().len())
            .and_then(|n| n.checked_add(128))
            .ok_or(Resource::Arithmetic)?,
    )?;
    if native.kind() != CompilerModuleKindV1::LlvmTextIr
        || native.code_object_version() != CodeObjectVersion::V6
        || DeviceTargetV1::parse(relation.profile().device_target()).ok() != Some(native.target())
        || native.module_bytes() != relation.final_llvm().as_bytes()
    {
        return Err(E::Mismatch("exact checked LLVM/profile/COV6"));
    }
    // V2 construction/decoding already enforces exact manifest/envelope directions.
    check_parts(
        relation.output().module(),
        relation.descriptors(),
        native.symbol_manifest(),
        budget,
    )
}

fn expect(manifest: &Manifest, role: Role, name: &str, budget: &mut Budget<'_>) -> R<()> {
    budget.charge_work(
        manifest
            .canonical_bytes()
            .len()
            .checked_add(name.len())
            .and_then(|n| n.checked_add(1))
            .ok_or(Resource::Arithmetic)?,
    )?;
    if !contains(manifest, role, name) {
        return Err(E::Mismatch("complete F/V5 symbol manifest"));
    }
    Ok(())
}

fn check_parts(
    module: &Module,
    table: &DeviceDescriptorTableV5<'_>,
    manifest: &Manifest,
    budget: &mut Budget<'_>,
) -> R<()> {
    budget.reserve_storage(SCRATCH)?;
    budget.charge_work(3)?;
    if table.kernel_count() != module.kernels.len() {
        return Err(E::Mismatch("complete F/V5 kernel count"));
    }
    let mut counts = [0usize; 5];
    for function in &module.functions {
        budget.charge_work(
            function
                .id
                .as_str()
                .len()
                .checked_mul(32)
                .and_then(|n| n.checked_add(8))
                .ok_or(Resource::Arithmetic)?,
        )?;
        let index = match function.role {
            FunctionRole::KernelEntry => Some(0),
            FunctionRole::InternalHelper => Some(1),
            FunctionRole::DeviceFfiExport => Some(2),
            FunctionRole::ExternalImport => None,
        };
        if let Some(index) = index {
            expect(manifest, ROLES[index], function.id.as_str(), budget)?;
            counts[index] = counts[index].checked_add(1).ok_or(Resource::Arithmetic)?;
        }
        if let Some(name) = external_symbol(function, budget)? {
            expect(manifest, Role::UnresolvedExternalImport, name, budget)?;
            counts[3] = counts[3].checked_add(1).ok_or(Resource::Arithmetic)?;
        }
    }
    budget.charge_work(1)?;
    if counts[0] != module.kernels.len() {
        return Err(E::Mismatch("complete F entry functions"));
    }
    for kernel in &module.kernels {
        budget.charge_work(
            kernel
                .id
                .as_str()
                .len()
                .checked_add(kernel.entry.as_str().len())
                .and_then(|n| n.checked_add(1))
                .ok_or(Resource::Arithmetic)?,
        )?;
        if kernel.id.as_str() != kernel.entry.as_str() {
            return Err(E::Mismatch("same conditional F kernel/entry"));
        }
        expect(manifest, Role::KernelEntry, kernel.id.as_str(), budget)?;
    }
    for ordinal in 0..table.kernel_count() {
        let row = table
            .kernel(ordinal, &mut |n| budget.charge_work(n))
            .map_err(E::Descriptor)?;
        expect(manifest, Role::KernelEntry, row.entry_name(), budget)?;
        expect(
            manifest,
            Role::KernelDescriptor,
            row.descriptor_symbol(),
            budget,
        )?;
        counts[4] = counts[4].checked_add(1).ok_or(Resource::Arithmetic)?;
    }
    for (role, count) in ROLES.into_iter().zip(counts) {
        budget.charge_work(
            manifest
                .canonical_bytes()
                .len()
                .checked_add(1)
                .ok_or(Resource::Arithmetic)?,
        )?;
        if !count_matches(manifest, role, count) {
            return Err(E::Mismatch("no extra native symbols"));
        }
    }
    budget.release_storage(SCRATCH)?;
    Ok(())
}

// Reuse the typed IR recognizers used by production emission. F32 recognition
// creates at most three operand slots; pay their bounded backing before entry.
fn external_symbol<'a>(function: &'a Function, budget: &mut Budget<'_>) -> R<Option<&'a str>> {
    budget.reserve_storage(FLOAT_STORAGE)?;
    let float = FloatOperation::from_intrinsic_id(&function.id);
    let capacity = match &float {
        Some(FloatOperation::F32Math { arguments, .. }) => arguments.capacity(),
        _ => 0,
    };
    let extra = capacity
        .saturating_sub(3)
        .checked_mul(size_of::<ValueId>())
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(extra)?;
    let symbol = match &float {
        Some(FloatOperation::F32Math {
            function,
            implementation: F32MathImplementation::OcmlAbiV1,
            ..
        }) => Some(match function {
            F32MathFunction::Sin => "__ocml_sin_f32",
            F32MathFunction::Cos => "__ocml_cos_f32",
            F32MathFunction::Exp => "__ocml_exp_f32",
            F32MathFunction::Exp2 => "__ocml_exp2_f32",
            F32MathFunction::Ln => "__ocml_log_f32",
            F32MathFunction::Log2 => "__ocml_log2_f32",
            F32MathFunction::Log10 => "__ocml_log10_f32",
            _ => return Err(E::Mismatch("invalid OCML intrinsic role")),
        }),
        None if function.role == FunctionRole::ExternalImport
            && AmdGpuDiagnosticOperation::from_intrinsic_id(&function.id).is_none() =>
        {
            Some(function.id.as_str())
        }
        _ => None,
    };
    drop(float);
    budget.release_storage(
        FLOAT_STORAGE
            .checked_add(extra)
            .ok_or(Resource::Arithmetic)?,
    )?;
    Ok(symbol)
}

#[cfg(test)]
#[path = "manifest_tests.rs"]
mod tests;
