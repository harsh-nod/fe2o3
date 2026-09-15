use crate::*;
use fe2o3_kernel_ir::*;

#[allow(dead_code)]
mod fixture {
    use fe2o3_kernel_ir::*;
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../fe2o3-kernel-ir/src/execution_capability_v1/subgroup_partition/ordered_max_v1/fixture.rs"
    ));
}

fn input_module() -> Module {
    let mut module = fixture::module();
    let input = Type::pointer(Type::F32, AddressSpace::Global, AccessMode::ReadOnly);
    let output = Type::pointer(Type::F32, AddressSpace::Global, AccessMode::ReadWrite);
    let function = &mut module.functions[0];
    function.signature = Signature::new(vec![input.clone(), output.clone()], vec![]);
    function.body.as_mut().unwrap().parameters = vec![ValueId(200), ValueId(201)];
    let block = &mut function.body.as_mut().unwrap().blocks[0];
    // Feed the existing ordered-max call from exact per-lane bits without arithmetic.
    block.operations.splice(
        4..5,
        [
            Operation::effect_free(
                ValueDef::new(ValueId(202), Type::INDEX),
                OperationKind::Intrinsic(IntrinsicOperation::new(
                    IntrinsicKind::InvocationIndex {
                        kind: IndexKind::Global,
                        axis: Axis::X,
                    },
                    Type::INDEX,
                )),
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(203), input),
                OperationKind::GetElementPointer {
                    base: ValueId(200),
                    offset: ValueId(202),
                },
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(4), Type::F32),
                OperationKind::Load {
                    pointer: ValueId(203),
                    access: MemoryAccess::new(AddressSpace::Global, 4),
                },
            ),
        ],
    );
    block.operations.extend([
        Operation::effect_free(
            ValueDef::new(ValueId(204), output),
            OperationKind::GetElementPointer {
                base: ValueId(201),
                offset: ValueId(202),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(204),
                value: ValueId(6),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ]);
    function.required_capabilities.extend(
        block
            .operations
            .iter()
            .flat_map(Operation::required_capabilities),
    );
    module.required_capabilities = function.required_capabilities.clone();
    module.kernels[0].required_capabilities = function.required_capabilities.clone();
    module
}

fn run(input: &[u32; 64]) -> Vec<u32> {
    let target = SimulationTargetV1::amdgpu_64();
    let scalar = |bits| ScalarBitsV1::new(ScalarType::F32, u128::from(bits), target).unwrap();
    let source = input.iter().copied().map(scalar).collect::<Vec<_>>();
    let result = vec![scalar(0); 64];
    let canonical = VerifiedCanonicalKernelIrV13::from_module(input_module()).unwrap();
    let admitted =
        AdmittedSimulationModuleV1::admit_v13(canonical, SimulationLimitsV1::default()).unwrap();
    assert!(!admitted.grants_execution_authority());
    let projection = admitted.capability_projection_receipt_v13().unwrap();
    assert!(projection.coordinates().iter().any(|c| c.execution_family()
        == Some(SimulationExecutionCapabilityFamilyV13::SubgroupPartitionReduceMaxF32)));
    let execution = admitted
        .simulate(
            &SimulationRequestV1::new(
                "partition",
                [64, 1, 1],
                [64, 1, 1],
                vec![
                    SimulationArgumentV1::Buffer(
                        BufferArgumentV1::from_scalars(AccessMode::ReadOnly, 4, &source, target)
                            .unwrap(),
                    ),
                    SimulationArgumentV1::Buffer(
                        BufferArgumentV1::from_scalars(AccessMode::ReadWrite, 4, &result, target)
                            .unwrap(),
                    ),
                ],
            ),
            target,
            SimulationLimitsV1::default(),
        )
        .unwrap();
    assert_eq!(execution.invocations_executed(), 64);
    execution
        .buffer(1)
        .unwrap()
        .bytes()
        .chunks_exact(4)
        .map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap()))
        .collect()
}

#[test]
fn ordered_partition_max_preserves_nan_bits_signed_zero_and_partition_boundaries() {
    let mut input = [0_u32; 64];
    let mut expected = input;
    for lane in 0..16 {
        input[lane] = if lane % 2 == 0 { 0 } else { 0x8000_0000 };
        expected[lane] = input[lane];
        // Both signaling/quiet and positive/negative NaNs retain their own payload.
        input[16 + lane] = [0x7f80_0001, 0xff80_0042, 0x7fc0_1234, 0xffc0_2345][lane % 4];
        expected[16 + lane] = input[16 + lane];
        input[32 + lane] = 0xff80_0000;
        expected[32 + lane] = 0x7f80_0000;
        input[48 + lane] = 0xbf80_0000;
        expected[48 + lane] = 0x4080_0000;
    }
    input[35] = 0x7f80_0000;
    input[59] = 0x4080_0000;
    assert_eq!(run(&input), expected);
    for value in &mut input[..16] {
        *value ^= 0x8000_0000;
    }
    expected[..16].copy_from_slice(&input[..16]);
    assert_eq!(run(&input), expected);
}

#[test]
fn maximum_uses_ascending_xor_tree_not_reassociation_or_tile_broadcast() {
    let mut tile = std::array::from_fn::<_, 16, _>(|lane| 0x7fc0_0100 + lane as u32);
    tile[0] = 0x3f80_0000; // 1
    tile[1] = 0x4000_0000; // 2
    tile[3] = 0x4040_0000; // 3
    let mut expected_tile = tile;
    expected_tile[0] = 0x4000_0000;
    expected_tile[1] = 0x4040_0000;
    let input = std::array::from_fn(|lane| tile[lane % 16]);
    let expected = std::array::from_fn::<_, 64, _>(|lane| expected_tile[lane % 16]);
    let actual = run(&input);
    assert_eq!(actual, expected);
    // Descending XOR would give lane0 3; numeric-max/fmax would discard its NaN barriers.
    for partition in actual.chunks_exact(16) {
        assert_eq!(partition[0], 0x4000_0000);
        assert_ne!(partition[0], partition[1]);
        assert_eq!(partition[2], 0x7fc0_0102);
    }
}

#[test]
fn ordered_partition_max_preserves_finite_infinite_and_signed_zero_ties() {
    let input = std::array::from_fn(|lane| match lane / 16 {
        0 => 0x3f80_0000,
        1 => 0xbf80_0000,
        2 => 0x7f80_0000,
        3 => {
            if lane % 2 == 0 {
                0
            } else {
                0x8000_0000
            }
        }
        _ => unreachable!(),
    });
    assert_eq!(run(&input), input);
}
