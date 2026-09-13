use super::*;
use crate::production_analysis::pliron_ir_identity::LivePlironStructuralIdentityProviderV1;
use crate::production_analysis::pliron_pass_contract::PlironStructuralIdentityProviderV1;
use dialect_kernel::{
    AccessKindAttr, DIALECT_NAME, IndexConstantOp, RankedViewType, ReturnOp, register_dialect,
};
use pliron::{
    builtin::types::FunctionType, dialect::DialectName, op::Op, operation::verify_operation,
};

fn unlimited() -> ProductionAnalysisResourceLimitsV1 {
    ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX)
}

fn setup() -> Context {
    let mut context = Context::new();
    register_dialect(&mut context, &DialectName::try_new(DIALECT_NAME).unwrap()).unwrap();
    dialect_gpu::register_dialect(&mut context).unwrap();
    dialect_proof::register_dialect(&mut context).unwrap();
    context
}

fn fixture(
    context: &mut Context,
    origin: u64,
    class: u64,
    accesses: usize,
    effects: usize,
    irrelevant: usize,
) -> (FuncOp, Value) {
    let function = FuncOp::new(
        context,
        "provenance_population".try_into().unwrap(),
        FunctionType::get(context, vec![], vec![]),
    );
    let entry = function.get_entry_block(context);
    let view_type = RankedViewType::new(context, 32, true, vec![1]).unwrap();
    let view = RankedViewOp::new_in_space_with_allocation_contract(
        context,
        view_type,
        vec![],
        MemorySpaceAttr::Global,
        origin,
        class,
    )
    .unwrap();
    view.get_operation().insert_at_back(entry, context);
    let zero = IndexConstantOp::new(context, 0);
    zero.get_operation().insert_at_back(entry, context);
    for _ in 0..irrelevant {
        IndexConstantOp::new(context, 1)
            .get_operation()
            .insert_at_back(entry, context);
    }
    for _ in 0..accesses {
        RankedAccessOp::new(
            context,
            AccessKindAttr::Read,
            view.result(context),
            vec![zero.result(context)],
        )
        .unwrap()
        .get_operation()
        .insert_at_back(entry, context);
    }
    for _ in 0..effects {
        AllocationEffectOp::new(
            context,
            AccessKindAttr::Read,
            MemorySpaceAttr::Global,
            12,
            102,
        )
        .unwrap()
        .get_operation()
        .insert_at_back(entry, context);
    }
    ReturnOp::new(context)
        .get_operation()
        .insert_at_back(entry, context);
    (function, view.result(context))
}

#[test]
fn authenticated_subject_census_preserves_repeated_uses_and_skips_unrelated_results() {
    let context = &mut setup();
    let (function, _) = fixture(context, 11, 101, 3, 2, 64);
    verify_operation(function.get_operation(), context).unwrap();
    let capture = LivePlironStructuralIdentityProviderV1::new(context, &function)
        .capture_with_resource_limits_v1(unlimited())
        .ok()
        .unwrap();
    let census = capture.input_census;
    assert_eq!(
        (
            census.operations,
            census.ranked_accesses,
            census.allocation_effects
        ),
        (72, 3, 2)
    );
    let inventory = BoundedPlironFunctionInventoryV1::collect(context, &function).unwrap();
    let analysis = collect_pliron_provenance_alias_with_inventory_v1(context, &inventory).unwrap();
    assert_eq!(analysis.subjects.len(), 5);
    assert_eq!(analysis.views.len(), 1);
    assert_eq!(
        analysis.subjects[3].label,
        "allocation effect at block 0 op 69"
    );
    assert_eq!(
        analysis.subjects[4].label,
        "allocation effect at block 0 op 70"
    );
    for memory_space in [
        MemorySpaceAttr::Private,
        MemorySpaceAttr::Workgroup,
        MemorySpaceAttr::Global,
    ] {
        analysis.validate_space(memory_space).unwrap();
    }
    // O=72, N=5: work=8*72+160*5; retained=152*5+3;
    // validation transient=24*5+2*128+1024+8, peak=2171.
    let bound = preflight_provenance_alias_resource_upper_bound_v1(
        census,
        ProductionAnalysisResourceLimitsV1::new(1376, 2171),
    )
    .unwrap();
    assert_eq!(bound.work_upper_bound(), 1376);
    assert_eq!(bound.retained_storage_upper_bound(), 763);
    assert_eq!(bound.peak_storage_upper_bound(), 2171);
}

#[test]
fn retained_numeric_subject_labels_ignore_unauthenticated_debug_aliases() {
    let context = &mut setup();
    let (function, view) = fixture(context, 11, 101, 2, 0, 0);
    view.set_name(context, Some("initial_alias".try_into().unwrap()));
    verify_operation(function.get_operation(), context).unwrap();
    let mut provider = LivePlironStructuralIdentityProviderV1::new(context, &function);
    let before = provider
        .capture_with_resource_limits_v1(unlimited())
        .ok()
        .unwrap();
    let expected: String = view.id(context).into();
    let bound =
        preflight_provenance_alias_resource_upper_bound_v1(before.input_census, unlimited())
            .unwrap();
    for alias in ["short_alias".to_owned(), "d".repeat(262_144)] {
        view.set_name(context, Some(alias.try_into().unwrap()));
        verify_operation(function.get_operation(), context).unwrap();
        let after = provider
            .capture_with_resource_limits_v1(unlimited())
            .ok()
            .unwrap();
        assert!(
            provider
                .require_exact_identity(&before.snapshot, &after.snapshot)
                .is_ok()
        );
        assert_eq!(after.input_census, before.input_census);
        assert_eq!(
            preflight_provenance_alias_resource_upper_bound_v1(after.input_census, unlimited()),
            Ok(bound)
        );
        let analysis = analyze_pliron_provenance_alias_v1(context, &function).unwrap();
        assert_eq!(analysis.subjects.len(), 2);
        for subject in &analysis.subjects {
            assert_eq!(subject.label, expected);
            assert!(subject.label.capacity() <= 128);
        }
    }
}

#[test]
fn rejected_numeric_subject_label_and_copy_ignore_debug_aliases() {
    let context = &mut setup();
    // Deliberately malformed alias metadata exercises the collector's existing
    // rejection path, not an assertion that this is structurally admitted IR.
    let (function, view) = fixture(context, 0, 7, 1, 0, 0);
    let expected: String = view.id(context).into();
    for alias in ["short_alias".to_owned(), "d".repeat(262_144)] {
        view.set_name(context, Some(alias.try_into().unwrap()));
        let error = analyze_pliron_provenance_alias_v1(context, &function).unwrap_err();
        assert_eq!(
            error,
            PlironProvenanceFailureV1::ClaimedNoAliasWithoutOrigin {
                subject: expected.clone(),
                class: 7,
            }
        );
        let PlironProvenanceFailureV1::ClaimedNoAliasWithoutOrigin { subject, .. } = &error else {
            unreachable!()
        };
        assert!(subject.capacity() <= 128);
        let projected = error.bounded_description_v1();
        assert!(projected.len() <= MAX_PLIRON_PROVENANCE_DIAGNOSTIC_BYTES_V1);
        assert!(projected.contains(&expected));
    }
}

#[test]
fn numeric_and_allocation_site_label_capacity_fits_fixed_payload() {
    let numeric = format!("v{}", u64::MAX);
    assert_eq!(numeric.len(), 21);
    assert!(numeric.capacity() <= 128);
    let location = format!(
        "allocation effect at block {} op {}",
        usize::MAX,
        usize::MAX
    );
    assert!(location.len() <= 71);
    assert!(location.capacity() <= 128);
    let context = &mut setup();
    let (function, view) = fixture(context, 11, 101, 1, 1, 0);
    view.set_name(context, Some("d".repeat(262_144).try_into().unwrap()));
    let label: String = view.id(context).into();
    assert!(label.starts_with('v'));
    assert!(label[1..].bytes().all(|byte| byte.is_ascii_digit()));
    assert!(label.capacity() <= 128);
    let analysis = analyze_pliron_provenance_alias_v1(context, &function).unwrap();
    assert!(
        analysis
            .subjects
            .iter()
            .all(|subject| subject.label.capacity() <= 128)
    );
}

#[test]
fn exact_subject_population_bounds_and_one_under_limits_are_literal() {
    // Columns: total operations, ranked accesses, allocation effects, work,
    // retained, peak. The all-operation scan remains even with zero subjects.
    for (operations, accesses, effects, work, retained, peak) in [
        (23, 0, 0, 184, 3, 3),
        (1, 1, 0, 168, 155, 1467),
        (1, 0, 1, 168, 155, 1467),
        (4, 3, 1, 672, 611, 1995),
        (23, 3, 2, 984, 763, 2171),
    ] {
        for identifier_bytes in [0, 71, usize::MAX] {
            let census = ProductionAnalysisInputCensusV1 {
                operations,
                ranked_accesses: accesses,
                allocation_effects: effects,
                identifier_bytes,
                ..ProductionAnalysisInputCensusV1::default()
            };
            let exact = preflight_provenance_alias_resource_upper_bound_v1(
                census,
                ProductionAnalysisResourceLimitsV1::new(work, peak),
            )
            .unwrap();
            assert_eq!(exact.work_upper_bound(), work);
            assert_eq!(exact.retained_storage_upper_bound(), retained);
            assert_eq!(exact.peak_storage_upper_bound(), peak);
            for (work_limit, storage_limit, resource) in [
                (work - 1, peak, "work upper bound"),
                (work, peak - 1, "peak storage upper bound"),
                (work - 1, peak - 1, "work upper bound"),
            ] {
                assert_eq!(
                    preflight_provenance_alias_resource_upper_bound_v1(
                        census,
                        ProductionAnalysisResourceLimitsV1::new(work_limit, storage_limit),
                    ),
                    Err(ProductionAnalysisResourceLimitV1 {
                        phase: ProductionAnalysisResourcePhaseV1::ProvenanceAlias,
                        resource,
                    })
                );
            }
        }
    }
}

#[test]
fn subject_population_and_full_scan_overflow_fail_closed() {
    for census in [
        ProductionAnalysisInputCensusV1 {
            ranked_accesses: usize::MAX,
            allocation_effects: 1,
            ..ProductionAnalysisInputCensusV1::default()
        },
        ProductionAnalysisInputCensusV1 {
            operations: usize::MAX,
            ..ProductionAnalysisInputCensusV1::default()
        },
    ] {
        assert_eq!(
            preflight_provenance_alias_resource_upper_bound_v1(census, unlimited()),
            Err(provenance_resource_overflow_v1())
        );
    }
}

#[test]
fn subject_cap_and_rejecting_next_publication_are_unchanged() {
    let census = ProductionAnalysisInputCensusV1 {
        operations: 65_537,
        allocation_effects: 65_537,
        ..ProductionAnalysisInputCensusV1::default()
    };
    // N saturates only at the existing subject cap; scan includes the next op.
    let bound = preflight_provenance_alias_resource_upper_bound_v1(census, unlimited()).unwrap();
    assert_eq!(bound.work_upper_bound(), 11_010_056);
    assert_eq!(bound.retained_storage_upper_bound(), 9_961_475);
    assert_eq!(bound.peak_storage_upper_bound(), 11_535_627);
    let mut subjects = Vec::new();
    for index in 0..=MAX_PLIRON_PROVENANCE_SUBJECTS_V1 {
        let subject = SubjectV1 {
            identity: SubjectIdentityV1::AllocationSite(0, index),
            label: String::new(),
            allocation_origin: 0,
            noalias_class: 0,
            memory_space: MemorySpaceAttr::Global,
            signature: None,
            writes: false,
        };
        let result = push_subject(&mut subjects, subject);
        if index < MAX_PLIRON_PROVENANCE_SUBJECTS_V1 {
            result.unwrap();
        } else {
            assert_eq!(
                result,
                Err(PlironProvenanceFailureV1::ResourceLimit {
                    limit: 65_536,
                    actual: 65_537,
                })
            );
            assert_eq!(subjects.len(), 65_536);
        }
    }
}
