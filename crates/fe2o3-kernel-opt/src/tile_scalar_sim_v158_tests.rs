use super::*;
use fe2o3_kernel_ir::{Axis, IndexKind, IntrinsicKind, IntrinsicOperation, MemoryAccess};
use fe2o3_kir_sim::{
    AdmittedSimulationModuleV1, BufferArgumentV1, ScalarBitsV1, SimulationArgumentV1,
    SimulationEventKindV1, SimulationEventSinkErrorV1, SimulationEventSinkV1, SimulationEventV1,
    SimulationLimitsV1, SimulationRequestV1, SimulationTargetV1,
};

fn scalar(id: u32, ty: Type, kind: Kind) -> Operation {
    Operation::effect_free(ValueDef::new(ValueId(id), ty), kind)
}

fn observable_fixture() -> Module {
    let mut module = fixture();
    let function = &mut module.functions[0];
    function.signature.parameters.push(Type::slice(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    ));
    let body = function.body.as_mut().unwrap();
    body.parameters.push(ValueId(4));
    let operations = &mut body.blocks[1].operations;
    let end = operations.pop().unwrap();
    let pointer = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    );
    operations.extend([
        scalar(200, pointer.clone(), Kind::SliceData { slice: ValueId(4) }),
        scalar(
            201,
            Type::INDEX,
            Kind::Intrinsic(IntrinsicOperation::new(
                IntrinsicKind::InvocationIndex {
                    kind: IndexKind::Local,
                    axis: Axis::X,
                },
                Type::INDEX,
            )),
        ),
        scalar(202, Type::INDEX, Kind::Constant(Constant::Index(6))),
        scalar(
            203,
            Type::INDEX,
            Kind::Binary {
                op: BinaryOp::Multiply,
                lhs: ValueId(201),
                rhs: ValueId(202),
            },
        ),
        scalar(
            204,
            Type::Scalar(ScalarType::U32),
            Kind::Constant(Constant::U32(0)),
        ),
        scalar(
            205,
            Type::Scalar(ScalarType::U32),
            Kind::Constant(Constant::U32(1)),
        ),
    ]);
    for component in 0..6 {
        let first = 210 + 4 * component;
        let value = if component < 3 {
            ValueId(20 + component)
        } else {
            operations.push(scalar(
                first + 3,
                Type::Scalar(ScalarType::U32),
                Kind::Select {
                    condition: ValueId(20 + component),
                    true_value: ValueId(205),
                    false_value: ValueId(204),
                },
            ));
            ValueId(first + 3)
        };
        operations.extend([
            scalar(
                first,
                Type::INDEX,
                Kind::Constant(Constant::Index(u64::from(component))),
            ),
            scalar(
                first + 1,
                Type::INDEX,
                Kind::Binary {
                    op: BinaryOp::Add,
                    lhs: ValueId(203),
                    rhs: ValueId(first),
                },
            ),
            scalar(
                first + 2,
                pointer.clone(),
                Kind::GetElementPointer {
                    base: ValueId(200),
                    offset: ValueId(first + 1),
                },
            ),
            Operation::new(
                vec![],
                Kind::Store {
                    pointer: ValueId(first + 2),
                    value,
                    access: MemoryAccess::new(AddressSpace::Global, 4),
                },
            ),
        ]);
    }
    operations.push(end);
    module
}

#[derive(Default)]
struct Reads(Vec<(u32, BlockId, usize, usize)>);

impl SimulationEventSinkV1 for Reads {
    fn record(
        &mut self,
        event: &SimulationEventV1,
    ) -> std::result::Result<(), SimulationEventSinkErrorV1> {
        if let SimulationEventKindV1::MemoryRead { offset, bytes, .. } = event.kind {
            if self.0.len() == 384 {
                return Err(SimulationEventSinkErrorV1 {
                    detail: "complete tile read bound".into(),
                });
            }
            self.0
                .push((event.invocation.local[0], event.site.block, offset, bytes));
        }
        Ok(())
    }
}

#[test]
fn tile_scalar_whole_graph_simulates_values_masks_and_original_discarded_read_sites() {
    let target = SimulationTargetV1::amdgpu_64();
    for layout in [
        ExecutionTileLayoutV1::Blocked,
        ExecutionTileLayoutV1::Striped,
    ] {
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        let input = owner(&observable_fixture(), &mut budget);
        let candidate =
            prepare_owned_tile_scalar_v18(&input, &selection(layout), LAYOUTS, &mut budget)
                .unwrap();
        budget
            .reserve_storage(candidate.retained_storage())
            .unwrap();
        candidate.replay_against(&input, &mut budget).unwrap();
        let (module, storage) = AdmittedSimulationModuleV1::admit_v18_with_verification_budget(
            candidate.output(),
            SimulationLimitsV1::default(),
            &mut budget,
        )
        .unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        for length in [0_usize, 1, 5, 191, 192] {
            let values: Vec<u32> = (0..length)
                .map(|i| (i as u32).wrapping_mul(17) + 9)
                .collect();
            let bytes: Vec<u8> = values
                .iter()
                .flat_map(|value| value.to_le_bytes())
                .collect();
            for base in [0, 3, u64::MAX - 1, u64::MAX] {
                for branch in [false, true] {
                    let request = SimulationRequestV1::new(
                        "entry",
                        [64, 1, 1],
                        [64, 1, 1],
                        vec![
                            SimulationArgumentV1::Buffer(
                                BufferArgumentV1::new(
                                    ScalarType::U32,
                                    AccessMode::ReadOnly,
                                    4,
                                    bytes.clone(),
                                    vec![true; bytes.len()],
                                    target,
                                )
                                .unwrap(),
                            ),
                            SimulationArgumentV1::Scalar(
                                ScalarBitsV1::index(base, target).unwrap(),
                            ),
                            SimulationArgumentV1::Scalar(ScalarBitsV1::boolean(branch)),
                            SimulationArgumentV1::Buffer(
                                BufferArgumentV1::new(
                                    ScalarType::U32,
                                    AccessMode::ReadWrite,
                                    4,
                                    vec![0xa5; 64 * 6 * 4],
                                    vec![false; 64 * 6 * 4],
                                    target,
                                )
                                .unwrap(),
                            ),
                        ],
                    );
                    let unchanged = request.clone();
                    let mut reads = Reads::default();
                    let actual = module
                        .simulate_observed_with_sink(
                            &request,
                            target,
                            SimulationLimitsV1::default(),
                            &mut reads,
                        )
                        .unwrap();
                    let mut expected = Vec::new();
                    let mut expected_reads = Vec::new();
                    for lane in 0..64_u32 {
                        let mut lane_values = [0_u32; 3];
                        let mut lane_masks = [0_u32; 3];
                        let mut offsets = Vec::new();
                        for component in 0..3_usize {
                            let offset = match layout {
                                ExecutionTileLayoutV1::Blocked => {
                                    u128::from(lane) * 3 + component as u128
                                }
                                ExecutionTileLayoutV1::Striped => {
                                    component as u128 * 64 + u128::from(lane)
                                }
                            };
                            let index = u128::from(base) + offset;
                            if index <= u128::from(u64::MAX) && index < length as u128 {
                                lane_values[component] = values[index as usize];
                                lane_masks[component] = 1;
                                offsets.push(index as usize * 4);
                            }
                        }
                        expected.extend(
                            lane_values
                                .into_iter()
                                .chain(lane_masks)
                                .flat_map(u32::to_le_bytes),
                        );
                        // The discarded load still performs every active read at its original site.
                        for _ in 0..2 {
                            expected_reads.extend(
                                offsets.iter().map(|&offset| (lane, BlockId(7), offset, 4)),
                            );
                        }
                    }
                    let output = actual.buffer(3).unwrap();
                    assert_eq!(
                        output.bytes(),
                        expected,
                        "{layout:?} length={length} base={base} branch={branch}"
                    );
                    assert!(output.initialized().iter().all(|&byte| byte));
                    assert_eq!(reads.0, expected_reads);
                    assert_eq!(actual.invocations_executed(), 64);
                    assert!(!actual.grants_execution_authority());
                    assert_eq!(request, unchanged);
                }
            }
        }
        drop(module);
        budget.release_storage(storage.retained_storage()).unwrap();
    }
}
