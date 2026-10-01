use super::*;
use fe2o3_kernel_ir::LogicalStorageLimitsV1;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticFunctionIdV1, SemanticLocalIdV1, SemanticSourceProvenanceV1,
};

type Owners = (
    Box<[DebugSourceMapFileV1]>,
    Box<[RetainedDebugSourceScopeV2]>,
    Box<[RetainedDebugSourceVariableV2]>,
    Option<ProductionSemanticDebugProducerGapV1>,
);

fn counter(bytes: Option<usize>, items: usize) -> LogicalStorageCounterV1 {
    LogicalStorageCounterV1::new(LogicalStorageLimitsV1 {
        max_bytes: bytes,
        max_items: items,
    })
}

fn walk(owners: &Owners, c: &mut LogicalStorageCounterV1) -> Result<(), LogicalStorageErrorV1> {
    charge_retained_debug_sources_v1(&owners.0, &owners.1, &owners.2, &owners.3, c)
}

/// Inert row-shape fixture, not an authenticated bindings constructor.
fn owners() -> (Owners, usize) {
    let mut path = String::with_capacity(149);
    path.push_str("source.rs");
    let path_capacity = path.capacity();
    let file = DebugSourceMapFileV1::new([1; 32], 32, path).unwrap();
    let scope = RetainedDebugSourceScopeV2 {
        identity: [2; 32],
        function: SemanticFunctionIdV1::from_index(0),
        parent_identity: None,
        depth: 0,
        source: SemanticSourceProvenanceV1::unavailable(),
    };
    let mut name = String::with_capacity(211);
    name.push_str("same");
    let mut other_name = String::with_capacity(307);
    other_name.push_str("same");
    let row = |name, class| RetainedDebugSourceVariableV2 {
        identity: [3; 32],
        function: scope.function,
        name,
        scope_identity: scope.identity,
        class,
        entry_value_preserved: true,
    };
    let variables = vec![
        row(
            Some(name),
            RetainedDebugSourceVariableClassV2::Local(SemanticLocalIdV1::from_index(0)),
        ),
        row(
            Some(other_name),
            RetainedDebugSourceVariableClassV2::Unrepresented,
        ),
        row(None, RetainedDebugSourceVariableClassV2::Unrepresented),
    ]
    .into_boxed_slice();
    (
        (
            vec![file].into_boxed_slice(),
            vec![scope].into_boxed_slice(),
            variables,
            None,
        ),
        path_capacity,
    )
}

fn expected_bytes(owners: &Owners, path_capacity: usize) -> usize {
    owners.0.len() * size_of::<DebugSourceMapFileV1>()
        + owners.1.len() * size_of::<RetainedDebugSourceScopeV2>()
        + owners.2.len() * size_of::<RetainedDebugSourceVariableV2>()
        + path_capacity
        + owners
            .2
            .iter()
            .map(|v| v.name.as_ref().map_or(0, String::capacity))
            .sum::<usize>()
}

fn expected_items(owners: &Owners) -> usize {
    4 + owners.0.len() * 2
        + owners.1.len()
        + owners.2.len()
        + owners.2.iter().filter(|v| v.name.is_some()).count()
}

#[test]
fn exact_actual_box_payloads_and_distinct_name_capacities_preserve_enclosing_prefix() {
    let (owners, path_capacity) = owners();
    let bytes = expected_bytes(&owners, path_capacity);
    let items = expected_items(&owners);
    let mut c = counter(Some(29 + bytes), 3 + items);
    c.charge(29, 3).unwrap();
    walk(&owners, &mut c).unwrap();
    assert_eq!((c.bytes(), c.items()), (29 + bytes, 3 + items));
    assert_eq!(owners.0[0].display_path(), "source.rs");
    assert_eq!(owners.2[0].name.as_deref(), Some("same"));
    assert_eq!(owners.2[1].name.as_deref(), Some("same"));
    assert!(path_capacity > owners.0[0].display_path().len());
    assert!(owners.2[0].name.as_ref().unwrap().capacity() > 4);
    assert!(owners.2[1].name.as_ref().unwrap().capacity() > 4);
}

#[test]
fn empty_arrays_and_every_explicit_capture_gap_own_no_nested_payload() {
    use ProductionSemanticDebugProducerGapV1::*;
    let gaps = [
        None,
        Some(MultipleKirFunctionBodies),
        Some(NoStatementCorrespondence),
        Some(SourceMapUnavailable),
        Some(ResourceLimit),
        Some(CanonicalKirV7ProjectionUnavailable),
        Some(SourceObservationUnrepresentable),
        Some(SemanticMapConstructionUnavailable),
        Some(SemanticMapEncodingUnavailable),
        Some(FragmentConstructionUnavailable),
        Some(CarrierConstructionUnavailable),
        Some(ReceiptExtensionConstructionUnavailable),
        Some(CorrespondenceValidationUnavailable),
        Some(CanonicalKirModuleMismatch),
        Some(LegacyBareAssociationNoAttachment),
    ];
    for gap in gaps {
        let empty: Owners = (Box::default(), Box::default(), Box::default(), gap);
        let mut c = counter(Some(0), 4);
        walk(&empty, &mut c).unwrap();
        assert_eq!((c.bytes(), c.items()), (0, 4));
    }
}

#[test]
fn present_empty_names_charge_actual_capacity_and_a_visit_but_none_does_not() {
    let (mut owners, path_capacity) = owners();
    owners.2[0].name = Some(String::new());
    owners.2[1].name = Some(String::with_capacity(97));
    let mut c = counter(None, 100);
    walk(&owners, &mut c).unwrap();
    assert_eq!(c.bytes(), expected_bytes(&owners, path_capacity));
    assert_eq!(c.items(), expected_items(&owners));
    let before = (c.bytes(), c.items());
    let spare = owners.2[1].name.as_ref().unwrap().capacity();
    owners.2[0].name = None;
    owners.2[1].name = None;
    let mut without = counter(None, 100);
    walk(&owners, &mut without).unwrap();
    assert_eq!(
        (without.bytes(), without.items()),
        (before.0 - spare, before.1 - 2)
    );
}

#[test]
fn exhausted_item_prefixes_refuse_before_unbounded_row_work() {
    let (owners, _) = owners();
    let total_items = expected_items(&owners);
    for allowance in 0..total_items {
        let mut c = counter(None, 2 + allowance);
        c.charge(13, 2).unwrap();
        assert_eq!(walk(&owners, &mut c), Err(LogicalStorageErrorV1::ItemLimit));
        assert_eq!(c.items(), 2 + allowance);
        assert!(c.bytes() >= 13);
    }
}

#[test]
fn one_under_byte_budget_refuses_and_preserves_only_a_discardable_prefix() {
    let (owners, path_capacity) = owners();
    let bytes = expected_bytes(&owners, path_capacity);
    let mut c = counter(Some(19 + bytes - 1), 100);
    c.charge(19, 2).unwrap();
    assert_eq!(walk(&owners, &mut c), Err(LogicalStorageErrorV1::ByteLimit));
    assert!(c.bytes() >= 19 && c.bytes() < 19 + bytes);
    assert_eq!(owners.2[0].name.as_deref(), Some("same"));
}

#[test]
fn shared_counter_addition_overflow_refuses_without_large_allocations() {
    let (owners, _) = owners();
    let mut bytes = counter(None, usize::MAX);
    bytes.charge(usize::MAX, 0).unwrap();
    assert_eq!(
        walk(&owners, &mut bytes),
        Err(LogicalStorageErrorV1::Arithmetic)
    );
    assert_eq!((bytes.bytes(), bytes.items()), (usize::MAX, 1));
    let mut items = counter(None, usize::MAX);
    items.charge(0, usize::MAX).unwrap();
    assert_eq!(
        walk(&owners, &mut items),
        Err(LogicalStorageErrorV1::Arithmetic)
    );
    assert_eq!((items.bytes(), items.items()), (0, usize::MAX));
}

#[test]
fn authenticated_wrapper_is_type_checked_without_fabricating_custody() {
    let _: fn(
        &AuthenticatedProductionBindings,
        &mut LogicalStorageCounterV1,
    ) -> Result<(), LogicalStorageErrorV1> =
        AuthenticatedProductionBindings::charge_debug_source_retained_heap_v1;
}
