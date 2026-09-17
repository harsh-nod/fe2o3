use super::*;
use fe2o3_kir_sim::IndexWidthV1;

fn checked_guarded_index_module(scalar: ScalarType) -> Module {
    assert!(matches!(scalar, ScalarType::Index | ScalarType::U64));
    let arithmetic = Type::Scalar(scalar);
    let u32_ty = Type::Scalar(ScalarType::U32);
    let input = Type::slice(u32_ty.clone(), AddressSpace::Global, AccessMode::ReadOnly);
    let data = Type::pointer(u32_ty.clone(), AddressSpace::Global, AccessMode::ReadOnly);
    let output = Type::pointer(u32_ty.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let mask = Type::pointer(Type::BOOL, AddressSpace::Global, AccessMode::ReadWrite);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations.push(Operation::checked_binary(
        ValueDef::new(ValueId(5), arithmetic.clone()),
        ValueDef::new(ValueId(6), Type::BOOL),
        CheckedBinaryOperator::Add,
        ValueId(3),
        ValueId(4),
    ));
    block.operations.push(op(
        7,
        Type::INDEX,
        OperationKind::SliceLength { slice: ValueId(0) },
    ));
    let length = if scalar == ScalarType::U64 {
        block.operations.push(op(
            8,
            arithmetic.clone(),
            OperationKind::Cast {
                kind: CastKind::Bitcast,
                value: ValueId(7),
                to: arithmetic.clone(),
            },
        ));
        ValueId(8)
    } else {
        ValueId(7)
    };
    block.operations.extend([
        op(
            9,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(5),
                rhs: length,
            },
        ),
        op(
            10,
            Type::BOOL,
            OperationKind::Unary {
                op: UnaryOp::Not,
                operand: ValueId(6),
            },
        ),
        op(
            11,
            Type::BOOL,
            OperationKind::Binary {
                op: BinaryOp::BitAnd,
                lhs: ValueId(9),
                rhs: ValueId(10),
            },
        ),
        op(
            12,
            arithmetic.clone(),
            OperationKind::Constant(if scalar == ScalarType::U64 {
                Constant::U64(0)
            } else {
                Constant::Index(0)
            }),
        ),
        op(
            13,
            arithmetic,
            OperationKind::Select {
                condition: ValueId(11),
                true_value: ValueId(5),
                false_value: ValueId(12),
            },
        ),
    ]);
    let offset = if scalar == ScalarType::U64 {
        block.operations.push(op(
            14,
            Type::INDEX,
            OperationKind::Cast {
                kind: CastKind::Bitcast,
                value: ValueId(13),
                to: Type::INDEX,
            },
        ));
        ValueId(14)
    } else {
        ValueId(13)
    };
    block.operations.extend([
        op(
            15,
            data.clone(),
            OperationKind::SliceData { slice: ValueId(0) },
        ),
        op(
            16,
            data,
            OperationKind::GetElementPointer {
                base: ValueId(15),
                offset,
            },
        ),
        op(
            17,
            u32_ty.clone(),
            OperationKind::Constant(Constant::U32(0)),
        ),
        op(
            18,
            u32_ty,
            OperationKind::GuardedLoad {
                pointer: ValueId(16),
                predicate: ValueId(11),
                fallback: ValueId(17),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
        op(19, Type::INDEX, OperationKind::Constant(Constant::Index(1))),
        op(
            20,
            output.clone(),
            OperationKind::GetElementPointer {
                base: ValueId(1),
                offset: ValueId(19),
            },
        ),
        op(
            21,
            mask.clone(),
            OperationKind::GetElementPointer {
                base: ValueId(2),
                offset: ValueId(19),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(20),
                value: ValueId(18),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(21),
                value: ValueId(11),
                access: MemoryAccess::new(AddressSpace::Global, 1),
            },
        ),
    ]);
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("sim-tests::checked-guarded-index");
    module.functions.push(Function::kernel_entry(
        "checked_guarded_index_impl",
        Signature::new(
            vec![
                input,
                output,
                mask,
                Type::Scalar(scalar),
                Type::Scalar(scalar),
            ],
            vec![],
        ),
        (0..5).map(ValueId).collect(),
        vec![block],
    ));
    module.kernels.push(Kernel::new(
        "checked_guarded_index",
        "checked_guarded_index_impl",
        dynamic_domain_1d(),
    ));
    module
}

fn checked_guarded_index_request(
    input: &[u32],
    scalar: ScalarType,
    target: SimulationTargetV1,
    base: u64,
    offset: u64,
) -> SimulationRequestV1 {
    let bytes = input
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect::<Vec<_>>();
    let initialized = vec![true; bytes.len()];
    SimulationRequestV1::new(
        "checked_guarded_index",
        [1, 1, 1],
        [1, 1, 1],
        vec![
            SimulationArgumentV1::Buffer(
                BufferArgumentV1::new(
                    ScalarType::U32,
                    AccessMode::ReadOnly,
                    4,
                    bytes,
                    initialized,
                    target,
                )
                .unwrap(),
            ),
            SimulationArgumentV1::Buffer(u32_buffer(&[0xa5a5_a5a5; 3])),
            SimulationArgumentV1::Buffer(bool_buffer(&[true; 3])),
            SimulationArgumentV1::Scalar(
                ScalarBitsV1::new(scalar, u128::from(base), target).unwrap(),
            ),
            SimulationArgumentV1::Scalar(
                ScalarBitsV1::new(scalar, u128::from(offset), target).unwrap(),
            ),
        ],
    )
}

#[test]
fn checked_guarded_index_preserves_bounds_masks_and_non_speculative_reads() {
    for (scalar, target, max) in [
        (
            ScalarType::Index,
            SimulationTargetV1::little_endian(IndexWidthV1::Bits32),
            u64::from(u32::MAX),
        ),
        (ScalarType::Index, SimulationTargetV1::amdgpu_64(), u64::MAX),
        (ScalarType::U64, SimulationTargetV1::amdgpu_64(), u64::MAX),
    ] {
        let admitted = admitted(checked_guarded_index_module(scalar));
        for input in [&[][..], &[17_u32, 29, 43][..]] {
            for (base, offset) in [
                (0, 0),
                (0, 2),
                (1, 1),
                (0, 3),
                (2, 1),
                (max, 0),
                (max, 1),
                (max, 2),
                (max - 1, 1),
                (max / 2 + 1, 0),
                ((1_u64 << 32).min(max), 0),
            ] {
                // The independent oracle does not wrap the mathematical sum.
                let sum = u128::from(base) + u128::from(offset);
                let active = sum <= u128::from(max) && sum < input.len() as u128;
                let expected = if active { input[sum as usize] } else { 0 };
                let mut request =
                    checked_guarded_index_request(input, scalar, target, base, offset);
                request.events = EventPolicyV1::Enabled;
                let original = request.clone();
                let mut events = Collector::default();
                let result = admitted
                    .simulate_with_sink(
                        &request,
                        target,
                        SimulationLimitsV1 {
                            // Access records are per byte: four value bytes and one mask byte.
                            max_memory_access_records: 5,
                            ..SimulationLimitsV1::default()
                        },
                        &mut events,
                    )
                    .unwrap();
                assert_eq!(request, original);
                assert_eq!(words(result.buffer(0).unwrap().bytes()), input);
                assert_eq!(
                    words(result.buffer(1).unwrap().bytes()),
                    [0xa5a5_a5a5, expected, 0xa5a5_a5a5]
                );
                assert_eq!(result.buffer(2).unwrap().bytes(), &[1, u8::from(active), 1]);
                assert_eq!(
                    events
                        .0
                        .iter()
                        .filter(|event| matches!(
                            event.kind,
                            SimulationEventKindV1::MemoryRead { .. }
                        ))
                        .count(),
                    usize::from(active)
                );
                assert_eq!(
                    events
                        .0
                        .iter()
                        .filter(|event| matches!(
                            event.kind,
                            SimulationEventKindV1::MemoryWrite { .. }
                        ))
                        .count(),
                    2
                );
                if active {
                    assert!(matches!(
                        result.conflict_assessment(),
                        SimulationConflictAssessmentV1::Incomplete {
                            record_limit: 5,
                            ..
                        }
                    ));
                } else {
                    assert_eq!(
                        result.conflict_assessment(),
                        &SimulationConflictAssessmentV1::NoConflictsObserved
                    );
                }
            }
        }
    }
}

#[test]
fn checked_guarded_index_rejects_unequal_width_transport_before_execution() {
    let admitted = admitted(checked_guarded_index_module(ScalarType::U64));
    let target = SimulationTargetV1::little_endian(IndexWidthV1::Bits32);
    let request = checked_guarded_index_request(&[], ScalarType::U64, target, 0, 0);
    let error = admitted
        .preflight(&request, target, SimulationLimitsV1::default())
        .unwrap_err();
    let SimulationPreflightErrorV1::Unsupported(report) = error else {
        panic!("unexpected preflight error: {error:?}");
    };
    assert_eq!(report.findings().len(), 2);
    for (from, to) in [
        (ScalarType::Index, ScalarType::U64),
        (ScalarType::U64, ScalarType::Index),
    ] {
        assert!(report.findings().iter().any(|finding| finding.feature
            == UnsupportedFeatureV1::InvalidIntegerCast {
                from,
                to,
                kind: CastKind::Bitcast
            }));
    }
}
