#![cfg(all(target_os = "linux", feature = "authenticated-machine-effect"))]

use fe2o3_kernel_analysis::{
    AuthenticatedPhysicalMachineEffectLimitsV1, AuthenticatedPhysicalMachineEffectWorkerV1,
    Gfx942FillAnalysisErrorV1, PhysicalMachineEffectBudgetV1, PhysicalMachineEffectEntryRequestV1,
    check_gfx942_fill_analysis_v1, inspect_physical_machine_effect_worker_candidate_v1,
};
use std::{fs::OpenOptions, io::Write, os::unix::fs::OpenOptionsExt, time::Duration};

#[test]
#[ignore = "requires a native LLVM machine analyzer, no GPU required"]
fn native_authenticated_fill_analysis_retains_exact_model_and_dispatch() {
    let path = std::env::var_os("FE2O3_MACHINE_EFFECT_NATIVE_WORKER")
        .expect("FE2O3_MACHINE_EFFECT_NATIVE_WORKER is required");
    let payload = std::fs::read(
        std::env::var_os("FE2O3_FILL_ANALYSIS_HSACO")
            .expect("FE2O3_FILL_ANALYSIS_HSACO is required"),
    )
    .unwrap();
    let limits = AuthenticatedPhysicalMachineEffectLimitsV1::new(
        Duration::from_secs(60),
        1024 * 1024,
        16384,
    )
    .unwrap();
    let candidate = inspect_physical_machine_effect_worker_candidate_v1(&path, limits).unwrap();
    let worker =
        AuthenticatedPhysicalMachineEffectWorkerV1::open(&path, candidate.policy(), limits)
            .unwrap();
    worker
        .verify_deployed_no_fork_profile_for_test(limits)
        .unwrap();
    let inspected = fe2o3_hsaco::inspect_and_bind_kernel_descriptors(&payload).unwrap();
    let symbol = inspected.inspection().kernels()[0].name();
    let execution = worker
        .analyze(
            payload.clone(),
            vec![
                PhysicalMachineEffectEntryRequestV1::new(
                    symbol,
                    PhysicalMachineEffectBudgetV1::new(2, 1, 1, 1, 0),
                )
                .unwrap(),
            ],
            limits,
        )
        .unwrap();
    capture(&execution);
    for instruction in execution.analysis().trace().instructions() {
        eprintln!("fill instruction={instruction:?}");
    }
    let checked = check_gfx942_fill_analysis_v1(&execution, symbol).unwrap();
    assert!(std::ptr::eq(checked.execution(), &execution));
    assert_eq!(
        checked.kernel().code_object().as_ptr(),
        execution.request().exact_payload_bytes().as_ptr()
    );
    assert_eq!(checked.entry_symbol(), symbol);
    assert_eq!(checked.kernel().binding(), inspected.bindings()[0]);
    assert!(!checked.establishes_compiler_refinement());
    assert!(!checked.grants_launch_authority());
    assert_eq!(
        check_gfx942_fill_analysis_v1(&execution, "wrong_entry").unwrap_err(),
        Gfx942FillAnalysisErrorV1::Entry
    );
    for count in [0u64, 64, 65, 4097] {
        let mut kernarg = [0u8; 16];
        kernarg[..8].copy_from_slice(&0x8000u64.to_le_bytes());
        kernarg[8..].copy_from_slice(&count.to_le_bytes());
        let grid = (count as u32).max(1).div_ceil(64) * 64;
        let dispatch = checked
            .kernel()
            .check_dispatch(kernarg, 0x1000, 0x8000, count * 4, [grid, 1, 1], [64, 1, 1])
            .unwrap();
        for index in 0..count {
            for byte in 0..4 {
                assert_eq!(
                    dispatch.byte_after(0x8000 + index * 4 + byte, 0xa5),
                    (index as u32).to_le_bytes()[byte as usize]
                );
            }
        }
        assert_eq!(dispatch.byte_after(0x7fff, 0xa5), 0xa5);
        assert_eq!(dispatch.byte_after(0x8000 + count * 4, 0xa5), 0xa5);
    }
    eprintln!(
        "fill analyzer={:?} toolchain={:?} execution={:?} bundle={:?}",
        candidate.analyzer_identity(),
        candidate.toolchain_identity(),
        execution.identity(),
        execution.analysis().identity()
    );
}

fn capture(execution: &fe2o3_kernel_analysis::AuthenticatedPhysicalMachineAnalysisExecutionV1) {
    if let Some(directory) = std::env::var_os("FE2O3_FILL_ANALYSIS_CAPTURE") {
        let directory = std::path::Path::new(&directory);
        for (name, bytes) in [
            ("fill.request", execution.request().canonical_bytes()),
            ("fill.bundle", execution.analysis().canonical_bytes()),
            ("fill.receipt", execution.canonical_receipt_bytes()),
        ] {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(directory.join(name))
                .unwrap();
            file.write_all(bytes).unwrap();
            file.sync_all().unwrap();
        }
    }
}
