use super::*;
use fe2o3_kernel_ir::*;

#[path = "../../../../../fe2o3-kernel-ir/src/execution_capability_v1/workgroup_memory_index_v2_tests/fixture.rs"]
mod fixture;

#[test]
fn scoped_index_amd_lowering_preserves_flat_rank_and_conversion_ssa_name() {
    let mut module = fixture::module();
    module.kernels[0].workgroup_size = Some(WorkgroupSize::new(4, 4, 4));
    module.kernels[0].domain = LaunchDomain::D3 {
        x: LaunchExtent::Static(4),
        y: LaunchExtent::Static(4),
        z: LaunchExtent::Static(4),
    };
    VerifiedCanonicalKernelIrV13::from_module(module.clone()).unwrap();
    let function = &module.functions[0];
    let source = value_types(function);
    let mut types = source.clone();
    let mut aliases = BTreeMap::new();
    let mut next = next_value_id(&module, function).unwrap();
    let mut output = Vec::new();
    let operations = fixture::operations(&module);
    lower(
        &module,
        function,
        &operations[2],
        fixture::operation_contract(&operations[2]),
        &source,
        &mut types,
        &mut aliases,
        &mut next,
        &mut output,
    )
    .unwrap();
    let first_len = output.len();
    lower(
        &module,
        function,
        &operations[3],
        fixture::operation_contract(&operations[3]),
        &source,
        &mut types,
        &mut aliases,
        &mut next,
        &mut output,
    )
    .unwrap();
    assert!(
        !output[first_len..]
            .iter()
            .any(|op| matches!(op.kind, OperationKind::Intrinsic(_)))
    );
    assert_eq!(types[&ValueId(3)], Type::INDEX);
    assert!(matches!(aliases[&ValueId(3)], ExecutionAliasV1::Physical(p) if p.value == ValueId(3)));
    for z in 0..4 {
        for y in 0..4 {
            for x in 0..4 {
                let mut values = BTreeMap::new();
                for operation in &output {
                    let value = match operation.kind {
                        OperationKind::Constant(Constant::Index(n)) => n,
                        OperationKind::Intrinsic(IntrinsicOperation {
                            kind:
                                IntrinsicKind::InvocationIndex {
                                    kind: IndexKind::Local,
                                    axis,
                                },
                            ..
                        }) => match axis {
                            Axis::X => x,
                            Axis::Y => y,
                            Axis::Z => z,
                        },
                        OperationKind::Binary {
                            op: BinaryOp::Add,
                            lhs,
                            rhs,
                        } => values[&lhs] + values[&rhs],
                        OperationKind::Binary {
                            op: BinaryOp::Multiply,
                            lhs,
                            rhs,
                        } => values[&lhs] * values[&rhs],
                        _ => panic!("unexpected physical index operation"),
                    };
                    values.insert(operation.results[0].id, value);
                }
                assert_eq!(values[&ValueId(2)], (z * 4 + y) * 4 + x);
                assert_eq!(values[&ValueId(3)], values[&ValueId(2)]);
            }
        }
    }
}

#[test]
fn scoped_index_amd_phi_adapter_leaves_unrelated_capabilities_logical() {
    let module = fixture::module();
    let operations = fixture::operations(&module);
    let mut body = module.functions[0].body.clone().unwrap();
    body.blocks[0].parameters = vec![
        ValueDef::new(ValueId(10), operations[2].results[0].ty.clone()),
        ValueDef::new(ValueId(11), operations[1].results[0].ty.clone()),
    ];
    let mut types = BTreeMap::new();
    let mut aliases = BTreeMap::new();
    prepare_parameters(&mut body, &mut types, &mut aliases);
    assert_eq!(body.blocks[0].parameters[0].ty, Type::INDEX);
    assert_eq!(body.blocks[0].parameters[1].ty, operations[1].results[0].ty);
    assert!(!types.contains_key(&ValueId(11)));
    assert!(!aliases.contains_key(&ValueId(11)));
}

#[test]
fn scoped_index_amd_rejects_geometry_beyond_rank_range() {
    for size in [
        WorkgroupSize::new(0, 1, 1),
        WorkgroupSize::new(u32::MAX, u32::MAX, 2),
    ] {
        let mut module = fixture::module();
        module.kernels[0].workgroup_size = Some(size);
        let function = &module.functions[0];
        let source = value_types(function);
        let operation = &fixture::operations(&module)[2];
        assert!(
            lower(
                &module,
                function,
                operation,
                fixture::operation_contract(operation),
                &source,
                &mut source.clone(),
                &mut BTreeMap::new(),
                &mut 10,
                &mut Vec::new()
            )
            .is_err()
        );
    }
}
