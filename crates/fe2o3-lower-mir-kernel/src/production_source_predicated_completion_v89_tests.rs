use super::*;
use fe2o3_kernel_ir::{ExplicitLaunchExtent, FormalIndexWidth};

include!("production_source_contract_descriptor_fixture_v89.rs");

const LIMIT: usize = 1_000_000_000;

fn run_predicated_completion_v89(
    count: u32,
    receiver: usize,
    mode: u8,
    work: usize,
    storage: usize,
    reached: &std::cell::Cell<bool>,
) -> (
    Result<(), ProductionMixedSourceHandoffErrorV26>,
    usize,
    usize,
) {
    let retained_floor = std::cell::Cell::new(None);
    scoped_raw_admission_v29::issued_role_tests_v29::run_issued_role_source_abi_v89(
        write_calls_v86::write_owner_count_v87(true, true, receiver, count),
        fe2o3_kernel_descriptor::AccessMode::WriteOnly,
        work,
        storage,
        &retained_floor,
        |source, abi, budget| {
            let floor = budget.storage();
            let launches = [ExplicitLaunchExtent::Exact {
                rank: 1,
                extents: [64, 1, 1],
            }];
            if mode == 1 {
                let error = source
                    .conditional_mixed_fixedpoint_output_v29(
                        ProductionKernelArgumentAbiInputV18 { roots: abi.roots },
                        &launches,
                        FormalIndexWidth::Bits64,
                        budget,
                    )
                    .err()
                    .expect("legacy CFG family must refuse the guarded Store");
                assert_eq!(budget.storage(), floor);
                reached.set(true);
                return Err(error);
            }
            let handoff = source.conditional_predicated_fixedpoint_output_v89(
                ProductionKernelArgumentAbiInputV18 { roots: abi.roots },
                &launches,
                FormalIndexWidth::Bits64,
                budget,
            )?;
            let result = (|| -> SourceOwnedResultV18<()> {
                handoff.check_original_source(source.source_ssa(budget)?, budget)?;
                handoff.check_original_argument_abi_v26(
                    ProductionKernelArgumentAbiInputV18 { roots: abi.roots },
                    budget,
                )?;
                assert_eq!(handoff.output(budget)?.execution().policy_version(), 11);
                let premises = handoff.runtime_premises(budget)?;
                assert_eq!(premises.len(), 2);
                for (argument, premise) in premises.iter().enumerate() {
                    assert_eq!(premise.original_argument() as usize, argument);
                    assert_eq!(
                        premise.access_counts(),
                        [
                            0,
                            if argument == receiver {
                                count as usize
                            } else {
                                0
                            }
                        ]
                    );
                }
                let occurrences = handoff.runtime_occurrences(budget)?;
                assert_eq!(occurrences.len(), count as usize);
                for occurrence in occurrences {
                    assert!(matches!(
                        occurrence.output_guard(),
                        ProductionMixedRuntimeGuardV89::ExplicitPredicate { .. }
                    ));
                    assert_eq!(
                        occurrence.explicit_formation_invocation_axis(),
                        Some(Axis::X)
                    );
                    assert!(occurrence.requires_address_formation_domain());
                    assert!(!occurrence.grants_artifact_or_launch_authority());
                }
                assert!(!handoff.runtime_requirements_are_discharged());
                assert!(!handoff.ranked_verification_is_complete());
                assert!(!handoff.grants_artifact_or_launch_authority());
                reached.set(true);
                if mode == 2 {
                    let old_work = budget.work();
                    let old_storage = budget.storage();
                    let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
                    let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, LIMIT);
                    foreign.reserve_storage(old_storage)?;
                    for _ in 0..2 {
                        assert!(matches!(
                            handoff.runtime_occurrences(&foreign),
                            Err(ProductionSourceOwnedViewErrorV18::Resource(
                                ArgumentResourceV1::Accounting
                            ))
                        ));
                    }
                    assert_eq!((budget.work(), budget.storage()), (old_work, old_storage));
                    assert_eq!(foreign.work(), 0);
                    return Err(ArgumentResourceV1::Accounting.into());
                }
                let descriptor = mixed_source_contract_descriptor_v26(
                    &abi.roots[0],
                    if mode >= 16 { mode - 15 } else { 0 },
                )
                .unwrap();
                let table = fe2o3_kernel_descriptor::decode_device_descriptor_table_v3(
                    &descriptor,
                    &mut |_| Ok::<(), ArgumentResourceV1>(()),
                );
                let table = match table {
                    Ok(table) => table,
                    Err(_) if mode >= 16 => {
                        return Err(ProductionSourceOwnedViewErrorV18::Binding(
                            "hostile descriptor is malformed",
                        ));
                    }
                    Err(error) => panic!("{error:?}"),
                };
                let mut bytes = vec![0; fe2o3_kernel_descriptor::mixed_conditional_v86::MAX_MIXED_CONTRACT_BYTES_V86];
                let n = handoff.emit_predicated_contract_v89(
                    0,
                    ProductionKernelArgumentAbiInputV18 { roots: abi.roots },
                    &table,
                    0,
                    &mut bytes,
                    budget,
                )?;
                let contract =
                    fe2o3_kernel_descriptor::mixed_conditional_v86::decode_mixed_contract_v86(
                        &bytes[..n],
                        &mut |_| Ok::<(), ArgumentResourceV1>(()),
                    )
                    .unwrap();
                assert_eq!(contract.occurrence_count(), count as usize);
                assert_eq!(contract.argument_count(), 2);
                for i in 0..contract.occurrence_count() {
                    let row = contract
                        .occurrence(i, &mut |_| Ok::<(), ArgumentResourceV1>(()))
                        .unwrap();
                    assert_eq!(row.argument as usize, receiver);
                    assert!(row.output_guard.edge().is_none());
                    assert_eq!(row.formation_envelope, fe2o3_kernel_descriptor::mixed_conditional_v26::MixedIndexEnvelopeV26::InvocationAxis { axis: 0 });
                    assert!(
                        matches!(row.access_envelope, fe2o3_kernel_descriptor::mixed_conditional_v26::MixedIndexEnvelopeV26::LogicalExtent { argument } if argument as usize == receiver)
                    );
                }
                assert!(
                    fe2o3_kernel_descriptor::mixed_conditional_v26::decode_mixed_contract_v26(
                        &bytes[..n],
                        &mut |_| Ok::<(), ArgumentResourceV1>(()),
                    )
                    .is_err()
                );
                Ok(())
            })();
            let released = handoff.discard(budget);
            if mode == 2 {
                assert!(result.is_err());
                assert!(released.is_err());
                retained_floor.set(Some(budget.storage()));
            } else {
                released?;
                assert_eq!(budget.storage(), floor);
            }
            result.map_err(Into::into)
        },
    )
}

#[test]
fn predicated_completion_v89_admits_original_calls_and_emits_distinct_no_edge_contract() {
    for receiver in [0, 1] {
        for count in [1, 2, 4] {
            let reached = std::cell::Cell::new(false);
            run_predicated_completion_v89(count, receiver, 0, LIMIT, LIMIT, &reached)
                .0
                .unwrap();
            assert!(reached.get());
        }
    }
}

#[test]
fn predicated_completion_v89_preserves_legacy_cfg_refusal() {
    let reached = std::cell::Cell::new(false);
    let error = run_predicated_completion_v89(2, 0, 1, LIMIT, LIMIT, &reached)
        .0
        .unwrap_err();
    assert!(format!("{error:?}").contains("mixed source conditional family is incomplete"));
    assert!(reached.get());
}

#[test]
fn predicated_completion_v90_cfg_seed_requires_independent_checked_write_rows() {
    use slice_view_v1::PREDICATED_SEED_FAULT_V90;
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            PREDICATED_SEED_FAULT_V90.set(None);
        }
    }
    for mode in [1, 2, 3] {
        let reset = Reset;
        PREDICATED_SEED_FAULT_V90.set(Some((mode, 0)));
        let reached = std::cell::Cell::new(false);
        let error = run_predicated_completion_v89(2, 0, 0, LIMIT, LIMIT, &reached)
            .0
            .unwrap_err();
        let (_, observed) = PREDICATED_SEED_FAULT_V90.get().unwrap();
        assert!(observed > 0, "fault must reach the actual seed: {error:?}");
        assert!(
            !reached.get(),
            "seed alone cannot publish completed write facts"
        );
        if mode == 2 {
            assert!(format!("{error:?}").contains("predicated source/native predicate differs"));
        }
        if mode == 3 {
            assert!(
                format!("{error:?}")
                    .contains("checked write requires its explicit-predicate consumer")
            );
        }
        drop(reset);
    }
}

#[test]
fn predicated_completion_v89_refuses_foreign_ledger_without_refunding_retained_output() {
    let reached = std::cell::Cell::new(false);
    assert!(
        run_predicated_completion_v89(2, 0, 2, LIMIT, LIMIT, &reached)
            .0
            .is_err()
    );
    assert!(reached.get());
}

#[test]
fn predicated_completion_v89_refuses_descriptor_and_geometry_substitution() {
    for mode in [16, 17, 18, 20, 21, 22] {
        let reached = std::cell::Cell::new(false);
        assert!(
            run_predicated_completion_v89(2, 0, mode, LIMIT, LIMIT, &reached)
                .0
                .is_err(),
            "mode {mode}"
        );
        assert!(reached.get());
    }
}

#[test]
fn predicated_completion_v89_whole_source_handoff_and_codec_exact_and_one_short() {
    let reached = std::cell::Cell::new(false);
    let (result, work, storage) = run_predicated_completion_v89(2, 0, 0, LIMIT, LIMIT, &reached);
    result.unwrap();
    assert!(reached.get());
    let (result, exact_work, exact_storage) =
        run_predicated_completion_v89(2, 0, 0, work, storage, &reached);
    result.unwrap();
    assert_eq!((work, storage), (exact_work, exact_storage));
    for (work, storage) in [(work - 1, storage), (work, storage - 1)] {
        assert!(
            run_predicated_completion_v89(2, 0, 0, work, storage, &reached)
                .0
                .is_err()
        );
    }
}

#[test]
fn predicated_completion_v89_scalar_transport_requires_exact_intrinsic_write_shape() {
    use fe2o3_kernel_ir::{Operation, ValueDef};
    let before = [ValueId(1), ValueId(2), ValueId(3)];
    let after = [ValueId(4), ValueId(5), ValueId(6)];
    let access = MemoryAccess::new(AddressSpace::Global, 4);
    let operation = |values: [ValueId; 3]| {
        Operation::new(
            Vec::new(),
            OperationKind::GuardedStore {
                pointer: values[0],
                value: values[1],
                predicate: values[2],
                access,
            },
        )
    };
    let input = operation(before);
    let output = operation(after);
    for (row, values) in [(&input, before), (&output, after)] {
        let mut visited = Vec::new();
        row.kind.visit_operands(|value| visited.push(value));
        assert_eq!(visited, [values[0], values[2], values[1]]);
        let rhs = optimized_scalar_store_rhs_operand_v90(&row.kind).unwrap();
        assert_eq!(rhs, 2);
        assert_eq!(visited[rhs as usize], values[1]);
        assert_ne!(visited[1], values[1]);
        let ordinary = OperationKind::Store {
            pointer: values[0],
            value: values[1],
            access,
        };
        visited.clear();
        ordinary.visit_operands(|value| visited.push(value));
        let rhs = optimized_scalar_store_rhs_operand_v90(&ordinary).unwrap();
        assert_eq!(rhs, 1);
        assert_eq!(visited[rhs as usize], values[1]);
        assert!(
            optimized_scalar_store_rhs_operand_v90(&OperationKind::Load {
                pointer: values[0],
                access
            })
            .is_none()
        );
    }
    assert!(optimized_guarded_scalar_shape_v89(
        &input, &output, true, before, after
    ));
    assert!(!optimized_guarded_scalar_shape_v89(
        &input, &output, false, before, after
    ));
    for which in 0..2 {
        for operand in 0..3 {
            let mut values = if which == 0 { before } else { after };
            values[operand] = ValueId(91);
            let changed = operation(values);
            assert!(!optimized_guarded_scalar_shape_v89(
                if which == 0 { &changed } else { &input },
                if which == 1 { &changed } else { &output },
                true,
                before,
                after,
            ));
        }
        for mutation in 0..5 {
            let mut changed = if which == 0 {
                input.clone()
            } else {
                output.clone()
            };
            match mutation {
                0 => changed.results.push(ValueDef::new(ValueId(90), Type::BOOL)),
                1 => {
                    let OperationKind::GuardedStore { access, .. } = &mut changed.kind else {
                        unreachable!()
                    };
                    access.volatile = true;
                }
                2 => {
                    let OperationKind::GuardedStore { access, .. } = &mut changed.kind else {
                        unreachable!()
                    };
                    access.alignment = 8;
                }
                3 => {
                    let OperationKind::GuardedStore { access, .. } = &mut changed.kind else {
                        unreachable!()
                    };
                    access.address_space = AddressSpace::Private;
                }
                4 => {
                    let values = if which == 0 { before } else { after };
                    changed.kind = OperationKind::Store {
                        pointer: values[0],
                        value: values[1],
                        access,
                    };
                }
                _ => unreachable!(),
            }
            assert!(!optimized_guarded_scalar_shape_v89(
                if which == 0 { &changed } else { &input },
                if which == 1 { &changed } else { &output },
                true,
                before,
                after,
            ));
        }
    }
}
