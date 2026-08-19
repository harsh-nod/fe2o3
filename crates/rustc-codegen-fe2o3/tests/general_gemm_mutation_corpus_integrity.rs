#![forbid(unsafe_code)]

const BASELINE: &str =
    include_str!("fixtures/general-gemm-semantic-frontend/src/bin/valid_proof_sensitive.rs");
const MANIFEST: &str = include_str!("fixtures/general-gemm-semantic-frontend/Cargo.toml");

struct Mutation {
    id: &'static str,
    bin: &'static str,
    source: &'static str,
    before: &'static str,
    after: &'static str,
}

macro_rules! mutation {
    ($id:literal, $bin:literal, $file:literal, $before:literal, $after:literal) => {
        Mutation {
            id: $id,
            bin: $bin,
            source: include_str!(concat!(
                "fixtures/general-gemm-semantic-frontend/src/bin/",
                $file,
                ".rs"
            )),
            before: $before,
            after: $after,
        }
    };
}

const MUTATIONS: &[Mutation] = &[
    mutation!(
        "unguarded_a_tail_load",
        "unguarded-a-tail-load",
        "unguarded_a_tail_load",
        "let a_value = if row < m && depth < k {",
        "let a_value = if depth < k {"
    ),
    mutation!(
        "unguarded_b_tail_load",
        "unguarded-b-tail-load",
        "unguarded_b_tail_load",
        "let b_value = if depth < k && column < n {",
        "let b_value = if column < n {"
    ),
    mutation!(
        "unguarded_c_tail_store",
        "out-of-bounds-c-store",
        "out_of_bounds_c_store",
        "context.store_epilogue(\n            &mut c,\n            row_base,\n            column,",
        "context.store_epilogue(\n            &mut c,\n            m,\n            column,"
    ),
    mutation!(
        "duplicate_lane_c_write",
        "lane-output-collision",
        "lane_output_collision",
        "context.store_epilogue(\n            &mut c,\n            row_base,\n            column,\n            m,",
        "context.store_epilogue(\n            &mut c,\n            row_base,\n            group_x * 16,\n            m,"
    ),
    mutation!(
        "overlapping_workgroup_c_tile",
        "workgroup-output-collision",
        "workgroup_output_collision",
        "context.store_epilogue(\n            &mut c,\n            row_base,\n            column,\n            m,",
        "context.store_epilogue(\n            &mut c,\n            row_base,\n            lane_column,\n            m,"
    ),
    mutation!(
        "duplicate_lds_write",
        "lds-write-collision",
        "lds_write_collision",
        "context.stage_value(a_slot, phase, depth, k, a_value);",
        "context.stage_value(lane_row, phase, depth, k, a_value);"
    ),
    mutation!(
        "lds_read_before_initialization",
        "missing-b-stage-initialization",
        "missing_b_stage_initialization",
        "context.stage_value(a_slot, phase, depth, k, a_value);\n            context.stage_value(b_slot, phase, depth, k, b_value);\n            component += 1;",
        "context.stage_value(a_slot, phase, depth, k, a_value);\n            component += 1;"
    ),
    mutation!(
        "missing_publish_barrier",
        "missing-publish",
        "missing_publish",
        "context.wait_stage(phase);\n        context.publish();\n        let swizzled0",
        "context.wait_stage(phase);\n        let swizzled0"
    ),
    mutation!(
        "divergent_barrier",
        "divergent-publish",
        "divergent_publish",
        "        context.publish();",
        "        if lane.is_multiple_of(2) {\n            context.publish();\n        }"
    ),
    mutation!(
        "missing_reuse_barrier",
        "missing-reuse",
        "missing_reuse",
        "accumulator3 = context.multiply_accumulate_value(lhs3, rhs3, accumulator3);\n        context.reuse();\n        phase += 1;",
        "accumulator3 = context.multiply_accumulate_value(lhs3, rhs3, accumulator3);\n        phase += 1;"
    ),
    mutation!(
        "expired_lds_epoch",
        "expired-lds-epoch",
        "expired_lds_epoch",
        "context.read_stage(16 * lane_row + swizzled0, phase);",
        "context.read_stage(16 * lane_row + swizzled0, phase.wrapping_sub(1));"
    ),
    mutation!(
        "staged_read_before_wait",
        "read-before-wait",
        "read_before_wait",
        "context.wait_stage(phase);\n        context.publish();\n        let swizzled0 = depth_base ^ (4 * (lane_row % 4));\n        let lhs0 = context.read_stage(16 * lane_row + swizzled0, phase);",
        "context.publish();\n        let swizzled0 = depth_base ^ (4 * (lane_row % 4));\n        let lhs0 = context.read_stage(16 * lane_row + swizzled0, phase);\n        context.wait_stage(phase);"
    ),
    mutation!(
        "accumulator_reset",
        "reset-accumulator",
        "reset_accumulator",
        "context.multiply_accumulate_value(lhs0, rhs0, accumulator0);",
        "context.multiply_accumulate_value(lhs0, rhs0, 0.0);"
    ),
    mutation!(
        "incorrect_k_tail_zero_fill",
        "incorrect-k-tail-zero-fill",
        "incorrect_k_tail_zero_fill",
        "context.load_a(a, row, depth, m, k, lda)\n            } else {\n                0\n            };",
        "context.load_a(a, row, depth, m, k, lda)\n            } else {\n                0x3f80\n            };"
    ),
    mutation!(
        "incorrect_alpha_beta_epilogue",
        "incorrect-alpha-beta-epilogue",
        "incorrect_alpha_beta_epilogue",
        "let value = alpha * accumulator0 + beta * initial;",
        "let value = alpha * accumulator0 + initial;"
    ),
];

fn occurrences(haystack: &str, needle: &str) -> usize {
    haystack.match_indices(needle).count()
}

#[test]
fn every_issue_mutation_is_one_reversible_baseline_edit() {
    assert_eq!(MUTATIONS.len(), 15);

    let mut manifest_tail = MANIFEST;
    for mutation in MUTATIONS {
        assert_ne!(mutation.before, mutation.after, "{} is inert", mutation.id);
        assert_eq!(
            occurrences(BASELINE, mutation.before),
            1,
            "{} baseline edit must select exactly one byte range",
            mutation.id
        );

        let expected = BASELINE.replacen(mutation.before, mutation.after, 1);
        assert_eq!(
            mutation.source, expected,
            "{} contains source changes beyond its named mutation",
            mutation.id
        );
        assert_eq!(
            occurrences(mutation.source, mutation.after),
            1,
            "{} reverse edit must select exactly one byte range",
            mutation.id
        );
        assert_eq!(
            mutation.source.replacen(mutation.after, mutation.before, 1),
            BASELINE,
            "{} does not reverse to the byte-identical baseline",
            mutation.id
        );
        assert_eq!(
            occurrences(mutation.source, "pub fn valid_proof_sensitive("),
            1,
            "{} changed the attributed kernel/root symbol",
            mutation.id
        );
        assert!(mutation.source.starts_with("#![forbid(unsafe_code)]"));
        assert!(!mutation.source.contains("unsafe {"));

        let manifest_entry = format!("name = \"{}\"", mutation.bin);
        let position = manifest_tail
            .find(&manifest_entry)
            .unwrap_or_else(|| panic!("{} is missing from the fixture manifest", mutation.id));
        manifest_tail = &manifest_tail[position + manifest_entry.len()..];
    }
}
