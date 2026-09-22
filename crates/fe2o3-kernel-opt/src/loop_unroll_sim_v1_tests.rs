use super::fixture::*;
use super::*;
use fe2o3_kernel_ir::{
    AccessMode, FunctionRole, Kernel, LaunchDomain, LaunchExtent, MemoryAccess,
    VerifiedCanonicalKernelIrV12,
};
use fe2o3_kir_sim::{
    AdmittedSimulationModuleV1, BufferArgumentV1, ScalarBitsV1, SimulationArgumentV1,
    SimulationErrorV1, SimulationEventKindV1 as Kind, SimulationEventSinkErrorV1,
    SimulationEventSinkV1, SimulationEventV1 as Event, SimulationExecutionErrorKindV1,
    SimulationLimitsV1, SimulationRequestV1, SimulationTargetV1,
};
fn width(s: ScalarType) -> usize {
    match s {
        ScalarType::U8 => 1,
        ScalarType::U16 => 2,
        ScalarType::U32 => 4,
        ScalarType::U64 => 8,
        _ => panic!("SIM scalar"),
    }
}
fn store(pointer: u32, value: u32, s: ScalarType, space: AddressSpace) -> Operation {
    Operation::new(
        vec![],
        OperationKind::Store {
            pointer: ValueId(pointer),
            value: ValueId(value),
            access: MemoryAccess::new(space, width(s) as u32),
        },
    )
}
fn kernel(s: ScalarType, n: u64, diamond_body: bool, failure: u8) -> Module {
    let mut m = if diamond_body {
        diamond(s, n, true)
    } else {
        fixture(s, Some((0, n)), 1, true)
    };
    let f = &mut m.functions[0];
    f.role = FunctionRole::KernelEntry;
    f.signature.parameters.extend([
        Type::pointer(scalar(s), AddressSpace::Global, AccessMode::ReadWrite),
        Type::pointer(scalar(s), AddressSpace::Global, AccessMode::ReadWrite),
    ]);
    let body = f.body.as_mut().unwrap();
    body.parameters.extend([ValueId(3), ValueId(4)]);
    body.blocks[0].operations.push(Operation::effect_free(
        ValueDef::new(
            ValueId(50),
            Type::pointer(scalar(s), AddressSpace::Private, AccessMode::ReadWrite),
        ),
        OperationKind::Alloca {
            element: scalar(s),
            count: None,
            address_space: AddressSpace::Private,
            alignment: width(s) as u32,
        },
    ));
    if failure != 2 {
        body.blocks[0]
            .operations
            .push(store(50, 11, s, AddressSpace::Private));
    }
    body.blocks[1]
        .operations
        .push(store(3, 20, s, AddressSpace::Global));
    body.blocks[2].operations.extend([
        Operation::effect_free(
            ValueDef::new(ValueId(60), scalar(s)),
            OperationKind::Load {
                pointer: ValueId(50),
                access: MemoryAccess::new(AddressSpace::Private, width(s) as u32),
            },
        ),
        store(4, 25, s, AddressSpace::Global),
        store(50, 25, s, AddressSpace::Private),
    ]);
    if failure == 1 {
        body.blocks[0].operations.push(literal(51, s, 2));
        body.blocks[2].operations.extend([
            Operation::effect_free(
                ValueDef::new(ValueId(61), Type::BOOL),
                OperationKind::Compare {
                    predicate: ComparePredicate::Equal,
                    lhs: ValueId(20),
                    rhs: ValueId(51),
                },
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(62), scalar(s)),
                OperationKind::Select {
                    condition: ValueId(61),
                    true_value: ValueId(11),
                    false_value: ValueId(10),
                },
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(63), scalar(s)),
                OperationKind::Binary {
                    op: BinaryOp::Divide,
                    lhs: ValueId(20),
                    rhs: ValueId(62),
                },
            ),
        ]);
    }
    m.kernels.push(Kernel::new(
        "unroll",
        "unroll",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    m
}
fn sim(owner: &Owner) -> AdmittedSimulationModuleV1 {
    let bytes = VerifiedCanonicalKernelIrV12::from_canonical_bytes(
        owner.canonical().canonical_bytes().to_vec(),
    )
    .unwrap();
    assert_eq!(bytes.identity(), owner.canonical().identity());
    AdmittedSimulationModuleV1::admit_v12(bytes, SimulationLimitsV1::default()).unwrap()
}
fn request(s: ScalarType, control: bool) -> SimulationRequestV1 {
    let target = SimulationTargetV1::amdgpu_64();
    let buffer = || {
        SimulationArgumentV1::Buffer(
            BufferArgumentV1::new(
                s,
                AccessMode::ReadWrite,
                width(s) as u32,
                vec![0xa5; width(s) * 2],
                vec![false; width(s) * 2],
                target,
            )
            .unwrap(),
        )
    };
    SimulationRequestV1::new(
        "unroll",
        [1, 1, 1],
        [1, 1, 1],
        vec![
            SimulationArgumentV1::Scalar(ScalarBitsV1::new(s, 99, target).unwrap()),
            SimulationArgumentV1::Scalar(ScalarBitsV1::new(s, 88, target).unwrap()),
            SimulationArgumentV1::Scalar(
                ScalarBitsV1::new(ScalarType::Bool, u128::from(control), target).unwrap(),
            ),
            buffer(),
            buffer(),
        ],
    )
}
#[derive(Default)]
struct Events(Vec<Event>);
impl SimulationEventSinkV1 for Events {
    fn record(&mut self, e: &Event) -> std::result::Result<(), SimulationEventSinkErrorV1> {
        if self.0.len() == 16_384 {
            return Err(SimulationEventSinkErrorV1 {
                detail: "complete unroll event roster".into(),
            });
        }
        self.0.push(e.clone());
        Ok(())
    }
}
fn old_block(pair: &Pair<'_, '_, '_>, function: usize, id: BlockId) -> Block {
    let body = pair.output().module().functions[function]
        .body
        .as_ref()
        .unwrap();
    let ordinal = body.blocks.iter().position(|b| b.id == id).unwrap();
    let output = Block {
        function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(function as u32),
        block: ordinal as u32,
    };
    let mut rows = pair
        .origins()
        .blocks
        .iter()
        .filter(|r| r.output == Some(output));
    let row = *rows.next().unwrap();
    assert!(rows.next().is_none());
    row.input
}
fn old_id(pair: &Pair<'_, '_, '_>, block: Block) -> BlockId {
    pair.input().module().functions[block.function.0 as usize]
        .body
        .as_ref()
        .unwrap()
        .blocks[block.block as usize]
        .id
}
fn translated(pair: &Pair<'_, '_, '_>, events: &[Event]) -> Vec<Event> {
    events
        .iter()
        .map(|event| {
            let mut mapped = event.clone();
            let original = old_block(pair, event.site.function_ordinal, event.site.block);
            if let Some(operation) = event.site.operation {
                let out_body = pair.output().module().functions[event.site.function_ordinal]
                    .body
                    .as_ref()
                    .unwrap();
                let out_block = out_body
                    .blocks
                    .iter()
                    .position(|b| b.id == event.site.block)
                    .unwrap();
                let output = Site {
                    block: Block {
                        function: original.function,
                        block: out_block as u32,
                    },
                    operation,
                };
                let mut rows = pair
                    .origins()
                    .operations
                    .iter()
                    .filter(|r| r.output == Some(output));
                let row = rows.next().unwrap();
                assert!(rows.next().is_none());
                assert_eq!(row.input.block, original);
                mapped.site.operation = Some(row.input.operation);
            }
            mapped.site.block = old_id(pair, original);
            if let Kind::Branch { target } = &mut mapped.kind {
                *target = old_id(pair, old_block(pair, event.site.function_ordinal, *target));
            }
            mapped
        })
        .collect()
}

#[test]
fn bounded_unroll_sim_full_events_header_body_effect_counts_and_cpu_buffers() {
    let mut runs = 0usize;
    for s in [
        ScalarType::U8,
        ScalarType::U16,
        ScalarType::U32,
        ScalarType::U64,
    ] {
        for n in [0, 1, 2, 3, 8] {
            for diamond_body in [false, true] {
                with_input(kernel(s, n, diamond_body, 0), |input, budget| {
                    let owner = run(input, budget);
                    assert_eq!(owner.origins().selection.unwrap().iterations, n as u8);
                    let receipt = {
                        let (pair, receipt) = owner.replay(input, owner.limits(), budget).unwrap();
                        budget.reserve_storage(receipt.retained_storage()).unwrap();
                        let before = sim(input);
                        let after = sim(owner.output());
                        for control in [false, true] {
                            let request = request(s, control);
                            let unchanged = request.clone();
                            for _ in 0..2 {
                                let mut a = Events::default();
                                let mut b = Events::default();
                                let old = before
                                    .simulate_observed_with_sink(
                                        &request,
                                        SimulationTargetV1::amdgpu_64(),
                                        SimulationLimitsV1::default(),
                                        &mut a,
                                    )
                                    .unwrap();
                                let new = after
                                    .simulate_observed_with_sink(
                                        &request,
                                        SimulationTargetV1::amdgpu_64(),
                                        SimulationLimitsV1::default(),
                                        &mut b,
                                    )
                                    .unwrap();
                                runs += 2;
                                assert_eq!(old.arguments(), new.arguments());
                                assert_eq!(old.conflict_assessment(), new.conflict_assessment());
                                assert_eq!(a.0, translated(&pair, &b.0));
                                assert_eq!(
                                    a.0.iter()
                                        .filter(|e| e.site.block == BlockId(41)
                                            && matches!(e.kind, Kind::MemoryWrite { .. }))
                                        .count(),
                                    n as usize + 1
                                );
                                assert_eq!(
                                    a.0.iter()
                                        .filter(|e| e.site.block == BlockId(97)
                                            && matches!(e.kind, Kind::MemoryRead { .. }))
                                        .count(),
                                    n as usize
                                );
                                assert_eq!(
                                    a.0.iter()
                                        .filter(|e| e.site.block == BlockId(97)
                                            && matches!(e.kind, Kind::MemoryWrite { .. }))
                                        .count(),
                                    n as usize * 2
                                );
                                for result in [&old, &new] {
                                    assert_eq!(result.invocations_executed(), 1);
                                    assert!(!result.grants_execution_authority());
                                    for (arg, last, initialized) in
                                        [(3, n, true), (4, n.saturating_sub(1), n != 0)]
                                    {
                                        let actual = result.buffer(arg).unwrap();
                                        let bytes = last.to_le_bytes();
                                        let expected = if initialized {
                                            bytes[..width(s)].to_vec()
                                        } else {
                                            vec![0xa5; width(s)]
                                        };
                                        assert_eq!(&actual.bytes()[..width(s)], expected);
                                        assert_eq!(
                                            &actual.bytes()[width(s)..],
                                            vec![0xa5; width(s)]
                                        );
                                        assert_eq!(
                                            &actual.initialized()[..width(s)],
                                            vec![initialized; width(s)]
                                        );
                                        assert_eq!(
                                            &actual.initialized()[width(s)..],
                                            vec![false; width(s)]
                                        );
                                    }
                                }
                                assert_eq!(request, unchanged);
                            }
                        }
                        receipt.retained_storage()
                    };
                    budget.release_storage(receipt).unwrap();
                    release(owner, budget);
                });
            }
        }
    }
    assert_eq!(runs, 320);
}
#[test]
fn bounded_unroll_sim_partial_division_and_uninitialized_read_preserve_every_event() {
    let mut runs = 0usize;
    for failure in [1, 2] {
        for diamond_body in [false, true] {
            with_input(
                kernel(ScalarType::U32, 3, diamond_body, failure),
                |input, budget| {
                    let owner = run(input, budget);
                    assert_eq!(owner.origins().selection.unwrap().iterations, 3);
                    let receipt = {
                        let (pair, receipt) = owner.replay(input, owner.limits(), budget).unwrap();
                        budget.reserve_storage(receipt.retained_storage()).unwrap();
                        let before = sim(input);
                        let after = sim(owner.output());
                        for control in [false, true] {
                            let request = request(ScalarType::U32, control);
                            let mut a = Events::default();
                            let mut b = Events::default();
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
                            let (
                                SimulationErrorV1::Execution(first),
                                SimulationErrorV1::Execution(mut last),
                            ) = (first, last)
                            else {
                                panic!("actual dynamic failure");
                            };
                            assert_eq!(last.observation_failure, None);
                            let site = last.site.as_mut().unwrap();
                            let f = pair
                                .output()
                                .module()
                                .functions
                                .iter()
                                .position(|f| f.id == site.function)
                                .unwrap();
                            let old = old_block(&pair, f, site.block);
                            site.block = old_id(&pair, old);
                            // Every retained operation keeps its ordinal within each clone.
                            assert_eq!(first, last);
                            assert_eq!(first.site.as_ref().unwrap().block, BlockId(97));
                            if failure == 1 {
                                assert!(matches!(
                                    first.kind,
                                    SimulationExecutionErrorKindV1::UndefinedIntegerOperation(_)
                                ));
                            } else {
                                assert!(matches!(
                                    first.kind,
                                    SimulationExecutionErrorKindV1::UninitializedRead { .. }
                                ));
                            }
                            assert_eq!(a.0, translated(&pair, &b.0));
                            assert_eq!(
                                a.0.iter()
                                    .filter(|e| e.site.block == BlockId(41)
                                        && matches!(e.kind, Kind::MemoryWrite { .. }))
                                    .count(),
                                if failure == 1 { 3 } else { 1 }
                            );
                        }
                        receipt.retained_storage()
                    };
                    budget.release_storage(receipt).unwrap();
                    release(owner, budget);
                },
            );
        }
    }
    assert_eq!(runs, 16);
}

#[path = "loop_unroll_dead_exit_v1_tests.rs"]
mod dead_exit;
