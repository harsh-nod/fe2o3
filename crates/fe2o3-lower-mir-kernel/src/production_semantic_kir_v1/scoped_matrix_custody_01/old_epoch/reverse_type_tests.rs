use super::super::reverse_types::ReverseTypes;
use super::*;

// Exact structural-closure differentials, not a source/SSA/capability issuer.
fn compare(declarations: &[SemanticTypeDeclV1], seeds: &[SemanticTypeIdV1]) {
    let mut reverse = ReverseTypes::new(declarations, &mut |_| Ok(())).unwrap();
    let marked = reverse.classify(seeds, &mut |_| Ok(())).unwrap();
    let mut reference = Types {
        types: declarations,
        seeds,
        complete: BTreeMap::new(),
    };
    for start in 0..declarations.len() {
        let expected = reference
            .contains(id(start as u32), &mut |_| Ok(()))
            .unwrap();
        assert_eq!(
            marked.contains(start),
            expected,
            "start={start} seeds={seeds:?}"
        );
    }
}

#[test]
fn typed_cost_reverse_closure_matches_scc_peers_every_field_and_seed() {
    let mut declarations = declarations();
    declarations.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([91; 32]),
        SemanticLayoutIdentityV1::from_sha256([92; 32]),
        SemanticTypeLayoutV1::new(Some(0), 1).unwrap(),
        SemanticTypeShapeV1::Enum {
            discriminant: id(1),
            variants: vec![
                SemanticEnumVariantV1::new(0, fields(&[8])),
                SemanticEnumVariantV1::new(1, fields(&[7, 11])),
            ]
            .into_boxed_slice(),
        },
    ));
    for seed in 0..declarations.len() {
        compare(&declarations, &[id(seed as u32)]);
    }
    compare(&declarations, &[id(1), id(2), id(2)]);
}

#[test]
fn typed_cost_reverse_closure_propagates_opaque_without_cross_scope_marks() {
    let mut declarations = declarations();
    declarations[8] = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([9; 32]),
        SemanticLayoutIdentityV1::from_sha256([9; 32]),
        SemanticTypeLayoutV1::new(Some(0), 1).unwrap(),
        SemanticTypeShapeV1::Opaque,
    );
    declarations[5] = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([6; 32]),
        SemanticLayoutIdentityV1::from_sha256([6; 32]),
        SemanticTypeLayoutV1::new(Some(0), 1).unwrap(),
        SemanticTypeShapeV1::Array {
            element: id(8),
            length: 4,
        },
    );
    let mut reverse = ReverseTypes::new(&declarations, &mut |_| Ok(())).unwrap();
    for seeds in [
        vec![id(2)],
        vec![id(1)],
        vec![id(8)],
        vec![id(0)],
        vec![id(2)],
    ] {
        let marked = reverse.classify(&seeds, &mut |_| Ok(())).unwrap();
        let mut reference = Types {
            types: &declarations,
            seeds: &seeds,
            complete: BTreeMap::new(),
        };
        for start in 0..declarations.len() {
            assert_eq!(
                marked.contains(start),
                reference
                    .contains(id(start as u32), &mut |_| Ok(()))
                    .unwrap()
            );
        }
        assert!(marked.contains(8));
        assert!(marked.contains(5));
    }
}

#[test]
fn typed_cost_reverse_closure_never_returns_partial_marks_on_budget_failure() {
    let declarations = declarations();
    let mut total = 0;
    let mut reverse = ReverseTypes::new(&declarations, &mut |_| Ok(())).unwrap();
    reverse
        .classify(&[id(2)], &mut |n| {
            total += n;
            Ok(())
        })
        .unwrap();
    for limit in 0..total {
        let mut remaining = limit;
        let result = reverse.classify(&[id(2)], &mut |n| {
            remaining =
                remaining
                    .checked_sub(n)
                    .ok_or(ProductionSemanticKirErrorV1::ResourceLimit {
                        resource: ProductionSemanticKirResourceV1::AnalysisWork,
                        actual: limit + 1,
                        limit,
                    })?;
            Ok(())
        });
        assert!(
            matches!(result, Err(ProductionSemanticKirErrorV1::ResourceLimit { limit: found, .. }) if found == limit)
        );
        // An independent following query must reset even a half-written flag
        // array. This models caller recovery, not a fresh production allowance.
        let marked = reverse.classify(&[id(1)], &mut |_| Ok(())).unwrap();
        assert!(marked.contains(1));
        assert!(!marked.contains(2));
        assert!(!marked.contains(6));
    }
}

#[test]
fn typed_cost_reverse_closure_keeps_invalid_edges_and_seeds_closed() {
    let mut declarations = declarations();
    declarations[8] = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([9; 32]),
        SemanticLayoutIdentityV1::from_sha256([9; 32]),
        SemanticTypeLayoutV1::new(Some(0), 1).unwrap(),
        SemanticTypeShapeV1::Tuple(fields(&[99])),
    );
    assert!(matches!(
        ReverseTypes::new(&declarations, &mut |_| Ok(())),
        Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
    ));
    let mut reverse = ReverseTypes::new(&declarations[..8], &mut |_| Ok(())).unwrap();
    assert!(matches!(
        reverse.classify(&[], &mut |_| Ok(())),
        Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
    ));
    assert!(matches!(
        reverse.classify(&[id(99)], &mut |_| Ok(())),
        Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
    ));
}

#[test]
fn typed_cost_reverse_shared_subgraphs_have_exact_linear_query_debits() {
    for count in [32usize, 64, 128] {
        let declarations: Vec<_> = (0..count)
            .map(|index| {
                SemanticTypeDeclV1::new(
                    SemanticTypeIdentityV1::from_sha256([index as u8 + 1; 32]),
                    SemanticLayoutIdentityV1::from_sha256([index as u8 + 1; 32]),
                    SemanticTypeLayoutV1::new(Some(0), 1).unwrap(),
                    if index == 0 {
                        SemanticTypeShapeV1::Unit
                    } else if index == 1 {
                        SemanticTypeShapeV1::Tuple(fields(&[]))
                    } else {
                        SemanticTypeShapeV1::Tuple(fields(&[index as u32 - 1, index as u32 - 1]))
                    },
                )
            })
            .collect();
        let mut build = 0;
        let mut reverse = ReverseTypes::new(&declarations, &mut |n| {
            build += n;
            Ok(())
        })
        .unwrap();
        let mut query = 0;
        let marked = reverse
            .classify(&[id(1)], &mut |n| {
                query += n;
                Ok(())
            })
            .unwrap();
        let mut reference = Types {
            types: &declarations,
            seeds: &[id(1)],
            complete: BTreeMap::new(),
        };
        let mut old_work = 0;
        for start in 0..count {
            assert_eq!(
                marked.contains(start),
                reference
                    .contains(id(start as u32), &mut |n| {
                        old_work += n;
                        Ok(())
                    })
                    .unwrap()
            );
        }
        let marker_words = std::mem::size_of::<super::super::reverse_types::Marked<'_>>()
            .div_ceil(std::mem::size_of::<usize>());
        let nodes = count - 1;
        let edges = 2 * (count - 2);
        assert_eq!(
            query,
            2 + marker_words + 2 * count + 3 + 2 * nodes + 5 * edges
        );
        assert!(
            build + query < old_work,
            "T={count} new={} old={old_work}",
            build + query
        );
    }
}
