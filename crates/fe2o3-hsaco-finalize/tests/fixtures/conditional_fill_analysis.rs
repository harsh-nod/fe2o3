use super::*;
use fe2o3_kernel_analysis::{
    AuthenticatedPhysicalMachineEffectLimitsV1, AuthenticatedPhysicalMachineEffectWorkerV1,
    PhysicalMachineEffectBudgetV1, PhysicalMachineEffectEntryRequestV1,
    check_gfx942_fill_analysis_v1, inspect_physical_machine_effect_worker_candidate_v1,
};
use fe2o3_verifier::{
    check_conditional_fill_program_v1, validate_conditional_compiler_proof_inputs_v1,
    validate_conditional_compiler_target_lineage_v1,
};
use std::{fs::OpenOptions, io::Write, os::unix::fs::OpenOptionsExt};

#[test]
#[ignore = "requires current native Worker V3 executable and exact build identities, no GPU"]
fn genuine_conditional_handoff_reaches_finalizer_and_authenticated_fill_model() {
    let worker_path = PathBuf::from(std::env::var_os("FE2O3_TEST_REAL_WORKER").unwrap());
    let worker_build_identity = std::env::var("FE2O3_TEST_REAL_WORKER_BUILD_ID").unwrap();
    let llvm_build_identity: &'static str = Box::leak(
        std::env::var("FE2O3_TEST_REAL_LLVM_BUILD_ID")
            .unwrap()
            .into_boxed_str(),
    );
    let outer = InertSemanticCompilerModuleHandoffV3::decode(include_bytes!(
        "../../../fe2o3-verifier/src/conditional_fill_program_v1/fill.handoff"
    ))
    .unwrap();
    let receipts = outer.capsule().receipts();
    let inputs = validate_conditional_compiler_proof_inputs_v1(
        receipts.proof_binding(),
        receipts.semantic_mir(),
        receipts.middle_end(),
        receipts.kernel_ir(),
        receipts.mir_to_kir_correspondence(),
        receipts.formal_memory(),
    )
    .unwrap();
    let lineage =
        validate_conditional_compiler_target_lineage_v1(outer.capsule(), &inputs).unwrap();
    let program = check_conditional_fill_program_v1(&inputs, &lineage).unwrap();
    let directory = TestDirectory::new();
    let worker = pinned_external(
        &directory,
        &worker_path,
        &worker_build_identity,
        llvm_build_identity,
    );
    let config = EvidenceConfig::BASE;
    let attempt = begin_build_attempt(
        &directory.0,
        &producer(),
        BuildInvocation::from_bytes([config.attempt_seed; 32]),
        BuildSession::from_bytes([config.attempt_seed.wrapping_add(1); 16]),
    )
    .unwrap();
    let receipt = publish_compiler_module_handoff_in_slot_v3(
        &directory.0,
        &producer(),
        attempt,
        config.slot,
        &outer,
    )
    .unwrap();
    let consumed = consume_compiler_module_handoff_in_slot_v3(
        &directory.0,
        &producer(),
        attempt,
        config.slot,
        outer.identity(),
    )
    .unwrap();
    let evidence = execute_protected_reproducible_first_build_worker_v3(
        consumed,
        receipt,
        *outer.capsule().compiler_closure(),
        &worker,
        Vec::new(),
        options("3"),
        WorkerOutputConstraintsV1::new(1024 * 1024).unwrap(),
        WorkerExecutionLimitsV1::new(Duration::from_secs(60), 2 * 1024 * 1024, 64 * 1024).unwrap(),
    )
    .unwrap();
    assert!(evidence.replays_exact_llvm_object_lld_derivation());
    assert_eq!(
        evidence.handoff().canonical_bytes(),
        outer.canonical_bytes()
    );
    let bootstrap_request = evidence.bootstrap_request_bytes().to_vec();
    let replay_request = evidence.exact_replay_request_bytes().to_vec();
    let raw = evidence.output_bytes().to_vec();
    let finalized = finalize_protected_worker_v3_hsaco_v1(
        inspect_protected_worker_v3_hsaco_v1(evidence).unwrap(),
    )
    .unwrap();
    assert_eq!(finalized.outer_handoff_identity(), outer.identity());
    assert_eq!(
        finalized.outer_handoff().canonical_bytes(),
        outer.canonical_bytes()
    );
    assert_eq!(
        finalized.outer_handoff().module_handoff().module_bytes(),
        outer.module_handoff().module_bytes()
    );
    assert_eq!(
        lineage.final_llvm_identity().sha256(),
        *outer.module_handoff().module_identity().sha256()
    );
    assert_eq!(
        lineage.final_llvm_identity().byte_len(),
        outer.module_handoff().module_identity().byte_len()
    );
    assert!(finalized.canonical_descriptor_finalization_ran());
    assert!(!finalized.authenticates_compiler_origin());
    assert!(!finalized.grants_launch_authority());
    let limits = AuthenticatedPhysicalMachineEffectLimitsV1::new(
        Duration::from_secs(60),
        1024 * 1024,
        16384,
    )
    .unwrap();
    let analyzer_path = directory.0.join("real-worker");
    let worker_bytes = fs::read(&analyzer_path).unwrap();
    assert_eq!(
        ContentIdentityV1::calculate(&worker_bytes),
        worker.measurement().executable()
    );
    let analyzer_executable =
        fe2o3_kernel_analysis::PhysicalMachineWorkerExecutableIdentityV1::calculate(&worker_bytes);
    drop(worker_bytes);
    let candidate =
        inspect_physical_machine_effect_worker_candidate_v1(&analyzer_path, limits).unwrap();
    assert_eq!(candidate.policy().executable(), analyzer_executable);
    let analyzer = AuthenticatedPhysicalMachineEffectWorkerV1::open(
        &analyzer_path,
        candidate.policy(),
        limits,
    )
    .unwrap();
    let execution = analyzer
        .analyze(
            finalized.exact_finalized_bytes().to_vec(),
            vec![
                PhysicalMachineEffectEntryRequestV1::new(
                    program.function_symbol(),
                    PhysicalMachineEffectBudgetV1::new(2, 1, 1, 1, 0),
                )
                .unwrap(),
            ],
            limits,
        )
        .unwrap();
    if let Some(path) = std::env::var_os("FE2O3_FILL_FINALIZER_CAPTURE") {
        let path = Path::new(&path);
        for (name, bytes) in [
            ("bootstrap.request", bootstrap_request.as_slice()),
            ("replay.request", replay_request.as_slice()),
            ("raw.hsaco", raw.as_slice()),
            ("finalized.hsaco", finalized.exact_finalized_bytes()),
            ("fill.request", execution.request().canonical_bytes()),
            ("fill.bundle", execution.analysis().canonical_bytes()),
            ("fill.receipt", execution.canonical_receipt_bytes()),
        ] {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(path.join(name))
                .unwrap();
            file.write_all(bytes).unwrap();
            file.sync_all().unwrap();
        }
    }
    let inspection =
        fe2o3_hsaco::inspect_and_bind_kernel_descriptors(finalized.exact_finalized_bytes())
            .unwrap();
    eprintln!(
        "finalized bindings={:?} kernels={:?} entry={:?}",
        inspection.bindings(),
        inspection.inspection().kernels(),
        inspection.gfx942_initial_register_layout_v1(0)
    );
    let machine = check_gfx942_fill_analysis_v1(&execution, program.function_symbol()).unwrap();
    assert!(std::ptr::eq(machine.execution(), &execution));
    assert_eq!(
        machine.kernel().code_object(),
        finalized.exact_finalized_bytes()
    );
    assert!(!machine.establishes_compiler_refinement());
    assert!(!machine.grants_launch_authority());
    eprintln!(
        "genuine fill handoff={:?} finalized={:?} analyzer_receipt={:?} binding={:?}",
        outer.identity(),
        finalized.finalized_output_identity(),
        execution.identity(),
        machine.kernel().binding()
    );
}
