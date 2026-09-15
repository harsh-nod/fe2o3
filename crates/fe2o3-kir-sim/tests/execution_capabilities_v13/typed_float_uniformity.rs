use super::*;

fn effect(id: u32, ty: Type, kind: OperationKind) -> Operation {
    Operation::effect_free(ValueDef::new(ValueId(id), ty), kind)
}

fn lane_nan_inputs(module: &mut Module) {
    let operations = &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations;
    let position = operations
        .iter()
        .position(|op| {
            matches!(&op.kind,
                OperationKind::ExecutionCapability(contract) if matches!(contract.operation,
                    ExecutionCapabilityOperationV1::SubgroupCollective { .. }
                    | ExecutionCapabilityOperationV1::WorkgroupCollective { .. }
                )
            )
        })
        .unwrap();
    let OperationKind::ExecutionCapability(contract) = &mut operations[position].kind else {
        unreachable!()
    };
    *contract.operands.last_mut().unwrap() = ValueId(25);
    operations.splice(
        position..position,
        [
            effect(
                20,
                Type::INDEX,
                OperationKind::Intrinsic(IntrinsicOperation::new(
                    IntrinsicKind::InvocationIndex {
                        kind: IndexKind::Local,
                        axis: Axis::X,
                    },
                    Type::INDEX,
                )),
            ),
            effect(21, Type::INDEX, OperationKind::Constant(Constant::Index(0))),
            effect(
                22,
                Type::BOOL,
                OperationKind::Compare {
                    predicate: ComparePredicate::Equal,
                    lhs: ValueId(20),
                    rhs: ValueId(21),
                },
            ),
            effect(
                23,
                Type::F32,
                OperationKind::Constant(Constant::F32Bits(0x7fc0_0001)),
            ),
            effect(
                24,
                Type::F32,
                OperationKind::Constant(Constant::F32Bits(0x7fc0_0042)),
            ),
            effect(
                25,
                Type::F32,
                OperationKind::Select {
                    condition: ValueId(22),
                    true_value: ValueId(23),
                    false_value: ValueId(24),
                },
            ),
        ],
    );
}

fn simulate_f32(module: Module, kernel: &str, lanes: u32, input: u32) -> Vec<u32> {
    let target = SimulationTargetV1::amdgpu_64();
    let scalar = |bits| ScalarBitsV1::new(ScalarType::F32, u128::from(bits), target).unwrap();
    let canonical = VerifiedCanonicalKernelIrV13::from_module(module).unwrap();
    let admitted =
        AdmittedSimulationModuleV1::admit_v13(canonical, SimulationLimitsV1::default()).unwrap();
    let request = SimulationRequestV1::new(
        kernel,
        [u64::from(lanes), 1, 1],
        [lanes, 1, 1],
        vec![
            SimulationArgumentV1::Scalar(scalar(input)),
            SimulationArgumentV1::Buffer(
                BufferArgumentV1::from_scalars(
                    AccessMode::WriteOnly,
                    4,
                    &vec![scalar(0); lanes as usize],
                    target,
                )
                .unwrap(),
            ),
        ],
    );
    let execution = admitted
        .simulate(&request, target, SimulationLimitsV1::default())
        .unwrap();
    execution
        .buffer(1)
        .unwrap()
        .bytes()
        .chunks_exact(4)
        .map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap()))
        .collect()
}

#[test]
fn typed_subgroup_float_sum_can_retain_lane_distinct_nan_bits() {
    let mut module =
        subgroup_collective_typed_module(ExecutionCollectiveKindV1::ReduceSum, ScalarType::F32);
    lane_nan_inputs(&mut module);
    let output = simulate_f32(module, "subgroup-kernel", 64, 0);
    let expected = (0..64)
        .map(|lane| if lane == 0 { 0x7fc0_0001 } else { 0x7fc0_0042 })
        .collect::<Vec<_>>();
    assert_eq!(output, expected);
    assert_ne!(output[0], output[1]);
}

#[test]
fn typed_workgroup_float_sum_broadcasts_one_shared_nan_result() {
    let mut module =
        workgroup_collective_typed_module(ExecutionCollectiveKindV1::ReduceSum, ScalarType::F32);
    lane_nan_inputs(&mut module);
    assert_eq!(
        simulate_f32(module, "reduce-kernel", 4, 0),
        vec![0x7fc0_0001; 4]
    );
}

#[test]
fn typed_float_scans_from_uniform_one_are_not_uniform_prefixes() {
    for (kind, exclusive) in [
        (ExecutionCollectiveKindV1::InclusiveScanSum, false),
        (ExecutionCollectiveKindV1::ExclusiveScanSum, true),
    ] {
        for (module, kernel, lanes) in [
            (
                workgroup_collective_typed_module(kind, ScalarType::F32),
                "reduce-kernel",
                4,
            ),
            (
                subgroup_collective_typed_module(kind, ScalarType::F32),
                "subgroup-kernel",
                64,
            ),
        ] {
            let output = simulate_f32(module, kernel, lanes, 1.0_f32.to_bits());
            let expected = (0..lanes)
                .map(|lane| ((lane + u32::from(!exclusive)) as f32).to_bits())
                .collect::<Vec<_>>();
            assert_eq!(output, expected);
            assert_ne!(output[0], output[1]);
        }
    }
}
