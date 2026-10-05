//! These fixtures test accounting/content joins only. They install no backend,
//! current-record signer, checked device or executable native authority.
use super::*;
use std::cell::Cell;

fn subject() -> NativeSubjectV53 {
    NativeSubjectV53 {
        versions: VERSIONS,
        capsule: [1; 32],
        descriptor: [2; 32],
        typed_receipt: [3; 32],
        source: [[4; 32], [5; 32]],
        graphs: [([6; 32], 91); 4],
        compiler_subject: [7; 32],
        compiler_carriage: [8; 32],
        current_record: [9; 32],
        artifact: ([10; 32], 123),
        ordinal: 1,
        invocation: Gfx942RuntimeInvocationBindingV1::ConditionalMixedV26 {
            contract_identity: [11; 32],
            premise_identity: [12; 32],
        },
        dispatch_contract: [13; 32],
        prepared_content: [14; 32],
        device_unique_id: 15,
    }
}
fn bytes(_: &mut Budget<'_>) -> Result<WorkerV3ProtectedSemanticMachineRefinementEvidenceV1> {
    WorkerV3ProtectedSemanticMachineRefinementEvidenceV1::new(
        Box::new([21u8; 7]),
        Box::new([22u8; 11]),
        [23; 32],
        [24; 32],
    )
    .map_err(codec_error)
}
fn resource(error: &AdmissionError) -> Resource {
    *std::error::Error::source(error)
        .unwrap()
        .downcast_ref::<Resource>()
        .unwrap()
}
fn run(expected: NativeSubjectV53, budget: &mut Budget<'_>) -> Result<()> {
    codec_on_budget(budget, 0, |budget| {
        let reserved = ReservedNativeEvidenceV53::construct(subject(), 7, 11, budget, bytes)?;
        let (evidence, _) = reserved.into_checked(expected, budget)?;
        assert_eq!(evidence.machine_effect_evidence_bytes(), &[21; 7]);
        assert_eq!(evidence.refinement_proof_bytes(), &[22; 11]);
        Ok(())
    })
}

#[test]
fn mixed_v53_native_evidence_exact_subject_and_every_version_are_required() {
    let mut mutants = Vec::new();
    for axis in 0..5 {
        let mut s = subject();
        s.versions[axis] -= 1;
        mutants.push(s);
    }
    for axis in 0..2 {
        let mut s = subject();
        s.source[axis][0] ^= 1;
        mutants.push(s);
    }
    for axis in 0..4 {
        let mut s = subject();
        s.graphs[axis].0[0] ^= 1;
        mutants.push(s);
        let mut s = subject();
        s.graphs[axis].1 += 1;
        mutants.push(s);
    }
    for axis in 0..15 {
        let mut s = subject();
        match axis {
            0 => s.capsule[0] ^= 1,
            1 => s.descriptor[0] ^= 1,
            2 => s.typed_receipt[0] ^= 1,
            3 => s.compiler_subject[0] ^= 1,
            4 => s.compiler_carriage[0] ^= 1,
            5 => s.current_record[0] ^= 1,
            6 => s.artifact.0[0] ^= 1,
            7 => s.artifact.1 += 1,
            8 => s.ordinal += 1,
            9 => s.dispatch_contract[0] ^= 1,
            10 => s.prepared_content[0] ^= 1,
            11 => s.device_unique_id += 1,
            12 => s.invocation = Gfx942RuntimeInvocationBindingV1::OrdinaryV1,
            13 => {
                s.invocation = Gfx942RuntimeInvocationBindingV1::ConditionalMixedV26 {
                    contract_identity: [0; 32],
                    premise_identity: [12; 32],
                }
            }
            14 => {
                s.invocation = Gfx942RuntimeInvocationBindingV1::ConditionalMixedV26 {
                    contract_identity: [11; 32],
                    premise_identity: [0; 32],
                }
            }
            _ => unreachable!(),
        }
        mutants.push(s);
    }
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    budget.reserve_storage(41).unwrap();
    run(subject(), &mut budget).unwrap();
    for mutant in mutants {
        assert!(matches!(
            run(mutant, &mut budget),
            Err(AdmissionError::MixedV53(
                "V53 native evidence exact subject"
            ))
        ));
        assert_eq!(budget.storage(), 41);
    }
}

#[test]
fn mixed_v53_native_evidence_exact_work_storage_and_sticky_one_short() {
    let measure = |work_limit, storage_limit| {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(41).unwrap();
        let result = run(subject(), &mut budget);
        assert_eq!(budget.storage(), 41);
        (result, budget.work(), budget.peak_storage())
    };
    let (result, work, storage) = measure(1_000_000, 1_000_000);
    result.unwrap();
    // Outer codec, construction codec, all payload hashing and exact subject comparison.
    assert_eq!(work, 2 + 7 + 11 + size_of::<NativeSubjectV53>());
    measure(work, storage).0.unwrap();
    for (w, s) in [(work - 1, storage), (work, storage - 1)] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(w);
        let mut budget = Budget::new(&mut work, s);
        budget.reserve_storage(41).unwrap();
        let first = run(subject(), &mut budget).unwrap_err();
        assert!(matches!(
            resource(&first),
            Resource::Work(_) | Resource::Storage(_)
        ));
        let consumed = budget.work();
        let second = run(subject(), &mut budget).unwrap_err();
        assert_eq!(resource(&first), resource(&second));
        assert_eq!((budget.work(), budget.storage()), (consumed, 41));
    }
}

#[test]
fn mixed_v53_native_evidence_declared_extents_precede_callback_and_exact_boxes_follow_it() {
    let calls = Cell::new(0);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    budget.reserve_storage(41).unwrap();
    for (effect, proof) in [(0, 11), (7, 0), (usize::MAX, 11), (7, usize::MAX)] {
        let result =
            ReservedNativeEvidenceV53::construct(subject(), effect, proof, &mut budget, |budget| {
                calls.set(calls.get() + 1);
                bytes(budget)
            });
        assert!(matches!(
            result,
            Err(AdmissionError::MixedV53(
                "V53 native evidence declared extent"
            ))
        ));
    }
    assert_eq!(calls.get(), 0);
    for (effect, proof) in [(6, 11), (8, 11), (7, 10), (7, 12)] {
        let result =
            ReservedNativeEvidenceV53::construct(subject(), effect, proof, &mut budget, bytes);
        assert!(matches!(
            result,
            Err(AdmissionError::MixedV53(
                "V53 native evidence actual boxed extent"
            ))
        ));
        assert_eq!(budget.storage(), 41);
    }
    let mut invalid = subject();
    invalid.versions[0] = 1;
    assert!(matches!(
        ReservedNativeEvidenceV53::construct(invalid, 7, 11, &mut budget, bytes),
        Err(AdmissionError::MixedV53("V53 native evidence versions"))
    ));
}

#[test]
fn mixed_v53_native_evidence_foreign_ledger_refund_and_callback_failures_are_closed() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    budget.reserve_storage(41).unwrap();
    let mut other_work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    let mut other = Budget::new(&mut other_work, 1_000_000);
    for foreign in [false, true] {
        codec_on_budget(&mut budget, 0, |budget| {
            let reserved = ReservedNativeEvidenceV53::construct(subject(), 7, 11, budget, bytes)?;
            let result = if foreign {
                other.reserve_storage(budget.storage())?;
                reserved.into_checked(subject(), &mut other)
            } else {
                budget.release_storage(1)?;
                reserved.into_checked(subject(), budget)
            };
            assert!(matches!(
                result,
                Err(AdmissionError::MixedV53(
                    "V53 native evidence ledger or retained floor"
                ))
            ));
            Ok(())
        })
        .unwrap();
        assert_eq!(budget.storage(), 41);
    }
    let result = ReservedNativeEvidenceV53::construct(subject(), 7, 11, &mut budget, |_| {
        Err(binding("fixture backend refusal"))
    });
    assert!(matches!(
        result,
        Err(AdmissionError::MixedV53("fixture backend refusal"))
    ));
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = ReservedNativeEvidenceV53::construct(subject(), 7, 11, &mut budget, |_| {
            panic!("fixture backend unwind")
        });
    }));
    assert!(panic.is_err());
    assert_eq!(budget.storage(), 41);
    assert!(budget.failed_work().is_none() && budget.failed_storage().is_none());
}

#[test]
fn mixed_v53_execution_evidence_lease_refunds_only_owned_storage_after_drop_and_unwind() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(99);
    let mut budget = Budget::new(&mut work, 999);
    budget.reserve_storage(41).unwrap();
    budget.charge_work(7).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    for unwind in [false, true] {
        budget.reserve_storage(53).unwrap();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _lease = EvidenceStorageLease {
                budget: &mut budget,
                retained: 53,
            };
            if unwind {
                panic!("one-shot owner unwind");
            }
        }));
        assert_eq!(result.is_err(), unwind);
        assert_eq!(
            (budget.storage(), budget.work(), budget.peak_storage()),
            (41, 7, 94)
        );
        assert!(budget.work_ledger_identity_v1() == ledger);
    }
    let denied = budget.charge_work(100).unwrap_err();
    budget.reserve_storage(53).unwrap();
    drop(EvidenceStorageLease {
        budget: &mut budget,
        retained: 53,
    });
    assert_eq!(budget.check_prior_denials_v1().unwrap_err(), denied);
    assert_eq!((budget.storage(), budget.work()), (41, 7));
}

#[test]
fn mixed_v53_native_execution_does_not_coerce_gfx950_or_unqualified_targets() {
    require_execution_target(AmdTargetId::parse("gfx942:xnack-").unwrap()).unwrap();
    for target in ["gfx942", "gfx942:xnack+", "gfx950:xnack-"] {
        assert!(matches!(
            require_execution_target(AmdTargetId::parse(target).unwrap()),
            Err(AdmissionError::UnsupportedTarget)
        ));
    }
}
