use fe2o3_workgroup_sync_v1::LDS_REDUCTION_WORKGROUP_V1;

const REDUCTION_SOURCE: &str = include_str!("../src/kernel.rs");
const ATOMIC_SOURCE: &str = include_str!("../src/scoped_atomic.rs");
const README: &str = include_str!("../README.md");

#[test]
fn reduction_is_ordinary_attributed_rust_with_neutral_workgroup_contract() {
    syn::parse_file(REDUCTION_SOURCE).expect("reduction source parses as Rust");
    assert_eq!(LDS_REDUCTION_WORKGROUP_V1, [64, 1, 1]);
    for marker in [
        "#[kernel(",
        "typed,",
        "required = [64, 1, 1],",
        "max = [64, 1, 1],",
        "static_shared_memory_bytes = 256",
        "pub fn lds_publish_read_reduce_i32_v1",
        "DynamicLds::<i32>::exact_current::<64>",
        "WorkgroupCollectives::current()",
        "context.reduce_sum_portable(lds, value)",
        "if lane == 0",
    ] {
        assert!(REDUCTION_SOURCE.contains(marker), "missing {marker}");
    }
    assert!(!REDUCTION_SOURCE.contains("macro_rules!"));
    assert!(!REDUCTION_SOURCE.contains("namespace"));
    assert!(!REDUCTION_SOURCE.contains("from_raw_parts(&group"));
    assert!(!REDUCTION_SOURCE.contains("Invocation3D"));
    assert!(!REDUCTION_SOURCE.contains("WorkgroupCollectiveScratch"));
    assert!(!REDUCTION_SOURCE.contains("*mut i32"));
    assert!(!REDUCTION_SOURCE.contains("unsafe"));
    assert!(!REDUCTION_SOURCE.contains("Gfx942Collectives"));
    assert!(!REDUCTION_SOURCE.contains("Gfx950"));
}

#[test]
fn atomic_source_states_address_space_order_scope_and_eligibility() {
    syn::parse_file(ATOMIC_SOURCE).expect("atomic source parses as Rust");
    for marker in [
        "#[kernel(",
        "typed,",
        "launch(required = [64, 1, 1], max = [64, 1, 1])",
        "pub fn scoped_atomic_add_u32_v1",
        "target: DeviceGlobalMutPtr<u32>",
        "CoreAtomicDefaultScope::System",
        "CORE_ATOMIC_DEFAULT_SCOPE",
        ".as_atomic()",
        "fetch_add(values[lane], Ordering::Relaxed)",
        "if eligible[lane] != 0",
    ] {
        assert!(ATOMIC_SOURCE.contains(marker), "missing {marker}");
    }
    assert!(!ATOMIC_SOURCE.contains("macro_rules!"));
    assert!(!ATOMIC_SOURCE.contains("namespace"));
    assert!(!ATOMIC_SOURCE.contains("include_str!"));
    assert!(!ATOMIC_SOURCE.contains("unsafe"));
    assert!(!ATOMIC_SOURCE.contains("AtomicU32::from_ptr"));
    assert!(!ATOMIC_SOURCE.contains("target.as_raw()"));
}

#[test]
fn documentation_is_explicit_about_later_evidence_phases() {
    for marker in [
        "ordinary attributed Rust",
        "feature-independent production transaction",
        "semantic MIR",
        "ranked PLIRON",
        "verified Kernel IR",
        "compiler-bound handoff",
        "upstream LLVM target APIs",
        "in-process LLD",
        "COV6 inspection",
        "load/launch authority",
        "no workload-profile selector",
        "source-to-code-object evidence",
        "compiler-refinement",
        "no COMGR",
        "no shell linker",
        "fail before output mutation",
    ] {
        assert!(README.contains(marker), "missing boundary: {marker}");
    }
}
