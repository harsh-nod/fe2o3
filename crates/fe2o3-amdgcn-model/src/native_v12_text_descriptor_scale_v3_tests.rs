use super::*;
mod contract {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/support/nominal_descriptor_scale_v3.rs"
    ));
}

struct ScaleFixture {
    contract: contract::Contract,
    owner: Owner,
    owner_storage: usize,
    catalog: Catalog,
    catalog_storage: usize,
    wire: Vec<u8>,
    prefix: String,
    text: String,
    profile: Profile,
}
impl ScaleFixture {
    fn new(profile: Profile) -> Self {
        let contract = contract::Contract::standard();
        let bound = crate::bind_production_target_v1(&contract.module(false), profile).unwrap();
        let (owner, owner_storage) = admit(bound.module());
        assert_eq!(
            owner
                .module()
                .kernels
                .iter()
                .map(|k| k.id.as_str())
                .collect::<Vec<_>>(),
            contract::GRAPH_ORDER
        );
        assert_eq!(
            owner
                .module()
                .functions
                .iter()
                .map(|f| f.id.as_str())
                .collect::<Vec<_>>(),
            ["maple_impl", "amber_impl", "zebra_impl"]
        );
        let mut work = Work::new(W);
        let mut budget = Budget::new(&mut work, S);
        let (catalog, receipt) =
            Catalog::from_rows_with_budget([1; 32], &[], &[], &mut budget).unwrap();
        let raw = match profile {
            Profile::Gfx942 => lower_942(&owner),
            Profile::Gfx950 => lower_950(&owner),
        }
        .unwrap();
        let prefix = bind_production_llvm22_worker_layout_v1(&raw).unwrap();
        contract.check_native_signatures(&prefix);
        let wire = contract.wire(profile.device_target()).unwrap();
        let text = format!("{prefix}{}", contract::suffix(&wire));
        contract::check_suffix(&prefix, &wire, &text);
        Self {
            contract,
            owner,
            owner_storage,
            catalog,
            catalog_storage: receipt.retained_storage(),
            wire,
            prefix,
            text,
            profile,
        }
    }
    fn floor(&self) -> usize {
        self.owner_storage
            + self.catalog_storage
            + size_of::<Vec<u8>>()
            + self.wire.capacity()
            + 2 * size_of::<String>()
            + self.prefix.capacity()
            + self.text.capacity()
            + DESCRIPTOR_TABLE_VIEW_STORAGE_V3
            + SIBLING
    }
    fn run<T>(&self, run: impl FnOnce(&Table<'_>, &mut Budget<'_>) -> T) -> T {
        let mut work = Work::new(W);
        let mut budget = Budget::new(&mut work, S);
        budget.reserve_storage(self.floor()).unwrap();
        budget.charge_work(PRIOR).unwrap();
        let table = decode_device_descriptor_table_v3(&self.wire, &mut free).unwrap();
        let result = run(&table, &mut budget);
        drop(table);
        assert_eq!(budget.storage(), self.floor());
        result
    }
}

#[test]
fn nominal_native_v3_nonlexical_three_root_pairs_and_maximum_arguments_both_profiles() {
    for profile in PROFILES {
        let f = ScaleFixture::new(profile);
        f.run(|table, budget| {
            let (inventory, receipt) = Inventory::derive(&f.owner, budget).unwrap();
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            scoped(budget, |b| {
                let observed = roots(&inventory, table, b)?;
                assert_eq!(observed.len(), 3);
                for (row, &(name, kernel, descriptor)) in
                    observed.iter().zip(&contract::LEXICAL_PAIRS)
                {
                    assert_eq!((row.kernel, row.descriptor), (kernel, descriptor));
                    assert_eq!(f.owner.module().kernels[row.kernel].id.as_str(), name);
                }
                drop(observed);
                Ok(())
            })
            .unwrap();
            drop(inventory);
            budget.release_storage(receipt.retained_storage()).unwrap();
            let relation = check_native_v12_text_descriptor_relation_v3(
                &f.owner,
                &f.catalog,
                f.owner.canonical().canonical_bytes(),
                profile,
                table,
                &f.text,
                budget,
            )
            .unwrap();
            let retained = relation.storage().retained_storage();
            budget.reserve_storage(retained).unwrap();
            assert!(!relation.grants_authority());
            assert!(std::ptr::eq(relation.output(), &f.owner));
            assert_eq!(relation.profile(), profile);
            assert_eq!(relation.pre_descriptor_llvm(), f.prefix);
            assert_eq!(relation.final_llvm(), f.text);
            drop(relation);
            budget.release_storage(retained).unwrap();
        });
    }
}

#[test]
fn nominal_native_v3_maximum_layout_components_and_argument_boundary_are_independent() {
    for profile in PROFILES {
        let f = ScaleFixture::new(profile);
        f.run(|table, budget| {
            f.contract.check_table(table, budget);
            let physical = crate::check_canonical_v12_descriptor_physical_abi_v3(
                &f.owner, profile, table, budget,
            )
            .unwrap();
            let paid = physical.retained_storage();
            budget.reserve_storage(paid).unwrap();
            assert!(!physical.grants_source_ownership());
            drop(physical);
            budget.release_storage(paid).unwrap();
            let requirements = crate::check_canonical_v12_descriptor_requirements_v3(
                &f.owner, profile, table, budget,
            )
            .unwrap();
            let paid = requirements.retained_storage();
            budget.reserve_storage(paid).unwrap();
            assert_eq!(requirements.profile(), profile);
            assert!(!requirements.grants_authority());
            drop(requirements);
            budget.release_storage(paid).unwrap();
        });
        let mut excessive = contract::Contract::standard();
        excessive.roots[0].arguments.push(contract::Argument {
            shape: contract::Shape::U8,
            offset: 640,
        });
        excessive.roots[0].explicit = 648;
        excessive.roots[0].components = 81;
        assert!(matches!(
            excessive.wire(profile.device_target()),
            Err(DescriptorWireErrorV3::Decode(
                DecodeError::CountOutOfRange {
                    field: "kernel arguments",
                    count: 65,
                    max: 64
                }
            ))
        ));
    }
}

#[test]
fn nominal_native_v3_nonlexical_root_and_maximum_argument_donors_refuse() {
    for profile in PROFILES {
        let f = ScaleFixture::new(profile);
        for wrong_root in [true, false] {
            let mut donor = contract::Contract::standard();
            if wrong_root {
                donor.roots[0].entry = "wrong";
            } else {
                donor.roots[0].arguments[32].shape = contract::Shape::U64;
            }
            let wire = donor.wire(profile.device_target()).unwrap();
            f.run(|_, b| {
                let paid =
                    size_of::<Vec<u8>>() + wire.capacity() + DESCRIPTOR_TABLE_VIEW_STORAGE_V3;
                b.reserve_storage(paid).unwrap();
                let table = decode_device_descriptor_table_v3(&wire, &mut free).unwrap();
                let result = check_native_v12_text_descriptor_relation_v3(
                    &f.owner,
                    &f.catalog,
                    f.owner.canonical().canonical_bytes(),
                    profile,
                    &table,
                    &f.text,
                    b,
                );
                if wrong_root {
                    assert!(matches!(
                        &result,
                        Err(E::Invalid("descriptor entry bijection"))
                    ));
                } else {
                    assert!(matches!(
                        &result,
                        Err(E::Physical {
                            root: 2,
                            argument: Some(32),
                            ..
                        })
                    ));
                }
                drop(result);
                drop(table);
                b.release_storage(paid).unwrap();
            });
        }
    }
}

struct Expected {
    work: usize,
    peak: usize,
    peak_work: usize,
    prior_peak: usize,
}

fn reference_vector<T>(count: usize) -> Vec<T> {
    let mut rows = Vec::new();
    rows.try_reserve_exact(count).unwrap();
    rows
}
fn vector_extent<T>(rows: &Vec<T>) -> usize {
    size_of::<Vec<T>>() + rows.capacity() * size_of::<T>()
}

// Independent successful public children plus a fixed three-element sorting
// trace. No observed composite, private roots/sort/check, or failed run supplies
// an expected total. Reference capacities come from separate allocations.
fn expected(f: &ScaleFixture, table: &Table<'_>) -> Expected {
    let floor = f.floor();
    let guard = 2 * size_of::<usize>()
        + size_of::<CanonicalKernelIrWorkLedgerIdentityV1>()
        + size_of::<[Option<Box<dyn Any + Send>>; 2]>()
        + size_of::<R<ReplayedNativeV12TextDescriptorRelationV3<'_, '_, '_, '_, '_>>>();
    let mut work = Work::new(W);
    let mut b = Budget::new(&mut work, S);
    b.reserve_storage(floor).unwrap();
    let (inventory, receipt) = Inventory::derive(&f.owner, &mut b).unwrap();
    let inventory_work = b.work();
    let inventory_storage = receipt.retained_storage();
    b.reserve_storage(inventory_storage).unwrap();
    let inventory_peak = b.peak_storage() - floor;
    let start = b.work();
    let (binding, receipt) =
        check_kernel_ir_contract_catalog_v1(&inventory, &f.catalog, &mut b).unwrap();
    let binding_work = b.work() - start;
    b.reserve_storage(receipt.retained_storage()).unwrap();
    let binding_peak = b.peak_storage() - floor;
    drop(binding);
    b.release_storage(receipt.retained_storage()).unwrap();
    drop(inventory);
    b.release_storage(inventory_storage).unwrap();
    assert_eq!(b.storage(), floor);
    drop(b);

    let profile_work = 36 + f.profile.device_target().len();
    let mut work = Work::new(W);
    let mut b = Budget::new(&mut work, S);
    b.reserve_storage(floor + DESCRIPTOR_QUERY_STORAGE_V3)
        .unwrap();
    for (index, expected) in contract::DESCRIPTOR_ORDER.iter().enumerate() {
        let row = table.kernel(index, &mut |n| b.charge_work(n)).unwrap();
        assert_eq!(row.entry_name(), *expected);
        drop(row);
    }
    let query_work = b.work();
    b.release_storage(DESCRIPTOR_QUERY_STORAGE_V3).unwrap();
    drop(b);
    // Heap [maple,zebra,amber]: (zebra,amber),(maple,zebra),(amber,maple),
    // nine control steps. Heap [zebra,amber,maple]: (amber,maple),
    // (zebra,maple),(maple,amber), five controls. This is a literal schedule,
    // not a second sorting algorithm or observations from the producer.
    let cmp = |a: &str, b: &str| a.len() + b.len() + 1;
    let descriptor_sort = 9 + cmp("zebra", "amber") + cmp("maple", "zebra") + cmp("amber", "maple");
    let graph_sort = 5 + cmp("amber", "maple") + cmp("zebra", "maple") + cmp("maple", "amber");
    let pair_work: usize = ["amber", "maple", "zebra"]
        .iter()
        .map(|name| cmp(name, name) + (name.len() + 3 + 1) + cmp(name, name) + 1)
        .sum();
    let roots_work = 2
        + 3 * 2
        + 3 * 2
        + query_work
        + descriptor_sort
        + graph_sort
        + pair_work
        + cmp("amber", "maple")
        + cmp("maple", "zebra");
    let root_reference = reference_vector::<(usize, usize)>(3);
    let name_reference = reference_vector::<(usize, &str, &str)>(3);
    let index_reference = reference_vector::<usize>(3);
    let roots_storage = vector_extent(&root_reference);
    let roots_peak = roots_storage
        + DESCRIPTOR_QUERY_STORAGE_V3
        + vector_extent(&name_reference)
        + vector_extent(&index_reference);

    let mut work = Work::new(W);
    let mut b = Budget::new(&mut work, S);
    b.reserve_storage(floor).unwrap();
    let physical =
        crate::check_canonical_v12_descriptor_physical_abi_v3(&f.owner, f.profile, table, &mut b)
            .unwrap();
    let physical_work = b
        .work()
        .checked_sub(profile_work + inventory_work + roots_work + 1)
        .unwrap();
    let receipt = physical.retained_storage();
    b.reserve_storage(receipt).unwrap();
    drop(physical);
    b.release_storage(receipt).unwrap();
    assert_eq!(b.storage(), floor);
    drop(b);
    let mut work = Work::new(W);
    let mut b = Budget::new(&mut work, S);
    b.reserve_storage(floor).unwrap();
    let requirements =
        crate::check_canonical_v12_descriptor_requirements_v3(&f.owner, f.profile, table, &mut b)
            .unwrap();
    let requirements_work = b
        .work()
        .checked_sub(profile_work + inventory_work + roots_work + 1)
        .unwrap();
    let receipt = requirements.retained_storage();
    b.reserve_storage(receipt).unwrap();
    drop(requirements);
    b.release_storage(receipt).unwrap();
    assert_eq!(b.storage(), floor);
    drop(b);

    let prefix_work = 2 + 2 * f.owner.canonical().canonical_bytes().len() + 2 + profile_work;
    let before_engines = PRIOR
        + prefix_work
        + inventory_work
        + binding_work
        + roots_work
        + physical_work
        + requirements_work;
    let engine_floor = floor + guard + inventory_storage + roots_storage;
    let bindings_reference = reference_vector::<(bool, bool)>(3);
    let requirement_scratch = vector_extent(&bindings_reference)
        + DESCRIPTOR_QUERY_STORAGE_V3
        + size_of::<KernelTargetRequirementsV2>()
        + size_of::<KernelDescriptorRefV3<'_, '_>>()
        + size_of::<CapabilityCursorV3<'_, '_>>();
    let prior_peak = [
        floor + guard,
        floor + guard + inventory_peak,
        floor + guard + binding_peak,
        floor + guard + inventory_storage + roots_peak,
        engine_floor + 4 * DESCRIPTOR_QUERY_STORAGE_V3,
        engine_floor + requirement_scratch,
    ]
    .into_iter()
    .max()
    .unwrap();
    let raw = match f.profile {
        Profile::Gfx942 => lower_942(&f.owner),
        Profile::Gfx950 => lower_950(&f.owner),
    }
    .unwrap();
    let first_engine = engine_floor + size_of::<String>() + MAX_COMPILER_MODULE_TEXT_BYTES;
    let second_engine = engine_floor
        + size_of::<String>()
        + raw.capacity()
        + size_of::<String>()
        + MAX_PRODUCTION_SEMANTIC_ANCHOR_LLVM_TEXT_BYTES_V1;
    let after_engines =
        engine_floor + size_of::<String>() + f.prefix.capacity() + size_of::<(&[u8], usize)>() + 4;
    let relation =
        engine_floor + size_of::<ReplayedNativeV12TextDescriptorRelationV3<'_, '_, '_, '_, '_>>();
    assert!(
        first_engine.max(second_engine) > prior_peak.max(after_engines).max(relation),
        "an independently identified engine reserve must remain the global peak"
    );
    let first_is_peak = first_engine >= second_engine;
    let prior_peak = if first_is_peak {
        prior_peak
    } else {
        prior_peak
            .max(first_engine)
            .max(engine_floor + size_of::<String>() + raw.capacity())
    };
    Expected {
        work: before_engines + 2 + 2 + 2 * f.text.len() + 1,
        peak: first_engine.max(second_engine),
        peak_work: before_engines + if first_is_peak { 1 } else { 2 },
        prior_peak,
    }
}

struct Attempt {
    result: R<()>,
    work: usize,
    peak: usize,
    failed_storage: Option<usize>,
    failed_work: Option<usize>,
}
fn attempt(f: &ScaleFixture, work_limit: usize, storage_limit: usize) -> Attempt {
    let mut work = Work::new(work_limit);
    let mut b = Budget::new(&mut work, storage_limit);
    b.reserve_storage(f.floor()).unwrap();
    b.charge_work(PRIOR).unwrap();
    let table = decode_device_descriptor_table_v3(&f.wire, &mut free).unwrap();
    let result = check_native_v12_text_descriptor_relation_v3(
        &f.owner,
        &f.catalog,
        f.owner.canonical().canonical_bytes(),
        f.profile,
        &table,
        &f.text,
        &mut b,
    )
    .map(|relation| {
        let receipt = relation.storage().retained_storage();
        b.reserve_storage(receipt).unwrap();
        drop(relation);
        b.release_storage(receipt).unwrap();
    });
    drop(table);
    assert_eq!(b.storage(), f.floor());
    let (accepted, peak, failed_storage) = (b.work(), b.peak_storage(), b.failed_storage());
    drop(b);
    Attempt {
        result,
        work: accepted,
        peak,
        failed_storage,
        failed_work: work.failed_work(),
    }
}
fn resource(error: &(dyn Error + 'static)) -> Option<Resource> {
    error
        .downcast_ref::<Resource>()
        .copied()
        .or_else(|| error.source().and_then(resource))
}

#[test]
fn nominal_native_v3_three_root_exact_resources_use_independent_composition() {
    for profile in PROFILES {
        let f = ScaleFixture::new(profile);
        let expected = f.run(|table, _| expected(&f, table));
        let actual = attempt(&f, expected.work, expected.peak);
        actual.result.unwrap();
        assert_eq!(
            (
                actual.work,
                actual.peak,
                actual.failed_work,
                actual.failed_storage
            ),
            (expected.work, expected.peak, None, None)
        );
        let actual = attempt(&f, expected.work - 1, expected.peak);
        let error = actual.result.unwrap_err();
        let Some(Resource::Work(limit)) = resource(&error) else {
            panic!("exact typed work refusal")
        };
        assert_eq!(
            (limit.actual(), limit.limit()),
            (expected.work, expected.work - 1)
        );
        assert_eq!(
            (
                actual.work,
                actual.peak,
                actual.failed_work,
                actual.failed_storage
            ),
            (expected.work - 1, expected.peak, Some(expected.work), None)
        );
        let actual = attempt(&f, expected.work, expected.peak - 1);
        let error = actual.result.unwrap_err();
        let Some(Resource::Storage(limit)) = resource(&error) else {
            panic!("exact typed storage refusal")
        };
        assert_eq!(
            (limit.actual(), limit.limit()),
            (expected.peak, expected.peak - 1)
        );
        assert_eq!(
            (
                actual.work,
                actual.peak,
                actual.failed_work,
                actual.failed_storage
            ),
            (
                expected.peak_work,
                expected.prior_peak,
                None,
                Some(expected.peak)
            )
        );
        attempt(&f, expected.work, expected.peak).result.unwrap();
    }
}
