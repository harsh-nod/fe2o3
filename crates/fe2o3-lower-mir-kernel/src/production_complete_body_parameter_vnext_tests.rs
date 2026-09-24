//! Inert structural/ABI predicate tests only; these cannot create source owners.
use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticAbiIdentityV1, SemanticAbiValueAttributesV1, SemanticAbiValueV1,
    SemanticAggregateLayoutV1, SemanticAggregateTypeV1, SemanticFunctionAbiV1,
    SemanticLayoutIdentityV1, SemanticPointerTypeV1, SemanticRustTypeKindV1,
    SemanticTypeIdentityV1, SemanticTypeLayoutV1,
};

const SLICE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const U32: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const U8: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const POINTER: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);
const USIZE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);
const UNIT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(5);

fn declaration(
    tag: u8,
    layout: SemanticTypeLayoutV1,
    shape: SemanticTypeShapeV1,
) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([tag; 32]),
        SemanticLayoutIdentityV1::from_sha256([tag; 32]),
        layout,
        shape,
    )
}
fn scalar(tag: u8, bits: u16) -> SemanticTypeDeclV1 {
    declaration(
        tag,
        SemanticTypeLayoutV1::new(Some(u64::from(bits / 8)), u64::from(bits / 8)).unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits,
        }),
    )
}
fn aggregate(fields: Vec<SemanticTypeIdV1>, offsets: Vec<u64>) -> SemanticTypeDeclV1 {
    declaration(
        1,
        SemanticTypeLayoutV1::aggregate(
            Some(16),
            8,
            SemanticAggregateLayoutV1::new(offsets, vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(fields).unwrap()),
    )
}
fn pointer(mutability: SemanticMutabilityV1, pointee: SemanticTypeIdV1) -> SemanticTypeDeclV1 {
    declaration(
        4,
        SemanticTypeLayoutV1::new(Some(8), 8).unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new(pointee, mutability, 0, 64, SemanticPointerMetadataV1::None)
                .unwrap(),
        ),
    )
}
fn types() -> Vec<SemanticTypeDeclV1> {
    vec![
        aggregate(vec![POINTER, USIZE, UNIT], vec![0, 8, 16]),
        scalar(2, 32),
        scalar(3, 8),
        pointer(SemanticMutabilityV1::Mutable, U32),
        scalar(5, 64),
        declaration(
            6,
            SemanticTypeLayoutV1::new(Some(0), 1).unwrap(),
            SemanticTypeShapeV1::Unit,
        ),
    ]
}
fn abi(
    output: SemanticTypeIdV1,
    ownership: SemanticSourceArgumentOwnershipV1,
) -> SemanticFunctionAbiV1 {
    let plain = SemanticAbiValueAttributesV1::plain();
    let mut arguments = vec![SemanticAbiValueV1::new(
        output,
        SemanticAbiPassModeV1::Pair {
            first: plain,
            second: plain,
        },
    )];
    arguments
        .extend((0..4).map(|_| SemanticAbiValueV1::new(U32, SemanticAbiPassModeV1::Direct(plain))));
    arguments
        .extend((0..5).map(|_| SemanticAbiValueV1::new(U8, SemanticAbiPassModeV1::Direct(plain))));
    let mut ownerships = vec![ownership];
    ownerships.extend([SemanticSourceArgumentOwnershipV1::ByValue; 9]);
    SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1::from_sha256([7; 32]),
        SemanticLayoutIdentityV1::from_sha256([8; 32]),
        SemanticCanonAbiV1::Rust,
        false,
        false,
        arguments,
        SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap()
    .with_source_argument_ownership(ownerships)
    .unwrap()
}

#[test]
fn exact_marker_abi_and_field_roles_select_only_current_type() {
    let types = types();
    let abi = abi(SLICE, SemanticSourceArgumentOwnershipV1::ExclusiveOwner);
    assert_eq!(
        complete_body_marker_slice_abi_vnext(&types, &abi, SLICE),
        Some((U32, USIZE))
    );
    assert_eq!(
        complete_body_marker_slice_abi_vnext(&types, &abi, POINTER),
        None
    );
    // A structurally identical declaration is not the bound nominal type.
    let mut lookalike = types.clone();
    lookalike.push(types[0].clone());
    assert_eq!(
        complete_body_marker_slice_abi_vnext(&lookalike, &abi, SemanticTypeIdV1::from_index(6)),
        None
    );
}

#[test]
fn marker_ownership_must_be_exact_exclusive_owner() {
    for ownership in [
        SemanticSourceArgumentOwnershipV1::Unspecified,
        SemanticSourceArgumentOwnershipV1::ByValue,
        SemanticSourceArgumentOwnershipV1::RawPointer,
        SemanticSourceArgumentOwnershipV1::SharedBorrow,
        SemanticSourceArgumentOwnershipV1::UniqueBorrow,
    ] {
        assert_eq!(
            complete_body_marker_slice_abi_vnext(&types(), &abi(SLICE, ownership), SLICE),
            None
        );
    }
}

#[test]
fn marker_slice_rejects_reordered_missing_and_extra_fields() {
    for (fields, offsets) in [
        (vec![USIZE, POINTER, UNIT], vec![0, 8, 16]),
        (vec![POINTER, USIZE], vec![0, 8]),
        (vec![POINTER, USIZE, UNIT, UNIT], vec![0, 8, 16, 16]),
        (vec![POINTER, USIZE, UNIT], vec![8, 0, 16]),
        (vec![POINTER, USIZE, UNIT], vec![0, 8, 0]),
    ] {
        let mut altered = types();
        altered[0] = aggregate(fields, offsets);
        assert_eq!(
            complete_body_marker_slice_abi_vnext(
                &altered,
                &abi(SLICE, SemanticSourceArgumentOwnershipV1::ExclusiveOwner),
                SLICE
            ),
            None
        );
    }
}

#[test]
fn marker_slice_rejects_pointer_role_and_separate_schema_drift() {
    for replacement in [
        pointer(SemanticMutabilityV1::Immutable, U32),
        pointer(SemanticMutabilityV1::Mutable, U8),
    ] {
        let mut altered = types();
        altered[3] = replacement;
        assert_eq!(
            complete_body_marker_slice_abi_vnext(
                &altered,
                &abi(SLICE, SemanticSourceArgumentOwnershipV1::ExclusiveOwner),
                SLICE
            ),
            None
        );
    }
    let mut altered = types();
    altered[4] = scalar(5, 64).with_rust_type_kind(SemanticRustTypeKindV1::Usize);
    // V35 nominal kinds must not leak into the separate exact MIR36 profile.
    assert_eq!(
        complete_body_marker_slice_abi_vnext(
            &altered,
            &abi(SLICE, SemanticSourceArgumentOwnershipV1::ExclusiveOwner),
            SLICE
        ),
        None
    );
}

#[test]
fn marker_slice_rejects_scalar_and_phantom_drift() {
    let mut altered = types();
    altered[1] = scalar(2, 16);
    assert_eq!(
        complete_body_marker_slice_abi_vnext(
            &altered,
            &abi(SLICE, SemanticSourceArgumentOwnershipV1::ExclusiveOwner),
            SLICE
        ),
        None
    );
    let mut altered = types();
    altered[5] = scalar(6, 8);
    assert_eq!(complete_body_slice_fields_vnext(&altered, SLICE, U32), None);
}
