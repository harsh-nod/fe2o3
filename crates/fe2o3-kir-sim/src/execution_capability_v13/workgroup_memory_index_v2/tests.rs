use super::*;
use fe2o3_kernel_ir::*;

#[path = "../../../../fe2o3-kernel-ir/src/execution_capability_v1/workgroup_memory_index_v2_tests/fixture.rs"]
mod fixture;

fn evaluate(operations: &[Operation], local: [u64; 3]) -> BTreeMap<ValueId, u64> {
    let mut values = BTreeMap::new();
    for op in operations {
        let value = match op.kind {
            OperationKind::Constant(Constant::Index(n)) => n,
            OperationKind::Intrinsic(IntrinsicOperation {
                kind:
                    IntrinsicKind::InvocationIndex {
                        kind: IndexKind::Local,
                        axis,
                    },
                ..
            }) => {
                local[match axis {
                    Axis::X => 0,
                    Axis::Y => 1,
                    Axis::Z => 2,
                }]
            }
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
            _ => continue,
        };
        let [result] = op.results.as_slice() else {
            panic!("one physical rank component");
        };
        assert_eq!(result.ty, Type::INDEX);
        values.insert(result.id, value);
    }
    values
}

#[test]
fn scoped_index_simulation_projection_is_flat_and_conversion_is_not_reissuance() {
    for size in [
        WorkgroupSize::new(64, 1, 1),
        WorkgroupSize::new(4, 4, 4),
        WorkgroupSize::new(8, 2, 4),
    ] {
        let mut source = fixture::module();
        source.kernels[0].workgroup_size = Some(size);
        source.kernels[0].domain = LaunchDomain::D3 {
            x: LaunchExtent::Static(size.x),
            y: LaunchExtent::Static(size.y),
            z: LaunchExtent::Static(size.z),
        };
        verify_module(&source).unwrap();
        let source = fixture::operations(&source);
        let types = source
            .iter()
            .flat_map(|op| &op.results)
            .map(|v| (v.id, v.ty.clone()))
            .collect();
        let mut promoted = BTreeMap::new();
        let mut next = 10;
        let issued = issue(
            source[2].results.clone(),
            Some(size),
            &mut promoted,
            &mut next,
        )
        .unwrap();
        let converted = into_disjoint(
            source[3].results.clone(),
            fixture::operation_contract(&source[3]),
            &types,
            &mut Vec::new(),
            &mut promoted,
            &mut next,
        )
        .unwrap();
        assert!(
            !converted
                .iter()
                .any(|op| matches!(op.kind, OperationKind::Intrinsic(_)))
        );
        for z in 0..u64::from(size.z) {
            for y in 0..u64::from(size.y) {
                for x in 0..u64::from(size.x) {
                    let all = issued.iter().chain(&converted).cloned().collect::<Vec<_>>();
                    let values = evaluate(&all, [x, y, z]);
                    assert_eq!(
                        values[&ValueId(2)],
                        (z * u64::from(size.y) + y) * u64::from(size.x) + x
                    );
                    assert_eq!(values[&ValueId(3)], values[&ValueId(2)]);
                }
            }
        }
    }
}

#[test]
fn scoped_index_phi_keeps_its_exact_original_edge_value() {
    let mut source = fixture::module();
    let body = source.functions[0].body.as_mut().unwrap();
    let mut conversion = body.blocks[0].operations.pop().unwrap();
    let payload = body.blocks[0].operations[2].results[0].ty.clone();
    let OperationKind::ExecutionCapability(c) = &mut conversion.kind else {
        unreachable!()
    };
    c.operands = vec![ValueId(4)];
    body.blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![ValueId(2)],
    });
    let mut next = BasicBlock::new(BlockId(1));
    next.parameters.push(ValueDef::new(ValueId(4), payload));
    next.operations.push(conversion);
    next.terminator = Some(Terminator::Return { values: vec![] });
    body.blocks.push(next);
    verify_module(&source).unwrap();
    let admitted = crate::AdmittedSimulationModuleV1::admit_v13(
        VerifiedCanonicalKernelIrV13::from_module(source).unwrap(),
        crate::SimulationLimitsV1::default(),
    )
    .unwrap();
    assert!(!admitted.grants_execution_authority());
    let coordinates = admitted
        .capability_projection_receipt_v13()
        .unwrap()
        .coordinates();
    assert!(coordinates.iter().any(|c| c.execution_family()
        == Some(SimulationExecutionCapabilityFamilyV13::WorkgroupMemoryIndexIntoDisjoint)));
}

#[test]
fn scoped_conversion_rejects_an_unprojected_input_and_missing_geometry() {
    let module = fixture::module();
    let operations = fixture::operations(&module);
    let types = operations
        .iter()
        .flat_map(|op| &op.results)
        .map(|v| (v.id, v.ty.clone()))
        .collect();
    assert!(
        into_disjoint(
            operations[3].results.clone(),
            fixture::operation_contract(&operations[3]),
            &types,
            &mut Vec::new(),
            &mut BTreeMap::new(),
            &mut 10
        )
        .is_err()
    );
    assert!(
        issue(
            operations[2].results.clone(),
            None,
            &mut BTreeMap::new(),
            &mut 10
        )
        .is_err()
    );
    for size in [
        WorkgroupSize::new(0, 1, 1),
        WorkgroupSize::new(u32::MAX, u32::MAX, 2),
    ] {
        assert!(
            issue(
                operations[2].results.clone(),
                Some(size),
                &mut BTreeMap::new(),
                &mut 10
            )
            .is_err()
        );
    }
}
