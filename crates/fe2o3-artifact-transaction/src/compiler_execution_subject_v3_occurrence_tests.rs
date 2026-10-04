//! Additional inert subject binding tests; no protected occurrence authority.
use super::*;
use crate::compiler_module_handoff::conditional_v5::tests::fixture::outer_source_variant;

#[test]
fn conditional_subject_v3_same_payload_new_publication_occurrence_is_distinct() {
    for gfx950 in [false, true] {
        let mut f = Fixture::new();
        if gfx950 {
            f.handoff = outer_variant(true, 7);
        }
        let mut work = Work::new(1_000_000_000);
        let mut budget = Budget::new(&mut work, LIMIT);
        f.reserve(&mut budget);
        let ledger = budget.work_ledger_identity_v1();
        let first_receipt = f.publish(&mut budget).unwrap();
        let (first, paid) =
            Subject::from_publication(first_receipt, &f.handoff, &mut budget).unwrap();
        budget.reserve_storage(paid.0).unwrap();
        f.attempt = crate::begin_build_attempt(
            &f.path,
            &f.producer,
            crate::BuildInvocation::from_bytes([5; 32]),
            first_receipt.attempt().session(),
        )
        .unwrap();
        let second_receipt = f.publish(&mut budget).unwrap();
        let before = (budget.storage(), budget.work());
        let (second, paid) =
            Subject::from_publication(second_receipt, &f.handoff, &mut budget).unwrap();
        assert_eq!(budget.storage(), before.0);
        assert_eq!(budget.work(), before.1 + WORK);
        budget.reserve_storage(paid.0).unwrap();
        for (subject, receipt) in [(&first, first_receipt), (&second, second_receipt)] {
            assert_eq!(subject.attempt(), receipt.attempt());
            assert_eq!(subject.slot(), receipt.slot());
            assert_eq!(
                subject.transaction_identity(),
                receipt.transaction_identity()
            );
            inert(subject);
        }
        assert_eq!(
            first_receipt.handoff_identity(),
            second_receipt.handoff_identity()
        );
        assert_eq!(first.outer_handoff(), second.outer_handoff());
        // Nothing in the payload changed: invocation/closure and all seven bindings.
        assert_eq!(
            &first.canonical_bytes()[120..658],
            &second.canonical_bytes()[120..658]
        );
        assert_ne!(first.attempt(), second.attempt());
        assert_ne!(first.transaction_identity(), second.transaction_identity());
        assert_ne!(first.identity(), second.identity());
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!(budget.failed_storage(), None);
        drop(budget);
        assert_eq!(work.failed_work(), None);
    }
}

#[test]
fn conditional_subject_v3_resealed_source_only_donor_is_bound_on_both_profiles() {
    for gfx950 in [false, true] {
        let mut f = Fixture::new();
        if gfx950 {
            f.handoff = outer_variant(true, 7);
        }
        let donor = outer_source_variant(gfx950, 19);
        let original = f.handoff.capsule();
        let changed = donor.capsule();
        assert_ne!(
            original.source_packet_bytes()[0],
            changed.source_packet_bytes()[0]
        );
        assert_eq!(
            &original.source_packet_bytes()[1..],
            &changed.source_packet_bytes()[1..]
        );
        for (a, b) in [
            (original.history_bytes(), changed.history_bytes()),
            (original.catalog_bytes(), changed.catalog_bytes()),
            (original.descriptor_bytes(), changed.descriptor_bytes()),
            (
                original.semantic_target_layout_bytes(),
                changed.semantic_target_layout_bytes(),
            ),
            (
                original.final_module_commitment_bytes(),
                changed.final_module_commitment_bytes(),
            ),
            (
                f.handoff.module_handoff().canonical_bytes(),
                donor.module_handoff().canonical_bytes(),
            ),
        ] {
            assert_eq!(a, b);
        }
        assert_eq!(original.invocation(), changed.invocation());
        assert_eq!(
            original.rustc_identity_inventory(),
            changed.rustc_identity_inventory()
        );
        assert_eq!(
            original.rustc_preflight_plan(),
            changed.rustc_preflight_plan()
        );
        // Resealing updates only the carrier coordinate in this dependent record.
        let original_lowering = original.native_lowering().inputs();
        let mut donor_lowering = changed.native_lowering().inputs();
        assert_ne!(original_lowering.carrier, donor_lowering.carrier);
        donor_lowering.carrier = original_lowering.carrier;
        assert_eq!(original_lowering, donor_lowering);

        let mut work = Work::new(1_000_000_000);
        let mut budget = Budget::new(&mut work, LIMIT);
        f.reserve(&mut budget);
        budget
            .reserve_storage(handoff_floor(&donor).unwrap())
            .unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let receipt = f.publish(&mut budget).unwrap();
        let start_work = budget.work();
        let (expected, paid) = Subject::from_publication(receipt, &f.handoff, &mut budget).unwrap();
        budget.reserve_storage(paid.0).unwrap();
        let floor = budget.storage();
        assert!(matches!(
            Subject::from_publication(receipt, &donor, &mut budget),
            Err(Failure::HandoffIdentityMismatch)
        ));
        assert_eq!(budget.storage(), floor);
        let (alternative, paid) = Subject::from_replay_evidence(
            receipt.attempt(),
            receipt.slot(),
            receipt.transaction_identity(),
            &donor,
            &mut budget,
        )
        .unwrap();
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.work(), start_work + 3 * WORK);
        budget.reserve_storage(paid.0).unwrap();
        // Same occurrence and cached non-carrier inputs cannot hide the source change.
        assert_eq!(expected.attempt(), alternative.attempt());
        assert_eq!(expected.slot(), alternative.slot());
        assert_eq!(
            expected.transaction_identity(),
            alternative.transaction_identity()
        );
        for (a, b) in [
            (
                expected.rustc_identity_inventory(),
                alternative.rustc_identity_inventory(),
            ),
            (
                expected.rustc_preflight_plan(),
                alternative.rustc_preflight_plan(),
            ),
            (
                expected.final_compiler_module_commitment(),
                alternative.final_compiler_module_commitment(),
            ),
            (
                expected.compiler_module_handoff(),
                alternative.compiler_module_handoff(),
            ),
        ] {
            assert_eq!(a, b);
        }
        assert_ne!(expected.semantic_capsule(), alternative.semantic_capsule());
        assert_ne!(
            expected.compiler_module_pair_binding(),
            alternative.compiler_module_pair_binding()
        );
        assert_ne!(expected.outer_handoff(), alternative.outer_handoff());
        assert_ne!(expected.identity(), alternative.identity());
        inert(&expected);
        inert(&alternative);
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!(budget.failed_storage(), None);
        drop(budget);
        assert_eq!(work.failed_work(), None);
    }
}
