use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

struct Scratch(PathBuf);

impl Scratch {
    fn new(case: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after Unix epoch")
            .as_nanos();
        let path = PathBuf::from("/dev/shm").join(format!(
            "fe2o3-gfx950-systems-ui-{case}-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir_all(path.join("src")).expect("create UI fixture directory");
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn compile(case: &str, source: &str) -> Output {
    let scratch = Scratch::new(case);
    let device = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/fe2o3-device")
        .canonicalize()
        .expect("canonical fe2o3-device path");
    std::fs::write(
        scratch.0.join("Cargo.toml"),
        format!(
            "[package]\nname = \"fe2o3-gfx950-systems-ui-{case}\"\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n\n[workspace]\n\n[dependencies]\nfe2o3-device = {{ path = {device:?} }}\n"
        ),
    )
    .expect("write UI fixture manifest");
    std::fs::write(scratch.0.join("src/lib.rs"), source).expect("write UI fixture source");
    let target = std::env::var_os("FE2O3_GFX950_SYSTEMS_UI_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| scratch.0.join("target"));
    Command::new(env!("CARGO"))
        .current_dir(&scratch.0)
        .env("CARGO_TARGET_DIR", target)
        .env("RUSTUP_TOOLCHAIN", "nightly-2026-04-03")
        .env_remove("RUSTC")
        .env_remove("RUSTC_WRAPPER")
        .env_remove("RUSTC_WORKSPACE_WRAPPER")
        .args(["check", "--offline", "--quiet"])
        .output()
        .expect("compile capability UI fixture")
}

const PASS_SURFACE: &str = r#"
use fe2o3_device::{
    Acquire, AtomicReadWrite, CapabilityMemoryElementV1, CurrentTarget, Global,
    GlobalAddressSpace, KernelCapabilityBrand, KernelContext, ReadOnly, RegisteredLaunch,
    Release, SubgroupWidth64, SystemScope,
};

fn checked_load<T: CapabilityMemoryElementV1, Brand>(
    input: &Global<'_, T, ReadOnly, Brand>,
    index: usize,
    fallback: T,
) -> T {
    input.load(index).unwrap_or(fallback)
}

fn hierarchy<'kernel, Kernel>(mut context: KernelContext<'kernel, Kernel>) {
    let mut private = context.private_memory::<f32, 4>();
    assert!(private.store(0, 1.0));
    let value = private.load(0).unwrap_or(0.0);
    context.with_workgroup(|workgroup| {
        let subgroup = workgroup.subgroup::<SubgroupWidth64>();
        let _ = subgroup.reduce_sum(workgroup.epoch(), value);
    });
}

enum AtomicKernel {}

type AtomicBrand<'kernel> =
    KernelCapabilityBrand<'kernel, AtomicKernel, CurrentTarget, RegisteredLaunch>;

fn atomic_surface<'kernel>(
    mut context: KernelContext<'kernel, AtomicKernel>,
    counters: Global<'kernel, u32, AtomicReadWrite<SystemScope>, AtomicBrand<'kernel>>,
) {
    context.with_workgroup(|workgroup| {
        let location = workgroup.global_atomic(&counters, 0).unwrap();
        let _: u32 =
            workgroup.atomic_load::<u32, GlobalAddressSpace, SystemScope, Acquire>(&location);
        workgroup.atomic_store::<u32, GlobalAddressSpace, SystemScope, Release>(&location, 1);
    });
}
"#;

const FORGERY: &str = r#"
use fe2o3_device::KernelContext;

fn forge() {
    let _: KernelContext<'static> = KernelContext::__compiler_issue();
}
"#;

const CROSS_KERNEL: &str = r#"
use fe2o3_device::{DisjointIndex, DisjointWrite, Global, Index1D};

fn substitute<Left, Right>(
    output: &mut Global<'_, u32, DisjointWrite<Index1D>, Left>,
    index: DisjointIndex<Index1D, Right>,
) {
    let _ = output.store(index, 1);
}
"#;

const WRONG_MAPPING: &str = r#"
use fe2o3_device::{Blocked, DisjointIndex, DisjointWrite, Global, Index1D, RowStriped2D};

fn substitute_mapping<Brand>(
    output: &mut Global<'_, u32, DisjointWrite<RowStriped2D<Index1D, 64, 1>>, Brand>,
    index: DisjointIndex<Blocked<Index1D, 64, 1>, Brand>,
) {
    let _ = output.store(index, 1);
}
"#;

const WRONG_ROLE: &str = r#"
use fe2o3_device::{DisjointWrite, Global, Index1D};

fn read_write_only<Brand>(output: &Global<'_, u32, DisjointWrite<Index1D>, Brand>) {
    let _ = output.load(0);
}
"#;

const DISJOINT_CONSTANT_COLLISION: &str = r#"
use fe2o3_device::{DisjointWrite, Global, Index1D};

fn collide<Brand>(output: &mut Global<'_, u32, DisjointWrite<Index1D>, Brand>) {
    let _ = output.store(0, 1);
}
"#;

const WRONG_ADDRESS: &str = r#"
use fe2o3_device::{Global, PrivateMemoryView, ReadOnly};

fn consume_global<Brand>(_: &Global<'_, u32, ReadOnly, Brand>) {}

fn substitute_private<Brand>(input: &PrivateMemoryView<'_, u32, ReadOnly, Brand>) {
    consume_global(input);
}
"#;

const WRONG_ALIAS: &str = r#"
use fe2o3_device::{ExclusiveReadWrite, Global, ReadOnly};

fn consume_shared<Brand>(_: &Global<'_, u32, ReadOnly, Brand>) {}

fn substitute_exclusive<Brand>(input: &Global<'_, u32, ExclusiveReadWrite, Brand>) {
    consume_shared(input);
}
"#;

const UNCHECKED_BOUNDS: &str = r#"
use fe2o3_device::{Global, ReadOnly};

fn unchecked<Brand>(input: &Global<'_, u32, ReadOnly, Brand>) -> u32 {
    input[0]
}
"#;

const INVALID_ATOMIC_ORDER: &str = r#"
use fe2o3_device::{Acquire, AtomicStoreOrdering};

fn require_store_order<Ordering: AtomicStoreOrdering>() {}

fn invalid() {
    require_store_order::<Acquire>();
}
"#;

const INVALID_ATOMIC_SCOPE: &str = r#"
use fe2o3_device::{SubgroupScope, WorkgroupAtomicScope};

fn require_atomic_scope<Scope: WorkgroupAtomicScope>() {}

fn invalid() {
    require_atomic_scope::<SubgroupScope>();
}
"#;

const STALE_EPOCH: &str = r#"
use fe2o3_device::{
    AcquireRelease, InitialEpoch, SubgroupWidth64, WorkgroupCapability, WorkgroupMemory,
    WorkgroupScope,
};

fn stale<'workgroup, Brand>(workgroup: WorkgroupCapability<'workgroup, Brand, InitialEpoch>) {
    let subgroup = workgroup.subgroup::<SubgroupWidth64>();
    let workgroup = workgroup.barrier::<WorkgroupScope, AcquireRelease, WorkgroupMemory>();
    let _ = subgroup.reduce_sum(workgroup.epoch(), 1_u32);
}
"#;

const UNINITIALIZED_LDS: &str = r#"
use fe2o3_device::{InitialEpoch, WorkgroupCapability, WorkgroupLds, WorkgroupLdsUninitialized};

fn read_before_publish<'workgroup, Brand>(
    workgroup: &WorkgroupCapability<'workgroup, Brand, InitialEpoch>,
    lds: &WorkgroupLds<'workgroup, u32, 64, WorkgroupLdsUninitialized, Brand, InitialEpoch>,
) {
    let _ = lds.read(workgroup, 0);
}
"#;

const TARGET_BOUNDARY: &str = r#"
// expected-boundary: FE2O3-CAP-TARGET target legalization must reject gfx950 matrix authority on gfx942
use fe2o3_device::{KernelContext, StrictIeee, SubgroupWidth64};

fn exact_gfx950<'kernel, Brand>(mut context: KernelContext<'kernel, Brand>) {
    let policy = context.numerical_policy::<StrictIeee>();
    context.with_workgroup(|workgroup| {
        let subgroup = workgroup.subgroup::<SubgroupWidth64>();
        subgroup.with_matrix(workgroup.epoch(), |matrix, _lane| {
            let policy_matrix = matrix.with_numerical_policy(&policy);
            let _ = policy_matrix.gfx950();
        });
    });
}
"#;

const TAIL_ANALYSIS_BOUNDARY: &str = r#"
// expected-boundary: FE2O3-CAP-BOUNDS final-graph analysis must prove dynamic tail completeness
use fe2o3_device::{Global, ReadOnly};

fn source_typecheck_cannot_prove_tail<Brand>(input: &Global<'_, u32, ReadOnly, Brand>) -> u32 {
    input.load(0).unwrap_or(0)
}
"#;

#[test]
fn generic_global_private_and_workgroup_surface_typechecks() {
    let output = compile("pass-surface", PASS_SURFACE);
    assert!(
        output.status.success(),
        "capability surface did not typecheck:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn hostile_capability_programs_fail_during_rust_type_checking() {
    const CASES: [(&str, &str, &[&str]); 12] = [
        ("forgery", FORGERY, &["call to unsafe function"]),
        ("cross-kernel", CROSS_KERNEL, &["mismatched types", "Right"]),
        (
            "wrong-mapping",
            WRONG_MAPPING,
            &["mismatched types", "RowStriped2D", "Blocked"],
        ),
        ("wrong-role", WRONG_ROLE, &["no method named `load`"]),
        (
            "disjoint-constant-collision",
            DISJOINT_CONSTANT_COLLISION,
            &["mismatched types", "DisjointIndex"],
        ),
        ("wrong-address", WRONG_ADDRESS, &["mismatched types"]),
        (
            "wrong-alias",
            WRONG_ALIAS,
            &["mismatched types", "ExclusiveReadWrite"],
        ),
        ("unchecked-bounds", UNCHECKED_BOUNDS, &["cannot index"]),
        (
            "invalid-atomic-order",
            INVALID_ATOMIC_ORDER,
            &["AtomicStoreOrdering", "Acquire"],
        ),
        (
            "invalid-atomic-scope",
            INVALID_ATOMIC_SCOPE,
            &["WorkgroupAtomicScope", "SubgroupScope"],
        ),
        (
            "stale-epoch",
            STALE_EPOCH,
            &["mismatched types", "NextEpoch"],
        ),
        (
            "uninitialized-lds",
            UNINITIALIZED_LDS,
            &["no method named `read`", "WorkgroupLdsUninitialized"],
        ),
    ];

    for (case, source, diagnostics) in CASES {
        let output = compile(case, source);
        let stderr = String::from_utf8(output.stderr).expect("rustc diagnostics are UTF-8");
        assert!(!output.status.success(), "hostile fixture {case} compiled");
        for diagnostic in diagnostics {
            assert!(
                stderr.contains(diagnostic),
                "{case} omitted {diagnostic:?}:\n{stderr}"
            );
        }
    }
}

#[test]
fn target_mismatch_is_reserved_for_production_target_legalization() {
    assert!(TARGET_BOUNDARY.contains("expected-boundary: FE2O3-CAP-TARGET"));
    let output = compile("target-boundary", TARGET_BOUNDARY);
    assert!(
        output.status.success(),
        "gfx950 requirement failed before target legalization:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn dynamic_tail_completeness_is_reserved_for_final_graph_analysis() {
    assert!(TAIL_ANALYSIS_BOUNDARY.contains("expected-boundary: FE2O3-CAP-BOUNDS"));
    let output = compile("tail-analysis-boundary", TAIL_ANALYSIS_BOUNDARY);
    assert!(
        output.status.success(),
        "tail boundary failed before final-graph analysis:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
