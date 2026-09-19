//! Actual existing V12 CPU engine, with an independent host bitwise oracle.
//! Integration-layer coverage uses public optimizer and simulator APIs only.
//! Agreement is diagnostic evidence, not source admission or GPU authority.

#![forbid(unsafe_code)]

use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BinaryOp, BlockId,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
    CanonicalKirBlockCoordinateV1, CanonicalKirFunctionCoordinateV1, Constant, Function, Kernel,
    LaunchDomain, LaunchExtent, MemoryAccess, Module, Operation, OperationKind, ScalarType,
    Signature, Terminator, Type, ValueDef, ValueId, VerifiedCanonicalKernelIrModuleV12 as Owner,
    VerifiedCanonicalKernelIrV12,
};
use fe2o3_kernel_opt::{
    CheckedU32LocalOrderOutputV1, U32LocalOrderPreferenceV1, U32LocalOrderRegionV1,
    schedule_checked_u32_local_order_v1,
};
use fe2o3_kir_sim::{
    AdmittedSimulationModuleV1, BufferArgumentV1, ScalarBitsV1, SimulationArgumentV1,
    SimulationLimitsV1, SimulationRequestV1, SimulationTargetV1,
};

const LIMIT: usize = 20_000_000;
const U32: Type = Type::Scalar(ScalarType::U32);

fn binary(result: u32, op: BinaryOp, lhs: u32, rhs: u32) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(result), U32),
        OperationKind::Binary {
            op,
            lhs: ValueId(lhs),
            rhs: ValueId(rhs),
        },
    )
}

fn admit(mut module: Module) -> (Owner, usize) {
    for function in &mut module.functions {
        function.required_capabilities = function.derived_capabilities();
    }
    module.required_capabilities = module
        .functions
        .iter()
        .flat_map(|function| function.required_capabilities.iter().cloned())
        .collect();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let (owner, storage) =
        Owner::from_module_ref_with_verification_budget_v12(&module, &mut budget).unwrap();
    assert_eq!(budget.storage(), 0);
    (owner, storage.retained_storage())
}

fn scheduled(
    input: &Owner,
    retained: usize,
    preference: U32LocalOrderPreferenceV1,
    count: u32,
) -> CheckedU32LocalOrderOutputV1<'_> {
    let region = U32LocalOrderRegionV1 {
        expected_input: *input.canonical().identity(),
        block: CanonicalKirBlockCoordinateV1 {
            function: CanonicalKirFunctionCoordinateV1(0),
            block: 0,
        },
        first_operation: 0,
        operation_count: count,
    };
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(retained).unwrap();
    let result =
        schedule_checked_u32_local_order_v1(input, region, preference, &mut budget).unwrap();
    assert_eq!(budget.storage(), retained);
    budget.reserve_storage(result.retained_storage()).unwrap();
    result.replay(&mut budget).unwrap();
    assert_eq!(budget.storage(), retained + result.retained_storage());
    result
}

fn kernel() -> Module {
    let pointer = Type::pointer(U32, AddressSpace::Global, AccessMode::ReadWrite);
    let mut block = BasicBlock::new(BlockId(27));
    block.operations = vec![
        binary(5, BinaryOp::BitXor, 1, 2),
        binary(6, BinaryOp::BitOr, 3, 4),
        binary(7, BinaryOp::BitAnd, 5, 6),
        Operation::effect_free(
            ValueDef::new(ValueId(8), Type::INDEX),
            OperationKind::Constant(Constant::Index(1)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(9), pointer.clone()),
            OperationKind::GetElementPointer {
                base: ValueId(0),
                offset: ValueId(8),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(9),
                value: ValueId(7),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("local-order-cpu-oracle");
    module.functions.push(Function::kernel_entry(
        "root",
        Signature::new(vec![pointer, U32, U32, U32, U32], vec![]),
        (0..5).map(ValueId).collect(),
        vec![block],
    ));
    module.kernels.push(Kernel::new(
        "kernel",
        "root",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    module
}

#[test]
fn both_actual_orders_execute_independent_outputs_and_preserve_canaries() {
    let (input, retained) = admit(kernel());
    let before = input.canonical().canonical_bytes().to_vec();
    let target = SimulationTargetV1::amdgpu_64();
    let mut observed_orders = Vec::new();
    let mut checked_cases = 0;
    for preference in [
        U32LocalOrderPreferenceV1::SourceOrder,
        U32LocalOrderPreferenceV1::ReverseReady,
    ] {
        let output = scheduled(&input, retained, preference, 3);
        observed_orders.push(
            output.output().module().functions[0]
                .body
                .as_ref()
                .unwrap()
                .blocks[0]
                .operations[..3]
                .iter()
                .map(|op| op.results[0].id.0)
                .collect::<Vec<_>>(),
        );
        let canonical = VerifiedCanonicalKernelIrV12::from_canonical_bytes(
            output.output().canonical().canonical_bytes().to_vec(),
        )
        .unwrap();
        let simulator =
            AdmittedSimulationModuleV1::admit_v12(canonical, SimulationLimitsV1::default())
                .unwrap();
        for [a, b, c, d] in [
            [0, 0, 0, 0],
            [1, 2, 3, 4],
            [u32::MAX, 0, 0, u32::MAX],
            [0xffff_fff0, 0x25, 0xff, 0x100],
            [0x8000_0000, 0x7fff_ffff, 0xaaaa_aaaa, 0x5555_5555],
        ] {
            let buffer = BufferArgumentV1::from_scalars(
                AccessMode::ReadWrite,
                4,
                &[
                    ScalarBitsV1::u32(0xdead_beef),
                    ScalarBitsV1::u32(0xa5a5_a5a5),
                    ScalarBitsV1::u32(0xcafe_babe),
                ],
                target,
            )
            .unwrap();
            let mut arguments = vec![SimulationArgumentV1::Buffer(buffer)];
            arguments
                .extend([a, b, c, d].map(|v| SimulationArgumentV1::Scalar(ScalarBitsV1::u32(v))));
            let request = SimulationRequestV1::new("kernel", [1, 1, 1], [1, 1, 1], arguments);
            let original = request.clone();
            let result = simulator
                .simulate(&request, target, SimulationLimitsV1::default())
                .unwrap();
            let expected = [0xdead_beef, (a ^ b) & (c | d), 0xcafe_babe];
            let expected_bytes = expected
                .iter()
                .flat_map(|word| word.to_le_bytes())
                .collect::<Vec<_>>();
            assert_eq!(result.buffer(0).unwrap().bytes(), expected_bytes);
            assert_eq!(result.buffer(0).unwrap().initialized(), [true; 12]);
            assert_eq!(request, original);
            checked_cases += 1;
        }
    }
    assert_eq!(observed_orders, [vec![5, 6, 7], vec![6, 5, 7]]);
    assert_eq!(checked_cases, 10);
    assert_eq!(input.canonical().canonical_bytes(), before);
}
