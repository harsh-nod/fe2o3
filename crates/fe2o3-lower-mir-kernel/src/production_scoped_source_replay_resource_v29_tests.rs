#[derive(Debug)]
struct OwningStageProbe {
    result: Result<(), ScopedModuleErrorV29>,
    work: usize,
    extra_storage: usize,
    denied_work: Option<usize>,
    denied_storage: Option<usize>,
}

fn owning_stage_probe(
    kind: ModuleFixture,
    preexisting: bool,
    replay: bool,
    allowance: Option<(usize, usize)>,
) -> OwningStageProbe {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let (result, used, extra_storage, denied_storage) = {
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        let source = owning_source_fixture(kind, preexisting, &mut budget).unwrap();
        let input_storage = source.input.retained_storage;
        let mut donor = Some(source);
        let owner = if replay {
            Some(
                SourceOwnedScopedModuleV29::try_new(
                    &mut donor,
                    ProductionSemanticKirLimitsV1::default(),
                    &mut budget,
                )
                .unwrap(),
            )
        } else {
            None
        };
        // Establish a new peak before the measured stage; setup is not coverage.
        budget
            .reserve_storage(budget.peak_storage() + 1 - budget.storage())
            .unwrap();
        if let Some((work_left, storage_left)) = allowance {
            budget
                .charge_work(MODULE_LIMIT - budget.work() - work_left)
                .unwrap();
            budget
                .reserve_storage(MODULE_LIMIT - budget.storage() - storage_left)
                .unwrap();
        }
        let entry = budget.storage();
        let before = budget.work();
        let result = if let Some(owner) = owner {
            let identity = *owner.pending.graph.identity();
            let bytes = owner.pending.graph.canonical_bytes().as_ptr();
            let result = owner.replay(&mut budget);
            assert_eq!(owner.pending.graph.identity(), &identity);
            assert_eq!(owner.pending.graph.canonical_bytes().as_ptr(), bytes);
            assert_eq!(budget.storage(), entry);
            let retained = owner.retained_storage;
            drop(owner);
            budget.release_storage(retained).unwrap();
            result
        } else {
            let result = SourceOwnedScopedModuleV29::try_new(
                &mut donor,
                ProductionSemanticKirLimitsV1::default(),
                &mut budget,
            );
            assert!(
                donor.is_none(),
                "probe failed before consuming the prepared donor"
            );
            let result = match result {
                Ok(owner) => {
                    assert_eq!(
                        budget.storage(),
                        entry - input_storage + owner.retained_storage
                    );
                    let retained = owner.retained_storage;
                    drop(owner);
                    budget.release_storage(retained).unwrap();
                    Ok(())
                }
                Err(error) => Err(error),
            };
            assert_eq!(budget.storage(), entry - input_storage);
            result
        };
        let used = budget.work() - before;
        let extra = budget.peak_storage() - entry;
        let denied = budget.failed_storage();
        let remaining = budget.storage();
        budget.release_storage(remaining).unwrap();
        (result, used, extra, denied)
    };
    OwningStageProbe {
        result,
        work: used,
        extra_storage,
        denied_work: work.failed_work(),
        denied_storage,
    }
}

fn owning_resource(error: ScopedModuleErrorV29) -> ArgumentResourceV1 {
    match error {
        ScopedModuleErrorV29::Occurrences(
            fe2o3_pliron::ProductionSemanticSsaOccurrenceErrorV1::Resource(error),
        )
        | ScopedModuleErrorV29::Source(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error),
        )
        | ScopedModuleErrorV29::Source(ProductionSemanticKirErrorV1::AssertOrigin(
            SemanticKirAssertOriginErrorV1::Resource(error),
        ))
        | ScopedModuleErrorV29::Canonical(
            fe2o3_kernel_ir::CanonicalKernelIrReplayAdmissionErrorV15::Resource(error),
        )
        | ScopedModuleErrorV29::Canonical(
            fe2o3_kernel_ir::CanonicalKernelIrReplayAdmissionErrorV15::Decode(
                fe2o3_kernel_ir::KernelIrDecodeError::Resource(error),
            ),
        ) => error,
        other => panic!("expected a typed resource refusal: {other:?}"),
    }
}

#[test]
fn scoped_source_owner_constructor_and_replay_obey_exact_stage_budgets() {
    for kind in [
        ModuleFixture::Ordinary,
        ModuleFixture::Mixed,
        ModuleFixture::Array,
    ] {
        for preexisting in [false, true] {
            for replay in [false, true] {
                let measured = owning_stage_probe(kind, preexisting, replay, None);
                measured.result.unwrap();
                assert!(measured.work > 0 && measured.extra_storage > 0);
                let exact = owning_stage_probe(
                    kind,
                    preexisting,
                    replay,
                    Some((measured.work, measured.extra_storage)),
                );
                exact.result.unwrap();
                assert_eq!(exact.work, measured.work);
                assert_eq!(exact.extra_storage, measured.extra_storage);
                let under_work = owning_stage_probe(
                    kind,
                    preexisting,
                    replay,
                    Some((measured.work - 1, measured.extra_storage)),
                );
                assert!(matches!(
                    owning_resource(under_work.result.unwrap_err()),
                    ArgumentResourceV1::Work(_)
                ));
                assert!(under_work.denied_work.is_some());
                let under_storage = owning_stage_probe(
                    kind,
                    preexisting,
                    replay,
                    Some((measured.work, measured.extra_storage - 1)),
                );
                assert!(matches!(
                    owning_resource(under_storage.result.unwrap_err()),
                    ArgumentResourceV1::Storage(_)
                ));
                assert!(under_storage.denied_storage.is_some());
            }
        }
    }
}

#[test]
fn scoped_source_owner_attempt_never_accepts_or_refunds_a_replaced_ledger() {
    for outcome in 0..3 {
        let mut original_work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut original_work, MODULE_LIMIT);
        let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, MODULE_LIMIT);
        budget.reserve_storage(23).unwrap();
        foreign.reserve_storage(97).unwrap();
        let original = budget.work_ledger_identity_v1();
        let other = foreign.work_ledger_identity_v1();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            scoped_module_attempt_v29(&mut budget, 23, |budget| {
                budget.reserve_storage(31)?;
                std::mem::swap(budget, &mut foreign);
                match outcome {
                    0 => Ok(()),
                    1 => Err(scoped_module_error_v29().into()),
                    _ => panic!("original panic must survive ledger substitution"),
                }
            })
        }));
        assert!(budget.work_ledger_identity_v1() == other);
        assert!(foreign.work_ledger_identity_v1() == original);
        assert_eq!(budget.storage(), 97);
        assert_eq!(foreign.storage(), 54);
        if outcome == 2 {
            assert_eq!(
                *result.unwrap_err().downcast::<&str>().unwrap(),
                "original panic must survive ledger substitution"
            );
        } else {
            assert!(matches!(
                result.unwrap(),
                Err(ScopedModuleErrorV29::Source(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Accounting
                    )
                ))
            ));
        }
    }
}
