//! Reuse the original SYNTHETIC custody fixture and original bind/seal paths.
//! These are actual owned allocations, not rustc-authenticated provider evidence.
use super::*;
use fe2o3_kernel_ir::{LogicalStorageCounterV1, LogicalStorageErrorV1, LogicalStorageLimitsV1};
use std::mem::size_of;

fn counter(bytes: Option<usize>, items: usize) -> LogicalStorageCounterV1 {
    LogicalStorageCounterV1::new(LogicalStorageLimitsV1 {
        max_bytes: bytes,
        max_items: items,
    })
}
fn entry_expectation(owner: &RetainedContextEntriesV29) -> (usize, usize) {
    let mut bytes = owner.entries.capacity() * size_of::<RetainedContextEntryV29>();
    let mut items = 2; // retained root plus Vec
    for entry in &owner.entries {
        bytes += entry.source.arguments.capacity() * size_of::<SemanticOperandV1>();
        items += 2; // initialized entry plus argument Vec
        for operand in &entry.source.arguments {
            items += 1; // operand's zero-byte visit
            match operand {
                SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => {
                    bytes += place.projections().len() * size_of::<SemanticProjectionV1>();
                    items += 1;
                }
                SemanticOperandV1::Constant(value) => match value.value() {
                    SemanticConstantValueV1::Bytes(value) => {
                        bytes += value.as_bytes().len();
                        items += 1;
                    }
                    SemanticConstantValueV1::ZeroSized
                    | SemanticConstantValueV1::Scalar(_)
                    | SemanticConstantValueV1::Pointer(_)
                    | SemanticConstantValueV1::Callable(_) => {}
                },
            }
        }
    }
    (bytes, items)
}

#[test]
fn original_synthetic_bind_and_seal_retains_actual_spare_entry_and_argument_slots() {
    let semantic = fixture_roots(Mutation::None, 2);
    let mut owner = sealed_roots(&semantic);
    owner.entries.reserve(7);
    for entry in &mut owner.entries {
        entry.source.arguments.reserve(5);
    }
    assert!(owner.scopes.is_none());
    owner.validate_source(&semantic).unwrap();
    let (bytes, items) = entry_expectation(&owner);
    let header = size_of::<RetainedContextEntriesV29>();
    let mut c = counter(Some(header + bytes), items + 1);
    c.charge(header, 1).unwrap();
    owner.charge_retained_heap_storage_v1(&mut c).unwrap();
    assert_eq!((c.bytes(), c.items()), (header + bytes, items + 1));
    owner.validate_source(&semantic).unwrap();
}

#[test]
fn original_synthetic_scope_constructor_is_composed_and_not_silently_zeroed() {
    let semantic = fixture_roots(Mutation::None, 2);
    let owner = scope_tests::complete(&semantic);
    let scopes = owner.scopes.as_ref().unwrap();
    assert!(!scopes.classes().is_empty());
    assert!(!scopes.events().is_empty());
    // Scope actual-capacity arithmetic is independently controlled in its
    // owning module. Here verify this real optional child is composed once.
    let mut child = counter(None, 3);
    scopes.charge_retained_heap_storage_v1(&mut child).unwrap();
    assert!(child.bytes() > 0);
    assert_eq!(child.items(), 3);
    let (entry_bytes, entry_items) = entry_expectation(&owner);
    let mut c = counter(
        Some(entry_bytes + child.bytes()),
        entry_items + child.items(),
    );
    owner.charge_retained_heap_storage_v1(&mut c).unwrap();
    assert_eq!(
        (c.bytes(), c.items()),
        (entry_bytes + child.bytes(), entry_items + 3)
    );
    owner.validate_source(&semantic).unwrap();
}

#[test]
fn sealed_owner_observation_preserves_exact_shared_limits_and_first_refusal() {
    let semantic = fixture(Mutation::None);
    let owner = sealed_roots(&semantic);
    let (bytes, items) = entry_expectation(&owner);
    for limit in 0..items {
        let mut c = counter(None, limit);
        assert_eq!(
            owner.charge_retained_heap_storage_v1(&mut c),
            Err(LogicalStorageErrorV1::ItemLimit)
        );
        assert_eq!(c.items(), limit);
    }
    let mut short = counter(Some(bytes - 1), items);
    assert_eq!(
        owner.charge_retained_heap_storage_v1(&mut short),
        Err(LogicalStorageErrorV1::ByteLimit)
    );
    assert!(short.bytes() < bytes);
    let mut exact = counter(Some(bytes + 11), items + 2);
    exact.charge(11, 2).unwrap();
    owner.charge_retained_heap_storage_v1(&mut exact).unwrap();
    assert_eq!((exact.bytes(), exact.items()), (bytes + 11, items + 2));
    owner.validate_source(&semantic).unwrap();
}

#[test]
fn original_zero_issue_seal_preserves_empty_entry_vec_backing() {
    // Reuse the existing ordinary-root admission shape, without synthesizing
    // any CompletedContextEntry or authenticated production bindings owner.
    let original = fixture(Mutation::None);
    let root = &original.functions()[0];
    let ordinary = SemanticFunctionDeclV1::new(
        root.identity(),
        root.role(),
        root.item_definition_identity(),
        root.monomorphization_identity(),
        root.generic_type_arguments_identity(),
        root.const_generic_arguments_identity(),
        root.source(),
        root.abi().clone(),
        root.locals()[..3].to_vec(),
        SemanticBlockIdV1::from_index(0),
        vec![block(
            10,
            vec![unit_return()],
            SemanticTerminatorKindV1::Return,
        )],
    )
    .unwrap()
    .with_kernel_entry(root.kernel_entry().unwrap().clone());
    let semantic = InertSemanticMirRequestV1::new(
        original.target(),
        original.types()[..2].to_vec(),
        vec![],
        vec![],
        vec![],
        vec![ordinary],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    let entries = Vec::<RetainedContextEntryV29>::with_capacity(9);
    let expected = entries.capacity() * size_of::<RetainedContextEntryV29>();
    let owner = RetainedContextEntriesV29::seal(entries, &semantic, |_| Ok(())).unwrap();
    assert!(owner.entries.is_empty());
    assert!(owner.scopes.is_none());
    let mut c = counter(Some(expected), 2);
    owner.charge_retained_heap_storage_v1(&mut c).unwrap();
    assert_eq!((c.bytes(), c.items()), (expected, 2));
    owner.validate_source(&semantic).unwrap();
}
