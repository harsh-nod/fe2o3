use super::*;
use common::queries::TableQuery;
use std::mem::size_of_val;

fn resource(error: &(dyn Error + 'static)) -> Option<Resource> {
    if let Some(error) = error.downcast_ref::<Resource>() {
        return Some(*error);
    }
    error.source().and_then(resource)
}

fn attempt(
    fixture: &Fixture,
    work_limit: usize,
    storage_limit: usize,
) -> (
    Result<(), NativeV12TextDescriptorReplayErrorV5>,
    usize,
    usize,
    Option<usize>,
) {
    let table = decode_device_descriptor_table_v5(&fixture.wire, &mut free).unwrap();
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    budget.reserve_storage(fixture.floor()).unwrap();
    budget.charge_work(PRIOR).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let result = fixture.check(&table, &fixture.text, &mut budget);
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert_eq!(budget.storage(), fixture.floor());
    (
        result,
        budget.work(),
        budget.peak_storage(),
        budget.failed_storage(),
    )
}

#[test]
fn conditional_native_v5_exact_and_one_short_total_limits_preserve_original_floor() {
    for profile in PROFILES {
        let fixture = Fixture::new(profile, 1);
        let (result, work, peak, denied) = attempt(&fixture, W, S);
        result.unwrap();
        assert_eq!(denied, None);
        let exact = attempt(&fixture, work, peak);
        exact.0.unwrap();
        assert_eq!((exact.1, exact.2, exact.3), (work, peak, None));
        let short = attempt(&fixture, work - 1, peak);
        assert!(matches!(
            resource(&short.0.unwrap_err()),
            Some(Resource::Work(_))
        ));
        let short = attempt(&fixture, work, peak - 1);
        assert!(matches!(
            resource(&short.0.unwrap_err()),
            Some(Resource::Storage(_))
        ));
        assert_eq!(short.3, Some(peak));
    }
}

#[test]
fn conditional_native_v5_existing_denials_are_not_cleared_on_a_successful_relation() {
    let fixture = Fixture::new(Profile::Gfx942, 1);
    fixture.run(|table, budget| {
        assert!(budget.charge_work(W).is_err());
        assert!(budget.reserve_storage(S).is_err());
        let work_denial = budget.failed_work();
        let storage_denial = budget.failed_storage();
        fixture.check(table, &fixture.text, budget).unwrap();
        assert_eq!(budget.failed_work(), work_denial);
        assert_eq!(budget.failed_storage(), storage_denial);
    });
}

#[test]
fn conditional_native_v5_suffix_has_independent_exact_work_and_scratch_boundaries() {
    for length in [0usize, 1, 15, 16, 17, 255] {
        let bytes = vec![0xab; length];
        let prefix = b"native\n";
        let suffix = suffix(&bytes);
        assert_eq!(
            suffix.len(),
            SECTION.len() + 6 * length + 18 * length.div_ceil(16)
        );
        let text = [prefix.as_slice(), suffix.as_bytes()].concat();
        let expected_work = 2 + 2 * text.len();
        let expected_storage = SIBLING
            + common::SCOPE_STORAGE
            + size_of::<common::R<()>>()
            + size_of::<(&[u8], usize)>()
            + 4;
        for (work, storage, accept) in [
            (expected_work, expected_storage, true),
            (expected_work - 1, expected_storage, false),
            (expected_work, expected_storage - 1, false),
        ] {
            let mut work = Work::new(work);
            let mut budget = Budget::new(&mut work, storage);
            budget.reserve_storage(SIBLING).unwrap();
            let result = common::scoped(&mut budget, |budget| {
                common::compare_text_for(prefix, &bytes, &text, SECTION, budget)
            });
            assert_eq!(result.is_ok(), accept);
            assert_eq!(budget.storage(), SIBLING);
            if accept {
                assert_eq!(
                    (budget.work(), budget.peak_storage()),
                    (expected_work, expected_storage)
                );
            }
        }
    }
}

#[test]
fn conditional_native_v5_suffix_checks_every_byte_without_tolerating_an_extra_section() {
    for length in [1usize, 15, 16, 17] {
        let bytes = vec![0xa9; length];
        let prefix = b"native\n";
        let text = [prefix.as_slice(), suffix(&bytes).as_bytes()].concat();
        let mut work = Work::new(W);
        let mut budget = Budget::new(&mut work, S);
        budget.reserve_storage(SIBLING).unwrap();
        for index in 0..text.len() {
            let mut changed = text.clone();
            changed[index] ^= 1;
            assert!(
                common::scoped(&mut budget, |budget| {
                    common::compare_text_for(prefix, &bytes, &changed, SECTION, budget)
                })
                .is_err()
            );
            assert_eq!(budget.storage(), SIBLING);
        }
    }
}

#[test]
fn conditional_native_v5_shared_scope_keeps_foreign_account_and_unwind_fail_closed() {
    let mut work = Work::new(W);
    let mut foreign_work = Work::new(W);
    let mut budget = Budget::new(&mut work, S);
    let mut foreign = Budget::new(&mut foreign_work, S);
    budget.reserve_storage(SIBLING).unwrap();
    foreign.reserve_storage(SIBLING + 7).unwrap();
    budget.charge_work(PRIOR).unwrap();
    foreign.charge_work(29).unwrap();
    let identity = foreign.work_ledger_identity_v1();
    let result = common::scoped::<()>(&mut budget, |budget| {
        std::mem::swap(budget, &mut foreign);
        Ok(())
    })
    .map_err(NativeV12TextDescriptorReplayErrorV5);
    assert!(matches!(
        resource(&result.unwrap_err()),
        Some(Resource::Accounting)
    ));
    assert!(budget.work_ledger_identity_v1() == identity);
    assert_eq!((budget.work(), budget.storage()), (29, SIBLING + 7));
    std::mem::swap(&mut budget, &mut foreign);
    budget
        .release_storage(common::SCOPE_STORAGE + size_of::<common::R<()>>())
        .unwrap();
    let result = common::scoped::<()>(&mut budget, |budget| {
        budget.reserve_storage(99)?;
        panic!("inert native V5 scope");
    })
    .map_err(NativeV12TextDescriptorReplayErrorV5);
    assert!(matches!(
        result,
        Err(NativeV12TextDescriptorReplayErrorV5(common::E::Panicked))
    ));
    assert_eq!((budget.storage(), budget.work()), (SIBLING, PRIOR));
    let result = common::scoped(&mut budget, |budget| {
        budget.release_storage(1)?;
        Ok(())
    })
    .map_err(NativeV12TextDescriptorReplayErrorV5);
    assert!(matches!(
        resource(&result.unwrap_err()),
        Some(Resource::Accounting)
    ));
    assert_eq!(budget.storage(), SIBLING);
}

#[test]
fn conditional_native_v5_private_query_sharing_preserves_v3_row_sizes_and_debits() {
    assert_eq!(
        <DeviceDescriptorTableV3<'_> as TableQuery<'_>>::QUERY_STORAGE,
        DESCRIPTOR_QUERY_STORAGE_V3
    );
    assert_eq!(
        <Table<'_> as TableQuery<'_>>::QUERY_STORAGE,
        DESCRIPTOR_QUERY_STORAGE_V5
    );
    for profile in PROFILES {
        let fixture = legacy::Fixture::new(profile);
        fixture.run(|table, budget| {
            let before = budget.work();
            let row = table.kernel(0, &mut |w| budget.charge_work(w)).unwrap();
            let direct = budget.work() - before;
            let size = size_of_val(&row);
            drop(row);
            let before = budget.work();
            let row: KernelDescriptorRefV3<'_, '_> = TableQuery::kernel(table, 0, budget).unwrap();
            assert_eq!(budget.work() - before, direct);
            assert_eq!(size_of_val(&row), size);
            assert_eq!(row.entry_name(), "kernel");
        });
    }
}
