//! Live source owner -> unchanged KIR17 -> bounded CPU and diagnostic LLVM.
use super::*;
use fe2o3_mir_model::semantic_mir_v1::{SemanticFunctionRoleV1, SemanticMirWireVersionV1};

fn expected_roster(feature: &str) -> (usize, usize, usize, usize) {
    match feature {
        "ordered-composition-root" => (2, 0, 0, 2),
        "ordered-composition-helper" | "ordered-composition-wrapping" => (1, 1, 1, 1),
        "ordered-composition-two-calls" => (1, 1, 2, 2),
        "ordered-composition-root-helper" => (2, 1, 1, 2),
        "ordered-composition-const-monos" => (2, 2, 2, 2),
        "ordered-composition-scalar-helper" => (1, 1, 1, 1),
        _ => panic!("positive composition roster only"),
    }
}
pub(super) fn observe(
    mut target: crate::production_pipeline::ordered_composition_v1::AuthenticatedOrderedCompositionDiagnosticV1<'_>,
    feature: &str,
    output: &Path,
    started: std::time::Instant,
) -> Result<Value, String> {
    let original_usage = target.resource_usage();
    target
        .with_observation_budget(|materialized, budget| materialized.verify_equivalence(budget))
        .map_err(|e| format!("actual-source qualification replay: {e}"))?;
    let replayed_usage = target.resource_usage();
    assert!(replayed_usage.0 > original_usage.0);
    assert_eq!(replayed_usage.1, original_usage.1);
    assert!(replayed_usage.2 >= original_usage.2);
    let source = target.materialized();
    assert!(!source.grants_artifact_or_launch_authority());
    let executable = source.executable();
    let semantic = source.semantic_ssa().source_semantic();
    assert_eq!(semantic.wire_version(), SemanticMirWireVersionV1::V32);
    assert_eq!(
        target.source_seed().semantic_sha256(),
        semantic.semantic_sha256().as_bytes()
    );
    let composition = source.composition();
    let (definitions, helpers, calls, occurrences) = expected_roster(feature);
    assert_eq!(
        (
            composition.definitions().len(),
            composition.helpers().len(),
            composition.calls().len(),
            composition.occurrences().len()
        ),
        (definitions, helpers, calls, occurrences)
    );
    assert_eq!(source.definitions().count(), definitions);
    assert_eq!(source.occurrences().count(), occurrences);
    let [kernel] = executable.module().kernels.as_slice() else {
        panic!("one source kernel");
    };
    let root = &executable.module().functions[composition.root_function_ordinal() as usize];
    assert_eq!(root.id, kernel.entry);
    assert_eq!(executable.module().functions.len(), 1 + helpers);
    let source_root = &semantic.functions()[semantic.roots()[0].index() as usize];
    assert_eq!(
        source_root
            .kernel_entry()
            .unwrap()
            .export_symbol()
            .as_bytes(),
        root.id.as_str().as_bytes()
    );
    assert_eq!(source_root.abi().source_input_types().len(), 4);
    let semantic_helpers = semantic
        .functions()
        .iter()
        .filter(|f| f.role() == SemanticFunctionRoleV1::InternalHelper)
        .collect::<Vec<_>>();
    assert_eq!(semantic_helpers.len(), helpers);
    if feature == "ordered-composition-const-monos" {
        let [a, b] = semantic_helpers.as_slice() else {
            panic!("two actual monomorphized helpers");
        };
        assert_eq!(a.item_definition_identity(), b.item_definition_identity());
        assert_ne!(a.identity(), b.identity());
        assert_ne!(a.monomorphization_identity(), b.monomorphization_identity());
        assert_ne!(
            a.const_generic_arguments_identity(),
            b.const_generic_arguments_identity()
        );
    }
    if feature == "ordered-composition-two-calls" {
        assert_eq!(
            composition.calls()[0].callee(),
            composition.calls()[1].callee()
        );
        assert_ne!(composition.calls()[0].key(), composition.calls()[1].key());
        assert_eq!(
            composition.occurrences()[0].definition(),
            composition.occurrences()[1].definition()
        );
        assert_ne!(
            composition.occurrences()[0].incoming_call(),
            composition.occurrences()[1].incoming_call()
        );
    }
    if feature == "ordered-composition-scalar-helper" {
        assert!(composition.occurrences()[0].incoming_call().is_none());
        assert_eq!(
            composition.definitions()[0].site().function_ordinal(),
            composition.root_function_ordinal()
        );
    }
    let canonical = executable.canonical_bytes();
    assert!(canonical.len() <= 256 * 1024);
    let canonical_hash = digest(canonical);
    let llvm = target.llvm_ir();
    assert!(llvm.len() <= 256 * 1024 && llvm.contains("define amdgpu_kernel"));
    assert!(llvm.contains("asm sideeffect"));
    let cpu = cpu::observe_cpu(executable, feature, started);
    // CPU admission above is a separate observer. The original source phase's
    // work/floor and actual owner remain retained and cannot be reconstructed.
    assert_eq!(target.resource_usage(), replayed_usage);
    assert_eq!(digest(executable.canonical_bytes()), canonical_hash);
    let (inventory, preflight) = target.source_identities();
    let sites = composition.definitions().iter().map(|d| {
        let s = d.site();
        json!({"definition":d.key().ordinal(),"function":s.function_ordinal(),
            "block":s.block().0,"block_ordinal":s.block_ordinal(),"operation":s.operation_ordinal()})
    }).collect::<Vec<_>>();
    let call_rows = composition
        .calls()
        .iter()
        .map(|c| {
            let s = c.site();
            json!({"call":c.key().ordinal(),"callee":c.callee().ordinal(),
            "function":s.function_ordinal(),"block":s.block().0,"operation":s.operation_ordinal()})
        })
        .collect::<Vec<_>>();
    let occurrence_rows=composition.occurrences().iter().map(|o|json!({
        "occurrence":o.key().ordinal(),"root":o.root_function_ordinal(),
        "incoming_call":o.incoming_call().map(|k|k.ordinal()),"definition":o.definition().ordinal(),
    })).collect::<Vec<_>>();
    timely(started.elapsed(), 300).map_err(str::to_owned)?;
    fs::create_dir(output).map_err(|e| e.to_string())?;
    super::super::publish_new_inert_output(
        &output.join("canonical-v17.bin"),
        canonical,
        256 * 1024,
        "canonical V17",
    )?;
    super::super::publish_new_inert_output(
        &output.join("canonical.ll"),
        llvm.as_bytes(),
        256 * 1024,
        "diagnostic LLVM",
    )?;
    timely(started.elapsed(), 300).map_err(str::to_owned)?;
    Ok(json!({
        "stage":"actual_source_mir32_composition_kir17_cpu_diagnostic",
        "semantic_sha256":super::super::lower_hex_v1(semantic.semantic_sha256().as_bytes()),
        "canonical_identity":super::super::lower_hex_v1(executable.identity().digest()),
        "canonical_bytes_sha256":canonical_hash,"canonical_bytes":canonical.len(),
        "llvm_sha256":digest(llvm.as_bytes()),"llvm_bytes":llvm.len(),
        "entry_symbol":root.id.as_str(),
        "source_inventory_sha256":super::super::lower_hex_v1(&inventory),
        "source_preflight_sha256":super::super::lower_hex_v1(&preflight),
        "definitions":sites,"helper_count":helpers,"calls":call_rows,"occurrences":occurrence_rows,
        "expanded_instruction_count":composition.expanded_instruction_count(),
        "source_ledger_before_replay":original_usage,"source_ledger_after_replay":replayed_usage,
        "cpu":cpu,"cpu_does_not_reset_source_ledger":true,
        "same_item_distinct_const_instances_checked":feature=="ordered-composition-const-monos",
        "normal_ranked_formal_handoff_qualified":false,"native_llvm_executed":false,
        "source_custody_from_files":false,"grants_artifact_or_launch_authority":false,
    }))
}
#[test]
fn composition_roster_distinguishes_definition_call_and_execution() {
    assert_eq!(expected_roster(FEATURES[2]), (1, 1, 2, 2));
    assert_eq!(expected_roster(FEATURES[4]), (2, 2, 2, 2));
    assert_eq!(expected_roster(FEATURES[5]), (1, 1, 1, 1));
}
