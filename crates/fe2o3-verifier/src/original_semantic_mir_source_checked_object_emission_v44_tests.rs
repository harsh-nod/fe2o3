use super::*;

#[test]
fn retained_checked_event_headers_include_the_original_object_destination_frame() {
    #[allow(dead_code)]
    enum DestinationFields {
        Local(usize),
        Object { access: Access, offsets: [u64; 2] },
    }
    type CheckedFields = (
        DestinationFields,
        TypeId,
        Value,
        Value,
        ScalarV30,
        SemanticCheckedBinaryOpV1,
    );
    type PlaceFields = (usize, TypeId, TypeId, usize, usize);
    type TransferFields = (PlaceFields, PlaceFields, bool);
    assert_eq!(size_of::<Checked>(), size_of::<CheckedFields>());
    assert_eq!(size_of::<AggregatePlace>(), size_of::<PlaceFields>());
    assert_eq!(size_of::<Transfer>(), size_of::<TransferFields>());
    assert_eq!(
        size_of::<CheckedDestination>(),
        size_of::<DestinationFields>()
    );
    assert_eq!(
        headers(),
        size_of::<CheckedFields>()
            + 2 * size_of::<Result<Option<CheckedFields>>>()
            + size_of::<PlaceFields>()
            + 2 * size_of::<Result<Option<PlaceFields>>>()
            + size_of::<TransferFields>()
            + 2 * size_of::<Result<Option<TransferFields>>>()
            + 2 * size_of::<Value>()
            + size_of::<DestinationFields>()
            + size_of::<[u64; 2]>()
            + size_of::<&[u64]>()
            + 4 * size_of::<TypeId>()
            + 2 * size_of::<super::super::super::slots::SourceAggregateLeafV42<'_, '_, '_>>()
            + 8 * size_of::<usize>()
            + 6 * size_of::<&()>()
    );
}

#[test]
fn retained_checked_event_emission_has_an_independent_exact_resource_oracle() {
    use crate::mixed_optimizer_refinement_v26::SOURCE_LIMIT;
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrWorkBudgetV1 as Work,
    };
    for (operation, code) in [
        (SemanticCheckedBinaryOpV1::Add, 0),
        (SemanticCheckedBinaryOpV1::Subtract, 1),
        (SemanticCheckedBinaryOpV1::Multiply, 2),
    ] {
        for signed in [false, true] {
            let event = Checked {
                destination: CheckedDestination::Object {
                    access: Access {
                        address: Address::Object {
                            local: 4,
                            offset: 0,
                        },
                        ty: TypeId::from_index(2),
                        bytes: 8,
                        alignment: 4,
                    },
                    offsets: [0, 4],
                },
                source_type: TypeId::from_index(2),
                left: Value::Constant(7),
                right: Value::Constant(3),
                scalar: ScalarV30::Integer { signed, width: 32 },
                operation,
            };
            let expected = format!(
                "InvocationSourceByteEventV36::CheckedObject(InvocationSourceCheckedObjectV44 {{ access: InvocationSourceByteAccessV36 {{ base: InvocationSourceByteBaseV36::ObjectLocal(4int), offset: 0int, width: 8int, alignment: 4int }}, source_type: 2int, value_offset: 0int, overflow_offset: 4int, operation: {code}int, bits: 32int, signed: {signed}, left: InvocationSourceByteValueV36::Constant(7int), right: InvocationSourceByteValueV36::Constant(3int) }})"
            );
            // Event, object access, and the two original scalar values.
            let work = 4 + expected.len();
            let storage = SOURCE_LIMIT + headers();
            let emit = |work_limit, storage_limit| {
                let mut ledger = Work::new(work_limit);
                let mut budget = Budget::new(&mut ledger, storage_limit);
                let result = (|| {
                    budget.reserve_storage(SOURCE_LIMIT + headers())?;
                    let mut out = Writer::new(&mut budget)?;
                    event.emit(&mut out)?;
                    out.finish()
                })();
                (result, budget.work(), budget.peak_storage())
            };
            let exact = emit(work, storage);
            assert_eq!(exact.0.unwrap(), expected);
            assert_eq!((exact.1, exact.2), (work, storage));
            assert!(matches!(emit(work - 1, storage).0,
                Err(Error::Resource(Resource::Work(error)))
                    if error.actual() == work && error.limit() == work - 1));
            assert!(matches!(emit(work, storage - 1).0,
                Err(Error::Resource(Resource::Storage(error)))
                    if error.actual() == storage && error.limit() == storage - 1));
        }
    }
}
