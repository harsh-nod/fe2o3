use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::cell::Cell;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Observation {
    pub(crate) roots: usize,
    pub(crate) mutations: usize,
    pub(crate) completed: bool,
}

std::thread_local! {
    static MODE: Cell<Option<u8>> = const { Cell::new(None) };
    static OBSERVATION: Cell<Option<Observation>> = const { Cell::new(None) };
}

pub(crate) fn with_actual_root_controls_v18<T>(
    mode: u8,
    run: impl FnOnce() -> T,
) -> (T, Option<Observation>) {
    struct Restore(Option<u8>, Option<Observation>);
    impl Drop for Restore {
        fn drop(&mut self) {
            MODE.with(|slot| slot.set(self.0));
            OBSERVATION.with(|slot| slot.set(self.1));
        }
    }
    let restore = Restore(
        MODE.with(|slot| slot.replace(Some(mode))),
        OBSERVATION.with(|slot| slot.replace(None)),
    );
    let result = run();
    let observation = OBSERVATION.with(Cell::take);
    drop(restore);
    (result, observation)
}

fn binding(result: Result<(), Error>, expected: &'static str) {
    assert!(
        matches!(result, Err(Error::Source(SourceError::Binding(detail))) if detail == expected)
    );
}

fn resource_kind(error: &Error, kind: u8) -> bool {
    matches!(
        (error, kind),
        (
            Error::Source(SourceError::Resource(Resource::Accounting)),
            1
        ) | (Error::Source(SourceError::Resource(Resource::Work(_))), 3)
            | (
                Error::Source(SourceError::Resource(Resource::Storage(_))),
                4
            )
    )
}

pub(super) fn check(
    view: &SourceNativeRankedRootRosterV18<'_, '_, '_, '_>,
    budget: &mut Budget<'_>,
) -> Result<(), Error> {
    let Some(mode) = MODE.with(Cell::take) else {
        return Ok(());
    };
    view.check_complete(budget)?;
    let count = view.root_count(budget)?;
    assert_eq!(count, 2, "genuine two-root original source fixture");
    assert!(
        view.references.as_slice().is_empty(),
        "reference proof is a later role, not fabricated here"
    );
    for ordinal in 0..count {
        let (source, actual, input, reference) = view.root(ordinal, budget)?;
        assert_eq!(
            (source, actual),
            (view.rows[ordinal].original, view.rows[ordinal].output)
        );
        assert!(std::ptr::eq(input, &view.inputs[ordinal]));
        assert!(reference.is_none());
    }
    let mut mutations = 0;
    if mode == 0 {
        let floor = budget.storage();
        budget
            .reserve_storage(
                size_of::<Result<Vec<RootRow>, ProductionRankedProjectionErrorV1>>()
                    + size_of::<SourceNativeRankedRootRosterV18<'_, '_, '_, '_>>()
                    + size_of::<Result<(), Error>>(),
            )
            .map_err(|error| resource(view.original, error))?;
        let mut rows =
            resources::rows(count + 1, budget).map_err(|error| projection(view.original, error))?;
        let required = budget.storage();
        for fault in 0..10 {
            // These inert candidates exercise the private matcher; only the
            // real accessors below publish first-error failures to the owner.
            view.check_complete(budget)?;
            budget
                .charge_work(count + 1)
                .map_err(|error| resource(view.original, error))?;
            rows.clear();
            rows.extend_from_slice(view.rows);
            let expected = match fault {
                0 => {
                    rows.pop();
                    "native root attachment changed its complete roster"
                }
                1 => {
                    rows.push(rows[0]);
                    "native root attachment changed its complete roster"
                }
                2 => {
                    rows[1] = rows[0];
                    "native root attachment changed an exact source/output row"
                }
                3 => {
                    let left = rows[0];
                    let right = rows[1];
                    rows[0].output = right.output;
                    rows[0].output_kernel = right.output_kernel;
                    rows[1].output = left.output;
                    rows[1].output_kernel = left.output_kernel;
                    "native root attachment changed an exact source/output row"
                }
                4 => {
                    rows[0].original = rows[1].original;
                    "native root attachment changed an exact source/output row"
                }
                5 => {
                    rows[0].input = Function(u32::MAX);
                    "native root attachment changed an exact source/output row"
                }
                6 => {
                    rows[0].output = Function(u32::MAX);
                    "native root attachment changed an exact source/output row"
                }
                7 => {
                    rows[0].input_kernel = usize::MAX;
                    "native root attachment changed an exact source/output row"
                }
                8 => {
                    rows[0].output_kernel = usize::MAX;
                    "native root attachment changed an exact source/output row"
                }
                9 => {
                    rows[0].reference = Some(0);
                    "native root attachment changed its reference binding"
                }
                _ => unreachable!(),
            };
            let candidate = SourceNativeRankedRootRosterV18 {
                original: view.original,
                optimized: view.optimized,
                native: view.native,
                inputs: view.inputs,
                references: view.references,
                rows: &rows,
                required,
            };
            binding(candidate.check_complete(budget), expected);
            drop(candidate);
            view.check_complete(budget)?;
            mutations += 1;
        }
        drop(rows);
        budget
            .release_storage(required - floor)
            .map_err(|error| resource(view.original, error))?;
        assert_eq!(budget.storage(), floor);
    } else if mode == 1 {
        let before = (budget.work(), budget.storage());
        let mut work = Work::new(0);
        let mut foreign = Budget::new(&mut work, 0);
        let refused = view.root_count(&mut foreign).unwrap_err();
        assert!(resource_kind(&refused, mode), "{refused:?}");
        assert_eq!(
            (
                foreign.work(),
                foreign.storage(),
                foreign.failed_work(),
                foreign.failed_storage()
            ),
            (0, 0, None, None)
        );
        assert_eq!((budget.work(), budget.storage()), before);
        assert!(resource_kind(&view.root_count(budget).unwrap_err(), mode));
        assert_eq!((budget.work(), budget.storage()), before);
    } else if mode == 2 {
        let refused = view.root(count, budget).map(|_| ());
        binding(
            refused,
            "native root attachment query is outside its roster",
        );
        let before = (budget.work(), budget.storage());
        binding(
            view.root_count(budget).map(|_| ()),
            "native root attachment query is outside its roster",
        );
        assert_eq!((budget.work(), budget.storage()), before);
    } else if mode == 3 {
        let limit =
            usize::try_from(crate::production_canonical_phase_policy_v1::WORK_LIMIT).unwrap();
        budget
            .charge_work(limit - budget.work())
            .map_err(|error| resource(view.original, error))?;
        let error = view.root_count(budget).unwrap_err();
        let Error::Source(SourceError::Resource(Resource::Work(ref limit_error))) = error else {
            panic!("{error:?}");
        };
        assert_eq!(limit_error.limit(), limit);
        assert!(limit_error.actual() > limit);
        let before = (budget.work(), budget.storage());
        assert!(resource_kind(&view.root_count(budget).unwrap_err(), mode));
        assert_eq!((budget.work(), budget.storage()), before);
    } else if mode == 7 {
        // A second genuine relation over the same original graph is still not
        // the exact relation retained by this native source proof.
        let source = view.original.source(budget)?;
        let inventory = view.original.inventory(budget)?;
        let completed = Cell::new(false);
        let result: Result<(), Error> =
            source.with_ranked_correspondence_v18(inventory, budget, |foreign, budget| {
                assert!(!std::ptr::eq(view.original, foreign));
                binding(
                    view.native
                        .check_source_subject_v18(foreign, view.optimized, budget),
                    "native root attachment substituted its original source",
                );
                let before = (budget.work(), budget.storage());
                binding(
                    view.root_count(budget).map(|_| ()),
                    "native root attachment substituted its original source",
                );
                assert_eq!((budget.work(), budget.storage()), before);
                completed.set(true);
                Ok(())
            });
        assert!(completed.get());
        binding(
            result,
            "native root attachment substituted its original source",
        );
    } else if mode == 8 {
        OBSERVATION.with(|slot| {
            slot.set(Some(Observation {
                roots: count,
                mutations,
                completed: true,
            }))
        });
        return Err(refusal("selected root attachment callback error"));
    } else if mode >= 5 {
        assert!(mode <= 6);
        assert_eq!(budget.storage(), view.required);
        budget
            .release_storage(1)
            .map_err(|error| resource(view.original, error))?;
        assert_eq!(budget.storage() + 1, view.required);
        OBSERVATION.with(|slot| {
            slot.set(Some(Observation {
                roots: count,
                mutations,
                completed: true,
            }))
        });
        if mode == 5 {
            return Err(refusal("selected root attachment callback error"));
        }
        panic!("selected root attachment callback panic after local floor loss");
    } else {
        assert_eq!(mode, 4);
        let headers = root_headers(0, 1).unwrap();
        let padding = budget.storage_limit() - budget.storage() - (headers - 1);
        budget
            .reserve_storage(padding)
            .map_err(|error| resource(view.original, error))?;
        // The zero-capture callback has no dynamic header. It must never run.
        let result = with_source_native_ranked_roots_v18(
            view.original,
            view.optimized,
            view.native,
            view.inputs,
            view.references,
            budget,
            |_, _| panic!("one-short root header reached consumer"),
        );
        let error = result.unwrap_err();
        let Error::Source(SourceError::Resource(Resource::Storage(ref short))) = error else {
            panic!("{error:?}");
        };
        assert_eq!(short.limit(), budget.storage_limit());
        assert_eq!(short.actual(), budget.storage_limit() + 1);
        let before = (budget.work(), budget.storage());
        assert!(resource_kind(&view.root_count(budget).unwrap_err(), mode));
        assert_eq!((budget.work(), budget.storage()), before);
        budget
            .release_storage(padding)
            .map_err(|error| resource(view.original, error))?;
    }
    OBSERVATION.with(|slot| {
        slot.set(Some(Observation {
            roots: count,
            mutations,
            completed: true,
        }))
    });
    // The containing public scope must replay every deliberately swallowed
    // failure above. A completed marker alone never authorizes this callback.
    Ok(())
}

#[test]
fn native_root_fixed_header_preflight_is_exact_and_rejects_one_short() {
    let callback = 17usize;
    let alignment = 8usize;
    let headers = root_headers(callback, alignment).unwrap();
    // Independent fixed representation: three subject borrows, two slices,
    // one bindings borrow and one custody floor; all vectors are standard
    // header envelopes, independent of the row payload or measured counters.
    let expected = callback
        + 2 * alignment
        + size_of::<[usize; 2]>()
        + size_of::<[usize; 9]>()
        + size_of::<Result<Vec<()>, Error>>()
        + size_of::<Result<Vec<()>, ProductionRankedProjectionErrorV1>>()
        + size_of::<Result<Vec<Vec<()>>, Error>>()
        + size_of::<Result<Vec<Vec<()>>, ProductionRankedProjectionErrorV1>>()
        + size_of::<Result<Vec<usize>, ProductionRankedProjectionErrorV1>>()
        + size_of::<Result<Vec<&str>, ProductionRankedProjectionErrorV1>>()
        + size_of::<Result<Vec<(&str, usize)>, ProductionRankedProjectionErrorV1>>()
        + size_of::<
            Result<
                resources::SourceBindingAllocationV18<'_, '_>,
                ProductionRankedProjectionErrorV1,
            >,
        >()
        + size_of::<Result<Vec<Option<usize>>, Error>>()
        + size_of::<Result<Vec<Option<usize>>, ProductionRankedProjectionErrorV1>>()
        + size_of::<fe2o3_lower_mir_kernel::ProductionOptimizedSourceCfgRootV18<'_, '_>>()
        + size_of::<
            Result<
                fe2o3_lower_mir_kernel::ProductionOptimizedSourceCfgRootV18<'_, '_>,
                SourceError,
            >,
        >()
        + size_of::<
            Result<
                (
                    SemanticFunctionIdV1,
                    Function,
                    &ProductionRankedRootInputV1,
                    Option<&crate::reference_effect_v1::AuthenticatedReferenceEffectBindingV1>,
                ),
                Error,
            >,
        >()
        + size_of::<Result<usize, Error>>()
        + size_of::<Result<Option<&fe2o3_pliron::ProductionPlironPreloweringReportV2>, Error>>()
        + 6 * size_of::<Result<(), Error>>()
        + size_of::<std::thread::Result<Result<(), Error>>>();
    assert_eq!(headers, expected);
    assert_eq!(root_headers(usize::MAX, alignment), None);
    assert_eq!(root_headers(0, usize::MAX), None);
    for short in [0, 1] {
        let floor = 23;
        let mut work = Work::new(0);
        let mut budget = Budget::new(&mut work, floor + headers - short);
        budget.reserve_storage(floor).unwrap();
        let result = budget.reserve_storage(headers);
        if short == 0 {
            result.unwrap();
            assert_eq!(budget.storage(), floor + headers);
            budget.release_storage(headers).unwrap();
        } else {
            let Resource::Storage(error) = result.unwrap_err() else {
                panic!("wrong header refusal");
            };
            assert_eq!(
                (error.limit(), error.actual()),
                (floor + headers - 1, floor + headers)
            );
        }
        assert_eq!((budget.work(), budget.storage()), (0, floor));
    }
}

fn comparison_kernel(
    capabilities: impl IntoIterator<Item = fe2o3_kernel_ir::TargetCapability>,
) -> fe2o3_kernel_ir::Kernel {
    let mut kernel = fe2o3_kernel_ir::Kernel::new(
        "root",
        "entry",
        fe2o3_kernel_ir::LaunchDomain::D1 {
            x: fe2o3_kernel_ir::LaunchExtent::Dynamic,
        },
    );
    kernel.required_capabilities.extend(capabilities);
    kernel
}

fn assert_comparison_cut(
    before: &fe2o3_kernel_ir::Kernel,
    after: &fe2o3_kernel_ir::Kernel,
    expected: usize,
    equal: bool,
) {
    let floor = 23;
    for short in [0, 1] {
        let limit = expected - short;
        let mut work = Work::new(limit);
        let mut budget = Budget::new(&mut work, floor);
        budget.reserve_storage(floor).unwrap();
        let mut compared = false;
        let result = prepay_kernel_declaration_comparison(before, after, &mut budget).map(|()| {
            compared = true;
            before == after
        });
        if short == 0 {
            assert_eq!(result.unwrap(), equal);
            assert!(compared);
            assert_eq!(budget.work(), expected);
        } else {
            let Resource::Work(error) = result.unwrap_err() else {
                panic!("wrong kernel comparison refusal");
            };
            assert_eq!((error.limit(), error.actual()), (limit, expected));
            assert!(error.actual() > error.limit());
            assert!(!compared, "one-short budget reached string equality");
        }
        assert_eq!((budget.storage(), budget.peak_storage()), (floor, floor));
    }
}

#[test]
fn native_root_kernel_comparison_prepays_empty_and_fixed_capability_tags() {
    use fe2o3_kernel_ir::TargetCapability as Capability;
    let empty = comparison_kernel([]);
    // Both kernel IDs, both entries and the two additional function-ID joins.
    assert_comparison_cut(&empty, &empty, 2 * (4 + 2 * 5), true);
    let fixed = comparison_kernel([
        Capability::Float16,
        Capability::SubgroupSize(32),
        Capability::WorkgroupBarrier,
    ]);
    // One roster visit and 16 fixed-field units for each of six entries.
    assert_comparison_cut(&fixed, &fixed, 2 * (4 + 2 * 5) + 6 * 17, true);
    assert_comparison_cut(&empty, &fixed, 2 * (4 + 2 * 5) + 3 * 17, false);
}

#[test]
fn native_root_kernel_comparison_prepays_all_extension_name_bytes() {
    use fe2o3_kernel_ir::TargetCapability as Capability;
    let before = comparison_kernel([
        Capability::Float64,
        Capability::Extension {
            namespace: "n".repeat(257),
            name: "a".repeat(1025),
        },
        Capability::Extension {
            namespace: "z".repeat(17),
            name: "b".repeat(513),
        },
    ]);
    let expected = 2 * (4 + 2 * 5) + 6 * 17 + 2 * (257 + 1025 + 17 + 513);
    assert_comparison_cut(&before, &before, expected, true);
    let changed_namespace = comparison_kernel([
        Capability::Float64,
        Capability::Extension {
            namespace: format!("{}x", "n".repeat(256)),
            name: "a".repeat(1025),
        },
        Capability::Extension {
            namespace: "z".repeat(17),
            name: "b".repeat(513),
        },
    ]);
    assert_comparison_cut(&before, &changed_namespace, expected, false);
    let changed_name = comparison_kernel([
        Capability::Float64,
        Capability::Extension {
            namespace: "n".repeat(257),
            name: format!("{}x", "a".repeat(1024)),
        },
        Capability::Extension {
            namespace: "z".repeat(17),
            name: "b".repeat(513),
        },
    ]);
    assert_comparison_cut(&before, &changed_name, expected, false);
}

#[test]
fn native_root_kernel_comparison_denies_the_roster_walk_before_byte_work() {
    use fe2o3_kernel_ir::TargetCapability as Capability;
    let kernel = comparison_kernel([Capability::Extension {
        namespace: "n".repeat(257),
        name: "a".repeat(1025),
    }]);
    let mut work = Work::new(1);
    let mut budget = Budget::new(&mut work, 0);
    let Resource::Work(error) =
        prepay_kernel_declaration_comparison(&kernel, &kernel, &mut budget).unwrap_err()
    else {
        panic!("wrong roster census refusal");
    };
    assert_eq!((error.limit(), error.actual()), (1, 2));
    assert_eq!(
        (budget.work(), budget.storage(), budget.peak_storage()),
        (0, 0, 0)
    );
}
