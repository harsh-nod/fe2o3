//! Exact four-slot compiler ABI plus global-read/write condition preparation.
//! No condition below is a discharged host or runtime binding fact.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    FunctionOperationLocation as Location, Gfx942PhysicalEntrySourceSiteVNext as Site,
    Gfx942PhysicalGlobalCopyOpcodeV1 as Opcode, OperationKind as Op,
    PhysicalGlobalCopyKernargSlotV21 as Slot, ValueId,
};
use fe2o3_lower_mir_kernel::ProductionPhysicalGlobalCopyCheckedKirOwnerV21 as Checked;
#[path = "compiler_descriptor_physical_global_copy_abi_types_v21.rs"]
mod facts;
use facts::{Conditions, GlobalRead, GlobalStore, Read};

const WORK: usize = 32_768;
pub(crate) const UNRESOLVED_ABI_CONDITIONS_V21: &str = "full-EXEC input requires a readable and initialized 128-u32 prefix (512 bytes), even with empty output; output requires 512 writable bytes; input/output and output/kernarg disjointness remain runtime obligations; compiler kernarg requires a live readable immutable 32-byte prefix aligned to 8 bytes";

/// Move-only source/descriptor association. The fixed four rows and complete
/// conditional global access projection remain held until explicit inert export.
/// No protected/finalizer/native or host-admission API accepts this preparation.
pub(crate) struct PhysicalGlobalCopyPreparedAbiV21 {
    canonical: [u8; 32],
    semantic: [u8; 32],
    binding: [u8; 32],
    reads: [Option<Read>; 4],
    input_read: GlobalRead,
    output_store: GlobalStore,
    conditions: Conditions,
    retained: usize,
}
impl PhysicalGlobalCopyPreparedAbiV21 {
    pub(crate) const fn retained_storage(&self) -> usize {
        self.retained
    }
    pub(crate) const fn unresolved_conditions(&self) -> &'static str {
        UNRESOLVED_ABI_CONDITIONS_V21
    }
}
fn mismatch(detail: &'static str) -> CompilerDescriptorError {
    CompilerDescriptorError::ProductionDescriptorMismatch(detail)
}
fn resource(error: Resource) -> CompilerDescriptorError {
    CompilerDescriptorError::PhysicalGlobalCopyResourceV21(error)
}
// Keep this scope's resource mapping local: the existing V20 From<Resource>
// implementation must not relabel V21 denials or be duplicated.
struct ScopedError(CompilerDescriptorError);
impl From<Resource> for ScopedError {
    fn from(error: Resource) -> Self {
        Self(resource(error))
    }
}
impl From<CompilerDescriptorError> for ScopedError {
    fn from(error: CompilerDescriptorError) -> Self {
        Self(error)
    }
}
fn validate_source_and_slots<'a>(
    checked: &Checked,
    roots: &'a [TypedDescriptorRootV1],
) -> Result<&'a TypedDescriptorRootV1, CompilerDescriptorError> {
    let [root] = roots else {
        return Err(mismatch("global-copy ABI one typed source root"));
    };
    let semantic = checked.semantic_ssa().source_semantic();
    let [semantic_root] = semantic.roots() else {
        return Err(mismatch("global-copy ABI one semantic root"));
    };
    let function = semantic
        .functions()
        .get(semantic_root.index() as usize)
        .ok_or_else(|| mismatch("global-copy ABI actual semantic function"))?;
    let entry = function
        .kernel_entry()
        .ok_or_else(|| mismatch("global-copy ABI source entry"))?;
    let [kernel] = checked.executable().module().kernels.as_slice() else {
        return Err(mismatch("global-copy ABI one canonical kernel"));
    };
    let [launch] = checked.source_launch().roots() else {
        return Err(mismatch("global-copy ABI one source launch"));
    };
    if semantic.wire_version() != fe2o3_mir_model::semantic_mir_v1::SemanticMirWireVersionV1::V38
        || entry.kernel_binding_identity().as_bytes() != &root.kernel_binding_bytes()
        || launch.kernel_binding() != root.kernel_binding_bytes()
        || std::str::from_utf8(entry.export_symbol().as_bytes()).ok() != Some(root.entry_symbol())
        || kernel.id.as_str() != root.entry_symbol()
        || kernel.entry.as_str() != root.entry_symbol()
        || root.explicit_argument_bytes != 32
        || root.kernarg_alignment_bytes != 8
        || launch.source_launch().max_grid() != [2, 1, 1]
        || launch.layout().global_extents() != [128, 1, 1]
    {
        return Err(mismatch(
            "global-copy ABI exact source/canonical/descriptor association",
        ));
    }
    let source_launch = root
        .source_launch()
        .ok_or_else(|| mismatch("global-copy ABI retained validated source launch"))?;
    let source_block = match source_launch.block_size() {
        fe2o3_artifacts::BlockSize::Exact(block) => Some([block.x(), block.y(), block.z()]),
        fe2o3_artifacts::BlockSize::Any | fe2o3_artifacts::BlockSize::AtMost(_) => None,
    };
    let source_grid = source_launch.max_grid();
    if source_launch.rank() != launch.source_launch().rank()
        || source_block != launch.source_launch().exact_workgroup()
        || [source_grid.x(), source_grid.y(), source_grid.z()] != launch.source_launch().max_grid()
    {
        return Err(mismatch("global-copy ABI retained validated source launch"));
    }
    // This existing per-root validator joins real SharedBorrow/ExclusiveOwner
    // source evidence. It is not a runtime pointer/bounds/alias attestation.
    validate_production_v1_semantic_root_ownership_evidence(root, semantic, function)?;
    let [input, output] = root.arguments.as_slice() else {
        return Err(mismatch("global-copy ABI two logical arguments"));
    };
    for (argument, kind, access, offset, diagnostic) in [
        (
            input,
            DescriptorArgumentKindV1::SharedSlice(ScalarTypeV1::U32),
            AccessMode::ReadOnly,
            0,
            "global-copy ABI exact shared readonly input slice pair",
        ),
        (
            output,
            DescriptorArgumentKindV1::DisjointSlice(ScalarTypeV1::U32),
            AccessMode::ReadWrite,
            16,
            "global-copy ABI exact exclusive output slice pair",
        ),
    ] {
        if argument.kind != kind
            || argument.access != access
            || argument.offset != offset
            || argument.source_size != 16
            || argument.source_alignment != 8
            || argument.rustc_abi_class != RustcAbiClassV1::ScalarPair
        {
            return Err(mismatch(diagnostic));
        }
    }
    let report = checked.memory_obligations();
    let conditions = Conditions::from_report(report);
    let bounds = report.global().bounds_requirements();
    if report.canonical_identity() != checked.executable().identity().digest()
        || !conditions.is_exact_required_profile()
        || report.input_read().access().allocation() != conditions.input
        || report.output_store().access().allocation() != conditions.output
        || report.input_read().result() != report.output_store().value()
        || report.input_read().access().index() != report.output_store().access().index()
        || bounds.len() != 2
        || bounds.iter().any(|row| row.minimum_byte_len() != Some(512))
        || report.global().runtime_alias_requirements().len() != 1
        || report.global().runtime_alias_requirements()[0].left() != conditions.input
        || report.global().runtime_alias_requirements()[0].right() != conditions.output
        || bounds[0].allocation() != conditions.input
        || bounds[1].allocation() != conditions.output
        || report.grants_artifact_or_launch_authority()
        || report
            .runtime_requirements()
            .grants_runtime_binding_authority()
    {
        return Err(mismatch(
            "global-copy ABI complete global and kernarg conditions",
        ));
    }
    Ok(root)
}
fn validate_reads(
    checked: &Checked,
    rows: &[Option<Read>; 4],
) -> Result<(), CompilerDescriptorError> {
    let expected = checked.memory_obligations().kernarg_reads();
    if rows
        .iter()
        .copied()
        .ne(expected.iter().copied().map(|row| Some(Read::from(row))))
    {
        return Err(mismatch("global-copy ABI exact ordered read report"));
    }
    let [function] = checked.executable().module().functions.as_slice() else {
        return Err(mismatch("global-copy ABI one function"));
    };
    let body = function
        .body
        .as_ref()
        .ok_or_else(|| mismatch("global-copy ABI body"))?;
    let mut cursor = 0;
    for block in &body.blocks {
        for (index, operation) in block.operations.iter().enumerate() {
            let Op::Gfx942PhysicalGlobalCopyStep(step) = operation.kind else {
                continue;
            };
            if step.instruction.opcode != Opcode::LoadKernargPair {
                continue;
            }
            let row = rows
                .get(cursor)
                .copied()
                .flatten()
                .ok_or_else(|| mismatch("global-copy ABI missing authored read"))?;
            cursor += 1;
            let slot = match step.instruction.immediate {
                0 => Slot::InputPointer,
                8 => Slot::InputLength,
                16 => Slot::OutputPointer,
                24 => Slot::OutputLength,
                _ => return Err(mismatch("global-copy ABI read outside exact slots")),
            };
            if row.location != Location::new(block.id, index)
                || row.site != step.site
                || row.offset != step.instruction.immediate
                || row.width != 8
                || row.alignment != 8
                || row.slot != slot
                || row.base.map(Some) != [step.operands[0], step.operands[1]]
                || row
                    .results
                    .into_iter()
                    .ne(operation.results.iter().map(|result| result.id))
            {
                return Err(mismatch(
                    "global-copy ABI authored read operand or slot join",
                ));
            }
            let wait = block.operations[index + 1..]
                .iter()
                .enumerate()
                .find_map(|(relative, op)| match op.kind {
                    Op::Gfx942PhysicalGlobalCopyStep(s)
                        if s.instruction.opcode == Opcode::WaitLgkm0 =>
                    {
                        Some((Location::new(block.id, index + 1 + relative), s.site))
                    }
                    _ => None,
                })
                .ok_or_else(|| mismatch("global-copy ABI authored read readiness"))?;
            if (row.ready_at, row.ready_site) != wait {
                return Err(mismatch("global-copy ABI exact explicit wait occurrence"));
            }
        }
    }
    if cursor != 4 {
        return Err(mismatch("global-copy ABI exact four authored reads"));
    }
    Ok(())
}
pub(crate) fn prepare_physical_global_copy_abi_v21(
    checked: &Checked,
    roots: &[TypedDescriptorRootV1],
    budget: &mut Budget<'_>,
) -> Result<PhysicalGlobalCopyPreparedAbiV21, CompilerDescriptorError> {
    let retained = std::mem::size_of::<PhysicalGlobalCopyPreparedAbiV21>();
    // Fixed arrays need no additional heap allocation; this receipt retains
    // all four read rows, both global records and all unresolved conditions.
    budget
        .with_prepaid_scope::<_, ScopedError>(checked.retained_storage(), 1, WORK, retained, |_| {
            let root = validate_source_and_slots(checked, roots)?;
            let report = checked.memory_obligations();
            let reads = (*report.kernarg_reads()).map(|row| Some(Read::from(row)));
            validate_reads(checked, &reads)?;
            Ok(PhysicalGlobalCopyPreparedAbiV21 {
                canonical: *checked.executable().identity().digest(),
                semantic: *checked
                    .semantic_ssa()
                    .source_semantic()
                    .semantic_sha256()
                    .as_bytes(),
                binding: root.kernel_binding_bytes(),
                reads,
                input_read: report.input_read().into(),
                output_store: report.output_store().into(),
                conditions: Conditions::from_report(report),
                retained,
            })
        })
        .map_err(|error| error.0)
}
pub(super) fn validate_prepared_abi_v21(
    checked: &Checked,
    roots: &[TypedDescriptorRootV1],
    abi: &PhysicalGlobalCopyPreparedAbiV21,
    budget: &mut Budget<'_>,
) -> Result<(), CompilerDescriptorError> {
    budget.charge_work(1).map_err(resource)?;
    let retained = std::mem::size_of::<PhysicalGlobalCopyPreparedAbiV21>();
    let required_floor = checked
        .retained_storage()
        .checked_add(retained)
        .ok_or_else(|| resource(Resource::Arithmetic))?;
    if abi.retained != retained || budget.storage() < required_floor {
        return Err(resource(Resource::Accounting));
    }
    budget.charge_work(WORK - 1).map_err(resource)?;
    let root = validate_source_and_slots(checked, roots)?;
    let report = checked.memory_obligations();
    if abi.canonical != *checked.executable().identity().digest()
        || abi.semantic
            != *checked
                .semantic_ssa()
                .source_semantic()
                .semantic_sha256()
                .as_bytes()
        || abi.binding != root.kernel_binding_bytes()
    {
        return Err(mismatch("global-copy ABI retained source identity"));
    }
    if abi.conditions != Conditions::from_report(report)
        || !abi.conditions.is_exact_required_profile()
    {
        return Err(mismatch("global-copy ABI retained unresolved conditions"));
    }
    if abi.input_read != GlobalRead::from(report.input_read())
        || abi.output_store != GlobalStore::from(report.output_store())
    {
        return Err(mismatch(
            "global-copy ABI exact global read/write readiness and SSA report",
        ));
    }
    validate_reads(checked, &abi.reads)
}
#[cfg(test)]
#[path = "compiler_descriptor_physical_global_copy_abi_v21_tests.rs"]
mod tests;
#[cfg(test)]
pub(crate) use tests::qualify_actual_owner_abi_controls_v21;
