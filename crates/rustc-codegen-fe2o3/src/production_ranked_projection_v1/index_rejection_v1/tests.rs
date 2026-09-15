use super::*;

#[test]
fn flag_is_exact_and_default_off_without_environment_mutation() {
    use std::ffi::OsStr;
    assert!(!enabled(None));
    for value in ["", "0", "true", "01", "1 ", " 1"] {
        assert!(!enabled(Some(OsStr::new(value))));
    }
    assert!(enabled(Some(OsStr::new("1"))));
}

#[test]
fn one_fixed_record_keeps_the_first_recursive_rejection() {
    let first = RejectedUse {
        reason: "unsupported-projection",
        block: 5,
        statement: 7,
        local: Some(9),
        ty: Some(11),
        projections: [None; 2],
        projection_count: 0,
    };
    let later = RejectedUse {
        reason: "unresolved-root",
        block: 13,
        ..first
    };
    let mut off = Trace::default();
    off.note(first);
    assert_eq!(off.first, None);
    let mut on = Trace {
        enabled: true,
        ..Trace::default()
    };
    on.note(first);
    on.note(later);
    assert_eq!(on.first, Some(first));
    // No retained body, expression clone, or variable-size trace inventory.
    assert!(std::mem::size_of::<Trace>() <= 256);
}

#[test]
fn enum_carrier_keeps_original_rejection_and_first_enum_after_inner_failure() {
    let scalar = RejectedUse {
        reason: "unresolved-root",
        block: 12,
        statement: 3,
        local: Some(7),
        ty: Some(11),
        projections: [None; 2],
        projection_count: 0,
    };
    let carrier = RejectedUse {
        reason: "unsupported-projection",
        block: 165,
        statement: 0,
        local: Some(316),
        projections: [
            Some(
                SemanticProjectionV1::new(
                    SemanticProjectionKindV1::Downcast(0),
                    SemanticTypeIdV1::from_index(55),
                )
                .unwrap(),
            ),
            Some(
                SemanticProjectionV1::new(
                    SemanticProjectionKindV1::Field(0),
                    SemanticTypeIdV1::from_index(64),
                )
                .unwrap(),
            ),
        ],
        projection_count: 2,
        ..scalar
    };
    let mut off = Trace::default();
    off.note(carrier);
    assert!(off.first.is_none() && off.enum_carrier.is_none());
    let mut on = Trace {
        enabled: true,
        ..Trace::default()
    };
    on.note(scalar);
    on.note(carrier);
    on.note(RejectedUse {
        local: Some(99),
        ..carrier
    });
    assert_eq!(on.first, Some(scalar));
    assert_eq!(on.enum_carrier, EnumCarrierUse::from_rejection(carrier));
    assert!(on.enum_carrier.is_some());
    assert_eq!(on.enum_carrier.unwrap().local, 316);
    assert_eq!(on.enum_carrier.unwrap().carrier_type, 55);
    assert_eq!(on.enum_carrier.unwrap().field_type, 64);
    assert!(std::mem::size_of::<Trace>() <= 256);
}
