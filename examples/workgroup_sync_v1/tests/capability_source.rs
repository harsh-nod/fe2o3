const COLLECTIVE: &str = include_str!("../src/capability_collectives.rs");
const KERNELS: [&str; 10] = [
    include_str!("../src/kernel.rs"),
    include_str!("../src/kernel_u32.rs"),
    include_str!("../src/kernel_f32.rs"),
    include_str!("../src/kernel_scan_u32.rs"),
    include_str!("../src/kernel_scan_u32_exclusive.rs"),
    include_str!("../src/kernel_scan_i32.rs"),
    include_str!("../src/kernel_scan_i32_inclusive.rs"),
    include_str!("../src/kernel_scan_f32.rs"),
    include_str!("../src/kernel_scan_f32_exclusive.rs"),
    include_str!("../src/scoped_atomic.rs"),
];

#[test]
fn every_attributed_root_uses_the_context_and_typed_global_surface() {
    let source = KERNELS.join("\n");
    assert_eq!(source.matches("#[kernel(").count(), 22);
    assert_eq!(source.matches("context: KernelContext<'_>").count(), 22);
    assert!(source.matches("Global<'_,").count() >= 44);
    for forbidden in [
        "::current()",
        "from_raw_parts",
        "DeviceGlobalMutPtr",
        "DynamicLds",
        "WorkgroupCollectives",
        "Gfx942",
        "Gfx950",
        "IrBuilder",
        "namespace =",
        "*mut",
    ] {
        assert!(!source.contains(forbidden), "retained {forbidden}");
    }
}

#[test]
fn one_generic_shape_carries_publication_barrier_and_reuse_epochs() {
    let ordered = [
        "allocate_lds::<T, ELEMENTS>()",
        "initialize_by_invocation(&workgroup, value)",
        "workgroup.publish_lds(initialized)",
        "published.read(&workgroup",
        "barrier::<WorkgroupScope, AcquireRelease, WorkgroupMemory>()",
        "workgroup.reduce_sum(scratch, value)",
        "workgroup.inclusive_scan_sum(scratch, value)",
        "workgroup.exclusive_scan_sum(scratch, value)",
    ];
    let mut previous = 0;
    for marker in ordered {
        let position = COLLECTIVE
            .find(marker)
            .unwrap_or_else(|| panic!("missing {marker}"));
        assert!(position >= previous, "{marker} is reordered");
        previous = position;
    }
    assert_eq!(COLLECTIVE.matches("let scratch =").count(), 1);
    assert_eq!(COLLECTIVE.matches("(workgroup, scratch,").count(), 2);
    assert!(!COLLECTIVE.contains("::current()"));
    assert!(!COLLECTIVE.contains("unsafe"));
}
