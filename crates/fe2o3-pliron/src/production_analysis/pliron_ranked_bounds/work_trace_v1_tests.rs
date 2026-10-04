#[cfg(test)]
mod work_trace_tests_v1 {
    use super::*;

    fn site(line: u32) -> RankedBoundsWorkSiteV1 {
        RankedBoundsWorkSiteV1 {
            file: "diagnostic-fixture",
            line,
            column: 1,
        }
    }

    #[test]
    fn ranked_bounds_work_trace_exact_one_under_and_baseline_are_observers_only() {
        for available in [6, 5] {
            let mut budget = RankedBoundsBudget {
                work_units: MAX_RANKED_BOUNDS_WORK_UNITS - available,
                ..RankedBoundsBudget::default()
            };
            let baseline = budget.work_units;
            let call_line = line!() + 1;
            let first = budget.work(3);
            assert_eq!(first, Ok(()));
            let row = budget.work_trace.rows[0].unwrap();
            assert_eq!(row.site.file, file!());
            assert_eq!(row.site.line, call_line);
            assert_eq!(
                row.counts,
                RankedBoundsWorkCountsV1 {
                    calls: 1,
                    requested: 3,
                    admitted: 3
                }
            );
            let result = budget.work(3);
            if available == 6 {
                assert_eq!(result, Ok(()));
                assert_eq!(budget.work_units, MAX_RANKED_BOUNDS_WORK_UNITS);
                assert!(!budget.work_trace.reported);
            } else {
                assert!(
                    matches!(result, Err(RankedBoundsFindingV1::ResourceLimitExceeded {
                    resource: "analysis work unit", actual, limit
                }) if actual == MAX_RANKED_BOUNDS_WORK_UNITS + 1 && limit == MAX_RANKED_BOUNDS_WORK_UNITS)
                );
                assert_eq!(budget.work_units, MAX_RANKED_BOUNDS_WORK_UNITS - 2);
                assert!(budget.work_trace.reported);
            }
            assert_eq!(budget.work_trace.baseline, Some(baseline));
            assert_eq!(
                budget.work_trace.total,
                RankedBoundsWorkCountsV1 {
                    calls: 2,
                    requested: 6,
                    admitted: if available == 6 { 6 } else { 3 }
                }
            );
            assert!(budget.work_trace.reconciles(budget.work_units));
            assert_eq!(budget.storage_items, 0);
        }
        let mut budget = RankedBoundsBudget {
            work_units: 17,
            ..RankedBoundsBudget::default()
        };
        budget.work(5).unwrap();
        budget.work_units = 9;
        budget.work(2).unwrap();
        assert_eq!(budget.work_units, 11);
        assert_eq!(budget.work_trace.baseline, Some(17));
        assert!(budget.work_trace.counter_changed);
        assert!(!budget.work_trace.reconciles(11));
        let mut storage_only = RankedBoundsBudget::default();
        storage_only
            .storage(MAX_RANKED_BOUNDS_STORAGE_ITEMS)
            .unwrap();
        assert!(
            matches!(storage_only.storage(1), Err(RankedBoundsFindingV1::ResourceLimitExceeded {
            resource: "analysis storage item", actual, limit
        }) if actual == MAX_RANKED_BOUNDS_STORAGE_ITEMS + 1 && limit == MAX_RANKED_BOUNDS_STORAGE_ITEMS)
        );
        assert_eq!(storage_only.work_units, 0);
        assert_eq!(storage_only.work_trace.baseline, None);
        assert_eq!(
            storage_only.work_trace.total,
            RankedBoundsWorkCountsV1::default()
        );
        assert!(!storage_only.work_trace.reported);
    }

    #[test]
    fn ranked_bounds_work_trace_overflow_is_exclusive_bounded_and_explicit() {
        let mut trace = RankedBoundsWorkTraceV1::default();
        for line in 0..258 {
            trace.record(site(line), usize::try_from(line).unwrap(), 1, true);
        }
        trace.record(site(0), 258, 2, true);
        trace.record(site(999), 260, 7, false);
        assert_eq!(trace.len, 256);
        assert_eq!(
            trace.rows[0].unwrap().counts,
            RankedBoundsWorkCountsV1 {
                calls: 2,
                requested: 3,
                admitted: 3
            }
        );
        assert_eq!(
            trace.overflow,
            RankedBoundsWorkCountsV1 {
                calls: 3,
                requested: 9,
                admitted: 2
            }
        );
        assert_eq!(
            trace.total,
            RankedBoundsWorkCountsV1 {
                calls: 260,
                requested: 267,
                admitted: 260
            }
        );
        assert!(trace.reconciles(260));
        assert!(!trace.saturated);
        let mut output = Vec::new();
        trace.report_denial_to(&mut output, 260, 7, 267, 266);
        let text = String::from_utf8(output.clone()).unwrap();
        assert!(text.contains(
            "rows=256 truncated=true saturated=false counter_changed=false reconciled=true"
        ));
        assert!(text.ends_with("FE2O3_BOUNDS_WORK_TRACE_END rows=256\n"));
        assert_eq!(text.lines().count(), 259);
        let length = output.len();
        trace.report_denial_to(&mut output, 260, 7, 267, 266);
        assert_eq!(output.len(), length);
    }

    #[test]
    fn ranked_bounds_work_trace_saturation_and_output_failure_do_not_escape() {
        let mut trace = RankedBoundsWorkTraceV1::default();
        trace.record(site(0), 0, usize::MAX, false);
        trace.record(site(0), 0, 1, false);
        assert!(trace.saturated);
        assert!(!trace.reconciles(0));
        assert_eq!(trace.total.requested, usize::MAX);
        struct FailedOutput {
            writes: usize,
        }
        impl std::io::Write for FailedOutput {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                self.writes += 1;
                Err(std::io::Error::other("diagnostic output unavailable"))
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let mut budget = RankedBoundsBudget {
            work_units: MAX_RANKED_BOUNDS_WORK_UNITS - 1,
            ..RankedBoundsBudget::default()
        };
        let mut failed = FailedOutput { writes: 0 };
        budget.work_trace.report_denial_to(
            &mut failed,
            budget.work_units,
            2,
            MAX_RANKED_BOUNDS_WORK_UNITS + 1,
            MAX_RANKED_BOUNDS_WORK_UNITS,
        );
        assert_eq!(failed.writes, 1);
        assert!(budget.work_trace.reported);
        assert!(
            matches!(budget.work(2), Err(RankedBoundsFindingV1::ResourceLimitExceeded {
            resource: "analysis work unit", actual, limit
        }) if actual == MAX_RANKED_BOUNDS_WORK_UNITS + 1 && limit == MAX_RANKED_BOUNDS_WORK_UNITS)
        );
        assert_eq!(budget.work_units, MAX_RANKED_BOUNDS_WORK_UNITS - 1);
        assert_eq!(budget.storage_items, 0);
        assert_eq!(budget.work_trace.total.admitted, 0);
    }
}
