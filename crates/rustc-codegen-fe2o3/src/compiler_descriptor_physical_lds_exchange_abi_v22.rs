//! Exact four-slot compiler ABI plus global-read/write condition preparation.
//! No condition below is a discharged host or runtime binding fact.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    FunctionOperationLocation as Location, Gfx942PhysicalEntrySourceSiteVNext as Site,
    Gfx942PhysicalLdsExchangeOpcodeV1 as Opcode, OperationKind as Op,
    PhysicalLdsExchangeKernargSlotV22 as Slot, ValueId,
};
use fe2o3_lower_mir_kernel::ProductionPhysicalLdsExchangeCheckedKirOwnerV22 as Checked;
#[path = "compiler_descriptor_physical_lds_exchange_abi_types_v22.rs"]
mod facts;
use facts::{Conditions, GlobalRead, GlobalStore, Read};
#[path = "compiler_descriptor_physical_lds_exchange_lds_v22.rs"]
mod lds_facts;
use lds_facts::Lds;

const WORK: usize = 32_768;
pub(crate) const UNRESOLVED_ABI_CONDITIONS_V22: &str = "full-EXEC input requires a readable and initialized 128-u32 prefix (512 bytes), even with empty output; output requires 512 writable bytes; input/output and output/kernarg disjointness remain runtime obligations; compiler kernarg requires a live readable immutable 32-byte prefix aligned to 8 bytes; one complete 128-invocation workgroup must execute the declared 512-byte LDS frame with explicit write completion, workgroup publication and read completion; no global happens-before or host admission is granted";

/// Move-only source/descriptor association. The fixed four rows and complete
/// conditional global access projection remain held until explicit inert export.
/// No protected/finalizer/native or host-admission API accepts this preparation.
pub(crate) struct PhysicalLdsExchangePreparedAbiV22 {
    canonical: [u8; 32],
    semantic: [u8; 32],
    binding: [u8; 32],
    reads: [Option<Read>; 4],
    input_read: GlobalRead,
    output_store: GlobalStore,
    conditions: Conditions,
    lds: Lds,
    retained: usize,
}
impl PhysicalLdsExchangePreparedAbiV22 {
    pub(crate) const fn retained_storage(&self) -> usize {
        self.retained
    }
    pub(crate) const fn lds_frame(&self) -> fe2o3_kernel_ir::Gfx942PhysicalLdsExchangeFrameV1 {
        self.lds.frame
    }
    pub(crate) const fn unresolved_conditions(&self) -> &'static str {
        UNRESOLVED_ABI_CONDITIONS_V22
    }
}
fn mismatch(detail: &'static str) -> CompilerDescriptorError {
    CompilerDescriptorError::ProductionDescriptorMismatch(detail)
}
fn resource(error: Resource) -> CompilerDescriptorError {
    CompilerDescriptorError::PhysicalLdsExchangeResourceV22(error)
}
// Keep this scope's resource mapping local: the existing V20 From<Resource>
// implementation must not relabel V22 denials or be duplicated.
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
        return Err(mismatch("LDS-exchange ABI one typed source root"));
    };
    let semantic = checked.semantic_ssa().source_semantic();
    let [semantic_root] = semantic.roots() else {
        return Err(mismatch("LDS-exchange ABI one semantic root"));
    };
    let function = semantic
        .functions()
        .get(semantic_root.index() as usize)
        .ok_or_else(|| mismatch("LDS-exchange ABI actual semantic function"))?;
    let entry = function
        .kernel_entry()
        .ok_or_else(|| mismatch("LDS-exchange ABI source entry"))?;
    let [kernel] = checked.executable().module().kernels.as_slice() else {
        return Err(mismatch("LDS-exchange ABI one canonical kernel"));
    };
    let [launch] = checked.source_launch().roots() else {
        return Err(mismatch("LDS-exchange ABI one source launch"));
    };
    if semantic.wire_version() != fe2o3_mir_model::semantic_mir_v1::SemanticMirWireVersionV1::V39
        || entry.kernel_binding_identity().as_bytes() != &root.kernel_binding_bytes()
        || launch.kernel_binding() != root.kernel_binding_bytes()
        || std::str::from_utf8(entry.export_symbol().as_bytes()).ok() != Some(root.entry_symbol())
        || kernel.id.as_str() != root.entry_symbol()
        || kernel.entry.as_str() != root.entry_symbol()
        || root.explicit_argument_bytes != 32
        || root.kernarg_alignment_bytes != 8
        || launch.source_launch().max_grid() != [1, 1, 1]
        || launch.layout().global_extents() != [128, 1, 1]
    {
        return Err(mismatch(
            "LDS-exchange ABI exact source/canonical/descriptor association",
        ));
    }
    let source_launch = root
        .source_launch()
        .ok_or_else(|| mismatch("LDS-exchange ABI retained validated source launch"))?;
    let source_block = match source_launch.block_size() {
        fe2o3_artifacts::BlockSize::Exact(block) => Some([block.x(), block.y(), block.z()]),
        fe2o3_artifacts::BlockSize::Any | fe2o3_artifacts::BlockSize::AtMost(_) => None,
    };
    let source_grid = source_launch.max_grid();
    if source_launch.rank() != launch.source_launch().rank()
        || source_block != launch.source_launch().exact_workgroup()
        || [source_grid.x(), source_grid.y(), source_grid.z()] != launch.source_launch().max_grid()
    {
        return Err(mismatch(
            "LDS-exchange ABI retained validated source launch",
        ));
    }
    // This existing per-root validator joins real SharedBorrow/ExclusiveOwner
    // source evidence. It is not a runtime pointer/bounds/alias attestation.
    validate_production_v1_semantic_root_ownership_evidence(root, semantic, function)?;
    let [input, output] = root.arguments.as_slice() else {
        return Err(mismatch("LDS-exchange ABI two logical arguments"));
    };
    for (argument, kind, access, offset, diagnostic) in [
        (
            input,
            DescriptorArgumentKindV1::SharedSlice(ScalarTypeV1::U32),
            AccessMode::ReadOnly,
            0,
            "LDS-exchange ABI exact shared readonly input slice pair",
        ),
        (
            output,
            DescriptorArgumentKindV1::DisjointSlice(ScalarTypeV1::U32),
            AccessMode::ReadWrite,
            16,
            "LDS-exchange ABI exact exclusive output slice pair",
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
        || !Lds::from_report(report).is_exact_required_profile(report)
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
            "LDS-exchange ABI complete global and kernarg conditions",
        ));
    }
    let declaration =
        fe2o3_kernel_ir::gfx942_physical_lds_exchange_declaration_v22(checked.executable())
            .ok_or_else(|| mismatch("LDS-exchange ABI actual declaration"))?;
    if declaration.lds_frame != report.lds_frame().descriptor()
        || declaration.begin_site != report.lds_frame().source_site()
        || declaration.workgroup != conditions.workgroup
        || declaration.maximum_workgroups != conditions.workgroups
    {
        return Err(mismatch(
            "LDS-exchange ABI exact declared frame and participation",
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
        return Err(mismatch("LDS-exchange ABI exact ordered read report"));
    }
    let [function] = checked.executable().module().functions.as_slice() else {
        return Err(mismatch("LDS-exchange ABI one function"));
    };
    let body = function
        .body
        .as_ref()
        .ok_or_else(|| mismatch("LDS-exchange ABI body"))?;
    let mut cursor = 0;
    for block in &body.blocks {
        for (index, operation) in block.operations.iter().enumerate() {
            let Op::Gfx942PhysicalLdsExchangeStep(step) = operation.kind else {
                continue;
            };
            if step.instruction.opcode != Opcode::LoadKernargPair {
                continue;
            }
            let row = rows
                .get(cursor)
                .copied()
                .flatten()
                .ok_or_else(|| mismatch("LDS-exchange ABI missing authored read"))?;
            cursor += 1;
            let slot = match step.instruction.immediate {
                0 => Slot::InputPointer,
                8 => Slot::InputLength,
                16 => Slot::OutputPointer,
                24 => Slot::OutputLength,
                _ => return Err(mismatch("LDS-exchange ABI read outside exact slots")),
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
                    "LDS-exchange ABI authored read operand or slot join",
                ));
            }
            let wait = block.operations[index + 1..]
                .iter()
                .enumerate()
                .find_map(|(relative, op)| match op.kind {
                    Op::Gfx942PhysicalLdsExchangeStep(s)
                        if s.instruction.opcode == Opcode::WaitLgkm0 =>
                    {
                        Some((Location::new(block.id, index + 1 + relative), s.site))
                    }
                    _ => None,
                })
                .ok_or_else(|| mismatch("LDS-exchange ABI authored read readiness"))?;
            if (row.ready_at, row.ready_site) != wait {
                return Err(mismatch("LDS-exchange ABI exact explicit wait occurrence"));
            }
        }
    }
    if cursor != 4 {
        return Err(mismatch("LDS-exchange ABI exact four authored reads"));
    }
    Ok(())
}
pub(crate) fn prepare_physical_lds_exchange_abi_v22(
    checked: &Checked,
    roots: &[TypedDescriptorRootV1],
    budget: &mut Budget<'_>,
) -> Result<PhysicalLdsExchangePreparedAbiV22, CompilerDescriptorError> {
    let retained = std::mem::size_of::<PhysicalLdsExchangePreparedAbiV22>();
    // Fixed arrays need no additional heap allocation; this receipt retains
    // all four read rows, both global records and all unresolved conditions.
    budget
        .with_prepaid_scope::<_, ScopedError>(checked.retained_storage(), 1, WORK, retained, |_| {
            let root = validate_source_and_slots(checked, roots)?;
            let report = checked.memory_obligations();
            let reads = (*report.kernarg_reads()).map(|row| Some(Read::from(row)));
            validate_reads(checked, &reads)?;
            Ok(PhysicalLdsExchangePreparedAbiV22 {
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
                lds: Lds::from_report(report),
                retained,
            })
        })
        .map_err(|error| error.0)
}
pub(super) fn validate_prepared_abi_v22(
    checked: &Checked,
    roots: &[TypedDescriptorRootV1],
    abi: &PhysicalLdsExchangePreparedAbiV22,
    budget: &mut Budget<'_>,
) -> Result<(), CompilerDescriptorError> {
    budget.charge_work(1).map_err(resource)?;
    let retained = std::mem::size_of::<PhysicalLdsExchangePreparedAbiV22>();
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
        return Err(mismatch("LDS-exchange ABI retained source identity"));
    }
    if abi.conditions != Conditions::from_report(report)
        || !abi.conditions.is_exact_required_profile()
    {
        return Err(mismatch("LDS-exchange ABI retained unresolved conditions"));
    }
    if abi.input_read != GlobalRead::from(report.input_read())
        || abi.output_store != GlobalStore::from(report.output_store())
    {
        return Err(mismatch(
            "LDS-exchange ABI exact global read/write readiness and SSA report",
        ));
    }
    if abi.lds != Lds::from_report(report) || !abi.lds.is_exact_required_profile(report) {
        return Err(mismatch(
            "LDS-exchange ABI exact frame, completion and publication report",
        ));
    }
    validate_reads(checked, &abi.reads)
}
#[cfg(test)]
#[path = "compiler_descriptor_physical_lds_exchange_abi_v22_tests.rs"]
mod tests;
#[cfg(test)]
pub(crate) use tests::qualify_actual_owner_abi_controls_v22;
