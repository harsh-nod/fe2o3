#[test]
fn read_only_allocation_empty_extent_has_no_false_path_memory_access() {
    use fe2o3_kernel_ir::VerifiedCanonicalKernelIrV11;
    use fe2o3_kir_sim::{
        AdmittedSimulationModuleV1, BufferArgumentV1, ScalarBitsV1, SimulationArgumentV1,
        SimulationLimitsV1, SimulationRequestV1, SimulationTargetV1,
    };
    for float in [false, true] {
        let owner = owner(float);
        let target = SimulationTargetV1::amdgpu_64();
        for index in [0, u64::MAX] {
            let request = SimulationRequestV1::new(
                owner.module().kernels[0].id.clone(),
                [64, 1, 1],
                [64, 1, 1],
                vec![
                    SimulationArgumentV1::Buffer(
                        BufferArgumentV1::new(
                            if float {
                                ScalarType::F32
                            } else {
                                ScalarType::U16
                            },
                            AccessMode::ReadWrite,
                            if float { 4 } else { 2 },
                            vec![],
                            vec![],
                            target,
                        )
                        .unwrap(),
                    ),
                    SimulationArgumentV1::Scalar(
                        ScalarBitsV1::new(ScalarType::U64, u128::from(index), target).unwrap(),
                    ),
                    SimulationArgumentV1::Scalar(
                        ScalarBitsV1::new(ScalarType::U64, u128::from(index), target).unwrap(),
                    ),
                ],
            );
            for unguarded in [false, true] {
                let mut module = owner.module().clone();
                if unguarded {
                    let operation = module.functions[0]
                        .body
                        .as_mut()
                        .unwrap()
                        .blocks
                        .iter_mut()
                        .flat_map(|block| &mut block.operations)
                        .find(|op| matches!(op.kind, OperationKind::GuardedLoad { .. }))
                        .unwrap();
                    let OperationKind::GuardedLoad {
                        pointer, access, ..
                    } = operation.kind
                    else {
                        unreachable!()
                    };
                    operation.kind = OperationKind::Load { pointer, access };
                }
                verify_module(&module).unwrap();
                let canonical = VerifiedCanonicalKernelIrV11::from_module(module).unwrap();
                let admitted =
                    AdmittedSimulationModuleV1::admit_v11(canonical, SimulationLimitsV1::default())
                        .unwrap();
                let result = admitted.simulate(&request, target, SimulationLimitsV1::default());
                assert_eq!(
                    result.is_ok(),
                    !unguarded,
                    "float={float} index={index} unguarded={unguarded}: {result:?}"
                );
                if let Ok(result) = result {
                    assert!(result.buffer(0).unwrap().bytes().is_empty());
                }
            }
        }
    }
}

fn restricted_fixture() -> GuardedAddressFixture {
    let mut fixture = generated_matrix_tail_fixture(1);
    for ty in &mut fixture.module.functions[0].signature.parameters[..2] {
        *ty = Type::slice(
            Type::Scalar(ScalarType::U16),
            AddressSpace::Global,
            AccessMode::ReadWrite,
        );
    }
    let operations = guarded_fixture_operations_mut(&mut fixture);
    operations[0].results[0].ty = Type::pointer(
        Type::Scalar(ScalarType::U16),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    );
    let read_only = Type::pointer(
        Type::Scalar(ScalarType::U16),
        AddressSpace::Global,
        AccessMode::ReadOnly,
    );
    operations.insert(
        1,
        Operation::effect_free(
            ValueDef::new(ValueId(1000), read_only.clone()),
            OperationKind::Cast {
                kind: CastKind::RestrictPointerAccess,
                value: ValueId(3),
                to: read_only,
            },
        ),
    );
    for operation in operations {
        if let OperationKind::GetElementPointer { base, .. } = &mut operation.kind {
            *base = ValueId(1000);
        }
    }
    for location in &mut fixture.locations {
        location.operation_index += 1;
    }
    verify_module(&fixture.module).unwrap();
    fixture
}

#[test]
fn read_only_allocation_bounds_retain_original_slice_and_extent() {
    for mutation in 0..3 {
        let mut fixture = restricted_fixture();
        let operations = guarded_fixture_operations_mut(&mut fixture);
        match mutation {
            1 => operations[0].kind = OperationKind::SliceData { slice: ValueId(1) },
            2 => operations[2].kind = OperationKind::SliceLength { slice: ValueId(1) },
            _ => (),
        }
        let count = operations.len();
        verify_module(&fixture.module).unwrap();
        assert_eq!(
            guarded_accesses_have_structural_bounds(&fixture.module, &fixture.locations, count),
            mutation == 0
        );
    }
}

#[test]
fn read_only_allocation_bounds_reject_nonexact_restrictions_and_charge_lookups() {
    let fixture = restricted_fixture();
    let operations = &fixture.module.functions[0].body.as_ref().unwrap().blocks[0].operations;
    fn definitions(ops: &[Operation]) -> BTreeMap<ValueId, GuardedAddressDefinitionV1<'_>> {
        ops.iter()
            .flat_map(|op| {
                op.results
                    .iter()
                    .map(move |value| (value.id, GuardedAddressDefinitionV1::Operation(op)))
            })
            .collect::<BTreeMap<_, _>>()
    }
    // This private helper relies on verified KIR; malformed shapes are still fail-closed.
    for mutation in 0..8 {
        let mut ops = operations[..2].to_vec();
        match mutation {
            1 => {
                let mut restriction = ops[1].clone();
                restriction.results[0].id = ValueId(1001);
                let OperationKind::Cast { value, .. } = &mut restriction.kind else {
                    unreachable!()
                };
                *value = ValueId(1000);
                ops.push(restriction);
            }
            2..=4 => {
                let changed = Type::pointer(
                    Type::Scalar(if mutation == 3 {
                        ScalarType::F32
                    } else {
                        ScalarType::U16
                    }),
                    if mutation == 2 {
                        AddressSpace::Workgroup
                    } else {
                        AddressSpace::Global
                    },
                    if mutation == 4 {
                        AccessMode::ReadWrite
                    } else {
                        AccessMode::ReadOnly
                    },
                );
                ops[1].results[0].ty = changed.clone();
                let OperationKind::Cast { to, .. } = &mut ops[1].kind else {
                    unreachable!()
                };
                *to = changed;
            }
            5 => {
                ops[0].results[0].ty = Type::pointer(
                    Type::Scalar(ScalarType::U16),
                    AddressSpace::Global,
                    AccessMode::ReadOnly,
                )
            }
            6 => ops[0].kind = OperationKind::Constant(Constant::Index(0)),
            7 => {
                let OperationKind::Cast { kind, .. } = &mut ops[1].kind else {
                    unreachable!()
                };
                *kind = CastKind::Bitcast;
            }
            _ => (),
        }
        let definitions = definitions(&ops);
        let base = ValueId(if mutation == 1 { 1001 } else { 1000 });
        let mut budget = GuardedAddressProofBudgetV1 { remaining: 2 };
        assert_eq!(
            read_only_allocation_bound_slice_v1(base, &definitions, &mut budget),
            (mutation == 0).then_some(ValueId(0)),
            "mutation {mutation}"
        );
        if mutation == 0 {
            assert_eq!(budget.remaining, 0);
        }
    }
    let definitions = definitions(&operations[..2]);
    assert!(
        read_only_allocation_bound_slice_v1(
            ValueId(1000),
            &definitions,
            &mut GuardedAddressProofBudgetV1 { remaining: 1 }
        )
        .is_none()
    );
}

#[test]
fn read_only_allocation_conditional_witness_requires_exact_call_and_single_read() {
    let owner = owner(false);
    let semantic = owner.semantic_ssa.source_owner().semantic();
    let body = owner.module().functions[0].body.as_ref().unwrap();
    let index = build_kir_correlation_index(
        body,
        10_000,
        &mut UnsupportedIndexCorrelationBudgetV1 { remaining: 100_000 },
    )
    .unwrap();
    assert_eq!(index.memory_consumers.len(), 2);
    let consumer = index.memory_consumers[0];
    let operation = index.operations[&consumer.location];
    let site = SemanticAccessSiteV1 {
        block: 2,
        statement: None,
        ordinal: 0,
    };
    let function = SemanticFunctionIdV1::from_index(0);
    assert!(
        authenticate_conditional_total_read_v1(
            Some(semantic),
            function,
            site,
            site,
            consumer,
            operation,
            1
        )
        .is_some()
    );
    for block in [0, 1, 4] {
        let other = SemanticAccessSiteV1 { block, ..site };
        assert!(
            authenticate_conditional_total_read_v1(
                Some(semantic),
                function,
                other,
                other,
                consumer,
                operation,
                1
            )
            .is_none()
        );
    }
    for count in [0, 2] {
        assert!(
            authenticate_conditional_total_read_v1(
                Some(semantic),
                function,
                site,
                site,
                consumer,
                operation,
                count
            )
            .is_none()
        );
    }
    assert!(
        authenticate_conditional_total_read_v1(None, function, site, site, consumer, operation, 1)
            .is_none()
    );
    assert!(
        authenticate_conditional_total_read_v1(
            Some(semantic),
            function,
            site,
            SemanticAccessSiteV1 { block: 3, ..site },
            consumer,
            operation,
            1
        )
        .is_none()
    );
    assert!(
        authenticate_conditional_total_read_v1(
            Some(semantic),
            function,
            site,
            site,
            KirMemoryConsumerV1 {
                pointer: ValueId(u32::MAX),
                ..consumer
            },
            operation,
            1
        )
        .is_none()
    );
}
