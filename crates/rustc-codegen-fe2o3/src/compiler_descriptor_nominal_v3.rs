//! Actual typed-source V3 ABI production; no native or publication authority.
//!
//! This intermediate route refuses target-sensitive requirements without an
//! existing geometry derivation. It does not weaken the legacy native route.
#![allow(
    clippy::result_large_err,
    reason = "Keep exact typed child errors inline without unmetered error allocation."
)]

use super::*;
use fe2o3_kernel_descriptor::{
    AliasSemantics, AtomicRequirementsV2, DESCRIPTOR_ENCODER_SCRATCH_STORAGE_V3,
    DESCRIPTOR_QUERY_STORAGE_V3, DeviceDescriptorTableInputV3, DeviceTargetV1,
    KernelDescriptorInputV3, KernelTargetRequirementsV2, LdsRequirementsV2, LogicalArgumentInputV3,
    OwnershipSemantics, PhysicalAbiComponentKind, PhysicalComponentV3, RequiredWavefrontWidthV2,
    SourceTypeDescriptorV3, SourceTypeRecordV3, SynchronizationRequirementsV2,
    device_layout_record_v3, encode_device_descriptor_table_v3,
    encoded_device_descriptor_table_v3_len,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, TargetCapabilityRefV1,
};
use fe2o3_lower_mir_kernel::ProductionFormalMemoryOwnerV1;
use fe2o3_mir_model::semantic_mir_v1::SemanticRustTypeKindV1;
use std::{
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind},
};

#[derive(Debug)]
pub(crate) enum NominalDescriptorErrorV3 {
    Resource(Resource),
    Descriptor(CompilerDescriptorError),
    Validation(ValidationError),
    Wire(fe2o3_kernel_descriptor::DescriptorWireErrorV3<Resource>),
    Pipeline(crate::production_pipeline::ProductionPipelineError),
    Agreement(fe2o3_verifier::NominalSourceAbiErrorV3),
    UnsupportedRequirements,
    Mismatch(&'static str),
    Panicked,
}
type E = NominalDescriptorErrorV3;
type R<T> = Result<T, E>;
impl From<Resource> for E {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl fmt::Display for E {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Mismatch(message) => write!(f, "nominal source descriptor: {message}"),
            _ => write!(f, "nominal source descriptor: {self:?}"),
        }
    }
}
impl std::error::Error for E {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::Descriptor(error) => Some(error),
            Self::Validation(error) => Some(error),
            Self::Wire(error) => Some(error),
            Self::Pipeline(error) => Some(error),
            Self::Agreement(error) => Some(error),
            Self::UnsupportedRequirements | Self::Mismatch(_) | Self::Panicked => None,
        }
    }
}

/// The original collection and formal engines retain their existing bounded
/// domains. All new row, string and output backing is charged here.
pub(crate) fn scoped<'w, T>(
    budget: &mut Budget<'w>,
    run: impl FnOnce(&mut Budget<'w>) -> R<T>,
) -> R<T> {
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let slot = budget as *const Budget<'w> as usize;
    let mut payloads = [None, None];
    let mut result = match catch_unwind(AssertUnwindSafe(|| run(budget))) {
        Ok(result) => result,
        Err(payload) => {
            payloads[0] = Some(payload);
            Err(E::Panicked)
        }
    };
    let valid = budget.work_ledger_identity_v1() == ledger
        && budget as *const Budget<'w> as usize == slot
        && budget.storage() >= floor;
    if !valid {
        let rejected = std::mem::replace(&mut result, Err(Resource::Accounting.into()));
        payloads[1] = catch_unwind(AssertUnwindSafe(|| drop(rejected))).err();
    }
    if valid {
        budget.release_storage(budget.storage() - floor)?;
    }
    drop(payloads);
    result
}

pub(crate) fn vector<T>(count: usize, budget: &mut Budget<'_>) -> R<Vec<T>> {
    let requested = count
        .checked_mul(size_of::<T>())
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(
        requested
            .checked_add(size_of::<Vec<T>>())
            .ok_or(Resource::Arithmetic)?,
    )?;
    budget.charge_work(1)?;
    let mut result = Vec::new();
    if result.try_reserve_exact(count).is_err() {
        return Err(Resource::Allocation.into());
    }
    let actual = result
        .capacity()
        .checked_mul(size_of::<T>())
        .ok_or(Resource::Arithmetic)?;
    if actual < requested {
        return Err(Resource::Accounting.into());
    }
    if let Err(error) = budget.reserve_storage(actual - requested) {
        drop(result);
        return Err(error.into());
    }
    Ok(result)
}

fn text(value: &str, budget: &mut Budget<'_>) -> R<Text> {
    let mut bytes = vector::<u8>(value.len(), budget)?;
    budget.charge_work(value.len())?;
    bytes.extend_from_slice(value.as_bytes());
    let string = String::from_utf8(bytes).map_err(|_| E::Mismatch("UTF-8 metadata"))?;
    Text::new(string).map_err(E::Validation)
}

fn records(
    kind: DescriptorArgumentKindV1,
    budget: &mut Budget<'_>,
) -> R<(SourceTypeRecordV3, DeviceLayoutRecordV1)> {
    let (source, layout) = match kind {
        DescriptorArgumentKindV1::Scalar(s) => (
            SourceTypeDescriptorV3::Scalar(s),
            DeviceLayoutDescriptorV1::scalar(s),
        ),
        DescriptorArgumentKindV1::SharedSlice(s) => (
            SourceTypeDescriptorV3::SharedSlice(s),
            DeviceLayoutDescriptorV1::shared_slice(s),
        ),
        DescriptorArgumentKindV1::DisjointSlice(s) => (
            SourceTypeDescriptorV3::DisjointSlice(s),
            DeviceLayoutDescriptorV1::disjoint_slice(s),
        ),
        DescriptorArgumentKindV1::GlobalMutPointer(s) => (
            SourceTypeDescriptorV3::GlobalMutPointer(s),
            DeviceLayoutDescriptorV1::global_mut_pointer(s),
        ),
        DescriptorArgumentKindV1::CompilerLaidOutUsize => (
            SourceTypeDescriptorV3::Usize,
            DeviceLayoutDescriptorV1::scalar(ScalarTypeV1::U64),
        ),
        DescriptorArgumentKindV1::CompilerLaidOutIsize => (
            SourceTypeDescriptorV3::Isize,
            DeviceLayoutDescriptorV1::scalar(ScalarTypeV1::I64),
        ),
        DescriptorArgumentKindV1::CompilerLaidOutByValue => {
            return Err(E::Mismatch("aggregate packing remains deferred"));
        }
    };
    Ok((
        SourceTypeRecordV3::new(source, &mut |w| budget.charge_work(w)).map_err(E::Wire)?,
        device_layout_record_v3(layout, &mut |w| budget.charge_work(w)).map_err(E::Wire)?,
    ))
}

fn nominal_kind_matches(kind: DescriptorArgumentKindV1, rust: SemanticRustTypeKindV1) -> bool {
    match kind {
        DescriptorArgumentKindV1::CompilerLaidOutUsize => rust == SemanticRustTypeKindV1::Usize,
        DescriptorArgumentKindV1::CompilerLaidOutIsize => rust == SemanticRustTypeKindV1::Isize,
        DescriptorArgumentKindV1::CompilerLaidOutByValue => false,
        _ => !matches!(
            rust,
            SemanticRustTypeKindV1::Usize | SemanticRustTypeKindV1::Isize
        ),
    }
}

fn components(
    argument: &TypedDescriptorArgumentV1,
) -> R<(
    [PhysicalComponentV3; 2],
    usize,
    OwnershipSemantics,
    AliasSemantics,
)> {
    let scalar = matches!(
        argument.kind,
        DescriptorArgumentKindV1::Scalar(_)
            | DescriptorArgumentKindV1::CompilerLaidOutUsize
            | DescriptorArgumentKindV1::CompilerLaidOutIsize
    );
    let shared = matches!(argument.kind, DescriptorArgumentKindV1::SharedSlice(_));
    let slice = shared || matches!(argument.kind, DescriptorArgumentKindV1::DisjointSlice(_));
    let ownership = if scalar {
        OwnershipSemantics::ByValue
    } else if shared {
        OwnershipSemantics::SharedBorrow
    } else {
        OwnershipSemantics::UniqueBorrow
    };
    let alias = if scalar {
        AliasSemantics::Value
    } else if shared {
        AliasSemantics::SharedReadOnly
    } else {
        AliasSemantics::Exclusive
    };
    let scalar_type = match argument.kind {
        DescriptorArgumentKindV1::Scalar(s) => s,
        DescriptorArgumentKindV1::CompilerLaidOutUsize => ScalarTypeV1::U64,
        DescriptorArgumentKindV1::CompilerLaidOutIsize => ScalarTypeV1::I64,
        _ => ScalarTypeV1::U64,
    };
    let size = if scalar {
        u16::try_from(argument.source_size).map_err(|_| E::Mismatch("scalar size"))?
    } else {
        8
    };
    let alignment = if scalar {
        u16::try_from(argument.source_alignment).map_err(|_| E::Mismatch("scalar alignment"))?
    } else {
        8
    };
    let first = PhysicalComponentV3 {
        kind: if scalar {
            PhysicalAbiComponentKind::ScalarByValue(scalar_type)
        } else {
            PhysicalAbiComponentKind::GlobalPointer
        },
        offset: argument.offset,
        size,
        alignment,
        access: argument.access,
        alias,
    };
    let second = PhysicalComponentV3 {
        kind: PhysicalAbiComponentKind::SliceLengthU64,
        offset: argument.offset.checked_add(8).ok_or(Resource::Arithmetic)?,
        size: 8,
        alignment: 8,
        access: AccessMode::ByValue,
        alias: AliasSemantics::Value,
    };
    Ok(([first, second], if slice { 2 } else { 1 }, ownership, alias))
}

fn inert_capabilities(
    module: &Module,
    has_exact_diagnostic_target: bool,
    budget: &mut Budget<'_>,
) -> R<([CapabilityV1; 2], usize)> {
    // This source ABI route has no native geometry/atomic-scope proof. Never
    // convert a deferred requirement to an empty capability list.
    let mut subgroup = false;
    fn visit(
        capability: TargetCapabilityRefV1<'_>,
        has_exact_diagnostic_target: bool,
        budget: &mut Budget<'_>,
        subgroup: &mut bool,
    ) -> R<()> {
        budget.charge_work(1)?;
        let set = dialect_amdgcn::project_descriptor_capability_v1(
            capability,
            false,
            false,
            has_exact_diagnostic_target,
        )
        .ok_or(E::UnsupportedRequirements)?;
        for cap in set.iter() {
            budget.charge_work(1)?;
            match cap {
                CapabilityV1::Subgroup => *subgroup = true,
                CapabilityV1::AmdWave => {}
                _ => return Err(E::UnsupportedRequirements),
            }
        }
        Ok(())
    }
    for cap in &module.required_capabilities {
        visit(
            TargetCapabilityRefV1::from_owned(cap),
            has_exact_diagnostic_target,
            budget,
            &mut subgroup,
        )?;
    }
    for kernel in &module.kernels {
        budget.charge_work(1)?;
        for cap in &kernel.required_capabilities {
            visit(
                TargetCapabilityRefV1::from_owned(cap),
                has_exact_diagnostic_target,
                budget,
                &mut subgroup,
            )?;
        }
    }
    for function in &module.functions {
        budget.charge_work(1)?;
        for cap in &function.required_capabilities {
            visit(
                TargetCapabilityRefV1::from_owned(cap),
                has_exact_diagnostic_target,
                budget,
                &mut subgroup,
            )?;
        }
        if let Some(body) = &function.body {
            for block in &body.blocks {
                budget.charge_work(1)?;
                for operation in &block.operations {
                    budget.charge_work(
                        operation
                            .required_capability_visitation_work_v1()
                            .ok_or(Resource::Arithmetic)?,
                    )?;
                    if matches!(
                        operation.kind,
                        fe2o3_kernel_ir::OperationKind::Atomic(_)
                            | fe2o3_kernel_ir::OperationKind::Barrier(_)
                            | fe2o3_kernel_ir::OperationKind::Fence(_)
                            | fe2o3_kernel_ir::OperationKind::WorkgroupBarrier(_)
                    ) {
                        return Err(E::UnsupportedRequirements);
                    }
                    operation.try_visit_required_capabilities_v1(|capability| {
                        visit(
                            capability,
                            has_exact_diagnostic_target,
                            budget,
                            &mut subgroup,
                        )
                    })?;
                }
            }
        }
    }
    Ok(if subgroup {
        ([CapabilityV1::Subgroup, CapabilityV1::AmdWave], 2)
    } else {
        ([CapabilityV1::AmdWave; 2], 1)
    })
}

fn launch(source: &LaunchContract, budget: &mut Budget<'_>) -> R<LaunchConstraintsV1> {
    budget.charge_work(8)?;
    if source.static_shared_memory_bytes() != 0 || source.max_dynamic_shared_memory_bytes() != 0 {
        return Err(E::UnsupportedRequirements);
    }
    let fe2o3_artifacts::BlockSize::Exact(block) = source.block_size() else {
        return Err(E::Mismatch("exact source block"));
    };
    let grid = source.max_grid();
    let flat = block
        .x()
        .checked_mul(block.y())
        .and_then(|v| v.checked_mul(block.z()))
        .ok_or(Resource::Arithmetic)?;
    LaunchConstraintsV1::new(
        source.rank(),
        BlockSizeV1::Exact(
            DimensionsV1::new(block.x(), block.y(), block.z()).map_err(E::Validation)?,
        ),
        DimensionsV1::new(grid.x(), grid.y(), grid.z()).map_err(E::Validation)?,
        flat,
        0,
        0,
    )
    .map_err(E::Validation)
}

struct ArgumentRow {
    source: SourceTypeRecordV3,
    layout: DeviceLayoutRecordV1,
    components: [PhysicalComponentV3; 2],
    count: usize,
    ownership: OwnershipSemantics,
    alias: AliasSemantics,
}
struct RootRow<'a> {
    root: &'a TypedDescriptorRootV1,
    symbol: Vec<u8>,
    launch: LaunchConstraintsV1,
    start: usize,
    end: usize,
    source: BuildEvidenceV1,
    ir: BuildEvidenceV1,
}

fn evidence(
    domain: &[u8],
    binding: &[u8; 32],
    bytes: &[u8],
    budget: &mut Budget<'_>,
) -> R<BuildEvidenceV1> {
    budget.charge_work(
        domain
            .len()
            // Two frames: count + two lengths + binding. As in the FFI
            // content-identity hash, 128 additional units cover finalization.
            .checked_add(56 + 128)
            .and_then(|v| v.checked_add(bytes.len()))
            .ok_or(Resource::Arithmetic)?,
    )?;
    let digest = domain_hash(domain, &[binding, bytes]);
    Ok(BuildEvidenceV1::new(
        EvidenceIdentity::from_opaque_bytes(digest),
        EvidenceDigest::from_sha256_bytes(digest),
    ))
}

/// Produces a complete unfinalized table from retained compiler owners only.
/// Metadata is content evidence, not a signed/native receipt. The caller owns
/// and keeps the returned Vec backing paid until its enclosing owner is dropped.
pub(crate) fn produce(
    roots: &[TypedDescriptorRootV1],
    formal: &ProductionFormalMemoryOwnerV1,
    target: &crate::production_target_v1::AuthenticatedProductionTargetV1,
    budget: &mut Budget<'_>,
) -> R<Vec<u8>> {
    scoped(budget, |budget| {
        budget.charge_work(5)?;
        let source = formal.semantic_kir();
        let semantic = source.semantic().semantic();
        let module = source.module();
        if roots.is_empty()
            || roots.len() > fe2o3_kernel_descriptor::MAX_KERNELS
            || roots.len() != semantic.roots().len()
            || roots.len() != module.kernels.len()
            || roots.len() != formal.kernels().len()
        {
            return Err(E::Mismatch("complete typed/source/formal root roster"));
        }
        if target.rustc_layout().default_pointer_width_bits() != 64 {
            return Err(E::Mismatch("retained 64-bit rustc target"));
        }
        budget.reserve_storage(
            DESCRIPTOR_QUERY_STORAGE_V3
                .checked_add(size_of::<CompilerIdentityV1>())
                .and_then(|v| v.checked_add(size_of::<ProducerIdentityV1>()))
                .and_then(|v| v.checked_add(size_of::<DeviceDescriptorTableInputV3<'_>>()))
                .and_then(|v| v.checked_add(size_of::<Sha256>()))
                .and_then(|v| v.checked_add(size_of::<[&[u8]; 2]>() + size_of::<[u8; 32]>()))
                .and_then(|v| {
                    v.checked_add(size_of::<fe2o3_artifacts::RustNominalScalarEvidenceV3>())
                })
                .and_then(|v| v.checked_add(size_of::<ArgumentRow>()))
                .and_then(|v| v.checked_add(size_of::<RootRow<'_>>()))
                .and_then(|v| v.checked_add(size_of::<([CapabilityV1; 2], usize)>()))
                .and_then(|v| v.checked_add(size_of::<[bool; 2]>()))
                .ok_or(Resource::Arithmetic)?,
        )?;
        let mut count = 0usize;
        for root in roots {
            budget.charge_work(1)?;
            if root.arguments.len() > fe2o3_kernel_descriptor::MAX_ARGUMENTS_PER_KERNEL {
                return Err(E::Mismatch("bounded whole-root arguments"));
            }
            count = count
                .checked_add(root.arguments.len())
                .ok_or(Resource::Arithmetic)?;
        }
        let mut ordered = vector::<&TypedDescriptorRootV1>(roots.len(), budget)?;
        budget.charge_work(roots.len())?;
        ordered.extend(roots);
        // Fixed-key sorting is prepaid conservatively without a fallible comparator.
        budget.charge_work(
            roots
                .len()
                .checked_mul(roots.len())
                .and_then(|v| v.checked_mul(32))
                .ok_or(Resource::Arithmetic)?,
        )?;
        ordered.sort_unstable_by_key(|root| root.kernel_binding_bytes());
        if ordered
            .windows(2)
            .any(|rows| rows[0].kernel_binding_bytes() == rows[1].kernel_binding_bytes())
        {
            return Err(E::Mismatch("unique complete typed bindings"));
        }
        let mut argument_rows = vector::<ArgumentRow>(count, budget)?;
        let mut root_rows = vector::<RootRow<'_>>(roots.len(), budget)?;
        for root in ordered {
            budget.charge_work(
                root.arguments
                    .len()
                    .checked_mul(32)
                    .and_then(|v| v.checked_add(root.export_name.len()))
                    .ok_or(Resource::Arithmetic)?,
            )?;
            if root.arguments.len() != 0 {
                super::laid_out_plan_v1::check(root).map_err(E::Descriptor)?;
            }
            let mut matched = None;
            for semantic_root in semantic.roots() {
                budget.charge_work(2)?;
                let function = semantic
                    .functions()
                    .get(semantic_root.index() as usize)
                    .ok_or(E::Mismatch("semantic root function"))?;
                if function.kernel_entry().is_some_and(|entry| {
                    entry.kernel_binding_identity().as_bytes() == &root.kernel_binding_bytes()
                }) {
                    if matched.replace(function).is_some() {
                        return Err(E::Mismatch("unique source binding"));
                    }
                }
            }
            let function = matched.ok_or(E::Mismatch("exact typed/source binding"))?;
            validate_production_v1_semantic_root_ownership_evidence(root, semantic, function)
                .map_err(E::Descriptor)?;
            let start = argument_rows.len();
            for (argument, source_type) in root
                .arguments
                .as_slice()
                .iter()
                .zip(function.abi().source_input_types())
            {
                budget.charge_work(
                    argument
                        .name
                        .len()
                        .checked_add(16)
                        .ok_or(Resource::Arithmetic)?,
                )?;
                let ty = semantic
                    .types()
                    .get(source_type.index() as usize)
                    .ok_or(E::Mismatch("source ABI type"))?;
                if !nominal_kind_matches(argument.kind, ty.rust_type_kind()) {
                    return Err(E::Mismatch("actual rustc nominal kind"));
                }
                if matches!(
                    argument.kind,
                    DescriptorArgumentKindV1::CompilerLaidOutUsize
                        | DescriptorArgumentKindV1::CompilerLaidOutIsize
                ) {
                    let kind = if argument.kind == DescriptorArgumentKindV1::CompilerLaidOutUsize {
                        fe2o3_artifacts::RustNominalScalarKindV3::Usize
                    } else {
                        fe2o3_artifacts::RustNominalScalarKindV3::Isize
                    };
                    let evidence = fe2o3_artifacts::RustNominalScalarEvidenceV3::new(
                        kind,
                        fe2o3_artifacts::PointerWidth::Bits64,
                    )
                    .map_err(|_| E::Mismatch("nominal portable layout"))?;
                    if argument.source_size != evidence.size()
                        || argument.source_alignment != evidence.abi_alignment()
                        || argument.rustc_abi_class != evidence.abi_class()
                        || argument.layout.is_some()
                    {
                        return Err(E::Mismatch("nominal/physical rustc layout"));
                    }
                }
                let (source, layout) = records(argument.kind, budget)?;
                let (components, count, ownership, alias) = components(argument)?;
                argument_rows.push(ArgumentRow {
                    source,
                    layout,
                    components,
                    count,
                    ownership,
                    alias,
                });
            }
            let length = root
                .export_name
                .len()
                .checked_add(3)
                .ok_or(Resource::Arithmetic)?;
            let mut symbol = vector::<u8>(length, budget)?;
            budget.charge_work(length)?;
            symbol.extend_from_slice(root.export_name.as_bytes());
            symbol.extend_from_slice(b".kd");
            let launch = launch(
                root.source_launch()
                    .ok_or(E::Mismatch("retained source launch"))?,
                budget,
            )?;
            let binding = root.kernel_binding_bytes();
            let source_evidence = evidence(
                b"FE2O3/NOMINAL-SOURCE-ABI/V3\0",
                &binding,
                semantic.canonical_encoding(),
                budget,
            )?;
            let ir = evidence(
                b"FE2O3/NOMINAL-EXECUTABLE-ABI/V3\0",
                &binding,
                source.canonical_kernel_ir_bytes(),
                budget,
            )?;
            root_rows.push(RootRow {
                root,
                symbol,
                launch,
                start,
                end: argument_rows.len(),
                source: source_evidence,
                ir,
            });
        }
        let mut sources = vector::<SourceTypeRecordV3>(count, budget)?;
        let mut layouts = vector::<DeviceLayoutRecordV1>(count, budget)?;
        budget.charge_work(count)?;
        for row in &argument_rows {
            sources.push(row.source);
            layouts.push(row.layout.clone());
        }
        budget.charge_work(
            count
                .checked_mul(count)
                .and_then(|v| v.checked_mul(64))
                .ok_or(Resource::Arithmetic)?,
        )?;
        sources.sort_unstable_by_key(|row| row.identity());
        sources.dedup_by_key(|row| row.identity());
        layouts.sort_unstable_by_key(DeviceLayoutRecordV1::identity);
        layouts.dedup_by_key(|row| row.identity());
        let mut arguments = vector::<LogicalArgumentInputV3<'_>>(count, budget)?;
        for root in &root_rows {
            for (index, (argument, row)) in root
                .root
                .arguments
                .as_slice()
                .iter()
                .zip(&argument_rows[root.start..root.end])
                .enumerate()
            {
                budget.charge_work(1)?;
                arguments.push(LogicalArgumentInputV3 {
                    source_index: u16::try_from(index).map_err(|_| Resource::Arithmetic)?,
                    name: &argument.name,
                    source_type: row.source.identity(),
                    device_layout: row.layout.identity(),
                    ownership: row.ownership,
                    access: argument.access,
                    alias: row.alias,
                    components: &row.components[..row.count],
                });
            }
        }
        // Covers text comparisons in the allocation-free borrowed capability visitor.
        budget.charge_work(source.canonical_kernel_ir_bytes().len())?;
        // Use the retained live rustc target, never an unverified capability tag.
        budget.charge_work(1)?;
        let has_exact_diagnostic_target = match target.profile() {
            fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942
            | fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx950 => true,
        };
        let (capabilities, capability_count) =
            inert_capabilities(module, has_exact_diagnostic_target, budget)?;
        let mut kernels = vector::<KernelDescriptorInputV3<'_>>(roots.len(), budget)?;
        let mut requirements = vector::<KernelTargetRequirementsV2>(roots.len(), budget)?;
        for row in &root_rows {
            budget.charge_work(8)?;
            let root = row.root;
            let id = KernelId::from_bytes(root.kernel_binding_bytes());
            kernels.push(KernelDescriptorInputV3 {
                kernel_id: id,
                logical_name: &root.logical_name,
                entry_name: &root.export_name,
                descriptor_symbol: std::str::from_utf8(&row.symbol)
                    .map_err(|_| E::Mismatch("descriptor symbol"))?,
                source_evidence: row.source,
                executable_ir_evidence: row.ir,
                capabilities: &capabilities[..capability_count],
                abi_layout: KernelAbiLayoutV1::new(
                    root.explicit_argument_bytes,
                    root.explicit_argument_bytes
                        .checked_add(256)
                        .ok_or(Resource::Arithmetic)?,
                    root.kernarg_alignment_bytes,
                )
                .map_err(E::Validation)?,
                launch: &row.launch,
                arguments: &arguments[row.start..row.end],
            });
            requirements.push(KernelTargetRequirementsV2::new(
                id,
                LdsRequirementsV2::new(0, 0).map_err(E::Validation)?,
                RequiredWavefrontWidthV2::Wave64,
                false,
                SynchronizationRequirementsV2::empty(),
                AtomicRequirementsV2::empty(),
            ));
        }
        let compiler = CompilerIdentityV1::new(
            text(RUSTC_CODEGEN_FE2O3_COMPILER_NAME_V1, budget)?,
            text(env!("CARGO_PKG_VERSION"), budget)?,
            [0; 20],
        );
        let producer = ProducerIdentityV1::new(
            text(RUSTC_CODEGEN_FE2O3_PRODUCTION_V3_PRODUCER_NAME_V1, budget)?,
            text("inert-nominal-source-abi-v3", budget)?,
        );
        let target_name = target.profile().device_target();
        budget.charge_work(target_name.len())?;
        let device_target = DeviceTargetV1::new(
            fe2o3_amd_target::AmdTargetId::parse(target_name)
                .map_err(|_| E::Mismatch("retained target profile"))?,
        );
        let input = DeviceDescriptorTableInputV3 {
            canonical_code_object_digest: CanonicalCodeObjectDigest::from_bytes([0; 32]),
            code_object_version: CodeObjectVersion::V6,
            compiler: &compiler,
            producer: &producer,
            device_target,
            type_records: &sources,
            layout_records: &layouts,
            kernels: &kernels,
            requirements: &requirements,
        };
        budget.reserve_storage(DESCRIPTOR_ENCODER_SCRATCH_STORAGE_V3)?;
        let length = encoded_device_descriptor_table_v3_len(&input, &mut |w| budget.charge_work(w))
            .map_err(E::Wire)?;
        let mut wire = vector::<u8>(length, budget)?;
        budget.charge_work(length)?;
        wire.resize(length, 0);
        encode_device_descriptor_table_v3(&input, &mut wire, &mut |w| budget.charge_work(w))
            .map_err(E::Wire)?;
        Ok(wire)
    })
}

#[cfg(test)]
#[path = "production_nominal_abi_v3_tests.rs"]
mod tests;
