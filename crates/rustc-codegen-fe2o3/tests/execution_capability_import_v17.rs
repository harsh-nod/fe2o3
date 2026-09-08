use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

const TERMINALS: [(&str, &str, &str); 38] = [
    (
        "BindAtomicView",
        "CapabilityGlobalBindAtomic",
        "__compiler_bind_atomic",
    ),
    (
        "WorkgroupDerive",
        "ExecutionWorkgroupCurrent",
        "__compiler_workgroup_capability_current",
    ),
    ("SubgroupDerive", "ExecutionSubgroupCurrent", "subgroup"),
    ("LdsAllocate", "ExecutionLdsAllocate", "allocate_lds"),
    ("WorkgroupBarrier", "ExecutionWorkgroupBarrier", "barrier"),
    (
        "SubgroupBarrier",
        "ExecutionSubgroupBarrier",
        "subgroup_barrier",
    ),
    ("WorkgroupFence", "ExecutionWorkgroupFence", "fence"),
    ("LdsPublish", "ExecutionLdsPublish", "publish_lds"),
    ("AsyncCopy", "ExecutionAsyncCopy", "async_copy_from_global"),
    ("AsyncWait", "ExecutionAsyncWait", "wait_for"),
    (
        "WorkgroupReduceSum",
        "ExecutionWorkgroupReduceSum",
        "reduce_sum",
    ),
    (
        "WorkgroupInclusiveScanSum",
        "ExecutionWorkgroupInclusiveScanSum",
        "inclusive_scan_sum",
    ),
    (
        "WorkgroupExclusiveScanSum",
        "ExecutionWorkgroupExclusiveScanSum",
        "exclusive_scan_sum",
    ),
    (
        "BindGlobalAtomicLocation",
        "ExecutionGlobalAtomic",
        "global_atomic",
    ),
    ("AtomicLoad", "ExecutionAtomicLoad", "atomic_load"),
    ("AtomicStore", "ExecutionAtomicStore", "atomic_store"),
    (
        "AtomicFetchAdd",
        "ExecutionAtomicFetchAdd",
        "atomic_fetch_add",
    ),
    (
        "AtomicCompareExchange",
        "ExecutionAtomicCompareExchange",
        "atomic_compare_exchange",
    ),
    ("SubgroupFence", "ExecutionSubgroupFence", "fence"),
    (
        "SubgroupReduceSum",
        "ExecutionSubgroupReduceSum",
        "reduce_sum",
    ),
    (
        "SubgroupInclusiveScanSum",
        "ExecutionSubgroupInclusiveScanSum",
        "inclusive_scan_sum",
    ),
    (
        "MatrixAccess",
        "ExecutionMatrixAccess",
        "__compiler_matrix_access",
    ),
    (
        "LdsInitializeByInvocation",
        "ExecutionLdsInitializeByInvocation",
        "initialize_by_invocation",
    ),
    ("LdsReadPublished", "ExecutionLdsReadPublished", "read"),
    (
        "PrivateMemoryFromRawParts",
        "PrivateMemoryFromRawParts",
        "from_raw_parts",
    ),
    (
        "WorkgroupMemoryFromRawParts",
        "WorkgroupMemoryFromRawParts",
        "from_raw_parts",
    ),
    (
        "PrivateMemoryAllocate",
        "PrivateMemoryAllocate",
        "private_memory",
    ),
    (
        "WorkgroupMemoryIndex1D",
        "WorkgroupMemoryIndex1D",
        "memory_index_1d",
    ),
    (
        "WorkgroupMemoryAllocate",
        "WorkgroupMemoryAllocate",
        "allocate_memory",
    ),
    (
        "WorkgroupMemoryPublish",
        "WorkgroupMemoryPublish",
        "publish_memory",
    ),
    ("PrivateMemoryLoad", "PrivateMemoryLoad", "load"),
    (
        "PrivateMemoryExclusiveLoad",
        "PrivateMemoryExclusiveLoad",
        "load",
    ),
    (
        "PrivateMemoryExclusiveStore",
        "PrivateMemoryExclusiveStore",
        "store",
    ),
    (
        "PrivateMemoryDisjointStore",
        "PrivateMemoryDisjointStore",
        "store",
    ),
    ("WorkgroupMemoryLoad", "WorkgroupMemoryLoad", "load"),
    (
        "WorkgroupMemoryExclusiveLoad",
        "WorkgroupMemoryExclusiveLoad",
        "load",
    ),
    (
        "WorkgroupMemoryExclusiveStore",
        "WorkgroupMemoryExclusiveStore",
        "store",
    ),
    (
        "WorkgroupMemoryDisjointStore",
        "WorkgroupMemoryDisjointStore",
        "store",
    ),
];

struct Scratch {
    path: PathBuf,
}

impl Scratch {
    fn new(label: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after Unix epoch")
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("fe2o3-w2-{label}-{}-{nonce}", std::process::id()));
        std::fs::create_dir(&path).expect("create W2 scratch directory");
        Self { path }
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("canonical workspace")
}

fn pinned_rustc() -> PathBuf {
    let rustc = Path::new(env!("CARGO")).with_file_name("rustc");
    assert!(
        rustc.is_file(),
        "pinned rustc is missing: {}",
        rustc.display()
    );
    rustc
}

fn cargo_target(target: &Scratch) -> PathBuf {
    std::env::var_os("FE2O3_V17_CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| target.path.join("cargo"))
}

fn run_import(target: &Scratch, feature: &str) -> Output {
    let mut command = Command::new(env!("CARGO"));
    command
        .current_dir(workspace())
        .env("RUSTC", pinned_rustc())
        .env(
            "RUSTC_WORKSPACE_WRAPPER",
            env!("CARGO_BIN_EXE_fe2o3-rustc-extract"),
        )
        .env(
            "FE2O3_EXTRACT_CRATE_V1",
            "fe2o3_production_extraction_fixture",
        )
        .env("FE2O3_CARGO_METADATA_BUILD_OBSERVATION_V2", "55".repeat(32))
        .env("FE2O3_CRATE_BINDING_ID_V1", "77".repeat(32))
        .env("FE2O3_VERBOSE", "1")
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env(
            "CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS",
            "-Zalways-encode-mir -Zinline-mir=no -Copt-level=0 -Ctarget-cpu=gfx942 -Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32",
        )
        .args([
            "check",
            "--offline",
            "--locked",
            "-Zbuild-std=core",
            "-p",
            "fe2o3-production-extraction-fixture",
            "--features",
            feature,
            "--target",
            "amdgcn-amd-amdhsa",
            "--target-dir",
        ])
        .arg(cargo_target(target));
    command
        .output()
        .expect("run W2 rustc-backed import fixture")
}

fn materialize_fixture(target: &Scratch, source: &str, binary: bool) -> PathBuf {
    let fixture = target.path.join("fixture");
    std::fs::create_dir_all(fixture.join("src")).expect("create hostile fixture source directory");
    let root = workspace();
    let target_kind = if binary {
        "[[bin]]\nname = \"fe2o3-w2-hostile-fixture\"\npath = \"src/main.rs\""
    } else {
        "[lib]\nname = \"fe2o3_w2_hostile_fixture\"\npath = \"src/lib.rs\""
    };
    let manifest = format!(
        r#"[package]
name = "fe2o3-w2-hostile-fixture"
version = "0.0.0"
edition = "2024"
publish = false

[workspace]

[dependencies]
fe2o3-device = {{ path = "{}" }}

[target.'cfg(not(target_arch = "amdgpu"))'.dependencies]
fe2o3-host = {{ path = "{}" }}

{target_kind}
"#,
        root.join("crates/fe2o3-device").display(),
        root.join("crates/fe2o3-host").display(),
    );
    std::fs::write(fixture.join("Cargo.toml"), manifest).expect("write hostile manifest");
    std::fs::copy(root.join("Cargo.lock"), fixture.join("Cargo.lock"))
        .expect("copy workspace lockfile");
    std::fs::write(
        fixture
            .join("src")
            .join(if binary { "main.rs" } else { "lib.rs" }),
        source,
    )
    .expect("write hostile source");
    let stale_source = fixture
        .join("src")
        .join(if binary { "lib.rs" } else { "main.rs" });
    if stale_source.exists() {
        std::fs::remove_file(stale_source).expect("remove stale hostile source kind");
    }
    fixture
}

fn clear_rustc_overrides(command: &mut Command) {
    for variable in [
        "RUSTC",
        "CARGO_BUILD_RUSTC",
        "RUSTC_WRAPPER",
        "CARGO_BUILD_RUSTC_WRAPPER",
        "RUSTC_WORKSPACE_WRAPPER",
    ] {
        command.env_remove(variable);
    }
    command.env("RUSTC", pinned_rustc());
}

fn clean_hostile_package(target: &Scratch, fixture: &Path) {
    let mut command = Command::new(env!("CARGO"));
    command
        .current_dir(fixture)
        .args([
            "clean",
            "--package",
            "fe2o3-w2-hostile-fixture",
            "--target-dir",
        ])
        .arg(cargo_target(target));
    clear_rustc_overrides(&mut command);
    let output = command.output().expect("clean hostile package artifacts");
    assert!(
        output.status.success(),
        "failed to remove stale hostile package artifacts:\n{}",
        String::from_utf8_lossy(&output.stderr),
    );
}

fn compile_hostile_source(target: &Scratch, source: &str) -> Output {
    let fixture = materialize_fixture(target, source, false);
    clean_hostile_package(target, &fixture);
    let mut command = Command::new(env!("CARGO"));
    command
        .current_dir(fixture)
        .env("CARGO_TARGET_DIR", cargo_target(target))
        .env("FE2O3_CRATE_BINDING_ID_V1", "77".repeat(32))
        .args(["check", "--offline"]);
    clear_rustc_overrides(&mut command);
    command.output().expect("compile hostile source")
}

fn import_hostile_source(target: &Scratch, source: &str) -> Output {
    let fixture = materialize_fixture(target, source, false);
    clean_hostile_package(target, &fixture);
    let mut command = Command::new(env!("CARGO"));
    command
        .current_dir(fixture)
        .env("RUSTC", pinned_rustc())
        .env(
            "RUSTC_WORKSPACE_WRAPPER",
            env!("CARGO_BIN_EXE_fe2o3-rustc-extract"),
        )
        .env("FE2O3_EXTRACT_CRATE_V1", "fe2o3_w2_hostile_fixture")
        .env("FE2O3_CARGO_METADATA_BUILD_OBSERVATION_V2", "55".repeat(32))
        .env("FE2O3_CRATE_BINDING_ID_V1", "77".repeat(32))
        .env("FE2O3_VERBOSE", "1")
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env(
            "CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS",
            "-Zalways-encode-mir -Zinline-mir=no -Copt-level=0 -Ctarget-cpu=gfx942 -Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32",
        )
        .args([
            "check",
            "--offline",
            "-Zbuild-std=core",
            "--target",
            "amdgcn-amd-amdhsa",
            "--target-dir",
        ])
        .arg(cargo_target(target));
    command.output().expect("import hostile source")
}

fn field<'a>(line: &'a str, name: &str) -> &'a str {
    line.split_once(name)
        .unwrap_or_else(|| panic!("missing {name:?} in audit line: {line}"))
        .1
        .split_whitespace()
        .next()
        .expect("audit field has a value")
}

fn assert_sha256(value: &str, field: &str) {
    assert_eq!(value.len(), 64, "{field} is not a SHA-256 identity");
    assert!(
        value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "{field} is not lowercase hexadecimal: {value:?}",
    );
    assert_ne!(value, "0".repeat(64), "{field} is zero");
}

#[test]
#[ignore = "requires the pinned nightly rust-src component and AMD target"]
fn all_38_execution_terminals_import_as_authenticated_v17_records() {
    let target = Scratch::new("execution-capability-v17");
    let output = run_import(&target, "execution-capability-v17");
    let stderr = String::from_utf8(output.stderr).expect("rustc diagnostics are UTF-8");
    assert!(
        !output.status.success(),
        "W2 fixture unexpectedly crossed the pending lowering boundary"
    );
    assert!(
        stderr.contains("FE2O3-CAP Incomplete")
            && stderr.contains("stage=target-neutral-lowering")
            && stderr.contains("then admitted one complete semantic MIR request"),
        "fixture did not reach complete semantic import:\n{stderr}",
    );

    let audit = stderr
        .lines()
        .filter(|line| line.contains("[FE2O3-CAP-AUDIT001]"))
        .collect::<Vec<_>>();
    for (terminal, diagnostic, provider) in TERMINALS {
        let matches = audit
            .iter()
            .copied()
            .filter(|line| line.contains(&format!("terminal={terminal}")))
            .collect::<Vec<_>>();
        assert!(
            !matches.is_empty(),
            "missing authenticated {terminal} record:\n{stderr}"
        );
        assert!(matches.iter().any(|line| {
            line.contains(&format!("diagnostic={diagnostic}"))
                && line.contains("provider_crate=fe2o3_device")
                && line.contains("provider=fe2o3_device::")
                && line.contains(provider)
        }));
        for line in matches {
            assert_sha256(field(line, "root="), "root");
            assert_ne!(field(line, "span="), "unresolved");
            assert!(field(line, "helper_chain=").contains("->"));
            assert_eq!(field(line, "stage="), "semantic-mir-v17");
            for identity in [
                "provider_item_sha256=",
                "monomorphization_sha256=",
                "generic_types_sha256=",
                "const_generics_sha256=",
                "fn_abi_sha256=",
            ] {
                assert_sha256(field(line, identity), identity);
            }
            for retained in [
                "source=SemanticSourceProvenanceV1",
                "signature=SemanticExecutionCapabilitySignatureV1",
                "obligations=0x",
                "operation=",
                "trap=false",
            ] {
                assert!(line.contains(retained), "missing {retained:?}: {line}");
            }
        }
    }

    let summary = stderr
        .lines()
        .find(|line| line.contains("[FE2O3-CAP-AUDIT002]"))
        .unwrap_or_else(|| panic!("missing V17 audit summary:\n{stderr}"));
    assert!(summary.contains("wire=17"));
    assert!(summary.contains("roster_mask=0x3fffffffff"));
    assert!(summary.contains("trap_records=0"));
    let records = field(summary, "execution_records=")
        .parse::<usize>()
        .expect("execution record count");
    assert!(records >= TERMINALS.len());

    for contract in [
        "width: 32",
        "width: 64",
        "elements: 7",
        "elements: 64",
        "scope: System",
        "scope: Workgroup",
        "scope: Subgroup",
        "ordering: Acquire",
        "ordering: Release",
        "ordering: AcquireRelease",
        "ordering: SequentiallyConsistent",
        "space: Private",
        "space: Workgroup",
        "access: ReadOnly",
        "access: ExclusiveReadWrite",
        "access: DisjointWrite",
        "epoch_after=Some",
    ] {
        assert!(
            stderr.contains(contract),
            "missing contract axis {contract:?}"
        );
    }
    assert!(
        audit
            .iter()
            .filter(|line| line.contains("terminal=PrivateMemoryFromRawParts"))
            .all(|line| !line.contains("obligations=0x0000")),
    );
    assert!(!stderr.contains("operation=Trap"));
    assert!(!stderr.contains("trap=true"));
}

#[test]
fn roster_expectations_are_closed_and_unique() {
    let mut terminals = TERMINALS.map(|entry| entry.0);
    terminals.sort_unstable();
    assert!(terminals.windows(2).all(|pair| pair[0] != pair[1]));
    assert_eq!(terminals.len(), 38);
}

#[test]
#[ignore = "requires the pinned nightly compiler and workspace dependencies"]
fn rustc_rejects_fabrication_escape_substitution_and_invalid_contracts() {
    const CASES: [(&str, &str, &[&str]); 10] = [
        (
            "fabrication",
            include_str!("fixtures/execution-capability-v17-hostile/fabrication.rs"),
            &["field", "private"],
        ),
        (
            "copy-escape",
            include_str!("fixtures/execution-capability-v17-hostile/copy_escape.rs"),
            &["moved value", "private"],
        ),
        (
            "pointer-escape",
            include_str!("fixtures/execution-capability-v17-hostile/pointer_escape.rs"),
            &["lifetime may not live long enough", "'static"],
        ),
        (
            "cross-kernel",
            include_str!("fixtures/execution-capability-v17-hostile/cross_kernel.rs"),
            &["mismatched types", "Left", "Right"],
        ),
        (
            "cross-workgroup",
            include_str!("fixtures/execution-capability-v17-hostile/cross_workgroup.rs"),
            &["lifetime", "invariant"],
        ),
        (
            "cross-epoch",
            include_str!("fixtures/execution-capability-v17-hostile/cross_epoch.rs"),
            &["mismatched types", "NextEpoch"],
        ),
        (
            "wrong-role",
            include_str!("fixtures/execution-capability-v17-hostile/wrong_role.rs"),
            &["no method named `store`", "CapabilityMemoryView"],
        ),
        (
            "wrong-barrier",
            include_str!("fixtures/execution-capability-v17-hostile/wrong_barrier.rs"),
            &["WorkgroupBarrierSemantics", "Relaxed"],
        ),
        (
            "wrong-atomic",
            include_str!("fixtures/execution-capability-v17-hostile/wrong_atomic.rs"),
            &["AtomicLoadOrdering", "Release"],
        ),
        (
            "wrong-logical-context-abi",
            include_str!("fixtures/execution-capability-v17-hostile/wrong_logical_context_abi.rs"),
            &["KernelContext must be the first kernel parameter"],
        ),
    ];

    let target = Scratch::new("hostile-contracts");
    for (case, source, expected) in CASES {
        let output = compile_hostile_source(&target, source);
        let stderr = String::from_utf8(output.stderr).expect("rustc diagnostics are UTF-8");
        assert!(!output.status.success(), "hostile case {case} compiled");
        for expected in expected {
            assert!(
                stderr.contains(expected),
                "hostile case {case} omitted {expected:?}:\n{stderr}",
            );
        }
    }
}

#[test]
#[ignore = "requires the pinned nightly compiler and workspace dependencies"]
fn collector_rejects_a_lookalike_raw_memory_provider_item() {
    let target = Scratch::new("lookalike-raw-provider");
    let output = import_hostile_source(
        &target,
        include_str!("fixtures/execution-capability-v17-hostile/lookalike_diagnostic.rs"),
    );
    let stderr = String::from_utf8(output.stderr).expect("rustc diagnostics are UTF-8");
    assert!(
        !output.status.success(),
        "lookalike raw-memory provider crossed authenticated import"
    );
    assert!(
        stderr.contains("[FE2O3-CAP-SOURCE002]")
            && stderr.contains("root=lookalike_raw_memory_provider")
            && stderr.contains("stage=source-safety")
            && stderr.contains("from_raw_parts")
            && stderr.contains("reaches unsafe function instance"),
        "lookalike provider identity rejection changed:\n{stderr}",
    );
    assert!(!stderr.contains("[FE2O3-CAP-AUDIT001]"));
    assert!(!stderr.contains("FE2O3-CAP Incomplete"));
}

#[test]
#[ignore = "requires the pinned nightly rust-src component and AMD target"]
fn collector_admits_only_authenticated_higher_order_capability_terminals() {
    let target = Scratch::new("higher-order-capabilities");
    let output = import_hostile_source(
        &target,
        include_str!("fixtures/execution-capability-v17-hostile/higher_order_positive.rs"),
    );
    let stderr = String::from_utf8(output.stderr).expect("rustc diagnostics are UTF-8");
    assert!(
        !output.status.success(),
        "focused fixture unexpectedly crossed the pending lowering boundary"
    );
    assert!(
        stderr.contains("FE2O3-CAP Incomplete")
            && stderr.contains("stage=target-neutral-lowering")
            && stderr.contains("terminal=WorkgroupDerive")
            && stderr.contains("terminal=MatrixAccess"),
        "authenticated with_workgroup/with_matrix graph was not preserved through import:\n{stderr}",
    );
    assert!(!stderr.contains("closure value escapes"), "{stderr}");
}

#[test]
#[ignore = "requires the pinned nightly rust-src component and AMD target"]
fn collector_rejects_non_terminal_closure_escape_and_named_lookalike() {
    const CASES: [(&str, &str); 2] = [
        (
            "non-terminal-escape",
            include_str!(
                "fixtures/execution-capability-v17-hostile/closure_non_terminal_escape.rs"
            ),
        ),
        (
            "higher-order-lookalike",
            include_str!("fixtures/execution-capability-v17-hostile/higher_order_lookalike.rs"),
        ),
    ];
    let target = Scratch::new("higher-order-hostile");
    for (case, source) in CASES {
        let output = import_hostile_source(&target, source);
        let stderr = String::from_utf8(output.stderr).expect("rustc diagnostics are UTF-8");
        assert!(!output.status.success(), "hostile case {case} imported");
        assert!(
            stderr.contains("closure value escapes to a non-closure call"),
            "hostile case {case} changed rejection:\n{stderr}",
        );
        assert!(!stderr.contains("[FE2O3-CAP-AUDIT001]"), "{stderr}");
        assert!(!stderr.contains("FE2O3-CAP Incomplete"), "{stderr}");
    }
}

#[test]
#[ignore = "requires the pinned nightly rust-src component and AMD target"]
fn collector_rejects_unauthenticated_unsafe_and_reachable_traps() {
    const CASES: [(&str, &str, &[&str]); 2] = [
        (
            "unauthenticated-unsafe",
            include_str!("fixtures/execution-capability-v17-hostile/unauthenticated_unsafe.rs"),
            &[
                "[FE2O3-CAP-SOURCE002]",
                "FE2O3-CAP Rejected root=unauthenticated_unsafe",
                "helper_chain=",
                "stage=source-safety",
                "lookalike_private_memory_from_raw_parts",
            ],
        ),
        (
            "reachable-trap",
            include_str!("fixtures/execution-capability-v17-hostile/reachable_trap.rs"),
            &[
                "device code reaches a panic path",
                "unsupported_reachable_call",
            ],
        ),
    ];
    let target = Scratch::new("hostile-imports");
    for (case, source, expected) in CASES {
        let output = import_hostile_source(&target, source);
        let stderr = String::from_utf8(output.stderr).expect("rustc diagnostics are UTF-8");
        assert!(!output.status.success(), "hostile import {case} compiled");
        for expected in expected {
            assert!(
                stderr.contains(expected),
                "hostile import {case} omitted {expected:?}:\n{stderr}",
            );
        }
        assert!(!stderr.contains("[FE2O3-CAP-AUDIT001]"));
        assert!(!stderr.contains("FE2O3-CAP Incomplete"));
    }
}

#[test]
#[ignore = "requires the pinned nightly compiler and workspace dependencies"]
fn compiler_only_context_issuance_cannot_execute_on_the_host() {
    let target = Scratch::new("host-invocation");
    let fixture = materialize_fixture(
        &target,
        include_str!("fixtures/execution-capability-v17-hostile/host_invocation.rs"),
        true,
    );
    let mut command = Command::new(env!("CARGO"));
    command
        .current_dir(fixture)
        .env("CARGO_TARGET_DIR", cargo_target(&target))
        .args(["run", "--offline", "--quiet"]);
    clear_rustc_overrides(&mut command);
    let output = command.output().expect("run host invocation fixture");
    let stderr = String::from_utf8(output.stderr).expect("host diagnostics are UTF-8");
    assert!(
        !output.status.success(),
        "compiler-only context ran on host"
    );
    assert!(
        stderr.contains("KernelContext must be issued by the authenticated fe2o3 compiler"),
        "host fail-closed terminal changed:\n{stderr}",
    );
    assert!(!stderr.contains("[FE2O3-CAP-AUDIT001]"));
}
