use fe2o3_gfx950_gpt_oss_decode::{
    CONTEXT_TOKENS, EXPERTS, HIDDEN_SIZE, MATRIX_ROWS, MAX_WORKGROUPS, OPENAI_GPT_OSS_COMMIT,
    PROFILE_BOUNDARY, PROFILE_ITEMS, WAVE_SIZE, WAVES_PER_WORKGROUP, WORKGROUP_SIZE,
};

const KERNEL_SOURCES: [&str; 7] = [
    include_str!("../src/kernel.rs"),
    include_str!("../src/kernel_components.rs"),
    include_str!("../src/kernel_held_fragments.rs"),
    include_str!("../src/kernel_interleaved_stores.rs"),
    include_str!("../src/kernel_pipelined_attention.rs"),
    include_str!("../src/kernel_router_serial.rs"),
    include_str!("../src/kernel_scalar_attention.rs"),
];

#[test]
fn every_entry_uses_the_issue_272_logical_surface() {
    for source in KERNEL_SOURCES {
        for signature in source.match_indices("pub fn gfx950_gpt_oss") {
            let suffix = &source[signature.0..];
            let open = suffix.find('(').expect("kernel parameter list");
            let close = suffix
                .find(") -> KernelResult")
                .expect("kernel return type");
            let parameters = &suffix[open + 1..close];
            assert!(
                parameters
                    .trim_start()
                    .starts_with("mut context: KernelContext<'_>")
            );
            assert!(parameters.contains("Global<'_"));
            if parameters.contains("attention_output") || parameters.contains("expert_output") {
                assert!(parameters.contains("DisjointWrite<Blocked<Index1D, 16, 4>>"));
            }
            if parameters.contains("mut packed_top4") {
                assert!(parameters.contains("DisjointWrite<Index1D>"));
            }
            assert!(!parameters.contains("&["));
            assert!(!parameters.contains("&mut ["));
            assert!(!parameters.contains("DisjointSlice"));
            assert!(!parameters.contains("ExclusiveReadWrite"));
        }
        for forbidden in [
            "::current()",
            "thread::index_1d()",
            "StridedReadView2D::from_shared_slice",
            "Bf16MfmaAMatrix::row_major",
            "Gfx950Fp4MfmaAMatrix::row_major",
            "Gfx950Fp4MfmaBMatrix::row_major",
        ] {
            assert!(!source.contains(forbidden), "retained {forbidden}");
        }
        assert!(!source.contains("unsafe"));
    }
}

#[test]
fn shared_compute_is_policy_global_and_epoch_bound() {
    let source = include_str!("../src/capability_views.rs");
    for required in [
        "context.numerical_policy::<StrictIeee>()",
        "context.math()",
        "with_numerical_policy(&policy)",
        "context.with_workgroup",
        "workgroup.subgroup::<SubgroupWidth64>()",
        "workgroup.publish_lds(logits)",
        "subgroup.gfx950_wave16(workgroup.epoch())",
        "bf16_a_global_row_major",
        "bf16_b_global_row_major",
        "fp4_a_global_row_major",
        "fp4_b_global_row_major",
        "matrix.fp4_zero_accumulator(lane)",
        "checked_block::<16, 4>()",
        "output.store_block(&block, 3, values[3])",
        "index_1d().into_disjoint()",
    ] {
        assert!(source.contains(required), "missing {required}");
    }
    for forbidden in ["::current()", "unsafe", "DisjointSlice", "&[f32]", "&[u8]"] {
        assert!(!source.contains(forbidden), "retained {forbidden}");
    }
}

#[test]
fn package_selection_is_explicit_and_not_source_name_driven() {
    let library = include_str!("../src/lib.rs");
    assert!(library.contains("GPT-OSS full-kernel features are mutually exclusive"));
    for forbidden in ["source_name", "file_name", "ends_with(\"kernel"] {
        assert!(
            !library.contains(forbidden),
            "retained selector {forbidden}"
        );
    }
}

#[test]
fn ablations_preserve_their_algorithmic_delta() {
    let serial = include_str!("../src/kernel_router_serial.rs");
    assert!(serial.contains("select_serial_top4"));

    let held = include_str!("../src/kernel_held_fragments.rs");
    assert!(held.contains("compute_expert_held"));
    let helpers = include_str!("../src/capability_views.rs");
    let serial_start = helpers
        .find("fn select_serial_top4")
        .expect("serial router helper");
    let serial_helper = &helpers[serial_start..];
    assert!(serial_helper.contains("while expert < EXPERTS"));
    assert!(serial_helper.contains("while depth < crate::HIDDEN_SIZE"));
    let held_start = helpers.find("fn compute_expert_held").expect("held helper");
    let held = &helpers[held_start..];
    assert!(held.find("let activation3").unwrap() < held.find("let expert0").unwrap());

    let scalar = include_str!("../src/kernel_scalar_attention.rs");
    assert!(scalar.contains("compute_scalar_attention"));
    assert!(helpers.contains("fn widen_bf16"));

    let interleaved = include_str!("../src/kernel_interleaved_stores.rs");
    let attention0 = interleaved.find("attention_output.store").unwrap();
    let expert0 = interleaved.find("expert_output.store").unwrap();
    let attention1 = interleaved[attention0 + 1..]
        .find("attention_output.store")
        .unwrap()
        + attention0
        + 1;
    assert!(attention0 < expert0 && expert0 < attention1);

    let pipelined = include_str!("../src/kernel_pipelined_attention.rs");
    assert!(pipelined.contains("safe functional control"));
    assert!(pipelined.contains("reusable branded LDS-to-MFMA bridge is unavailable"));
    assert!(!pipelined.contains("WorkgroupPipeline"));
}

#[test]
fn fixed_profile_contract_is_unchanged() {
    assert_eq!(OPENAI_GPT_OSS_COMMIT.len(), 40);
    assert!(PROFILE_BOUNDARY.contains("batch=1"));
    assert!(PROFILE_BOUNDARY.contains("full 128-way top-4 router"));
    assert!(PROFILE_BOUNDARY.contains("selected top-1 MLP1"));
    assert_eq!((HIDDEN_SIZE, CONTEXT_TOKENS, MATRIX_ROWS), (2880, 16, 16));
    assert_eq!((EXPERTS, WAVE_SIZE, WORKGROUP_SIZE), (128, 64, 256));
    assert_eq!(
        (WAVES_PER_WORKGROUP, MAX_WORKGROUPS, PROFILE_ITEMS),
        (4, 4, 16)
    );
}
