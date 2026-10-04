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
                assert!(
                    source
                        .conditional_mixed_fixedpoint_output_v29(
                            ProductionKernelArgumentAbiInputV18 { roots: abi.roots },
                            &launches,
                            FormalIndexWidth::Bits64,
                            budget,
                        )
                        .is_err()
                );
                assert_eq!(budget.storage(), floor);
                reached.set(true);
                return Ok(());
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
    run_predicated_completion_v89(2, 0, 1, LIMIT, LIMIT, &reached)
        .0
        .unwrap();
    assert!(reached.get());
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
