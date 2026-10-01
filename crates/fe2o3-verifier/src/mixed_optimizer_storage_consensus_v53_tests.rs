use super::*;
use fe2o3_kernel_ir::{
    StorageLayoutIdV1, StorageLayoutKindV1, StorageLayoutV1, StorageOperationV1 as Storage,
};

fn storage_diamond(scalar: ScalarType) -> Module {
    let mut module = diamond(scalar);
    let bytes = u32::from(scalar.bit_width().unwrap() / 8);
    module.storage_layouts.push(StorageLayoutV1 {
        size: u64::from(bytes),
        alignment: bytes,
        kind: StorageLayoutKindV1::Scalar(scalar),
    });
    for block in &mut module.functions[0].body.as_mut().unwrap().blocks {
        for operation in &mut block.operations {
            operation.kind = match operation.kind {
                OperationKind::Alloca {
                    count,
                    address_space,
                    alignment,
                    ..
                } => {
                    let element = Type::StorageObject(StorageLayoutIdV1(0));
                    operation.results[0].ty = Type::pointer(
                        element.clone(),
                        AddressSpace::Private,
                        AccessMode::ReadWrite,
                    );
                    OperationKind::Alloca {
                        element,
                        count,
                        address_space,
                        alignment,
                    }
                }
                OperationKind::Load { pointer, .. } => OperationKind::Storage(Storage::ReadValue {
                    address: pointer,
                    access: MemoryAccess::new(AddressSpace::Private, 1),
                }),
                OperationKind::Store {
                    pointer,
                    value,
                    access,
                } => OperationKind::Storage(Storage::WriteValue {
                    address: pointer,
                    value,
                    access,
                }),
                _ => panic!("exact scalar storage fixture"),
            };
        }
    }
    module
}

fn emit_pair(
    before: &Inventory<'_>,
    after: &Inventory<'_>,
    pair: &Pair<'_>,
    bp: &Physical<'_, '_>,
    ap: &Physical<'_, '_>,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    let source = FixtureAllocations(before);
    let first = target_view_contracts_v38::TargetByteViewContractsV38::derive(
        before,
        FormalIndexWidth::Bits64,
        out,
    )?;
    let last = target_view_contracts_v38::TargetByteViewContractsV38::derive(
        after,
        FormalIndexWidth::Bits64,
        out,
    )?;
    let paid = out.budget.storage();
    assert_eq!(
        generate(
            before,
            after,
            pair,
            bp,
            ap,
            &first,
            &last,
            FormalIndexWidth::Bits64,
            1,
            &source,
            out
        )?,
        1
    );
    assert_eq!(out.budget.storage(), paid);
    Ok(())
}

#[test]
fn typed_store_consensus_storage_cells_keep_all_integer_widths_and_actual_read_alignment() {
    for scalar in [
        ScalarType::I8,
        ScalarType::U8,
        ScalarType::I16,
        ScalarType::U16,
        ScalarType::I32,
        ScalarType::U32,
        ScalarType::I64,
        ScalarType::U64,
    ] {
        with_pair(
            &storage_diamond(scalar),
            |before, after, pair, bp, ap, floor| {
                let text = run(floor, LIMIT, LIMIT, |out| {
                    emit_pair(before, after, pair, bp, ap, out)
                })
                .0
                .unwrap();
                assert!(text.contains("forwarding_operation_3_v46"));
                assert!(text.contains("alignment == 1 && value == observation.after.values["));
                assert!(text.contains("forwarding_store_fact_v46"));
                assert!(text.contains("forwarding_finite_trace_0_v46"));
                assert!(!text.contains("assume("));
                assert!(!text.contains("external_body"));
            },
        );
    }
}

#[test]
fn typed_store_consensus_storage_route_has_exact_work_and_storage_boundaries() {
    with_pair(
        &storage_diamond(ScalarType::U64),
        |before, after, pair, bp, ap, floor| {
            let emit = |out: &mut Writer<'_, '_>| emit_pair(before, after, pair, bp, ap, out);
            let measured = run(floor, LIMIT, LIMIT, emit);
            let text = measured.0.unwrap();
            let exact = run(floor, measured.1, measured.2, emit);
            assert_eq!(exact.0.unwrap(), text);
            assert_eq!((exact.1, exact.2), (measured.1, measured.2));
            assert!(matches!(run(floor, measured.1 - 1, measured.2, emit).0,
            Err(Error::Resource(Resource::Work(error))) if error.actual() == measured.1 && error.limit() == measured.1 - 1));
            assert!(matches!(run(floor, measured.1, measured.2 - 1, emit).0,
            Err(Error::Resource(Resource::Storage(error))) if error.actual() == measured.2 && error.limit() == measured.2 - 1));
        },
    );
}

#[test]
fn typed_store_consensus_storage_missing_arm_and_volatile_read_are_not_forwarded() {
    let mut missing = storage_diamond(ScalarType::I32);
    missing.functions[0].body.as_mut().unwrap().blocks[2]
        .operations
        .clear();
    assert_load_not_erased(missing);
    let mut volatile = storage_diamond(ScalarType::U32);
    let OperationKind::Storage(Storage::ReadValue { ref mut access, .. }) =
        volatile.functions[0].body.as_mut().unwrap().blocks[3].operations[0].kind
    else {
        unreachable!()
    };
    access.volatile = true;
    assert_load_not_erased(volatile);
}
