use std::collections::BTreeSet;

const OBSERVED: &str = include_str!("original_semantic_mir_observed_effects_v39.vrs");

fn body(name: &str) -> &str {
    OBSERVED
        .split_once(&format!("spec fn {name}"))
        .unwrap()
        .1
        .split("spec fn ")
        .next()
        .unwrap()
}

#[test]
fn original_mir_effect_projection_exhaustively_matches_the_shared_effect_enum() {
    let source = include_str!("mixed_optimizer_byte_memory_v30.rs");
    let fields = source
        .split_once("enum MemoryOperationEffectV30 {")
        .unwrap()
        .1
        .split_once("\n}\n")
        .unwrap()
        .0;
    let declaration: syn::ItemEnum =
        syn::parse_str(&format!("enum MemoryOperationEffectV30 {{{fields}\n}}")).unwrap();
    let expected: BTreeSet<_> = declaration
        .variants
        .iter()
        .map(|variant| variant.ident.to_string())
        .collect();
    let function: syn::ItemFn = syn::parse_str(&format!(
        "fn invocation_project_effect_v39{}",
        body("invocation_project_effect_v39")
    ))
    .unwrap();
    let [syn::Stmt::Expr(syn::Expr::Match(expression), None)] = function.block.stmts.as_slice()
    else {
        panic!("effect projection is one exhaustive match");
    };
    fn alternatives(pattern: &syn::Pat, output: &mut BTreeSet<String>) {
        match pattern {
            syn::Pat::Or(or) => {
                for case in &or.cases {
                    alternatives(case, output);
                }
            }
            syn::Pat::Path(path) => {
                assert_eq!(path.path.segments.len(), 2);
                assert_eq!(path.path.segments[0].ident, "MemoryOperationEffectV30");
                assert!(output.insert(path.path.segments[1].ident.to_string()));
            }
            syn::Pat::Struct(pattern) => {
                assert_eq!(pattern.path.segments.len(), 2);
                assert_eq!(pattern.path.segments[0].ident, "MemoryOperationEffectV30");
                assert!(output.insert(pattern.path.segments[1].ident.to_string()));
            }
            _ => panic!("top-level wildcard or non-enum effect projection"),
        }
    }
    let mut actual = BTreeSet::new();
    for arm in &expression.arms {
        assert!(arm.guard.is_none());
        alternatives(&arm.pat, &mut actual);
    }
    assert_eq!(actual, expected);
    assert!(actual.contains("Copy"));
    assert!(actual.contains("TagRead"));
}

#[test]
fn original_mir_effect_observations_retain_read_then_write_intermediate_source_states() {
    let statement = body("invocation_source_statement_observations_v39");
    for required in [
        "invocation_source_byte_evaluate_v36(observation.before, value, bits",
        "observation.before, evaluated.source)",
        "invocation_source_byte_address_v36(evaluated.source, access",
        "reads + invocation_source_observe_effects_v39(writes, evaluated.source, observation.after)",
        "!observation.before.machine.valid || !observation.after.machine.valid",
    ] {
        assert!(statement.contains(required), "{required}");
    }
    let actual = body("invocation_actual_observations_v39");
    assert!(actual.contains("!byte_observation_snapshots_valid_v39(head)"));
    assert!(actual.contains("MemoryOperationObservationV30 { effect, ..head }"));
    assert!(!actual.contains("before:"));
    assert!(!actual.contains("after:"));
}

#[test]
fn original_mir_effect_relation_uses_nominal_payloads_and_independent_copy_snapshots() {
    let relation = body("invocation_observed_effect_related_v39");
    for required in [
        "source.before.machine.valid && source.after.machine.valid",
        "byte_observation_snapshots_valid_v39(target)",
        "invocation_value_related_v36(sv, tv, before_map,\n                    source.before.machine.memory, target.before.memory)",
        "source.before.machine.memory, ss, sd, sw, ssa, sda, sn",
        "target.before.memory, ts, td, tw, tsa, tda, tn",
        "target.before.memory, ss, ts, sw, before_map)",
        "target.after.memory, sd, td, sw, after_map)",
        "MemoryOperationEffectV30::TagRead { .. }, _) => false",
    ] {
        assert!(relation.contains(required), "{required}");
    }
    let copied = body("invocation_copy_windows_related_v39");
    for required in [
        "source.live[original.allocation].initialized[original.byte_offset + i]",
        "target.live[actual.allocation].initialized[actual.byte_offset + i]",
        "invocation_byte_token_related_v37",
        "source_complete == target_complete",
        "invocation_relocation_related_v37",
        "target.live[actual.allocation].bytes[actual.byte_offset + i], map, source, target)",
        "target_cells[actual.byte_offset + i], map, source, target)",
    ] {
        assert!(copied.contains(required), "{required}");
    }
    assert!(!copied.contains("write_epochs"));
    assert!(!copied.contains("write_clock"));
    assert!(!copied.contains("byte_load_v30"));
    assert!(!relation.contains("source.effect == target.effect"));
}

#[test]
fn original_mir_native_provenance_census_rejects_private_payloads_not_only_live_keys() {
    let native = include_str!("original_semantic_mir_native_provenance_v39.vrs");
    for required in [
        "MemoryValueV30::Pointer(pointer) => !invocation_private_allocation_v36(pointer.allocation)",
        "MemoryValueV30::Slice(slice) => !invocation_private_allocation_v36(slice.pointer.allocation)",
        "0 <= i < arguments.len()",
        "invocation_native_live_allocation_v77(allocation)",
        "0 <= i < object.bytes.len() && object.initialized[i]",
        "MemoryByteV37::PointerFragment { pointer, .. }",
        "object.relocations.contains_key(at)",
        "!invocation_private_allocation_v36(object.relocations[at].pointer.allocation)",
    ] {
        assert!(native.contains(required), "{required}");
    }
    for forbidden in [
        "source.valid",
        "source_ready",
        "generation ==",
        "write_epochs",
    ] {
        assert!(!native.contains(forbidden), "{forbidden}");
    }
    let laws = include_str!("original_semantic_mir_native_provenance_laws_v39.vrs");
    assert_eq!(laws.matches("proof fn ").count(), 6);
    assert!(!laws.contains("assume("));
    assert!(!laws.contains("external_body"));
}

#[test]
fn original_mir_native_live_generation_domain_covers_unsigned_runtime_generations() {
    let native = include_str!("original_semantic_mir_native_provenance_v39.vrs");
    let function = native
        .split_once("spec fn invocation_native_live_allocation_v77")
        .unwrap()
        .1
        .split("spec fn ")
        .next()
        .unwrap();
    assert!(
        function.contains("MemoryAllocationV30::External { generation, .. } => 0 <= generation")
    );
    assert!(function.contains("MemoryAllocationV30::Private { .. } => false"));
    assert!(!function.contains("frames"));
    assert!(!function.contains("source_ready"));
    assert!(!function.contains("generation <="));

    let runtime = include_str!("../../fe2o3-kernel-ir/src/memory_safety_v2.rs");
    let runtime = syn::parse_file(runtime).unwrap();
    let declarations: Vec<_> = runtime
        .items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Struct(item) if item.ident == "ProvenanceV2" => Some(item),
            _ => None,
        })
        .collect();
    let [parsed] = declarations.as_slice() else {
        panic!("one runtime provenance definition");
    };
    let generation = parsed
        .fields
        .iter()
        .find(|field| {
            field
                .ident
                .as_ref()
                .is_some_and(|name| name == "generation")
        })
        .unwrap();
    assert!(matches!(&generation.ty, syn::Type::Path(ty) if ty.path.is_ident("u64")));
}

#[test]
fn original_mir_legacy_source_effect_helpers_have_no_silent_event_wildcards() {
    let legacy = super::INVOCATION_EFFECTS_V36;
    let statement = legacy
        .split_once("spec fn invocation_source_statement_effects_v36")
        .unwrap()
        .1
        .split("spec fn ")
        .next()
        .unwrap();
    assert!(!statement.contains("Some(_)"));
    assert!(!statement.contains("_ =>"));
    for variant in [
        "Scalar",
        "WitnessBorrow",
        "Pointer",
        "Transfer",
        "Address",
        "Deinitialize",
        "StorageLive",
        "StorageDead",
    ] {
        assert!(statement.contains(&format!("InvocationSourceByteEventV36::{variant}")));
    }
    let operands = legacy
        .split_once("spec fn invocation_source_operands_effects_v36")
        .unwrap()
        .1
        .split("spec fn ")
        .next()
        .unwrap();
    assert!(!operands.contains("_ =>"));
}

#[test]
fn original_execution_loan_effect_projection_is_exhaustive_and_checks_transition() {
    let source = include_str!("original_semantic_mir_invocation_source_bytes_v36.rs");
    let fields = source
        .split_once("enum InvocationSourceByteEventV36 {")
        .unwrap()
        .1
        .split_once("\n}\n")
        .unwrap()
        .0;
    let declaration: syn::ItemEnum =
        syn::parse_str(&format!("enum InvocationSourceByteEventV36 {{{fields}\n}}")).unwrap();
    let expected: BTreeSet<_> = declaration
        .variants
        .iter()
        .map(|variant| variant.ident.to_string())
        .collect();
    let statement = super::INVOCATION_EFFECTS_V36
        .split_once("spec fn invocation_source_statement_effects_v36")
        .unwrap()
        .1
        .split("spec fn ")
        .next()
        .unwrap();
    let function: syn::ItemFn = syn::parse_str(&format!(
        "fn invocation_source_statement_effects_v36{statement}"
    ))
    .unwrap();
    let [syn::Stmt::Expr(syn::Expr::If(condition), None)] = function.block.stmts.as_slice() else {
        panic!("statement effects must first check before/after validity");
    };
    let Some((_, alternative)) = &condition.else_branch else {
        panic!("valid statement branch");
    };
    let syn::Expr::Block(alternative) = alternative.as_ref() else {
        panic!("valid statement effects block");
    };
    let [syn::Stmt::Expr(syn::Expr::Match(events), None)] = alternative.block.stmts.as_slice()
    else {
        panic!("one exhaustive event match");
    };
    fn alternatives(pattern: &syn::Pat, output: &mut BTreeSet<String>, absent: &mut usize) {
        match pattern {
            syn::Pat::Or(or) => {
                for case in &or.cases {
                    alternatives(case, output, absent);
                }
            }
            syn::Pat::Path(path) if path.path.is_ident("None") => *absent += 1,
            syn::Pat::TupleStruct(some) if some.path.is_ident("Some") => {
                assert_eq!(some.elems.len(), 1);
                let path = match &some.elems[0] {
                    syn::Pat::Path(pattern) => &pattern.path,
                    syn::Pat::TupleStruct(pattern) => &pattern.path,
                    syn::Pat::Struct(pattern) => &pattern.path,
                    _ => panic!("event wildcard or non-variant pattern"),
                };
                assert_eq!(path.segments.len(), 2);
                assert_eq!(path.segments[0].ident, "InvocationSourceByteEventV36");
                assert!(output.insert(path.segments[1].ident.to_string()));
            }
            _ => panic!("top-level wildcard or non-event effect pattern"),
        }
    }
    let mut actual = BTreeSet::new();
    let mut absent = 0;
    for arm in &events.arms {
        assert!(arm.guard.is_none());
        alternatives(&arm.pat, &mut actual, &mut absent);
    }
    assert_eq!(actual, expected);
    assert_eq!(absent, 1);
    let loan = statement
        .split_once("Some(InvocationSourceByteEventV36::ExecutionLoan(event)) => {")
        .unwrap()
        .1
        .split_once("Some(InvocationSourceByteEventV36::")
        .unwrap()
        .0;
    assert!(loan.contains("invocation_source_execution_step_v168(observation.before, event)"));
    assert!(loan.contains("if after == observation.after { seq![] }"));
    assert!(loan.contains("else { seq![MemoryOperationEffectV30::Refused] }"));
}
