fn emit_finite_domain_program_v62(out: &mut Writer<'_, '_>) -> Result<()> {
    let aggregate = include_str!("original_semantic_mir_source_aggregate_values_v42.vrs");
    let marker = "spec fn invocation_source_path_prefix_v42(";
    let prefix = aggregate
        .split_once(marker)
        .expect("actual source prefix predicate")
        .1
        .split_once("\n}\n")
        .expect("complete prefix predicate body")
        .0;
    emit!(
        out,
        "use vstd::prelude::*;\nverus! {{\n{}\n{marker}{prefix}\n}}\n{}\n}}\n",
        super::super::byte_memory_v30::BYTE_MEMORY_V30,
        include_str!("mixed_optimizer_verus_domains_v62_tests.vrs")
    );
    Ok(())
}

#[test]
fn finite_domain_compatibility_program_retains_equations_and_private_parent_import() {
    let source = run(37, LIMIT, LIMIT, emit_finite_domain_program_v62)
        .0
        .unwrap();
    assert_eq!(source.matches("proof fn ").count(), 25);
    for name in [
        "finite_filter_domain_v62",
        "byte_end_frame_domain_v62",
        "relocation_filter_domain_v62",
        "relocation_copy_domain_v62",
        "enum_fields_domain_v62",
        "aggregate_suffix_domain_v62",
        "aggregate_replace_domain_v62",
        "private_parent_equation_v62",
        "indexed_match_conjunction_v62",
        "byte_execution_copied_record_has_no_slot_authority_v178",
        "byte_execution_departed_frame_has_no_authority_v178",
        "byte_execution_erased_slot_cannot_revive_resident_reference_v178",
        "byte_execution_suspended_context_refuses_second_child_v178",
        "byte_execution_scope_end_requires_current_reciprocal_parent_v178",
        "byte_execution_unknown_operation_is_not_neutral_v178",
        "byte_execution_scope_end_never_leaves_receiver_authority_v178",
        "byte_execution_all_steps_preserve_byte_storage_and_dynamic_frames_v178",
        "byte_execution_reissued_slot_increments_retained_epoch_v178",
        "byte_execution_new_frame_cannot_recreate_old_frame_identity_v178",
    ] {
        assert_eq!(
            source.matches(&format!("proof fn {name}(")).count()
                + source.matches(&format!("proof fn {name}<")).count(),
            1
        );
    }
    assert!(source.contains(super::super::byte_memory_v30::BYTE_MEMORY_V30));
    assert!(source.contains("use super::byte_scalar_type_v57 as imported_predicate;"));
    assert!(!source.contains("open spec fn"));
    assert!(!source.contains("assume("));
    assert!(!source.contains("new_assuming_finite"));
}

#[test]
fn finite_domain_compatibility_source_keeps_all_twenty_three_finite_maps() {
    let sources = [
        (super::super::byte_memory_v30::BYTE_MEMORY_V30, 3),
        (
            include_str!("original_semantic_mir_source_logical_locals_v38.vrs"),
            7,
        ),
        (
            include_str!("original_semantic_mir_invocation_source_frames_v36.rs"),
            2,
        ),
        (
            include_str!("original_semantic_mir_invocation_bytes_v36.rs"),
            1,
        ),
        (
            include_str!("original_semantic_mir_source_aggregate_values_v42.vrs"),
            3,
        ),
        (
            include_str!("original_semantic_mir_source_enum_values_v47.vrs"),
            1,
        ),
        (
            include_str!("original_semantic_mir_source_product_values_v282.vrs"),
            6,
        ),
    ];
    let mut count = 0;
    for (source, expected) in sources {
        assert_eq!(source.matches("Map::new(").count(), expected);
        for domain in source.split("Map::new(").skip(1) {
            assert!(!domain.trim_start().starts_with('|'));
        }
        assert!(!source.contains("Set::new("));
        assert!(!source.contains("new_assuming_finite"));
        count += expected;
    }
    assert_eq!(count, 23);
    let logical = include_str!("original_semantic_mir_source_logical_locals_v38.vrs");
    assert!(logical.contains("execution_references: Map::new(logical.execution_references.dom().filter(|i: int| !(begin <= i < end)),\n            |i: int| logical.execution_references[i])"));
    assert!(logical.contains("products: Map::new(logical.products.dom().filter(|i: int| !(begin <= i < end)),\n            |i: int| logical.products[i])"));
    let aggregate = include_str!("original_semantic_mir_source_aggregate_values_v42.vrs");
    assert!(aggregate.contains(".filter(|key: Seq<int>| invocation_source_path_prefix_v42(path, key))\n                .map(|key: Seq<int>| key.subrange(path.len() as int, key.len() as int))"));
    assert!(aggregate.contains(".union(value.leaves.dom().map(|suffix: Seq<int>| path + suffix))"));
}

#[test]
fn generated_indexed_patterns_use_spec_aware_match_syntax() {
    let unit = run(37, LIMIT, LIMIT, |out| {
        emit_value_type(&Type::Unit, FormalIndexWidth::Bits64, "values[index]", out)
    })
    .0
    .unwrap();
    assert_eq!(
        unit,
        "(match values[index] { MemoryValueV30::Unit => true, _ => false })"
    );
    let control = include_str!("mixed_optimizer_byte_control_v30.rs");
    assert!(control.contains("let control_valid = match done.values[{selector}]"));
    assert!(!control.contains("matches!(done.values["));
    let source = include_str!("original_semantic_mir_invocation_source_bytes_v36.rs");
    assert!(source.contains("match source.machine.values[local]"));
    assert!(!source.contains("matches!(source.machine.values["));
    let products = include_str!("original_semantic_mir_source_product_values_v282.vrs");
    assert!(products.contains("match value.components[path] {"));
    assert!(products.contains("match components[path] {"));
    assert!(!products.contains("matches!(value.components["));
    assert!(!products.contains("matches!(components["));
    let vocabulary = super::super::byte_memory_v30::BYTE_MEMORY_V30;
    assert!(vocabulary.contains("match memory.live[pointer.allocation].bytes[i]"));
    assert!(vocabulary.contains("match object.bytes[offset + i]"));
}

#[test]
fn generated_product_execution_roles_use_spec_integer_comparisons() {
    let source = run(37, LIMIT, LIMIT, |out| {
        emit!(
            out,
            "{}",
            include_str!("original_semantic_mir_source_product_values_v282.vrs")
        );
        Ok(())
    })
    .0
    .unwrap();
    let payload = source
        .split_once("spec fn invocation_source_product_execution_payload_v282(")
        .expect("actual Product execution payload predicate")
        .1
        .split_once("\nspec fn invocation_source_product_execution_pair_v282(")
        .expect("complete Product execution payload predicate")
        .0;
    assert_eq!(
        payload,
        r#"
    source: InvocationSourceByteStateV36, value: InvocationSourceAggregateV42,
) -> bool {
    invocation_source_aggregate_complete_v42(value)
        && match source.machine.frames.execution {
            Some(execution) => byte_execution_well_formed_v37(execution)
                && match invocation_source_product_atom_kind_v282(value.source_type) {
                    InvocationSourceProductAtomKindV282::ExecutionAggregate(role) =>
                        if role == 0 {
                            invocation_source_context_shape_v161(value.source_type)
                                && (forall|i: int| 0 <= i < 5 ==> value.leaves[seq![i]] == MemoryValueV30::Unit)
                        } else if role == 1 {
                            invocation_source_workgroup_shape_v168(value.source_type)
                                && value.leaves[seq![0int]] == MemoryValueV30::Scalar(invocation_source_workgroup_size_v168(execution))
                                && value.leaves[seq![1int]] == MemoryValueV30::Scalar(invocation_source_workgroup_rank_v168(execution))
                                && (forall|i: int| 0 <= i < 3 ==> value.leaves[seq![2int, i]] == MemoryValueV30::Unit)
                                && value.leaves[seq![3int]] == MemoryValueV30::Unit
                        } else { false },
                    _ => false,
                },
            None => false,
        }
}
"#
    );
    assert!(!source.contains("ExecutionAggregate(0) =>"));
    assert!(!source.contains("ExecutionAggregate(1) =>"));
}

#[test]
fn finite_domain_compatibility_program_has_exact_and_one_short_resources() {
    let (source, work, peak) = run(37, LIMIT, LIMIT, emit_finite_domain_program_v62);
    let source = source.unwrap();
    let (exact, exact_work, exact_peak) = run(37, work, peak, emit_finite_domain_program_v62);
    assert_eq!(exact.unwrap(), source);
    assert_eq!((exact_work, exact_peak), (work, peak));
    assert!(
        matches!(run(37, work - 1, peak, emit_finite_domain_program_v62).0,
        Err(Error::Resource(Resource::Work(error))) if error.limit() == work - 1)
    );
    assert!(
        matches!(run(37, work, peak - 1, emit_finite_domain_program_v62).0,
        Err(Error::Resource(Resource::Storage(error))) if error.limit() == peak - 1)
    );
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires the installed root-owned pinned functional-refinement runtime"]
fn protected_finite_map_domains_and_private_parent_import_preserve_exact_equations() {
    use crate::{CanonicalGeneratedVerusProofInputV3, FunctionalRefinementVerusRuntimeLeaseV1};
    use std::time::{Duration, Instant};
    let source = run(37, LIMIT, LIMIT, emit_finite_domain_program_v62)
        .0
        .unwrap();
    let source = CanonicalGeneratedVerusProofInputV3::new(source.into_bytes())
        .expect("canonical complete finite-domain equation program");
    let runtime = FunctionalRefinementVerusRuntimeLeaseV1::open_pinned_contexts_v2(
        "/opt/fe2o3/verus-runtime-v2/functional-refinement-0.2026.08.02-b677dd5",
    )
    .expect("requires the actual public pinned-runtime lease");
    runtime
        .revalidate()
        .expect("revalidate before domain proof");
    let mut attempt = runtime
        .begin_attempt()
        .expect("acquire domain proof attempt");
    let output = runtime
        .execute_generated_rust_verify(
            &mut attempt,
            &source,
            Instant::now() + Duration::from_secs(120),
            16 * 1024,
        )
        .expect("execute exact domain equations through the protected runtime");
    runtime.revalidate().expect("revalidate after domain proof");
    attempt.complete().expect("complete domain proof attempt");
    crate::functional_refinement_receipt_v2::validate_proved_output(&output)
        .expect("all finite-domain and private-import equations must genuinely verify");
}
