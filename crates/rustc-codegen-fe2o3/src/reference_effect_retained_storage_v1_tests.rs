//! Borrowed public inert payloads only; never fabricate authenticated bindings.
use super::*;
use fe2o3_kernel_ir::{LogicalStorageErrorV1, LogicalStorageLimitsV1};
use fe2o3_mir_model::semantic_mir_v1::{SemanticExternAbiV1, SemanticFunctionSafetyV1};
use fe2o3_verifier::portable_reference_v1::signature::*;
use fe2o3_verifier::portable_reference_v1::*;
use std::mem::size_of;

fn counter(bytes: Option<usize>, items: usize) -> LogicalStorageCounterV1 {
    LogicalStorageCounterV1::new(LogicalStorageLimitsV1 {
        max_bytes: bytes,
        max_items: items,
    })
}
fn signature() -> ReferenceLogicalSignaturePreimageV1 {
    ReferenceLogicalSignaturePreimageV1::new(
        Box::default(),
        Box::default(),
        ReferenceReturnShapeV1::Unit,
        SemanticExternAbiV1::Rust,
        SemanticFunctionSafetyV1::Safe,
        false,
    )
    .unwrap()
}
fn output() -> ReferenceOutputWriteV1 {
    ReferenceOutputWriteV1 {
        argument: 0,
        block: 0,
        statement: 0,
        coordinate: ReferenceOutputCoordinateV1::SingleCoordinate,
        guard: ReferencePathPredicateV1::unconditional_v1(),
        rhs: ReferenceEffectExpressionV1::Unary {
            operation: ReferenceUnaryOpV1::Not,
            operand: Box::new(ReferenceEffectExpressionV1::PointCoordinate { axis: 0 }),
        },
        value: ReferenceValueV1::InputLength {
            reference_argument: 0,
        },
    }
}
fn ir() -> ReferenceEffectIrV1 {
    ReferenceEffectIrV1 {
        argument_count: 0,
        local_count: 0,
        relations: Box::default(),
        blocks: Box::default(),
        loop_summaries: Box::default(),
        observable_output_effects: vec![output()].into_boxed_slice(),
    }
}
fn strings() -> (String, String) {
    let mut registration = String::with_capacity(37);
    registration.push_str("fixture::reference");
    let mut name = String::with_capacity(41);
    name.push_str("fixture");
    (registration, name)
}
fn owned_output_bytes() -> usize {
    size_of::<ReferenceOutputWriteV1>()
        + size_of::<ReferenceGuardClauseV1>()
        + size_of::<ReferenceEffectExpressionV1>()
}

#[test]
fn actual_string_spare_capacity_and_both_deep_output_copies_are_counted() {
    let (registration, name) = strings();
    let signature = signature();
    let ir = ir();
    let writes = ir.observable_output_effects.clone();
    assert_ne!(ir.observable_output_effects.as_ptr(), writes.as_ptr());
    let expected = registration.capacity() + name.capacity() + 2 * owned_output_bytes();
    let mut c = counter(Some(expected), 100);
    charge_payload(&registration, &name, &signature, &ir, &writes, &mut c).unwrap();
    assert_eq!(c.bytes(), expected);
    assert_eq!(ir.observable_output_effects, writes);
}
#[test]
fn payload_first_and_late_refusals_preserve_the_original_counter() {
    let (registration, name) = strings();
    let signature = signature();
    let ir = ir();
    let writes = ir.observable_output_effects.clone();
    let expected = registration.capacity() + name.capacity() + 2 * owned_output_bytes();
    let mut full = counter(Some(expected), 100);
    charge_payload(&registration, &name, &signature, &ir, &writes, &mut full).unwrap();
    for limit in [0, full.items() - 1] {
        let mut c = counter(None, limit);
        assert_eq!(
            charge_payload(&registration, &name, &signature, &ir, &writes, &mut c),
            Err(ReferenceRetainedStorageErrorV1::Counter(
                LogicalStorageErrorV1::ItemLimit
            ))
        );
        assert_eq!(c.items(), limit);
    }
    let mut c = counter(Some(expected - 1), 100);
    assert_eq!(
        charge_payload(&registration, &name, &signature, &ir, &writes, &mut c),
        Err(ReferenceRetainedStorageErrorV1::Counter(
            LogicalStorageErrorV1::ByteLimit
        ))
    );
    let mut c = counter(None, usize::MAX);
    c.charge(usize::MAX, 5).unwrap();
    assert_eq!(
        charge_payload(&registration, &name, &signature, &ir, &writes, &mut c),
        Err(ReferenceRetainedStorageErrorV1::Counter(
            LogicalStorageErrorV1::Arithmetic
        ))
    );
    assert_eq!((c.bytes(), c.items()), (usize::MAX, 5));
}
#[test]
fn separate_write_copy_depth_refusal_is_propagated_not_zeroed() {
    let mut write = output();
    let mut rhs = ReferenceEffectExpressionV1::PointCoordinate { axis: 0 };
    for _ in 0..128 {
        rhs = ReferenceEffectExpressionV1::Unary {
            operation: ReferenceUnaryOpV1::Not,
            operand: Box::new(rhs),
        };
    }
    write.rhs = rhs;
    let writes = vec![write].into_boxed_slice();
    let empty_ir = ReferenceEffectIrV1 {
        argument_count: 0,
        local_count: 0,
        relations: Box::default(),
        blocks: Box::default(),
        loop_summaries: Box::default(),
        observable_output_effects: Box::default(),
    };
    let mut c = counter(None, 1000);
    assert_eq!(
        charge_payload(
            &String::new(),
            &String::new(),
            &signature(),
            &empty_ir,
            &writes,
            &mut c
        ),
        Err(ReferenceRetainedStorageErrorV1::ExpressionDepthLimit)
    );
}
#[test]
fn authenticated_binding_method_is_type_checked_without_an_owner_fixture() {
    let _: fn(
        &AuthenticatedReferenceEffectBindingsV1,
        &mut LogicalStorageCounterV1,
    ) -> Result<(), ReferenceRetainedStorageErrorV1> =
        AuthenticatedReferenceEffectBindingsV1::charge_retained_heap_storage_v1;
}
