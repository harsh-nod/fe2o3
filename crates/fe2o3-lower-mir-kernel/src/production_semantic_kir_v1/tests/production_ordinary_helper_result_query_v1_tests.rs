mod diagnostic_transport_query_tests_v1 {
    use super::*;

    const QUERY_CHARGES: &[usize] = &[16, 1, 1, 1, 1, 3, 1, 1, 1, 8];

    #[test]
    fn shared_result_query_preserves_the_legacy_charge_prefix_and_avoids_only_rc() {
        let types = types();
        let function = function(ARRAY, integer_cast(8, false));
        let mut charges = Vec::new();
        check_ordinary_helper_result_transport_v1(
            &types,
            &function,
            &mut |amount| {
                charges.push(amount);
                Ok(())
            },
            7,
        )
        .unwrap();
        assert_eq!(charges, QUERY_CHARGES);
        assert_eq!(charges.iter().sum::<usize>(), 34);
        let legacy_charges = [QUERY_CHARGES, &[1]].concat();
        for limit in 0..=35 {
            let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(limit);
            let result = plan_ordinary_helper_result_v1(&types, &function, &mut work, 7);
            let mut accepted = 0;
            let mut denied = None;
            for charge in &legacy_charges {
                if accepted + charge > limit {
                    denied = Some(accepted + charge);
                    break;
                }
                accepted += charge;
            }
            assert_eq!(work.work(), accepted, "limit {limit}");
            assert_eq!(work.failed_work(), denied, "limit {limit}");
            assert_eq!(result.is_ok(), denied.is_none(), "limit {limit}");
        }
    }

    #[test]
    fn diagnostic_query_has_independent_exact_under_and_shared_counter_boundaries() {
        let types = types();
        let function = function(ARRAY, integer_cast(8, false));
        for limit in [41, 40] {
            let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(limit);
            work.charge_work(7).unwrap();
            let result = check_ordinary_helper_result_transport_v1(
                &types,
                &function,
                &mut |amount| charge_ordinary_helper_result_work_v1(&mut work, amount),
                7,
            );
            if limit == 41 {
                assert!(result.is_ok());
                assert_eq!((work.work(), work.failed_work()), (41, None));
            } else {
                assert!(matches!(
                    result,
                    Err(ProductionSemanticKirErrorV1::ResourceLimit {
                        resource: ProductionSemanticKirResourceV1::AnalysisWork,
                        actual: 41,
                        limit: 40,
                    })
                ));
                assert_eq!((work.work(), work.failed_work()), (33, Some(41)));
                assert!(charge_ordinary_helper_result_work_v1(&mut work, 0).is_err());
                assert_eq!((work.work(), work.failed_work()), (33, Some(41)));
            }
        }
        for limit in [68, 67] {
            let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(limit);
            for ordinal in 0..2 {
                let result = check_ordinary_helper_result_transport_v1(
                    &types,
                    &function,
                    &mut |amount| charge_ordinary_helper_result_work_v1(&mut work, amount),
                    7,
                );
                assert_eq!(result.is_ok(), ordinal == 0 || limit == 68);
            }
            assert_eq!(work.work(), if limit == 68 { 68 } else { 60 });
            assert_eq!(work.failed_work(), (limit == 67).then_some(68));
        }
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(100);
        let result = check_ordinary_helper_result_transport_v1(
            &types,
            &function,
            &mut |amount| charge_ordinary_helper_result_work_v1(&mut work, amount),
            6,
        );
        assert!(matches!(
            result,
            Err(ProductionSemanticKirErrorV1::ResourceLimit {
                resource: ProductionSemanticKirResourceV1::AnalysisStorage,
                actual: 7,
                limit: 6,
            })
        ));
        assert_eq!(work.work(), 20);
    }

    #[test]
    fn diagnostic_queries_return_caller_errors_without_treating_them_as_ineligibility() {
        let types = types();
        let function = function(ARRAY, integer_cast(8, false));
        for stop in 0..QUERY_CHARGES.len() {
            let mut calls = 0;
            let error = check_ordinary_helper_result_transport_v1(
                &types,
                &function,
                &mut |_| {
                    let current = calls;
                    calls += 1;
                    if current == stop {
                        Err(unsupported(
                            71,
                            Some(9),
                            Some(4),
                            "caller query charge refusal",
                        ))
                    } else {
                        Ok(())
                    }
                },
                7,
            )
            .unwrap_err();
            assert!(matches!(
                error,
                ProductionSemanticKirErrorV1::Unsupported {
                    function: 71,
                    block: Some(9),
                    statement: Some(4),
                    detail: "caller query charge refusal",
                }
            ));
            assert_eq!(calls, stop + 1);
        }
        let error = check_ordinary_helper_value_type_v1(&types, ARRAY, &mut |_| {
            Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
        })
        .unwrap_err();
        assert!(matches!(
            error,
            ProductionSemanticKirErrorV1::CorrespondenceMismatch
        ));
        let mut charges = Vec::new();
        check_ordinary_helper_value_type_v1(&types, ARRAY, &mut |amount| {
            charges.push(amount);
            Ok(())
        })
        .unwrap();
        assert_eq!(charges, [1, 1, 1]);
    }

    #[test]
    fn legacy_final_publication_denial_keeps_the_original_prefix_and_input_reusable() {
        let types = types();
        let function = function(ARRAY, integer_cast(8, false));
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(34);
        let result = plan_ordinary_helper_result_v1(&types, &function, &mut work, 7);
        assert!(matches!(
            result,
            Err(ProductionSemanticKirErrorV1::ResourceLimit {
                resource: ProductionSemanticKirResourceV1::AnalysisWork,
                actual: 35,
                limit: 34,
            })
        ));
        assert_eq!((work.work(), work.failed_work()), (34, Some(35)));
        assert!(charge_ordinary_helper_result_work_v1(&mut work, 0).is_err());
        assert_eq!((work.work(), work.failed_work()), (34, Some(35)));
        // No retained plan escapes the failed final publication. A new phase
        // can reuse the borrowed source; allocator destruction timing is not
        // represented by this logical-row/work counter.
        let mut next = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(34);
        check_ordinary_helper_result_transport_v1(
            &types,
            &function,
            &mut |amount| charge_ordinary_helper_result_work_v1(&mut next, amount),
            7,
        )
        .unwrap();
        assert_eq!((next.work(), next.failed_work()), (34, None));
    }
}
