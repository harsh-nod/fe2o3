//! Exact compiler-ABI preparation for source-owned physical KIR20.
//! Required runtime conditions are retained, never marked discharged.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    FunctionOperationLocation as Location, Gfx942PhysicalEntryOpcodeV20 as Opcode,
    Gfx942PhysicalEntrySourceSiteVNext as Site, OperationKind as Op, PhysicalEntryKernargReadV20,
    PhysicalEntryKernargSlotV20 as Slot, ValueId,
};
use fe2o3_lower_mir_kernel::ProductionPhysicalEntryCheckedKirOwnerV20 as Checked;

const WORK: usize = 32_768;
pub(crate) const UNRESOLVED_ABI_CONDITIONS_V20: &str = "kernarg immutability and output/kernarg disjointness remain runtime obligations; output launch envelope requires 512 bytes";
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Read {
    location: Location,
    site: Site,
    offset: u32,
    width: u32,
    alignment: u32,
    slot: Slot,
    base: [ValueId; 2],
    results: [Option<ValueId>; 2],
    ready_at: Location,
    ready_site: Site,
}
impl From<PhysicalEntryKernargReadV20> for Read {
    fn from(row: PhysicalEntryKernargReadV20) -> Self {
        Self {
            location: row.location(),
            site: row.source_site(),
            offset: row.byte_offset(),
            width: row.byte_width(),
            alignment: row.alignment(),
            slot: row.slot(),
            base: row.base(),
            results: row.results(),
            ready_at: row.ready_at(),
            ready_site: row.ready_source_site(),
        }
    }
}
/// Private source/descriptor association plus the complete read projection.
/// It is not an authority type; unresolved conditions survive until explicit
/// inert LLVM/handoff demotion. No protected/host/native-admission API accepts it.
pub(crate) struct PhysicalEntryPreparedAbiV20 {
    canonical: [u8; 32],
    semantic: [u8; 32],
    binding: [u8; 32],
    reads: Vec<Read>,
    immutable_kernarg_required: bool,
    output_disjoint_kernarg_required: bool,
    retained: usize,
}
impl PhysicalEntryPreparedAbiV20 {
    pub(crate) const fn retained_storage(&self) -> usize {
        self.retained
    }
    pub(crate) const fn unresolved_conditions(&self) -> &'static str {
        UNRESOLVED_ABI_CONDITIONS_V20
    }
}
fn mismatch(detail: &'static str) -> CompilerDescriptorError {
    CompilerDescriptorError::ProductionDescriptorMismatch(detail)
}
impl From<Resource> for CompilerDescriptorError {
    fn from(error: Resource) -> Self {
        Self::PhysicalEntryResourceV20(error)
    }
}
fn resource(error: Resource) -> CompilerDescriptorError {
    CompilerDescriptorError::PhysicalEntryResourceV20(error)
}
fn validate_source_and_slots<'a>(
    checked: &Checked,
    roots: &'a [TypedDescriptorRootV1],
) -> Result<&'a TypedDescriptorRootV1, CompilerDescriptorError> {
    let [root] = roots else {
        return Err(mismatch("physical ABI one typed source root"));
    };
    let semantic = checked.semantic_ssa().source_semantic();
    let [semantic_root] = semantic.roots() else {
        return Err(mismatch("physical ABI one semantic root"));
    };
    let function = semantic
        .functions()
        .get(semantic_root.index() as usize)
        .ok_or_else(|| mismatch("physical ABI actual semantic function"))?;
    let entry = function
        .kernel_entry()
        .ok_or_else(|| mismatch("physical ABI source entry"))?;
    let [kernel] = checked.executable().module().kernels.as_slice() else {
        return Err(mismatch("physical ABI one canonical kernel"));
    };
    let [launch] = checked.source_launch().roots() else {
        return Err(mismatch("physical ABI one source launch"));
    };
    if semantic.wire_version() != fe2o3_mir_model::semantic_mir_v1::SemanticMirWireVersionV1::V37
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
            "physical ABI exact source/canonical/descriptor association",
        ));
    }
    let source_launch = root
        .source_launch()
        .ok_or_else(|| mismatch("physical ABI retained validated source launch"))?;
    let source_block = match source_launch.block_size() {
        fe2o3_artifacts::BlockSize::Exact(block) => Some([block.x(), block.y(), block.z()]),
        fe2o3_artifacts::BlockSize::Any | fe2o3_artifacts::BlockSize::AtMost(_) => None,
    };
    let source_grid = source_launch.max_grid();
    if source_launch.rank() != launch.source_launch().rank()
        || source_block != launch.source_launch().exact_workgroup()
        || [source_grid.x(), source_grid.y(), source_grid.z()] != launch.source_launch().max_grid()
    {
        return Err(mismatch("physical ABI retained validated source launch"));
    }
    // The exact singleton roster/binding check above permits the allocation-free
    // existing per-root ABI/ownership validator; do not rebuild roster vectors.
    validate_production_v1_semantic_root_ownership_evidence(root, semantic, function)?;
    let [output, a, b, c, selector] = root.arguments.as_slice() else {
        return Err(mismatch("physical ABI five logical arguments"));
    };
    if output.kind != DescriptorArgumentKindV1::DisjointSlice(ScalarTypeV1::U32)
        || output.access != AccessMode::ReadWrite
        || output.offset != 0
        || output.source_size != 16
        || output.source_alignment != 8
        || output.rustc_abi_class != RustcAbiClassV1::ScalarPair
    {
        return Err(mismatch("physical ABI exact exclusive output slice pair"));
    }
    for (argument, offset) in [a, b, c, selector].into_iter().zip([16, 20, 24, 28]) {
        if argument.kind != DescriptorArgumentKindV1::Scalar(ScalarTypeV1::U32)
            || argument.offset != offset
            || argument.source_size != 4
            || argument.source_alignment != 4
            || argument.rustc_abi_class != RustcAbiClassV1::Scalar
        {
            return Err(mismatch("physical ABI exact u32 scalar slot"));
        }
    }
    let report = checked.memory_obligations();
    if report.canonical_identity() != checked.executable().identity().digest()
        || report.kernarg_abi().minimum_bytes() != 32
        || report.kernarg_abi().alignment() != 8
        || !report.kernarg_abi().requires_immutable_kernarg()
        || report.kernarg_abi().disjoint_output() != report.store().allocation()
        || report.store().allocation().parameter_index() != 0
        || report.output().bounds_requirements().len() != 1
        || report.output().bounds_requirements()[0].minimum_byte_len() != Some(512)
    {
        return Err(mismatch(
            "physical ABI complete output and kernarg conditions",
        ));
    }
    Ok(root)
}
fn validate_reads(checked: &Checked, rows: &[Read]) -> Result<(), CompilerDescriptorError> {
    let expected = checked.memory_obligations().kernarg_reads();
    if rows.is_empty()
        || rows.len() > 64
        || rows.len() != expected.len()
        || rows
            .iter()
            .copied()
            .ne(expected.iter().copied().map(Read::from))
    {
        return Err(mismatch("physical ABI exact ordered read report"));
    }
    let module = checked.executable().module();
    let [function] = module.functions.as_slice() else {
        return Err(mismatch("physical ABI one function"));
    };
    let body = function
        .body
        .as_ref()
        .ok_or_else(|| mismatch("physical ABI body"))?;
    let mut cursor = 0;
    for block in &body.blocks {
        for (index, operation) in block.operations.iter().enumerate() {
            let Op::Gfx942PhysicalEntryStep(step) = operation.kind else {
                continue;
            };
            if !matches!(
                step.instruction.opcode,
                Opcode::LoadKernargPair | Opcode::LoadKernargDword
            ) {
                continue;
            }
            let row = rows
                .get(cursor)
                .ok_or_else(|| mismatch("physical ABI missing authored read"))?;
            cursor += 1;
            let (width, slot) = match (step.instruction.opcode, step.instruction.immediate) {
                (Opcode::LoadKernargPair, 0) => (8, Slot::OutputPointer),
                (Opcode::LoadKernargPair, 8) => (8, Slot::OutputLength),
                (Opcode::LoadKernargDword, offset @ (16 | 20 | 24 | 28)) => {
                    (4, Slot::ScalarArgument(((offset - 16) / 4 + 1) as u8))
                }
                _ => return Err(mismatch("physical ABI read outside exact slots")),
            };
            if row.location != Location::new(block.id, index)
                || row.site != step.site
                || row.offset != step.instruction.immediate
                || row.width != width
                || row.alignment != width
                || row.slot != slot
                || row.base.map(Some) != [step.operands[0], step.operands[1]]
                || row
                    .results
                    .iter()
                    .flatten()
                    .copied()
                    .ne(operation.results.iter().map(|r| r.id))
            {
                return Err(mismatch("physical ABI authored read operand or slot join"));
            }
            let wait = block.operations[index + 1..]
                .iter()
                .enumerate()
                .find_map(|(relative, op)| match op.kind {
                    Op::Gfx942PhysicalEntryStep(s) if s.instruction.opcode == Opcode::WaitLgkm0 => {
                        Some((Location::new(block.id, index + 1 + relative), s.site))
                    }
                    _ => None,
                })
                .ok_or_else(|| mismatch("physical ABI authored read readiness"))?;
            if (row.ready_at, row.ready_site) != wait {
                return Err(mismatch("physical ABI exact explicit wait occurrence"));
            }
        }
    }
    if cursor != rows.len() {
        return Err(mismatch("physical ABI extra detached read"));
    }
    Ok(())
}
pub(crate) fn prepare_physical_entry_abi_v20(
    checked: &Checked,
    roots: &[TypedDescriptorRootV1],
    budget: &mut Budget<'_>,
) -> Result<PhysicalEntryPreparedAbiV20, CompilerDescriptorError> {
    let retained = std::mem::size_of::<PhysicalEntryPreparedAbiV20>()
        .checked_add(64 * std::mem::size_of::<Read>())
        .ok_or_else(|| resource(Resource::Arithmetic))?;
    budget.with_prepaid_scope(checked.retained_storage(), 1, WORK, retained, |_| {
        let root = validate_source_and_slots(checked, roots)?;
        let source_reads = checked.memory_obligations().kernarg_reads();
        if source_reads.is_empty() || source_reads.len() > 64 {
            return Err(mismatch("physical ABI exact ordered read report"));
        }
        let mut reads = Vec::new();
        reads
            .try_reserve_exact(64)
            .map_err(|_| resource(Resource::Allocation))?;
        if reads.capacity() > 64 {
            return Err(resource(Resource::Accounting));
        }
        reads.extend(source_reads.iter().copied().map(Read::from));
        validate_reads(checked, &reads)?;
        Ok(PhysicalEntryPreparedAbiV20 {
            canonical: *checked.executable().identity().digest(),
            semantic: *checked
                .semantic_ssa()
                .source_semantic()
                .semantic_sha256()
                .as_bytes(),
            binding: root.kernel_binding_bytes(),
            reads,
            immutable_kernarg_required: true,
            output_disjoint_kernarg_required: true,
            retained,
        })
    })
}
pub(super) fn validate_prepared_abi_v20(
    checked: &Checked,
    roots: &[TypedDescriptorRootV1],
    abi: &PhysicalEntryPreparedAbiV20,
    budget: &mut Budget<'_>,
) -> Result<(), CompilerDescriptorError> {
    budget.charge_work(1).map_err(resource)?;
    let retained = std::mem::size_of::<PhysicalEntryPreparedAbiV20>()
        .checked_add(64 * std::mem::size_of::<Read>())
        .ok_or_else(|| resource(Resource::Arithmetic))?;
    let required_floor = checked
        .retained_storage()
        .checked_add(retained)
        .ok_or_else(|| resource(Resource::Arithmetic))?;
    if abi.retained != retained || budget.storage() < required_floor {
        return Err(resource(Resource::Accounting));
    }
    budget.charge_work(WORK - 1).map_err(resource)?;
    let root = validate_source_and_slots(checked, roots)?;
    if abi.canonical != *checked.executable().identity().digest()
        || abi.semantic
            != *checked
                .semantic_ssa()
                .source_semantic()
                .semantic_sha256()
                .as_bytes()
        || abi.binding != root.kernel_binding_bytes()
        || !abi.immutable_kernarg_required
        || !abi.output_disjoint_kernarg_required
    {
        return Err(mismatch(
            "physical ABI retained source or unresolved conditions",
        ));
    }
    validate_reads(checked, &abi.reads)
}

#[cfg(test)]
#[path = "compiler_descriptor_physical_entry_abi_v20_tests.rs"]
mod tests;
#[cfg(test)]
pub(crate) use tests::qualify_actual_owner_abi_controls_v20;
