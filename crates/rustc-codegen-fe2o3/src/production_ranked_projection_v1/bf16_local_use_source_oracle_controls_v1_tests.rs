// Synthetic controls for the independent observer oracle only. No actual loan.
mod local_use_source_oracle_controls {
    use super::super::bf16_nominal_preparation_resources_v1::PreparationResourcesV1;
    use super::super::local_use_source_oracle::*;
    use super::*;
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrWorkBudgetV1 as Work,
    };
    const LIMIT: usize = 16 * 1024 * 1024;
    fn function(
        statements: Vec<SemanticStatementV1>,
        term: SemanticTerminatorKindV1,
    ) -> SemanticFunctionDeclV1 {
        projection_function_with_locals(
            vec![block(51, statements, term)],
            (0u8..8)
                .map(|i| {
                    local(
                        50 + i,
                        SCALAR_TYPE,
                        match i {
                            0 => SemanticLocalRoleV1::Return,
                            1 => SemanticLocalRoleV1::Argument(0),
                            _ => SemanticLocalRoleV1::Temporary,
                        },
                    )
                })
                .collect(),
        )
    }
    fn place(i: u32) -> SemanticPlaceV1 {
        SemanticPlaceV1::new(SemanticLocalIdV1::from_index(i), vec![], SCALAR_TYPE).unwrap()
    }
    fn deref(i: u32) -> SemanticPlaceV1 {
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(i),
            vec![
                SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, SCALAR_TYPE)
                    .unwrap(),
            ],
            SCALAR_TYPE,
        )
        .unwrap()
    }
    fn assign(dst: u32, src: u32) -> SemanticStatementV1 {
        typed_assignment(
            dst,
            SCALAR_TYPE,
            SemanticRvalueKindV1::Use(typed_operand(src, SCALAR_TYPE)),
        )
    }
    fn with_scalar(f: &SemanticFunctionDeclV1, inspect: impl FnOnce(&ScalarOracle)) {
        let mut w = Work::new(LIMIT);
        let mut b = Budget::new(&mut w, LIMIT);
        b.reserve_storage(37).unwrap();
        let mut owned = 0;
        let mut o = ScalarOracle::default();
        o.prepare(f, &mut PreparationResourcesV1::new(&mut b, &mut owned))
            .unwrap();
        inspect(&o);
        drop(o);
        b.release_storage(owned).unwrap();
        assert_eq!(b.storage(), 37);
    }
    #[test]
    fn independent_local_oracle_counts_assignments_and_four_term_predicate() {
        let f = function(
            vec![assign(1, 0), assign(2, 0), assign(3, 0), assign(3, 0)],
            SemanticTerminatorKindV1::Return,
        );
        with_scalar(&f, |o| {
            assert_eq!(o.counts, [0, 1, 1, 2, 0, 0, 0, 0]);
            assert!(!o.immutable(&f, 1).unwrap());
            assert!(o.immutable(&f, 2).unwrap());
            assert!(!o.immutable(&f, 3).unwrap());
            assert!(!o.immutable(&f, 4).unwrap());
            assert_eq!(
                o.assignments[2],
                Some(ScalarAssignmentSiteV1 {
                    block: 0,
                    statement: 1
                })
            );
            assert!(o.assignments[3].is_none());
        });
    }
    #[test]
    fn independent_local_oracle_escape_blocks_single_definition_immutability() {
        let f = function(
            vec![
                assign(2, 0),
                typed_assignment(
                    3,
                    POINTER_TYPE,
                    SemanticRvalueKindV1::Borrow {
                        kind: SemanticBorrowKindV1::Mutable,
                        place: place(2),
                    },
                ),
            ],
            SemanticTerminatorKindV1::Return,
        );
        with_scalar(&f, |o| {
            assert!(o.escaped[2]);
            assert!(!o.immutable(&f, 2).unwrap());
        });
    }
    #[test]
    fn independent_local_oracle_dereference_write_does_not_define_pointer() {
        let f = function(
            vec![statement(SemanticStatementKindV1::Assign(
                SemanticAssignmentV1::new(
                    deref(2),
                    SemanticRvalueV1::new(
                        SCALAR_TYPE,
                        SemanticRvalueKindV1::Use(typed_operand(1, SCALAR_TYPE)),
                    ),
                ),
            ))],
            SemanticTerminatorKindV1::Return,
        );
        with_scalar(&f, |o| {
            assert_eq!(o.counts[2], 0);
            assert!(o.assignments[2].is_none());
        });
    }
    #[test]
    fn independent_local_oracle_refuses_revisit_unmetered_and_bad_query() {
        let f = function(vec![assign(2, 0)], SemanticTerminatorKindV1::Return);
        assert!(
            ScalarOracle::default()
                .prepare(&f, &mut PreparationResourcesV1::unmetered())
                .is_err()
        );
        let mut w = Work::new(LIMIT);
        let mut b = Budget::new(&mut w, LIMIT);
        let mut owned = 0;
        let mut o = ScalarOracle::default();
        o.prepare(&f, &mut PreparationResourcesV1::new(&mut b, &mut owned))
            .unwrap();
        let before = (b.work(), b.storage());
        assert!(
            o.prepare(&f, &mut PreparationResourcesV1::new(&mut b, &mut owned))
                .is_err()
        );
        assert_eq!((b.work(), b.storage()), before);
        assert!(o.immutable(&f, 999).is_err());
        drop(o);
        b.release_storage(owned).unwrap();
    }
    #[test]
    fn independent_local_oracle_partial_allocation_stays_owned_until_drop() {
        let f = function(vec![assign(2, 0)], SemanticTerminatorKindV1::Return);
        let mut w = Work::new(LIMIT);
        let mut b = Budget::new(&mut w, 8);
        let mut owned = 0;
        let mut o = ScalarOracle::default();
        assert!(
            o.prepare(&f, &mut PreparationResourcesV1::new(&mut b, &mut owned))
                .is_err()
        );
        assert!(b.failed_storage().is_some());
        assert!(owned > 0);
        assert_eq!(b.storage(), owned);
        drop(o);
        b.release_storage(owned).unwrap();
        assert_eq!(b.storage(), 0);
        assert!(b.failed_storage().is_some());
    }
    #[test]
    fn independent_occurrence_walk_preserves_rhs_then_lhs_and_actual_pointer() {
        let f = function(
            vec![statement(SemanticStatementKindV1::Assign(
                SemanticAssignmentV1::new(
                    deref(2),
                    SemanticRvalueV1::new(
                        SCALAR_TYPE,
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(deref(1))),
                    ),
                ),
            ))],
            SemanticTerminatorKindV1::Return,
        );
        let SemanticStatementKindV1::Assign(a) = f.blocks()[0].statements()[0].kind() else {
            panic!("fixture")
        };
        let SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(rhs)) = a.value().kind() else {
            panic!("fixture")
        };
        let mut w = Work::new(LIMIT);
        let mut b = Budget::new(&mut w, LIMIT);
        let mut owned = 0;
        let mut count = 0;
        walk_occurrences(
            &f,
            &mut PreparationResourcesV1::new(&mut b, &mut owned),
            &mut |o, _| {
                assert_eq!(
                    o.coordinate,
                    Coordinate {
                        block: 0,
                        statement: Some(0),
                        ordinal: count
                    }
                );
                assert!(std::ptr::eq(
                    o.place,
                    if count == 0 { rhs } else { a.destination() }
                ));
                assert_eq!(
                    o.access,
                    if count == 0 {
                        AccessKindAttr::Read
                    } else {
                        AccessKindAttr::Write
                    }
                );
                count += 1;
                Ok(())
            },
            &mut |_, _| panic!("unexpected call"),
        )
        .unwrap();
        assert_eq!(count, 2);
        assert_eq!(owned, 0);
    }
    #[test]
    fn independent_occurrence_walk_borrow_address_is_not_value_access() {
        let f = function(
            vec![typed_assignment(
                2,
                POINTER_TYPE,
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Shared,
                    place: deref(1),
                },
            )],
            SemanticTerminatorKindV1::Return,
        );
        let mut w = Work::new(LIMIT);
        let mut b = Budget::new(&mut w, LIMIT);
        let mut owned = 0;
        let mut count = 0;
        walk_occurrences(
            &f,
            &mut PreparationResourcesV1::new(&mut b, &mut owned),
            &mut |o, _| {
                assert_eq!(o.coordinate.ordinal, 0);
                assert!(o.place.local() == SemanticLocalIdV1::from_index(2));
                assert_eq!(o.access, AccessKindAttr::Write);
                count += 1;
                Ok(())
            },
            &mut |_, _| Ok(()),
        )
        .unwrap();
        assert_eq!(count, 1);
    }
    #[test]
    fn independent_occurrence_walk_call_boundary_never_manufactures_operands() {
        let term = SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(0),
                vec![typed_operand(1, SCALAR_TYPE)],
                None,
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        );
        let f = function(vec![], term);
        let mut w = Work::new(LIMIT);
        let mut b = Budget::new(&mut w, LIMIT);
        let mut owned = 0;
        let mut count = 0;
        walk_occurrences(
            &f,
            &mut PreparationResourcesV1::new(&mut b, &mut owned),
            &mut |_, _| panic!("call operands are not admitted use occurrences"),
            &mut |c, _| {
                assert_eq!(
                    c,
                    Coordinate {
                        block: 0,
                        statement: None,
                        ordinal: 0
                    }
                );
                count += 1;
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(count, 1);
    }
    #[test]
    fn independent_occurrence_walk_exact_work_boundary_and_sticky_denial() {
        let f = function(
            vec![statement(SemanticStatementKindV1::Assume(typed_operand(
                1,
                SCALAR_TYPE,
            )))],
            SemanticTerminatorKindV1::Return,
        );
        // block64 + statement64 + operand16 + place64 + terminator64.
        for cap in [271, 272] {
            let mut w = Work::new(cap);
            let mut b = Budget::new(&mut w, LIMIT);
            let mut owned = 0;
            let result = walk_occurrences(
                &f,
                &mut PreparationResourcesV1::new(&mut b, &mut owned),
                &mut |_, _| Ok(()),
                &mut |_, _| Ok(()),
            );
            assert_eq!(result.is_ok(), cap == 272);
            assert_eq!(owned, 0);
            if cap == 271 {
                assert!(b.failed_work().is_some());
                assert!(
                    walk_occurrences(
                        &f,
                        &mut PreparationResourcesV1::new(&mut b, &mut owned),
                        &mut |_, _| Ok(()),
                        &mut |_, _| Ok(())
                    )
                    .is_err()
                );
            }
        }
    }
    #[test]
    fn independent_origin_decision_some_none_and_region_refusal_are_distinct() {
        let (f, producers) = option_dominance_chain(1);
        let option = SemanticOptionDominanceV1::analyze(&f, &producers).unwrap();
        let enums = SemanticEnumPayloadDominanceV1::analyze(&f, &projection_types()).unwrap();
        let mut origins = vec![None; f.locals().len()];
        origins[1] = Some(CheckedReferenceOriginV1 {
            source: CheckedReferenceSourceV1::GuardedAccess(0),
            availability: Some(CapabilityAvailabilityV1::Option(
                option
                    .availability(SemanticLocalIdV1::from_index(1))
                    .unwrap(),
            )),
        });
        let mut w = Work::new(LIMIT);
        let mut b = Budget::new(&mut w, LIMIT);
        let mut owned = 0;
        let mut some = 0;
        let mut refused = 0;
        for bi in 0..f.blocks().len() {
            let mut r = PreparationResourcesV1::new(&mut b, &mut owned);
            match expected_origin(&deref(1), bi, &origins, &option, &enums, &mut r).unwrap() {
                OriginDecision::Allowed(Some(CheckedReferenceSourceV1::GuardedAccess(0))) => {
                    some += 1
                }
                OriginDecision::RegionRefused => refused += 1,
                _ => panic!("wrong expected decision"),
            }
            assert_eq!(
                expected_origin(&place(1), bi, &origins, &option, &enums, &mut r).unwrap(),
                OriginDecision::Allowed(None)
            );
        }
        assert!(some > 0 && refused > 0);
        assert_eq!(owned, 0);
    }
}
