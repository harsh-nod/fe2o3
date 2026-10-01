//! Public inert constructors only; no admitted or authenticated owner is forged.
use fe2o3_mir_model::semantic_mir_v1::*;
use std::mem::size_of;

const TY: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
fn events(operand: &SemanticOperandV1) -> Vec<(usize, usize)> {
    let mut rows = Vec::new();
    operand
        .visit_retained_heap_storage_v1(|n, w| {
            rows.push((n, w));
            Ok::<_, ()>(())
        })
        .unwrap();
    rows
}
fn constant(value: SemanticConstantValueV1) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(TY, value))
}

#[test]
fn fixed_constants_visit_once_without_following_separate_referents() {
    for value in [
        SemanticConstantValueV1::ZeroSized,
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(9, 4).unwrap()),
        SemanticConstantValueV1::Pointer(SemanticPointerValueV1::new_with_metadata(
            7,
            SemanticPointerProvenanceV1::Allocation(SemanticAllocationIdV1::from_index(99)),
            SemanticPointerValueMetadataV1::VTable(SemanticVTableIdV1::from_index(8)),
        )),
        SemanticConstantValueV1::Callable(SemanticCallableIdV1::from_index(12)),
    ] {
        assert_eq!(events(&constant(value)), vec![(0, 1)]);
    }
}

#[test]
fn copy_and_move_charge_actual_box_payload_not_old_vec_capacity() {
    let projection = SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), TY).unwrap();
    let mut rows = Vec::with_capacity(19);
    rows.extend([projection, projection]);
    let place = SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), rows, TY).unwrap();
    assert_eq!(place.projections().len(), 2);
    for operand in [
        SemanticOperandV1::Copy(place.clone()),
        SemanticOperandV1::Move(place),
    ] {
        let before = operand.clone();
        assert_eq!(
            events(&operand),
            vec![(0, 1), (2, size_of::<SemanticProjectionV1>())]
        );
        assert_eq!(operand, before);
    }
}

#[test]
fn constant_bytes_use_owned_box_length_and_include_empty_box_visit() {
    let mut bytes = Vec::with_capacity(23);
    bytes.extend([4, 5, 6]);
    let operand = constant(SemanticConstantValueV1::Bytes(
        SemanticConstantBytesV1::new(bytes).unwrap(),
    ));
    assert_eq!(events(&operand), vec![(0, 1), (3, 1)]);
    assert_eq!(
        events(&constant(SemanticConstantValueV1::Bytes(
            SemanticConstantBytesV1::new(vec![]).unwrap()
        ))),
        vec![(0, 1), (0, 1)]
    );
    let empty = SemanticPlaceV1::new(SemanticLocalIdV1::from_index(0), vec![], TY).unwrap();
    assert_eq!(
        events(&SemanticOperandV1::Move(empty)),
        vec![(0, 1), (0, size_of::<SemanticProjectionV1>())]
    );
}

#[test]
fn each_callback_failure_is_returned_unchanged_without_a_later_visit() {
    let operand = constant(SemanticConstantValueV1::Bytes(
        SemanticConstantBytesV1::new(vec![1, 2]).unwrap(),
    ));
    for stop in [1, 2] {
        let mut calls = 0;
        let result = operand.visit_retained_heap_storage_v1(|_, _| {
            calls += 1;
            if calls == stop {
                Err("sentinel")
            } else {
                Ok(())
            }
        });
        assert_eq!(result, Err("sentinel"));
        assert_eq!(calls, stop);
    }
}

#[test]
fn external_consumer_preserves_a_shared_prefix_and_checked_arithmetic() {
    let operand = constant(SemanticConstantValueV1::Bytes(
        SemanticConstantBytesV1::new(vec![1]).unwrap(),
    ));
    let mut bytes = usize::MAX;
    let mut calls = 0;
    let result = operand.visit_retained_heap_storage_v1(|count, width| {
        calls += 1;
        let amount = count.checked_mul(width).ok_or("overflow")?;
        bytes = bytes.checked_add(amount).ok_or("overflow")?;
        Ok(())
    });
    assert_eq!(result, Err("overflow"));
    assert_eq!(calls, 2);
    assert_eq!(bytes, usize::MAX);
}
