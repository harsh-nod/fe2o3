use super::super::storage_native_v18::tests::*;
use super::*;

fn header_premise() -> usize {
    use storage_v1::{
        StoragePointerEmissionErrorV1 as E, StoragePointerRecipeV1 as R,
        StoragePointerShapeV1 as S, StoragePointerTextOperandV1 as O, StorageTargetContextV1 as T,
    };
    // Independent source roster: entry/move slots, both table contexts, the
    // eleven named fixed-buffer sites, two ABI formatters and copy-declaration set.
    let entry = size_of::<MeterV18<'_, '_>>()
        + 2 * size_of::<Result<MeterV18<'_, '_>, Resource>>()
        + size_of::<storage_native_v18::StorageEmissionContextV18<'_>>()
        + 2 * size_of::<Result<String, LoweringErrors>>()
        + 2 * size_of::<Result<String, storage_native_v18::StorageLoweringErrorV18>>()
        + size_of::<CapacityLimitedText<'_>>();
    let pointer = 2 * size_of::<fe2o3_kernel_ir::VerifiedStorageKernelIrModuleV1<'_>>()
        + 2 * size_of::<T<'_, '_>>()
        + 2 * size_of::<Result<T<'_, '_>, E>>()
        + 2 * size_of::<R<'_, '_, '_>>()
        + 2 * size_of::<Result<R<'_, '_, '_>, E>>()
        + size_of::<S>()
        + size_of::<O<'_>>()
        + 2 * size_of::<Result<O<'_>, E>>();
    let names = 11 * size_of::<NameV18>()
        + 2 * size_of::<Result<NameV18, fmt::Error>>()
        + 2 * size_of::<Result<NameV18, LoweringErrors>>();
    entry
        + pointer
        + names
        + 2 * size_of::<storage_values_v18::ResultTypeV18<'_>>()
        + size_of::<[[[bool; 5]; 5]; 2]>()
        + size_of::<[KernelAddressSpace; 5]>()
}

#[test]
fn source_header_exact_and_one_short_preserve_nonzero_floor() {
    let header = header_premise();
    assert_eq!(header, headers_v18());
    for limit in [17 + header, 16 + header] {
        let mut work = Work::new(1000);
        let mut budget = Budget::new(&mut work, limit);
        budget.reserve_storage(17).unwrap();
        let admitted = MeterV18::new(&mut budget);
        assert_eq!(admitted.is_ok(), limit == 17 + header);
        drop(admitted);
        assert_eq!(budget.storage(), 17);
        assert_eq!(budget.work(), 0);
        assert_eq!(
            budget.failed_storage(),
            (limit < 17 + header).then_some(17 + header)
        );
    }
}

#[test]
fn swallowed_work_failure_is_sticky_without_debiting_a_smaller_retry() {
    let mut work = Work::new(7);
    let mut budget = Budget::new(&mut work, headers_v18() + 19);
    budget.reserve_storage(19).unwrap();
    let meter = MeterV18::new(&mut budget).unwrap();
    let first = meter.charge(8).unwrap_err();
    assert_eq!(meter.charge(1), Err(first));
    assert_eq!(meter.charge(0), Err(first));
    drop(meter);
    assert_eq!(budget.work(), 0);
    assert_eq!(budget.storage(), 19);
}

#[test]
fn actual_owner_entry_denies_first_current_owner_check_and_returns_original_error() {
    let module = module(
        vec![scalar()],
        vec![allocate(0, 0, Space::Private, 4)],
        Profile::Gfx942,
    );
    with_owner(&module, |owner, _| {
        let mut work = Work::new(0);
        let mut budget = Budget::new(&mut work, headers_v18() + 31);
        budget.reserve_storage(31).unwrap();
        let result = lower_canonical_storage_module_v18(owner, Profile::Gfx942, &mut budget);
        assert!(
            matches!(result, Err(storage_native_v18::StorageLoweringErrorV18::Resource(Resource::Work(error))) if error.actual() == 1 && error.limit() == 0)
        );
        assert_eq!(budget.work(), 0);
        assert_eq!(budget.storage(), 31);
    });
}

#[test]
fn text_writer_debits_utf8_bytes_before_mutation_and_preserves_the_first_denial() {
    let module = Module::new("writer");
    let mut work = Work::new(3);
    let mut budget = Budget::new(&mut work, headers_v18());
    let meter = MeterV18::new(&mut budget).unwrap();
    let mut output = CapacityLimitedText::try_new(&module, 32).unwrap();
    output.storage_meter = Some(&meter);
    output.write_str("abc").unwrap();
    output.write_str("de").unwrap();
    let first = meter.failure().unwrap();
    output.write_str("").unwrap();
    assert_eq!(meter.failure(), Some(first));
    assert_eq!(output.finish(&module).unwrap(), "abc");
    drop(meter);
    assert_eq!(budget.storage(), 0);
    assert_eq!(budget.work(), 3);
}

#[test]
fn fixed_operand_names_refuse_overflow_without_heap_fallback() {
    let accepted = NameV18::new(format_args!(
        "%storage.bb{}.op{}.encoded",
        u32::MAX,
        usize::MAX
    ))
    .unwrap();
    assert!(accepted.as_str().len() < 128);
    assert!(NameV18::new(format_args!("{}", "x".repeat(129))).is_err());
}

#[test]
fn actual_owner_preflight_exact_and_one_short_follow_source_rows_and_type_nodes() {
    let module = module(
        vec![scalar()],
        vec![allocate(0, 0, Space::Private, 4)],
        Profile::Gfx942,
    );
    // Current-owner(1), exact layout strings(1+D), row(1), entry name(6),
    // block(1), operation(1), pointer/result nodes+row(3), element+row(2).
    let predicted = 16
        + fe2o3_amd_target::PRODUCTION_AMDHSA_RUSTC_DATA_LAYOUT_V1.len()
        + fe2o3_amd_target::PRODUCTION_AMDHSA_LLVM22_WORKER_DATA_LAYOUT_V1.len();
    with_owner(&module, |owner, _| {
        for limit in [predicted, predicted - 1] {
            let mut work = Work::new(limit);
            let mut budget = Budget::new(&mut work, 29 + header_premise());
            budget.reserve_storage(29).unwrap();
            let meter = MeterV18::new(&mut budget).unwrap();
            let context = storage_native_v18::StorageEmissionContextV18 {
                owner,
                target: LoweringTarget::Gfx942XnackMinusV1,
                meter: &meter,
                root_roles: None,
            };
            let result = storage_preflight_v18::check(&context);
            assert_eq!(result.is_ok(), limit == predicted);
            if limit < predicted {
                assert!(matches!(meter.failure(), Some(Resource::Work(error))
                    if error.actual() == predicted && error.limit() == limit));
            }
            drop(result);
            drop(meter);
            assert_eq!(budget.storage(), 29);
            assert_eq!(budget.work(), limit);
        }
    });
}

#[test]
fn actual_owner_discriminant_selection_prepays_every_variant_before_output() {
    let module = module(vec![scalar(), Row {
        size: 4, alignment: 4, kind: Kind::Variants {
            encoding: Encoding::Direct { tag: Field { offset: 0, layout: Id(0) } },
            variants: [3, 17, 250].into_iter().zip([255, 1_u128 << 100, u128::MAX])
                .map(|(bits, discriminant)| Variant { discriminant, direct_tag_bits: Some(bits),
                    uninhabited: false, layout: Id(0) }).collect::<Vec<_>>().into_boxed_slice(),
        },
    }], vec![], Profile::Gfx942);
    with_owner(&module, |owner, _| {
        let Kind::Variants { encoding, variants } = &owner.module().storage_layouts[1].kind else { unreachable!() };
        for limit in [3, 2] {
            let mut work = Work::new(limit);
            let mut budget = Budget::new(&mut work, 29 + header_premise());
            budget.reserve_storage(29).unwrap();
            let meter = MeterV18::new(&mut budget).unwrap();
            let context = storage_native_v18::StorageEmissionContextV18 {
                owner, target: LoweringTarget::Gfx942XnackMinusV1, meter: &meter,
                root_roles: None,
            };
            let mut output = String::new();
            let result = storage_variants_v18::emit_logical_discriminant_selection(
                &context, &mut output, *encoding, variants, 32, "%tag");
            assert_eq!(result.is_ok(), limit == 3);
            if limit == 3 {
                assert_eq!(result.unwrap(), 2);
                assert_eq!(output.matches(" = select i1 ").count(), 3);
                assert!(output.contains(&u128::MAX.to_string()));
            } else {
                assert!(output.is_empty());
                let first = meter.failure().unwrap();
                assert!(matches!(first, Resource::Work(error) if error.actual() == 3 && error.limit() == 2));
                assert_eq!(context.charge(1).is_err(), true);
                assert_eq!(meter.failure(), Some(first));
            }
            drop(output);
            drop(meter);
            assert_eq!(budget.storage(), 29);
            assert_eq!(budget.work(), if limit == 3 { 3 } else { 0 });
        }
    });
}
