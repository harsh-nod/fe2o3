use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

struct Scratch(PathBuf);

impl Scratch {
    fn new(case: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = PathBuf::from("/dev/shm").join(format!(
            "fe2o3-moe-top2-{case}-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir_all(path.join("src")).unwrap();
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
        .unwrap();
    std::fs::write(
        scratch.0.join("Cargo.toml"),
        format!("[package]\nname='moe-top2-ui-{case}'\nversion='0.0.0'\nedition='2024'\npublish=false\n[workspace]\n[dependencies]\nfe2o3-device={{path={device:?}}}\n"),
    )
    .unwrap();
    std::fs::write(scratch.0.join("src/lib.rs"), source).unwrap();
    Command::new(env!("CARGO"))
        .current_dir(&scratch.0)
        .env(
            "CARGO_TARGET_DIR",
            std::env::var_os("CARGO_TARGET_DIR")
                .expect("managed tests require the shared CARGO_TARGET_DIR"),
        )
        .env("CARGO_INCREMENTAL", "0")
        .env("RUSTUP_TOOLCHAIN", "nightly-2026-04-03")
        .env_remove("RUSTC")
        .env_remove("RUSTC_WRAPPER")
        .env_remove("RUSTC_WORKSPACE_WRAPPER")
        .args(["check", "--offline", "--quiet"])
        .output()
        .unwrap()
}

#[test]
fn attributed_source_has_no_ambient_or_legacy_memory_authority() {
    let source = include_str!("../src/kernel.rs");
    for required in [
        "context: KernelContext<'_>",
        "logits: Global<'_, f32, ReadOnly>",
        "let Some(grid) = context.grid()",
        "let Some(leader) = grid.leader()",
        "output.store(leader.index(index), value)",
    ] {
        assert!(source.contains(required), "missing {required}");
    }
    assert!(
        source
            .matches("Global<'_, u32, DisjointWrite<GridExclusive>>")
            .count()
            >= 7
    );
    for forbidden in [
        "ExclusiveReadWrite",
        "WriteOnlyDisjointSlice",
        "thread::grid_leader",
        "::current()",
        "logits: &[",
    ] {
        assert!(!source.contains(forbidden), "retained {forbidden}");
    }
}

#[test]
fn read_only_input_and_grid_exclusive_output_shape_typechecks() {
    let output = compile(
        "pass",
        "use fe2o3_device::{DisjointWrite,Global,GridExclusive,GridLeader,ReadOnly}; fn f<B>(i:&Global<'_,f32,ReadOnly,B>,o:&mut Global<'_,u32,DisjointWrite<GridExclusive>,B>,leader:&GridLeader<B>){if i.load(0).is_some(){let _=o.store(leader.index(0),1);}}",
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn forgery_wrong_role_brand_and_host_use_fail() {
    const CASES: [(&str, &str, &str); 5] = [
        (
            "forgery",
            "use fe2o3_device::KernelContext; fn f(){let _:KernelContext<'static>=KernelContext::__compiler_issue();}",
            "unsafe function",
        ),
        (
            "role",
            "use fe2o3_device::{ExclusiveReadWrite,Global,ReadOnly}; fn take<B>(_:&Global<'_,u32,ReadOnly,B>){} fn f<B>(x:&Global<'_,u32,ExclusiveReadWrite,B>){take(x);}",
            "mismatched types",
        ),
        (
            "brand",
            "use fe2o3_device::{ExclusiveReadWrite,Global}; fn take<B>(_:&Global<'_,u32,ExclusiveReadWrite,B>){} fn f<A,B>(x:&Global<'_,u32,ExclusiveReadWrite,A>){take::<B>(x);}",
            "mismatched types",
        ),
        (
            "leader-brand",
            "use fe2o3_device::{DisjointWrite,Global,GridExclusive,GridLeader}; fn f<A,B>(x:&mut Global<'_,u32,DisjointWrite<GridExclusive>,A>,leader:&GridLeader<B>){let _=x.store(leader.index(0),1);}",
            "mismatched types",
        ),
        (
            "host",
            "use fe2o3_device::KernelContext; fn send<T:Send>(){} fn f(){send::<KernelContext<'static>>();}",
            "cannot be sent between threads safely",
        ),
    ];
    for (case, source, expected) in CASES {
        let output = compile(case, source);
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(!output.status.success(), "{case} compiled");
        assert!(stderr.contains(expected), "{case}: {stderr}");
    }
}

#[test]
fn tie_and_capacity_mutations_diverge_from_the_independent_oracle() {
    use fe2o3_moe_top2_v1::{RoutingOutputsV1, deterministic_vectors_v1, moe_top2_oracle_v1};
    let vector = deterministic_vectors_v1()[1];
    let mut expected = RoutingOutputsV1::filled(0);
    moe_top2_oracle_v1(&vector.logits, &mut expected).unwrap();
    let mut higher_id_tie_break = expected.clone();
    higher_id_tie_break.top2_experts[0] = 3;
    assert_ne!(higher_id_tie_break, expected);
    let mut unbounded_capacity = expected.clone();
    unbounded_capacity.admitted_counts[0] = unbounded_capacity.requested_counts[0];
    assert_ne!(unbounded_capacity, expected);
}
