use super::*;

#[test]
fn checked_write_payload_live_header_keeps_the_value_until_store_emission() {
    let expected = std::mem::size_of::<Option<ScopedMemoryStoreSourceV29>>()
        + std::mem::size_of::<SemanticValueBindingV1>()
        + std::mem::size_of::<ExecutionOperandV29>();
    assert_eq!(scoped_checked_write_payload_header_v84().unwrap(), expected);
    for available in [expected, expected - 1] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
        let mut budget = ArgumentBudgetV1::new(&mut work, available);
        let result = budget.reserve_storage(scoped_checked_write_payload_header_v84().unwrap());
        assert_eq!(result.is_ok(), available == expected);
        assert_eq!(
            budget.storage(),
            if available == expected { expected } else { 0 }
        );
    }
}

fn with_write_payload(
    witness: Witness,
    consume: impl FnOnce(
        &ExecutionInstancesV29<'_>,
        ProductionCallInstanceIdV1,
        &mut ArgumentBudgetV1<'_>,
    ),
) {
    let mut owner = witness_owner(witness, true);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
    let capture = owner
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    budget.reserve_storage(capture.retained_storage()).unwrap();
    production_call_instances_v1::with_production_call_instances_v1(
        &owner,
        ROOT,
        &mut budget,
        |instances, budget| {
            consume(instances, instances.id_at(0).unwrap(), budget);
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(())
        },
    )
    .unwrap();
}

#[test]
fn checked_write_payloads_name_exact_original_value_operands() {
    for witness in [
        Witness::Grid,
        Witness::Block,
        Witness::Tile,
        Witness::Stripe,
    ] {
        with_write_payload(witness, |instances, instance, budget| {
            let semantic = instances.owner().source_semantic();
            let function = instances.instance(instance).unwrap().declaration();
            let SemanticTerminatorKindV1::Call(call) = function.blocks()[0].terminator().kind()
            else {
                unreachable!();
            };
            let argument = (call.arguments().len() - 1) as u32;
            let site = ExecutionSiteV29::Terminator {
                block: SsaBlockIdV1::new(0),
            };
            let before = budget.work();
            let operand = scoped_write_payload_operand_v84(
                function,
                semantic.callables(),
                site,
                argument,
                U32,
                budget,
            )
            .unwrap();
            assert!(std::ptr::eq(operand, &call.arguments()[argument as usize]));
            assert_eq!(budget.work() - before, 12);
            for (site, argument, ty) in [
                (site, 0, U32),
                (site, argument - 1, U32),
                (site, argument + 1, U32),
                (site, argument, INDEX),
                (
                    ExecutionSiteV29::Terminator {
                        block: SsaBlockIdV1::new(1),
                    },
                    argument,
                    U32,
                ),
                (
                    ExecutionSiteV29::Statement {
                        block: SsaBlockIdV1::new(0),
                        statement: 0,
                    },
                    argument,
                    U32,
                ),
            ] {
                assert!(
                    scoped_write_payload_operand_v84(
                        function,
                        semantic.callables(),
                        site,
                        argument,
                        ty,
                        budget,
                    )
                    .is_err()
                );
            }
            let mut callables = semantic.callables().to_vec();
            let SemanticCallableDeclV1::CompilerIntrinsic { operation, .. } = &mut callables[1]
            else {
                unreachable!();
            };
            *operation = SemanticCompilerIntrinsicOperationV1::ColdPath;
            assert!(scoped_write_payload_operand_v84(
                function, &callables, site, argument, U32, budget,
            ).is_err());
        });
    }
}

#[test]
fn checked_write_payloads_are_distinct_from_stores_and_call_results() {
    for witness in [
        Witness::Grid,
        Witness::Block,
        Witness::Tile,
        Witness::Stripe,
    ] {
        with_write_payload(witness, |instances, instance, budget| {
            let semantic = instances.owner().source_semantic();
            let function = instances.instance(instance).unwrap().declaration();
            let occurrences = instances.occurrences(instance).unwrap();
            let SemanticTerminatorKindV1::Call(call) = function.blocks()[0].terminator().kind()
            else {
                unreachable!();
            };
            let site = ExecutionSiteV29::Terminator {
                block: SsaBlockIdV1::new(0),
            };
            let source = ScopedMemoryStoreSourceV29::Operand {
                site,
                role: ExecutionOperandV29::CallArgument((call.arguments().len() - 1) as u32),
                ty: U32,
                source: ScopedMemoryOperandSourceV29::Constant,
            };
            let frame = ScopedMemoryFrameV29 {
                site,
                role: Some(ScopedMemoryRoleV29::IntrinsicWrite),
            };
            let mut recorder = ScopedMemoryRecorderV29 {
                anchors: ScopedMemoryAnchorsV29 {
                    subject: ScopedInitializationSubjectV29 {
                        source: ExecutionCallSourceV29::from_instances(instances, budget).unwrap(),
                        instance,
                        function: ROOT,
                        ledger: budget.work_ledger_identity_v1(),
                    },
                    placement: SemanticEmissionPlacementV1::default(),
                    rows: vec![],
                    objects: vec![],
                    object_components: vec![],
                    zero_objects: vec![],
                    compiler_enum: vec![],
                },
                block: Some(BlockId(0)),
                frame: Some(frame),
                read_payload: None,
                index_payload: None,
                store_payload: Some((ValueId(7), source)),
                object_role: None,
                last_load: None,
            };
            // This is a payload-only probe; it supplies no address or guard authority.
            let operation = Operation::new(
                vec![],
                OperationKind::GuardedStore {
                    pointer: ValueId(5),
                    predicate: ValueId(6),
                    value: ValueId(7),
                    access: MemoryAccess::new(AddressSpace::Global, 4),
                },
            );
            let payload = scoped_recorded_payload_v29(
                &recorder,
                semantic.types(),
                semantic.callables(),
                function,
                &operation.kind,
                &operation.results,
                budget,
            )
            .unwrap()
            .unwrap();
            let row = ScopedMemoryAnchorV29 {
                block: BlockId(0),
                position: 0,
                source: Some(frame),
                kind: ScopedMemoryAnchorKindV29::Access {
                    pointer: ValueId(5),
                    payload: Some(payload),
                },
            };
            check_scoped_payload_v29(function, &occurrences, &row, &operation, budget).unwrap();
            for role in [
                None,
                Some(ScopedMemoryRoleV29::CallResult),
                Some(ScopedMemoryRoleV29::Operand(
                    ExecutionOperandV29::CallArgument(0),
                )),
            ] {
                recorder.frame = Some(ScopedMemoryFrameV29 { site, role });
                assert!(
                    scoped_recorded_payload_v29(
                        &recorder,
                        semantic.types(),
                        semantic.callables(),
                        function,
                        &operation.kind,
                        &operation.results,
                        budget,
                    )
                    .unwrap()
                    .is_none()
                );
                let changed = ScopedMemoryAnchorV29 {
                    source: recorder.frame,
                    ..row
                };
                assert!(
                    check_scoped_payload_v29(function, &occurrences, &changed, &operation, budget)
                        .is_err()
                );
            }
            recorder.frame = Some(frame);
            for mode in 0..3 {
                let mut changed = operation.clone();
                if mode == 0 {
                    changed.kind = OperationKind::Store {
                        pointer: ValueId(5),
                        value: ValueId(7),
                        access: MemoryAccess::new(AddressSpace::Global, 4),
                    };
                } else if let OperationKind::GuardedStore { value, access, .. } = &mut changed.kind
                {
                    if mode == 1 {
                        *value = ValueId(8);
                    } else {
                        access.volatile = true;
                    }
                }
                assert!(
                    check_scoped_payload_v29(function, &occurrences, &row, &changed, budget)
                        .is_err()
                );
            }
        });
    }
}
