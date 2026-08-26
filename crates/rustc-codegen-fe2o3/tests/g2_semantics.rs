use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

#[path = "support/cargo_fe2o3.rs"]
mod cargo_fe2o3;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CollectionOutcome {
    CollectedThenLoweringGap {
        gap: &'static str,
    },
    CollectionGap {
        gap: &'static str,
        skipped_callee: &'static str,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SemanticCase {
    feature: &'static str,
    kernel: &'static str,
    collected_items: &'static [(&'static str, &'static str)],
    outcome: CollectionOutcome,
}

const CASES: &[SemanticCase] = &[
    SemanticCase {
        feature: "generic",
        kernel: "generic_kernel",
        collected_items: &[
            ("g2_semantic_kernels", "fe2o3_kernel_generic_kernel"),
            ("g2_semantic_kernels", "generic_identity"),
        ],
        outcome: CollectionOutcome::CollectedThenLoweringGap {
            gap: "G2-LOWERING-GENERIC-MONOMORPHIZATION",
        },
    },
    SemanticCase {
        feature: "const-generic",
        kernel: "const_generic_kernel",
        collected_items: &[
            ("g2_semantic_kernels", "fe2o3_kernel_const_generic_kernel"),
            ("g2_semantic_kernels", "const_bias"),
        ],
        outcome: CollectionOutcome::CollectedThenLoweringGap {
            gap: "G2-LOWERING-CONST-GENERIC-MONOMORPHIZATION",
        },
    },
    SemanticCase {
        feature: "aggregate",
        kernel: "aggregate_field_kernel",
        collected_items: &[
            ("g2_semantic_kernels", "fe2o3_kernel_aggregate_field_kernel"),
            ("g2_semantic_kernels", "sum_pair"),
        ],
        outcome: CollectionOutcome::CollectedThenLoweringGap {
            gap: "G2-LOWERING-AGGREGATE-FIELD-ACCESS",
        },
    },
    SemanticCase {
        feature: "integer-match",
        kernel: "integer_match_kernel",
        collected_items: &[
            ("g2_semantic_kernels", "fe2o3_kernel_integer_match_kernel"),
            ("g2_semantic_kernels", "classify_integer"),
        ],
        outcome: CollectionOutcome::CollectedThenLoweringGap {
            gap: "G2-LOWERING-INTEGER-MATCH",
        },
    },
    SemanticCase {
        feature: "loops",
        kernel: "loop_kernel",
        collected_items: &[
            ("g2_semantic_kernels", "fe2o3_kernel_loop_kernel"),
            ("g2_semantic_kernels", "repeat_bias"),
        ],
        outcome: CollectionOutcome::CollectedThenLoweringGap {
            gap: "G2-LOWERING-LOOPS",
        },
    },
    SemanticCase {
        feature: "cross-crate",
        kernel: "cross_crate_kernel",
        collected_items: &[("g2_semantic_kernels", "fe2o3_kernel_cross_crate_kernel")],
        outcome: CollectionOutcome::CollectionGap {
            gap: "G2-COLLECTION-CROSS-CRATE-MIR",
            skipped_callee: "g2_semantic_helpers::cross_crate_bias",
        },
    },
];

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("canonical workspace")
}

fn fixture_manifest(workspace: &Path) -> PathBuf {
    workspace.join("crates/rustc-codegen-fe2o3/tests/fixtures/g2-semantics/Cargo.toml")
}

fn run_frontend_check(workspace: &Path, case: SemanticCase) -> Output {
    Command::new(env!("CARGO"))
        .current_dir(workspace)
        .args([
            "check",
            "--locked",
            "--manifest-path",
            fixture_manifest(workspace)
                .to_str()
                .expect("UTF-8 fixture manifest"),
            "--no-default-features",
            "--features",
            case.feature,
        ])
        .output()
        .expect("check G2 semantic fixture")
}

fn run_backend_build(workspace: &Path, case: SemanticCase) -> Output {
    cargo_fe2o3::non_production_command(workspace)
        .current_dir(workspace)
        .env("FE2O3_VERBOSE", "1")
        .env("FE2O3_QUALIFICATION_ORACLE_V1", "kernel-ir-v1")
        .args([
            "build",
            "--manifest-path",
            fixture_manifest(workspace)
                .to_str()
                .expect("UTF-8 fixture manifest"),
            "--locked",
            "--no-default-features",
            "--features",
            case.feature,
        ])
        .output()
        .expect("build G2 semantic fixture with fe2o3 backend")
}

#[test]
fn semantic_case_table_is_complete_and_machine_readable() {
    assert_eq!(CASES.len(), 6);

    let expected_features = BTreeSet::from([
        "aggregate",
        "const-generic",
        "cross-crate",
        "generic",
        "integer-match",
        "loops",
    ]);
    let actual_features = CASES
        .iter()
        .map(|case| case.feature)
        .collect::<BTreeSet<_>>();
    assert_eq!(actual_features, expected_features);

    let mut gaps = BTreeSet::new();
    for case in CASES {
        assert!(!case.kernel.is_empty());
        assert!(!case.collected_items.is_empty());
        let gap = match case.outcome {
            CollectionOutcome::CollectedThenLoweringGap { gap } => {
                assert!(gap.starts_with("G2-LOWERING-"));
                gap
            }
            CollectionOutcome::CollectionGap {
                gap,
                skipped_callee,
            } => {
                assert!(gap.starts_with("G2-COLLECTION-"));
                assert!(!skipped_callee.is_empty());
                gap
            }
        };
        assert!(
            gaps.insert(gap),
            "duplicate or non-machine-readable gap identifier `{gap}`"
        );
    }
}

#[test]
fn semantic_fixtures_clear_the_standard_rust_frontend() {
    let workspace = workspace();

    for &case in CASES {
        let output = run_frontend_check(&workspace, case);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            output.status.success(),
            "G2 case `{}` failed before collection:\nstdout:\n{stdout}\nstderr:\n{stderr}",
            case.feature
        );
    }
}

#[test]
#[ignore = "gap:G2-ENV-ROCM-TOOLCHAIN_REQUIRED"]
fn semantic_fixtures_reach_the_expected_collection_frontier() {
    let workspace = workspace();

    for &case in CASES {
        let output = run_backend_build(&workspace, case);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        assert!(
            stderr.contains(&format!("[kernel] {}", case.kernel)),
            "G2 case `{}` did not collect kernel `{}`:\nstdout:\n{stdout}\nstderr:\n{stderr}",
            case.feature,
            case.kernel
        );
        for &(crate_name, path) in case.collected_items {
            let collected_item = format!("crate: {crate_name}\n      path: {path}");
            assert!(
                stderr.contains(&collected_item),
                "G2 case `{}` did not collect `{crate_name}` item `{path}`:\nstdout:\n{stdout}\nstderr:\n{stderr}",
                case.feature
            );
        }

        match case.outcome {
            CollectionOutcome::CollectedThenLoweringGap { gap } => assert!(
                !output.status.success(),
                "G2 case `{}` unexpectedly closed `{gap}`; promote its table outcome",
                case.feature
            ),
            CollectionOutcome::CollectionGap {
                gap,
                skipped_callee,
            } => {
                assert!(
                    stderr.contains(&format!(
                        "[collector] skipping no-MIR callee {skipped_callee}"
                    )),
                    "G2 case `{}` no longer exposes `{gap}`; promote its table outcome:\nstdout:\n{stdout}\nstderr:\n{stderr}",
                    case.feature
                );
                assert!(
                    !output.status.success(),
                    "G2 case `{}` unexpectedly closed `{gap}`; promote its table outcome",
                    case.feature
                );
            }
        }
    }
}
