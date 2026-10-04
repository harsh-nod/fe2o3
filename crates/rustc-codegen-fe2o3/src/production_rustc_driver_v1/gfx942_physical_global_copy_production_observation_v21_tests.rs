//! Real rustc-derived checked owner; written files are inert diagnostics only.
use super::*;
use fe2o3_kernel_ir::{
    FormalMemoryAccessKind, Gfx942PhysicalGlobalCopyOpcodeV1 as Opcode, OperationKind,
};
use fe2o3_mir_model::semantic_mir_v1::SemanticMirWireVersionV1;

pub(super) fn observe(
    mut target: crate::production_pipeline::AuthenticatedPhysicalGlobalCopyTargetModuleV21,
    feature: &str,
    output: &Path,
) -> Result<Value, String> {
    let abi_controls = target.qualify_actual_abi_controls_v21();
    // The same owned source ledger pays replay plus existing simulator admission.
    // The CPU oracle's small/ragged buffers do NOT discharge production's
    // conservative 512-byte input/output obligations or host alias conditions.
    let cpu_cases = target.observe_with_budget(|checked, budget| {
        checked.verify_equivalence(budget).unwrap();
        super::super::gfx942_physical_global_copy_qualification_v21_tests::observation::observe_cpu(
            checked.executable(),
            budget,
        )
    });
    assert_eq!(cpu_cases, 64);
    let checked = target.checked();
    assert!(!checked.grants_artifact_or_launch_authority());
    let executable = checked.executable();
    let semantic = checked.semantic_ssa().source_semantic();
    assert_eq!(semantic.wire_version(), SemanticMirWireVersionV1::V38);
    let [kernel] = executable.module().kernels.as_slice() else {
        panic!("one actual kernel");
    };
    let [function] = executable.module().functions.as_slice() else {
        panic!("one actual function");
    };
    assert_eq!(kernel.entry, function.id);
    let source_function = &semantic.functions()[semantic.roots()[0].index() as usize];
    let entry_symbol = function.id.as_str().to_owned();
    assert_eq!(
        source_function
            .kernel_entry()
            .unwrap()
            .export_symbol()
            .as_bytes(),
        entry_symbol.as_bytes()
    );
    assert_eq!(source_function.abi().source_input_types().len(), 2);
    assert_eq!(function.signature.parameters.len(), 2);
    let body = function.body.as_ref().unwrap();
    assert_eq!(body.blocks.len(), 1);
    let declaration_operation = &body.blocks[0].operations[0];
    let OperationKind::Gfx942PhysicalGlobalCopyDeclaration(declaration) =
        declaration_operation.kind
    else {
        panic!("actual physical global-copy declaration");
    };
    assert_eq!(declaration.native_instruction_count, 24);
    let register = if feature == FEATURES[0] { 8 } else { 22 };
    assert!(body.blocks[0].operations.iter().any(|operation|matches!(operation.kind,
        OperationKind::Gfx942PhysicalGlobalCopyStep(step) if
            step.instruction.opcode==Opcode::GlobalLoadDword && step.instruction.destination==register)));
    let formal = checked.memory_obligations();
    assert_eq!(formal.canonical_identity(), executable.identity().digest());
    assert_eq!(formal.global().allocations().len(), 2);
    assert_eq!(formal.global().accesses().len(), 2);
    assert_eq!(
        formal.global().accesses()[0].kind(),
        FormalMemoryAccessKind::Read
    );
    assert_eq!(
        formal.global().accesses()[1].kind(),
        FormalMemoryAccessKind::Write
    );
    assert_eq!(formal.global().bounds_requirements().len(), 2);
    assert!(
        formal
            .global()
            .bounds_requirements()
            .iter()
            .all(|row| row.minimum_byte_len() == Some(512))
    );
    assert_eq!(formal.global().runtime_alias_requirements().len(), 1);
    assert_eq!(formal.kernarg_reads().len(), 4);
    assert_eq!(
        formal.kernarg_reads().map(|r| r.byte_offset()),
        [0, 8, 16, 24]
    );
    let runtime = formal.runtime_requirements();
    assert_eq!(
        (
            runtime.minimum_input_bytes(),
            runtime.minimum_output_bytes()
        ),
        (512, 512)
    );
    assert!(runtime.requires_input_readable() && runtime.requires_input_initialized());
    assert!(runtime.requires_output_writable() && runtime.requires_input_output_disjoint());
    assert!(!runtime.grants_runtime_binding_authority());
    let abi = formal.kernarg_abi();
    assert_eq!((abi.minimum_bytes(), abi.alignment()), (32, 8));
    assert!(
        abi.requires_live_kernarg()
            && abi.requires_readable_kernarg()
            && abi.requires_immutable_kernarg()
    );
    assert_eq!(abi.disjoint_output(), runtime.output());
    assert_eq!(
        formal.input_read().access().exec(),
        declaration_operation.results[4].id
    );
    assert_eq!(formal.input_read().result(), formal.output_store().value());
    assert_eq!(
        formal.input_read().access().index(),
        formal.output_store().access().index()
    );
    let reads=formal.kernarg_reads().iter().map(|read|json!({
        "block":read.location().block.0,"operation":read.location().operation_index,
        "source_occurrence":read.source_site().occurrence,"offset":read.byte_offset(),"width":read.byte_width(),
        "ready_operation":read.ready_at().operation_index,"ready_source_occurrence":read.ready_source_site().occurrence,
    })).collect::<Vec<_>>();
    let input_read = json!({
        "allocation":formal.input_read().access().allocation().parameter_index(),
        "operation":formal.input_read().access().location().operation_index,
        "source_occurrence":formal.input_read().access().source_site().occurrence,
        "wait_operation":formal.input_read().ready_at().operation_index,
        "wait_source_occurrence":formal.input_read().ready_source_site().occurrence,
        "full_exec":true,"data_remains_opaque":true,
    });
    let canonical = executable.canonical_bytes().to_vec();
    let canonical_identity = super::super::lower_hex_v1(executable.identity().digest());
    let semantic_identity = super::super::lower_hex_v1(semantic.semantic_sha256().as_bytes());
    let evidence_identity = digest(checked.middle_end_evidence().canonical_bytes());
    let llvm = target.llvm_ir().to_owned();
    let conditions = target.unresolved_abi_conditions();
    for fragment in [
        "full-EXEC",
        "initialized 128-u32 prefix",
        "input/output",
        "output/kernarg",
        "live readable immutable",
        "remain runtime obligations",
    ] {
        assert!(conditions.contains(fragment));
    }
    assert!(llvm.contains("define amdgpu_kernel") && llvm.contains("asm sideeffect"));
    assert!(llvm.contains("unreachable") && !llvm.contains("ret void"));
    let prepared =
        crate::production_worker_handoff::prepare_physical_global_copy_worker_handoff_v21(target)
            .map_err(|error| error.to_string())?;
    assert_eq!(prepared.unresolved_abi_conditions(), conditions);
    let (handoff, descriptor) = prepared
        .into_inert_for_extraction()
        .map_err(|error| error.to_string())?;
    assert_eq!(handoff.target().to_string(), "gfx942:xnack-");
    assert!(!handoff.authenticates_compiler_origin() && !handoff.grants_compiler_authority());
    assert!(!descriptor.authenticates_compiler_origin() && !descriptor.grants_launch_authority());
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
    // Uses the production descriptor serializer only to compare an inert
    // relation. It neither constructs source custody nor trusts a suffix name.
    let relation = crate::kernel_ir_codegen::exact_inert_descriptor_extension_v21;
    assert!(relation(&llvm, worker_llvm, &descriptor));
    assert!(!relation(&llvm, &format!("{worker_llvm}\n"), &descriptor));
    assert!(!relation(&format!("{llvm}\n"), worker_llvm, &descriptor));
    assert!(!relation(&llvm, &llvm, &descriptor));
    let decoded =
        fe2o3_compiler_ffi::CompilerModuleHandoffV2::decode(handoff.canonical_bytes()).unwrap();
    assert_eq!(decoded.canonical_bytes(), handoff.canonical_bytes());
    let decoded_descriptor =
        fe2o3_compiler_ffi::CompilerDescriptorSourceV1::decode(descriptor.canonical_bytes())
            .unwrap();
    assert_eq!(decoded_descriptor, descriptor);
    fs::create_dir(output).map_err(|error| error.to_string())?;
    for (name, bytes, cap) in [
        ("canonical-v21.bin", canonical.as_slice(), 1024 * 1024),
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
        "stage":"actual_source_mir38_checked_kir21_cpu_normal_inert_handoff",
        "semantic_sha256":semantic_identity,"canonical_identity":canonical_identity,
        "canonical_bytes_sha256":digest(&canonical),"middle_end_evidence_sha256":evidence_identity,
        "entry_symbol":entry_symbol,"authored_blocks":1,"native_instruction_count":24,
        "same_owner_source_canonical_descriptor_entry":true,
        "logical_slice_arguments":2,"native_abi_slots":4,"kernarg_reads":reads,"kernarg_minimum_bytes":32,"kernarg_alignment":8,
        "full_exec_input_read":input_read,"input_launch_minimum_bytes":512,"output_launch_minimum_bytes":512,
        "runtime_abi_conditions":conditions,"runtime_abi_conditions_discharged":false,
        "actual_owner_abi_negative_controls":abi_controls,"abi_resource_denial_controls":4,
        "cpu_cases":cpu_cases,"cpu_negative_controls":3,"cpu_grids":[64,128],
        "cpu_tail_lengths":[0,1,63,64,65,127,128,129],"cpu_seeds":[0,u32::MAX,0xaaaa_5555_u32,19],
        "canaries_unchanged":true,"llvm_sha256":digest(llvm.as_bytes()),
        "handoff_sha256":digest(handoff.canonical_bytes()),"descriptor_sha256":digest(descriptor.canonical_bytes()),
        "exact_canonical_descriptor_extension":true,"descriptor_extension_negative_controls":3,
        "normal_worker_preparation":true,"native_llvm_executed":false,"hardware_observed":false,
        "protected_finalizer_admitted":false,"host_admitted":false,
        "grants_artifact_or_launch_authority":false,
    }))
}
