include!("static_publication_handoff_v1.rs");

fn static_publication_source_v1(before: &str, after: &str, required: u32) -> String {
    format!(
        r#"
#![no_std]
use core::sync::atomic::AtomicU32;
use fe2o3_device::{{DisjointSlice, WriteOnlyDisjointSlice, kernel, publish_once_128, thread}};
#[kernel(typed, launch(required=[{required},1,1], max=[{required},1,1], max_grid=[2,1,1]))]
pub fn static_publication_roundtrip(
    mut payload: DisjointSlice<f32>, flags: &[AtomicU32],
    mut output: WriteOnlyDisjointSlice<f32>,
) {{
    if output.len() != 256 {{ fe2o3_device::trap(); }}
    {before}
    let position = thread::index_1d().get();
    let attempt = publish_once_128(payload, flags, position as u32 as f32 + 1.0);
    {after}
    let index = thread::index_1d();
    if !output.write(index, attempt.value) {{ fe2o3_device::trap(); }}
}}
"#
    )
}

#[test]
#[ignore = "requires pinned nightly rust-src, AMD target and production extractor"]
fn static_publication_source_reaches_checked_llvm_with_exact_atomic_effects() {
    assert_static_publication_llvm_v1(&static_publication_source_v1("", "", 128));
}

#[test]
#[ignore = "requires pinned nightly rust-src, AMD target and production extractor"]
fn static_publication_source_accepts_independent_consumed_read_only_input() {
    let source = r#"
#![no_std]
use core::sync::atomic::AtomicU32;
use fe2o3_device::{DisjointSlice, WriteOnlyDisjointSlice, kernel, publish_once_128, thread};
#[kernel(typed, launch(required=[128,1,1], max=[128,1,1], max_grid=[2,1,1]))]
pub fn static_publication_read_only_input(
    payload: DisjointSlice<f32>, flags: &[AtomicU32], input: DisjointSlice<f32>,
    mut statuses: WriteOnlyDisjointSlice<u32>,
    mut values: WriteOnlyDisjointSlice<f32>,
) {
    if input.len() != 128 || statuses.len() != 256 || values.len() != 256 {
        fe2o3_device::trap();
    }
    let input = input.into_read_only();
    let position = thread::index_1d().get();
    let producer_value = input.load_or(position % 128, 0.0);
    let attempt = publish_once_128(payload, flags, producer_value);
    if !statuses.write(thread::index_1d(), attempt.status) { fe2o3_device::trap(); }
    if !values.write(thread::index_1d(), attempt.value) { fe2o3_device::trap(); }
}
"#;
    assert_static_publication_llvm_v1(source);
}

fn assert_static_publication_llvm_v1(source: &str) {
    let (output, llvm) = run_atomic_slice_admission(source);
    assert!(
        output.status.success(),
        "publication LLVM extraction failed:\n{}",
        String::from_utf8_lossy(&output.stderr),
    );
    let llvm = llvm.expect("successful publication extraction contains LLVM");
    let atomic: Vec<_> = llvm
        .lines()
        .filter(|line| line.contains("load atomic i32") || line.contains("store atomic i32"))
        .collect();
    assert_eq!(atomic.len(), 3, "{llvm}");
    assert_eq!(
        atomic
            .iter()
            .filter(|line| line.contains(" release,"))
            .count(),
        2
    );
    assert_eq!(
        atomic
            .iter()
            .filter(|line| line.contains(" acquire,"))
            .count(),
        1
    );
    assert!(
        atomic
            .iter()
            .all(|line| !line.contains("syncscope") && line.contains("align 4")),
        "{llvm}"
    );
    assert!(llvm.contains("load float, ptr addrspace(1)"), "{llvm}");
    assert!(llvm.contains("store float"), "{llvm}");
}

#[test]
#[ignore = "requires pinned nightly rust-src, AMD target and production extractor"]
fn static_publication_source_handoff_requires_authenticated_functional_evidence() {
    let source = static_publication_source_v1("", "", 128);
    assert_static_publication_llvm_v1(&source);
    let (output, handoff) = run_static_publication_handoff_v1(&source);
    assert!(
        !output.status.success(),
        "missing functional evidence was admitted"
    );
    assert!(handoff.is_none(), "failed lineage emitted a handoff");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr
            .contains("every production root requires authenticated MIR-to-PLIRON Verus execution"),
        "expected the final lineage evidence boundary, not an earlier rejection:\n{stderr}",
    );
    assert!(!stderr.contains("proof-carrying semantic compiler-bound inert handoff"));
}

#[test]
#[ignore = "requires pinned nightly rust-src, AMD target and production extractor"]
fn static_publication_source_rejects_extra_root_uses_and_wrong_geometry() {
    for (before, after, required) in [
        (
            "let Some(cell)=payload.get_mut(thread::index_1d()) else { fe2o3_device::trap(); }; *cell=0.0;",
            "",
            128,
        ),
        (
            "flags[0].store(2,core::sync::atomic::Ordering::Release);",
            "",
            128,
        ),
        (
            "",
            "flags[0].store(2,core::sync::atomic::Ordering::Release);",
            128,
        ),
        ("", "", 64),
    ] {
        let (output, llvm) =
            run_atomic_slice_admission(&static_publication_source_v1(before, after, required));
        assert!(
            !output.status.success(),
            "unexpected publication admission for WG{required}, before={before}, after={after}"
        );
        assert!(llvm.is_none());
    }
}

#[test]
#[ignore = "requires pinned nightly rust-src, AMD target and production extractor"]
fn static_publication_source_keeps_move_privacy_and_nominal_boundaries() {
    let moved = static_publication_source_v1("", "let _ = payload.len();", 128);
    let private = r#"
#![no_std]
use core::sync::atomic::AtomicU32;
use fe2o3_device::{DisjointSlice,kernel};
#[kernel(typed,launch(required=[128,1,1],max=[128,1,1],max_grid=[2,1,1]))]
pub fn private_call(payload:DisjointSlice<f32>,flags:&[AtomicU32]) {
    let _=fe2o3_device::static_publication::publish_cell_128(payload,flags,0,1.0);
}
"#;
    let spoof = r#"
#![no_std]
use fe2o3_device::kernel;
#[repr(C)] pub struct PublicationAttemptF32 { pub status:u32,pub value:f32 }
#[kernel(typed,launch(required=[128,1,1],max=[128,1,1],max_grid=[2,1,1]))]
pub fn lookalike(value:u32) {
    let forged=PublicationAttemptF32 {status:value,value:0.0};
    let _:fe2o3_device::PublicationAttemptF32=forged;
}
"#;
    for (source, expected) in [
        (moved.as_str(), "moved value"),
        (private, "private"),
        (spoof, "mismatched types"),
    ] {
        let (output, llvm) = run_atomic_slice_admission(source);
        assert!(!output.status.success(), "a source boundary was bypassed");
        assert!(llvm.is_none());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains(expected), "wrong boundary: {stderr}");
    }
}
