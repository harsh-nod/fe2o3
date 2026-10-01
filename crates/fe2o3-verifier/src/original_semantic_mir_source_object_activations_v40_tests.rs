use super::*;

const LIMIT: usize = 100_000_000;

fn run(
    explicit: bool,
    work: usize,
    storage: usize,
    examine: impl FnOnce(
        &SourceSlots<'_, '_>,
        &InvocationPlan<'_, '_>,
        &mut Writer<'_, '_>,
    ) -> Result<()>,
) -> (Result<()>, usize, usize, usize) {
    let execute = |plan: &mut InvocationPlan<'_, '_>, out: &mut Writer<'_, '_>| {
        let source = plan.source(out)?;
        let owner = source.canonical(out.budget)?;
        let (inventory, receipt) =
            crate::mixed_optimizer_refinement_v26::semantics::Inventory::derive_v18(
                owner, out.budget,
            )?;
        out.budget.reserve_storage(receipt.retained_storage())?;
        let result =
            source.with_ranked_correspondence_v18(&inventory, out.budget, |relation, budget| {
                let mut writer = Writer::new(budget)?;
                let slots = SourceSlots::derive(plan, relation, &mut writer)?;
                examine(&slots, plan, &mut writer)
            });
        drop(inventory);
        if result.is_ok() {
            out.budget.release_storage(receipt.retained_storage())?;
        }
        result
    };
    if explicit {
        super::super::super::super::invocations::tests::run_scalar_lifetime_variant(
            work, storage, execute,
        )
    } else {
        super::super::super::super::invocations::tests::run_allocation_variant(
            work, storage, execute,
        )
    }
}

fn check(
    explicit: bool,
    slots: &SourceSlots<'_, '_>,
    plan: &InvocationPlan<'_, '_>,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    let source = plan.source(out)?.source_semantic(out.budget)?;
    assert_eq!(slots.objects.rows.len(), 2, "one original object per root");
    for root in 0..2 {
        let instance = plan.instance(root, 0, out)?;
        let original = &source.functions()[instance.function.index() as usize];
        let row = slots
            .objects
            .rows
            .iter()
            .find(|row| row.key[0] == root)
            .unwrap();
        assert_eq!(&row.key[..3], &[root, 0, 4]);
        assert_eq!(row.flat_local, instance.locals.start + 4);
        assert_eq!(row.ty, original.locals()[4].ty());
        let frame = slots.frames[row.descriptor].as_ref().unwrap();
        assert_eq!(frame.semantic_type(), row.ty);
        assert_eq!(
            (frame.root(), frame.instance(), frame.local()),
            (root, 0, 4)
        );
        if explicit {
            let mut next = 1;
            let mut expected = None;
            for (block, body) in original.blocks().iter().enumerate() {
                for (statement, item) in body.statements().iter().enumerate() {
                    if matches!(item.kind(), Statement::StorageLive(local) if local.index() == 4) {
                        assert!(expected.is_none());
                        expected = Some((next + statement, block, statement));
                    }
                }
                next += body.statements().len();
            }
            let (generation, block, statement) = expected.expect("genuine original StorageLive");
            assert_eq!(row.key[3], generation);
            assert!(
                matches!(row.origin, Activation::StorageLive { block: found, statement: at }
                if found.index() as usize == block && at == statement)
            );
        } else {
            assert_eq!((row.key[3], row.origin), (0, Activation::Entry));
        }
        assert_eq!(
            slots.object_activation(root, 0, 4, row.key[3] as u32, out)?,
            Some(*row)
        );
        assert!(slots.has_original_object(root, 0, 4, out)?);
        assert!(!slots.has_original_object(root, 0, 1, out)?);
        assert!(
            slots
                .object_activation(root, 0, 4, u32::MAX, out)?
                .is_none()
        );
    }
    slots.emit(out)?;
    assert_eq!(
        out.text
            .matches("fn invocation_source_object_binding_v40(")
            .count(),
        1
    );
    assert_eq!(out.text.matches("object.activation == ").count(), 2);
    for row in &slots.objects.rows {
        let expected = format!(
            "local == {}int && object.descriptor == {}int && object.activation == {}int && object.slot == invocation_source_slot_{}_v36()",
            row.flat_local, row.descriptor, row.key[3], row.descriptor,
        );
        assert_eq!(out.text.matches(&expected).count(), 1);
    }
    Ok(())
}

#[test]
fn original_object_activation_index_authenticates_entry_and_explicit_source_sites() {
    for explicit in [false, true] {
        run(explicit, LIMIT, LIMIT, |slots, plan, out| {
            check(explicit, slots, plan, out)
        })
        .0
        .unwrap();
    }
}

#[test]
fn original_object_activation_queries_are_paid_logarithmic_and_allocate_nothing() {
    run(false, LIMIT, LIMIT, |slots, _, out| {
        let before = (out.budget.work(), out.budget.storage());
        for root in 0..2 {
            let (work, storage) = (out.budget.work(), out.budget.storage());
            for _ in 0..64 {
                assert!(slots.objects.activation(root, 0, 4, 0, out)?.is_some());
            }
            // Two-element lower_bound probes twice for either present key,
            // followed by the final exact-key comparison.
            assert_eq!(out.budget.work() - work, 64 * 3);
            assert_eq!(out.budget.storage(), storage);
        }
        assert_eq!(out.budget.work() - before.0, 2 * 64 * 3);
        assert_eq!(out.budget.storage(), before.1);
        Ok(())
    })
    .0
    .unwrap();
}

#[test]
fn original_object_activation_custody_failure_stays_latched_after_restore() {
    let result = run(true, LIMIT, LIMIT, |slots, _, out| {
        let storage = out.budget.storage();
        let debit = storage - slots.required + 1;
        out.budget.release_storage(debit)?;
        assert!(matches!(
            slots.has_original_object(0, 0, 4, out),
            Err(Error::Source(SourceError::Resource(Resource::Accounting)))
        ));
        out.budget.reserve_storage(debit)?;
        let work = out.budget.work();
        assert!(slots.object_activation(0, 0, 4, 0, out).is_err());
        assert_eq!(out.budget.work(), work);
        slots.emit(out)
    });
    assert!(matches!(
        result.0,
        Err(Error::Source(SourceError::Resource(Resource::Accounting)))
    ));
}

#[test]
fn original_object_activation_emission_has_exact_and_one_short_resources() {
    let execute = |work, storage| {
        run(true, work, storage, |slots, plan, out| {
            check(true, slots, plan, out)
        })
    };
    let measured = execute(LIMIT, LIMIT);
    measured.0.unwrap();
    let exact = execute(measured.1, measured.3);
    exact.0.unwrap();
    assert_eq!(
        (exact.1, exact.2, exact.3),
        (measured.1, measured.2, measured.3)
    );
    assert!(matches!(execute(measured.1 - 1, measured.3).0,
        Err(Error::Source(SourceError::Resource(Resource::Work(error))))
            if error.actual() == measured.1 && error.limit() == measured.1 - 1));
    assert!(matches!(execute(measured.1, measured.3 - 1).0,
        Err(Error::Source(SourceError::Resource(Resource::Storage(error))))
            if error.actual() == measured.3 && error.limit() == measured.3 - 1));
}

#[test]
fn original_object_activation_index_headers_cover_exact_retained_fields() {
    type Fields = ([usize; 4], usize, usize, TypeId, Activation);
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T>>()
    }
    assert_eq!(size_of::<ObjectActivation>(), size_of::<Fields>());
    assert_eq!(
        size_of::<SourceObjects>(),
        size_of::<Vec<ObjectActivation>>()
    );
    assert_eq!(
        headers(),
        h::<SourceObjects>()
            + h::<Vec<ObjectActivation>>()
            + size_of::<Fields>()
            + 2 * size_of::<Result<ObjectActivation>>()
            + h::<Activation>()
            + h::<[usize; 4]>()
            + h::<Option<ObjectActivation>>()
            + h::<fe2o3_lower_mir_kernel::ProductionSourceObjectLifetimeV40<'_, '_>>()
            + h::<(usize, usize, Local, u32, TypeId)>()
            + h::<(u32, Activation)>()
            + h::<Operation>()
            + h::<&Frame>()
            + h::<std::ops::Range<usize>>()
            + 30 * size_of::<usize>()
            + 12 * size_of::<&()>()
    );
}
