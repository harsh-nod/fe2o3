#[derive(Clone, Copy)]
enum IndexedAtomicInputV1 {
    Shared,
    Exclusive,
    None,
}

fn indexed_atomic_source_v1(input: IndexedAtomicInputV1) -> String {
    let (argument, length, value) = match input {
        IndexedAtomicInputV1::Shared => (
            "inputs: &[u32],",
            "|| inputs.len() != 256",
            "let value = inputs[position];",
        ),
        IndexedAtomicInputV1::Exclusive => (
            "mut inputs: DisjointSlice<u32>,",
            "|| inputs.len() != 256",
            "let Some(input) = inputs.get_mut(thread::index_1d()) else { fe2o3_device::trap(); }; let value = *input;",
        ),
        IndexedAtomicInputV1::None => ("", "", "let value = 17;"),
    };
    format!(
        r#"
#![no_std]
use core::sync::atomic::{{AtomicU32, Ordering}};
use fe2o3_device::{{DisjointSlice, WriteOnlyDisjointSlice, kernel, thread}};
#[kernel(typed, launch(required=[128,1,1], max=[128,1,1], max_grid=[2,1,1]))]
pub fn indexed_atomic_roundtrip(
    channels: &[AtomicU32], {argument} mut output: WriteOnlyDisjointSlice<u32>,
) {{
    if channels.len() != 256 {length} || output.len() != 256 {{
        fe2o3_device::trap();
    }}
    let index = thread::index_1d();
    let position = index.get();
    if position >= 256 {{ return; }}
    {value}
    channels[position].store(value, Ordering::Release);
    let observed = channels[position].load(Ordering::Acquire);
    if !output.write(index, observed) {{ fe2o3_device::trap(); }}
}}
"#
    )
}

#[test]
#[ignore = "requires pinned nightly rust-src, AMD target and production extractor"]
fn indexed_atomic_slice_shared_input_remains_fail_closed_at_alias_analysis() {
    let (output, llvm) =
        run_atomic_slice_admission(&indexed_atomic_source_v1(IndexedAtomicInputV1::Shared));
    assert!(!output.status.success());
    assert!(llvm.is_none());
    assert!(String::from_utf8_lossy(&output.stderr).contains("FE2O3-RACE-002"));
}

fn check_indexed_atomic_llvm_v1(input: IndexedAtomicInputV1) {
    let (output, llvm) = run_atomic_slice_admission(&indexed_atomic_source_v1(input));
    assert!(
        output.status.success(),
        "indexed atomic source failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let llvm = llvm.expect("successful indexed atomic source emitted LLVM");
    assert!(llvm.contains("indexed_atomic_roundtrip"));
    let stores = llvm
        .lines()
        .filter(|line| line.contains("store atomic i32"))
        .collect::<Vec<_>>();
    let loads = llvm
        .lines()
        .filter(|line| line.contains("load atomic i32"))
        .collect::<Vec<_>>();
    assert_eq!(stores.len(), 1, "{llvm}");
    assert_eq!(loads.len(), 1, "{llvm}");
    assert!(stores[0].contains("release"), "{}", stores[0]);
    assert!(loads[0].contains("acquire"), "{}", loads[0]);
    for line in stores.into_iter().chain(loads) {
        assert!(line.contains("align 4"), "{line}");
        assert!(
            !line.contains("syncscope"),
            "System scope must retain LLVM default: {line}"
        );
    }
}

#[test]
#[ignore = "requires pinned nightly rust-src, AMD target and production extractor"]
fn indexed_atomic_slice_two_argument_control_reaches_exact_system_scope_llvm() {
    check_indexed_atomic_llvm_v1(IndexedAtomicInputV1::None);
}

#[test]
#[ignore = "requires pinned nightly rust-src, AMD target and production extractor"]
fn indexed_atomic_slice_exclusive_input_reaches_exact_system_scope_llvm() {
    check_indexed_atomic_llvm_v1(IndexedAtomicInputV1::Exclusive);
}
