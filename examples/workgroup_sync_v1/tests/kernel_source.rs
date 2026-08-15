use fe2o3_workgroup_sync_v1::{
    LDS_REDUCTION_COMPILER_PROFILE_REGISTERED_V1, LDS_REDUCTION_WORKGROUP_V1,
    SCOPED_ATOMIC_COMPILER_PROFILE_REGISTERED_V1, SCOPED_ATOMIC_SOURCE_V1,
};

const REDUCTION_SOURCE: &str = include_str!("../src/kernel.rs");
const README: &str = include_str!("../README.md");
const _: () = assert!(!LDS_REDUCTION_COMPILER_PROFILE_REGISTERED_V1);
const _: () = assert!(!SCOPED_ATOMIC_COMPILER_PROFILE_REGISTERED_V1);

#[test]
fn reduction_is_ordinary_attributed_rust_with_fixed_wave64_contract() {
    syn::parse_file(REDUCTION_SOURCE).expect("reduction source parses as Rust");
    assert_eq!(LDS_REDUCTION_WORKGROUP_V1, [64, 1, 1]);
    for marker in [
        "#[kernel(",
        "typed,",
        "launch(required = [64, 1, 1], max = [64, 1, 1])",
        "pub fn lds_publish_read_reduce_i32_v1",
        "WorkgroupCollectiveScratch::from_raw_parts",
        "group.reduce_sum",
        "if lane == 0",
        "workgroup64_lds_i32_base_v1",
    ] {
        assert!(REDUCTION_SOURCE.contains(marker), "missing {marker}");
    }
    assert!(!REDUCTION_SOURCE.contains("macro_rules!"));
}

#[test]
fn atomic_source_states_address_space_order_scope_and_eligibility() {
    syn::parse_file(SCOPED_ATOMIC_SOURCE_V1).expect("atomic source parses as Rust");
    for marker in [
        "#[kernel(",
        "typed,",
        "launch(required = [64, 1, 1], max = [64, 1, 1])",
        "pub fn scoped_atomic_add_u32_v1",
        "target: DeviceGlobalMutPtr<u32>",
        "CoreAtomicDefaultScope::System",
        "CORE_ATOMIC_DEFAULT_SCOPE",
        "AtomicU32::from_ptr",
        "fetch_add(values[lane], Ordering::Relaxed)",
        "if eligible[lane] != 0",
    ] {
        assert!(SCOPED_ATOMIC_SOURCE_V1.contains(marker), "missing {marker}");
    }
    assert!(!SCOPED_ATOMIC_SOURCE_V1.contains("macro_rules!"));
}

#[test]
fn documentation_is_explicit_about_later_evidence_phases() {
    for marker in [
        "compiler profile authentication",
        "source-to-IR",
        "IR-to-machine correspondence",
        "artifact admission",
        "MI300X execution evidence",
        "no COMGR",
        "no shell linker",
        "fail before output mutation",
    ] {
        assert!(README.contains(marker), "missing boundary: {marker}");
    }
}
