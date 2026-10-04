use super::tests::{Fixture, PRIOR, PROFILES, S, SIBLING, W, Work, free};
use super::*;
use fe2o3_kernel_analysis::CanonicalKirInventoryV1 as Inventory;
use fe2o3_kernel_descriptor::{DESCRIPTOR_QUERY_STORAGE_V3, decode_device_descriptor_table_v3};

struct Oracle {
    work: usize,
    peak: usize,
    peak_work: usize,
}

// Independently compose existing public child costs and the new one-root
// wrapper's explicit operations. No call to the composite checker, producer,
// roots/sort helpers, requirement/physical private implementation, or scope.
fn oracle(f: &Fixture, table: &Table<'_>) -> Oracle {
    let floor = f.floor();
    let mut work = Work::new(W);
    let mut b = Budget::new(&mut work, S);
    b.reserve_storage(floor).unwrap();
    let (inventory, ir) = Inventory::derive(&f.owner, &mut b).unwrap();
    let inventory_work = b.work();
    let inventory_peak = b.peak_storage() - floor;
    let inventory_storage = ir.retained_storage();
    b.reserve_storage(inventory_storage).unwrap();
    let before = b.work();
    let (binding, br) =
        check_kernel_ir_contract_catalog_v1(&inventory, &f.catalog, &mut b).unwrap();
    let binding_work = b.work() - before;
    b.reserve_storage(br.retained_storage()).unwrap();
    let binding_peak = b.peak_storage() - floor;
    drop(binding);
    drop(inventory);
    drop(b);

    let profile_work = 36 + f.profile.device_target().len();
    let mut work = Work::new(W);
    let mut b = Budget::new(&mut work, S);
    b.reserve_storage(floor + DESCRIPTOR_QUERY_STORAGE_V3)
        .unwrap();
    let row = table.kernel(0, &mut |n| b.charge_work(n)).unwrap();
    let query_work = b.work();
    assert_eq!(table.kernel_count(), 1);
    assert_eq!(row.entry_name(), "kernel");
    let name = row.entry_name().len();
    let roots_work = 17 + 5 * name + query_work;
    // One-element Vec capacities are observed independently, not assumed len.
    let root_capacity = Vec::<(usize, usize)>::with_capacity(1).capacity();
    let name_capacity = Vec::<(usize, &str, &str)>::with_capacity(1).capacity();
    let index_capacity = Vec::<usize>::with_capacity(1).capacity();
    let roots_storage =
        size_of::<Vec<(usize, usize)>>() + root_capacity * size_of::<(usize, usize)>();
    let roots_peak = roots_storage
        + DESCRIPTOR_QUERY_STORAGE_V3
        + size_of::<Vec<(usize, &str, &str)>>()
        + name_capacity * size_of::<(usize, &str, &str)>()
        + size_of::<Vec<usize>>()
        + index_capacity * size_of::<usize>();
    drop(b);

    let mut work = Work::new(W);
    let mut b = Budget::new(&mut work, S);
    b.reserve_storage(floor).unwrap();
    let physical =
        crate::check_canonical_v12_descriptor_physical_abi_v3(&f.owner, f.profile, table, &mut b)
            .unwrap();
    let physical_work = b.work() - profile_work - inventory_work - roots_work - 1;
    drop(physical);
    drop(b);
    let mut work = Work::new(W);
    let mut b = Budget::new(&mut work, S);
    b.reserve_storage(floor).unwrap();
    let requirements =
        crate::check_canonical_v12_descriptor_requirements_v3(&f.owner, f.profile, table, &mut b)
            .unwrap();
    let requirements_work = b.work() - profile_work - inventory_work - roots_work - 1;
    drop(requirements);
    drop(b);

    let guard = 2 * size_of::<usize>()
        + size_of::<CanonicalKernelIrWorkLedgerIdentityV1>()
        + size_of::<[Option<Box<dyn Any + Send>>; 2]>()
        + size_of::<R<ReplayedNativeV12TextDescriptorRelationV3<'_, '_, '_, '_, '_>>>();
    let prefix_work = 2 + 2 * f.owner.canonical().canonical_bytes().len() + 2 + profile_work;
    let before_engines = PRIOR
        + prefix_work
        + inventory_work
        + binding_work
        + roots_work
        + physical_work
        + requirements_work;
    let raw = match f.profile {
        Profile::Gfx942 => lower_942(&f.owner),
        Profile::Gfx950 => lower_950(&f.owner),
    }
    .unwrap();
    let raw_storage = size_of::<String>() + raw.capacity();
    let engine_floor = floor + guard + inventory_storage + roots_storage;
    let first_engine = engine_floor + size_of::<String>() + MAX_COMPILER_MODULE_TEXT_BYTES;
    let second_engine = engine_floor
        + raw_storage
        + size_of::<String>()
        + MAX_PRODUCTION_SEMANTIC_ANCHOR_LLVM_TEXT_BYTES_V1;
    let functions = f.owner.module().functions.len();
    let bindings_capacity = Vec::<(bool, bool)>::with_capacity(functions).capacity();
    let requirements_storage = size_of::<Vec<(bool, bool)>>()
        + bindings_capacity * size_of::<(bool, bool)>()
        + DESCRIPTOR_QUERY_STORAGE_V3
        + size_of::<fe2o3_kernel_descriptor::KernelTargetRequirementsV2>()
        + size_of::<fe2o3_kernel_descriptor::KernelDescriptorRefV3<'_, '_>>()
        + size_of::<fe2o3_kernel_descriptor::CapabilityCursorV3<'_, '_>>();
    let other_peak = [
        floor + guard + inventory_peak,
        floor + guard + binding_peak,
        engine_floor - roots_storage + roots_peak,
        engine_floor + 4 * DESCRIPTOR_QUERY_STORAGE_V3,
        engine_floor + requirements_storage,
        engine_floor + size_of::<String>() + f.prefix.capacity() + size_of::<(&[u8], usize)>() + 4,
        engine_floor + size_of::<ReplayedNativeV12TextDescriptorRelationV3<'_, '_, '_, '_, '_>>(),
    ]
    .into_iter()
    .max()
    .unwrap();
    assert!(
        first_engine.max(second_engine) > other_peak,
        "fixture peak is a known independent engine reservation"
    );
    Oracle {
        work: before_engines + 2 + 2 + 2 * f.text.len() + 1,
        peak: first_engine.max(second_engine),
        peak_work: before_engines + if first_engine >= second_engine { 1 } else { 2 },
    }
}

fn resource(error: &(dyn Error + 'static)) -> Option<Resource> {
    if let Some(e) = error.downcast_ref::<Resource>() {
        return Some(*e);
    }
    error.source().and_then(resource)
}
fn attempt(
    f: &Fixture,
    work_limit: usize,
    storage_limit: usize,
) -> (R<()>, usize, usize, Option<usize>) {
    let table = decode_device_descriptor_table_v3(&f.wire, &mut free).unwrap();
    let mut work = Work::new(work_limit);
    let mut b = Budget::new(&mut work, storage_limit);
    b.reserve_storage(f.floor()).unwrap();
    b.charge_work(PRIOR).unwrap();
    let result = check_native_v12_text_descriptor_relation_v3(
        &f.owner,
        &f.catalog,
        f.owner.canonical().canonical_bytes(),
        f.profile,
        &table,
        &f.text,
        &mut b,
    )
    .and_then(|relation| {
        assert_eq!(
            relation.storage().retained_storage(),
            size_of::<ReplayedNativeV12TextDescriptorRelationV3<'_, '_, '_, '_, '_>>()
        );
        let retained = relation.storage().retained_storage();
        b.reserve_storage(retained)?;
        drop(relation);
        b.release_storage(retained)?;
        Ok(())
    });
    assert_eq!(b.storage(), f.floor());
    (result, b.work(), b.peak_storage(), b.failed_storage())
}

#[test]
fn nominal_native_v3_independent_composition_exact_work_peak_and_interior_prefix() {
    for profile in PROFILES {
        let f = Fixture::new(profile);
        let table = decode_device_descriptor_table_v3(&f.wire, &mut free).unwrap();
        let expected = oracle(&f, &table);
        let (result, work, peak, failed) = attempt(&f, expected.work, expected.peak);
        result.unwrap();
        assert_eq!((work, peak, failed), (expected.work, expected.peak, None));
        let (result, work, _, _) = attempt(&f, expected.work - 1, expected.peak);
        let error = result.unwrap_err();
        assert!(matches!(resource(&error), Some(Resource::Work(_))));
        assert_eq!(work, expected.work - 1);
        let (result, work, peak, failed) = attempt(&f, expected.work, expected.peak - 1);
        let error = result.unwrap_err();
        let Some(Resource::Storage(limit)) = resource(&error) else {
            panic!("exact typed storage failure")
        };
        assert_eq!(
            (limit.actual(), limit.limit()),
            (expected.peak, expected.peak - 1)
        );
        assert_eq!((work, failed), (expected.peak_work, Some(expected.peak)));
        assert!(peak < expected.peak);
        // Failure in an earlier exact byte comparison must not refund accepted
        // work, nor poison a later fresh ledger over the same retained inputs.
        let cutoff = PRIOR + 2;
        let (result, work, _, _) = attempt(&f, cutoff, expected.peak);
        assert!(matches!(
            resource(&result.unwrap_err()),
            Some(Resource::Work(_))
        ));
        assert_eq!(work, cutoff);
        attempt(&f, expected.work, expected.peak).0.unwrap();
    }
}

#[test]
fn nominal_native_v3_vector_overflow_and_every_suffix_query_boundary_restore_floor() {
    let mut work = Work::new(W);
    let mut b = Budget::new(&mut work, S);
    b.reserve_storage(SIBLING).unwrap();
    assert!(matches!(
        scoped(&mut b, |b| vector::<u64>(usize::MAX, b)),
        Err(E::Resource(Resource::Arithmetic))
    ));
    assert_eq!(b.storage(), SIBLING);
    for length in [0usize, 1, 15, 16, 17, 255] {
        let bytes = vec![0xab; length];
        let suffix = super::tests::suffix(&bytes);
        let prefix = b"native\n";
        let text = [prefix.as_slice(), suffix.as_bytes()].concat();
        let expected_work = 2 + 2 * text.len();
        let expected_storage =
            SIBLING + SCOPE_STORAGE + size_of::<R<()>>() + size_of::<(&[u8], usize)>() + 4;
        for (w, s, accept) in [
            (expected_work, expected_storage, true),
            (expected_work - 1, expected_storage, false),
            (expected_work, expected_storage - 1, false),
        ] {
            let mut work = Work::new(w);
            let mut b = Budget::new(&mut work, s);
            b.reserve_storage(SIBLING).unwrap();
            let result = scoped(&mut b, |b| compare_text(prefix, &bytes, &text, b));
            assert_eq!(result.is_ok(), accept);
            assert_eq!(b.storage(), SIBLING);
            if accept {
                assert_eq!(
                    (b.work(), b.peak_storage()),
                    (expected_work, expected_storage)
                );
            }
        }
    }
}

#[test]
fn nominal_native_v3_panic_error_released_guard_and_foreign_ledger_cleanup() {
    let mut work = Work::new(W);
    let mut foreign_work = Work::new(W);
    let mut b = Budget::new(&mut work, S);
    b.reserve_storage(SIBLING).unwrap();
    b.charge_work(PRIOR).unwrap();
    assert!(matches!(
        scoped::<()>(&mut b, |b| {
            b.reserve_storage(99)?;
            panic!("inert callback panic")
        }),
        Err(E::Panicked)
    ));
    assert_eq!(b.storage(), SIBLING);
    assert!(matches!(
        scoped::<()>(&mut b, |b| {
            b.reserve_storage(99)?;
            Err(E::Invalid("injected"))
        }),
        Err(E::Invalid("injected"))
    ));
    assert_eq!(b.storage(), SIBLING);
    struct Bomb;
    impl Drop for Bomb {
        fn drop(&mut self) {
            panic!("rejected result destructor")
        }
    }
    assert!(matches!(
        scoped(&mut b, |b| {
            b.release_storage(1)?;
            Ok(Bomb)
        }),
        Err(E::Resource(Resource::Accounting))
    ));
    assert_eq!(b.storage(), SIBLING);
    let mut foreign = Budget::new(&mut foreign_work, S);
    foreign.reserve_storage(SIBLING + 3).unwrap();
    foreign.charge_work(29).unwrap();
    let identity = foreign.work_ledger_identity_v1();
    let result = scoped::<()>(&mut b, |b| {
        std::mem::swap(b, &mut foreign);
        Ok(())
    });
    assert!(matches!(result, Err(E::Resource(Resource::Accounting))));
    assert!(b.work_ledger_identity_v1() == identity);
    assert_eq!((b.storage(), b.work()), (SIBLING + 3, 29));
    assert_eq!(
        (foreign.storage(), foreign.work()),
        (SIBLING + SCOPE_STORAGE + size_of::<R<()>>(), PRIOR)
    );
    std::mem::swap(&mut b, &mut foreign);
    b.release_storage(SCOPE_STORAGE + size_of::<R<()>>())
        .unwrap();
    scoped(&mut b, |b| b.charge_work(1).map_err(E::Resource)).unwrap();
    assert_eq!((b.storage(), b.work()), (SIBLING, PRIOR + 1));
}

#[test]
fn nominal_native_v3_panic_payload_destructor_unwinds_only_after_original_cleanup() {
    struct Bomb;
    impl Drop for Bomb {
        fn drop(&mut self) {
            panic!("panic payload destructor")
        }
    }
    let mut work = Work::new(W);
    let mut b = Budget::new(&mut work, S);
    b.reserve_storage(SIBLING).unwrap();
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _: R<()> = scoped(&mut b, |b| {
                b.reserve_storage(71)?;
                std::panic::panic_any(Bomb)
            });
        }))
        .is_err()
    );
    assert_eq!(b.storage(), SIBLING);
    scoped(&mut b, |_| Ok(())).unwrap();
}
