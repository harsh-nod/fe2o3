//! Inert codec fixtures only. No signing, source admission or proof execution.
use super::*;
use fe2o3_compiler_lineage::{
    NativeConditionalPolicyRootInputV1 as PolicyRoot,
    NativeConditionalPolicyRosterInputV1 as PolicyInput,
    encode_native_conditional_policy_roster_v1,
};
use fe2o3_functional_proof::FUNCTIONAL_REFINEMENT_RECEIPT_WIRE_BYTES_V2 as SIGNATURE_BYTES;
use fe2o3_lower_mir_kernel::{
    ProductionSourceLaunchInputV1 as Geometry, ProductionSourceLaunchRootInputV1 as Launch,
};
use fe2o3_verifier::{
    InertFunctionalRefinementReceiptSignatureV2 as Signature,
    NativeCompilerStagingCommitmentV1 as Staging, NativeConditionalSourcePacketInputV2 as Packet,
    NativeConditionalSourceRootV2 as Root, encode_native_conditional_source_packet_v2,
};
use std::{ffi::OsString, os::unix::fs::symlink};

// RFC 8032 public key, not a private key or an imported proof.
const PUBLIC_KEY: [u8; 32] = [
    0xd7, 0x5a, 0x98, 0x01, 0x82, 0xb1, 0x0a, 0xb7, 0xd5, 0x4b, 0xfe, 0xd3, 0xc9, 0x64, 0x07, 0x3a,
    0x0e, 0xe1, 0x72, 0xf3, 0xda, 0xa6, 0x23, 0x25, 0xaf, 0x02, 0x1a, 0x68, 0xf7, 0x07, 0x51, 0x1a,
];

fn account() -> Owned {
    Owned::new(Work::new(ACCOUNT_WORK), STORAGE)
}

fn private_dir() -> tempfile::TempDir {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    dir
}

#[derive(Clone, Copy)]
struct Shape {
    count: usize,
    reverse_order: bool,
    source_root: u32,
    policy_root: u32,
    policy_binding: [u8; 32],
    launch_rank: u8,
    rank: u8,
    block: Option<[u32; 3]>,
    grid: [u32; 3],
}

const SINGLE: Shape = Shape {
    count: 1,
    reverse_order: false,
    source_root: 3,
    policy_root: 3,
    policy_binding: [7; 32],
    launch_rank: 1,
    rank: 1,
    block: Some([64, 1, 1]),
    grid: [4096, 1, 1],
};

fn fixture(shape: Shape) -> (Vec<u8>, Vec<u8>) {
    account().with_budget(|budget| {
        let staging = [Staging {
            receipt: [1; 32],
            effect: [2; 32],
            signer: [3; 32],
            execution: [4; 32],
            toolchain: [[5; 32]; 5],
        }];
        let receipts = [Signature::from_untrusted_parts(
            [11; SIGNATURE_BYTES],
            [12; 32],
        )];
        let formula = Signature::from_untrusted_parts([21; SIGNATURE_BYTES], [22; 32]);
        let roots = (0..shape.count)
            .map(|index| Root {
                semantic_root: shape.source_root + index as u32,
                launch_rank: shape.launch_rank,
                launch: Launch::new(
                    "unverified_fixture",
                    [7; 32],
                    Geometry::new(shape.rank, shape.block, shape.grid),
                ),
                induction_bytes: b"opaque induction",
                recipe_bytes: b"opaque recipe",
                source_rows_bytes: b"opaque rows",
                ranked_ir: "opaque ranked module",
                cpu_input_bytes: b"opaque CPU replay",
                staging_commitments: &staging,
                effect_receipts: &receipts,
                formula_receipt: &formula,
            })
            .collect::<Vec<_>>();
        let mut order = (0..shape.count as u32).collect::<Vec<_>>();
        if shape.reverse_order {
            order.reverse();
        }
        let (source, charge) = encode_native_conditional_source_packet_v2(
            Packet {
                semantic_mir: b"unverified semantic MIR",
                native_module: b"unverified N module",
                canonical_kernel_order: &order,
                roots: &roots,
            },
            budget,
        )
        .unwrap();
        budget.reserve_storage(charge.retained_storage()).unwrap();
        let signers = [[31; 32], [32; 32]];
        let policies = (0..shape.count)
            .map(|index| PolicyRoot {
                semantic_root: shape.policy_root + index as u32,
                kernel_binding: shape.policy_binding,
                effect_signers: &signers,
                effect_toolchain: [[41; 32]; 5],
                formula_verifying_key: PUBLIC_KEY,
                formula_toolchain: [[42; 32]; 5],
                formula_boundary: 2,
            })
            .collect::<Vec<_>>();
        let roster = encode_native_conditional_policy_roster_v1(
            PolicyInput {
                source_packet: &source,
                roots: &policies,
            },
            STORAGE,
            |_| Ok::<_, std::convert::Infallible>(()),
        )
        .unwrap();
        (source, roster)
    })
}

fn prepay(source: &[u8], roster: &[u8], budget: &mut Budget<'_>) {
    budget
        .reserve_storage(source.len() + roster.len() + 2 * std::mem::size_of::<Vec<u8>>())
        .unwrap();
}

#[test]
fn candidate_cli_requires_two_distinct_canonical_pins_and_no_extra_arguments() {
    use super::super::super::{Command, parse};
    let args = |parts: &[&str]| parts.iter().map(OsString::from).collect::<Vec<_>>();
    let source_pin = "ab".repeat(32);
    let roster_pin = "cd".repeat(32);
    assert!(matches!(
        parse(&args(&[
            "export-native-policy-candidate",
            "source",
            &source_pin,
            "roster",
            &roster_pin,
            "output",
        ]))
        .unwrap(),
        Command::ExportNativePolicy(_, [0xab, ..], _, [0xcd, ..], _)
    ));
    for parts in [
        vec![
            "export-native-policy-candidate",
            "source",
            "roster",
            "output",
        ],
        vec![
            "export-native-policy-candidate",
            "source",
            &source_pin,
            "roster",
            "output",
        ],
        vec![
            "export-native-policy-candidate",
            "source",
            "AB",
            "roster",
            &roster_pin,
            "output",
        ],
        vec![
            "export-native-policy-candidate",
            "source",
            &source_pin,
            "roster",
            "bad",
            "output",
        ],
        vec![
            "export-native-policy-candidate",
            "source",
            &source_pin,
            "roster",
            &roster_pin,
            "output",
            "--install",
        ],
    ] {
        assert!(parse(&args(&parts)).is_err());
    }
}

#[test]
fn candidate_preserves_canonical_policy_bytes_and_reports_no_authority() {
    let (source, roster) = fixture(SINGLE);
    account().with_budget(|budget| {
        prepay(&source, &roster, budget);
        let ledger = budget.work_ledger_identity_v1();
        let storage = budget.storage_account_identity_v1();
        let exported = prepare(&source, &roster, budget).unwrap();
        let (expected, charge) = encode(&roster, budget).unwrap();
        budget.reserve_storage(charge).unwrap();
        assert_eq!(exported.policy, expected);
        for field in [
            "semantic_root=3",
            "effect_signer_count=2",
            "formula_boundary=2",
            "grants_authority=false",
            "source_target_verified=false",
            "fill_semantics_verified=false",
            "policy_independently_approved=false",
        ] {
            assert!(exported.report.lines().any(|line| line == field), "{field}");
        }
        for expected in [
            format!("source_sha256={}", hex(Sha256::digest(&source).into())),
            format!("roster_sha256={}", hex(Sha256::digest(&roster).into())),
            format!(
                "candidate_sha256={}",
                hex(Sha256::digest(&exported.policy).into())
            ),
            format!("effect_signer_0={}", hex([31; 32])),
            format!("effect_signer_1={}", hex([32; 32])),
            format!("formula_verifying_key={}", hex(PUBLIC_KEY)),
        ] {
            assert!(exported.report.lines().any(|line| line == expected));
        }
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!(budget.storage_account_identity_v1(), storage);
    });
}

#[test]
fn candidate_rejects_source_root_binding_roster_order_and_launch_shape_substitution() {
    for shape in [
        Shape {
            policy_root: 4,
            ..SINGLE
        },
        Shape {
            policy_binding: [8; 32],
            ..SINGLE
        },
        Shape { count: 2, ..SINGLE },
        Shape {
            count: 2,
            reverse_order: true,
            ..SINGLE
        },
        Shape {
            launch_rank: 2,
            ..SINGLE
        },
        Shape { rank: 2, ..SINGLE },
        Shape {
            block: None,
            ..SINGLE
        },
        Shape {
            block: Some([32, 1, 1]),
            ..SINGLE
        },
        Shape {
            block: Some([64, 2, 1]),
            ..SINGLE
        },
        Shape {
            grid: [0, 1, 1],
            ..SINGLE
        },
        Shape {
            grid: [64, 2, 1],
            ..SINGLE
        },
    ] {
        let (source, roster) = fixture(shape);
        account().with_budget(|budget| {
            prepay(&source, &roster, budget);
            assert!(prepare(&source, &roster, budget).is_err());
        });
    }
    let (source, roster) = fixture(SINGLE);
    let (other, _) = fixture(Shape {
        grid: [2048, 1, 1],
        ..SINGLE
    });
    account().with_budget(|budget| {
        prepay(&other, &roster, budget);
        assert!(prepare(&other, &roster, budget).is_err());
    });
    for (source, roster) in [
        (roster.clone(), source.clone()),
        (source, b"legacy policy".to_vec()),
    ] {
        account().with_budget(|budget| {
            prepay(&source, &roster, budget);
            assert!(prepare(&source, &roster, budget).is_err());
        });
    }
}

#[test]
fn candidate_exact_resource_limits_and_one_short_refusals_preserve_original_account() {
    let (source, roster) = fixture(SINGLE);
    let run = |work, storage| {
        Owned::new(Work::new(work), storage).with_budget(|budget| {
            prepay(&source, &roster, budget);
            let ledger = budget.work_ledger_identity_v1();
            let storage = budget.storage_account_identity_v1();
            let result = prepare(&source, &roster, budget);
            assert!(budget.work_ledger_identity_v1() == ledger);
            assert_eq!(budget.storage_account_identity_v1(), storage);
            (result.is_ok(), budget.work(), budget.peak_storage())
        })
    };
    let baseline = run(ACCOUNT_WORK, STORAGE);
    assert!(baseline.0);
    assert_eq!(run(baseline.1, baseline.2), baseline);
    assert!(!run(baseline.1 - 1, baseline.2).0);
    assert!(!run(baseline.1, baseline.2 - 1).0);
    account().with_budget(|budget| {
        assert!(prepare(&source, &roster, budget).is_err());
    });
}

#[test]
fn candidate_export_requires_exact_pins_and_never_clobbers_inputs_or_existing_outputs() {
    let dir = private_dir();
    let source_path = dir.path().join("source");
    let roster_path = dir.path().join("roster");
    let output = dir.path().join("candidate");
    let (source, roster) = fixture(SINGLE);
    std::fs::write(&source_path, &source).unwrap();
    std::fs::write(&roster_path, &roster).unwrap();
    let source_pin = Sha256::digest(&source).into();
    let roster_pin = Sha256::digest(&roster).into();
    for (a, b) in [
        ([0; 32], roster_pin),
        ([9; 32], roster_pin),
        (source_pin, [9; 32]),
    ] {
        account().with_budget(|budget| {
            assert!(export_using(&source_path, a, &roster_path, b, &output, budget).is_err());
        });
        assert!(!output.exists());
    }
    for path in [&source_path, &roster_path] {
        account().with_budget(|budget| {
            assert!(
                export_using(
                    &source_path,
                    source_pin,
                    &roster_path,
                    roster_pin,
                    path,
                    budget
                )
                .is_err()
            );
        });
    }
    assert_eq!(std::fs::read(&source_path).unwrap(), source);
    assert_eq!(std::fs::read(&roster_path).unwrap(), roster);
    account().with_budget(|budget| {
        let report = export_using(
            &source_path,
            source_pin,
            &roster_path,
            roster_pin,
            &output,
            budget,
        )
        .unwrap();
        assert!(report.contains("policy_independently_approved=false"));
    });
    let original = std::fs::read(&output).unwrap();
    account().with_budget(|budget| {
        assert!(
            export_using(
                &source_path,
                source_pin,
                &roster_path,
                roster_pin,
                &output,
                budget
            )
            .is_err()
        );
    });
    assert_eq!(std::fs::read(&output).unwrap(), original);
    let alias = dir.path().join("alias");
    symlink(&source_path, &alias).unwrap();
    account().with_budget(|budget| {
        assert!(
            export_using(
                &source_path,
                source_pin,
                &roster_path,
                roster_pin,
                &alias,
                budget
            )
            .is_err()
        );
    });
    assert_eq!(std::fs::read(&source_path).unwrap(), source);
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 4);
}

#[test]
fn candidate_input_refusal_precedes_any_output_creation() {
    let dir = private_dir();
    let output = dir.path().join("candidate");
    for (work, storage) in [(0, STORAGE), (ACCOUNT_WORK, 0)] {
        Owned::new(Work::new(work), storage).with_budget(|budget| {
            assert!(
                export_using(
                    Path::new("/never-open-source"),
                    [1; 32],
                    Path::new("/never-open-roster"),
                    [2; 32],
                    &output,
                    budget
                )
                .is_err()
            );
        });
    }
    assert!(!output.exists());
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
}
