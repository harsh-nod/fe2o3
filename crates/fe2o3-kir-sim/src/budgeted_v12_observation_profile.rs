//! Allocation-free closed shape checks and conservative fixed prepayment.
use super::*;
use fe2o3_kernel_ir::{
    AddressSpace, AmdGpuDiagnosticOperation, Convergence, FunctionRole, MatrixOperationKind,
    MemoryIntrinsicOperation, OperationKind, SynchronizationScope, Type, WaveOperationKind,
    WaveWidth,
};
use std::mem::size_of;

pub(super) const STEPS: usize = 131072;
pub(super) const RECORDS: usize = 65536;
pub(super) const STORAGE: usize = 128 * 1024 * 1024;
// Existing per-step fixed overhead includes the full-wave LaneId rendezvous:
// four conservative 64-by-64 scans plus eight linear 64-lane passes.
const STEP_FIXED_WORK: usize = 32768;
const LANE_ID_WORK: usize = 4 * 64 * 64 + 8 * 64;
const _: () = assert!(LANE_ID_WORK <= STEP_FIXED_WORK);
const TRAP: &str = "__fe2o3_ir_amdgpu_diagnostics_gfx942_v1_trap";

// Separately sum the worst-case memory term and all other bounded logical work;
// do not pretend the 2^30 memory term has spare space for the other terms.
pub(super) fn work(records: usize) -> Result<usize, Resource> {
    let memory = 4usize
        .checked_mul(65536)
        .and_then(|n| n.checked_mul(4096))
        .ok_or(Resource::Arithmetic)?;
    let other = 4usize
        .checked_mul(1024 * 512)
        .and_then(|n| n.checked_add(64 * 512 * 8))
        .and_then(|n| n.checked_add(65536 * 4 + STEP_FIXED_WORK))
        .ok_or(Resource::Arithmetic)?;
    let execution = memory
        .checked_add(other)
        .and_then(|n| n.checked_mul(STEPS))
        .ok_or(Resource::Arithmetic)?;
    let one_debug = 2usize * 512 * 512 + 512 * 4 + 2 * 128 * 128 + 128 * 4 + 65536 * 2 + 32;
    execution
        .checked_add(
            records
                .checked_add(1)
                .and_then(|n| n.checked_mul(one_debug))
                .ok_or(Resource::Arithmetic)?,
        )
        .and_then(|n| n.checked_add(16_777_216))
        .ok_or(Resource::Arithmetic)
}

pub(super) fn scratch_fits() -> bool {
    // Existing calculators allocate one capacity probe at a time. The maximum
    // probe roster is frontier4096 cells, <=1024 operation/index cells or512
    // values. 4096 bytes/cell is a conservative inspected private-cell bound;
    // this is pinned implementation accounting, not allocator/RSS control.
    // Concurrent snapshots use their ACTUAL public types, doubled for moves.
    let snapshot = 512usize
        .checked_mul(size_of::<SimulationDebugBindingV1>() + size_of::<usize>())
        .and_then(|n| {
            n.checked_add(128 * (size_of::<SimulationDebugAllocationV1>() + size_of::<usize>()))
        })
        .and_then(|n| n.checked_add(65536 * 2))
        .and_then(|n| {
            n.checked_add(
                size_of::<SimulationDebugFrameV1>() + size_of::<SimulationDebugRecordV1>(),
            )
        })
        .and_then(|n| n.checked_mul(2));
    // 4096 entries require at most8192 hash buckets at7/8 load, including
    // one control byte per bucket and bounded header/alignment slack.
    let probes = 8192usize
        .checked_mul(4096 + 1)
        .and_then(|n| n.checked_add(8192))
        .and_then(|n| n.checked_add(16 * 1024 * 8 + 65536 * 4));
    snapshot
        .zip(probes)
        .and_then(|(a, b)| a.checked_add(b))
        // resolve_ready_waves has one fixed stack array even for LaneId.
        // Its already-reserved result Vec is accounted in Engine residency;
        // explicitly cover its allocation-probe cell without new heap storage.
        .and_then(|n| n.checked_add(64 * size_of::<ScalarBitsV1>()))
        .is_some_and(|n| n <= STORAGE / 2)
        && size_of::<SimulationDebugBindingV1>() <= 192
        && size_of::<SimulationDebugValueV1>() <= 176
        && size_of::<(usize, ScalarBitsV1)>() <= 4096
        && crate::execute::v12_observation_probe_cells_fit()
}

// No recursive containers: scalar/pointer-to-scalar/logical-slice-of-scalar only.
fn ty(ty: &Type) -> bool {
    match ty {
        Type::Scalar(_) | Type::Unit => true,
        Type::Pointer(p) => {
            matches!(p.pointee.as_ref(), Type::Scalar(_))
                && matches!(
                    p.address_space,
                    AddressSpace::Private | AddressSpace::Global
                )
        }
        Type::Slice(s) => {
            matches!(s.element.as_ref(), Type::Scalar(_)) && s.address_space == AddressSpace::Global
        }
        _ => false,
    }
}
fn allowed(kind: &OperationKind) -> bool {
    match kind {
        OperationKind::Constant(_)
        | OperationKind::Intrinsic(_)
        | OperationKind::Unary { .. }
        | OperationKind::Binary { .. }
        | OperationKind::Compare { .. }
        | OperationKind::Cast { .. }
        | OperationKind::Select { .. }
        | OperationKind::SliceLength { .. }
        | OperationKind::SliceData { .. }
        | OperationKind::GetElementPointer { .. }
        | OperationKind::Load { .. }
        | OperationKind::GuardedLoad { .. }
        | OperationKind::Store { .. }
        | OperationKind::GuardedStore { .. } => true,
        OperationKind::Alloca {
            element,
            address_space,
            ..
        } => matches!(element, Type::Scalar(_)) && *address_space == AddressSpace::Private,
        OperationKind::MemoryIntrinsic(
            MemoryIntrinsicOperation::CopyNonOverlapping { .. }
            | MemoryIntrinsicOperation::PointerDistance { .. },
        ) => true,
        OperationKind::Call { callee, arguments } => {
            callee.as_str() == TRAP
                && matches!(
                    AmdGpuDiagnosticOperation::from_intrinsic_call(callee, arguments),
                    Some(AmdGpuDiagnosticOperation::Trap)
                )
        }
        OperationKind::Matrix(matrix) => {
            crate::matrix_bf16_exact_v1::supported(matrix)
                && matches!(matrix.kind, MatrixOperationKind::MultiplyAccumulate { .. })
        }
        OperationKind::Wave(wave) => {
            wave.kind == WaveOperationKind::LaneId
                && wave.width == WaveWidth::Wave64
                && wave.active_lanes == 64
                && wave.convergence == Convergence::uniform(SynchronizationScope::Subgroup)
        }
        // Fail closed for vectors, contracts, atomic/barrier/physical/asm
        // families and every other wave operation. Ordinary preflight is still mandatory.
        _ => false,
    }
}
pub(super) fn check(
    view: &AdmittedSimulationModuleV1,
    input: V12CpuObservationInputV1<'_>,
) -> Result<(), V12CpuObservationProfileErrorV1> {
    use V12CpuObservationProfileErrorV1 as P;
    let canonical = input.owner.canonical();
    if canonical.canonical_bytes().len() > 16 * 1024 {
        return Err(P::CanonicalBytes);
    }
    if view.identity != SimulationKernelIrIdentityV1::from(*canonical.identity())
        || view.module != *input.owner.module()
    {
        return Err(P::OwnerMismatch);
    }
    let module = input.owner.module();
    let root = match module.functions.as_slice() {
        [root] => root,
        [root, trap]
            if trap.role == FunctionRole::ExternalImport
                && trap.body.is_none()
                && trap.id.as_str() == TRAP
                && trap.signature.parameters.is_empty()
                && trap.signature.results.is_empty() =>
        {
            root
        }
        _ => return Err(P::FunctionRoster),
    };
    let [kernel] = module.kernels.as_slice() else {
        return Err(P::FunctionRoster);
    };
    if root.role != FunctionRole::KernelEntry
        || root.id != kernel.entry
        || root.signature.parameters.len() != 4
        || !root.signature.results.is_empty()
        || kernel.id != input.request.kernel
    {
        return Err(P::FunctionRoster);
    }
    let body = root.body.as_ref().ok_or(P::FunctionRoster)?;
    if body.parameters.len() != 4 || body.blocks.len() > 32 {
        return Err(P::Counts);
    }
    if root.signature.parameters.iter().any(|t| !ty(t)) {
        return Err(P::Type);
    }
    let mut definitions = body.parameters.len();
    let mut operations = 0usize;
    let mut matrices = 0usize;
    for block in &body.blocks {
        definitions = definitions
            .checked_add(block.parameters.len())
            .ok_or(P::Counts)?;
        for value in &block.parameters {
            if !ty(&value.ty) {
                return Err(P::Type);
            }
        }
        operations = operations
            .checked_add(block.operations.len())
            .ok_or(P::Counts)?;
        for op in &block.operations {
            definitions = definitions.checked_add(op.results.len()).ok_or(P::Counts)?;
            if op.results.iter().any(|v| !ty(&v.ty)) {
                return Err(P::Type);
            }
            if !allowed(&op.kind) {
                return Err(P::Operation);
            }
            if matches!(op.kind, OperationKind::Matrix(_)) {
                matrices += 1;
            }
        }
    }
    if operations > 1024 || definitions > 512 {
        return Err(P::Counts);
    }
    if matrices != 1 {
        return Err(P::Matrix);
    }
    let request = input.request;
    if request.grid.0 != [64, 1, 1] || request.workgroup.0 != [64, 1, 1] {
        return Err(P::Launch);
    }
    if request.arguments.len() != 4
        || request.arguments.capacity() > 4
        || request.shared_buffers.len() > 4
        || request.shared_buffers.capacity() > 4
        || request.kernel.retained_capacity_bytes() > 16384
    {
        return Err(P::Request);
    }
    let mut bytes = 0usize;
    for buffer in request
        .arguments
        .iter()
        .filter_map(|a| match a {
            SimulationArgumentV1::Buffer(b) => Some(b),
            _ => None,
        })
        .chain(request.shared_buffers.iter().map(|b| &b.buffer))
    {
        if buffer.bytes().len() > 4096
            || buffer
                .retained_payload_capacity_bytes()
                .is_none_or(|n| n > 8192)
        {
            return Err(P::Request);
        }
        bytes = bytes.checked_add(buffer.bytes().len()).ok_or(P::Request)?;
    }
    if bytes > 65536 {
        return Err(P::Request);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn full_summed_work_and_sixty_four_attempts_fit_original_ceiling() {
        assert_eq!(4usize * 65536 * 4096, 1usize << 30);
        let one = work(RECORDS).unwrap();
        assert!(one > (1usize << 47)); // other work is NOT hidden in the full memory term
        assert!(one.checked_mul(64).unwrap() < (1usize << 54));
        assert!(work(usize::MAX).is_err());
        assert!(scratch_fits());
    }
    #[test]
    fn profile_options_never_raise_old_global_limits() {
        let base = V12CpuObservationOptionsV1::default();
        assert!(base.with_step_limit(0).is_err());
        assert!(base.with_step_limit(STEPS as u64 + 1).is_err());
        assert!(base.with_record_limit(0).is_err());
        assert!(base.with_record_limit(RECORDS + 1).is_err());
        assert_eq!(
            base.with_step_limit(1)
                .unwrap()
                .simulation_limits()
                .max_steps,
            1
        );
    }

    #[test]
    fn lane_id_is_exact_full_wave64_subgroup_only() {
        use fe2o3_kernel_ir::WaveOperation;
        let full = WaveOperation::full(WaveOperationKind::LaneId, WaveWidth::Wave64);
        assert!(allowed(&OperationKind::Wave(full.clone())));
        assert_eq!(work(RECORDS).unwrap(), 141130665200160);
        for lanes in [0, 1, 32, 63, 65, u32::MAX] {
            let mut changed = full.clone();
            changed.active_lanes = lanes;
            assert!(!allowed(&OperationKind::Wave(changed)));
        }
        assert!(!allowed(&OperationKind::Wave(WaveOperation::full(
            WaveOperationKind::LaneId,
            WaveWidth::Wave32,
        ))));
        for scope in [
            SynchronizationScope::Invocation,
            SynchronizationScope::Workgroup,
            SynchronizationScope::Device,
            SynchronizationScope::System,
        ] {
            let mut changed = full.clone();
            changed.convergence = Convergence::uniform(scope);
            assert!(!allowed(&OperationKind::Wave(changed)));
        }
    }

    #[test]
    fn lane_id_admission_does_not_enable_other_wave_families() {
        use fe2o3_kernel_ir::{ValueId, WaveF32ReductionKindV1, WaveOperation};
        for kind in [
            WaveOperationKind::Ballot {
                predicate: ValueId(0),
            },
            WaveOperationKind::Any {
                predicate: ValueId(0),
            },
            WaveOperationKind::All {
                predicate: ValueId(0),
            },
            WaveOperationKind::ShuffleIndex {
                value: ValueId(0),
                source_lane: ValueId(1),
                tile_width: 64,
            },
            WaveOperationKind::ReduceF32 {
                value: ValueId(0),
                tile_width: 64,
                kind: WaveF32ReductionKindV1::Sum,
            },
            WaveOperationKind::ReduceF32 {
                value: ValueId(0),
                tile_width: 64,
                kind: WaveF32ReductionKindV1::Maximum,
            },
            WaveOperationKind::BroadcastF32 {
                value: ValueId(0),
                source_lane: ValueId(1),
                tile_width: 64,
            },
        ] {
            assert!(!allowed(&OperationKind::Wave(WaveOperation::full(
                kind,
                WaveWidth::Wave64,
            ))));
        }
    }
}
