// These are inert ownership/accounting components, not authenticated source
// fixtures. No test constructs protected bindings or a functional Some result.
mod checked_output_export_buffer_v1_tests {
    use super::*;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    const FLOOR: usize = 97;

    struct DropMarker {
        name: &'static str,
        log: Rc<RefCell<Vec<&'static str>>>,
    }

    impl Drop for DropMarker {
        fn drop(&mut self) {
            self.log.borrow_mut().push(self.name);
        }
    }

    struct InertStage {
        source: DropMarker,
        bindings: DropMarker,
    }

    fn stage(log: &Rc<RefCell<Vec<&'static str>>>) -> InertStage {
        InertStage {
            source: DropMarker {
                name: "source",
                log: Rc::clone(log),
            },
            bindings: DropMarker {
                name: "bindings",
                log: Rc::clone(log),
            },
        }
    }

    fn split(stage: InertStage) -> DropMarker {
        let InertStage { source, bindings } = stage;
        drop(source);
        bindings
    }

    fn semantic_error() -> ProductionPipelineError {
        export_error_v1(CheckedOutputExportErrorV1::Capacity)
    }

    fn is_accounting(result: &Result<(), ProductionPipelineError>) -> bool {
        matches!(
            result,
            Err(ProductionPipelineError::CheckedOutputExport(
                CheckedOutputExportErrorV1::Resource(ExportResourceV1::Accounting)
            ))
        )
    }

    #[test]
    fn b1_constructor_has_exact_work_and_requested_storage_boundaries() {
        let header = export_header_v1::<()>();
        for limit in [10, 11] {
            let mut work = Work::new(limit);
            let mut budget = ExportBudgetV1::new(&mut work, usize::MAX);
            budget.charge_work(7).unwrap();
            budget.reserve_storage(FLOOR).unwrap();
            let result = CheckedOutputExportWriterV1::try_new::<()>(8, &mut budget);
            if limit == 10 {
                assert!(
                    matches!(result, Err(CheckedOutputExportErrorV1::Resource(ExportResourceV1::Work(error))) if error.actual() == 11)
                );
                assert_eq!(budget.work(), 7);
            } else {
                let writer = result.unwrap();
                assert_eq!(budget.work(), 11);
                assert_eq!(budget.storage(), FLOOR + header + writer.bytes.capacity());
                drop(writer);
                restore_export_floor_v1(&mut budget, FLOOR).unwrap();
            }
            assert_eq!(budget.storage(), FLOOR);
            assert_eq!(work.failed_work(), (limit == 10).then_some(11));
        }
        let mut work = Work::new(100);
        let mut budget = ExportBudgetV1::new(&mut work, FLOOR + header + 7);
        budget.reserve_storage(FLOOR).unwrap();
        let allocated = Cell::new(false);
        assert!(matches!(
            CheckedOutputExportWriterV1::try_new_with_v1::<()>(8, &mut budget, |_, _| {
                allocated.set(true);
                Ok(())
            }),
            Err(CheckedOutputExportErrorV1::Resource(
                ExportResourceV1::Storage(_)
            ))
        ));
        assert!(!allocated.get());
        assert_eq!(budget.failed_storage(), Some(FLOOR + header + 8));
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.work(), 4);
    }

    #[test]
    fn b1_component_cap_and_checked_extent_refuse_without_allocation() {
        assert_eq!(EXPORT_BUFFER_COMPONENT_CAP_V1, 4 * 1024 * 1024);
        for cap in [
            EXPORT_BUFFER_COMPONENT_CAP_V1,
            EXPORT_BUFFER_COMPONENT_CAP_V1 + 1,
        ] {
            let mut work = Work::new(100);
            let mut budget = ExportBudgetV1::new(&mut work, FLOOR);
            budget.reserve_storage(FLOOR).unwrap();
            let result =
                CheckedOutputExportWriterV1::try_new_with_v1::<()>(cap, &mut budget, |_, _| {
                    panic!("allocation must not run")
                });
            if cap == EXPORT_BUFFER_COMPONENT_CAP_V1 {
                assert!(matches!(
                    result,
                    Err(CheckedOutputExportErrorV1::Resource(
                        ExportResourceV1::Storage(_)
                    ))
                ));
            } else {
                assert!(matches!(result, Err(CheckedOutputExportErrorV1::Capacity)));
                assert_eq!(budget.failed_storage(), None);
            }
            assert_eq!(budget.storage(), FLOOR);
            assert_eq!(budget.work(), 4);
        }
        assert_eq!(export_end_v1(2, 3, 5).unwrap(), 5);
        assert!(matches!(
            export_end_v1(2, 4, 5),
            Err(CheckedOutputExportErrorV1::Capacity)
        ));
        assert!(matches!(
            export_end_v1(usize::MAX, 1, usize::MAX),
            Err(CheckedOutputExportErrorV1::Resource(
                ExportResourceV1::Arithmetic
            ))
        ));
    }

    #[test]
    fn b1_allocator_excess_is_paid_but_never_expands_logical_extent() {
        let mut work = Work::new(100);
        let mut budget = ExportBudgetV1::new(&mut work, usize::MAX);
        budget.reserve_storage(FLOOR).unwrap();
        let mut writer =
            CheckedOutputExportWriterV1::try_new_with_v1::<()>(3, &mut budget, |bytes, _| {
                bytes
                    .try_reserve_exact(19)
                    .map_err(|_| ExportResourceV1::Allocation)
            })
            .unwrap();
        let capacity = writer.bytes.capacity();
        let pointer = writer.bytes.as_ptr();
        assert!(capacity >= 19);
        assert_eq!(writer.retained, export_header_v1::<()>() + capacity);
        assert_eq!(budget.storage(), FLOOR + writer.retained);
        assert_eq!(writer.append_v1(&[1, 2], &mut budget).unwrap(), 0..2);
        assert_eq!(writer.append_v1(&[], &mut budget).unwrap(), 2..2);
        assert_eq!(writer.append_v1(&[3], &mut budget).unwrap(), 2..3);
        assert_eq!(writer.bytes.as_ptr(), pointer);
        assert_eq!(writer.bytes.capacity(), capacity);
        assert!(matches!(
            writer.append_v1(&[4], &mut budget),
            Err(CheckedOutputExportErrorV1::Capacity)
        ));
        assert_eq!(writer.bytes, [1, 2, 3]);
        assert!(matches!(
            writer.transfer_v1(&mut budget),
            Err(CheckedOutputExportErrorV1::Poisoned)
        ));
        restore_export_floor_v1(&mut budget, FLOOR).unwrap();
        assert_eq!(budget.work(), 4 + 5 + 3 + 4 + 4 + 3);
        assert_eq!(budget.storage(), FLOOR);
    }

    #[test]
    fn b1_allocator_excess_denial_error_and_unwind_restore_prefix() {
        for mode in 0..3 {
            let mut work = Work::new(100);
            let header = export_header_v1::<()>();
            let mut budget = ExportBudgetV1::new(&mut work, FLOOR + header + 3);
            budget.reserve_storage(FLOOR).unwrap();
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                CheckedOutputExportWriterV1::try_new_with_v1::<()>(3, &mut budget, |bytes, _| {
                    if mode == 1 {
                        return Err(ExportResourceV1::Allocation);
                    }
                    if mode == 2 {
                        std::panic::panic_any(781_u32);
                    }
                    bytes
                        .try_reserve_exact(19)
                        .map_err(|_| ExportResourceV1::Allocation)
                })
            }));
            match (mode, outcome) {
                (
                    0,
                    Ok(Err(CheckedOutputExportErrorV1::Resource(ExportResourceV1::Storage(error)))),
                ) => {
                    assert!(error.actual() >= FLOOR + header + 19);
                    assert_eq!(budget.failed_storage(), Some(error.actual()));
                }
                (
                    1,
                    Ok(Err(CheckedOutputExportErrorV1::Resource(ExportResourceV1::Allocation))),
                ) => {}
                (2, Err(payload)) => assert_eq!(payload.downcast_ref::<u32>(), Some(&781)),
                _ => panic!("unexpected allocation outcome"),
            }
            assert_eq!(budget.storage(), FLOOR);
            assert_eq!(budget.work(), 4);
        }
    }

    #[test]
    fn b1_work_denial_poison_survives_smaller_charges_and_finish() {
        // Prior 7 + constructor 4 = 11; rejected append attempts 34. Smaller
        // empty append and transfer each charge 3, but neither can finish.
        let mut work = Work::new(17);
        let mut budget = ExportBudgetV1::new(&mut work, usize::MAX);
        budget.charge_work(7).unwrap();
        budget.reserve_storage(FLOOR).unwrap();
        let mut writer = CheckedOutputExportWriterV1::try_new::<()>(20, &mut budget).unwrap();
        assert!(
            matches!(writer.append_v1(&[0; 20], &mut budget), Err(CheckedOutputExportErrorV1::Resource(ExportResourceV1::Work(error))) if error.actual() == 34)
        );
        assert!(matches!(
            writer.append_v1(&[], &mut budget),
            Err(CheckedOutputExportErrorV1::Poisoned)
        ));
        assert!(matches!(
            writer.transfer_v1(&mut budget),
            Err(CheckedOutputExportErrorV1::Poisoned)
        ));
        restore_export_floor_v1(&mut budget, FLOOR).unwrap();
        assert_eq!(budget.work(), 17);
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(work.failed_work(), Some(34));
    }

    #[test]
    fn b1_foreign_ledger_or_released_floor_poison_the_writer() {
        for foreign in [false, true] {
            let mut work = Work::new(100);
            let mut budget = ExportBudgetV1::new(&mut work, usize::MAX);
            budget.reserve_storage(FLOOR).unwrap();
            let mut writer = CheckedOutputExportWriterV1::try_new::<()>(0, &mut budget).unwrap();
            if foreign {
                let mut other_work = Work::new(100);
                let mut other = ExportBudgetV1::new(&mut other_work, usize::MAX);
                other.reserve_storage(budget.storage()).unwrap();
                assert!(matches!(
                    writer.append_v1(&[], &mut other),
                    Err(CheckedOutputExportErrorV1::Resource(
                        ExportResourceV1::Accounting
                    ))
                ));
            } else {
                budget.release_storage(1).unwrap();
                assert!(matches!(
                    writer.append_v1(&[], &mut budget),
                    Err(CheckedOutputExportErrorV1::Resource(
                        ExportResourceV1::Accounting
                    ))
                ));
                budget.reserve_storage(1).unwrap();
            }
            assert!(matches!(
                writer.transfer_v1(&mut budget),
                Err(CheckedOutputExportErrorV1::Poisoned)
            ));
            restore_export_floor_v1(&mut budget, FLOOR).unwrap();
        }
    }

    #[test]
    fn b1_owned_stage_drops_source_then_transfers_original_binding_and_actual_capacity() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let mut work = Work::new(100);
        let mut budget = ExportBudgetV1::new(&mut work, usize::MAX);
        budget.charge_work(7).unwrap();
        budget.reserve_storage(FLOOR).unwrap();
        let writer = CheckedOutputExportWriterV1::try_new::<DropMarker>(9, &mut budget).unwrap();
        let retained = writer.retained;
        budget.reserve_storage(41).unwrap();
        let owner = stage(&log);
        let identity = Rc::as_ptr(&owner.bindings.log);
        let prepared = with_owned_export_stage_v1(
            owner,
            writer,
            41,
            &mut budget,
            |_, writer, budget| {
                assert!(log.borrow().is_empty());
                assert_eq!(budget.storage(), FLOOR + retained + 41);
                writer.append_v1(&[7, 8], budget).map_err(export_error_v1)?;
                // Represents scratch owned and dropped by an inner callback.
                budget
                    .reserve_storage(13)
                    .map_err(|e| export_error_v1(e.into()))?;
                Ok(())
            },
            split,
        )
        .unwrap();
        assert_eq!(&*log.borrow(), &["source"]);
        assert_eq!(Rc::as_ptr(&prepared.bindings.log), identity);
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.work(), 7 + 4 + 5 + 5 + 3);
        assert_eq!(prepared.packet.retained, retained);
        prepared
            .with_accepted_v1(&mut budget, |bytes, bindings, budget| {
                assert_eq!(bytes, [7, 8]);
                assert_eq!(Rc::as_ptr(&bindings.log), identity);
                assert_eq!(budget.storage(), FLOOR + retained);
                assert_eq!(&*log.borrow(), &["source"]);
                Ok(())
            })
            .unwrap();
        assert_eq!(&*log.borrow(), &["source", "bindings"]);
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.work(), 26);
        assert_eq!(budget.peak_storage(), FLOOR + retained + 41 + 13);
    }

    #[test]
    fn b1_owned_stage_error_unwind_and_postflight_failure_drop_before_cleanup() {
        for mode in 0..5 {
            let log = Rc::new(RefCell::new(Vec::new()));
            let mut work = Work::new(100);
            let mut budget = ExportBudgetV1::new(&mut work, usize::MAX);
            budget.reserve_storage(FLOOR).unwrap();
            let writer =
                CheckedOutputExportWriterV1::try_new::<DropMarker>(2, &mut budget).unwrap();
            budget.reserve_storage(41).unwrap();
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                with_owned_export_stage_v1(
                    stage(&log),
                    writer,
                    41,
                    &mut budget,
                    |_, writer, budget| {
                        writer.append_v1(&[1], budget).map_err(export_error_v1)?;
                        match mode {
                            0 => Err(semantic_error()),
                            1 => std::panic::panic_any(927_u32),
                            2 => {
                                budget.release_storage(1).unwrap();
                                Err(semantic_error())
                            }
                            3 => {
                                budget
                                    .charge_work(1000)
                                    .map_err(|e| export_error_v1(e.into()))?;
                                Ok(())
                            }
                            _ => {
                                budget
                                    .release_storage(budget.storage() - FLOOR + 1)
                                    .unwrap();
                                std::panic::panic_any(993_u32);
                            }
                        }
                    },
                    split,
                )
            }));
            assert_eq!(&*log.borrow(), &["source", "bindings"]);
            if mode == 4 {
                assert_eq!(budget.storage(), FLOOR - 1);
                budget.reserve_storage(1).unwrap();
            }
            assert_eq!(budget.storage(), FLOOR);
            match (mode, outcome) {
                (
                    0,
                    Ok(Err(ProductionPipelineError::CheckedOutputExport(
                        CheckedOutputExportErrorV1::Capacity,
                    ))),
                ) => {}
                (1, Err(payload)) => assert_eq!(payload.downcast_ref::<u32>(), Some(&927)),
                (
                    2 | 4,
                    Ok(Err(ProductionPipelineError::CheckedOutputExport(
                        CheckedOutputExportErrorV1::Resource(ExportResourceV1::Accounting),
                    ))),
                ) => {}
                (
                    3,
                    Ok(Err(ProductionPipelineError::CheckedOutputExport(
                        CheckedOutputExportErrorV1::Resource(ExportResourceV1::Work(_)),
                    ))),
                ) => {}
                _ => panic!("unexpected owned-stage outcome"),
            }
            assert_eq!(budget.work(), 4 + 5 + 4);
            assert_eq!(work.failed_work(), (mode == 3).then_some(1013));
        }
    }

    #[test]
    fn b1_swallowed_append_error_cannot_make_an_owned_packet() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let mut work = Work::new(100);
        let mut budget = ExportBudgetV1::new(&mut work, usize::MAX);
        budget.reserve_storage(FLOOR).unwrap();
        let writer = CheckedOutputExportWriterV1::try_new::<DropMarker>(0, &mut budget).unwrap();
        budget.reserve_storage(41).unwrap();
        let result = with_owned_export_stage_v1(
            stage(&log),
            writer,
            41,
            &mut budget,
            |_, writer, budget| {
                assert!(writer.append_v1(&[1], budget).is_err());
                Ok(())
            },
            split,
        );
        assert!(matches!(
            result,
            Err(ProductionPipelineError::CheckedOutputExport(
                CheckedOutputExportErrorV1::Poisoned
            ))
        ));
        assert_eq!(&*log.borrow(), &["source", "bindings"]);
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.work(), 4 + 5 + 4 + 3);
    }

    #[test]
    fn b1_owned_transfer_and_receiver_exact_work_boundaries_keep_history() {
        // Prior 7 + new 4 + owned scope 5 + empty append 3 + transfer 3 = 22.
        // A receiver on that same ledger adds 2, without resetting history.
        for limit in [21, 22, 23, 24] {
            let log = Rc::new(RefCell::new(Vec::new()));
            let mut work = Work::new(limit);
            let mut budget = ExportBudgetV1::new(&mut work, usize::MAX);
            budget.charge_work(7).unwrap();
            budget.reserve_storage(FLOOR).unwrap();
            let writer =
                CheckedOutputExportWriterV1::try_new::<DropMarker>(0, &mut budget).unwrap();
            budget.reserve_storage(41).unwrap();
            let result = with_owned_export_stage_v1(
                stage(&log),
                writer,
                41,
                &mut budget,
                |_, writer, budget| {
                    writer.append_v1(&[], budget).map_err(export_error_v1)?;
                    Ok(())
                },
                split,
            );
            if limit == 21 {
                assert!(
                    matches!(result, Err(ProductionPipelineError::CheckedOutputExport(CheckedOutputExportErrorV1::Resource(ExportResourceV1::Work(error)))) if error.actual() == 22)
                );
                assert_eq!(budget.work(), 19);
            } else {
                let prepared = result.unwrap();
                assert_eq!(budget.work(), 22);
                let called = Cell::new(false);
                let result = prepared.with_accepted_v1(&mut budget, |_, _, _| {
                    called.set(true);
                    Ok(())
                });
                assert_eq!(called.get(), limit == 24);
                if limit == 24 {
                    result.unwrap();
                    assert_eq!(budget.work(), 24);
                } else {
                    assert!(
                        matches!(result, Err(ProductionPipelineError::CheckedOutputExport(CheckedOutputExportErrorV1::Resource(ExportResourceV1::Work(error)))) if error.actual() == 24)
                    );
                    assert_eq!(budget.work(), 22);
                }
            }
            assert_eq!(budget.storage(), FLOOR);
            assert_eq!(&*log.borrow(), &["source", "bindings"]);
            assert_eq!(
                work.failed_work(),
                match limit {
                    21 => Some(22),
                    22 | 23 => Some(24),
                    _ => None,
                }
            );
        }
    }

    #[test]
    fn b1_receiver_prepayment_and_all_exit_modes_preserve_prefix() {
        for mode in 0..5 {
            let log = Rc::new(RefCell::new(Vec::new()));
            let mut producer_work = Work::new(100);
            let mut producer = ExportBudgetV1::new(&mut producer_work, usize::MAX);
            let writer =
                CheckedOutputExportWriterV1::try_new::<DropMarker>(2, &mut producer).unwrap();
            producer.reserve_storage(41).unwrap();
            let prepared = with_owned_export_stage_v1(
                stage(&log),
                writer,
                41,
                &mut producer,
                |_, _, _| Ok(()),
                split,
            )
            .unwrap();
            assert_eq!(producer.storage(), 0);
            let retained = prepared.packet.retained;
            let mut work = Work::new(100);
            let mut budget = ExportBudgetV1::new(
                &mut work,
                if mode == 0 {
                    FLOOR + retained - 1
                } else {
                    usize::MAX
                },
            );
            budget.charge_work(7).unwrap();
            budget.reserve_storage(FLOOR).unwrap();
            let called = Cell::new(false);
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                prepared.with_accepted_v1(&mut budget, |_, _, budget| {
                    called.set(true);
                    match mode {
                        1 => Err(semantic_error()),
                        2 => std::panic::panic_any(619_u32),
                        3 => {
                            budget.release_storage(retained + 1).unwrap();
                            Err(semantic_error())
                        }
                        _ => Ok(()),
                    }
                })
            }));
            assert_eq!(&*log.borrow(), &["source", "bindings"]);
            assert_eq!(called.get(), mode != 0);
            if mode == 3 {
                assert!(is_accounting(&outcome.unwrap()));
                assert_eq!(budget.storage(), FLOOR - 1);
                budget.reserve_storage(1).unwrap(); // Restore only the deliberately stolen byte.
            } else {
                match (mode, outcome) {
                    (
                        0,
                        Ok(Err(ProductionPipelineError::CheckedOutputExport(
                            CheckedOutputExportErrorV1::Resource(ExportResourceV1::Storage(_)),
                        ))),
                    ) => {}
                    (
                        1,
                        Ok(Err(ProductionPipelineError::CheckedOutputExport(
                            CheckedOutputExportErrorV1::Capacity,
                        ))),
                    ) => {}
                    (2, Err(payload)) => assert_eq!(payload.downcast_ref::<u32>(), Some(&619)),
                    (4, Ok(Ok(()))) => {}
                    _ => panic!("unexpected receiver outcome"),
                }
            }
            assert_eq!(budget.storage(), FLOOR);
            assert_eq!(budget.work(), 9);
        }
    }

    #[test]
    fn b1_unit_initializer_preserves_capture_audit_and_default_route() {
        let pipeline = include_str!("production_pipeline.rs");
        let span = pipeline
            .split("fn with_materialized_capture_mode_v1<T>(")
            .nth(1)
            .unwrap()
            .split("impl MaterializedNeutralProductionCompilation")
            .next()
            .unwrap();
        assert!(span.contains("|_budget| Ok(())"));
        assert_eq!(
            span.matches(".try_capture_occurrences_with_budget_v1(")
                .count(),
            1
        );
        let initialization = span.find("let outer = initialize(&mut budget)?;").unwrap();
        let capture = span
            .find(".try_capture_occurrences_with_budget_v1(")
            .unwrap();
        let materialize = span
            .find("ProductionPreRankedKirOwnerV1::try_materialize_with_budget(")
            .unwrap();
        assert!(initialization < capture && capture < materialize);
        assert!(pipeline.contains("self.with_materialized_capture_mode_v1(false, next)"));
        assert!(!pipeline.contains(".prepare_inert_checked_output_buffer_v1("));
    }
}
