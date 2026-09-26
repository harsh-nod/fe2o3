use super::*;
use fe2o3_kir_sim::*;

#[derive(Debug, PartialEq)]
struct MemoryRow {
    invocation: SimulationInvocationV1,
    access: SimulationDebugMemoryAccessV1,
    offset: usize,
    bytes: usize,
    value: ScalarBitsV1,
}
struct MemoryTrace {
    rows: Vec<MemoryRow>,
    limit: usize,
}
impl SimulationDebugSinkV1 for MemoryTrace {
    fn record(&mut self, record: SimulationDebugRecordV1) -> SimulationDebugSinkControlV1 {
        if let SimulationDebugRecordKindV1::Memory {
            access,
            byte_offset,
            byte_len,
            address_space,
            value,
            ..
        } = record.kind
        {
            assert_eq!(address_space, fe2o3_kernel_ir::AddressSpace::Private);
            let SimulationDebugValueV1::Scalar(value) = value else {
                panic!("actual scalar private access required")
            };
            assert!(
                self.rows.len() < self.limit,
                "source-derived memory trace bound"
            );
            self.rows.push(MemoryRow {
                invocation: record.invocation,
                access,
                offset: byte_offset,
                bytes: byte_len,
                value,
            });
        }
        SimulationDebugSinkControlV1::Continue
    }
}
#[derive(Default, Debug, PartialEq)]
struct Events {
    calls: usize,
    returns: usize,
    reads: usize,
    writes: usize,
}
impl SimulationEventSinkV1 for Events {
    fn record(&mut self, event: &SimulationEventV1) -> Result<(), SimulationEventSinkErrorV1> {
        match event.kind {
            SimulationEventKindV1::Call { .. } => self.calls = self.calls.checked_add(1).unwrap(),
            SimulationEventKindV1::Return => self.returns = self.returns.checked_add(1).unwrap(),
            SimulationEventKindV1::MemoryRead { .. } => {
                self.reads = self.reads.checked_add(1).unwrap()
            }
            SimulationEventKindV1::MemoryWrite { .. } => {
                self.writes = self.writes.checked_add(1).unwrap()
            }
            _ => {}
        }
        Ok(())
    }
}

fn compare_actual_execution(
    owner: &ProductionCanonicalScalarFixedPointOwnerV1,
    bits: u64,
    calls: usize,
) {
    use fe2o3_kernel_ir::{LaunchExtent, VerifiedCanonicalKernelIrV12};
    cpc_run(owner, |view, budget| cpc_reports(owner, view, budget)).unwrap();
    let admit = |graph: &CsGraphV1| {
        AdmittedSimulationModuleV1::admit_v12(
            VerifiedCanonicalKernelIrV12::from_canonical_bytes(
                graph.canonical().canonical_bytes().to_vec(),
            )
            .unwrap(),
            SimulationLimitsV1::default(),
        )
        .unwrap()
    };
    let original = admit(owner.original_source().executable());
    let final_graph = admit(owner.output());
    assert_eq!(original.module().kernels, final_graph.module().kernels);
    for kernel in &original.module().kernels {
        let mut grid = [1; 3];
        for (axis, extent) in kernel.domain.extents().enumerate() {
            let LaunchExtent::Static(value) = extent else {
                panic!("source static extent required")
            };
            grid[axis] = u64::from(value);
        }
        assert_eq!(kernel.domain.rank(), 1);
        let workgroup = kernel.workgroup_size.unwrap();
        let request = SimulationRequestV1::new(
            kernel.id.clone(),
            grid,
            [workgroup.x, workgroup.y, workgroup.z],
            vec![],
        );
        let invocations = grid.into_iter().try_fold(1u64, u64::checked_mul).unwrap();
        let count = usize::try_from(invocations).unwrap();
        assert!(count > 0);
        // Each real path has exactly one source store followed by one load.
        let limit = count.checked_mul(2).unwrap();
        let run = |module: &AdmittedSimulationModuleV1| {
            let mut trace = MemoryTrace {
                rows: Vec::with_capacity(limit),
                limit,
            };
            let execution = module
                .simulate_debugged_with_sink(
                    &request,
                    SimulationTargetV1::amdgpu_64(),
                    SimulationLimitsV1::default(),
                    SimulationDebugCaptureLimitsV1::new(8, 128, 8, 1024).unwrap(),
                    &mut trace,
                )
                .unwrap();
            let mut events = Events::default();
            let observed = module
                .simulate_observed_with_sink(
                    &request,
                    SimulationTargetV1::amdgpu_64(),
                    SimulationLimitsV1::default(),
                    &mut events,
                )
                .unwrap();
            assert_eq!(execution.arguments(), observed.arguments());
            assert_eq!(execution.invocations_executed(), invocations);
            assert_eq!(observed.invocations_executed(), invocations);
            assert_eq!(events.calls, count.checked_mul(calls).unwrap());
            assert_eq!(events.returns, count.checked_mul(calls + 1).unwrap());
            assert_eq!((events.reads, events.writes), (count, count));
            assert_eq!(trace.rows.len(), limit);
            for pair in trace.rows.chunks_exact(2) {
                assert_eq!(
                    pair[0].access,
                    SimulationDebugMemoryAccessV1::WriteCommitted
                );
                assert_eq!(pair[1].access, SimulationDebugMemoryAccessV1::Read);
                for row in pair {
                    assert_eq!(
                        (row.offset, row.bytes, row.value),
                        (
                            0,
                            8,
                            ScalarBitsV1::new(
                                fe2o3_kernel_ir::ScalarType::U64,
                                bits.into(),
                                SimulationTargetV1::amdgpu_64(),
                            )
                            .unwrap()
                        )
                    );
                }
            }
            (trace.rows, events)
        };
        assert_eq!(run(&original), run(&final_graph));
        assert_eq!(run(&final_graph), run(&final_graph));
    }
}

#[test]
fn real_scalar_memory_rewrite_preserves_nonempty_typed_private_execution() {
    for bits in [0, 1, u64::MAX] {
        let source = cpc_memory_source(true, true, true, bits.into());
        let original = source.executable().canonical().canonical_bytes().to_vec();
        let owner = cpc_prepare(source);
        assert_ne!(original, owner.output().canonical().canonical_bytes());
        compare_actual_execution(&owner, bits, 0);
    }
}

#[test]
fn shared_helper_calls_keep_real_private_access_values_for_every_source_root() {
    let owner = cpc_prepare(cpc_shared_source());
    assert_eq!(owner.output().module().kernels.len(), 2);
    compare_actual_execution(&owner, 11, 1);
}
