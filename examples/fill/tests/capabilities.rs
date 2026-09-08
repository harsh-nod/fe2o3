use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

struct Scratch(PathBuf);

impl Scratch {
    fn new(label: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = PathBuf::from("/dev/shm")
            .join(format!("fe2o3-fill-{label}-{}-{nonce}", std::process::id()));
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
        format!(
            "[package]\nname='fill-ui-{case}'\nversion='0.0.0'\nedition='2024'\npublish=false\n[workspace]\n[dependencies]\nfe2o3-device={{path={device:?}}}\n"
        ),
    )
    .unwrap();
    std::fs::write(scratch.0.join("src/lib.rs"), source).unwrap();
    Command::new(env!("CARGO"))
        .current_dir(&scratch.0)
        .env("CARGO_TARGET_DIR", scratch.0.join("target"))
        .env("RUSTUP_TOOLCHAIN", "nightly-2026-04-03")
        .env_remove("RUSTC")
        .env_remove("RUSTC_WRAPPER")
        .env_remove("RUSTC_WORKSPACE_WRAPPER")
        .args(["check", "--offline", "--quiet"])
        .output()
        .unwrap()
}

#[test]
fn attributed_source_has_one_logical_context_and_one_typed_global() {
    let source = include_str!("../src/lib.rs");
    let file = syn::parse_file(source).unwrap();
    let kernel = file
        .items
        .iter()
        .find_map(|item| match item {
            syn::Item::Fn(function)
                if function
                    .attrs
                    .iter()
                    .any(|attribute| attribute.path().is_ident("kernel")) =>
            {
                Some(function)
            }
            _ => None,
        })
        .unwrap();
    assert_eq!(kernel.sig.inputs.len(), 2);
    for required in [
        "context: KernelContext<'_>",
        "Global<'_, f32, DisjointWrite<Index1D>>",
        "context.invocation().index_1d()",
        "index.into_disjoint()",
    ] {
        assert!(source.contains(required), "missing {required}");
    }
    for forbidden in ["DisjointSlice", "thread::", "::current()"] {
        assert!(!source.contains(forbidden), "retained {forbidden}");
    }
}

#[test]
fn capability_shape_typechecks() {
    let output = compile(
        "pass",
        r#"use fe2o3_device::{DisjointWrite, Global, Index1D};
fn store<Brand>(output: &mut Global<'_, f32, DisjointWrite<Index1D>, Brand>, index: fe2o3_device::DisjointIndex<Index1D, Brand>) {
    let _ = output.store(index, 42.5);
}"#,
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn forgery_brand_role_launch_and_host_substitutions_fail() {
    const CASES: [(&str, &str, &str); 4] = [
        (
            "forgery",
            "use fe2o3_device::KernelContext; fn f(){ let _: KernelContext<'static> = KernelContext::__compiler_issue(); }",
            "unsafe function",
        ),
        (
            "brand",
            "use fe2o3_device::{DisjointIndex,DisjointWrite,Global,Index1D}; fn f<A,B>(o:&mut Global<'_,u32,DisjointWrite<Index1D>,A>,i:DisjointIndex<Index1D,B>){let _=o.store(i,1);}",
            "mismatched types",
        ),
        (
            "role",
            "use fe2o3_device::{DisjointWrite,Global,Index1D}; fn f<B>(o:&Global<'_,u32,DisjointWrite<Index1D>,B>){let _=o.load(0);}",
            "no method named `load`",
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
fn cpu_reference_preserves_unlaunched_tail() {
    let mut output = [-1.0_f32; 7];
    fe2o3_fill::fill_cpu_reference(&mut output, 4);
    assert_eq!(output, [42.5, 42.5, 42.5, 42.5, -1.0, -1.0, -1.0]);
}
