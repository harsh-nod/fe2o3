// Shared ordinary semantics only. These fixtures are not nominal authority.
mod borrowed_checked_reference_controls {
    use super::super::root_checked_references_v1::{origin_for_owned_v1, require_same_source_v1};
    use super::*;

    fn owned() -> (SemanticFunctionDeclV1, CheckedReferencesV1) {
        let (function, producers) = option_dominance_chain(1);
        let option_dominance = SemanticOptionDominanceV1::analyze(&function, &producers).unwrap();
        let enum_payload_dominance =
            SemanticEnumPayloadDominanceV1::analyze(&function, &projection_types()).unwrap();
        let mut origins = vec![None; function.locals().len()];
        origins[1] = Some(CheckedReferenceOriginV1 {
            source: CheckedReferenceSourceV1::GuardedAccess(0),
            availability: Some(CapabilityAvailabilityV1::Option(
                option_dominance
                    .availability(SemanticLocalIdV1::from_index(1))
                    .unwrap(),
            )),
        });
        (
            function,
            CheckedReferencesV1 {
                origins,
                option_dominance,
                enum_payload_dominance,
            },
        )
    }

    fn place(local: u32, projections: Vec<SemanticProjectionKindV1>) -> SemanticPlaceV1 {
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(local),
            projections
                .into_iter()
                .map(|kind| SemanticProjectionV1::new(kind, SCALAR_TYPE).unwrap())
                .collect(),
            SCALAR_TYPE,
        )
        .unwrap()
    }

    // Frozen old owned-table decision, retained as a test oracle only.
    fn old_owned_decision(
        place: &SemanticPlaceV1,
        block_index: usize,
        references: &CheckedReferencesV1,
    ) -> Result<Option<CheckedReferenceSourceV1>, ProductionRankedProjectionErrorV1> {
        let Some(origin) = checked_reference_origin_for_place(place, &references.origins) else {
            return Ok(None);
        };
        if !origin.availability.is_none_or(|availability| {
            capability_availability_allows(
                &references.option_dominance,
                &references.enum_payload_dominance,
                availability,
                SemanticBlockIdV1::from_index(block_index as u32),
            )
        }) {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "a checked reference is dereferenced outside its authenticated payload region",
            ));
        }
        Ok(Some(origin.source))
    }

    #[test]
    fn borrowed_decision_matches_old_owned_decision_at_every_fixture_block() {
        let (function, references) = owned();
        let pointer = references.origins.as_ptr();
        let capacity = references.origins.capacity();
        let source = place(1, vec![SemanticProjectionKindV1::Dereference]);
        for block in 0..function.blocks().len() {
            let old = old_owned_decision(&source, block, &references);
            let new = origin_for_owned_v1(&source, block, &references);
            let wrapper = super::super::checked_reference_origin(&source, block, &references);
            assert_eq!(format!("{old:?}"), format!("{new:?}"));
            assert_eq!(format!("{old:?}"), format!("{wrapper:?}"));
        }
        assert_eq!(references.origins.as_ptr(), pointer);
        assert_eq!(references.origins.capacity(), capacity);
    }

    #[test]
    fn actual_option_some_block_allows_and_none_block_keeps_the_exact_refusal() {
        let (_, references) = owned();
        let source = place(1, vec![SemanticProjectionKindV1::Dereference]);
        assert_eq!(
            origin_for_owned_v1(&source, 2, &references).unwrap(),
            Some(CheckedReferenceSourceV1::GuardedAccess(0))
        );
        assert!(matches!(
            origin_for_owned_v1(&source, 3, &references),
            Err(ProductionRankedProjectionErrorV1::Unsupported(
                "a checked reference is dereferenced outside its authenticated payload region"
            ))
        ));
    }

    #[test]
    fn place_shape_and_absent_origin_are_rejected_before_availability() {
        let (_, references) = owned();
        for source in [
            place(1, vec![]),
            place(
                1,
                vec![
                    SemanticProjectionKindV1::Dereference,
                    SemanticProjectionKindV1::ConstantIndex {
                        offset: 0,
                        minimum_length: 1,
                        from_end: false,
                    },
                ],
            ),
            place(0, vec![SemanticProjectionKindV1::Dereference]),
            place(999, vec![SemanticProjectionKindV1::Dereference]),
        ] {
            assert_eq!(origin_for_owned_v1(&source, 3, &references).unwrap(), None);
        }
    }

    #[test]
    fn transparent_trailing_projections_and_unconditional_shared_borrow_keep_semantics() {
        let (_, mut references) = owned();
        for tail in [
            SemanticProjectionKindV1::Field(0),
            SemanticProjectionKindV1::Downcast(0),
            SemanticProjectionKindV1::OpaqueCast,
            SemanticProjectionKindV1::Subtype,
        ] {
            let source = place(1, vec![SemanticProjectionKindV1::Dereference, tail]);
            assert_eq!(
                origin_for_owned_v1(&source, 2, &references).unwrap(),
                Some(CheckedReferenceSourceV1::GuardedAccess(0))
            );
        }
        references.origins[1] = Some(CheckedReferenceOriginV1 {
            source: CheckedReferenceSourceV1::ProjectedSharedBorrow,
            availability: None,
        });
        let source = place(1, vec![SemanticProjectionKindV1::Dereference]);
        assert_eq!(
            origin_for_owned_v1(&source, 3, &references).unwrap(),
            Some(CheckedReferenceSourceV1::ProjectedSharedBorrow)
        );
    }

    #[test]
    fn source_binding_rejects_equal_content_in_foreign_function_storage() {
        let (function, _) = owned();
        let foreign = function.clone();
        require_same_source_v1(&function, &function).unwrap();
        assert!(matches!(
            require_same_source_v1(&function, &foreign),
            Err(ProductionRankedProjectionErrorV1::Incomplete(
                "nominal recipe tables do not borrow the actual source function"
            ))
        ));
    }
}
