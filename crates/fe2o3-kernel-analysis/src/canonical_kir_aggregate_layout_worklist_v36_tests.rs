use super::*;
use crate::canonical_kir_private_cell_pair_resources_v1::scoped;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
    ScalarType, StorageFieldV1, StorageLayoutIdV1,
};

const LIMIT: usize = 10_000_000;

fn row(kind: LayoutKind) -> StorageLayoutV1 {
    StorageLayoutV1 {
        size: 4,
        alignment: 4,
        kind,
    }
}
fn record(children: &[u32]) -> StorageLayoutV1 {
    row(LayoutKind::Record(
        children
            .iter()
            .map(|&child| StorageFieldV1 {
                offset: 0,
                layout: StorageLayoutIdV1(child),
            })
            .collect(),
    ))
}
fn run(
    layouts: &[StorageLayoutV1],
    work: usize,
    storage: usize,
) -> (Result<Vec<Option<bool>>>, usize, usize) {
    let mut work = Work::new(work);
    let mut budget = Budget::new(&mut work, storage);
    budget.reserve_storage(37).unwrap();
    let result = scoped(&mut budget, |meter| derive(layouts, meter));
    assert_eq!(budget.storage(), 37);
    (result, budget.work(), budget.peak_storage())
}

// Deliberately independent full-table least-fixed-point oracle. Cyclic rows
// exercise the internal algorithm; they are not admitted production layouts.
fn oracle(layouts: &[StorageLayoutV1]) -> Vec<Option<bool>> {
    let mut safe = vec![None; layouts.len()];
    loop {
        let before = safe.clone();
        for (id, row) in layouts.iter().enumerate() {
            if safe[id].is_some() {
                continue;
            }
            safe[id] = match &row.kind {
                LayoutKind::Scalar(_) | LayoutKind::Vector(_) => Some(true),
                LayoutKind::Record(fields) => {
                    if fields
                        .iter()
                        .any(|f| safe[f.layout.0 as usize] == Some(false))
                    {
                        Some(false)
                    } else if fields
                        .iter()
                        .all(|f| safe[f.layout.0 as usize] == Some(true))
                    {
                        Some(true)
                    } else {
                        None
                    }
                }
                LayoutKind::Array { element, .. } => safe[element.0 as usize],
                _ => Some(false),
            };
        }
        if safe == before {
            return safe;
        }
    }
}

#[test]
fn aggregate_layout_worklist_matches_fixed_point_for_exhaustive_small_graphs() {
    let choices = [
        row(LayoutKind::Scalar(ScalarType::U32)),
        row(LayoutKind::Union(Box::new([]))),
        record(&[]),
        record(&[0]),
        record(&[1]),
        record(&[2]),
        record(&[0, 1, 1]),
        row(LayoutKind::Array {
            element: StorageLayoutIdV1(2),
            length: 0,
            stride: 4,
        }),
    ];
    for a in &choices {
        for b in &choices {
            for c in &choices {
                let layouts = [a.clone(), b.clone(), c.clone()];
                assert_eq!(run(&layouts, LIMIT, LIMIT).0.unwrap(), oracle(&layouts));
            }
        }
    }
}

fn reverse_chain(count: usize) -> Vec<StorageLayoutV1> {
    let mut layouts: Vec<_> = (1..count).map(|child| record(&[child as u32])).collect();
    layouts.push(row(LayoutKind::Scalar(ScalarType::U32)));
    layouts
}

#[test]
fn aggregate_layout_worklist_reverse_dependency_work_and_storage_scale_linearly() {
    let mut prior = None;
    for count in [32, 64, 128, 256, 512, 1024, 2048] {
        let result = run(&reverse_chain(count), LIMIT, LIMIT);
        assert_eq!(result.0.unwrap(), vec![Some(true); count]);
        // 20 table-work units, 13 per settled row, 10 per dependency edge.
        assert_eq!(result.1, 23 * count + 10);
        assert!(result.1 <= 32 * count + 64, "work {} for {count}", result.1);
        if let Some((work, storage)) = prior {
            assert!(result.1 <= 2 * work);
            assert!(result.2 <= 2 * storage);
        }
        prior = Some((result.1, result.2));
    }
}

#[test]
fn aggregate_layout_worklist_keeps_unknown_cycles_and_propagates_false_through_cycles() {
    let layouts = [record(&[1]), record(&[0])];
    assert_eq!(run(&layouts, LIMIT, LIMIT).0.unwrap(), [None, None]);
    let layouts = [
        record(&[1]),
        record(&[0, 2]),
        row(LayoutKind::Union(Box::new([]))),
    ];
    assert_eq!(run(&layouts, LIMIT, LIMIT).0.unwrap(), [Some(false); 3]);
    let layouts = [record(&[1, 1, 1]), row(LayoutKind::Scalar(ScalarType::U32))];
    assert_eq!(run(&layouts, LIMIT, LIMIT).0.unwrap(), [Some(true); 2]);
}

#[test]
fn aggregate_layout_worklist_does_not_expand_array_extents() {
    let make = |length| {
        [
            row(LayoutKind::Array {
                element: StorageLayoutIdV1(1),
                length,
                stride: 4,
            }),
            row(LayoutKind::Scalar(ScalarType::U32)),
        ]
    };
    let small = run(&make(0), LIMIT, LIMIT);
    let large = run(&make(u64::MAX), LIMIT, LIMIT);
    assert_eq!(small.0.unwrap(), large.0.unwrap());
    assert_eq!((small.1, small.2), (large.1, large.2));
}

#[test]
fn aggregate_layout_worklist_exact_and_one_short_limits_restore_storage() {
    let layouts = reverse_chain(128);
    let measured = run(&layouts, LIMIT, LIMIT);
    let expected = measured.0.unwrap();
    assert_eq!(run(&layouts, measured.1, measured.2).0.unwrap(), expected);
    assert!(matches!(
        run(&layouts, measured.1 - 1, measured.2).0,
        Err(Error::Resource(_))
    ));
    assert!(matches!(
        run(&layouts, measured.1, measured.2 - 1).0,
        Err(Error::Resource(_))
    ));
    assert!(run(&[], LIMIT, LIMIT).0.unwrap().is_empty());
}

#[test]
fn aggregate_layout_worklist_rejects_foreign_child_without_panicking() {
    assert!(matches!(
        run(&[record(&[1])], LIMIT, LIMIT).0,
        Err(Error::Inconsistent("aggregate layout child"))
    ));
}
