//! Actual source regression: identical physical layouts retain distinct types.

use super::*;

pub(super) fn check(
    plan: &ProductionSemanticPreflightPlanV1<'_>,
    mir: &AdmittedInertSemanticMirV1,
) {
    let producers = plan.type_producers();
    let unit = producers
        .iter()
        .position(
            |producer| matches!(producer.ty.kind(), TyKind::Tuple(fields) if fields.is_empty()),
        )
        .expect("the actual checked-constructor closure retains its unit capture tuple");
    let mut found = 0;
    for (index, producer) in producers.iter().enumerate() {
        if !matches!(producer.ty.kind(), TyKind::Closure(..))
            || producer.layout.layout != producers[unit].layout.layout
        {
            continue;
        }
        let closure = &mir.types()[index];
        let unit = &mir.types()[unit];
        assert_ne!(closure.identity(), unit.identity());
        assert_eq!(closure.layout_identity(), unit.layout_identity());
        assert_ne!(closure.layout(), unit.layout());
        assert!(matches!(unit.shape(), SemanticTypeShapeV1::Unit));
        assert!(
            matches!(closure.shape(), SemanticTypeShapeV1::Aggregate(fields) if fields.fields().is_empty())
        );
        found += 1;
    }
    assert!(
        found > 0,
        "retain the actual zero-capture checked-constructor closure"
    );
}
