#[test]
fn standalone_checkpoint_still_requires_full_old_and_new_overlap() {
    for (peak, succeeds) in [(9, true), (8, false)] {
        let mut session = begin_production_pliron_pass_contract_session_with_resource_limits_v1(
            provider(&[1, 1]),
            ProductionAnalysisResourceLimitsV1::new(5, 14),
        )
        .unwrap();
        let called = Cell::new(false);
        let result = session.run_contiguous_pass_with_resource_limits_v1(
            KernelCheckPassKindV1::TensorLayout,
            ProductionAnalysisResourceLimitsV1::new(13, peak),
            || {
                called.set(true);
                Ok::<_, ()>(())
            },
        );
        assert!(called.get());
        if succeeds {
            result.unwrap().unwrap();
            let bound = session.last_checkpoint_resource_upper_bound_v1().unwrap();
            assert_eq!(bound.work_upper_bound(), 13);
            assert_eq!(bound.retained_storage_upper_bound(), 5);
            assert_eq!(bound.peak_storage_upper_bound(), 9);
            assert_eq!(session.certificates.len(), 1);
        } else {
            assert_eq!(
                result,
                Err(PlironPassPreservationErrorV1::ResourceLimit {
                    resource: "peak storage upper bound",
                })
            );
            assert!(session.certificates.is_empty());
            assert!(session.last_checkpoint_resource_upper_bound_v1().is_none());
            assert_eq!(session.next, 0);
        }
    }
}

#[test]
fn standalone_finish_still_requires_live_snapshot_and_report_overlap() {
    for (peak, succeeds) in [(19, true), (18, false)] {
        let values = vec![7; MAX_PLIRON_PASS_CONTRACTS_V1 + 1];
        let mut session =
            begin_production_pliron_pass_contract_session_v1(provider(&values)).unwrap();
        for contract in PRODUCTION_PLIRON_PASS_CONTRACTS_V1 {
            session
                .run_contiguous_pass(contract.pass(), || Ok::<_, ()>(()))
                .unwrap()
                .unwrap();
        }
        // Canonical4 + certificates9 + fields2 = retained15; the old
        // snapshot4 remains live while the exact output owner is retained.
        let result = session
            .finish_with_resource_limits_v1(ProductionAnalysisResourceLimitsV1::new(17, peak));
        if succeeds {
            let report = result.unwrap();
            assert!(report.is_exact_identity());
            assert_eq!(report.certificates().len(), 9);
            assert_eq!(report.resource_upper_bound.work_upper_bound(), 17);
            assert_eq!(
                report.resource_upper_bound.retained_storage_upper_bound(),
                15
            );
            assert_eq!(report.resource_upper_bound.peak_storage_upper_bound(), 19);
        } else {
            assert_eq!(
                result,
                Err(PlironPassPreservationErrorV1::ResourceLimit {
                    resource: "peak storage upper bound",
                })
            );
        }
    }
}
