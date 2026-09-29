#[repr(align(256))]
struct OriginalCorrespondenceCaptureV18<'a> {
    bytes: [u8; 2048],
    drops: &'a std::cell::Cell<usize>,
    deny: Option<&'a ScopedSourceCleanupV29>,
    panic: Option<Box<u64>>,
}

impl Drop for OriginalCorrespondenceCaptureV18<'_> {
    fn drop(&mut self) {
        assert_eq!(self.bytes[0], 0x45);
        self.drops.set(self.drops.get() + 1);
        if let Some(cleanup) = self.deny {
            cleanup.deny_refund();
        }
        if let Some(payload) = self.panic.take() {
            std::panic::resume_unwind(payload);
        }
    }
}

fn original_correspondence_callback_fixture_v18(
    consume: impl for<'source, 'inventory, 'work> FnOnce(
        &ProductionSourceOwnedViewV18<'source>,
        &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'inventory>,
        &mut ArgumentBudgetV1<'work>,
    ) -> SourceOwnedResultV18<()>,
) -> (SourceOwnedResultV18<()>, usize) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = scalar_payload_prepared_v18(&mut budget);
    let entered = std::cell::Cell::new(false);
    let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
        source.with_analysis_v18(budget, |scope| {
            scope.with_inventory_v1(|inventory, budget| {
                entered.set(true);
                consume(source, inventory, budget)
            })
        })
    });
    assert!(
        entered.get(),
        "the genuine source inventory must reach the boundary"
    );
    (result, budget.storage())
}

fn original_correspondence_header_oracle_v18<T, E, F>(_: &F) -> usize {
    use std::mem::{align_of, size_of};
    use std::panic::AssertUnwindSafe;
    type Capture<'a, F> = (
        &'a ProductionSourceOwnedViewV18<'a>,
        &'a fe2o3_kernel_analysis::CanonicalKirInventoryV18<'a>,
        F,
    );
    type Catch<'a, 'work, F> = (
        Capture<'a, F>,
        &'a mut ArgumentBudgetV1<'work>,
        &'a std::cell::Cell<usize>,
    );
    type Entry<F> = (Vec<SourceAttachmentV18>, usize, F);
    type Construction<'a, 'work, F> = (
        &'a ProductionSourceOwnedViewV18<'a>,
        &'a fe2o3_kernel_analysis::CanonicalKirInventoryV18<'a>,
        &'a mut ArgumentBudgetV1<'work>,
        &'a std::cell::Cell<usize>,
        &'a F,
    );
    type Invoke<'a, 'work, F> = (
        F,
        &'a ProductionSourceCorrespondenceV18<'a>,
        &'a mut ArgumentBudgetV1<'work>,
    );
    type Payload = Box<dyn std::any::Any + Send>;
    let disposal = size_of::<[Option<Payload>; 2]>()
        + size_of::<AssertUnwindSafe<[Option<Payload>; 2]>>()
        + 2 * size_of::<Payload>()
        + 2 * size_of::<std::thread::Result<()>>()
        + size_of::<AssertUnwindSafe<Payload>>()
        + size_of::<std::ops::Range<usize>>()
        + size_of::<usize>()
        + size_of::<bool>();
    2 * size_of::<Capture<'_, F>>()
        + 2 * align_of::<Capture<'_, F>>()
        + size_of::<Catch<'_, '_, F>>()
        + size_of::<AssertUnwindSafe<Catch<'_, '_, F>>>()
        + size_of::<Entry<F>>()
        + 2 * size_of::<SourceOwnedResultV18<Entry<F>>>()
        + size_of::<std::thread::Result<SourceOwnedResultV18<Entry<F>>>>()
        + size_of::<AssertUnwindSafe<Entry<F>>>()
        + size_of::<std::thread::Result<()>>()
        + size_of::<Construction<'_, '_, F>>()
        + size_of::<AssertUnwindSafe<Construction<'_, '_, F>>>()
        + size_of::<SourceOwnedResultV18<Vec<SourceAttachmentV18>>>()
        + size_of::<std::thread::Result<SourceOwnedResultV18<Vec<SourceAttachmentV18>>>>()
        + size_of::<std::cell::Cell<usize>>()
        + 8 * size_of::<usize>()
        + size_of::<fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1>()
        + size_of::<SourceOwnedResultV18<()>>()
        + size_of::<Invoke<'_, '_, F>>()
        + size_of::<AssertUnwindSafe<Invoke<'_, '_, F>>>()
        + size_of::<ProductionSourceCorrespondenceV18<'_>>()
        + size_of::<std::thread::Result<Result<T, E>>>()
        + size_of::<Result<T, E>>()
        + size_of::<AssertUnwindSafe<Result<T, E>>>()
        + disposal
        + source_owned_finish_header_oracle_v26::<T, E>()
}

#[test]
fn original_correspondence_owned_header_and_rows_exclude_transient_helper_credit() {
    let drops = std::cell::Cell::new(0);
    let invoked = std::cell::Cell::new(0);
    let (result, retained) =
        original_correspondence_callback_fixture_v18(|source, inventory, budget| {
            // Caller-owned credit must survive both correspondence disposal and the
            // MAIN helper's unrelated transient header convention.
            budget.reserve_storage(37)?;
            let floor = budget.storage();
            let expected = std::cell::Cell::new(0);
            let owned = OriginalCorrespondenceCaptureV18 {
                bytes: [0x45; 2048],
                drops: &drops,
                deny: None,
                panic: None,
            };
            let called = &invoked;
            let header = &expected;
            let consume = move |view: &ProductionSourceCorrespondenceV18<'_>,
                                budget: &mut ArgumentBudgetV1<'_>| {
                std::hint::black_box(&owned);
                called.set(called.get() + 1);
                assert!(!view.attachments.is_empty());
                let rows = view.attachments.len() * size_of::<SourceAttachmentV18>();
                assert_eq!(budget.storage(), floor + header.get() + rows);
                assert_eq!(view.floor, budget.storage());
                Ok::<_, ProductionSourceOwnedViewErrorV18>(())
            };
            assert!(std::mem::size_of_val(&consume) >= 2048);
            assert_eq!(std::mem::align_of_val(&consume), 256);
            expected.set(original_correspondence_header_oracle_v18::<
                (),
                ProductionSourceOwnedViewErrorV18,
                _,
            >(&consume));
            assert_eq!(
                source_correspondence_owned_headers_v18::<(), ProductionSourceOwnedViewErrorV18, _>(
                    &consume
                )? + source_owned_finish_header_oracle_v26::<(), ProductionSourceOwnedViewErrorV18>(
                ),
                expected.get()
            );
            source.with_ranked_correspondence_v18(inventory, budget, consume)?;
            assert_eq!((invoked.get(), drops.get()), (1, 1));
            assert_eq!(budget.storage(), floor);
            budget.release_storage(37)?;
            Ok(())
        });
    result.unwrap();
    assert_eq!(retained, MODULE_FLOOR);
}

#[test]
fn original_correspondence_owned_header_one_short_disposes_before_refusal_settlement() {
    for deny_drop in [false, true] {
        let drops = std::cell::Cell::new(0);
        let invoked = std::cell::Cell::new(0);
        let refusal = std::cell::Cell::new(None);
        let (result, retained) =
            original_correspondence_callback_fixture_v18(|source, inventory, budget| {
                let owned = OriginalCorrespondenceCaptureV18 {
                    bytes: [0x45; 2048],
                    drops: &drops,
                    deny: deny_drop.then_some(source.cleanup),
                    panic: None,
                };
                let called = &invoked;
                let consume = move |_: &ProductionSourceCorrespondenceV18<'_>,
                                    _: &mut ArgumentBudgetV1<'_>| {
                    std::hint::black_box(&owned);
                    called.set(called.get() + 1);
                    Ok::<_, ProductionSourceOwnedViewErrorV18>(())
                };
                let header = original_correspondence_header_oracle_v18::<
                    (),
                    ProductionSourceOwnedViewErrorV18,
                    _,
                >(&consume);
                let filler = MODULE_LIMIT - budget.storage() - header + 1;
                budget.reserve_storage(filler)?;
                let before = (budget.work(), budget.storage());
                let Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Storage(
                    error,
                ))) = source.with_ranked_correspondence_v18(inventory, budget, consume)
                else {
                    panic!("owned header must refuse");
                };
                assert_eq!(
                    (error.actual(), error.limit()),
                    (MODULE_LIMIT + 1, MODULE_LIMIT)
                );
                assert_eq!(budget.failed_storage(), Some(MODULE_LIMIT + 1));
                assert_eq!(
                    (budget.work(), budget.storage()),
                    (before.0 + 1 + 32 + 2 + 1 + 4, before.1)
                );
                assert_eq!((invoked.get(), drops.get()), (0, 1));
                assert_eq!(source.cleanup.is_denied(), deny_drop);
                refusal.set(Some(error));
                budget.release_storage(filler)?;
                Ok(())
            });
        assert!(
            matches!(result, Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Storage(error))) if Some(error) == refusal.get())
        );
        assert_eq!(drops.get(), 1);
        if deny_drop {
            assert!(retained > MODULE_FLOOR);
        } else {
            assert_eq!(retained, MODULE_FLOOR);
        }
    }
}

#[test]
fn original_correspondence_credit_reservation_is_atomic_exact_short_and_overflow_checked() {
    for short in [false, true] {
        let drops = std::cell::Cell::new(0);
        let owned = OriginalCorrespondenceCaptureV18 {
            bytes: [0x45; 2048],
            drops: &drops,
            deny: None,
            panic: None,
        };
        let consume = move |_: &ProductionSourceCorrespondenceV18<'_>,
                            _: &mut ArgumentBudgetV1<'_>| {
            std::hint::black_box(&owned);
            Ok::<_, ProductionSourceOwnedViewErrorV18>(())
        };
        let header =
            original_correspondence_header_oracle_v18::<(), ProductionSourceOwnedViewErrorV18, _>(
                &consume,
            );
        assert_eq!(
            source_correspondence_owned_headers_v18::<(), ProductionSourceOwnedViewErrorV18, _>(
                &consume
            )
            .unwrap()
                + source_owned_finish_header_oracle_v26::<(), ProductionSourceOwnedViewErrorV18>(),
            header
        );
        let mut work = CanonicalKernelIrWorkBudgetV1::new(100);
        work.charge_work(7).unwrap();
        let mut budget = ArgumentBudgetV1::new(&mut work, 13 + header - usize::from(short));
        budget.reserve_storage(13).unwrap();
        let accepted = std::cell::Cell::new(0);
        let actual = reserve_source_correspondence_credit_v18(&mut budget, &accepted, header);
        if short {
            assert!(
                matches!(actual, Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Storage(error))) if error.actual() == 13 + header && error.limit() == 12 + header)
            );
            assert_eq!((accepted.get(), budget.storage()), (0, 13));
        } else {
            actual.unwrap();
            assert_eq!((accepted.get(), budget.storage()), (header, 13 + header));
            budget.release_storage(accepted.get()).unwrap();
            assert_eq!(budget.storage(), 13);
        }
        accepted.set(usize::MAX);
        assert!(matches!(
            reserve_source_correspondence_credit_v18(&mut budget, &accepted, 1),
            Err(ProductionSourceOwnedViewErrorV18::Resource(
                ArgumentResourceV1::Arithmetic
            ))
        ));
        assert_eq!(accepted.get(), usize::MAX);
        assert_eq!(budget.storage(), 13);
        assert_eq!(budget.work(), 7);
        drop(consume);
        assert_eq!(drops.get(), 1);
    }
}

#[test]
fn original_correspondence_paid_header_and_rows_are_refunded_only_after_owned_disposal() {
    for rows_paid in [false, true] {
        for deny_drop in [false, true] {
            let drops = std::cell::Cell::new(0);
            let invoked = std::cell::Cell::new(0);
            let refusal = std::cell::Cell::new(None);
            let (result, retained) =
                original_correspondence_callback_fixture_v18(|source, inventory, budget| {
                    let census_start = budget.work();
                    let mut row_count = 0;
                    visit_source_attachment_inventory_v18(
                        source.owner,
                        inventory,
                        budget,
                        |_, _, budget| {
                            budget.charge_work(1)?;
                            row_count += 1;
                            Ok(())
                        },
                    )
                    .map_err(source_attachment_error_v18)?;
                    let census_work = budget.work() - census_start;
                    assert!(row_count > 0);
                    let prefix = 1 + 32 + 2 + 1 + 4 + if rows_paid { census_work } else { 0 };
                    // Entry query and disposal prepayment precede the mapper;
                    // the first or second mapper census
                    // begins with an atomic two-unit charge.
                    budget.charge_work(MODULE_LIMIT - budget.work() - prefix - 1)?;
                    let owned = OriginalCorrespondenceCaptureV18 {
                        bytes: [0x45; 2048],
                        drops: &drops,
                        deny: deny_drop.then_some(source.cleanup),
                        panic: None,
                    };
                    let called = &invoked;
                    let consume =
                        move |_: &ProductionSourceCorrespondenceV18<'_>,
                              _: &mut ArgumentBudgetV1<'_>| {
                            std::hint::black_box(&owned);
                            called.set(called.get() + 1);
                            Ok::<_, ProductionSourceOwnedViewErrorV18>(())
                        };
                    let header = original_correspondence_header_oracle_v18::<
                        (),
                        ProductionSourceOwnedViewErrorV18,
                        _,
                    >(&consume);
                    let floor = budget.storage();
                    let Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Work(
                        error,
                    ))) = source.with_ranked_correspondence_v18(inventory, budget, consume)
                    else {
                        panic!("trusted construction work must refuse");
                    };
                    assert_eq!(
                        (error.actual(), error.limit()),
                        (MODULE_LIMIT + 1, MODULE_LIMIT)
                    );
                    assert_eq!(budget.work(), MODULE_LIMIT - 1);
                    assert_eq!(budget.failed_work(), Some(MODULE_LIMIT + 1));
                    assert_eq!((invoked.get(), drops.get()), (0, 1));
                    let accepted = header
                        + if rows_paid {
                            row_count * size_of::<SourceAttachmentV18>()
                        } else {
                            0
                        };
                    assert_eq!(
                        budget.storage(),
                        floor + if deny_drop { accepted } else { 0 }
                    );
                    assert_eq!(source.cleanup.is_denied(), deny_drop);
                    refusal.set(Some(error));
                    Ok(())
                });
            assert!(
                matches!(result, Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Work(error))) if Some(error) == refusal.get())
            );
            if deny_drop {
                assert!(retained > MODULE_FLOOR);
            } else {
                assert_eq!(retained, MODULE_FLOOR);
            }
        }
    }
}

#[test]
fn original_correspondence_owned_capture_preserves_typed_and_single_raw_drop_outcomes() {
    // Raw body panic and raw destructor panic are separate cases; no double
    // panic recovery is promised.
    for mode in 0..4 {
        for deny_drop in [false, true] {
            let drops = std::cell::Cell::new(0);
            let invoked = std::cell::Cell::new(0);
            let (result, retained) =
                original_correspondence_callback_fixture_v18(|source, inventory, budget| {
                    if mode == 3 {
                        budget.reserve_storage(size_of::<u64>())?;
                    }
                    let floor = budget.storage();
                    let owned = OriginalCorrespondenceCaptureV18 {
                        bytes: [0x45; 2048],
                        drops: &drops,
                        deny: deny_drop.then_some(source.cleanup),
                        panic: (mode == 3).then(|| Box::new(0x1745_u64)),
                    };
                    let paid = std::cell::Cell::new(0);
                    let called = &invoked;
                    let last = &paid;
                    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        source.with_ranked_correspondence_v18(
                            inventory,
                            budget,
                            move |_, budget| {
                                std::hint::black_box(&owned);
                                called.set(called.get() + 1);
                                if mode == 2 {
                                    budget.reserve_storage(size_of::<u64>())?;
                                }
                                last.set(budget.storage());
                                if mode == 2 {
                                    std::panic::resume_unwind(Box::new(0x1745_u64));
                                }
                                if mode == 1 {
                                    return Err(ProductionSourceOwnedViewErrorV18::Binding(
                                        "original correspondence selected",
                                    ));
                                }
                                Ok(())
                            },
                        )
                    }));
                    assert_eq!((invoked.get(), drops.get()), (1, 1));
                    match mode {
                        0 if deny_drop => assert!(matches!(
                            caught.unwrap(),
                            Err(ProductionSourceOwnedViewErrorV18::Resource(
                                ArgumentResourceV1::Accounting
                            ))
                        )),
                        0 => caught.unwrap()?,
                        1 => assert!(matches!(
                            caught.unwrap(),
                            Err(ProductionSourceOwnedViewErrorV18::Binding(
                                "original correspondence selected"
                            ))
                        )),
                        _ => assert_eq!(*caught.unwrap_err().downcast::<u64>().unwrap(), 0x1745),
                    }
                    assert_eq!(
                        budget.storage(),
                        if deny_drop {
                            paid.get()
                        } else {
                            floor + if mode == 2 { size_of::<u64>() } else { 0 }
                        }
                    );
                    if mode == 2 || mode == 3 {
                        budget.release_storage(size_of::<u64>())?;
                    }
                    assert_eq!(source.cleanup.is_denied(), deny_drop);
                    Ok(())
                });
            if deny_drop {
                assert!(matches!(
                    result,
                    Err(ProductionSourceOwnedViewErrorV18::Resource(
                        ArgumentResourceV1::Accounting
                    ))
                ));
                assert!(retained > MODULE_FLOOR);
            } else {
                result.unwrap();
                assert_eq!(retained, MODULE_FLOOR);
            }
        }
    }
}

#[test]
fn original_correspondence_early_refusal_disposes_without_new_debits() {
    let drops = std::cell::Cell::new(0);
    let invoked = std::cell::Cell::new(0);
    let (result, retained) =
        original_correspondence_callback_fixture_v18(|source, inventory, budget| {
            let _: SourceOwnedResultV18<()> = source.retain_query(Err(
                ProductionSourceOwnedViewErrorV18::Binding("original correspondence early"),
            ));
            let owned = OriginalCorrespondenceCaptureV18 {
                bytes: [0x45; 2048],
                drops: &drops,
                deny: Some(source.cleanup),
                panic: None,
            };
            let before = (budget.work(), budget.storage());
            let called = &invoked;
            let actual = source.with_ranked_correspondence_v18(inventory, budget, move |_, _| {
                std::hint::black_box(&owned);
                called.set(called.get() + 1);
                Ok::<_, ProductionSourceOwnedViewErrorV18>(())
            });
            assert!(matches!(
                actual,
                Err(ProductionSourceOwnedViewErrorV18::Binding(
                    "original correspondence early"
                ))
            ));
            assert_eq!((invoked.get(), drops.get()), (0, 1));
            assert_eq!((budget.work(), budget.storage()), before);
            assert!(source.cleanup.is_denied());
            Ok(())
        });
    assert!(matches!(
        result,
        Err(ProductionSourceOwnedViewErrorV18::Binding(
            "original correspondence early"
        ))
    ));
    assert!(retained > MODULE_FLOOR);
}

#[test]
fn original_correspondence_foreign_inventory_is_rejected_before_owned_header() {
    let drops = std::cell::Cell::new(0);
    let invoked = std::cell::Cell::new(0);
    let (result, retained) = original_correspondence_callback_fixture_v18(|source, _, budget| {
        let foreign = prepared_source_fixture(ModuleFixture::Ordinary, false, budget);
        foreign.with_source_consumer_v18(budget, |other, budget| {
            other.with_analysis_v18(budget, |scope| {
                scope.with_inventory_v1(|inventory, budget| {
                    assert!(!inventory.belongs_to(&source.owner.inner.pending.graph));
                    let owned = OriginalCorrespondenceCaptureV18 {
                        bytes: [0x45; 2048],
                        drops: &drops,
                        deny: None,
                        panic: None,
                    };
                    let before = (budget.work(), budget.storage());
                    let called = &invoked;
                    let actual =
                        source.with_ranked_correspondence_v18(inventory, budget, move |_, _| {
                            std::hint::black_box(&owned);
                            called.set(called.get() + 1);
                            Ok::<_, ProductionSourceOwnedViewErrorV18>(())
                        });
                    assert!(matches!(
                        actual,
                        Err(ProductionSourceOwnedViewErrorV18::Binding(
                            "foreign canonical inventory"
                        ))
                    ));
                    assert_eq!((budget.work(), budget.storage()), (before.0 + 1, before.1));
                    assert_eq!((invoked.get(), drops.get()), (0, 1));
                    assert!(!source.cleanup.is_denied());
                    Ok::<_, ProductionSourceOwnedViewErrorV18>(())
                })
            })
        })?;
        Ok(())
    });
    assert!(matches!(
        result,
        Err(ProductionSourceOwnedViewErrorV18::Binding(
            "foreign canonical inventory"
        ))
    ));
    assert_eq!(retained, MODULE_FLOOR);
}

#[test]
fn original_correspondence_owned_entry_rejects_foreign_ledger_and_same_ledger_wrong_slot() {
    for wrong_slot in [false, true] {
        let drops = std::cell::Cell::new(0);
        let invoked = std::cell::Cell::new(0);
        let (result, retained) =
            original_correspondence_callback_fixture_v18(|source, inventory, budget| {
                let before = (budget.work(), budget.storage());
                let meter = Box::leak(Box::new(CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT)));
                let mut foreign = ArgumentBudgetV1::new(meter, MODULE_LIMIT);
                foreign.reserve_storage(before.1)?;
                if wrong_slot {
                    std::mem::swap(budget, &mut foreign);
                }
                let foreign_before = (foreign.work(), foreign.storage());
                let owned = OriginalCorrespondenceCaptureV18 {
                    bytes: [0x45; 2048],
                    drops: &drops,
                    deny: None,
                    panic: None,
                };
                let called = &invoked;
                let actual =
                    source.with_ranked_correspondence_v18(inventory, &mut foreign, move |_, _| {
                        std::hint::black_box(&owned);
                        called.set(called.get() + 1);
                        Ok::<_, ProductionSourceOwnedViewErrorV18>(())
                    });
                assert!(matches!(
                    actual,
                    Err(ProductionSourceOwnedViewErrorV18::Resource(
                        ArgumentResourceV1::Accounting
                    ))
                ));
                assert_eq!((foreign.work(), foreign.storage()), foreign_before);
                if wrong_slot {
                    std::mem::swap(budget, &mut foreign);
                }
                assert_eq!((budget.work(), budget.storage()), before);
                assert_eq!((invoked.get(), drops.get()), (0, 1));
                assert!(source.cleanup.is_denied());
                Ok(())
            });
        assert!(matches!(
            result,
            Err(ProductionSourceOwnedViewErrorV18::Resource(
                ArgumentResourceV1::Accounting
            ))
        ));
        assert!(retained > MODULE_FLOOR);
    }
}

#[test]
fn original_correspondence_restored_paid_floor_does_not_restore_refund_permission() {
    let drops = std::cell::Cell::new(0);
    let paid = std::cell::Cell::new(0);
    let (result, retained) =
        original_correspondence_callback_fixture_v18(|source, inventory, budget| {
            let owned = OriginalCorrespondenceCaptureV18 {
                bytes: [0x45; 2048],
                drops: &drops,
                deny: None,
                panic: None,
            };
            let last = &paid;
            let actual =
                source.with_ranked_correspondence_v18(inventory, budget, move |view, budget| {
                    std::hint::black_box(&owned);
                    budget.release_storage(1)?;
                    let work = budget.work();
                    assert!(matches!(
                        view.check(budget),
                        Err(ProductionSourceOwnedViewErrorV18::Resource(
                            ArgumentResourceV1::Accounting
                        ))
                    ));
                    assert_eq!(budget.work(), work);
                    budget.reserve_storage(1)?;
                    last.set(budget.storage());
                    Ok::<_, ProductionSourceOwnedViewErrorV18>(())
                });
            assert!(matches!(
                actual,
                Err(ProductionSourceOwnedViewErrorV18::Resource(
                    ArgumentResourceV1::Accounting
                ))
            ));
            assert_eq!(budget.storage(), paid.get());
            assert_eq!(drops.get(), 1);
            assert!(source.cleanup.is_denied());
            Ok(())
        });
    assert!(matches!(
        result,
        Err(ProductionSourceOwnedViewErrorV18::Resource(
            ArgumentResourceV1::Accounting
        ))
    ));
    assert!(retained >= paid.get());
}
