use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BinaryOp, BlockId, Function, Kernel, LaunchDomain,
    LaunchExtent, MemoryAccess, Module, Operation, OperationKind, ScalarType, Signature,
    Terminator, Type, ValueDef, ValueId, VerifiedCanonicalKernelIrV7,
};
use fe2o3_kir_sim::{
    AdmittedSimulationModuleV1, BufferArgumentV1, ScalarBitsV1, SimulationArgumentV1,
    SimulationErrorV1, SimulationExecutionErrorKindV1, SimulationLimitsV1, SimulationRequestV1,
    SimulationTargetV1,
};

const TARGET: SimulationTargetV1 = SimulationTargetV1::amdgpu_64();
const SIGNED_TYPES: [ScalarType; 5] = [
    ScalarType::I8,
    ScalarType::I16,
    ScalarType::I32,
    ScalarType::I64,
    ScalarType::I128,
];
const UNSIGNED_TYPES: [ScalarType; 5] = [
    ScalarType::U8,
    ScalarType::U16,
    ScalarType::U32,
    ScalarType::U64,
    ScalarType::U128,
];
const OPERATORS: [BinaryOp; 2] = [BinaryOp::Divide, BinaryOp::Remainder];

fn mask(ty: ScalarType) -> u128 {
    u128::MAX >> (128 - ty.bit_width().unwrap())
}

fn division_module(ty: ScalarType, op: BinaryOp) -> AdmittedSimulationModuleV1 {
    let scalar = Type::Scalar(ty);
    let output = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        Operation::effect_free(
            ValueDef::new(ValueId(3), scalar.clone()),
            OperationKind::Binary {
                op,
                lhs: ValueId(1),
                rhs: ValueId(2),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(0),
                value: ValueId(3),
                access: MemoryAccess::new(
                    AddressSpace::Global,
                    u32::from(ty.bit_width().unwrap() / 8),
                ),
            },
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("sim-tests::signed-division");
    module.functions.push(Function::kernel_entry(
        "division_impl",
        Signature::new(vec![output, scalar.clone(), scalar], vec![]),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![block],
    ));
    module.kernels.push(Kernel::new(
        "division",
        "division_impl",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    let canonical = VerifiedCanonicalKernelIrV7::from_module(module).unwrap();
    AdmittedSimulationModuleV1::admit(canonical, SimulationLimitsV1::default()).unwrap()
}

fn run(
    module: &AdmittedSimulationModuleV1,
    ty: ScalarType,
    lhs: u128,
    rhs: u128,
) -> Result<u128, Box<SimulationErrorV1>> {
    let scalar = |bits| ScalarBitsV1::new(ty, bits & mask(ty), TARGET).unwrap();
    let output = BufferArgumentV1::from_scalars(
        AccessMode::ReadWrite,
        u32::from(ty.bit_width().unwrap() / 8),
        &[scalar(0)],
        TARGET,
    )
    .unwrap();
    let request = SimulationRequestV1::new(
        "division",
        [1, 1, 1],
        [1, 1, 1],
        vec![
            SimulationArgumentV1::Buffer(output),
            SimulationArgumentV1::Scalar(scalar(lhs)),
            SimulationArgumentV1::Scalar(scalar(rhs)),
        ],
    );
    let execution = module.simulate(&request, TARGET, SimulationLimitsV1::default())?;
    let bytes = execution.buffer(0).unwrap().bytes();
    let mut result = [0; 16];
    result[..bytes.len()].copy_from_slice(bytes);
    Ok(u128::from_le_bytes(result))
}

fn assert_undefined(error: Box<SimulationErrorV1>, reason: &'static str) {
    let SimulationErrorV1::Execution(error) = *error else {
        panic!("expected execution failure, got {error:?}");
    };
    assert_eq!(
        error.kind,
        SimulationExecutionErrorKindV1::UndefinedIntegerOperation(reason)
    );
}

#[test]
fn signed_min_over_negative_one_is_undefined_for_both_operators_at_every_width() {
    for ty in SIGNED_TYPES {
        let min = 1_u128 << (ty.bit_width().unwrap() - 1);
        for op in OPERATORS {
            let module = division_module(ty, op);
            let error = run(&module, ty, min, mask(ty))
                .expect_err("MIN / -1 and MIN % -1 must overflow at the declared width");
            assert_undefined(error, "signed division overflow");
        }
    }
}

#[test]
fn signed_division_and_remainder_preserve_valid_boundaries_and_signs() {
    for ty in SIGNED_TYPES {
        let min = i128::MIN >> (128 - ty.bit_width().unwrap());
        let max = -(min + 1);
        for op in OPERATORS {
            let module = division_module(ty, op);
            for (lhs, rhs) in [
                (min, 1),
                (min, 2),
                (min, -2),
                (min + 1, -1),
                (max, -1),
                (max, 1),
                (7, 3),
                (-7, 3),
                (7, -3),
                (-7, -3),
                (0, -1),
            ] {
                let expected = match op {
                    BinaryOp::Divide => lhs.checked_div(rhs).unwrap(),
                    BinaryOp::Remainder => lhs.checked_rem(rhs).unwrap(),
                    _ => unreachable!(),
                };
                assert_eq!(
                    run(&module, ty, lhs as u128, rhs as u128).unwrap(),
                    expected as u128 & mask(ty),
                    "{ty:?}: {lhs} {op:?} {rhs}",
                );
            }
        }
    }
}

#[test]
fn division_and_remainder_by_zero_remain_undefined_for_all_integer_widths() {
    for ty in SIGNED_TYPES.into_iter().chain(UNSIGNED_TYPES) {
        for op in OPERATORS {
            let module = division_module(ty, op);
            for lhs in [0, 1, mask(ty), 1_u128 << (ty.bit_width().unwrap() - 1)] {
                assert_undefined(
                    run(&module, ty, lhs, 0).expect_err("division by zero"),
                    "division by zero",
                );
            }
        }
    }
}

#[test]
fn unsigned_division_and_remainder_preserve_the_full_range() {
    for ty in UNSIGNED_TYPES {
        let max = mask(ty);
        for op in OPERATORS {
            let module = division_module(ty, op);
            for (lhs, rhs) in [(max, 1), (max, max), (max, 2), (max / 2 + 1, max), (0, max)] {
                let expected = match op {
                    BinaryOp::Divide => lhs / rhs,
                    BinaryOp::Remainder => lhs % rhs,
                    _ => unreachable!(),
                };
                assert_eq!(
                    run(&module, ty, lhs, rhs).unwrap(),
                    expected,
                    "{ty:?}: {lhs} {op:?} {rhs}",
                );
            }
        }
    }
}
