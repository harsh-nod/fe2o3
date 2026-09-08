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
            "fe2o3-gpt-oss-ui-{case}-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir_all(path.join("src")).expect("create UI fixture");
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
        .expect("canonical device path");
    std::fs::write(
        scratch.0.join("Cargo.toml"),
        format!(
            "[package]\nname='gpt-oss-ui-{case}'\nversion='0.0.0'\nedition='2024'\npublish=false\n\n[workspace]\n\n[dependencies]\nfe2o3-device={{path={device:?}}}\n"
        ),
    )
    .expect("write UI manifest");
    std::fs::write(scratch.0.join("src/lib.rs"), source).expect("write UI source");
    Command::new(env!("CARGO"))
        .current_dir(&scratch.0)
        .env(
            "CARGO_TARGET_DIR",
            std::env::var_os("FE2O3_GPT_OSS_UI_TARGET_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| scratch.0.join("target")),
        )
        .env("RUSTUP_TOOLCHAIN", "nightly-2026-04-03")
        .env_remove("RUSTC")
        .env_remove("RUSTC_WRAPPER")
        .args(["check", "--offline", "--quiet"])
        .output()
        .expect("compile UI fixture")
}

#[test]
fn genuine_capability_composition_typechecks() {
    let output = compile(
        "pass",
        r#"
use fe2o3_device::{
    Blocked, CurrentTarget, DisjointWrite, Global, Index1D, KernelCapabilityBrand, KernelContext,
    ReadOnly, RegisteredLaunch, StrictIeee, SubgroupWidth64,
};

type Brand<'kernel, Kernel> =
    KernelCapabilityBrand<'kernel, Kernel, CurrentTarget, RegisteredLaunch>;

fn body<'kernel, Kernel>(
    mut context: KernelContext<'kernel, Kernel>,
    input: Global<'kernel, f32, ReadOnly, Brand<'kernel, Kernel>>,
    mut output: Global<'kernel, f32, DisjointWrite<Blocked<Index1D, 16, 4>>, Brand<'kernel, Kernel>>,
    mut packed: Global<'kernel, u32, DisjointWrite<Index1D>, Brand<'kernel, Kernel>>,
) {
    let policy = context.numerical_policy::<StrictIeee>();
    let math = context.math();
    let math = math.with_numerical_policy(&policy);
    let value = input.load(context.invocation().index_1d().get()).unwrap_or(0.0);
    let block = context.invocation().index_1d().checked_block::<16, 4>().unwrap();
    assert!(output.store_block(&block, 3, value));
    assert!(packed.store(context.invocation().index_1d().into_disjoint(), 7));
    context.with_workgroup(|workgroup| {
        let subgroup = workgroup.subgroup::<SubgroupWidth64>();
        let wave16 = subgroup.gfx950_wave16(workgroup.epoch());
        let _ = wave16.reduce_sum_f32(math.exp_f32(value));
    });
}
"#,
    );
    assert!(
        output.status.success(),
        "valid capability composition failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn hostile_capability_programs_fail_type_checking() {
    let cases = [
        (
            "forge-context",
            "use fe2o3_device::KernelContext; fn f(){ let _: KernelContext<'static> = KernelContext::__compiler_issue(); }",
            "unsafe function",
        ),
        (
            "wrong-role",
            "use fe2o3_device::{ExclusiveReadWrite,Global,ReadOnly}; fn read<B>(x:&Global<'_,u32,ExclusiveReadWrite,B>){ let _: &Global<'_,u32,ReadOnly,B> = x; }",
            "mismatched types",
        ),
        (
            "cross-brand",
            "use fe2o3_device::{DisjointIndex,DisjointWrite,Global,Index1D}; fn f<A,B>(x:&mut Global<'_,u32,DisjointWrite<Index1D>,A>, i:DisjointIndex<Index1D,B>){ let _=x.store(i,1); }",
            "mismatched types",
        ),
        (
            "wrong-block-geometry",
            "use fe2o3_device::{Blocked,DisjointBlock,DisjointWrite,Global,Index1D}; fn f<B>(x:&mut Global<'_,f32,DisjointWrite<Blocked<Index1D,16,4>>,B>, b:&DisjointBlock<Index1D,64,4,B>){ let _=x.store_block(b,0,1.0); }",
            "mismatched types",
        ),
        (
            "unchecked-index",
            "use fe2o3_device::{Global,ReadOnly}; fn f<B>(x:&Global<'_,u32,ReadOnly,B>)->u32{x[0]}",
            "cannot index",
        ),
    ];
    for (case, source, diagnostic) in cases {
        let output = compile(case, source);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success(), "{case} unexpectedly typechecked");
        assert!(
            stderr.contains(diagnostic),
            "{case} missed diagnostic {diagnostic:?}:\n{stderr}"
        );
    }
}
