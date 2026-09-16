fn consumed_read_only_source_v1(
    element: &str,
    before_conversion: &str,
    after_conversion: &str,
) -> String {
    format!(
        r#"
#![no_std]
use core::sync::atomic::{{AtomicU32, Ordering}};
use fe2o3_device::{{DisjointSlice, WriteOnlyDisjointSlice, kernel, thread}};
#[kernel(typed, launch(required=[128,1,1], max=[128,1,1], max_grid=[2,1,1]))]
pub fn consumed_read_only_roundtrip(
    mut inputs: DisjointSlice<{element}>, channels: &[AtomicU32],
    mut output: WriteOnlyDisjointSlice<{element}>,
) {{
    if inputs.len() != 256 || channels.len() != 256 || output.len() != 256 {{
        fe2o3_device::trap();
    }}
    {before_conversion}
    let reads = inputs.into_read_only();
    {after_conversion}
    if reads.is_empty() || reads.len() != 256 {{ fe2o3_device::trap(); }}
    let index = thread::index_1d();
    let position = index.get();
    if position >= 256 {{ return; }}
    // Every invocation reads the same input cell. This is not get_mut(index).
    let value = reads.load_or(0, 0 as {element});
    channels[position].store(17, Ordering::Release);
    let observed = channels[position].load(Ordering::Acquire);
    if observed != 17 {{ fe2o3_device::trap(); }}
    if !output.write(index, value) {{ fe2o3_device::trap(); }}
}}
"#
    )
}

#[test]
#[ignore = "requires pinned nightly rust-src, AMD target and production extractor"]
fn consumed_read_only_u16_and_f32_many_reader_sources_reach_checked_llvm() {
    for (element, llvm_type) in [("u16", "i16"), ("f32", "float")] {
        let (output, llvm) =
            run_atomic_slice_admission(&consumed_read_only_source_v1(element, "", ""));
        assert!(
            output.status.success(),
            "{element}:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let llvm = llvm.expect("successful readonly source emitted LLVM");
        assert!(llvm.contains("consumed_read_only_roundtrip"));
        assert!(
            llvm.contains(&format!("load {llvm_type}, ptr addrspace(1)")),
            "{llvm}"
        );
        let atomic: Vec<_> = llvm
            .lines()
            .filter(|line| line.contains("load atomic i32") || line.contains("store atomic i32"))
            .collect();
        assert_eq!(atomic.len(), 2, "{llvm}");
        assert!(atomic.iter().any(|line| line.contains(" acquire,")));
        assert!(atomic.iter().any(|line| line.contains(" release,")));
        assert!(
            atomic
                .iter()
                .all(|line| !line.contains("syncscope") && line.contains("align 4"))
        );
    }
}

#[test]
#[ignore = "requires pinned nightly rust-src, AMD target and production extractor"]
fn consumed_read_only_source_rejects_writes_on_every_control_path() {
    for before in [
        "let Some(cell) = inputs.get_mut(thread::index_1d()) else { fe2o3_device::trap(); }; *cell = 0;",
        "if thread::index_1d().get() % 2 == 0 { let Some(cell) = inputs.get_mut(thread::index_1d()) else { fe2o3_device::trap(); }; *cell = 0; return; }",
    ] {
        let (output, llvm) =
            run_atomic_slice_admission(&consumed_read_only_source_v1("u16", before, ""));
        assert!(
            !output.status.success(),
            "a readonly source with writes was admitted"
        );
        assert!(llvm.is_none());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("consumed readonly") || stderr.contains("readonly root"),
            "wrong boundary:\n{stderr}"
        );
    }
}

#[test]
#[ignore = "requires pinned nightly rust-src, AMD target and production extractor"]
fn consumed_read_only_source_keeps_rust_move_and_sealed_element_boundaries() {
    for (element, after, expected) in [
        ("u16", "let _ = inputs.len();", "moved value"),
        ("u32", "", "ReadOnlyAllocationElement"),
    ] {
        let (output, llvm) =
            run_atomic_slice_admission(&consumed_read_only_source_v1(element, "", after));
        assert!(!output.status.success());
        assert!(llvm.is_none());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(expected),
            "wrong source boundary:\n{stderr}"
        );
    }
}

#[test]
#[ignore = "requires pinned nightly rust-src, AMD target and production extractor"]
fn consumed_read_only_source_rejects_nominal_and_index_space_spoofs() {
    let wrong_space = r#"
#![no_std]
use fe2o3_device::{DisjointSlice, GridExclusive, kernel};
#[kernel(typed, launch(required=[128,1,1], max=[128,1,1], max_grid=[2,1,1]))]
pub fn wrong_space(inputs: DisjointSlice<u16, GridExclusive>) {
    let _ = inputs.into_read_only();
}
"#;
    let spoof = r#"
#![no_std]
use fe2o3_device::kernel;
#[repr(C)]
pub struct ReadOnlyAllocation { ptr: *const u16, len: usize }
#[kernel(typed, launch(required=[128,1,1], max=[128,1,1], max_grid=[2,1,1]))]
pub fn spoof(inputs: ReadOnlyAllocation) { let _ = inputs; }
"#;
    for (source, expected) in [
        (wrong_space, "into_read_only"),
        (spoof, "by-value aggregate contains a pointer or reference"),
    ] {
        let (output, llvm) = run_atomic_slice_admission(source);
        assert!(!output.status.success());
        assert!(llvm.is_none());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(expected),
            "wrong source boundary:\n{stderr}"
        );
    }
}
