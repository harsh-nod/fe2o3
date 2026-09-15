use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticAggregateLayoutV1, SemanticAggregateTypeV1, SemanticLayoutIdentityV1,
    SemanticTypeIdentityV1, SemanticTypeLayoutV1,
};

fn ty(index: u32) -> SemanticTypeIdV1 {
    SemanticTypeIdV1::from_index(index)
}

fn record(shape: SemanticTypeShapeV1, count: usize) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([1; 32]),
        SemanticLayoutIdentityV1::from_sha256([2; 32]),
        SemanticTypeLayoutV1::aggregate(
            Some(0),
            1,
            SemanticAggregateLayoutV1::new(vec![0; count], vec![]).unwrap(),
        )
        .unwrap(),
        shape,
    )
}

#[test]
fn publish_uses_exact_tuple_fields_not_an_aggregate_accessor() {
    // Component field extraction only: no issuer, Workgroup, or epoch receipt.
    let tuple = record(
        SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![ty(7), ty(9)]).unwrap()),
        2,
    );
    assert_eq!(
        canonical_publish_fields(&[tuple.clone()], ty(0)).unwrap(),
        [ty(7), ty(9)]
    );
    assert!(
        canonical_field(&[tuple], ty(0), 0).is_err(),
        "the aggregate-only accessor must remain closed for struct consumers"
    );
    let reversed = record(
        SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![ty(9), ty(7)]).unwrap()),
        2,
    );
    assert_eq!(
        canonical_publish_fields(&[reversed], ty(0)).unwrap(),
        [ty(9), ty(7)],
        "source identity checks see original field order; extraction must not sort"
    );
}

#[test]
fn publish_field_extraction_rejects_struct_wrong_arity_and_absent_type() {
    let aggregate = record(
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![ty(7), ty(9)]).unwrap()),
        2,
    );
    assert!(canonical_publish_fields(&[aggregate.clone()], ty(0)).is_err());
    assert_eq!(canonical_field(&[aggregate], ty(0), 0).unwrap(), ty(7));
    for fields in [vec![], vec![ty(7)], vec![ty(7), ty(9), ty(11)]] {
        let count = fields.len();
        let tuple = record(
            SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(fields).unwrap()),
            count,
        );
        assert!(canonical_publish_fields(&[tuple], ty(0)).is_err());
    }
    assert!(canonical_publish_fields(&[], ty(0)).is_err());
}
