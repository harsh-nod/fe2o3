use super::*;
use fe2o3_lower_mir_kernel::ProductionSemanticKirResourceV1;

#[test]
fn guarded_event_keeps_exact_shared_budget_error_and_spent_work() {
    let mut remaining = 7usize;
    let mut charge = |n| {
        if let Some(next) = remaining.checked_sub(n) {
            remaining = next;
            Ok(())
        } else {
            Err(E::ResourceLimit {
                resource: ProductionSemanticKirResourceV1::AnalysisWork,
                actual: 8,
                limit: 7,
            })
        }
    };
    let result = with_formula_budget(&mut charge, |f| {
        formula::charge(f, 3)?;
        formula::charge(f, 5)
    });
    assert!(
        matches!(
            result,
            Err(E::ResourceLimit {
                resource: ProductionSemanticKirResourceV1::AnalysisWork,
                actual: 8,
                limit: 7,
            })
        ),
        "{result:?}"
    );
    assert_eq!(remaining, 4);
    let mut remaining = 8usize;
    let result = with_formula_budget(
        &mut |n| {
            remaining = remaining.checked_sub(n).ok_or(E::CorrespondenceMismatch)?;
            Ok(())
        },
        |f| {
            formula::charge(f, 3)?;
            formula::charge(f, 5)
        },
    );
    result.unwrap();
    assert_eq!(remaining, 0);
}

#[test]
fn guarded_event_never_relabels_source_authentication_failure_as_budget() {
    let result = with_formula_budget(&mut |_| Err(E::CorrespondenceMismatch), |f| {
        formula::charge(f, 1)
    });
    assert!(matches!(result, Err(E::CorrespondenceMismatch)));
}

#[test]
fn physical_extent_requires_exact_retained_shared_slice_edge() {
    let element = SemanticTypeIdV1::from_index(0);
    let slice = SemanticTypeIdV1::from_index(1);
    let physical = SemanticTypeIdV1::from_index(2);
    let decl = |index: u8, shape| {
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([index; 32]),
            SemanticLayoutIdentityV1::from_sha256([index; 32]),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(8),
                8,
                SemanticBackendReprV1::memory(true),
                false,
            )
            .unwrap(),
            shape,
        )
    };
    // Shape consistency only. Actual authority always comes from the private
    // source batch over an admitted type table, never this predicate alone.
    for case in 0..7 {
        let mut types = vec![
            decl(1, SemanticTypeShapeV1::Opaque),
            decl(2, SemanticTypeShapeV1::Slice { element }),
            decl(
                3,
                SemanticTypeShapeV1::Pointer(
                    SemanticPointerTypeV1::new_with_kind(
                        slice,
                        if case == 1 {
                            SemanticPointerKindV1::Raw
                        } else {
                            SemanticPointerKindV1::Reference
                        },
                        if case == 2 {
                            SemanticMutabilityV1::Mutable
                        } else {
                            SemanticMutabilityV1::Immutable
                        },
                        if case == 3 { 1 } else { 0 },
                        if case == 4 { 32 } else { 64 },
                        if case == 5 {
                            SemanticPointerMetadataV1::None
                        } else {
                            SemanticPointerMetadataV1::SliceLength
                        },
                    )
                    .unwrap(),
                ),
            ),
        ];
        if case == 6 {
            types[1] = decl(2, SemanticTypeShapeV1::Slice { element: physical });
        }
        let result = physical_extent_slice(&types, physical, element, &mut |_| Ok(()));
        if case == 0 {
            assert_eq!(result.unwrap(), slice)
        } else {
            assert!(
                matches!(result, Err(E::CorrespondenceMismatch)),
                "case {case}: {result:?}"
            );
        }
    }
}
