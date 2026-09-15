//! Exact inert operand forms only. These tests do not assert source ownership,
//! SSA initialization, barrier discharge, or an executable transpose flow.
use super::*;

fn id(index: u32) -> SemanticTypeIdV1 {
    SemanticTypeIdV1::from_index(index)
}
fn local(index: u32) -> SemanticLocalIdV1 {
    SemanticLocalIdV1::from_index(index)
}
fn place(index: u32, ty: u32) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(local(index), vec![], id(ty)).unwrap()
}
fn erased(ty: u32) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        id(ty),
        SemanticConstantValueV1::ZeroSized,
    ))
}
fn types() -> Vec<SemanticTypeDeclV1> {
    (0..2)
        .map(|index| {
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([index + 1; 32]),
                SemanticLayoutIdentityV1::from_sha256([index + 1; 32]),
                SemanticTypeLayoutV1::with_exact_rustc_layout(
                    0,
                    1,
                    SemanticFieldsShapeV1::Arbitrary {
                        source_order_offsets_bytes: Box::new([]),
                        memory_order_source_indices: Box::new([]),
                    },
                    SemanticRustcVariantsV1::Single { index: 0 },
                    SemanticBackendReprV1::memory(true),
                    None,
                    false,
                    None,
                    1,
                    0,
                    SemanticTypeLayoutDetailsV1::None,
                )
                .unwrap(),
                SemanticTypeShapeV1::Unit,
            )
        })
        .collect()
}

#[test]
fn transpose_footer_tile_preserves_exact_move_or_typed_erasure() {
    let types = types();
    for producer in [place(3, 0), place(5, 1)] {
        assert!(tile_operand(
            &types,
            &SemanticOperandV1::Move(producer.clone()),
            &producer
        ));
        assert!(tile_operand(
            &types,
            &erased(producer.ty().index()),
            &producer
        ));
        assert!(!tile_operand(
            &types,
            &SemanticOperandV1::Copy(producer.clone()),
            &producer
        ));
    }
}

#[test]
fn transpose_footer_erased_tile_rejects_equal_layout_wrong_type() {
    let types = types();
    assert_eq!(types[0].layout(), types[1].layout());
    assert!(!tile_operand(&types, &erased(1), &place(3, 0)));
    assert!(!tile_operand(
        &types,
        &SemanticOperandV1::Move(place(3, 1)),
        &place(3, 0)
    ));
}

#[test]
fn transpose_footer_tile_rejects_wrong_local_projection_or_scalar_constant() {
    let types = types();
    let producer = place(3, 0);
    assert!(!tile_operand(
        &types,
        &SemanticOperandV1::Move(place(4, 0)),
        &producer
    ));
    let projected = SemanticPlaceV1::new(
        local(3),
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), id(0)).unwrap()],
        id(0),
    )
    .unwrap();
    assert!(!tile_operand(
        &types,
        &SemanticOperandV1::Move(projected),
        &producer
    ));
    let scalar = SemanticOperandV1::Constant(SemanticConstantV1::new(
        id(0),
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(0, 1).unwrap()),
    ));
    assert!(!tile_operand(&types, &scalar, &producer));
}

#[test]
fn transpose_footer_erased_tile_requires_sized_inhabited_unit_alignment() {
    for (size, alignment, uninhabited) in [
        (Some(1), 1, false),
        (None, 1, false),
        (Some(0), 2, false),
        (Some(0), 1, true),
    ] {
        let mut types = types();
        // Inert test-only declarations isolate each layout gate. Complete
        // request admission independently validates the whole type layout.
        types[0] = SemanticTypeDeclV1::new(
            types[0].identity(),
            types[0].layout_identity(),
            SemanticTypeLayoutV1::new_with_backend_repr(
                size,
                alignment,
                SemanticBackendReprV1::memory(size.is_some()),
                uninhabited,
            )
            .unwrap(),
            SemanticTypeShapeV1::Unit,
        );
        assert!(
            !tile_operand(&types, &erased(0), &place(3, 0)),
            "accepted size={size:?} alignment={alignment} uninhabited={uninhabited}"
        );
    }
    assert!(!tile_operand(&[], &erased(0), &place(3, 0)));
}

#[test]
fn transpose_footer_publish_workgroup_accepts_only_exact_whole_copy_or_move() {
    for operand in [
        SemanticOperandV1::Move(place(9, 1)),
        SemanticOperandV1::Copy(place(9, 1)),
    ] {
        assert!(workgroup_operand(&operand, local(9), id(1)));
        assert!(!workgroup_operand(&operand, local(8), id(1)));
        assert!(!workgroup_operand(&operand, local(9), id(0)));
    }
    assert!(!workgroup_operand(&erased(1), local(9), id(1)));
    let projected = SemanticPlaceV1::new(
        local(9),
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), id(1)).unwrap()],
        id(1),
    )
    .unwrap();
    for operand in [
        SemanticOperandV1::Copy(projected.clone()),
        SemanticOperandV1::Move(projected),
    ] {
        assert!(!workgroup_operand(&operand, local(9), id(1)));
    }
}

#[test]
fn transpose_footer_closure_handoff_still_requires_original_whole_move() {
    let closure = place(7, 1);
    assert!(moved(&SemanticOperandV1::Move(closure.clone()), local(7)));
    assert!(!moved(&SemanticOperandV1::Copy(closure), local(7)));
    assert!(!moved(&erased(1), local(7)));
}
