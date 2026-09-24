//! Local signature/accounting tests, not evidence of protected proof execution.
use super::*;
use ed25519_dalek::{Signer, SigningKey};
use fe2o3_functional_proof::{
    FunctionalRefinementResultV2, FunctionalRefinementSubjectsV2, SafeReferenceKindV2,
    UnsignedFunctionalRefinementReceiptV2, VerusToolchainIdentityV2,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Owned,
    CanonicalKernelIrWorkBudgetV1 as Work,
};

fn digest(n: u8) -> DigestV1 {
    DigestV1::from_untrusted_bytes([n; 32])
}

struct Fixture {
    report: ProductionConditionalFormulaReportV1,
    policy: FunctionalRefinementImportPolicyV2,
    wire: Vec<u8>,
    prepared: Prepared,
}
fn fixture() -> Fixture {
    let signing = SigningKey::from_bytes(&[0x52; 32]);
    let toolchain =
        VerusToolchainIdentityV2::new(digest(10), digest(11), digest(12), digest(13), digest(14))
            .unwrap();
    let subjects = FunctionalRefinementSubjectsV2::new(
        SafeReferenceKindV2::Mir,
        digest(1),
        DigestV1::ZERO,
        digest(2),
        digest(3),
        digest(4),
    )
    .unwrap();
    let binding = FunctionalRefinementBindingV2::from_subjects(subjects, digest(15)).unwrap();
    let boundary = FunctionalRefinementBoundaryV2::SafeReferenceMirToLivePliron;
    let policy = FunctionalRefinementImportPolicyV2::new(
        signing.verifying_key().to_bytes(),
        toolchain,
        boundary,
    )
    .unwrap();
    let unsigned = UnsignedFunctionalRefinementReceiptV2::from_verified_execution_join(
        policy.signer_identity(),
        binding,
        toolchain,
        digest(16),
        FunctionalRefinementResultV2::Proved,
        boundary,
    )
    .unwrap();
    let signature = signing.sign(unsigned.signing_bytes()).to_bytes();
    let wire = unsigned.attach_signature(signature);
    let proof = FunctionalRefinementReceiptImporterV2::new(policy.clone(), 1)
        .unwrap()
        .import(FunctionalRefinementImportExpectationV2::new(binding), &wire)
        .unwrap();
    let source = CanonicalGeneratedVerusProofInputV3::new(
        b"use vstd::prelude::*;\nverus! { proof fn fixture() {} }\n".to_vec(),
    )
    .unwrap();
    let generated_source = DigestV1::from_untrusted_bytes(source.identity().as_bytes());
    Fixture {
        report: ProductionConditionalFormulaReportV1 {
            statement: digest(15),
            generated_source,
            binding,
            execution: digest(16),
            receipt: proof.receipt_identity().digest(),
        },
        policy,
        wire: wire.to_vec(),
        prepared: Prepared {
            source,
            generated_source,
            binding,
        },
    }
}
fn check(f: &Fixture) -> Result<(), Error> {
    let mut ledger = Owned::new(Work::new(100_000), usize::MAX);
    ledger.with_budget(|budget| reimport(f.report, &f.policy, &f.wire, &f.prepared, budget))
}

#[test]
fn retained_signature_reimports_after_move_without_execution_or_resigning() {
    let f = fixture();
    let before = (f.wire.clone(), f.report);
    let moved = Box::new(f);
    check(&moved).unwrap();
    check(&moved).unwrap();
    assert_eq!(
        (moved.wire.as_slice(), moved.report),
        (before.0.as_slice(), before.1)
    );
}

#[test]
fn retained_signature_policy_binding_and_execution_substitutions_fail_closed() {
    for mutation in 0..12 {
        let mut f = fixture();
        match mutation {
            0 => {
                let last = f.wire.len() - 1;
                f.wire[last] ^= 1;
            }
            1 => {
                f.wire.pop();
            }
            2 => f.report.generated_source = digest(91),
            3 => f.report.statement = digest(91),
            4 => f.report.execution = digest(91),
            5 => f.report.receipt = digest(91),
            6 => {
                f.report.binding = FunctionalRefinementBindingV2::from_subjects(
                    f.report.binding.subjects(),
                    digest(91),
                )
                .unwrap()
            }
            7 => {
                f.policy = FunctionalRefinementImportPolicyV2::new(
                    SigningKey::from_bytes(&[0x53; 32])
                        .verifying_key()
                        .to_bytes(),
                    f.policy.toolchain(),
                    f.policy.boundary(),
                )
                .unwrap()
            }
            8 => {
                f.policy = FunctionalRefinementImportPolicyV2::new(
                    SigningKey::from_bytes(&[0x52; 32])
                        .verifying_key()
                        .to_bytes(),
                    f.policy.toolchain(),
                    FunctionalRefinementBoundaryV2::SafeReferenceMirToKernelMir,
                )
                .unwrap()
            }
            9 => {
                f.policy = FunctionalRefinementImportPolicyV2::new(
                    SigningKey::from_bytes(&[0x52; 32])
                        .verifying_key()
                        .to_bytes(),
                    VerusToolchainIdentityV2::new(
                        digest(90),
                        digest(11),
                        digest(12),
                        digest(13),
                        digest(14),
                    )
                    .unwrap(),
                    f.policy.boundary(),
                )
                .unwrap()
            }
            10 => {
                let subjects = FunctionalRefinementSubjectsV2::new(
                    SafeReferenceKindV2::Mir,
                    digest(91),
                    DigestV1::ZERO,
                    digest(2),
                    digest(3),
                    digest(4),
                )
                .unwrap();
                f.prepared.binding =
                    FunctionalRefinementBindingV2::from_subjects(subjects, digest(15)).unwrap();
            }
            11 => f.prepared.generated_source = digest(91),
            _ => unreachable!(),
        }
        assert!(check(&f).is_err(), "mutation {mutation}");
    }
}

#[test]
fn retained_reimport_denial_preserves_original_cumulative_work() {
    let f = fixture();
    let bytes = f.wire.len();
    let mut ledger = Owned::new(Work::new(bytes), 0);
    ledger
        .with_budget(|budget| reimport(f.report, &f.policy, &f.wire, &f.prepared, budget))
        .unwrap();
    let denied =
        ledger.with_budget(|budget| reimport(f.report, &f.policy, &f.wire, &f.prepared, budget));
    assert!(matches!(denied, Err(Error::Resource(Resource::Work(_)))));
    assert_eq!(
        (ledger.work(), ledger.failed_work()),
        (bytes, Some(bytes * 2))
    );
}

#[test]
fn retention_reserves_before_producer_and_keeps_charge_after_move() {
    let mut ledger = Owned::new(Work::new(100), 100);
    ledger
        .with_budget(|budget| budget.reserve_storage(7))
        .unwrap();
    let proof = ledger
        .with_budget(|budget| {
            retain_reservation(budget, 20, |budget| {
                assert_eq!(budget.storage(), 27);
                budget.charge_work(11)?;
                Ok(Box::new(42))
            })
        })
        .unwrap();
    assert_eq!((ledger.storage(), ledger.work()), (27, 12));
    drop(proof);
    ledger
        .with_budget(|budget| budget.release_storage(20))
        .unwrap();
    assert_eq!((ledger.storage(), ledger.work()), (7, 12));
}

#[test]
fn retention_denies_storage_before_producer_and_remembers_denial() {
    let mut ledger = Owned::new(Work::new(100), 19);
    let result = ledger.with_budget(|budget| {
        retain_reservation::<()>(budget, 20, |_| panic!("unadmitted producer"))
    });
    assert!(matches!(result, Err(Error::Resource(Resource::Storage(_)))));
    assert_eq!(
        (ledger.storage(), ledger.failed_storage(), ledger.work()),
        (0, Some(20), 1)
    );
}

#[test]
fn retention_refusal_and_unwind_release_only_their_reservation() {
    let mut ledger = Owned::new(Work::new(100), 100);
    ledger
        .with_budget(|budget| budget.reserve_storage(7))
        .unwrap();
    let result = ledger.with_budget(|budget| {
        retain_reservation::<()>(budget, 20, |budget| {
            budget.charge_work(5)?;
            budget.reserve_storage(13)?;
            Err(Error::Subject("denied"))
        })
    });
    assert!(matches!(result, Err(Error::Subject("denied"))));
    assert_eq!((ledger.storage(), ledger.work()), (20, 6));
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        ledger.with_budget(|budget| retain_reservation::<()>(budget, 20, |_| panic!("unwind")))
    }));
    assert!(panic.is_err());
    assert_eq!((ledger.storage(), ledger.work()), (20, 7));
}

#[test]
fn retention_detects_replaced_active_budget_without_refunding_original() {
    let mut ledger = Owned::new(Work::new(100), 100);
    let replacement = Box::leak(Box::new(Work::new(100)));
    let result = ledger.with_budget(|budget| {
        retain_reservation(budget, 20, |budget| {
            *budget = Budget::new(replacement, 100);
            budget.reserve_storage(20)?;
            Ok(())
        })
    });
    assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
    assert_eq!((ledger.storage(), ledger.work()), (20, 1));
}
