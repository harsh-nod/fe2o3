#[test]
fn attributed_gemm_uses_only_compiler_issued_capabilities() {
    let source = include_str!("../src/kernel.rs");

    for required in [
        "context: KernelContext<'_>",
        "a: Global<'_, u16, ReadOnly>",
        "b: Global<'_, u16, ReadOnly>",
        "c: Global<'_, f32, ExclusiveReadWrite>",
        "context.subgroup_lane::<SubgroupWidth64>()",
        "context.numerical_policy::<StrictIeee>()",
    ] {
        assert!(
            source.contains(required),
            "missing capability contract: {}",
            required
        );
    }
    for forbidden in [
        "WaveLane::<Wave64>::current",
        "Matrix::current",
        "WorkgroupLdsScope::current",
        "a: &[u16]",
        "b: &[u16]",
        "c: DisjointSlice",
    ] {
        assert!(
            !source.contains(forbidden),
            "ambient or raw contract returned: {}",
            forbidden
        );
    }
}
