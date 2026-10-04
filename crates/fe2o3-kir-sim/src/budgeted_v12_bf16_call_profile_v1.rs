//! Allocation-free graph seal and composed two-frame prepayment.
use super::*;
use fe2o3_kernel_ir::{
    AddressSpace, AmdGpuDiagnosticOperation, Function, FunctionRole, MatrixOperationKind,
    MemoryIntrinsicOperation, OperationKind, ScalarType, Terminator, Type, ValueId,
};
use std::mem::size_of;
type P = V12CpuObservationProfileErrorV1;
const TRAP: &str = "__fe2o3_ir_amdgpu_diagnostics_gfx942_v1_trap";
const BLOCKS: usize = 32;
const OPERATIONS: usize = 1024;
const VALUES: usize = 512;

// Identity edge rows borrow no owner and confer no authority. They exist only
// during checking inside the prepaid scope. No heap allocation or recursion.
struct Edges {
    from: [ValueId; VALUES],
    to: [ValueId; VALUES],
    used: usize,
}
impl Edges {
    fn new() -> Self {
        Self {
            from: [ValueId(0); VALUES],
            to: [ValueId(0); VALUES],
            used: 0,
        }
    }
    fn add(&mut self, from: ValueId, to: ValueId) -> Result<(), P> {
        if self.used == VALUES || self.from[..self.used].contains(&from) {
            return Err(P::Counts);
        }
        self.from[self.used] = from;
        self.to[self.used] = to;
        self.used += 1;
        Ok(())
    }
    fn resolve(&self, mut value: ValueId) -> Result<ValueId, P> {
        for _ in 0..=BLOCKS {
            let Some(index) = self.from[..self.used].iter().position(|id| *id == value) else {
                return Ok(value);
            };
            value = self.to[index];
        }
        Err(P::Operation)
    }
}

pub(super) fn work(records: usize) -> Result<usize, Resource> {
    // Keep the full memory term, and explicitly double separately summed
    // non-memory/frame work and debug snapshots. There is no claimed slack in
    // the saturated 2^30 memory term. All arithmetic is checked.
    let memory = 4usize
        .checked_mul(65536)
        .and_then(|n| n.checked_mul(4096))
        .ok_or(Resource::Arithmetic)?;
    let other = 4usize
        .checked_mul(1024 * 512)
        .and_then(|n| n.checked_add(64 * 512 * 8))
        .and_then(|n| n.checked_add(65536 * 4 + 32768))
        .and_then(|n| n.checked_mul(2))
        .ok_or(Resource::Arithmetic)?;
    let execution = memory
        .checked_add(other)
        .and_then(|n| n.checked_mul(profile::STEPS))
        .ok_or(Resource::Arithmetic)?;
    let one_debug = 2usize * 512 * 512 + 512 * 4 + 2 * 128 * 128 + 128 * 4 + 65536 * 2 + 32;
    execution
        .checked_add(
            records
                .checked_add(1)
                .and_then(|n| n.checked_mul(one_debug))
                .and_then(|n| n.checked_mul(2))
                .ok_or(Resource::Arithmetic)?,
        )
        .and_then(|n| n.checked_add(2 * 16_777_216))
        .ok_or(Resource::Arithmetic)
}
pub(super) fn scratch_fits() -> bool {
    let snapshot = 2usize
        .checked_mul(512)
        .and_then(|n| n.checked_mul(size_of::<SimulationDebugBindingV1>() + size_of::<usize>()))
        .and_then(|n| {
            n.checked_add(128 * (size_of::<SimulationDebugAllocationV1>() + size_of::<usize>()))
        })
        .and_then(|n| n.checked_add(65536 * 2))
        .and_then(|n| {
            n.checked_add(
                2 * size_of::<SimulationDebugFrameV1>() + size_of::<SimulationDebugRecordV1>(),
            )
        })
        .and_then(|n| n.checked_mul(2));
    let probes = 8192usize
        .checked_mul(4096 + 1)
        .and_then(|n| n.checked_add(8192))
        .and_then(|n| n.checked_add(16 * 1024 * 8 + 65536 * 4));
    snapshot
        .zip(probes)
        .and_then(|(a, b)| a.checked_add(b))
        .and_then(|n| n.checked_add(64 * size_of::<ScalarBitsV1>()))
        .and_then(|n| n.checked_add(2 * size_of::<Edges>() + 4096))
        .is_some_and(|n| n <= profile::STORAGE / 2)
        && size_of::<SimulationDebugBindingV1>() <= 192
        && size_of::<SimulationDebugValueV1>() <= 176
        && size_of::<(usize, ScalarBitsV1)>() <= 4096
        && crate::execute::v12_observation_probe_cells_fit()
}
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
fn root_operation(kind: &OperationKind) -> bool {
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
        OperationKind::Wave(wave) => {
            use fe2o3_kernel_ir::{
                Convergence, SynchronizationScope, WaveOperationKind, WaveWidth,
            };
            wave.kind == WaveOperationKind::LaneId
                && wave.width == WaveWidth::Wave64
                && wave.active_lanes == 64
                && wave.convergence == Convergence::uniform(SynchronizationScope::Subgroup)
        }
        _ => false,
    }
}
fn exact_trap(function: &Function) -> bool {
    function.role == FunctionRole::ExternalImport
        && function.body.is_none()
        && function.id.as_str() == TRAP
        && function.signature.parameters.is_empty()
        && function.signature.results.is_empty()
}
fn helper(function: &Function) -> Result<(), P> {
    let body = function.body.as_ref().ok_or(P::FunctionRoster)?;
    if function.signature.parameters.len() != 12
        || body.parameters.len() != 12
        || (function.signature.results.len() != 4
            || function.signature.results.iter().any(|t| *t != Type::F32))
        || function.signature.parameters[..8]
            .iter()
            .any(|t| *t != Type::Scalar(ScalarType::Bf16))
        || function.signature.parameters[8..]
            .iter()
            .any(|t| *t != Type::F32)
        || body.blocks.is_empty()
        || body.blocks.len() > BLOCKS
    {
        return Err(P::FunctionRoster);
    }
    let mut edges = Edges::new();
    let mut current = 0usize;
    let mut visited = 0u32;
    let mut matrix = None;
    let returned = loop {
        if current >= body.blocks.len() || visited & (1 << current) != 0 {
            return Err(P::Operation);
        }
        visited |= 1 << current;
        let block = &body.blocks[current];
        if current == 0 && !block.parameters.is_empty() {
            return Err(P::Operation);
        }
        for op in &block.operations {
            let OperationKind::Matrix(actual) = &op.kind else {
                return Err(P::Operation);
            };
            if matrix.replace(op).is_some()
                || !crate::matrix_bf16_exact_v1::supported(actual)
                || op.results.len() != 4
                || op.results.iter().any(|v| v.ty != Type::F32)
            {
                return Err(P::Matrix);
            }
        }
        match block.terminator.as_ref().ok_or(P::Operation)? {
            Terminator::Branch { target, arguments } => {
                let next = body
                    .blocks
                    .iter()
                    .position(|b| b.id == *target)
                    .ok_or(P::Operation)?;
                let params = &body.blocks[next].parameters;
                if params.len() != arguments.len() {
                    return Err(P::Operation);
                }
                for (p, a) in params.iter().zip(arguments) {
                    edges.add(p.id, *a)?;
                }
                current = next;
            }
            Terminator::Return { values } if values.len() == 4 => break values,
            _ => return Err(P::Operation),
        }
    };
    if visited.count_ones() as usize != body.blocks.len() {
        return Err(P::Operation);
    }
    let matrix = matrix.ok_or(P::Matrix)?;
    let OperationKind::Matrix(actual) = &matrix.kind else {
        return Err(P::Matrix);
    };
    let MatrixOperationKind::MultiplyAccumulate {
        lhs,
        rhs,
        accumulator,
        ..
    } = &actual.kind
    else {
        return Err(P::Matrix);
    };
    if lhs.len() != 4 || rhs.len() != 4 || accumulator.len() != 4 {
        return Err(P::Matrix);
    }
    for (actual, expected) in lhs
        .iter()
        .chain(rhs)
        .chain(accumulator)
        .zip(&body.parameters)
    {
        if edges.resolve(*actual)? != *expected {
            return Err(P::Matrix);
        }
    }
    let actual = [
        edges.resolve(returned[0])?,
        edges.resolve(returned[1])?,
        edges.resolve(returned[2])?,
        edges.resolve(returned[3])?,
    ];
    let ids = [
        matrix.results[0].id,
        matrix.results[1].id,
        matrix.results[2].id,
        matrix.results[3].id,
    ];
    if actual != ids && actual != [ids[1], ids[0], ids[2], ids[3]] {
        return Err(P::Operation);
    }
    Ok(())
}

pub(super) fn check(
    view: &AdmittedSimulationModuleV1,
    input: V12CpuObservationInputV1<'_>,
) -> Result<(), P> {
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
    if !(2..=3).contains(&module.functions.len()) || module.kernels.len() != 1 {
        return Err(P::FunctionRoster);
    }
    let kernel = &module.kernels[0];
    let mut root = None;
    let mut called = None;
    for f in &module.functions {
        match f.role {
            FunctionRole::KernelEntry if root.replace(f).is_none() => {}
            FunctionRole::InternalHelper if called.replace(f).is_none() => {}
            FunctionRole::ExternalImport if exact_trap(f) => {}
            _ => return Err(P::FunctionRoster),
        }
    }
    let root = root.ok_or(P::FunctionRoster)?;
    let called = called.ok_or(P::FunctionRoster)?;
    if root.id != kernel.entry
        || kernel.id != input.request.kernel
        || root.signature.parameters.len() != 4
        || !root.signature.results.is_empty()
    {
        return Err(P::FunctionRoster);
    }
    // Establish combined lengths before any per-operation/definition walk.
    let mut blocks = 0usize;
    let mut operations = 0usize;
    let mut definitions = 0usize;
    for function in [root, called] {
        let body = function.body.as_ref().ok_or(P::FunctionRoster)?;
        if body.parameters.len() != function.signature.parameters.len()
            || function.signature.parameters.len() > 12
            || function.signature.results.len() > 4
        {
            return Err(P::Counts);
        }
        blocks = blocks.checked_add(body.blocks.len()).ok_or(P::Counts)?;
        definitions = definitions
            .checked_add(body.parameters.len())
            .ok_or(P::Counts)?;
        if blocks > BLOCKS || definitions > VALUES {
            return Err(P::Counts);
        }
        for block in &body.blocks {
            operations = operations
                .checked_add(block.operations.len())
                .ok_or(P::Counts)?;
            definitions = definitions
                .checked_add(block.parameters.len())
                .ok_or(P::Counts)?;
            if operations > OPERATIONS || definitions > VALUES {
                return Err(P::Counts);
            }
            for op in &block.operations {
                definitions = definitions.checked_add(op.results.len()).ok_or(P::Counts)?;
                if definitions > VALUES {
                    return Err(P::Counts);
                }
            }
        }
    }
    let mut calls = 0;
    for function in [root, called] {
        if function
            .signature
            .parameters
            .iter()
            .chain(&function.signature.results)
            .any(|t| !ty(t))
        {
            return Err(P::Type);
        }
        for block in &function.body.as_ref().ok_or(P::FunctionRoster)?.blocks {
            if block.parameters.iter().any(|v| !ty(&v.ty)) {
                return Err(P::Type);
            }
            for op in &block.operations {
                if op.results.iter().any(|v| !ty(&v.ty)) {
                    return Err(P::Type);
                }
                if std::ptr::eq(function, root) {
                    match &op.kind {
                        OperationKind::Call { callee, arguments } if *callee == called.id => {
                            calls += 1;
                            if arguments.len() != 12
                                || op.results.len() != 4
                                || op.results.iter().any(|v| v.ty != Type::F32)
                            {
                                return Err(P::Operation);
                            }
                        }
                        OperationKind::Call { callee, arguments }
                            if callee.as_str() == TRAP
                                && matches!(
                                    AmdGpuDiagnosticOperation::from_intrinsic_call(
                                        callee, arguments
                                    ),
                                    Some(AmdGpuDiagnosticOperation::Trap)
                                ) => {}
                        kind if root_operation(kind) => {}
                        _ => return Err(P::Operation),
                    }
                }
            }
        }
    }
    if calls != 1 {
        return Err(P::Operation);
    }
    helper(called)?;
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
    fn checked_work_envelope_is_composed_not_saturated_and_allows_sixty_four_attempts() {
        let actual = work(profile::RECORDS).unwrap();
        assert_eq!(actual, 141_523_842_044_992);
        assert!(actual > profile::work(profile::RECORDS).unwrap());
        assert!(actual.checked_mul(64).unwrap() < 1usize << 54);
        assert_eq!(work(usize::MAX), Err(Resource::Arithmetic));
        assert!(scratch_fits());
        assert_eq!(
            Bf16CallCpuObservationOptionsV1::default()
                .simulation_limits()
                .max_call_depth,
            2
        );
        assert_eq!(
            V12CpuObservationOptionsV1::default()
                .simulation_limits()
                .max_call_depth,
            1
        );
    }
    #[test]
    fn identity_edge_capacity_duplicate_and_cycle_fail_closed() {
        let mut rows = Edges::new();
        for n in 0..VALUES {
            rows.add(ValueId(n as u32), ValueId(1000 + n as u32))
                .unwrap();
        }
        assert_eq!(rows.resolve(ValueId(511)), Ok(ValueId(1511)));
        assert_eq!(rows.add(ValueId(512), ValueId(1512)), Err(P::Counts));
        let mut duplicate = Edges::new();
        duplicate.add(ValueId(1), ValueId(2)).unwrap();
        assert_eq!(duplicate.add(ValueId(1), ValueId(3)), Err(P::Counts));
        duplicate.add(ValueId(2), ValueId(1)).unwrap();
        assert_eq!(duplicate.resolve(ValueId(1)), Err(P::Operation));
    }
}
