use fe2o3_kernel_ir::*;

#[allow(dead_code)]
mod fixture {
    use fe2o3_kernel_ir::*;
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../fe2o3-kernel-ir/src/execution_capability_v1/subgroup_partition/ordered_max_v1/fixture.rs"
    ));
}

#[test]
fn ordered_partition_max_reaches_gfx950_compare_select_tree_without_fmax_or_sum() {
    let module = fixture::module();
    let canonical = VerifiedCanonicalKernelIrV13::from_module(module).unwrap();
    let launch =
        crate::ProductionTargetLaunchEvidenceV13::for_static_launches(&canonical, 0).unwrap();
    let lowered = crate::lower_verified_canonical_kir_v13_to_amd_llvm_ir_v1(
        &canonical,
        0,
        &launch,
        fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx950,
    )
    .unwrap();
    assert!(!lowered.grants_load_authority());
    assert!(!lowered.grants_launch_authority());
    let ir = lowered.llvm_ir();
    let comparisons = ir
        .lines()
        .filter(|line| line.contains(" = fcmp olt float "))
        .collect::<Vec<_>>();
    assert_eq!(comparisons.len(), 4, "{ir}");
    let partners = ir
        .lines()
        .filter(|line| line.contains(".source.") && line.contains(" = xor i32 "))
        .collect::<Vec<_>>();
    assert_eq!(partners.len(), 4, "{ir}");
    for (index, (line, distance)) in partners.iter().zip([1, 2, 4, 8]).enumerate() {
        assert!(
            line.trim_end().ends_with(&format!(", {distance}")),
            "stage {index}: {line}"
        );
        let comparison = comparisons[index];
        assert!(
            comparison.contains(&format!(".remote.{index}")),
            "{comparison}"
        );
        let select = ir
            .lines()
            .find(|line| line.contains(&format!(".less.{index}, float")))
            .unwrap();
        assert!(
            select.contains(&format!(".remote.{index}, float")),
            "right chosen only on ordered less: {select}"
        );
        if index > 0 {
            assert!(
                select.ends_with(&format!(".reduce.{}", index - 1)),
                "retain exact prior lhs: {select}"
            );
        }
    }
    for forbidden in [
        "fadd float",
        "llvm.max",
        "llvm.maximum",
        "maxnum",
        "fmax",
        "fcmp fast",
        "select fast",
        "nnan",
        "nsz",
    ] {
        assert!(
            !ir.contains(forbidden),
            "unreviewed numeric behavior {forbidden}: {ir}"
        );
    }
}
