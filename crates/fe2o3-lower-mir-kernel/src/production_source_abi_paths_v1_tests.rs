use super::*;
use ProductionArgumentProjectionV1::{ArrayIndex as A, Field as F};

#[test]
fn source_abi_paths_header_equation_and_first_header_denial_are_exact() {
    use std::mem::{align_of, size_of};
    type Error = ProductionSemanticKirErrorV1;
    type SourceError = source_arguments_v1::ProductionSourceArgumentErrorV1;
    type Callback = for<'n> fn(ProductionArgumentNodeV1<'n>) -> Result<(), Error>;
    let expected = size_of::<Callback>()
        + align_of::<Callback>()
        + size_of::<(&mut Callback, &mut Option<Error>)>()
        + align_of::<(&mut Callback, &mut Option<Error>)>()
        + size_of::<SourceAbiPathRunV1<'_, '_, '_, Callback>>()
        + align_of::<SourceAbiPathRunV1<'_, '_, '_, Callback>>()
        + size_of::<std::panic::AssertUnwindSafe<SourceAbiPathRunV1<'_, '_, '_, Callback>>>()
        + size_of::<ProductionArgumentNodeV1<'_>>()
        + size_of::<Option<Error>>()
        + size_of::<Error>()
        + size_of::<&Option<Error>>()
        + size_of::<&ArgumentResourceV1>()
        + size_of::<&mut ArgumentResourceV1>()
        + size_of::<&SourceError>()
        + size_of::<&mut Option<SourceAbiPlanFailureV1>>()
        + size_of::<&mut SourceAbiPlanFailureV1>()
        + size_of::<&Result<Result<(), SourceError>, Box<dyn std::any::Any + Send>>>()
        + 3 * size_of::<Result<(), Error>>()
        + 2 * size_of::<Result<(), SourceError>>()
        + size_of::<Result<Result<(), SourceError>, Box<dyn std::any::Any + Send>>>()
        + size_of::<Result<(), ArgumentResourceV1>>()
        + size_of::<Result<usize, ArgumentResourceV1>>()
        + size_of::<Option<usize>>()
        + 6 * size_of::<usize>()
        + size_of::<bool>()
        + size_of::<SourceAbiPlanFailureV1>()
        + size_of::<Option<SourceAbiPlanFailureV1>>();
    assert_eq!(source_abi_paths_headers_v1::<Callback>().unwrap(), expected);
    fn unreachable_callback(_: ProductionArgumentNodeV1<'_>) -> Result<(), Error> {
        panic!("unpaid path callback");
    }
    let owner = plan_owner(4);
    let mut work = Work::new(usize::MAX);
    let limit = 1_000_000;
    let mut budget = ArgumentBudgetV1::new(&mut work, limit);
    let result = owner.with_checked_source_abi_plan_v1(ROOT, ROOT, &mut budget, |plan| {
        let floor = plan.budget.storage();
        let padding = limit.checked_sub(floor + expected - 1).unwrap();
        plan.budget.reserve_storage(padding)?;
        let before = plan.budget.work();
        assert!(matches!(plan.visit_nodes(unreachable_callback as Callback), Err(Error::ArgumentCorrespondenceResource(ArgumentResourceV1::Storage(error))) if error.actual()==limit+1 && error.limit()==limit));
        assert_eq!(plan.budget.work(), before+1);
        assert_eq!(plan.budget.storage(), floor+padding);
        plan.budget.release_storage(padding)?;
        let before = plan.budget.work();
        assert!(matches!(plan.visit_nodes(unreachable_callback as Callback), Err(Error::ArgumentCorrespondenceResource(ArgumentResourceV1::Storage(error))) if error.actual()==limit+1));
        assert_eq!(plan.budget.work(), before);
        Ok(())
    });
    assert!(
        matches!(result, Err(Error::ArgumentCorrespondenceResource(ArgumentResourceV1::Storage(error))) if error.actual()==limit+1)
    );
    assert_eq!(budget.storage(), 0);
}

#[derive(Debug, Eq, PartialEq)]
struct Observed {
    argument: u32,
    ty: SemanticTypeIdV1,
    path: Vec<ProductionArgumentProjectionV1>,
    slots: std::ops::Range<usize>,
    zero: bool,
}

#[test]
fn source_abi_paths_walker_quota_refusal_survives_owned_visitor_drop_panic() {
    use std::{
        cell::Cell,
        panic::{AssertUnwindSafe, catch_unwind},
    };
    struct PanicDrop;
    impl Drop for PanicDrop {
        fn drop(&mut self) {
            panic!("path visitor drop after quota refusal")
        }
    }
    let owner = plan_owner(4);
    let mut work = Work::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    budget.reserve_storage(23).unwrap();
    let result = owner.with_checked_source_abi_plan_v1(ROOT, ROOT, &mut budget, |plan| {
        let floor = plan.budget.storage();
        let reached = Cell::new(false);
        let reached_ref = &reached;
        let capture = PanicDrop;
        let stopped = catch_unwind(AssertUnwindSafe(|| {
            plan.visit_nodes_with_work(usize::MAX, 0, move |_| {
                let _keep = &capture;
                reached_ref.set(true);
                Ok(())
            })
        }));
        let payload = stopped.unwrap_err();
        assert_eq!(payload.downcast_ref::<&str>(), Some(&"path visitor drop after quota refusal"));
        assert!(!reached.get(), "exact node work prepayment must precede callback");
        assert_eq!(plan.budget.storage(), floor);
        let before = (plan.budget.work(), plan.budget.storage());
        assert!(matches!(plan.visit_nodes(|_| panic!("lost traversal resource")),
            Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(error)))
            if error.limit() == usize::MAX && error.actual() == usize::MAX));
        assert_eq!((plan.budget.work(), plan.budget.storage()), before);
        Ok(())
    });
    assert!(matches!(result,
        Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(error)))
        if error.limit() == usize::MAX && error.actual() == usize::MAX));
    assert_eq!(budget.storage(), 23);
    drop(budget);
    assert_eq!(work.failed_work(), Some(usize::MAX));
}
fn observe(
    plan: &mut ProductionSourceAbiPlanV1<'_, '_, '_>,
    per_node: usize,
    per_projection: usize,
) -> Result<Vec<Observed>, ProductionSemanticKirErrorV1> {
    let mut rows = Vec::new();
    plan.visit_nodes_with_work(per_node, per_projection, |node| {
        let (slots, zero) = match node.coverage() {
            ProductionArgumentCoverageV1::Zero => (0..0, true),
            ProductionArgumentCoverageV1::Components { first, end } => (first..end, false),
            ProductionArgumentCoverageV1::Parameter(row) => (row.slot()..row.slot() + 1, false),
            ProductionArgumentCoverageV1::WithinAtomicParameter(_) => {
                panic!("pointer-free source fixture")
            }
        };
        assert_eq!(
            node.local_binding().unwrap().0.index(),
            node.source_argument() + 1
        );
        assert_eq!(node.local_binding().unwrap().1, node.source_path());
        rows.push(Observed {
            argument: node.source_argument(),
            ty: node.semantic_type(),
            path: node.source_path().to_vec(),
            slots,
            zero,
        });
        Ok(())
    })?;
    Ok(rows)
}

#[test]
fn source_abi_paths_exact_siblings_arrays_nested_and_zero_nodes() {
    let zero = vec![
        vec![F(0), F(0)],
        vec![F(0), F(1)],
        vec![F(0)],
        vec![F(1), A(0)],
        vec![F(1), A(1)],
        vec![F(1)],
        vec![F(2)],
        vec![],
    ];
    for shape in 0..5 {
        let owner = plan_owner(shape);
        let mut work = Work::new(usize::MAX);
        let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
        owner
            .with_checked_source_abi_plan_v1(ROOT, ROOT, &mut budget, |plan| {
                let rows = observe(plan, 7, 3)?;
                let first = rows
                    .iter()
                    .filter(|row| row.argument == 0)
                    .collect::<Vec<_>>();
                let expected = match shape {
                    0 | 1 => vec![vec![F(0)], vec![F(1)], vec![F(2)], vec![]],
                    2 => vec![vec![A(0)], vec![A(1)], vec![A(2)], vec![]],
                    3 => zero.clone(),
                    4 => {
                        let mut paths = vec![
                            vec![F(0), F(0)],
                            vec![F(0), F(1)],
                            vec![F(0), F(2)],
                            vec![F(0)],
                        ];
                        paths.extend(zero.iter().map(|path| {
                            std::iter::once(F(1))
                                .chain(path.iter().copied())
                                .collect::<Vec<_>>()
                        }));
                        paths.extend([vec![F(2)], vec![]]);
                        paths
                    }
                    _ => unreachable!(),
                };
                assert_eq!(
                    first.iter().map(|row| row.path.clone()).collect::<Vec<_>>(),
                    expected
                );
                assert_eq!(
                    rows.iter()
                        .filter(|row| row.argument == 2)
                        .map(|row| row.path.clone())
                        .collect::<Vec<_>>(),
                    zero
                );
                assert!(
                    rows.iter()
                        .filter(|row| row.argument == 2)
                        .all(|row| row.zero)
                );
                for (slot, path) in match shape {
                    0 | 1 => vec![(0, vec![F(0)]), (1, vec![F(2)])],
                    2 => vec![(0, vec![A(0)]), (1, vec![A(1)]), (2, vec![A(2)])],
                    3 => vec![],
                    4 => vec![
                        (0, vec![F(0), F(0)]),
                        (1, vec![F(0), F(2)]),
                        (2, vec![F(2)]),
                    ],
                    _ => unreachable!(),
                } {
                    let node = first.iter().find(|node| node.path == path).unwrap();
                    assert_eq!(node.slots, slot..slot + 1);
                    assert_eq!(node.ty, plan.component(slot)?.semantic_type());
                }
                if shape == 0 {
                    assert_eq!(first[0].ty, first[2].ty);
                    assert_ne!(first[0].path, first[2].path);
                    assert_ne!(first[0].slots, first[2].slots);
                }
                Ok(())
            })
            .unwrap();
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn source_abi_paths_weighted_work_is_exact_and_repeated_queries_keep_floor() {
    let owner = plan_owner(4);
    let mut work = Work::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    owner
        .with_checked_source_abi_plan_v1(ROOT, ROOT, &mut budget, |plan| {
            let floor = plan.budget.storage();
            let before = plan.budget.work();
            let plain = observe(plan, 0, 0)?;
            let ordinary_work = plan.budget.work() - before;
            for _ in 0..4 {
                let before = plan.budget.work();
                assert_eq!(observe(plan, 13, 5)?, plain);
                assert_eq!(
                    plan.budget.work() - before,
                    ordinary_work
                        + plain.len() * 13
                        + plain.iter().map(|row| row.path.len()).sum::<usize>() * 5
                );
                assert_eq!(plan.budget.storage(), floor);
            }
            Ok(())
        })
        .unwrap();
    assert_eq!(budget.storage(), 0);
}

#[test]
fn source_abi_paths_selected_error_and_unwind_drop_visitor_before_refund() {
    use std::{
        cell::Cell,
        panic::{AssertUnwindSafe, catch_unwind},
    };
    struct DropFlag<'a>(&'a Cell<bool>);
    impl Drop for DropFlag<'_> {
        fn drop(&mut self) {
            self.0.set(true);
        }
    }
    let owner = plan_owner(4);
    let mut work = Work::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    owner
        .with_checked_source_abi_plan_v1(ROOT, ROOT, &mut budget, |plan| {
            let floor = plan.budget.storage();
            for unwind in [false, true] {
                let dropped = Cell::new(false);
                let guard = DropFlag(&dropped);
                let result = catch_unwind(AssertUnwindSafe(|| {
                    plan.visit_nodes(move |_| {
                        let _ = &guard;
                        if unwind {
                            panic!("path visitor selected panic");
                        }
                        Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
                    })
                }));
                assert!(dropped.get());
                if unwind {
                    assert!(result.is_err());
                } else {
                    assert!(matches!(
                        result,
                        Ok(Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch))
                    ));
                }
                assert_eq!(plan.budget.storage(), floor);
                observe(plan, 0, 0)?;
            }
            Ok(())
        })
        .unwrap();
    assert_eq!(budget.storage(), 0);
}

#[test]
fn source_abi_paths_arithmetic_quota_refuses_before_callback_and_replays_first() {
    let owner = plan_owner(4);
    let mut work = Work::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    let error = owner
        .with_checked_source_abi_plan_v1(ROOT, ROOT, &mut budget, |plan| {
            let floor = plan.budget.storage();
            let mut reached = false;
            assert!(matches!(
                plan.visit_nodes_with_work(usize::MAX, usize::MAX, |_| {
                    reached = true;
                    Ok(())
                }),
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Arithmetic
                    )
                )
            ));
            assert!(!reached);
            assert_eq!(plan.budget.storage(), floor);
            let before = plan.budget.work();
            assert!(matches!(
                plan.visit_nodes(|_| {
                    reached = true;
                    Ok(())
                }),
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Arithmetic
                    )
                )
            ));
            assert_eq!(plan.budget.work(), before);
            assert!(!reached);
            Err::<(), _>(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
        })
        .unwrap_err();
    assert!(matches!(
        error,
        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
            ArgumentResourceV1::Arithmetic
        )
    ));
    assert_eq!(budget.storage(), 0);
}

fn measure(
    wl: usize,
    sl: usize,
) -> (
    Result<(), ProductionSemanticKirErrorV1>,
    usize,
    usize,
    usize,
) {
    let owner = plan_owner(4);
    let mut work = Work::new(wl);
    let mut budget = ArgumentBudgetV1::new(&mut work, sl);
    budget.reserve_storage(23).unwrap();
    let result = owner.with_checked_source_abi_plan_v1(ROOT, ROOT, &mut budget, |plan| {
        observe(plan, 13, 5)?;
        Ok(())
    });
    (
        result,
        budget.work(),
        budget.peak_storage(),
        budget.storage(),
    )
}

#[test]
fn source_abi_paths_whole_transaction_exact_and_one_short() {
    let (result, work, peak, floor) = measure(usize::MAX, usize::MAX);
    result.unwrap();
    assert_eq!(floor, 23);
    let (result, used, actual, floor) = measure(work, peak);
    result.unwrap();
    assert_eq!((used, actual, floor), (work, peak, 23));
    for (wl, sl, work_cut) in [(work - 1, peak, true), (work, peak - 1, false)] {
        let (result, used, actual, floor) = measure(wl, sl);
        match (result.unwrap_err(), work_cut) {
            (
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Work(error),
                ),
                true,
            ) => {
                assert_eq!(error.limit(), wl);
                assert!(error.actual() > wl);
            }
            (
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Storage(error),
                ),
                false,
            ) => {
                assert_eq!(error.limit(), sl);
                assert!(error.actual() > sl);
            }
            other => panic!("exact path resource cause: {other:?}"),
        }
        assert!(used <= wl && actual <= sl);
        assert_eq!(floor, 23);
    }
}
