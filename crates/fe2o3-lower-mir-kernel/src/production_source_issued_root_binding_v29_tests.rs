thread_local! {
    static ISSUED_ROOT_BINDING_FAULT_V29: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
    static ISSUED_ROOT_BINDING_VISITED_V29: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn observe_issued_root_binding_v29(
    pending: &mut PendingScopedRootEmissionV29,
    instances: &ExecutionInstancesV29<'_>,
    _: &SourceReferencePlanV29<'_, '_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let root = instances.root();
    let occurrences = instances.occurrences(root).unwrap();
    let entry = occurrences
        .entry_definitions()
        .iter()
        .find(|row| row.variable().get() == 1)
        .expect("genuine first descriptor entry definition");
    assert!(
        entry.value().is_none(),
        "repeated source borrows keep this carrier unpromoted"
    );
    let sidecar = pending
        .sidecars
        .rows
        .iter_mut()
        .find(|row| row.source_call_instance == Some(root))
        .unwrap();
    assert_eq!(sidecar.parameter_bindings.len(), 2);
    let first = sidecar
        .parameter_bindings
        .iter()
        .position(|row| row.semantic_local.index() == 1)
        .unwrap();
    let second = sidecar
        .parameter_bindings
        .iter()
        .position(|row| row.semantic_local.index() == 2)
        .unwrap();
    let body = pending.function.body.as_mut().unwrap();
    assert_eq!(body.parameters.len(), 2);
    assert_eq!(
        sidecar.parameter_bindings[first].kernel_ir_value,
        body.parameters[0]
    );
    assert_eq!(
        sidecar.parameter_bindings[second].kernel_ir_value,
        body.parameters[1]
    );
    let peer = sidecar.parameter_bindings[second];
    match ISSUED_ROOT_BINDING_FAULT_V29.get() {
        0 => {}
        1 => {
            sidecar.parameter_bindings.remove(first);
        }
        2 => {
            let duplicate = sidecar.parameter_bindings[first];
            emission_push_v1(&mut sidecar.parameter_bindings, duplicate, budget)?;
        }
        3 => sidecar.parameter_bindings[first].kernel_ir_value = peer.kernel_ir_value,
        4 => sidecar.parameter_bindings[first].semantic_local = peer.semantic_local,
        5 => {
            sidecar.parameter_bindings[first].semantic_function =
                SemanticFunctionIdV1::from_index(u32::MAX)
        }
        6 => {
            sidecar.parameter_bindings[first].correspondence_owner =
                SemanticFunctionIdV1::from_index(u32::MAX)
        }
        7 => body.parameters.swap(0, 1),
        8 => {
            // Coherent same-typed parameter/locator substitution still cannot
            // make the receiver originate from the other source argument.
            body.parameters.swap(0, 1);
            sidecar.parameter_bindings[first].kernel_ir_value = body.parameters[0];
            sidecar.parameter_bindings[second].kernel_ir_value = body.parameters[1];
        }
        _ => unreachable!(),
    }
    ISSUED_ROOT_BINDING_VISITED_V29.set(true);
    Ok(())
}

fn with_issued_root_binding_fault_v29<T>(fault: u8, run: impl FnOnce() -> T) -> (T, bool) {
    struct Restore(Option<RootExecutionArchiveObserverV29>, u8, bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            ROOT_EXECUTION_ARCHIVE_OBSERVER_V29.set(self.0);
            ISSUED_ROOT_BINDING_FAULT_V29.set(self.1);
            ISSUED_ROOT_BINDING_VISITED_V29.set(self.2);
        }
    }
    let _scope = Restore(
        ROOT_EXECUTION_ARCHIVE_OBSERVER_V29.replace(Some(observe_issued_root_binding_v29)),
        ISSUED_ROOT_BINDING_FAULT_V29.replace(fault),
        ISSUED_ROOT_BINDING_VISITED_V29.replace(false),
    );
    let result = run();
    (result, ISSUED_ROOT_BINDING_VISITED_V29.get())
}

#[test]
fn issued_root_binding_authenticates_unpromoted_original_carrier_without_ssa_fabrication() {
    let completed = std::cell::Cell::new(false);
    let ((result, _, _), observed) = with_issued_root_binding_fault_v29(0, || {
        run_issued_role_source_shape_v18(
            2,
            1,
            ISSUED_ROLE_LIMIT,
            ISSUED_ROLE_LIMIT,
            &std::cell::Cell::new(None),
            |original, budget| {
                let rows = issued_rows_v18(original);
                assert_eq!(
                    (rows.sources.len(), rows.issuers.len(), rows.accesses.len()),
                    (2, 2, 2)
                );
                assert_eq!(rows.issuers[0].root_parameter, 0);
                assert_eq!(rows.issuers[1].root_parameter, 0);
                assert_eq!(rows.issuers[0].root_input, rows.issuers[1].root_input);
                assert_ne!(rows.issuers[0].definition, rows.issuers[1].definition);
                check_immutable_issued_roles_v18(original, 0, rows, budget)?;
                completed.set(true);
                Ok(())
            },
        )
    });
    result.unwrap();
    assert!(observed && completed.get());
}

#[test]
fn issued_root_binding_rejects_missing_duplicate_foreign_and_coherent_peer_substitutions() {
    for fault in 1..=8 {
        let completed = std::cell::Cell::new(false);
        let ((result, _, _), observed) = with_issued_root_binding_fault_v29(fault, || {
            run_issued_role_source_shape_v18(
                2,
                1,
                ISSUED_ROLE_LIMIT,
                ISSUED_ROLE_LIMIT,
                &std::cell::Cell::new(None),
                |_, _| {
                    completed.set(true);
                    Ok(())
                },
            )
        });
        assert!(
            observed,
            "hostile original compiler output was not observed: {fault}"
        );
        assert!(
            !completed.get(),
            "hostile root input reached immutable consumer: {fault}"
        );
        assert!(
            result.is_err(),
            "root binding substitution admitted: {fault}"
        );
    }
}

#[test]
fn issued_root_binding_multiissuer_transaction_has_exact_and_one_short_resources() {
    let run = |work, storage| {
        run_issued_role_source_shape_v18(
            2,
            1,
            work,
            storage,
            &std::cell::Cell::new(None),
            |original, budget| {
                let rows = issued_rows_v18(original);
                assert_eq!(
                    (rows.sources.len(), rows.issuers.len(), rows.accesses.len()),
                    (2, 2, 2)
                );
                check_immutable_issued_roles_v18(original, 0, rows, budget)
            },
        )
    };
    let (result, work, peak) = run(ISSUED_ROLE_LIMIT, ISSUED_ROLE_LIMIT);
    result.unwrap();
    let (result, exact_work, exact_peak) = run(work, peak);
    result.unwrap();
    assert_eq!((exact_work, exact_peak), (work, peak));
    for (work_limit, storage_limit) in [(work - 1, peak), (work, peak - 1)] {
        let (result, _, _) = run(work_limit, storage_limit);
        let error = match result {
            Err(ProductionSourceOwnedViewErrorV18::Resource(error))
            | Err(ProductionSourceOwnedViewErrorV18::Source(
                ProductionPendingScopedSourceErrorV29::Source(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error),
                ),
            )) => error,
            other => {
                panic!("one-short issuer transaction must retain its resource failure: {other:?}")
            }
        };
        match error {
            ArgumentResourceV1::Work(limit) if work_limit < work => {
                assert_eq!(limit.limit(), work_limit);
                assert!(limit.actual() > work_limit);
            }
            ArgumentResourceV1::Storage(limit) if storage_limit < peak => {
                assert_eq!(limit.limit(), storage_limit);
                assert_eq!(limit.actual(), peak);
            }
            other => panic!("wrong one-short resource dimension: {other:?}"),
        }
    }
}
