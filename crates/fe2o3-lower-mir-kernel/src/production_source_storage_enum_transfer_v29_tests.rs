use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1;
use fe2o3_mir_model::semantic_mir_v1::*;

include!("production_source_storage_read_guards_v29_tests.rs");
include!("production_source_storage_enum_tag_data_v29_tests.rs");

const EMPTY_DIRECT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(21);
const OUTER: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(22);
const TWO_NICHES: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(23);
const UNION_ENUMS: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(24);
const UNION_WITH_WORD: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(25);

fn payload_or_empty(tag: u8, payload: SemanticTypeIdV1) -> SemanticTypeDeclV1 {
    let tag_scalar = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::integer(true, 8, 1),
        SemanticScalarValidityRangeV1::new(0, 255),
    );
    let variants = (0..2)
        .map(|index| {
            let fields = if index == 0 { vec![8] } else { vec![] };
            let order = if index == 0 { vec![0] } else { vec![] };
            SemanticEnumVariantLayoutV1::from_rustc(
                index,
                16,
                8,
                SemanticFieldsShapeV1::arbitrary(fields.clone(), order).unwrap(),
                SemanticBackendReprV1::memory(true),
                None,
                false,
                None,
                8,
                100 + u64::from(index),
                SemanticAggregateLayoutV1::new(
                    fields,
                    vec![SemanticPaddingV1::new(1, if index == 0 { 7 } else { 15 }).unwrap()],
                )
                .unwrap(),
            )
            .unwrap()
        })
        .collect();
    declaration(
        tag,
        SemanticTypeLayoutV1::enum_layout_with_backend_repr(
            16,
            8,
            SemanticBackendReprV1::memory(true),
            false,
            SemanticEnumLayoutV1::new(
                variants,
                SemanticEnumEncodingV1::Direct(SemanticDirectEnumEncodingV1::new(0, 0, tag_scalar)),
            )
            .unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::enum_type(
            SIGNED,
            vec![
                SemanticEnumVariantV1::new(
                    255,
                    SemanticAggregateTypeV1::new(vec![payload]).unwrap(),
                ),
                SemanticEnumVariantV1::new(1, SemanticAggregateTypeV1::new(vec![]).unwrap()),
            ],
        )
        .unwrap(),
    )
}

fn extended_types() -> Vec<SemanticTypeDeclV1> {
    let mut declarations = types();
    declarations.push(payload_or_empty(201, WORD));
    declarations.push(payload_or_empty(202, NICHE));
    declarations.push(declaration(
        203,
        SemanticTypeLayoutV1::aggregate(
            Some(16),
            8,
            SemanticAggregateLayoutV1::new(vec![0, 8], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![NICHE, NICHE]).unwrap()),
    ));
    declarations.push(declaration(
        204,
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            16,
            8,
            SemanticFieldsShapeV1::Union { field_count: 3 },
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(true),
            None,
            false,
            None,
            8,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Union(
            SemanticAggregateTypeV1::new(vec![DIRECT, NICHE, REFERENCE]).unwrap(),
        ),
    ));
    declarations.push(declaration(
        205,
        SemanticTypeLayoutV1::aggregate(
            Some(24),
            8,
            SemanticAggregateLayoutV1::new(vec![0, 16], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![UNION_ENUMS, WORD]).unwrap()),
    ));
    declarations
}

fn run_extended(
    consume: impl FnOnce(&ProductionCallInstancePlanV1<'_>, &mut Budget<'_>) -> Result<(), Error>,
) {
    let owner = owner_with(extended_types());
    let mut work = work();
    let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
    production_call_instances_v1::with_production_call_instances_v1(
        &owner,
        ROOT,
        &mut budget,
        |instances, budget| {
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(consume(
                instances, budget,
            ))
        },
    )
    .unwrap()
    .unwrap();
}

fn payload<'layout, 'source>(
    layouts: &'layout SourceStorageLayoutsV29<'source>,
    place: &SourceStorageSubobjectV29<'layout, 'source>,
    variant: u32,
    budget: &mut Budget<'_>,
) -> Result<SourceStorageSubobjectV29<'layout, 'source>, Error> {
    let selected = layouts.project_step(place, SubobjectStep::Variant(variant), budget)?;
    let field = layouts.project_step(&selected, SubobjectStep::Field(0), budget)?;
    selected.discard(budget)?;
    Ok(field)
}

#[test]
fn niche_join_tag_noop_cannot_convert_conditional_reference_to_unconditional() {
    run(|instances, budget| {
        let floor = budget.storage();
        let layouts = SourceStorageLayoutsV29::new(instances.owner(), &[NICHE], budget)?;
        let root = layouts.root_subobject(instances.owner(), NICHE, budget)?;
        let field = payload(&layouts, &root, 1, budget)?;
        let mut some = SourceStorageStateV29::new(
            &layouts,
            instances,
            instances.root(),
            local_for(NICHE),
            budget,
        )?;
        let mut none = some.copy(budget)?;
        some.begin_variant(&root, 1, budget)?;
        some.initialize(&field, budget)?;
        some.set_discriminant(&root, 1, budget)?;
        none.set_discriminant(&root, 0, budget)?;
        assert!(some.is_initialized(&root, budget)?);
        assert!(none.is_initialized(&root, budget)?);
        let mut joined = some.join(&none, budget)?;
        let before = joined.copy(budget)?;
        assert!(joined.is_initialized(&root, budget)?);
        assert!(!joined.variant_is_readable(&root, 1, budget)?);
        joined.set_discriminant(&root, 1, budget)?;
        assert!(joined.equivalent(&before, budget)?);
        assert!(!joined.variant_is_readable(&root, 1, budget)?);
        let proof = joined.initialization_proof(&field, true, budget)?;
        assert!(proof.initialized);
        assert_eq!(proof.guards, [0]);
        proof.discard(&layouts.lease, budget)?;
        joined.initialize(&field, budget)?;
        joined.set_discriminant(&root, 1, budget)?;
        assert!(joined.variant_is_readable(&root, 1, budget)?);
        assert!(joined.is_initialized(&root, budget)?);
        before.discard(budget)?;
        joined.discard(budget)?;
        some.discard(budget)?;
        none.discard(budget)?;
        field.discard(budget)?;
        root.discard(budget)?;
        layouts.release(budget)?;
        assert_eq!(budget.storage(), floor);
        Ok(())
    });
}

#[test]
fn encoded_niche_write_invalidates_old_reference_without_uninitializing_tag_bytes() {
    run(|instances, budget| {
        let layouts = SourceStorageLayoutsV29::new(instances.owner(), &[NICHE], budget)?;
        let root = layouts.root_subobject(instances.owner(), NICHE, budget)?;
        let field = payload(&layouts, &root, 1, budget)?;
        let mut state = SourceStorageStateV29::new(
            &layouts,
            instances,
            instances.root(),
            local_for(NICHE),
            budget,
        )?;
        state.initialize(&field, budget)?;
        state.set_discriminant(&root, 1, budget)?;
        let exact = state.copy(budget)?;
        state.set_discriminant(&root, 1, budget)?;
        assert!(state.equivalent(&exact, budget)?);
        state.set_discriminant(&root, 0, budget)?;
        assert!(state.variant_is_readable(&root, 0, budget)?);
        assert!(state.is_initialized(&root, budget)?);
        assert!(!state.is_initialized(&field, budget)?);
        assert!(state.bytes_initialized(field.range, budget)?);
        state.set_discriminant(&root, 1, budget)?;
        assert!(state.variant_is_readable(&root, 0, budget)?);
        assert!(!state.variant_is_readable(&root, 1, budget)?);
        assert!(!state.is_initialized(&field, budget)?);
        state.initialize(&field, budget)?;
        state.set_discriminant(&root, 1, budget)?;
        assert!(state.is_initialized(&root, budget)?);
        state.deinitialize(&field, budget)?;
        state.set_discriminant(&root, 1, budget)?;
        assert!(!state.variant_is_readable(&root, 1, budget)?);
        assert!(!state.is_initialized(&root, budget)?);
        exact.discard(budget)?;
        state.discard(budget)?;
        field.discard(budget)?;
        root.discard(budget)?;
        layouts.release(budget)
    });
}

#[test]
fn direct_tag_assignment_never_initializes_joined_or_whole_default_payload() {
    run_extended(|instances, budget| {
        let layouts = SourceStorageLayoutsV29::new(instances.owner(), &[EMPTY_DIRECT], budget)?;
        let root = layouts.root_subobject(instances.owner(), EMPTY_DIRECT, budget)?;
        let field = payload(&layouts, &root, 0, budget)?;
        let mut full = SourceStorageStateV29::new(
            &layouts,
            instances,
            instances.root(),
            local_for(EMPTY_DIRECT),
            budget,
        )?;
        let mut empty = full.copy(budget)?;
        full.initialize(&field, budget)?;
        full.set_discriminant(&root, 0, budget)?;
        let exact = full.copy(budget)?;
        full.set_discriminant(&root, 0, budget)?;
        assert!(full.equivalent(&exact, budget)?);
        empty.set_discriminant(&root, 1, budget)?;
        let mut joined = full.join(&empty, budget)?;
        assert!(joined.is_initialized(&root, budget)?);
        assert!(!joined.bytes_initialized(field.range, budget)?);
        joined.set_discriminant(&root, 0, budget)?;
        assert!(joined.variant_is_readable(&root, 0, budget)?);
        assert!(!joined.is_initialized(&root, budget)?);
        assert!(!joined.is_initialized(&field, budget)?);
        assert!(!joined.bytes_initialized(field.range, budget)?);
        assert!(joined.bytes_initialized(SourceStorageRangeV29::new(0, 1, 16)?, budget)?);
        joined.initialize(&field, budget)?;
        assert!(joined.is_initialized(&root, budget)?);
        empty.initialize(&root, budget)?;
        empty.set_discriminant(&root, 0, budget)?;
        assert!(!empty.is_initialized(&field, budget)?);
        assert!(!empty.is_initialized(&root, budget)?);
        exact.discard(budget)?;
        full.discard(budget)?;
        empty.discard(budget)?;
        joined.discard(budget)?;
        field.discard(budget)?;
        root.discard(budget)?;
        layouts.release(budget)
    });
}

#[test]
fn whole_niche_default_and_physical_bytes_do_not_certify_nonnull_payload() {
    run(|instances, budget| {
        let layouts = SourceStorageLayoutsV29::new(instances.owner(), &[NICHE], budget)?;
        let root = layouts.root_subobject(instances.owner(), NICHE, budget)?;
        let field = payload(&layouts, &root, 1, budget)?;
        let mut state = SourceStorageStateV29::new(
            &layouts,
            instances,
            instances.root(),
            local_for(NICHE),
            budget,
        )?;
        state.initialize(&root, budget)?;
        assert!(state.is_initialized(&root, budget)?);
        state.set_discriminant(&root, 1, budget)?;
        assert!(!state.variant_is_readable(&root, 1, budget)?);
        state.set_discriminant(&root, 0, budget)?;
        assert!(state.is_initialized(&root, budget)?);
        assert!(!state.is_initialized(&field, budget)?);
        state.set_discriminant(&root, 1, budget)?;
        assert!(state.variant_is_readable(&root, 0, budget)?);
        assert!(state.is_initialized(&root, budget)?);
        state.discard(budget)?;
        field.discard(budget)?;
        root.discard(budget)?;
        layouts.release(budget)
    });
}

#[test]
fn nested_variant_guards_cannot_be_discharged_by_inner_tag_assignment() {
    run_extended(|instances, budget| {
        let layouts = SourceStorageLayoutsV29::new(instances.owner(), &[OUTER], budget)?;
        let root = layouts.root_subobject(instances.owner(), OUTER, budget)?;
        let inner = payload(&layouts, &root, 0, budget)?;
        let field = payload(&layouts, &inner, 1, budget)?;
        let mut full = SourceStorageStateV29::new(
            &layouts,
            instances,
            instances.root(),
            local_for(OUTER),
            budget,
        )?;
        let mut empty = full.copy(budget)?;
        full.initialize(&field, budget)?;
        full.set_discriminant(&inner, 1, budget)?;
        full.set_discriminant(&root, 0, budget)?;
        empty.set_discriminant(&root, 1, budget)?;
        let mut joined = full.join(&empty, budget)?;
        assert!(joined.is_initialized(&root, budget)?);
        assert!(!joined.bytes_initialized(field.range, budget)?);
        let before = joined.copy(budget)?;
        joined.set_discriminant(&inner, 1, budget)?;
        assert!(joined.equivalent(&before, budget)?);
        joined.set_discriminant(&root, 0, budget)?;
        assert!(!joined.is_initialized(&root, budget)?);
        assert!(!joined.is_initialized(&field, budget)?);
        assert!(!joined.bytes_initialized(field.range, budget)?);
        joined.initialize(&field, budget)?;
        joined.set_discriminant(&inner, 1, budget)?;
        assert!(joined.is_initialized(&root, budget)?);
        before.discard(budget)?;
        joined.discard(budget)?;
        full.discard(budget)?;
        empty.discard(budget)?;
        field.discard(budget)?;
        inner.discard(budget)?;
        root.discard(budget)?;
        layouts.release(budget)
    });
}

#[test]
fn external_unknown_variant_guard_is_not_promoted_by_typed_subobject_copy() {
    run_extended(|instances, budget| {
        let layouts = SourceStorageLayoutsV29::new(instances.owner(), &[OUTER, NICHE], budget)?;
        let outer = layouts.root_subobject(instances.owner(), OUTER, budget)?;
        let inner = payload(&layouts, &outer, 0, budget)?;
        let field = payload(&layouts, &inner, 1, budget)?;
        let root = layouts.root_subobject(instances.owner(), NICHE, budget)?;
        let destination_field = payload(&layouts, &root, 1, budget)?;
        let mut full = SourceStorageStateV29::new(
            &layouts,
            instances,
            instances.root(),
            local_for(OUTER),
            budget,
        )?;
        let mut empty = full.copy(budget)?;
        full.initialize(&field, budget)?;
        full.set_discriminant(&inner, 1, budget)?;
        full.set_discriminant(&outer, 0, budget)?;
        empty.set_discriminant(&outer, 1, budget)?;
        let joined = full.join(&empty, budget)?;
        let mut target = SourceStorageStateV29::new(
            &layouts,
            instances,
            instances.root(),
            local_for(NICHE),
            budget,
        )?;
        target.copy_subobject_from(&root, &joined, &inner, budget)?;
        assert!(!target.is_initialized(&destination_field, budget)?);
        assert!(!target.variant_is_readable(&root, 1, budget)?);
        target.set_discriminant(&root, 1, budget)?;
        assert!(!target.is_initialized(&root, budget)?);
        target.copy_subobject_from(&root, &full, &inner, budget)?;
        assert!(target.is_initialized(&root, budget)?);
        assert!(target.variant_is_readable(&root, 1, budget)?);
        target.discard(budget)?;
        joined.discard(budget)?;
        full.discard(budget)?;
        empty.discard(budget)?;
        destination_field.discard(budget)?;
        root.discard(budget)?;
        field.discard(budget)?;
        inner.discard(budget)?;
        outer.discard(budget)?;
        layouts.release(budget)
    });
}

#[test]
fn overlapping_enum_copy_remaps_internal_guards_and_preserves_independent_source() {
    run_extended(|instances, budget| {
        let layouts = SourceStorageLayoutsV29::new(instances.owner(), &[TWO_NICHES], budget)?;
        let root = layouts.root_subobject(instances.owner(), TWO_NICHES, budget)?;
        let first = layouts.project_step(&root, SubobjectStep::Field(0), budget)?;
        let second = layouts.project_step(&root, SubobjectStep::Field(1), budget)?;
        let source_field = payload(&layouts, &first, 1, budget)?;
        let target_field = payload(&layouts, &second, 1, budget)?;
        let mut some = SourceStorageStateV29::new(
            &layouts,
            instances,
            instances.root(),
            local_for(TWO_NICHES),
            budget,
        )?;
        let mut none = some.copy(budget)?;
        some.initialize(&source_field, budget)?;
        some.set_discriminant(&first, 1, budget)?;
        none.set_discriminant(&first, 0, budget)?;
        let mut joined = some.join(&none, budget)?;
        joined.copy_subobject_within(&second, &first, budget)?;
        assert!(joined.is_initialized(&first, budget)?);
        assert!(joined.is_initialized(&second, budget)?);
        let proof = joined.initialization_proof(&target_field, true, budget)?;
        assert_eq!(proof.guards, [1]);
        proof.discard(&layouts.lease, budget)?;
        joined.set_discriminant(&second, 1, budget)?;
        assert!(!joined.variant_is_readable(&second, 1, budget)?);
        joined.initialize(&target_field, budget)?;
        joined.set_discriminant(&second, 1, budget)?;
        assert!(joined.variant_is_readable(&second, 1, budget)?);
        assert!(!joined.variant_is_readable(&first, 1, budget)?);
        let before = joined.copy(budget)?;
        joined.copy_subobject_within(&root, &root, budget)?;
        assert!(joined.equivalent(&before, budget)?);
        joined.deinitialize(&source_field, budget)?;
        assert!(joined.is_initialized(&target_field, budget)?);
        before.discard(budget)?;
        joined.discard(budget)?;
        some.discard(budget)?;
        none.discard(budget)?;
        source_field.discard(budget)?;
        target_field.discard(budget)?;
        first.discard(budget)?;
        second.discard(budget)?;
        root.discard(budget)?;
        layouts.release(budget)
    });
}

#[test]
fn nested_nullary_tag_cannot_escape_its_outer_variant_through_copy_or_retag() {
    run_extended(|instances, budget| {
        let layouts = SourceStorageLayoutsV29::new(instances.owner(), &[OUTER, NICHE], budget)?;
        let outer = layouts.root_subobject(instances.owner(), OUTER, budget)?;
        let inner = payload(&layouts, &outer, 0, budget)?;
        let root = layouts.root_subobject(instances.owner(), NICHE, budget)?;
        let mut full = SourceStorageStateV29::new(
            &layouts,
            instances,
            instances.root(),
            local_for(OUTER),
            budget,
        )?;
        let mut empty = full.copy(budget)?;
        full.set_discriminant(&inner, 0, budget)?;
        full.set_discriminant(&outer, 0, budget)?;
        empty.set_discriminant(&outer, 1, budget)?;
        let mut joined = full.join(&empty, budget)?;
        assert!(joined.is_initialized(&outer, budget)?);
        let index = joined.enum_entry(&inner, budget)?.unwrap();
        assert_eq!(joined.enums[index].guards, [0]);
        let mut target = SourceStorageStateV29::new(
            &layouts,
            instances,
            instances.root(),
            local_for(NICHE),
            budget,
        )?;
        target.copy_subobject_from(&root, &joined, &inner, budget)?;
        assert!(!target.variant_is_readable(&root, 0, budget)?);
        assert!(!target.is_initialized(&root, budget)?);
        joined.set_discriminant(&outer, 0, budget)?;
        assert!(!joined.variant_is_readable(&inner, 0, budget)?);
        assert!(!joined.is_initialized(&outer, budget)?);
        joined.set_discriminant(&inner, 0, budget)?;
        assert!(joined.variant_is_readable(&inner, 0, budget)?);
        assert!(joined.is_initialized(&outer, budget)?);
        target.discard(budget)?;
        joined.discard(budget)?;
        full.discard(budget)?;
        empty.discard(budget)?;
        root.discard(budget)?;
        inner.discard(budget)?;
        outer.discard(budget)?;
        layouts.release(budget)
    });
}

#[test]
fn subobject_copy_rebases_internal_guards_without_discharging_them() {
    run_extended(|instances, budget| {
        let layouts = SourceStorageLayoutsV29::new(instances.owner(), &[OUTER, NICHE], budget)?;
        let outer = layouts.root_subobject(instances.owner(), OUTER, budget)?;
        let inner = payload(&layouts, &outer, 0, budget)?;
        let source_field = payload(&layouts, &inner, 1, budget)?;
        let root = layouts.root_subobject(instances.owner(), NICHE, budget)?;
        let target_field = payload(&layouts, &root, 1, budget)?;
        let mut some = SourceStorageStateV29::new(
            &layouts,
            instances,
            instances.root(),
            local_for(OUTER),
            budget,
        )?;
        let mut none = some.copy(budget)?;
        some.initialize(&source_field, budget)?;
        some.set_discriminant(&inner, 1, budget)?;
        some.set_discriminant(&outer, 0, budget)?;
        none.set_discriminant(&inner, 0, budget)?;
        none.set_discriminant(&outer, 0, budget)?;
        let joined = some.join(&none, budget)?;
        assert!(joined.variant_is_readable(&outer, 0, budget)?);
        let mut target = SourceStorageStateV29::new(
            &layouts,
            instances,
            instances.root(),
            local_for(NICHE),
            budget,
        )?;
        target.copy_subobject_from(&root, &joined, &inner, budget)?;
        assert!(target.is_initialized(&root, budget)?);
        let proof = target.initialization_proof(&target_field, true, budget)?;
        assert_eq!(proof.guards, [0]);
        proof.discard(&layouts.lease, budget)?;
        target.set_discriminant(&root, 1, budget)?;
        assert!(!target.variant_is_readable(&root, 1, budget)?);
        target.initialize(&target_field, budget)?;
        target.set_discriminant(&root, 1, budget)?;
        assert!(target.variant_is_readable(&root, 1, budget)?);
        target.discard(budget)?;
        joined.discard(budget)?;
        some.discard(budget)?;
        none.discard(budget)?;
        target_field.discard(budget)?;
        root.discard(budget)?;
        source_field.discard(budget)?;
        inner.discard(budget)?;
        outer.discard(budget)?;
        layouts.release(budget)
    });
}

#[test]
fn guarded_joins_are_commutative_idempotent_and_storage_stable() {
    run_extended(|instances, budget| {
        let floor = budget.storage();
        let layouts = SourceStorageLayoutsV29::new(instances.owner(), &[EMPTY_DIRECT], budget)?;
        let root = layouts.root_subobject(instances.owner(), EMPTY_DIRECT, budget)?;
        let field = payload(&layouts, &root, 0, budget)?;
        let mut full = SourceStorageStateV29::new(
            &layouts,
            instances,
            instances.root(),
            local_for(EMPTY_DIRECT),
            budget,
        )?;
        let mut empty = full.copy(budget)?;
        full.initialize(&field, budget)?;
        full.set_discriminant(&root, 0, budget)?;
        empty.set_discriminant(&root, 1, budget)?;
        let mut fixed = full.join(&empty, budget)?;
        let reverse = empty.join(&full, budget)?;
        assert!(fixed.equivalent(&reverse, budget)?);
        reverse.discard(budget)?;
        let idempotent = fixed.join(&fixed, budget)?;
        assert!(fixed.equivalent(&idempotent, budget)?);
        fixed.discard(budget)?;
        fixed = idempotent;
        // Normalize retained vector capacities for this self-join shape once.
        let stable_storage = budget.storage();
        for _ in 0..32 {
            let next = fixed.join(&fixed, budget)?;
            assert!(fixed.equivalent(&next, budget)?);
            fixed.discard(budget)?;
            fixed = next;
            assert_eq!(budget.storage(), stable_storage);
        }
        fixed.discard(budget)?;
        full.discard(budget)?;
        empty.discard(budget)?;
        field.discard(budget)?;
        root.discard(budget)?;
        layouts.release(budget)?;
        assert_eq!(budget.storage(), floor);
        Ok(())
    });
}

#[test]
fn tag_writes_invalidate_overlapping_union_tags_but_preserve_nonoverlapping_payloads() {
    run_extended(|instances, budget| {
        let layouts = SourceStorageLayoutsV29::new(instances.owner(), &[UNION_ENUMS], budget)?;
        let root = layouts.root_subobject(instances.owner(), UNION_ENUMS, budget)?;
        let direct = layouts.project_step(&root, SubobjectStep::Field(0), budget)?;
        let niche = layouts.project_step(&root, SubobjectStep::Field(1), budget)?;
        let zero = payload(&layouts, &direct, 0, budget)?;
        let one = payload(&layouts, &direct, 1, budget)?;
        let mut state = SourceStorageStateV29::new(
            &layouts,
            instances,
            instances.root(),
            local_for(UNION_ENUMS),
            budget,
        )?;
        state.initialize(&zero, budget)?;
        state.set_discriminant(&direct, 0, budget)?;
        assert!(state.is_initialized(&direct, budget)?);
        state.set_discriminant(&niche, 0, budget)?;
        assert!(state.variant_is_readable(&niche, 0, budget)?);
        assert!(!state.variant_is_readable(&direct, 0, budget)?);
        assert!(state.is_initialized(&zero, budget)?);
        assert!(state.bytes_initialized(zero.range, budget)?);
        state.set_discriminant(&direct, 1, budget)?;
        assert!(!state.variant_is_readable(&niche, 0, budget)?);
        assert!(!state.is_initialized(&niche, budget)?);
        assert!(state.variant_is_readable(&direct, 1, budget)?);
        assert!(!state.is_initialized(&direct, budget)?);
        assert!(state.is_initialized(&zero, budget)?);
        state.initialize(&one, budget)?;
        assert!(state.is_initialized(&direct, budget)?);
        state.discard(budget)?;
        zero.discard(budget)?;
        one.discard(budget)?;
        direct.discard(budget)?;
        niche.discard(budget)?;
        root.discard(budget)?;
        layouts.release(budget)
    });
}

#[test]
fn nested_union_tag_write_kills_typed_reference_alias_not_initialized_bytes_or_sibling() {
    run_extended(|instances, budget| {
        let layouts = SourceStorageLayoutsV29::new(instances.owner(), &[UNION_WITH_WORD], budget)?;
        let root = layouts.root_subobject(instances.owner(), UNION_WITH_WORD, budget)?;
        let union = layouts.project_step(&root, SubobjectStep::Field(0), budget)?;
        let sibling = layouts.project_step(&root, SubobjectStep::Field(1), budget)?;
        let direct = layouts.project_step(&union, SubobjectStep::Field(0), budget)?;
        let niche = layouts.project_step(&union, SubobjectStep::Field(1), budget)?;
        let reference = layouts.project_step(&union, SubobjectStep::Field(2), budget)?;
        let niche_field = payload(&layouts, &niche, 1, budget)?;
        let mut state = SourceStorageStateV29::new(
            &layouts,
            instances,
            instances.root(),
            local_for(UNION_WITH_WORD),
            budget,
        )?;
        state.initialize(&sibling, budget)?;
        state.initialize(&reference, budget)?;
        let before = state.copy(budget)?;
        state.set_discriminant(&niche, 1, budget)?;
        assert!(state.equivalent(&before, budget)?);
        state.set_discriminant(&direct, 0, budget)?;
        assert!(!state.is_initialized(&reference, budget)?);
        assert!(state.bytes_initialized(reference.range, budget)?);
        assert!(state.is_initialized(&sibling, budget)?);
        state.initialize(&niche_field, budget)?;
        state.set_discriminant(&niche, 1, budget)?;
        assert!(state.variant_is_readable(&niche, 1, budget)?);
        state.set_discriminant(&direct, 0, budget)?;
        assert!(!state.variant_is_readable(&niche, 1, budget)?);
        assert!(!state.is_initialized(&niche_field, budget)?);
        assert!(state.bytes_initialized(niche_field.range, budget)?);
        assert!(state.is_initialized(&sibling, budget)?);
        state.set_discriminant(&niche, 1, budget)?;
        assert!(!state.variant_is_readable(&niche, 1, budget)?);
        before.discard(budget)?;
        state.discard(budget)?;
        niche_field.discard(budget)?;
        reference.discard(budget)?;
        direct.discard(budget)?;
        niche.discard(budget)?;
        sibling.discard(budget)?;
        union.discard(budget)?;
        root.discard(budget)?;
        layouts.release(budget)
    });
}

fn resource(error: Error) -> ArgumentResourceV1 {
    match error {
        Error::ArgumentCorrespondenceResource(resource) => resource,
        other => panic!("not a resource denial: {other:?}"),
    }
}

#[test]
fn guard_proof_copy_has_independent_exact_and_one_short_resource_boundaries() {
    const COUNT: usize = 2;
    const FLOOR: usize = 17;
    // Header and Result envelope, then emission_vec's three work units and
    // one visit per copied guard. No inferred successful-run quota.
    let header =
        size_of::<InitializationProofV29>() + size_of::<Result<InitializationProofV29, Error>>();
    let bytes = header + COUNT * size_of::<usize>();
    let work_limit = 3 + COUNT;
    for (work_limit, storage_limit, denial) in [
        (work_limit, FLOOR + bytes, None),
        (work_limit - 1, FLOOR + bytes, Some(true)),
        (work_limit, FLOOR + bytes - 1, Some(false)),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(FLOOR).unwrap();
        let lease = Lease::new(&budget);
        let result = InitializationProofV29::new(true, &[0, 2], &lease, &mut budget);
        match (result, denial) {
            (Ok(proof), None) => {
                assert_eq!(proof.guards, [0, 2]);
                assert_eq!(budget.work(), work_limit);
                assert_eq!(budget.peak_storage(), FLOOR + bytes);
                proof.discard(&lease, &mut budget).unwrap();
            }
            (Err(error), Some(short_work)) => {
                let first = resource(error);
                assert!(matches!(first, ArgumentResourceV1::Work(_)) == short_work);
                assert!(matches!(first, ArgumentResourceV1::Storage(_)) != short_work);
                assert_eq!(resource(lease.work(0, &mut budget).unwrap_err()), first);
                lease.refund(lease.owned.get(), &mut budget).unwrap();
            }
            _ => panic!("guard proof admission disagreed with independent boundary"),
        }
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn guard_intersection_merge_is_metered_deduplicated_and_preserves_inputs_on_denial() {
    // Two two-guard copies, one four-slot output vector, three sorted merge
    // visits, and three non-growing emission_push admissions (two each).
    let work_limit = 2 * (3 + 2) + 3 + 3 * (1 + 2);
    let header =
        size_of::<InitializationProofV29>() + size_of::<Result<InitializationProofV29, Error>>();
    let storage_limit = 3 * header + 8 * size_of::<usize>();
    for short in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit - usize::from(short));
        let mut budget = Budget::new(&mut work, storage_limit);
        let lease = Lease::new(&budget);
        let left = InitializationProofV29::new(true, &[0, 2], &lease, &mut budget).unwrap();
        let right = InitializationProofV29::new(true, &[1, 2], &lease, &mut budget).unwrap();
        let merged = left.intersect(&right, &lease, &mut budget);
        if short {
            let error = match merged {
                Ok(_) => panic!("short merge admitted"),
                Err(error) => resource(error),
            };
            assert!(matches!(error, ArgumentResourceV1::Work(_)));
            assert_eq!(
                resource(lease.reserve(usize::MAX, &mut budget).unwrap_err()),
                error
            );
        } else {
            let merged = merged.unwrap();
            assert_eq!(merged.guards, [0, 1, 2]);
            assert_eq!(budget.work(), work_limit);
            assert_eq!(budget.peak_storage(), storage_limit);
            merged.discard(&lease, &mut budget).unwrap();
        }
        assert_eq!(left.guards, [0, 2]);
        assert_eq!(right.guards, [1, 2]);
        left.discard(&lease, &mut budget).unwrap();
        right.discard(&lease, &mut budget).unwrap();
        // Failed merge scratch is dropped on return but deliberately retains
        // its charge until the owning layout's failure cleanup boundary.
        lease.refund(lease.owned.get(), &mut budget).unwrap();
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn guard_proof_foreign_or_lost_custody_cannot_refund_another_owners_credit() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(100);
    let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(100);
    let mut budget = Budget::new(&mut work, 4096);
    let mut foreign = Budget::new(&mut foreign_work, 4096);
    budget.reserve_storage(17).unwrap();
    foreign.reserve_storage(17).unwrap();
    let lease = Lease::new(&budget);
    let proof = InitializationProofV29::new(true, &[0], &lease, &mut budget).unwrap();
    let original = budget.storage();
    assert_eq!(
        resource(proof.discard(&lease, &mut foreign).unwrap_err()),
        ArgumentResourceV1::Accounting
    );
    assert_eq!(foreign.storage(), 17);
    assert_eq!(budget.storage(), original);
    lease.refund(lease.owned.get(), &mut budget).unwrap();
    assert_eq!(budget.storage(), 17);
    let lease = Lease::new(&budget);
    let proof = InitializationProofV29::new(true, &[0], &lease, &mut budget).unwrap();
    budget.release_storage(1).unwrap();
    let lost = budget.storage();
    assert_eq!(
        resource(proof.discard(&lease, &mut budget).unwrap_err()),
        ArgumentResourceV1::Accounting
    );
    assert_eq!(budget.storage(), lost);
    assert!(lease.refund(lease.owned.get(), &mut budget).is_err());
    assert_eq!(budget.storage(), lost);
}

#[test]
fn niche_activation_requires_source_validity_excluding_every_encoded_value() {
    // A fully valid scalar has no source niche and cannot enter this helper.
    assert_eq!(
        SemanticLayoutNicheV1::new(
            0,
            SemanticBackendPrimitiveV1::integer(false, 64, 8),
            SemanticScalarValidityRangeV1::new(0, u128::from(u64::MAX)),
        ),
        Err(SemanticMirErrorV1::InvalidTypeLayout)
    );
    let mut work = CanonicalKernelIrWorkBudgetV1::new(100);
    let mut budget = Budget::new(&mut work, 0);
    let lease = Lease::new(&budget);
    for (bits, low, high, variants, start, expected) in [
        (64, 1, u128::from(u64::MAX), (0, 0), 0, true),
        (64, 0, u128::from(u64::MAX) - 1, (0, 0), 0, false),
        (8, 10, 200, (0, 10), 250, true),
        (8, 10, 200, (0, 10), 5, false),
        (8, 200, 10, (0, 100), 50, true),
        (8, 200, 10, (0, 100), 180, false),
        (8, 1, 1, (0, 256), 2, false),
        (8, 1, 1, (0, 255), 2, false),
        (128, 1, u128::MAX, (0, 0), 0, true),
        (128, 1, u128::MAX, (0, 1), u128::MAX, false),
    ] {
        let scalar = SemanticBackendPrimitiveV1::integer(false, bits, u64::from(bits / 8));
        let source =
            SemanticLayoutNicheV1::new(0, scalar, SemanticScalarValidityRangeV1::new(low, high))
                .unwrap();
        assert_eq!(
            niche_values_excluded_v29(source, variants, start, &lease, &mut budget).unwrap(),
            expected
        );
    }
    assert_eq!(budget.work(), 10 * 4);
    assert_eq!(budget.storage(), 0);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(3);
    let mut budget = Budget::new(&mut work, 0);
    let lease = Lease::new(&budget);
    let source = SemanticLayoutNicheV1::new(
        0,
        SemanticBackendPrimitiveV1::pointer(0, 8, 8),
        SemanticScalarValidityRangeV1::new(1, u128::from(u64::MAX)),
    )
    .unwrap();
    assert!(matches!(
        resource(niche_values_excluded_v29(source, (0, 0), 0, &lease, &mut budget).unwrap_err()),
        ArgumentResourceV1::Work(_)
    ));
}

const NESTED_UNION: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(26);
const UNION_PAIR_BYTE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(27);
const UNION_HUGE_WORD: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(28);

fn union_declaration(tag: u8, size: u64, fields: Vec<SemanticTypeIdV1>) -> SemanticTypeDeclV1 {
    declaration(
        tag,
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            size,
            8,
            SemanticFieldsShapeV1::Union {
                field_count: u64::try_from(fields.len()).unwrap(),
            },
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(true),
            None,
            false,
            None,
            8,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Union(SemanticAggregateTypeV1::new(fields).unwrap()),
    )
}

fn run_union_extended(
    consume: impl FnOnce(&ProductionCallInstancePlanV1<'_>, &mut Budget<'_>) -> Result<(), Error>,
) {
    let mut types = extended_types();
    types.push(declaration(
        206,
        SemanticTypeLayoutV1::aggregate(
            Some(16),
            8,
            SemanticAggregateLayoutV1::new(vec![0, 8], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![UNION, WORD]).unwrap()),
    ));
    types.push(union_declaration(207, 16, vec![PAIR, BYTE]));
    types.push(union_declaration(
        208,
        u64::from(u32::MAX) * 8,
        vec![HUGE, WORD],
    ));
    let owner = owner_with(types);
    let mut work = work();
    let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
    production_call_instances_v1::with_production_call_instances_v1(
        &owner,
        ROOT,
        &mut budget,
        |instances, budget| {
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(consume(
                instances, budget,
            ))
        },
    )
    .unwrap()
    .unwrap();
}

#[test]
fn whole_union_default_is_killed_by_full_range_deinitialization() {
    run(|instances, budget| {
        let floor = budget.storage();
        let layouts = SourceStorageLayoutsV29::new(instances.owner(), &[UNION], budget)?;
        let root = layouts.root_subobject(instances.owner(), UNION, budget)?;
        let word = layouts.project_step(&root, SubobjectStep::Field(0), budget)?;
        let byte = layouts.project_step(&root, SubobjectStep::Field(1), budget)?;
        let mut state = SourceStorageStateV29::new(
            &layouts,
            instances,
            instances.root(),
            local_for(UNION),
            budget,
        )?;
        state.initialize(&root, budget)?;
        assert!(state.is_initialized(&root, budget)?);
        assert!(!state.is_initialized(&word, budget)?);
        assert!(!state.is_initialized(&byte, budget)?);
        state.deinitialize(&word, budget)?;
        assert!(!state.is_initialized(&root, budget)?);
        assert!(!state.is_initialized(&word, budget)?);
        assert!(!state.is_initialized(&byte, budget)?);
        assert!(!state.bytes_initialized(word.range, budget)?);
        state.initialize(&byte, budget)?;
        assert!(state.is_initialized(&root, budget)?);
        assert!(state.is_initialized(&byte, budget)?);
        assert!(!state.is_initialized(&word, budget)?);
        state.deinitialize(&byte, budget)?;
        assert!(!state.is_initialized(&root, budget)?);
        state.discard(budget)?;
        word.discard(budget)?;
        byte.discard(budget)?;
        root.discard(budget)?;
        layouts.release(budget)?;
        assert_eq!(budget.storage(), floor);
        Ok(())
    });
}

#[test]
fn whole_union_enum_default_cannot_survive_two_incompatible_tag_writes() {
    run_extended(|instances, budget| {
        let floor = budget.storage();
        let layouts = SourceStorageLayoutsV29::new(instances.owner(), &[UNION_ENUMS], budget)?;
        let root = layouts.root_subobject(instances.owner(), UNION_ENUMS, budget)?;
        let direct = layouts.project_step(&root, SubobjectStep::Field(0), budget)?;
        let niche = layouts.project_step(&root, SubobjectStep::Field(1), budget)?;
        let reference = layouts.project_step(&root, SubobjectStep::Field(2), budget)?;
        let word = payload(&layouts, &direct, 0, budget)?;
        let mut state = SourceStorageStateV29::new(
            &layouts,
            instances,
            instances.root(),
            local_for(UNION_ENUMS),
            budget,
        )?;
        state.initialize(&root, budget)?;
        assert!(state.is_initialized(&root, budget)?);
        state.set_discriminant(&niche, 0, budget)?;
        assert!(state.is_initialized(&root, budget)?);
        assert!(state.variant_is_readable(&niche, 0, budget)?);
        state.set_discriminant(&direct, 0, budget)?;
        assert!(!state.is_initialized(&root, budget)?);
        assert!(!state.is_initialized(&direct, budget)?);
        assert!(!state.is_initialized(&niche, budget)?);
        assert!(!state.is_initialized(&reference, budget)?);
        assert!(!state.is_initialized(&word, budget)?);
        assert!(!state.bytes_initialized(word.range, budget)?);
        let invalid = state.copy(budget)?;
        state.initialize(&word, budget)?;
        assert!(state.is_initialized(&direct, budget)?);
        assert!(state.is_initialized(&root, budget)?);
        let joined = state.join(&invalid, budget)?;
        assert!(!joined.is_initialized(&root, budget)?);
        assert!(!joined.is_initialized(&word, budget)?);
        joined.discard(budget)?;
        invalid.discard(budget)?;
        state.discard(budget)?;
        word.discard(budget)?;
        reference.discard(budget)?;
        direct.discard(budget)?;
        niche.discard(budget)?;
        root.discard(budget)?;
        layouts.release(budget)?;
        assert_eq!(budget.storage(), floor);
        Ok(())
    });
}

#[test]
fn nested_union_mutation_preserves_aggregate_siblings_through_reinitialization_and_join() {
    run_union_extended(|instances, budget| {
        let floor = budget.storage();
        let layouts = SourceStorageLayoutsV29::new(instances.owner(), &[NESTED_UNION], budget)?;
        let root = layouts.root_subobject(instances.owner(), NESTED_UNION, budget)?;
        let union = layouts.project_step(&root, SubobjectStep::Field(0), budget)?;
        let sibling = layouts.project_step(&root, SubobjectStep::Field(1), budget)?;
        let word = layouts.project_step(&union, SubobjectStep::Field(0), budget)?;
        let byte = layouts.project_step(&union, SubobjectStep::Field(1), budget)?;
        let mut state = SourceStorageStateV29::new(
            &layouts,
            instances,
            instances.root(),
            local_for(NESTED_UNION),
            budget,
        )?;
        state.initialize(&root, budget)?;
        state.deinitialize(&word, budget)?;
        assert!(!state.is_initialized(&root, budget)?);
        assert!(!state.is_initialized(&union, budget)?);
        assert!(state.is_initialized(&sibling, budget)?);
        assert!(state.bytes_initialized(sibling.range, budget)?);
        let partial = state.copy(budget)?;
        state.initialize(&byte, budget)?;
        assert!(state.is_initialized(&root, budget)?);
        assert!(state.is_initialized(&sibling, budget)?);
        let joined = state.join(&partial, budget)?;
        assert!(!joined.is_initialized(&root, budget)?);
        assert!(!joined.is_initialized(&union, budget)?);
        assert!(joined.is_initialized(&sibling, budget)?);
        state.initialize(&union, budget)?;
        assert!(state.is_initialized(&root, budget)?);
        state.deinitialize(&union, budget)?;
        assert!(!state.is_initialized(&root, budget)?);
        assert!(state.is_initialized(&sibling, budget)?);
        assert!(state.facts.len() <= 3);
        joined.discard(budget)?;
        partial.discard(budget)?;
        state.discard(budget)?;
        word.discard(budget)?;
        byte.discard(budget)?;
        union.discard(budget)?;
        sibling.discard(budget)?;
        root.discard(budget)?;
        layouts.release(budget)?;
        assert_eq!(budget.storage(), floor);
        Ok(())
    });
}

#[test]
fn overlapping_union_alternative_mutation_preserves_explicit_disjoint_payload() {
    run_union_extended(|instances, budget| {
        let floor = budget.storage();
        let layouts = SourceStorageLayoutsV29::new(instances.owner(), &[UNION_PAIR_BYTE], budget)?;
        let root = layouts.root_subobject(instances.owner(), UNION_PAIR_BYTE, budget)?;
        let pair = layouts.project_step(&root, SubobjectStep::Field(0), budget)?;
        let alias = layouts.project_step(&root, SubobjectStep::Field(1), budget)?;
        let head = layouts.project_step(&pair, SubobjectStep::Field(0), budget)?;
        let tail = layouts.project_step(&pair, SubobjectStep::Field(1), budget)?;
        let mut state = SourceStorageStateV29::new(
            &layouts,
            instances,
            instances.root(),
            local_for(UNION_PAIR_BYTE),
            budget,
        )?;
        state.initialize(&root, budget)?;
        state.initialize(&tail, budget)?;
        assert!(!state.is_initialized(&root, budget)?);
        assert!(state.is_initialized(&tail, budget)?);
        state.initialize(&alias, budget)?;
        assert!(state.is_initialized(&root, budget)?);
        assert!(state.is_initialized(&tail, budget)?);
        state.deinitialize(&alias, budget)?;
        assert!(!state.is_initialized(&root, budget)?);
        assert!(!state.is_initialized(&head, budget)?);
        assert!(state.is_initialized(&tail, budget)?);
        assert!(state.bytes_initialized(tail.range, budget)?);
        state.initialize(&head, budget)?;
        assert!(state.is_initialized(&pair, budget)?);
        assert!(state.is_initialized(&root, budget)?);
        state.discard(budget)?;
        head.discard(budget)?;
        tail.discard(budget)?;
        alias.discard(budget)?;
        pair.discard(budget)?;
        root.discard(budget)?;
        layouts.release(budget)?;
        assert_eq!(budget.storage(), floor);
        Ok(())
    });
}

#[test]
fn sparse_union_alias_mutation_does_not_expand_huge_array() {
    run_union_extended(|instances, budget| {
        let floor = budget.storage();
        let layouts = SourceStorageLayoutsV29::new(instances.owner(), &[UNION_HUGE_WORD], budget)?;
        let root = layouts.root_subobject(instances.owner(), UNION_HUGE_WORD, budget)?;
        let array = layouts.project_step(&root, SubobjectStep::Field(0), budget)?;
        let word = layouts.project_step(&root, SubobjectStep::Field(1), budget)?;
        let last = layouts.project_step(
            &array,
            SubobjectStep::Element(u64::from(u32::MAX) - 1),
            budget,
        )?;
        let mut state = SourceStorageStateV29::new(
            &layouts,
            instances,
            instances.root(),
            local_for(UNION_HUGE_WORD),
            budget,
        )?;
        state.initialize(&last, budget)?;
        state.initialize(&word, budget)?;
        assert!(state.is_initialized(&root, budget)?);
        state.deinitialize(&word, budget)?;
        assert!(!state.is_initialized(&root, budget)?);
        assert!(!state.is_initialized(&array, budget)?);
        assert!(state.is_initialized(&last, budget)?);
        assert!(state.facts.len() <= 2);
        assert!(layouts.rows(instances.owner(), budget).unwrap().len() < 32);
        state.discard(budget)?;
        last.discard(budget)?;
        word.discard(budget)?;
        array.discard(budget)?;
        root.discard(budget)?;
        layouts.release(budget)?;
        assert_eq!(budget.storage(), floor);
        Ok(())
    });
}
