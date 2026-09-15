use super::*;

#[path = "canonical_fixture.rs"]
mod canonical_fixture;
#[path = "bound_read_tests.rs"]
mod bound_read_tests;
include!("capture_flow_tests.rs");
#[path = "capture_cursor_flow_tests.rs"]
mod capture_cursor_flow_tests;

#[path = "tuple_carrier_tests.rs"]
mod tuple_carriers;

fn source() -> AdmittedInertSemanticMirV1 {
    super::super::super::defined_matrix_results::tests::fixture_source()
}

#[test]
fn matrix_context_borrow_requires_replayed_getter_occurrence() {
    let source = source();
    let expansion = SemanticCallExpansionV1::try_new(
        &source,
        fe2o3_mir_model::SemanticCallExpansionLimitsV1::default(),
    )
    .unwrap();
    let view = expansion.root(source.roots()[0]).unwrap();
    let bindings = expansion.defined_capability_bindings(&source).unwrap();
    let facts = MatrixBorrowSitesV1::new(&source, &expansion, view, &bindings, 4096).unwrap();
    assert_eq!(facts.pairs.len(), 1);
    assert!(
        MatrixBorrowSitesV1::new(&source, &expansion, view, &[], 4096)
            .unwrap()
            .pairs
            .is_empty()
    );
    let math =
        super::super::math_borrows_v1::MathBorrowSitesV1::new(&source, view, &bindings, 4096)
            .unwrap();
    assert!(
        math.pairs.is_empty(),
        "Matrix does not enter the Math authority family"
    );
}

#[test]
fn matrix_context_borrow_rejects_foreign_view_and_respects_bound() {
    let source = source();
    let expansion = SemanticCallExpansionV1::try_new(
        &source,
        fe2o3_mir_model::SemanticCallExpansionLimitsV1::default(),
    )
    .unwrap();
    let other = SemanticCallExpansionV1::try_new(
        &source,
        fe2o3_mir_model::SemanticCallExpansionLimitsV1::default(),
    )
    .unwrap();
    let view = expansion.root(source.roots()[0]).unwrap();
    let bindings = expansion.defined_capability_bindings(&source).unwrap();
    let required = bindings.len() * 32;
    assert!(MatrixBorrowSitesV1::new(&source, &expansion, view, &bindings, required).is_ok());
    assert!(MatrixBorrowSitesV1::new(&source, &expansion, view, &bindings, required - 1).is_err());
    assert!(
        MatrixBorrowSitesV1::new(
            &source,
            &expansion,
            other.root(source.roots()[0]).unwrap(),
            &bindings,
            required
        )
        .is_err()
    );
}
