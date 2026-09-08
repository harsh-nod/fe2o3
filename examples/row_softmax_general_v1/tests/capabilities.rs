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
            "fe2o3-row-softmax-{case}-{}-{nonce}",
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
        format!("[package]\nname='row-softmax-ui-{case}'\nversion='0.0.0'\nedition='2024'\npublish=false\n[workspace]\n[dependencies]\nfe2o3-device={{path={device:?}}}\n"),
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
fn source_uses_only_context_derived_execution_and_typed_memory() {
    let source = include_str!("../src/kernel.rs");
    let file = syn::parse_file(source).unwrap();
    let kernels = file
        .items
        .iter()
        .filter(|item| matches!(item, syn::Item::Fn(function) if function.attrs.iter().any(|attribute| attribute.path().is_ident("kernel"))))
        .count();
    assert_eq!(kernels, 1);
    for required in [
        "context: KernelContext<'_>",
        "input: Global<'_, f32, ReadOnly>",
        "output: Global<'_, f32, ExclusiveReadWrite>",
        "context.invocation().index_1d()",
        "context.numerical_policy::<StrictIeee>()",
        "context.with_workgroup(|workgroup|",
        "workgroup.publish_memory(maxima)",
        "subgroup.reduce_sum(workgroup.epoch(), local_sum)",
    ] {
        assert!(source.contains(required), "missing {required}");
    }
    for forbidden in [
        "DisjointSlice",
        "thread::",
        "::current()",
        "input: &[",
        "Math::",
    ] {
        assert!(!source.contains(forbidden), "retained {forbidden}");
    }
}

#[test]
fn policy_bound_math_and_typed_global_shape_typecheck() {
    let output = compile(
        "pass",
        r#"use fe2o3_device::{Global,ReadOnly,ExclusiveReadWrite,PolicyDeviceMath,StrictIeee};
fn body<B>(input:&Global<'_,f32,ReadOnly,B>, output:&mut Global<'_,f32,ExclusiveReadWrite,B>, math:&PolicyDeviceMath<'_,B,StrictIeee>){
 if let Some(value)=input.load(0){let _=output.store(0,math.exp_f32(value));}
}"#,
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn wrong_role_launch_and_host_use_fail_during_type_checking() {
    const CASES: [(&str, &str, &str); 3] = [
        (
            "role",
            "use fe2o3_device::{Global,ReadOnly,ExclusiveReadWrite}; fn read<B>(_:&Global<'_,f32,ReadOnly,B>){} fn f<B>(x:&Global<'_,f32,ExclusiveReadWrite,B>){read(x);}",
            "mismatched types",
        ),
        (
            "launch",
            "use fe2o3_device::{Global,KernelCapabilityBrand,KernelLaunch,KernelTarget,ReadOnly}; fn take<'a,K,T:KernelTarget,L:KernelLaunch>(_:&Global<'a,f32,ReadOnly,KernelCapabilityBrand<'a,K,T,L>>){} fn f<'a,K,T:KernelTarget,L:KernelLaunch,R:KernelLaunch>(x:&Global<'a,f32,ReadOnly,KernelCapabilityBrand<'a,K,T,R>>){take::<K,T,L>(x);}",
            "mismatched types",
        ),
        (
            "host",
            "use fe2o3_device::KernelContext; fn sync<T:Sync>(){} fn f(){sync::<KernelContext<'static>>();}",
            "cannot be shared between threads safely",
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
fn semantic_mutations_diverge_from_the_cpu_reference() {
    use fe2o3_row_softmax_general_v1::reference::{ReferenceLayoutV1, evaluate_reference_v1};
    let input = [4.0_f32, 1.0, -2.0, 0.0];
    let expected = evaluate_reference_v1(
        &input,
        &[0.0; 4],
        ReferenceLayoutV1 {
            rows: 1,
            columns: 4,
            input_stride: 4,
            output_stride: 4,
        },
    )
    .unwrap();
    let wrong_denominator = input.iter().map(|value| value.exp()).sum::<f32>() + 1.0;
    let mutated = input.map(|value| value.exp() / wrong_denominator);
    assert!(
        expected
            .iter()
            .zip(mutated)
            .any(|(left, right)| left.to_bits() != right.to_bits())
    );
}
