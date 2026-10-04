use super::*;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, AmdGpuDiagnosticOperation, FunctionRole, Kernel, LaunchDomain,
    LaunchExtent, MemoryAccess, VerifiedCanonicalKernelIrV12,
};
use fe2o3_kir_sim::{
    AdmittedSimulationModuleV1, BufferArgumentV1, ScalarBitsV1, SimulationArgumentV1,
    SimulationErrorV1, SimulationEventKindV1 as EventKind, SimulationEventSinkErrorV1,
    SimulationEventSinkV1, SimulationEventV1, SimulationExecutionErrorKindV1, SimulationLimitsV1,
    SimulationRequestV1, SimulationTargetV1,
};
fn width(s: ScalarType) -> usize {
    match s {
        ScalarType::U8 => 1,
        ScalarType::U16 => 2,
        ScalarType::U32 => 4,
        ScalarType::U64 => 8,
        _ => panic!("SIM integer"),
    }
}
fn maximum(s: ScalarType) -> u64 {
    match s {
        ScalarType::U8 => u8::MAX as u64,
        ScalarType::U16 => u16::MAX as u64,
        ScalarType::U32 => u32::MAX as u64,
        ScalarType::U64 => u64::MAX,
        _ => panic!("SIM integer"),
    }
}
fn kernel(s: ScalarType, literals: Option<(u64, u64)>, step: u64) -> Module {
    let mut module = fixture(s, literals, step, true);
    let function = &mut module.functions[0];
    function.id = "refine_impl".into();
    function.role = FunctionRole::KernelEntry;
    function.signature.parameters.extend([
        Type::pointer(scalar(s), AddressSpace::Global, AccessMode::ReadWrite),
        Type::pointer(scalar(s), AddressSpace::Global, AccessMode::ReadWrite),
    ]);
    let body = function.body.as_mut().unwrap();
    body.parameters.extend([ValueId(100), ValueId(101)]);
    body.blocks[0].operations.push(literal(13, s, 0));
    body.blocks[2].operations.extend([
        Operation::effect_free(
            ValueDef::new(ValueId(32), scalar(s)),
            OperationKind::Select {
                condition: ValueId(31),
                true_value: ValueId(10),
                false_value: ValueId(13),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(100),
                value: ValueId(30),
                access: MemoryAccess::new(AddressSpace::Global, width(s).try_into().unwrap()),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(101),
                value: ValueId(32),
                access: MemoryAccess::new(AddressSpace::Global, width(s).try_into().unwrap()),
            },
        ),
    ]);
    body.blocks[2].terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(31),
        then_target: BlockId(900),
        then_arguments: vec![],
        else_target: BlockId(41),
        else_arguments: vec![ValueId(30)],
    });
    body.blocks.push(block(
        900,
        vec![],
        vec![AmdGpuDiagnosticOperation::Trap.operation(None)],
        Terminator::Unreachable,
    ));
    module
        .functions
        .push(AmdGpuDiagnosticOperation::Trap.declaration());
    module.kernels.push(Kernel::new(
        "refine",
        "refine_impl",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    module
}
fn sim(owner: &Owner) -> AdmittedSimulationModuleV1 {
    let bytes = VerifiedCanonicalKernelIrV12::from_canonical_bytes(
        owner.canonical().canonical_bytes().to_vec(),
    )
    .unwrap();
    assert_eq!(bytes.identity(), owner.canonical().identity());
    AdmittedSimulationModuleV1::admit_v12(bytes, SimulationLimitsV1::default()).unwrap()
}
fn request(s: ScalarType, a: u64, b: u64) -> SimulationRequestV1 {
    let target = SimulationTargetV1::amdgpu_64();
    let buffer = || {
        SimulationArgumentV1::Buffer(
            BufferArgumentV1::new(
                s,
                AccessMode::ReadWrite,
                width(s).try_into().unwrap(),
                vec![0xa5; width(s) * 2],
                vec![false; width(s) * 2],
                target,
            )
            .unwrap(),
        )
    };
    SimulationRequestV1::new(
        "refine",
        [1, 1, 1],
        [1, 1, 1],
        vec![
            SimulationArgumentV1::Scalar(ScalarBitsV1::new(s, a as u128, target).unwrap()),
            SimulationArgumentV1::Scalar(ScalarBitsV1::new(s, b as u128, target).unwrap()),
            buffer(),
            buffer(),
        ],
    )
}
#[derive(Default)]
struct Effects(Vec<SimulationEventV1>);
impl SimulationEventSinkV1 for Effects {
    fn record(
        &mut self,
        event: &SimulationEventV1,
    ) -> std::result::Result<(), SimulationEventSinkErrorV1> {
        // The refinement intentionally adds one pure operation lifecycle. Every
        // memory, control, call, allocation and invocation event stays observable.
        if !matches!(
            event.kind,
            EventKind::OperationBegin | EventKind::OperationEnd { .. }
        ) {
            if self.0.len() == 4096 {
                return Err(SimulationEventSinkErrorV1 {
                    detail: "complete refinement effects".into(),
                });
            }
            self.0.push(event.clone());
        }
        Ok(())
    }
}
fn translated(pair: &Pair<'_>, events: &[SimulationEventV1]) -> Vec<SimulationEventV1> {
    events
        .iter()
        .map(|event| {
            let mut result = event.clone();
            let f = event.site.function_ordinal;
            let a = pair.input().module().functions[f].body.as_ref().unwrap();
            let b = pair.output().module().functions[f].body.as_ref().unwrap();
            let block = a
                .blocks
                .iter()
                .position(|b| b.id == event.site.block)
                .unwrap();
            assert_eq!(a.blocks[block].id, b.blocks[block].id);
            if let Some(operation) = event.site.operation {
                let mut matching = pair.origins().iter().filter(|r| {
                    r.input().block.function.0 as usize == f
                        && r.input().block.block as usize == block
                        && r.input().operation == operation
                });
                let row = matching.next().unwrap();
                assert!(matching.next().is_none());
                let Row::Unchanged { input, output } = row else {
                    panic!("refined pure arithmetic cannot produce an effect")
                };
                assert_eq!(input.block, output.block);
                assert_eq!(
                    a.blocks[block].operations[operation as usize],
                    b.blocks[block].operations[output.operation as usize]
                );
                result.site.operation = Some(output.operation);
            } else {
                assert_eq!(a.blocks[block].terminator, b.blocks[block].terminator);
            }
            result
        })
        .collect()
}
fn compare_case(
    s: ScalarType,
    literals: Option<(u64, u64)>,
    step: u64,
    requests: &[(u64, u64)],
    dedicated_latch: bool,
) -> usize {
    let mut runs = 0;
    let mut module = kernel(s, literals, step);
    if dedicated_latch {
        // Preserve the overflow exit while giving recurrence discovery a canonical latch.
        let body = module.functions[0].body.as_mut().unwrap();
        let Some(Terminator::ConditionalBranch {
            else_target,
            else_arguments,
            ..
        }) = &mut body.blocks[2].terminator
        else {
            panic!("checked update overflow branch")
        };
        assert_eq!(*else_target, BlockId(41));
        assert_eq!(else_arguments, &[ValueId(30)]);
        *else_target = BlockId(811);
        else_arguments.clear();
        body.blocks
            .push(block(811, vec![], vec![], branch(41, &[30])));
    }
    with_input(module, |input, budget| {
        let owner =
            prepare_owned_induction_refinement_v1(input, Limits::default(), budget).unwrap();
        budget.reserve_storage(owner.retained_storage()).unwrap();
        assert_eq!(selected(&owner), usize::from(dedicated_latch));
        if !dedicated_latch {
            assert_eq!(
                input.canonical().canonical_bytes(),
                owner.output().canonical().canonical_bytes(),
            );
        }
        let retained = {
            let (pair, receipt) = owner
                .replay_against(input, Limits::default(), budget)
                .unwrap();
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            let before = sim(input);
            let after = sim(owner.output());
            for &(initial, bound) in requests {
                let request = request(s, initial, bound);
                let unchanged = request.clone();
                let (a, b) = literals.unwrap_or((initial, bound));
                let n = if a >= b {
                    0
                } else {
                    (b as u128 - a as u128).div_ceil(step as u128)
                };
                let last = (a as u128 + n * step as u128) as u64;
                for _ in 0..2 {
                    let mut old = Effects::default();
                    let mut new = Effects::default();
                    let a = before
                        .simulate_observed_with_sink(
                            &request,
                            SimulationTargetV1::amdgpu_64(),
                            SimulationLimitsV1::default(),
                            &mut old,
                        )
                        .unwrap();
                    let b = after
                        .simulate_observed_with_sink(
                            &request,
                            SimulationTargetV1::amdgpu_64(),
                            SimulationLimitsV1::default(),
                            &mut new,
                        )
                        .unwrap();
                    runs += 2;
                    assert_eq!(a.arguments(), b.arguments());
                    assert_eq!(translated(&pair, &old.0), new.0);
                    assert_eq!(a.conflict_assessment(), b.conflict_assessment());
                    assert_eq!(
                        old.0
                            .iter()
                            .filter(|e| matches!(e.kind, EventKind::MemoryWrite { .. }))
                            .count(),
                        usize::try_from(n * 2).unwrap()
                    );
                    for result in [&a, &b] {
                        for (argument, value) in [(2, last), (3, 0)] {
                            let buffer = result.buffer(argument).unwrap();
                            let bytes = value.to_le_bytes();
                            let expected = if n == 0 {
                                vec![0xa5; width(s)]
                            } else {
                                bytes[..width(s)].to_vec()
                            };
                            assert_eq!(&buffer.bytes()[..width(s)], expected);
                            assert_eq!(&buffer.bytes()[width(s)..], vec![0xa5; width(s)]);
                            assert_eq!(&buffer.initialized()[..width(s)], vec![n != 0; width(s)]);
                            assert_eq!(&buffer.initialized()[width(s)..], vec![false; width(s)]);
                        }
                        assert_eq!(result.invocations_executed(), 1);
                        assert!(!result.grants_execution_authority());
                    }
                    assert_eq!(request, unchanged);
                }
            }
            receipt.retained_storage()
        };
        budget.release_storage(retained).unwrap();
        release(owner, budget);
    });
    runs
}
#[test]
fn induction_refinement_sim_sum_overflow_uses_and_complete_effects_match_cpu() {
    let mut runs = 0;
    for s in [
        ScalarType::U8,
        ScalarType::U16,
        ScalarType::U32,
        ScalarType::U64,
    ] {
        let m = maximum(s);
        runs += compare_case(
            s,
            None,
            1,
            &[(0, 0), (0, 1), (0, 3), (2, 2), (3, 1), (m - 1, m), (m, m)],
            true,
        );
    }
    for (s, a, b, step) in [
        (ScalarType::U8, 1, 8, 3),
        (ScalarType::U8, 250, 255, 1),
        (ScalarType::U16, 65526, 65535, 3),
        (ScalarType::U32, u32::MAX as u64 - 9, u32::MAX as u64, 3),
        (ScalarType::U64, u64::MAX - 9, u64::MAX, 3),
    ] {
        runs += compare_case(s, Some((a, b)), step, &[(0, 0)], true);
    }
    assert_eq!(runs, 132);
}
#[test]
fn induction_refinement_sim_conditional_latch_remains_an_exact_noop() {
    let mut runs = 0;
    for s in [
        ScalarType::U8,
        ScalarType::U16,
        ScalarType::U32,
        ScalarType::U64,
    ] {
        let m = maximum(s);
        runs += compare_case(
            s,
            None,
            1,
            &[(0, 0), (0, 1), (0, 3), (2, 2), (3, 1), (m - 1, m), (m, m)],
            false,
        );
    }
    assert_eq!(runs, 112);
}
#[test]
fn induction_refinement_sim_unselected_overflow_and_traps_are_unchanged() {
    let mut runs = 0;
    with_input(kernel(ScalarType::U8, None, 2), |input, budget| {
        let owner =
            prepare_owned_induction_refinement_v1(input, Limits::default(), budget).unwrap();
        budget.reserve_storage(owner.retained_storage()).unwrap();
        assert_eq!(selected(&owner), 0);
        assert_eq!(
            input.canonical().canonical_bytes(),
            owner.output().canonical().canonical_bytes()
        );
        let retained = {
            let (pair, receipt) = owner
                .replay_against(input, Limits::default(), budget)
                .unwrap();
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            let before = sim(input);
            let after = sim(owner.output());
            for _ in 0..2 {
                let request = request(ScalarType::U8, 250, 255);
                let mut a = Effects::default();
                let mut b = Effects::default();
                let first = before
                    .simulate_observed_with_sink(
                        &request,
                        SimulationTargetV1::amdgpu_64(),
                        SimulationLimitsV1::default(),
                        &mut a,
                    )
                    .unwrap_err();
                let last = after
                    .simulate_observed_with_sink(
                        &request,
                        SimulationTargetV1::amdgpu_64(),
                        SimulationLimitsV1::default(),
                        &mut b,
                    )
                    .unwrap_err();
                runs += 2;
                let (SimulationErrorV1::Execution(first), SimulationErrorV1::Execution(last)) =
                    (first, last)
                else {
                    panic!("actual overflow reaches retained trap")
                };
                assert_eq!(first, last);
                assert_eq!(
                    first.kind,
                    SimulationExecutionErrorKindV1::ReachedUnreachable
                );
                assert_eq!(first.site.unwrap().block, BlockId(900));
                assert_eq!(translated(&pair, &a.0), b.0);
                assert_eq!(
                    a.0.iter()
                        .filter(|e| matches!(e.kind, EventKind::MemoryWrite { .. }))
                        .count(),
                    6
                );
            }
            receipt.retained_storage()
        };
        budget.release_storage(retained).unwrap();
        release(owner, budget);
    });
    assert_eq!(runs, 4);
}
