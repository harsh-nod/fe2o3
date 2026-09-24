//! Actual-source owner only. Nothing is admitted from the files written here.
use super::*;
use fe2o3_kernel_ir::OperationKind;
use fe2o3_mir_model::semantic_mir_v1::SemanticMirWireVersionV1;

pub(super) fn observe(
    mut target: crate::production_pipeline::AuthenticatedPhysicalEntryTargetModuleV20,
    feature: &str,
    output: &Path,
) -> Result<Value, String> {
    let abi_controls = target.qualify_actual_abi_controls_v20();
    let checked = target.checked();
    assert!(!checked.grants_artifact_or_launch_authority());
    let executable = checked.executable();
    let semantic = checked.semantic_ssa().source_semantic();
    assert_eq!(semantic.wire_version(), SemanticMirWireVersionV1::V37);
    let [kernel] = executable.module().kernels.as_slice() else {
        panic!("one actual kernel")
    };
    let [function] = executable.module().functions.as_slice() else {
        panic!("one actual function")
    };
    assert_eq!(kernel.entry, function.id);
    let source_root = semantic.roots()[0];
    let source_function = &semantic.functions()[source_root.index() as usize];
    let entry_symbol = function.id.as_str().to_owned();
    assert_eq!(
        source_function
            .kernel_entry()
            .unwrap()
            .export_symbol()
            .as_bytes(),
        entry_symbol.as_bytes()
    );
    let body = function.body.as_ref().unwrap();
    let OperationKind::Gfx942PhysicalEntryDeclaration(declaration) =
        body.blocks[0].operations[0].kind
    else {
        panic!("actual physical declaration")
    };
    let diamond = feature != FEATURES[0];
    assert_eq!(declaration.block_count, if diamond { 4 } else { 1 });
    assert_eq!(
        declaration.native_instruction_count,
        if diamond { 25 } else { 21 }
    );
    let formal = checked.memory_obligations();
    assert_eq!(formal.canonical_identity(), executable.identity().digest());
    assert_eq!(formal.output().bounds_requirements().len(), 1);
    assert_eq!(
        formal.output().bounds_requirements()[0].minimum_byte_len(),
        Some(512)
    );
    assert_eq!(formal.kernarg_reads().len(), 6);
    assert_eq!(formal.kernarg_abi().minimum_bytes(), 32);
    assert_eq!(formal.kernarg_abi().alignment(), 8);
    assert!(formal.kernarg_abi().requires_immutable_kernarg());
    assert_eq!(
        formal.kernarg_abi().disjoint_output(),
        formal.store().allocation()
    );
    let reads = formal.kernarg_reads().iter().map(|read| json!({
        "block": read.location().block.0, "operation": read.location().operation_index,
        "source_occurrence": read.source_site().occurrence,
        "offset": read.byte_offset(), "width": read.byte_width(),
        "ready_block": read.ready_at().block.0, "ready_operation": read.ready_at().operation_index,
        "ready_source_occurrence": read.ready_source_site().occurrence,
    })).collect::<Vec<_>>();
    // Reuse the existing real KIR simulator and the already-reviewed CPU oracle;
    // no second interpreter, numeric address synthesis or native observation.
    let cpu_cases =
        super::super::gfx942_physical_entry_qualification_v20_tests::observation::observe_cpu(
            executable, diamond,
        );
    assert_eq!(cpu_cases, 192);
    let canonical = executable.canonical_bytes().to_vec();
    let canonical_identity = super::super::lower_hex_v1(executable.identity().digest());
    let semantic_identity = super::super::lower_hex_v1(semantic.semantic_sha256().as_bytes());
    let evidence_identity = digest(checked.middle_end_evidence().canonical_bytes());
    let llvm = target.llvm_ir().to_owned();
    let conditions = target.unresolved_abi_conditions();
    assert!(conditions.contains("remain runtime obligations"));
    assert!(llvm.contains("define amdgpu_kernel") && llvm.contains("asm sideeffect"));
    assert!(llvm.contains("unreachable") && !llvm.contains("ret void"));
    let prepared =
        crate::production_worker_handoff::prepare_physical_entry_worker_handoff_v20(target)
            .map_err(|error| error.to_string())?;
    assert_eq!(prepared.unresolved_abi_conditions(), conditions);
    let (handoff, descriptor) = prepared
        .into_inert_for_extraction()
        .map_err(|error| error.to_string())?;
    assert_eq!(handoff.target().to_string(), "gfx942:xnack-");
    assert!(!handoff.authenticates_compiler_origin());
    assert!(!handoff.grants_compiler_authority());
    assert!(!descriptor.authenticates_compiler_origin());
    assert!(!descriptor.grants_launch_authority());
    assert_eq!(descriptor.table().kernels().len(), 1);
    assert_eq!(
        descriptor.table().kernels()[0].entry_name().as_str(),
        entry_symbol
    );
    assert_eq!(
        descriptor.table().canonical_code_object_digest().as_bytes(),
        &[0; 32]
    );
    let worker_llvm = std::str::from_utf8(handoff.module_bytes()).unwrap();
    assert!(worker_llvm.starts_with(&llvm));
    assert!(worker_llvm[llvm.len()..].contains(".fe2o3.kd.v1"));
    let decoded =
        fe2o3_compiler_ffi::CompilerModuleHandoffV2::decode(handoff.canonical_bytes()).unwrap();
    assert_eq!(decoded.canonical_bytes(), handoff.canonical_bytes());
    let decoded_descriptor =
        fe2o3_compiler_ffi::CompilerDescriptorSourceV1::decode(descriptor.canonical_bytes())
            .unwrap();
    assert_eq!(decoded_descriptor, descriptor);
    fs::create_dir(output).map_err(|error| error.to_string())?;
    for (name, bytes, cap) in [
        ("canonical-v20.bin", canonical.as_slice(), 1024 * 1024),
        ("canonical.ll", llvm.as_bytes(), 64 * 1024),
        ("worker.ll", handoff.module_bytes(), 1024 * 1024),
        ("handoff-v2.bin", handoff.canonical_bytes(), 1024 * 1024),
        (
            "descriptor-v1.bin",
            descriptor.canonical_bytes(),
            1024 * 1024,
        ),
    ] {
        super::super::publish_new_inert_output(&output.join(name), bytes, cap, name)?;
    }
    Ok(json!({
        "stage": "actual_source_mir37_checked_kir20_cpu_normal_inert_handoff",
        "semantic_sha256": semantic_identity, "canonical_identity": canonical_identity,
        "canonical_bytes_sha256": digest(&canonical), "middle_end_evidence_sha256": evidence_identity,
        "entry_symbol": entry_symbol, "authored_blocks": declaration.block_count,
        "native_instruction_count": declaration.native_instruction_count,
        "same_owner_source_canonical_descriptor_entry": true,
        "kernarg_reads": reads, "kernarg_minimum_bytes": 32, "kernarg_alignment": 8,
        "output_launch_minimum_bytes": 512, "runtime_abi_conditions": conditions,
        "runtime_abi_conditions_discharged": false, "actual_owner_abi_negative_controls": abi_controls, "abi_resource_denial_controls": 4,
        "cpu_cases": cpu_cases, "cpu_grids": [64,128], "cpu_tail_lengths": [0,1,63,64,65,127,128,129],
        "cpu_selectors": [0,1,u32::MAX], "canaries_unchanged": true,
        "llvm_sha256": digest(llvm.as_bytes()), "handoff_sha256": digest(handoff.canonical_bytes()),
        "descriptor_sha256": digest(descriptor.canonical_bytes()), "normal_worker_preparation": true,
        "native_llvm_executed": false, "hardware_observed": false,
        "protected_finalizer_admitted": false, "grants_artifact_or_launch_authority": false,
    }))
}
