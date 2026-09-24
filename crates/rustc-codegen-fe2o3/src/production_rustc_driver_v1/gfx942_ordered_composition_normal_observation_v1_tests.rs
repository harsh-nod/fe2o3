//! Same actual checked owner through ordinary descriptor/worker preparation.
use super::*;
#[path = "gfx942_ordered_composition_normal_native_rows_v1_tests.rs"]
mod native_rows;
pub(super) fn observe(
    target: crate::production_pipeline::ordered_composition_target_v1::AuthenticatedOrderedCompositionTargetModuleV1<'_>,
    feature: &str,
    output: &Path,
) -> Result<Value, String> {
    let checked = target.checked();
    assert!(!checked.grants_artifact_or_launch_authority());
    let source = checked.source_launch();
    assert_eq!(source.roots().len(), 1);
    assert_eq!(source.roots()[0].layout().global_extents(), [128, 1, 1]);
    let owner = checked.composition();
    let roster = (
        owner.definitions().len(),
        owner.helpers().len(),
        owner.calls().len(),
        owner.occurrences().len(),
    );
    assert_eq!(roster, expected_roster(feature));
    let module = checked.executable().module();
    assert_eq!(module.kernels.len(), 1);
    let root = &module.functions[owner.root_function_ordinal() as usize];
    assert_eq!(root.signature.parameters.len(), 4);
    let kernel_symbol = module.kernels[0].id.as_str().to_owned();
    assert_eq!(checked.formal_obligations().accesses().len(), 1);
    assert!(
        checked
            .formal_obligations()
            .inter_invocation_conflicts()
            .is_empty()
    );
    let conditions = checked.launch_envelope_requirements();
    assert_eq!(conditions.len(), 1);
    assert_eq!(conditions[0].parameter_index(), 0);
    assert_eq!(conditions[0].minimum_byte_len(), 512);
    assert!(!conditions[0].requires_initialized_read());
    assert!(conditions[0].requires_write_permission());
    let condition_copy = conditions.to_vec();
    let original_formal = format!("{:?}", checked.formal_obligations());
    assert!(original_formal.len() <= 128 * 1024);
    let dependencies = checked.ranked_dependencies();
    assert!(!dependencies.is_empty());
    for call in owner.calls() {
        assert!(
            dependencies
                .iter()
                .any(|row| row.incoming_call() == Some(call.key()))
        );
    }
    let canonical = checked.executable().canonical_bytes().to_vec();
    let identity = super::super::super::lower_hex_v1(checked.executable().identity().digest());
    let evidence = digest(checked.middle_end_evidence().canonical_bytes());
    let llvm = target.llvm_ir().to_owned();
    assert!(llvm.contains("define amdgpu_kernel") && llvm.contains("asm sideeffect"));
    let native_rows = native_rows::rows(checked);
    let prepared =
        crate::production_worker_handoff::prepare_ordered_composition_worker_handoff_v1(target)
            .map_err(|e| e.to_string())?;
    assert_eq!(
        prepared.checked().launch_envelope_requirements(),
        condition_copy
    );
    assert_eq!(
        format!("{:?}", prepared.checked().formal_obligations()),
        original_formal
    );
    let (handoff, descriptor) = prepared
        .into_inert_for_extraction()
        .map_err(|e| e.to_string())?;
    assert!(!handoff.authenticates_compiler_origin() && !handoff.grants_compiler_authority());
    assert!(!descriptor.authenticates_compiler_origin() && !descriptor.grants_launch_authority());
    assert_eq!(handoff.target().to_string(), "gfx942:xnack-");
    let [kernel] = descriptor.table().kernels() else {
        panic!("actual one kernel");
    };
    assert_eq!(kernel.entry_name().as_str(), kernel_symbol);
    assert_eq!(kernel.arguments().len(), 4);
    assert_eq!(
        [
            kernel.launch().max_grid().x(),
            kernel.launch().max_grid().y(),
            kernel.launch().max_grid().z()
        ],
        [2, 1, 1]
    );
    assert_eq!(kernel.launch().static_shared_memory_bytes(), 0);
    assert_eq!(
        descriptor.table().canonical_code_object_digest().as_bytes(),
        &[0; 32]
    );
    let worker = std::str::from_utf8(handoff.module_bytes()).unwrap();
    let relation = crate::kernel_ir_codegen::exact_ordered_composition_descriptor_extension_v1;
    assert!(relation(&llvm, worker, &descriptor));
    assert!(!relation(&llvm, &format!("{worker}\n"), &descriptor));
    assert!(!relation(&format!("{llvm}\n"), worker, &descriptor));
    assert!(!relation(&llvm, &llvm, &descriptor));
    let decoded =
        fe2o3_compiler_ffi::CompilerModuleHandoffV2::decode(handoff.canonical_bytes()).unwrap();
    assert_eq!(decoded.canonical_bytes(), handoff.canonical_bytes());
    let decoded_descriptor =
        fe2o3_compiler_ffi::CompilerDescriptorSourceV1::decode(descriptor.canonical_bytes())
            .unwrap();
    assert_eq!(decoded_descriptor, descriptor);
    fs::create_dir(output).map_err(|e| e.to_string())?;
    for (name, bytes) in [
        ("canonical-v17.bin", canonical.as_slice()),
        ("canonical.ll", llvm.as_bytes()),
        ("worker.ll", handoff.module_bytes()),
        ("handoff-v2.bin", handoff.canonical_bytes()),
        ("descriptor-v1.bin", descriptor.canonical_bytes()),
    ] {
        super::super::super::publish_new_inert_output(
            &output.join(name),
            bytes,
            4 * 1024 * 1024,
            name,
        )?;
    }
    Ok(json!({"stage":"actual_checked_composition_inert_handoff",
        "native_rows":native_rows,
        "canonical_identity":identity,"canonical_sha256":digest(&canonical),
        "llvm_sha256":digest(llvm.as_bytes()),"handoff_sha256":digest(handoff.canonical_bytes()),
        "descriptor_sha256":digest(descriptor.canonical_bytes()),"middle_end_evidence_sha256":evidence,
        "definition_count":roster.0,"helper_count":roster.1,"call_count":roster.2,"occurrence_count":roster.3,
        "entry":kernel_symbol,"logical_arguments":4,"actual_global_extent":128,
        "original_formal":original_formal,"extra_parameter":0,"extra_minimum_bytes":512,
        "extra_initialized_read":false,"extra_write_permission":true,
        "runtime_conditions_discharged":false,"source_custody_exported":false,
        "exact_descriptor_extension":true,"descriptor_extension_mutations":3,
        "functional_equivalence_claim":false,"native_llvm_executed":false,
        "hardware_observed":false,"protected_authority":false}))
}
pub(super) fn expected_roster(feature: &str) -> (usize, usize, usize, usize) {
    match feature {
        "ordered-composition-root" => (2, 0, 0, 2),
        "ordered-composition-helper" | "ordered-composition-wrapping" => (1, 1, 1, 1),
        "ordered-composition-two-calls" => (1, 1, 2, 2),
        "ordered-composition-root-helper" => (2, 1, 1, 2),
        "ordered-composition-const-monos" => (2, 2, 2, 2),
        "ordered-composition-scalar-helper" => (1, 1, 1, 1),
        _ => panic!("not a positive shape"),
    }
}
