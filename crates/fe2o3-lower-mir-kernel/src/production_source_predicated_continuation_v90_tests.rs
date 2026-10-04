use super::*;
use fe2o3_kernel_ir::{ExplicitLaunchExtent, FormalIndexWidth};

include!("production_source_contract_descriptor_fixture_v89.rs");

const LIMIT: usize = 1_000_000_000;
type Error = Box<dyn std::error::Error>;

fn run_continuation(
    receiver: usize,
    count: u32,
    mode: u8,
    work: usize,
    storage: usize,
    reached: &std::cell::Cell<bool>,
) -> (Result<(), Error>, usize, usize) {
    let retained_floor = std::cell::Cell::new(None);
    scoped_raw_admission_v29::issued_role_tests_v29::run_issued_role_source_abi_v89(
        write_calls_v86::write_owner_count_v87(true, true, receiver, count),
        fe2o3_kernel_descriptor::AccessMode::WriteOnly,
        work,
        storage,
        &retained_floor,
        |source, abi, budget| -> Result<(), Error> {
            let floor = budget.storage();
            let launches = [ExplicitLaunchExtent::Exact {
                rank: 1,
                extents: [64, 1, 1],
            }];
            let prefix = source.conditional_predicated_fixedpoint_output_v89(
                ProductionKernelArgumentAbiInputV18 { roots: abi.roots },
                &launches,
                FormalIndexWidth::Bits64,
                budget,
            )?;
            let prefix_floor = budget.storage();
            let result = (|| -> Result<(), Error> {
                assert_eq!(prefix.checked_prefix_v29(budget)?.policy_version(), 11);
                let motion = prefix.prepare_predicated_fixedpoint_licm_v90(budget)?;
                let motion_floor = budget.storage();
                let result = (|| -> Result<(), Error> {
                    assert!(std::ptr::eq(motion.prefix(budget)?, &prefix));
                    motion.replay(budget)?;
                    let consensus = motion.prepare_predicated_store_consensus_v90(budget)?;
                    let consensus_floor = budget.storage();
                    let result = (|| -> Result<(), Error> {
                        assert!(std::ptr::eq(consensus.relocation(budget)?, &motion));
                        if (1..=13).contains(&mode) {
                            let error = consensus
                                .complete_native_fault_v90(mode, budget)
                                .err()
                                .expect("hostile final join must refuse");
                            assert_eq!(budget.storage(), consensus_floor);
                            reached.set(true);
                            return Err(Box::new(error));
                        }
                        let native = consensus.complete_native_v90(budget)?;
                        let result = (|| -> Result<(), Error> {
                            assert!(std::ptr::eq(native.relocation(budget)?, &motion));
                            assert!(std::ptr::eq(
                                native.store_consensus_v46(budget)?.unwrap(),
                                &consensus
                            ));
                            assert!(std::ptr::eq(
                                native.output(budget)?,
                                consensus.output(budget)?
                            ));
                            native.check_original_source(source.source_ssa(budget)?, budget)?;
                            native.check_original_argument_abi_v26(
                                ProductionKernelArgumentAbiInputV18 { roots: abi.roots },
                                budget,
                            )?;
                            assert_eq!(
                                native.runtime_premises(budget)?,
                                prefix.runtime_premises(budget)?
                            );
                            assert_eq!(
                                native.launch_context(budget)?,
                                prefix.launch_context(budget)?
                            );
                            let occurrences = native.runtime_occurrences(budget)?;
                            assert_eq!(occurrences.len(), count as usize);
                            for (row, original) in
                                occurrences.iter().zip(prefix.runtime_occurrences(budget)?)
                            {
                                assert_eq!(row.original_operation(), original.original_operation());
                                assert_eq!(
                                    row.original_address_formation(),
                                    original.original_address_formation()
                                );
                                assert!(matches!(
                                    row.output_guard(),
                                    ProductionMixedRuntimeGuardV89::ExplicitPredicate { .. }
                                ));
                                assert_eq!(row.explicit_formation_invocation_axis(), Some(Axis::X));
                                assert!(row.requires_address_formation_domain());
                                assert!(!row.grants_artifact_or_launch_authority());
                            }
                            let histories = native.native_histories(budget)?;
                            let functions = &native.output(budget)?.module().functions;
                            assert_eq!(histories.len(), functions.len());
                            for (history, function) in histories.iter().zip(functions) {
                                assert_eq!(history.is_some(), function.body.is_some());
                            }
                            assert!(native.source_roles_are_complete());
                            assert!(native.final_native_completion_is_complete());
                            assert!(!native.runtime_requirements_are_discharged());
                            assert!(!native.ranked_verification_is_complete());
                            assert!(!native.grants_artifact_or_launch_authority());
                            reached.set(true);
                            if mode == 20 {
                                let old = (budget.work(), budget.storage());
                                let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
                                let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, LIMIT);
                                foreign.reserve_storage(old.1)?;
                                for _ in 0..2 {
                                    assert!(matches!(
                                        native.runtime_occurrences(&foreign),
                                        Err(ProductionSourceOwnedViewErrorV18::Resource(
                                            ArgumentResourceV1::Accounting
                                        ))
                                    ));
                                }
                                assert_eq!(foreign.work(), 0);
                                assert_eq!((budget.work(), budget.storage()), old);
                                return Err(Box::new(ProductionSourceOwnedViewErrorV18::Resource(
                                    ArgumentResourceV1::Accounting,
                                )));
                            }
                            if mode == 21 {
                                let old_work = budget.work();
                                budget.release_storage(1)?;
                                for _ in 0..2 {
                                    assert!(matches!(
                                        native.runtime_occurrences(budget),
                                        Err(ProductionSourceOwnedViewErrorV18::Resource(
                                            ArgumentResourceV1::Accounting
                                        ))
                                    ));
                                }
                                assert_eq!(budget.work(), old_work);
                                return Err(Box::new(ProductionSourceOwnedViewErrorV18::Resource(
                                    ArgumentResourceV1::Accounting,
                                )));
                            }
                            let descriptor =
                                mixed_source_contract_descriptor_v26(&abi.roots[0], 0).unwrap();
                            let table = fe2o3_kernel_descriptor::decode_device_descriptor_table_v3(
                                &descriptor,
                                &mut |_| Ok::<(), ArgumentResourceV1>(()),
                            )
                            .unwrap();
                            let mut bytes = vec![0; fe2o3_kernel_descriptor::mixed_conditional_v86::MAX_MIXED_CONTRACT_BYTES_V86];
                            let n = native.emit_predicated_contract_v90(
                                0,
                                ProductionKernelArgumentAbiInputV18 { roots: abi.roots },
                                &table,
                                0,
                                &mut bytes,
                                budget,
                            )?;
                            let contract = fe2o3_kernel_descriptor::mixed_conditional_v86::decode_mixed_contract_v86(
                                &bytes[..n], &mut |_| Ok::<(), ArgumentResourceV1>(()),
                            ).unwrap();
                            assert_eq!(contract.occurrence_count(), count as usize);
                            for index in 0..contract.occurrence_count() {
                                let row = contract
                                    .occurrence(index, &mut |_| Ok::<(), ArgumentResourceV1>(()))
                                    .unwrap();
                                assert_eq!(row.argument as usize, receiver);
                                assert!(row.output_guard.edge().is_none());
                                assert_eq!(row.formation_envelope,
                                    fe2o3_kernel_descriptor::mixed_conditional_v26::MixedIndexEnvelopeV26::InvocationAxis { axis: 0 });
                            }
                            assert!(fe2o3_kernel_descriptor::mixed_conditional_v26::decode_mixed_contract_v26(
                                &bytes[..n], &mut |_| Ok::<(), ArgumentResourceV1>(()),
                            ).is_err());
                            Ok(())
                        })();
                        let released = native.discard(budget);
                        if matches!(mode, 20 | 21) {
                            assert!(released.is_err());
                        } else {
                            released?;
                            assert_eq!(budget.storage(), consensus_floor);
                        }
                        result
                    })();
                    let released = consensus.discard(budget);
                    if matches!(mode, 20 | 21) && reached.get() {
                        assert!(released.is_err());
                    } else {
                        released?;
                        assert_eq!(budget.storage(), motion_floor);
                    }
                    result
                })();
                let released = motion.discard(budget);
                if matches!(mode, 20 | 21) && reached.get() {
                    assert!(released.is_err());
                } else {
                    released?;
                    assert_eq!(budget.storage(), prefix_floor);
                }
                result
            })();
            let released = prefix.discard(budget);
            if matches!(mode, 20 | 21) && reached.get() {
                assert!(released.is_err());
                retained_floor.set(Some(budget.storage()));
            } else {
                released?;
                assert_eq!(budget.storage(), floor);
            }
            result
        },
    )
}

#[test]
fn predicated_continuation_v90_runs_full_policy11_licm_consensus_native_and_distinct_codec() {
    for receiver in [0, 1] {
        for count in [1, 2, 4] {
            let reached = std::cell::Cell::new(false);
            run_continuation(receiver, count, 0, LIMIT, LIMIT, &reached)
                .0
                .unwrap();
            assert!(reached.get());
        }
    }
}

#[test]
fn predicated_continuation_v90_rechecks_every_final_census_guard_and_formation() {
    for mode in 1..=13 {
        let reached = std::cell::Cell::new(false);
        let error = run_continuation(0, 2, mode, LIMIT, LIMIT, &reached)
            .0
            .unwrap_err();
        assert!(reached.get(), "mode {mode}: {error:?}");
        assert!(
            error
                .downcast_ref::<ProductionMixedLicmCompletionErrorV28>()
                .is_some(),
            "mode {mode}: {error:?}"
        );
    }
}

#[test]
fn predicated_continuation_v90_foreign_ledger_and_retained_undercut_are_sticky() {
    for mode in [20, 21] {
        let reached = std::cell::Cell::new(false);
        assert!(
            run_continuation(1, 2, mode, LIMIT, LIMIT, &reached)
                .0
                .is_err()
        );
        assert!(reached.get());
    }
}

#[test]
fn predicated_continuation_v90_full_source_transaction_exact_and_one_short() {
    let reached = std::cell::Cell::new(false);
    let (result, work, storage) = run_continuation(0, 2, 0, LIMIT, LIMIT, &reached);
    result.unwrap();
    assert!(reached.get());
    let (result, exact_work, exact_storage) = run_continuation(0, 2, 0, work, storage, &reached);
    result.unwrap();
    assert_eq!((work, storage), (exact_work, exact_storage));
    for (work, storage) in [(work - 1, storage), (work, storage - 1)] {
        assert!(
            run_continuation(0, 2, 0, work, storage, &reached)
                .0
                .is_err()
        );
    }
}

#[test]
fn predicated_continuation_v90_retains_distinct_public_closed_owner_families() {
    use std::any::TypeId;
    fn public_family<R: crate::ProductionContinuationOccurrenceV90>() {}
    public_family::<crate::ProductionMixedRuntimeOccurrenceV26>();
    public_family::<crate::ProductionMixedRuntimeOccurrenceV89>();
    assert_ne!(
        TypeId::of::<ProductionMixedFixedpointLicmRelocationV29<'static, 'static, 'static>>(),
        TypeId::of::<ProductionPredicatedFixedpointLicmRelocationV90<'static, 'static, 'static>>()
    );
    assert_ne!(
        TypeId::of::<ProductionMixedFixedpointStoreConsensusV46<'static, 'static, 'static, 'static>>(
        ),
        TypeId::of::<ProductionPredicatedStoreConsensusV90<'static, 'static, 'static, 'static>>()
    );
    assert_ne!(
        TypeId::of::<
            ProductionConditionalMixedFixedpointLicmOutputHandoffV29<
                'static,
                'static,
                'static,
                'static,
            >,
        >(),
        TypeId::of::<
            ProductionConditionalPredicatedLicmOutputHandoffV90<'static, 'static, 'static, 'static>,
        >()
    );
}
