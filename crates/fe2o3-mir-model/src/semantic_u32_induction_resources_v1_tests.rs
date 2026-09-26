// Included below the existing admitted synthetic induction fixture.
mod strict_resource_tests {
    use super::super::strict_resources::frame_storage_v1;
    use super::*;
    use std::mem::size_of;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum Denied {
        Work,
        Storage,
        Injected,
    }
    #[derive(Default)]
    struct Meter {
        work: usize,
        storage: usize,
        work_limit: Option<usize>,
        storage_limit: Option<usize>,
        work_calls: usize,
        storage_calls: usize,
        first_storage: Option<usize>,
        deny_work_call: Option<usize>,
        deny_storage_call: Option<usize>,
        panic_storage_call: Option<usize>,
        failed: Option<Denied>,
        calls_after_denial: usize,
    }
    impl SemanticU32InductionBoundSnapshotMeterV1 for Meter {
        type Error = Denied;
        fn charge_work(&mut self, amount: usize) -> Result<(), Denied> {
            if let Some(error) = self.failed {
                self.calls_after_denial += 1;
                return Err(error);
            }
            self.work_calls += 1;
            let next = self.work.checked_add(amount).unwrap();
            let denied = if self.deny_work_call == Some(self.work_calls) {
                Some(Denied::Injected)
            } else if self.work_limit.is_some_and(|limit| next > limit) {
                Some(Denied::Work)
            } else {
                None
            };
            if let Some(error) = denied {
                self.failed = Some(error);
                return Err(error);
            }
            self.work = next;
            Ok(())
        }
        fn reserve_storage(&mut self, amount: usize) -> Result<(), Denied> {
            if let Some(error) = self.failed {
                self.calls_after_denial += 1;
                return Err(error);
            }
            self.storage_calls += 1;
            self.first_storage.get_or_insert(amount);
            if self.panic_storage_call == Some(self.storage_calls) {
                panic!("strict induction meter panic");
            }
            let next = self.storage.checked_add(amount).unwrap();
            let denied = if self.deny_storage_call == Some(self.storage_calls) {
                Some(Denied::Injected)
            } else if self.storage_limit.is_some_and(|limit| next > limit) {
                Some(Denied::Storage)
            } else {
                None
            };
            if let Some(error) = denied {
                self.failed = Some(error);
                return Err(error);
            }
            self.storage = next;
            Ok(())
        }
    }
    fn run<M: SemanticU32InductionBoundSnapshotMeterV1>(
        source: &AdmittedInertSemanticMirV1,
        meter: &mut M,
    ) -> Result<SemanticU32InductionNoOverflowReportV1, SemanticU32InductionMeteredErrorV1<M::Error>>
    {
        analyze_semantic_u32_induction_no_overflow_with_meter_v1(
            source,
            SemanticFunctionIdV1::from_index(0),
            SemanticU32InductionAnalysisLimitsV1::default(),
            meter,
        )
    }

    #[test]
    fn strict_report_and_local_work_match_legacy_supported_and_unproved_shapes() {
        for shape in [
            Shape::default(),
            Shape {
                guard_snapshot: true,
                ..Shape::default()
            },
            Shape {
                step: 2,
                ..Shape::default()
            },
            Shape {
                expected_overflow: true,
                ..Shape::default()
            },
            Shape {
                // Keep ABI/local roles valid while denying the exact +1 shape.
                step: 0,
                ..Shape::default()
            },
            Shape {
                extra_induction_definition: true,
                ..Shape::default()
            },
            Shape {
                alias_induction: true,
                ..Shape::default()
            },
            Shape {
                guard_snapshot: true,
                guard_snapshot_extra_use: true,
                ..Shape::default()
            },
            Shape {
                mutate_assert_operands: true,
                ..Shape::default()
            },
            Shape {
                identity_seed: 17,
                ..Shape::default()
            },
        ] {
            let source = admitted(shape);
            let expected = report(&source);
            if shape.step == 0 {
                assert!(expected.certificates().is_empty());
            }
            let mut meter = Meter::default();
            let actual = run(&source, &mut meter).unwrap();
            assert_eq!(actual, expected);
            assert_eq!(actual.work_units(), expected.work_units());
            assert!(!actual.uses_reachable_scope_v2());
            assert!(!actual.grants_authority() && !actual.authorizes_compiler_transform());
            assert!(meter.work > actual.work_units());
            assert!(meter.storage > frame_storage_v1::<Meter>().unwrap());
        }
    }
    #[test]
    fn strict_legacy_local_caps_and_semantic_refusal_order_are_unchanged() {
        let source = admitted(Shape::default());
        let work = report(&source).work_units();
        let function = SemanticFunctionIdV1::from_index(0);
        for limits in [
            SemanticU32InductionAnalysisLimitsV1::new(0, 10),
            SemanticU32InductionAnalysisLimitsV1::new(work - 1, 10),
            SemanticU32InductionAnalysisLimitsV1::new(work, 0),
            SemanticU32InductionAnalysisLimitsV1::new(MAX_SEMANTIC_U32_INDUCTION_WORK_V1 + 1, 10),
            SemanticU32InductionAnalysisLimitsV1::new(
                work,
                MAX_SEMANTIC_U32_INDUCTION_CERTIFICATES_V1 + 1,
            ),
        ] {
            let expected = analyze_semantic_u32_induction_no_overflow_with_limits_v1(
                &source, function, limits,
            )
            .unwrap_err();
            let mut meter = Meter::default();
            assert_eq!(
                analyze_semantic_u32_induction_no_overflow_with_meter_v1(
                    &source, function, limits, &mut meter
                ),
                Err(SemanticU32InductionMeteredErrorV1::Analysis(expected))
            );
        }
        let limits = SemanticU32InductionAnalysisLimitsV1::new(usize::MAX, usize::MAX);
        let absent = SemanticFunctionIdV1::from_index(99);
        let expected =
            analyze_semantic_u32_induction_no_overflow_with_limits_v1(&source, absent, limits)
                .unwrap_err();
        assert!(matches!(
            expected,
            SemanticU32InductionAnalysisErrorV1::InvalidModel(_)
        ));
        assert_eq!(
            analyze_semantic_u32_induction_no_overflow_with_meter_v1(
                &source,
                absent,
                limits,
                &mut Meter::default()
            ),
            Err(SemanticU32InductionMeteredErrorV1::Analysis(expected))
        );
        let exact = SemanticU32InductionAnalysisLimitsV1::new(work, 1);
        assert_eq!(
            analyze_semantic_u32_induction_no_overflow_with_meter_v1(
                &source,
                function,
                exact,
                &mut Meter::default()
            )
            .unwrap(),
            report(&source)
        );
    }
    #[test]
    fn strict_complete_cfg_still_refuses_unreachable_source_not_a_v2_conversion() {
        let source = admitted(Shape {
            dead_predecessor: true,
            ..Shape::default()
        });
        let expected = analyze_semantic_u32_induction_no_overflow_v1(
            &source,
            SemanticFunctionIdV1::from_index(0),
        )
        .unwrap_err();
        assert_eq!(
            run(&source, &mut Meter::default()),
            Err(SemanticU32InductionMeteredErrorV1::Analysis(expected))
        );
    }
    #[test]
    fn strict_exact_original_work_storage_and_each_one_short() {
        let source = admitted(Shape::default());
        let mut measured = Meter::default();
        let expected = run(&source, &mut measured).unwrap();
        for short in [None, Some(Denied::Work), Some(Denied::Storage)] {
            let mut meter = Meter {
                work: 7,
                storage: 11,
                work_limit: Some(7 + measured.work - usize::from(short == Some(Denied::Work))),
                storage_limit: Some(
                    11 + measured.storage - usize::from(short == Some(Denied::Storage)),
                ),
                ..Default::default()
            };
            let result = run(&source, &mut meter);
            if let Some(error) = short {
                assert_eq!(
                    result,
                    Err(SemanticU32InductionMeteredErrorV1::Meter(error))
                );
                assert_eq!(meter.failed, Some(error));
            } else {
                assert_eq!(result.unwrap(), expected);
                assert_eq!(
                    (meter.work, meter.storage),
                    (7 + measured.work, 11 + measured.storage)
                );
            }
            assert_eq!(meter.calls_after_denial, 0);
        }
    }
    #[test]
    fn strict_every_resource_call_denial_is_terminal_and_retains_accepted_prefix() {
        let source = admitted(Shape::default());
        let mut measured = Meter::default();
        drop(run(&source, &mut measured).unwrap());
        for storage in [false, true] {
            let calls = if storage {
                measured.storage_calls
            } else {
                measured.work_calls
            };
            for call in 1..=calls {
                let mut meter = Meter {
                    work: 7,
                    storage: 11,
                    deny_work_call: (!storage).then_some(call),
                    deny_storage_call: storage.then_some(call),
                    ..Default::default()
                };
                assert_eq!(
                    run(&source, &mut meter),
                    Err(SemanticU32InductionMeteredErrorV1::Meter(Denied::Injected))
                );
                assert_eq!(meter.calls_after_denial, 0);
                assert_eq!(
                    if storage {
                        meter.storage_calls
                    } else {
                        meter.work_calls
                    },
                    call
                );
                assert!(meter.work >= 7 && meter.storage >= 11);
                if storage && call == 1 {
                    assert_eq!((meter.work, meter.storage), (7, 11));
                }
            }
        }
    }
    #[test]
    fn strict_preexisting_sticky_denial_and_unwind_never_release_original_credit() {
        let source = admitted(Shape::default());
        for failed in [Denied::Work, Denied::Storage] {
            let mut meter = Meter {
                work: 7,
                storage: 11,
                failed: Some(failed),
                ..Default::default()
            };
            assert_eq!(
                run(&source, &mut meter),
                Err(SemanticU32InductionMeteredErrorV1::Meter(failed))
            );
            assert_eq!((meter.work, meter.storage), (7, 11));
            assert_eq!(
                (
                    meter.work_calls,
                    meter.storage_calls,
                    meter.calls_after_denial
                ),
                (0, 0, 1)
            );
        }
        let mut meter = Meter {
            work: 7,
            storage: 11,
            panic_storage_call: Some(2),
            ..Default::default()
        };
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(&source, &mut meter)))
                .is_err()
        );
        assert_eq!(meter.storage, 11 + frame_storage_v1::<Meter>().unwrap());
        assert_eq!(meter.storage_calls, 2);
    }

    #[derive(Default)]
    struct LargeErrorMeter {
        inner: Meter,
    }
    impl SemanticU32InductionBoundSnapshotMeterV1 for LargeErrorMeter {
        type Error = [u8; 8192];
        fn charge_work(&mut self, n: usize) -> Result<(), Self::Error> {
            self.inner.charge_work(n).map_err(|_| [1; 8192])
        }
        fn reserve_storage(&mut self, n: usize) -> Result<(), Self::Error> {
            self.inner.reserve_storage(n).map_err(|_| [2; 8192])
        }
    }
    #[test]
    fn strict_generic_error_report_frames_are_prepaid_before_first_source_allocation() {
        let source = admitted(Shape::default());
        let frame = frame_storage_v1::<LargeErrorMeter>().unwrap();
        assert!(frame > 4 * 8192);
        for short in [false, true] {
            let mut meter = LargeErrorMeter {
                inner: Meter {
                    work: 7,
                    storage: 11,
                    storage_limit: Some(11 + frame - usize::from(short)),
                    ..Default::default()
                },
            };
            assert!(
                matches!(run(&source,&mut meter),Err(SemanticU32InductionMeteredErrorV1::Meter(error)) if error==[2;8192])
            );
            assert_eq!(meter.inner.first_storage, Some(frame));
            if short {
                assert_eq!(
                    (
                        meter.inner.work,
                        meter.inner.storage,
                        meter.inner.storage_calls
                    ),
                    (7, 11, 1)
                );
            } else {
                assert_eq!(meter.inner.storage, 11 + frame);
                assert_eq!(meter.inner.storage_calls, 2);
            }
        }
    }
    #[test]
    fn strict_large_error_complete_route_exact_and_one_short() {
        let source = admitted(Shape::default());
        let mut measured = LargeErrorMeter::default();
        let expected = run(&source, &mut measured).unwrap();
        for short in [false, true] {
            let mut meter = LargeErrorMeter {
                inner: Meter {
                    work: 7,
                    storage: 11,
                    work_limit: Some(7 + measured.inner.work),
                    storage_limit: Some(11 + measured.inner.storage - usize::from(short)),
                    ..Default::default()
                },
            };
            let result = run(&source, &mut meter);
            if short {
                assert!(
                    matches!(result,Err(SemanticU32InductionMeteredErrorV1::Meter(error)) if error==[2;8192])
                );
            } else {
                assert_eq!(result.unwrap(), expected);
                assert_eq!(
                    (meter.inner.work, meter.inner.storage),
                    (7 + measured.inner.work, 11 + measured.inner.storage)
                );
            }
        }
    }

    #[derive(Default)]
    struct StrictProbe {
        meter: Meter,
        arithmetic: Option<bool>,
    }
    impl bound_snapshot::InternalMeter for StrictProbe {
        fn strict_resources(&self) -> bool {
            true
        }
        fn strict_failure(&mut self, arithmetic: bool) {
            self.arithmetic = Some(arithmetic);
        }
        fn charge_work(&mut self, n: usize) -> Result<(), SemanticU32InductionAnalysisErrorV1> {
            self.meter
                .charge_work(n)
                .map_err(|_| SemanticU32InductionAnalysisErrorV1::Storage)
        }
        fn reserve_storage(&mut self, n: usize) -> Result<(), SemanticU32InductionAnalysisErrorV1> {
            self.meter
                .reserve_storage(n)
                .map_err(|_| SemanticU32InductionAnalysisErrorV1::Storage)
        }
    }
    #[test]
    fn strict_vector_denial_and_arithmetic_happen_before_allocation() {
        let mut probe = StrictProbe {
            meter: Meter {
                storage_limit: Some(0),
                ..Default::default()
            },
            ..Default::default()
        };
        let mut values = Vec::<u64>::new();
        {
            let mut budget = WorkBudgetV1 {
                used: 0,
                limit: 100,
                meter: Some(&mut probe),
            };
            assert_eq!(
                budget.reserve_vec(&mut values, 3, false),
                Err(SemanticU32InductionAnalysisErrorV1::Storage)
            );
            assert_eq!(budget.used, 0);
        }
        assert_eq!((values.len(), values.capacity()), (0, 0));
        assert_eq!(probe.meter.storage, 0);
        let mut probe = StrictProbe::default();
        let mut values = vec![7u64];
        let capacity = values.capacity();
        {
            let mut budget = WorkBudgetV1 {
                used: 0,
                limit: 100,
                meter: Some(&mut probe),
            };
            assert!(budget.reserve_vec(&mut values, usize::MAX, true).is_err());
        }
        assert_eq!(probe.arithmetic, Some(true));
        assert_eq!(values, vec![7]);
        assert_eq!(values.capacity(), capacity);
        assert_eq!((probe.meter.work_calls, probe.meter.storage_calls), (0, 0));
        let mut probe = StrictProbe::default();
        {
            let mut budget = WorkBudgetV1 {
                used: 0,
                limit: 100,
                meter: Some(&mut probe),
            };
            assert!(
                budget
                    .reserve_vec(&mut Vec::<u64>::new(), usize::MAX / 8 + 1, true)
                    .is_err()
            );
        }
        assert_eq!(probe.arithmetic, Some(true));
        assert_eq!((probe.meter.work_calls, probe.meter.storage_calls), (0, 0));
    }
    #[test]
    fn strict_box_shrink_prepays_copy_and_new_storage_without_refunding_old() {
        let mut values = Vec::with_capacity(3);
        values.push(9u64);
        for deny in [false, true] {
            let mut probe = StrictProbe {
                meter: Meter {
                    storage: 31,
                    storage_limit: deny.then_some(31),
                    ..Default::default()
                },
                ..Default::default()
            };
            let values = values.clone(); // clone may trim capacity: construct explicit slack below.
            let mut slack = Vec::with_capacity(3);
            slack.extend_from_slice(&values);
            let mut budget = WorkBudgetV1 {
                used: 0,
                limit: 100,
                meter: Some(&mut probe),
            };
            let result = budget.boxed(slack);
            if deny {
                assert!(result.is_err());
            } else {
                assert_eq!(&*result.unwrap(), &[9u64]);
            }
            assert_eq!(probe.meter.work, 8);
            assert_eq!(probe.meter.storage, if deny { 31 } else { 39 });
        }
    }
    #[test]
    fn original_snapshot_meter_keeps_frame_formula_report_and_work_counter_relation() {
        let source = admitted(Shape::default());
        let expected = analyze_semantic_u32_induction_bound_snapshots_v1(
            &source,
            SemanticFunctionIdV1::from_index(0),
        )
        .unwrap();
        let mut meter = Meter::default();
        let actual = analyze_semantic_u32_induction_bound_snapshots_with_meter_v1(
            &source,
            SemanticFunctionIdV1::from_index(0),
            SemanticU32InductionAnalysisLimitsV1::default(),
            &mut meter,
        )
        .unwrap();
        assert_eq!(actual, expected);
        let frame = size_of::<SemanticU32InductionBoundSnapshotReportV1>()
            + size_of::<SemanticCfgV1>()
            + size_of::<SemanticInventoryV1>()
            + size_of::<bound_snapshot::LifetimeIndex>()
            + size_of::<WorkBudgetV1<'_>>();
        assert_eq!(meter.first_storage, Some(frame));
        assert_eq!(meter.work, actual.work_units());
        assert_eq!(
            actual.retained_storage(),
            size_of::<SemanticU32InductionBoundSnapshotReportV1>()
                + actual.certificates().len()
                    * size_of::<SemanticU32InductionBoundSnapshotCertificateV1>()
        );
        assert_eq!(
            size_of::<WorkBudgetV1<'_>>(),
            size_of::<(usize, usize, Option<&mut dyn bound_snapshot::InternalMeter>)>()
        );
    }
    #[test]
    fn strict_local_counter_overflow_keeps_old_error_before_external_work() {
        let mut old = WorkBudgetV1 {
            used: usize::MAX,
            limit: usize::MAX,
            meter: None,
        };
        let expected = old.charge(1).unwrap_err();
        let mut probe = StrictProbe::default();
        {
            let mut strict = WorkBudgetV1 {
                used: usize::MAX,
                limit: usize::MAX,
                meter: Some(&mut probe),
            };
            assert_eq!(strict.charge(1), Err(expected));
            assert_eq!(strict.used, usize::MAX);
        }
        assert_eq!(probe.meter.work_calls, 0);
        assert_eq!(probe.arithmetic, None);
    }
    #[test]
    fn strict_allocator_refusal_is_recorded_after_full_prepaid_capacity() {
        let mut probe = StrictProbe::default();
        let mut values = Vec::<u8>::new();
        {
            let mut strict = WorkBudgetV1 {
                used: 0,
                limit: usize::MAX,
                meter: Some(&mut probe),
            };
            // Vec rejects capacities above isize::MAX without an allocation.
            assert!(strict.reserve_vec(&mut values, usize::MAX, true).is_err());
        }
        assert_eq!((values.len(), values.capacity()), (0, 0));
        assert_eq!(probe.arithmetic, Some(false));
        assert_eq!(probe.meter.storage, usize::MAX);
    }
}
