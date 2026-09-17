const REALTIME_SOURCE_V32: &str = r#"#![no_std]
use fe2o3_device::{kernel, thread, WriteOnlyDisjointSlice};
use fe2o3_device::diagnostics::realtime64;

#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn realtime_observations(
    mut starts: WriteOnlyDisjointSlice<u64>,
    mut ends: WriteOnlyDisjointSlice<u64>,
) {
    let start = realtime64();
    let _ = starts.write(thread::index_1d(), start);
    let end = realtime64();
    let _ = ends.write(thread::index_1d(), end);
}
"#;

fn run_realtime_source_v32(source: &str, cpu: &str) -> (std::process::Output, Option<String>) {
    let target = ScratchTarget::new();
    let fixture = materialize_source_safety_fixture(&target, source);
    let llvm = target.path().join("realtime.ll");
    let mut command = Command::new(env!("CARGO"));
    command.current_dir(fixture)
        .env("RUSTC_WORKSPACE_WRAPPER", env!("CARGO_BIN_EXE_fe2o3-rustc-extract"))
        .env("FE2O3_EXTRACT_CRATE_V1", "fe2o3_production_source_safety_fixture")
        .env("FE2O3_EXTRACT_AMDGPU_LLVM_PATH_V1", &llvm)
        .env("CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS", format!(
            "-Zalways-encode-mir -Zinline-mir=no -Zmir-enable-passes=-JumpThreading -Copt-level=0 -Ctarget-cpu={cpu} -Ctarget-feature=-wavefrontsize32,+wavefrontsize64,-xnack"
        ));
    for variable in [
        "RUSTFLAGS",
        "CARGO_ENCODED_RUSTFLAGS",
        "RUSTC_WRAPPER",
        "CARGO_BUILD_RUSTC_WRAPPER",
        "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER",
        "FE2O3_CARGO_METADATA_BUILD_OBSERVATION_V2",
        "FE2O3_CRATE_BINDING_ID_V1",
        "FE2O3_EXTRACT_CRATE_BINDING_PATH_V1",
        "FE2O3_EXTRACT_INERT_RUSTC_INVOCATION_V3_HEX",
        "FE2O3_EXTRACT_RANKED_MEMORY_V1",
        "FE2O3_EXTRACT_GFX942_LLVM_PATH_V1",
        "FE2O3_EXTRACT_AMDGPU_COMPILER_HANDOFF_PATH_V1",
        "FE2O3_EXTRACT_GFX942_COMPILER_HANDOFF_PATH_V1",
    ] {
        command.env_remove(variable);
    }
    let output = command
        .args([
            "check",
            "--offline",
            "-Zbuild-std=core",
            "--lib",
            "--target",
            "amdgcn-amd-amdhsa",
            "--target-dir",
        ])
        .arg(target.path().join("cargo"))
        .output()
        .expect("run ordinary checked realtime source");
    (output, std::fs::read_to_string(llvm).ok())
}

#[test]
#[ignore = "requires matched pinned-nightly extractor/backend and rust-src"]
fn realtime_v32_source_retains_two_u64_observations() {
    let (output, llvm) = run_realtime_source_v32(REALTIME_SOURCE_V32, "gfx950");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let llvm = llvm.expect("successful checked LLVM output");
    assert_eq!(
        llvm.matches("call i64 @llvm.amdgcn.s.memrealtime()")
            .count(),
        2,
        "{llvm}"
    );
    assert!(llvm.contains("store i64"));
    assert!(!llvm.contains("trunc i64"));
    println!("{llvm}");
}

#[test]
#[ignore = "requires matched pinned-nightly extractor/backend and rust-src"]
fn realtime_v32_source_rejects_unadmitted_target() {
    let (output, llvm) = run_realtime_source_v32(REALTIME_SOURCE_V32, "gfx942");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "gfx942 must not gain realtime64 admission"
    );
    assert!(llvm.is_none());
    assert!(
        stderr.contains("gfx950") || stderr.contains("realtime64"),
        "{stderr}"
    );
}

#[test]
#[ignore = "requires matched pinned-nightly extractor/backend and rust-src"]
fn realtime_v32_source_spelling_does_not_grant_counter_authority() {
    let source = REALTIME_SOURCE_V32.replace(
        "use fe2o3_device::diagnostics::realtime64;",
        "#[inline(always)] fn realtime64() -> u64 { 42 }",
    );
    let (output, llvm) = run_realtime_source_v32(&source, "gfx950");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let llvm = llvm.expect("ordinary constant source control");
    assert!(!llvm.contains("memrealtime"), "{llvm}");
}

#[test]
#[ignore = "requires matched pinned-nightly extractor/backend and rust-src"]
fn realtime_v32_source_retains_unused_conditional_observations() {
    let source = REALTIME_SOURCE_V32.replace(
        "    let start = realtime64();\n    let _ = starts.write(thread::index_1d(), start);\n    let end = realtime64();\n    let _ = ends.write(thread::index_1d(), end);",
        "    if thread::index_1d().get() == 0 {\n        let _ = realtime64();\n        let _ = realtime64();\n    }\n    let _ = starts.write(thread::index_1d(), 0);\n    let _ = ends.write(thread::index_1d(), 0);",
    );
    assert_ne!(source, REALTIME_SOURCE_V32);
    let (output, llvm) = run_realtime_source_v32(&source, "gfx950");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let llvm = llvm.expect("conditional diagnostic source LLVM");
    let call = "call i64 @llvm.amdgcn.s.memrealtime()";
    assert_eq!(llvm.matches(call).count(), 2, "{llvm}");
    println!("{llvm}");
    assert_conditional_counter_cfg_v32(&llvm);
}

fn assert_conditional_counter_cfg_v32(llvm: &str) {
    use std::collections::{BTreeMap, BTreeSet};
    let (_, kernel) = llvm.split_once("define amdgpu_kernel ").unwrap();
    let (_, body) = kernel.split_once("{\n").unwrap();
    let (body, _) = body.split_once("\n}").unwrap();
    // The fixture emits only labels, scalar operations, br, and ret. Read its
    // actual branch operands; textual order alone is not placement evidence.
    let mut blocks = BTreeMap::<&str, Vec<&str>>::new();
    let mut current = None;
    let mut source_branch = None;
    for line in body.lines().map(str::trim).filter(|line| !line.is_empty()) {
        if let Some(label) = line.strip_suffix(':') {
            assert!(blocks.insert(label, Vec::new()).is_none());
            current = Some(label);
        } else {
            let block = current.expect("every emitted instruction has a block label");
            blocks.get_mut(block).unwrap().push(line);
            if source_branch.is_none() && line.starts_with("br i1 ") {
                source_branch = Some((block, line));
            }
        }
    }
    let (condition_block, branch) = source_branch.expect("source conditional branch");
    let operands = branch
        .strip_prefix("br i1 ")
        .unwrap()
        .split(", ")
        .collect::<Vec<_>>();
    assert_eq!(operands.len(), 3);
    let definition = format!("{} = icmp eq i64 ", operands[0]);
    assert!(
        blocks[condition_block]
            .iter()
            .any(|line| line.starts_with(&definition) && line.ends_with(", 0")),
        "source index==0 branch missing: {llvm}"
    );
    let taken = operands[1].strip_prefix("label %").unwrap();
    let untaken = operands[2].strip_prefix("label %").unwrap();
    let clock_blocks = blocks
        .iter()
        .filter_map(|(label, lines)| {
            lines
                .iter()
                .any(|line| line.contains("call i64 @llvm.amdgcn.s.memrealtime()"))
                .then_some(*label)
        })
        .collect::<BTreeSet<_>>();
    let reachable = |start| {
        let mut pending = vec![start];
        let mut seen = BTreeSet::new();
        while let Some(block) = pending.pop() {
            if !seen.insert(block) {
                continue;
            }
            let terminal = *blocks[block].last().unwrap();
            if let Some(target) = terminal.strip_prefix("br label %") {
                pending.push(target);
            } else if terminal.starts_with("br i1 ") {
                pending.extend(
                    terminal
                        .split(", ")
                        .skip(1)
                        .map(|target| target.strip_prefix("label %").unwrap()),
                );
            } else {
                assert!(
                    terminal == "ret void" || terminal == "unreachable",
                    "unsupported fixture terminator: {terminal}"
                );
            }
        }
        seen
    };
    assert!(!clock_blocks.is_empty());
    assert!(
        !clock_blocks.contains(condition_block),
        "counter precedes source condition"
    );
    assert!(
        clock_blocks.is_subset(&reachable(taken)),
        "counter not on taken path"
    );
    assert!(
        clock_blocks.is_disjoint(&reachable(untaken)),
        "counter reachable on untaken path"
    );
}
