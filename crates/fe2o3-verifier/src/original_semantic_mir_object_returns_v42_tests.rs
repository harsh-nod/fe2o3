use super::*;
use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18 as SourceError;

const LIMIT: usize = 256 * 1024 * 1024;

fn run(
    work: usize,
    storage: usize,
    examine: impl FnOnce(&ObjectReturnsV42<'_, '_, '_>, &mut Writer<'_, '_>) -> Result<()>,
) -> (Result<()>, usize, usize, usize) {
    super::super::super::super::invocations::tests::run_source_transform(
        work,
        storage,
        |types, functions| {
            super::super::aggregate_tests::retained_call_transform(types, functions, true, true)
        },
        |plan, out| {
            super::super::super::source_function::tests::with_slots(plan, out, |slots, out| {
                let index = ObjectReturnsV42::derive(slots, FormalIndexWidth::Bits64, out)?;
                examine(&index, out)
            })
        },
    )
}

fn examine(index: &ObjectReturnsV42<'_, '_, '_>, out: &mut Writer<'_, '_>) -> Result<()> {
    assert_eq!(index.rows.len(), 4);
    let inventory = index.slots.correspondence(out)?.inventory(out.budget)?;
    for (ordinal, row) in index.rows.iter().enumerate() {
        assert_eq!(row.key, (ordinal / 2, 0, (ordinal % 2) as u32));
        assert_eq!(row.local, 4);
        assert_eq!(row.ty, TypeId::from_index(0));
        let actual = &inventory.operations()[row.operation];
        assert!(matches!(
            actual.operation.kind,
            OperationKind::Storage(StorageOperationV1::WriteValue { .. })
        ));
        let uses = &inventory.uses()[actual.operands.clone()];
        assert_eq!(uses.len(), 2);
        assert_eq!(row.definition, uses[1].definition);
        assert_eq!(
            index.definition(row.key.0, row.key.1, row.key.2, row.local, row.ty, out)?,
            row.definition
        );
    }
    Ok(())
}

#[test]
fn original_object_return_index_binds_exact_store_inputs_and_rejects_foreign_coordinates() {
    run(LIMIT, LIMIT, |index, out| {
        examine(index, out)?;
        let row = index.rows[0];
        for (root, instance, block, local, ty) in [
            (2, 0, 0, 4, row.ty),
            (0, 1, 0, 4, row.ty),
            (0, 0, u32::MAX, 4, row.ty),
            (0, 0, 0, 3, row.ty),
            (0, 0, 0, 4, TypeId::from_index(1)),
        ] {
            assert!(matches!(
                index.definition(root, instance, block, local, ty, out),
                Err(Error::Statement(_))
            ));
        }
        assert_eq!(index.definition(0, 0, 0, 4, row.ty, out)?, row.definition);
        Ok(())
    })
    .0
    .unwrap();
}

#[test]
fn original_object_return_index_latches_undercut_even_after_credit_is_restored() {
    let result = run(LIMIT, LIMIT, |index, out| {
        let row = index.rows[0];
        let debit = out.budget.storage() - index.required + 1;
        out.budget.release_storage(debit)?;
        assert!(matches!(
            index.definition(0, 0, 0, 4, row.ty, out),
            Err(Error::Source(SourceError::Resource(Resource::Accounting)))
        ));
        out.budget.reserve_storage(debit)?;
        let before = (out.budget.work(), out.budget.storage(), out.text.len());
        let result = index.definition(0, 0, 0, 4, row.ty, out);
        assert!(matches!(
            result,
            Err(Error::Source(SourceError::Resource(Resource::Accounting)))
        ));
        assert_eq!(
            (out.budget.work(), out.budget.storage(), out.text.len()),
            before
        );
        result.map(|_| ())
    });
    assert!(matches!(
        result.0,
        Err(Error::Source(SourceError::Resource(Resource::Accounting)))
    ));
}

#[test]
fn original_object_return_index_rejects_funded_foreign_ledgers_before_work() {
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrWorkBudgetV1 as Work,
    };
    let result = run(LIMIT, LIMIT, |index, out| {
        let row = index.rows[0];
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(out.budget.storage())?;
        let before = (budget.work(), budget.storage(), budget.peak_storage());
        {
            let mut foreign = Writer::new(&mut budget)?;
            assert!(matches!(
                index.definition(0, 0, 0, 4, row.ty, &mut foreign),
                Err(Error::Source(SourceError::Resource(Resource::Accounting)))
            ));
            assert!(foreign.text.is_empty());
        }
        assert_eq!(
            (budget.work(), budget.storage(), budget.peak_storage()),
            before
        );
        let before = (out.budget.work(), out.budget.storage(), out.text.len());
        let result = index.definition(0, 0, 0, 4, row.ty, out);
        assert_eq!(
            (out.budget.work(), out.budget.storage(), out.text.len()),
            before
        );
        result.map(|_| ())
    });
    assert!(matches!(
        result.0,
        Err(Error::Source(SourceError::Resource(Resource::Accounting)))
    ));
}

#[test]
fn original_object_return_index_complete_transaction_has_exact_resource_boundaries() {
    let measured = run(LIMIT, LIMIT, examine);
    measured.0.unwrap();
    let exact = run(measured.1, measured.3, examine);
    exact.0.unwrap();
    assert_eq!(
        (exact.1, exact.2, exact.3),
        (measured.1, measured.2, measured.3)
    );
    assert!(matches!(run(measured.1 - 1, measured.3, examine).0,
        Err(Error::Source(SourceError::Resource(Resource::Work(error))))
        if error.limit() == measured.1 - 1 && error.actual() == measured.1));
    assert!(matches!(run(measured.1, measured.3 - 1, examine).0,
        Err(Error::Source(SourceError::Resource(Resource::Storage(error))))
        if error.limit() == measured.3 - 1 && error.actual() == measured.3));
}

#[test]
fn original_object_return_index_headers_are_independent() {
    type RowFields = ((usize, usize, u32), u32, TypeId, usize, usize);
    type IndexFields<'a> = (&'a SourceSlots<'a, 'a>, Vec<Row>, usize);
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T>>()
    }
    assert_eq!(size_of::<Row>(), size_of::<RowFields>());
    assert_eq!(
        size_of::<ObjectReturnsV42<'_, '_, '_>>(),
        size_of::<IndexFields<'_>>()
    );
    assert_eq!(
        headers(),
        h::<IndexFields<'_>>()
            + h::<RowFields>()
            + h::<Vec<RowFields>>()
            + h::<fe2o3_lower_mir_kernel::ProductionSourceObjectRecipeV39<'_, '_>>()
            + h::<fe2o3_lower_mir_kernel::ProductionSourceObjectEndpointV39<'_, '_>>()
            + 24 * size_of::<usize>()
            + 18 * size_of::<&()>()
    );
}
