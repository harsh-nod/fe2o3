//! Inert V5 production rows from retained typed source and actual F.
//! No original collector custody, formula proof, or native authority is minted.
#![allow(
    clippy::result_large_err,
    reason = "Typed errors without hidden allocation."
)]
use super::{
    DescriptorArgumentKindV1, TypedDescriptorRootV1, nominal_v3,
    production_descriptor_argument_matches_kernel_type_v1,
};
use crate::production_target_v1::AuthenticatedProductionTargetV1;
use fe2o3_amd_target::ProductionAmdTargetProfileV1 as Profile;
use fe2o3_kernel_descriptor::{
    CONDITIONAL_INVOCATION_CODEC_STORAGE_V2, ConditionalInvocationContractV2,
    DESCRIPTOR_ENCODER_SCRATCH_STORAGE_V5, DescriptorWireErrorV5, DeviceDescriptorTableInputV3,
    DeviceDescriptorTableInputV5, MAX_KERNELS, decode_conditional_invocation_contract_v2,
    encode_device_descriptor_table_v5, encoded_device_descriptor_table_v5_len,
};
use fe2o3_kernel_ir::{
    AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE, AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAME,
    AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAMESPACE,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkLedgerIdentityV1, Module, ScalarType, TargetCapabilityRefV1, Type,
    VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1 as Semantic;
use std::{
    fmt,
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
};

const OUTPUT_DOMAIN: &[u8] = b"FE2O3/CONDITIONAL-NATIVE-OUTPUT/V5\0";
const PRODUCER: &str = "inert-conditional-native-output-v5";

#[derive(Debug)]
pub(crate) enum ConditionalNativeDescriptorErrorV5 {
    Resource(Resource),
    Target(fe2o3_compiler_lineage::ProductionTargetLineageErrorV3),
    Nominal(nominal_v3::NominalDescriptorErrorV3),
    Contract(fe2o3_kernel_descriptor::ConditionalInvocationWireErrorV1<Resource>),
    Wire(DescriptorWireErrorV5<Resource>),
    Mismatch(&'static str),
}
type E = ConditionalNativeDescriptorErrorV5;
type R<T> = Result<T, E>;
impl From<Resource> for E {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl From<nominal_v3::NominalDescriptorErrorV3> for E {
    fn from(value: nominal_v3::NominalDescriptorErrorV3) -> Self {
        Self::Nominal(value)
    }
}
impl fmt::Display for E {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "conditional native descriptor: {self:?}")
    }
}
impl std::error::Error for E {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Resource(e) => e,
            Self::Target(e) => e,
            Self::Nominal(e) => e,
            Self::Contract(e) => e,
            Self::Wire(e) => e,
            Self::Mismatch(_) => return None,
        })
    }
}

/// Unreserved Vec header plus actual output capacity, not input custody.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ConditionalNativeDescriptorStorageV5(usize);
impl ConditionalNativeDescriptorStorageV5 {
    pub(crate) const fn retained_storage(self) -> usize {
        self.0
    }
}
type Output = (Vec<u8>, ConditionalNativeDescriptorStorageV5);

struct Frame {
    floor: usize,
    ledger: CanonicalKernelIrWorkLedgerIdentityV1,
    slot: usize,
}
const HEADER: usize = size_of::<Frame>()
    + size_of::<std::thread::Result<R<Output>>>()
    + size_of::<DeviceDescriptorTableInputV5<'static>>()
    + size_of::<ConditionalNativeDescriptorStorageV5>();

/// Caller retains original input backing. Only complete success returns inert
/// bytes, with an UNRESERVED receipt to reserve before further controlled use.
/// Contract views may be in source order; full bindings select canonical order.
pub(crate) fn encode_conditional_native_descriptor_v5(
    roots: &[TypedDescriptorRootV1],
    semantic: &Semantic,
    output: &Owner,
    target: &AuthenticatedProductionTargetV1,
    contracts: &[ConditionalInvocationContractV2<'_>],
    budget: &mut Budget<'_>,
) -> R<Output> {
    scope(budget, |budget| {
        check_source_target(semantic, target, budget)?;
        encode_rows(
            roots,
            semantic,
            output,
            target.profile(),
            target.rustc_layout().default_pointer_width_bits(),
            contracts,
            budget,
        )
    })
}

// This is a local encoder scope, never an opaque whole-proof refund boundary.
fn scope(budget: &mut Budget<'_>, run: impl FnOnce(&mut Budget<'_>) -> R<Output>) -> R<Output> {
    let frame = Frame {
        floor: budget.storage(),
        ledger: budget.work_ledger_identity_v1(),
        slot: budget as *const Budget<'_> as usize,
    };
    budget.reserve_storage(HEADER)?;
    let result = catch_unwind(AssertUnwindSafe(|| run(budget)));
    let valid = budget.work_ledger_identity_v1() == frame.ledger
        && budget as *const Budget<'_> as usize == frame.slot
        && frame
            .floor
            .checked_add(HEADER)
            .is_some_and(|n| budget.storage() >= n);
    let result = match result {
        Err(payload) => resume_unwind(payload),
        Ok(result) if !valid => {
            drop(result);
            return Err(Resource::Accounting.into());
        }
        Ok(result) => result?,
    };
    let live = budget.storage() - frame.floor;
    if live < result.1.retained_storage() {
        drop(result);
        return Err(Resource::Accounting.into());
    }
    // All row vectors and borrowed codec views have died in run(). Output is
    // transferred unreserved; errors/unwind above keep terminal reservations.
    budget.release_storage(live)?;
    Ok(result)
}

fn check_source_target(
    semantic: &Semantic,
    target: &AuthenticatedProductionTargetV1,
    budget: &mut Budget<'_>,
) -> R<()> {
    let layout = target.rustc_layout();
    // Reuse the original target transcript codec. Its temporary Vec/Box domain
    // is bounded separately; this reservation is not an exact allocator claim.
    let bound = [
        layout.llvm_target(),
        layout.data_layout(),
        layout.active_cpu().unwrap_or_default(),
        layout.active_features().unwrap_or_default(),
    ]
    .into_iter()
    .try_fold(6usize * 8 + 2 + 64, |n, s| n.checked_add(s.len()))
    .ok_or(Resource::Arithmetic)?;
    if bound > fe2o3_compiler_lineage::MAX_PRODUCTION_TARGET_LINEAGE_TRANSCRIPT_BYTES_V3 {
        return Err(E::Mismatch("bounded original target transcript"));
    }
    let scratch = bound
        .checked_mul(2)
        .and_then(|n| {
            n.checked_add(size_of::<Box<[u8]>>() + size_of::<Vec<u8>>() + size_of::<sha2::Sha256>())
        })
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(scratch)?;
    budget.charge_work(
        bound
            .checked_mul(2)
            .and_then(|n| n.checked_add(128))
            .ok_or(Resource::Arithmetic)?,
    )?;
    // The fallible shared primitive is also used by the live semantic adapter.
    // Do not turn a target transcript allocation failure into its legacy expect.
    let identity = fe2o3_compiler_lineage::derive_semantic_target_layout_identity_v1(
        layout.llvm_target(),
        layout.data_layout(),
        layout.default_pointer_width_bits(),
        layout.active_cpu().unwrap_or_default(),
        layout.active_features().unwrap_or_default(),
    )
    .map_err(E::Target)?;
    let actual = fe2o3_mir_model::semantic_mir_v1::SemanticTargetDataLayoutV1::gfx942(
        fe2o3_mir_model::semantic_mir_v1::SemanticLayoutIdentityV1::from_sha256(identity.sha256()),
    );
    if semantic.target() != actual {
        return Err(E::Mismatch("original semantic/rustc target layout"));
    }
    budget.release_storage(scratch)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn encode_rows(
    roots: &[TypedDescriptorRootV1],
    semantic: &Semantic,
    output: &Owner,
    profile: Profile,
    pointer_width: u16,
    contracts: &[ConditionalInvocationContractV2<'_>],
    budget: &mut Budget<'_>,
) -> R<Output> {
    budget.charge_work(5)?;
    if roots.is_empty()
        || roots.len() > MAX_KERNELS
        || roots.len() != semantic.roots().len()
        || roots.len() != output.module().kernels.len()
        || roots.len() != contracts.len()
    {
        return Err(E::Mismatch(
            "complete conditional typed/source/F/contract roster",
        ));
    }
    check_output(roots, output.module(), profile, budget)?;
    nominal_v3::with_subject_rows(
        roots,
        semantic,
        output.module(),
        output.canonical().canonical_bytes(),
        profile,
        pointer_width,
        OUTPUT_DOMAIN,
        PRODUCER,
        budget,
        |nominal, budget| encode_v5(nominal, semantic, contracts, budget),
    )?
}

fn encode_v5(
    nominal: DeviceDescriptorTableInputV3<'_>,
    semantic: &Semantic,
    contracts: &[ConditionalInvocationContractV2<'_>],
    budget: &mut Budget<'_>,
) -> R<Output> {
    budget.reserve_storage(CONDITIONAL_INVOCATION_CODEC_STORAGE_V2)?;
    let mut ordered = nominal_v3::vector(contracts.len(), budget)?;
    for kernel in nominal.kernels {
        let mut selected = None;
        for contract in contracts {
            budget.charge_work(33)?;
            if &contract.subjects().kernel_id == kernel.kernel_id.as_bytes() {
                if selected.replace(contract).is_some() {
                    return Err(E::Mismatch("duplicate full conditional contract binding"));
                }
            }
        }
        let contract = selected.ok_or(E::Mismatch("missing full conditional contract binding"))?;
        budget.charge_work(33)?;
        if &contract.subjects().source_semantic_identity != semantic.semantic_sha256().as_bytes() {
            return Err(E::Mismatch("contract original semantic content"));
        }
        // Existing views are not Clone. Redecode borrowed bytes in canonical
        // order, without duplicating contract backing or inventing a key map.
        ordered.push(
            decode_conditional_invocation_contract_v2(contract.canonical_bytes(), &mut |w| {
                budget.charge_work(w)
            })
            .map_err(E::Contract)?,
        );
    }
    let input = DeviceDescriptorTableInputV5 {
        nominal,
        contracts: &ordered,
    };
    budget.reserve_storage(DESCRIPTOR_ENCODER_SCRATCH_STORAGE_V5)?;
    let length = encoded_device_descriptor_table_v5_len(&input, &mut |w| budget.charge_work(w))
        .map_err(E::Wire)?;
    let mut bytes = nominal_v3::vector::<u8>(length, budget)?;
    budget.charge_work(length)?;
    bytes.resize(length, 0);
    encode_device_descriptor_table_v5(&input, &mut bytes, &mut |w| budget.charge_work(w))
        .map_err(E::Wire)?;
    let retained = size_of::<Vec<u8>>()
        .checked_add(bytes.capacity())
        .ok_or(Resource::Arithmetic)?;
    Ok((bytes, ConditionalNativeDescriptorStorageV5(retained)))
}

fn check_output(
    roots: &[TypedDescriptorRootV1],
    module: &Module,
    profile: Profile,
    budget: &mut Budget<'_>,
) -> R<()> {
    require_target(&module.required_capabilities, profile, budget)?;
    for root in roots {
        let mut selected = None;
        for kernel in &module.kernels {
            budget.charge_work(
                root.entry_symbol()
                    .len()
                    .checked_add(kernel.id.as_str().len())
                    .and_then(|n| n.checked_add(1))
                    .ok_or(Resource::Arithmetic)?,
            )?;
            if kernel.id.as_str() == root.entry_symbol() {
                if selected.replace(kernel).is_some() {
                    return Err(E::Mismatch("unique F entry"));
                }
            }
        }
        let kernel = selected.ok_or(E::Mismatch("exact typed/F entry"))?;
        require_target(&kernel.required_capabilities, profile, budget)?;
        let launch = root
            .source_launch()
            .ok_or(E::Mismatch("original source launch"))?;
        budget.charge_work(
            kernel
                .entry
                .as_str()
                .len()
                .checked_add(root.entry_symbol().len())
                .and_then(|n| n.checked_add(8))
                .ok_or(Resource::Arithmetic)?,
        )?;
        if kernel.entry.as_str() != root.entry_symbol() || kernel.domain.rank() != launch.rank() {
            return Err(E::Mismatch("exact typed/F entry/rank"));
        }
        let fe2o3_artifacts::BlockSize::Exact(block) = launch.block_size() else {
            return Err(E::Mismatch("exact original source workgroup"));
        };
        if kernel.workgroup_size
            != Some(fe2o3_kernel_ir::WorkgroupSize::new(
                block.x(),
                block.y(),
                block.z(),
            ))
        {
            return Err(E::Mismatch("original source/F workgroup"));
        }
        let mut function = None;
        for candidate in &module.functions {
            budget.charge_work(
                candidate
                    .id
                    .as_str()
                    .len()
                    .checked_add(kernel.entry.as_str().len())
                    .and_then(|n| n.checked_add(1))
                    .ok_or(Resource::Arithmetic)?,
            )?;
            if candidate.id == kernel.entry {
                function = Some(candidate);
                break;
            }
        }
        let function = function.ok_or(E::Mismatch("actual F entry function"))?;
        require_target(&function.required_capabilities, profile, budget)?;
        if function.signature.parameters.len() != root.arguments.len()
            || !function.signature.results.is_empty()
        {
            return Err(E::Mismatch("flat original/F argument roster"));
        }
        for (argument, parameter) in root
            .arguments
            .as_slice()
            .iter()
            .zip(&function.signature.parameters)
        {
            budget.charge_work(8)?;
            let matches = match argument.kind {
                DescriptorArgumentKindV1::CompilerLaidOutUsize => {
                    argument.access == super::AccessMode::ByValue
                        && parameter == &Type::Scalar(ScalarType::U64)
                }
                DescriptorArgumentKindV1::CompilerLaidOutIsize => {
                    argument.access == super::AccessMode::ByValue
                        && parameter == &Type::Scalar(ScalarType::I64)
                }
                kind => production_descriptor_argument_matches_kernel_type_v1(
                    kind,
                    argument.access,
                    parameter,
                ),
            };
            if !matches {
                return Err(E::Mismatch("original nominal/F argument type/access"));
            }
        }
    }
    // Shared nominal projection checks supported capabilities. This additional
    // visit rejects conflicting exact target declarations at every retained site.
    for cap in &module.required_capabilities {
        check_cap(TargetCapabilityRefV1::from_owned(cap), profile, budget)?;
    }
    for kernel in &module.kernels {
        budget.charge_work(1)?;
        for cap in &kernel.required_capabilities {
            check_cap(TargetCapabilityRefV1::from_owned(cap), profile, budget)?;
        }
    }
    for function in &module.functions {
        budget.charge_work(1)?;
        for cap in &function.required_capabilities {
            check_cap(TargetCapabilityRefV1::from_owned(cap), profile, budget)?;
        }
        if let Some(body) = &function.body {
            for block in &body.blocks {
                budget.charge_work(1)?;
                for op in &block.operations {
                    budget.charge_work(
                        op.required_capability_visitation_work_v1()
                            .ok_or(Resource::Arithmetic)?,
                    )?;
                    op.try_visit_required_capabilities_v1(|cap| check_cap(cap, profile, budget))?;
                }
            }
        }
    }
    Ok(())
}

fn require_target(
    capabilities: &std::collections::BTreeSet<fe2o3_kernel_ir::TargetCapability>,
    profile: Profile,
    budget: &mut Budget<'_>,
) -> R<()> {
    let mut found = false;
    for capability in capabilities {
        let cap = TargetCapabilityRefV1::from_owned(capability);
        check_cap(cap, profile, budget)?;
        if let TargetCapabilityRefV1::Extension { namespace, name } = cap {
            if namespace == AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE
                && name.matches(profile.device_target())
            {
                found = true;
            }
        }
    }
    if !found {
        return Err(E::Mismatch("missing actual F target binding"));
    }
    Ok(())
}

fn check_cap(cap: TargetCapabilityRefV1<'_>, profile: Profile, budget: &mut Budget<'_>) -> R<()> {
    budget.charge_work(1)?;
    if let TargetCapabilityRefV1::Extension { namespace, name } = cap {
        budget.charge_work(
            namespace
                .len()
                .checked_add(name.visible_len().ok_or(Resource::Arithmetic)?)
                .and_then(|n| n.checked_add(profile.device_target().len() + 64))
                .ok_or(Resource::Arithmetic)?,
        )?;
        if (namespace == AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE
            && !name.matches(profile.device_target()))
            || (namespace == AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAMESPACE
                && name.matches(AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAME)
                && profile != Profile::Gfx942)
        {
            return Err(E::Mismatch("actual F target declaration"));
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "compiler_descriptor_conditional_native_v5_tests.rs"]
mod tests;
