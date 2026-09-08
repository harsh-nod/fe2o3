const KERNEL_SOURCE: &str = include_str!("../../../../../../examples/vecadd/src/main.rs");
const HOST_SOURCE: &str = include_str!("../../../../../../examples/vecadd/src/host_app.rs");
const SHARED_BODY: &str = include_str!("../../../../../../examples/vecadd/src/vecadd_body.rs");

#[test]
fn logical_capabilities_map_to_three_physical_slices() {
    for source_type in [
        "context: KernelContext<'_>",
        "a: Global<'_, f32, ReadOnly>",
        "b: Global<'_, f32, ReadOnly>",
        "c: Global<'_, f32, DisjointWrite<Index1D>>",
    ] {
        assert!(
            KERNEL_SOURCE.contains(source_type),
            "missing `{source_type}`"
        );
    }

    for physical_binding in [
        "GeneratedKfdReadSlice::new(a)",
        "GeneratedKfdReadSlice::new(b)",
        "GeneratedKfdWriteSlice::new(c)",
        "super::vecadd_gpu::Arguments::new(",
    ] {
        assert!(
            HOST_SOURCE.contains(physical_binding),
            "missing `{physical_binding}`"
        );
    }
    let context_kernarg = ["Arguments::new(", "context"].concat();
    assert!(!HOST_SOURCE.contains(&context_kernarg));
}

#[test]
fn production_body_has_only_checked_capability_memory_accesses() {
    let start = SHARED_BODY.find("@capability").unwrap();
    let end = start + SHARED_BODY[start..].find("$thread:ident").unwrap();
    let body = &SHARED_BODY[start..end];

    let output_guard = body.find("if i < $output.len()").unwrap();
    let first_load = body.find("$a.load(i)").unwrap();
    assert!(output_guard < first_load, "input load escaped output guard");

    for required in [
        "$a.load(i)",
        "$b.load(i)",
        "$output.store($index.into_disjoint(), $add!(left, right))",
    ] {
        assert!(body.contains(required), "missing `{required}`");
    }
    for raw_access in ["$a[i]", "$b[i]", "get_mut"] {
        assert!(!body.contains(raw_access), "retained `{raw_access}`");
    }
}

#[test]
fn production_entry_remains_fail_closed() {
    let production = HOST_SOURCE.split("#[cfg(test)]").next().unwrap();
    for required in [
        "ProtectedVecaddPrerequisite",
        "AdmittedGeneratedHostContractV2",
        "GeneratedHostDispatchEvidenceV2",
        "PreparedGeneratedHostInvocationV2",
        "ProductionGeneratedHostFactsV2",
        "AuthenticatedWorkerV3ExecutableV1",
        "prepare_direct_kfd_invocation_v2",
    ] {
        assert!(production.contains(required), "missing `{required}`");
    }
    assert!(!production.contains("load_module_from_file"));
    assert!(!production.contains("launch!"));
    assert!(!production.contains("ProductionGeneratedHostFactsV2::from_authenticated"));
    assert!(!production.contains("for_test_only"));
    assert!(!production.contains("unsafe {"));
}

#[test]
fn simulator_consumes_only_bundle_v8_with_canonical_kir_v13() {
    const SIMULATOR: &str = include_str!("../../../../../../examples/vecadd/src/simulator.rs");
    assert!(SIMULATOR.contains("load_debug_simulation_bundle_v8"));
    assert!(SIMULATOR.contains("exact canonical KIR V13 graph"));
    assert!(!SIMULATOR.contains("load_debug_simulation_bundle_v7"));
}
