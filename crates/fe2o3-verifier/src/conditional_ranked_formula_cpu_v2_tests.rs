//! Commitment rejection tests only; no receipt or source authority is fabricated.
use super::{fixtures::*, *};
use crate::portable_reference_v1::{
    ReferenceEffectExpressionV1, ReferenceOperandV1, ReferenceValueV1,
    codec::with_decoded_native_cpu_input_v1,
};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

fn commitment(input: NativeCpuInputV1<'_>) -> DigestV1 {
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget.reserve_storage(31).unwrap();
    let identity = budget.work_ledger_identity_v1();
    let hash = with_encoded_native_cpu_input_v1(input, &mut budget, |bytes, hash, budget| {
        // The independently decoded object reproduces the same full commitment.
        with_decoded_native_cpu_input_v1(bytes, budget, |decoded, _| {
            assert_eq!(decoded.commitment_v1(), hash);
            DigestV1::from_untrusted_bytes(hash)
        })
        .unwrap()
    })
    .unwrap();
    assert_eq!(budget.storage(), 31);
    assert!(budget.work_ledger_identity_v1() == identity);
    hash
}

fn rejects(expected: DigestV1, input: NativeCpuInputV1<'_>) {
    let actual = commitment(input);
    assert!(matches!(
        require_commitment(expected, actual),
        Err(Error::Subject("conditional V2 CPU input substitution"))
    ));
    let six = std::array::from_fn(|i| digest(i as u8 + 1));
    assert_ne!(
        obligation_identity_v2(six, expected),
        obligation_identity_v2(six, actual)
    );
}

#[test]
fn v2_obligation_has_exact_seven_ordered_fields_and_no_v1_upgrade() {
    let six = std::array::from_fn(|i| digest(i as u8 + 1));
    let cpu = digest(7);
    let v2 = obligation_identity_v2(six, cpu);
    let mut independent = Sha256::new();
    independent.update(b"FE2O3/CONDITIONAL-MEMORY-TRANSITION/V2/LE/SHARED-IEEE/CPU-SOURCE\0");
    for tag in 1u8..=7 {
        independent.update([tag; 32]);
    }
    assert_eq!(
        v2,
        DigestV1::from_untrusted_bytes(independent.finalize().into())
    );
    assert_ne!(v2, obligation_identity(six));
    let mut old_domain_with_seventh = Sha256::new();
    old_domain_with_seventh.update(OBLIGATION_DOMAIN);
    for field in six.into_iter().chain([cpu]) {
        old_domain_with_seventh.update(field.as_bytes());
    }
    assert_ne!(
        v2,
        DigestV1::from_untrusted_bytes(old_domain_with_seventh.finalize().into())
    );
    for i in 0..6 {
        let mut changed = six;
        changed[i] = digest(99);
        assert_ne!(v2, obligation_identity_v2(changed, cpu));
        changed = six;
        changed.swap(i, (i + 1) % 6);
        assert_ne!(v2, obligation_identity_v2(changed, cpu));
    }
    assert_ne!(v2, obligation_identity_v2(six, digest(99)));
}

#[test]
fn v2_commitment_rejects_changed_cpu_operand_and_all_associations() {
    let mut fixture = Fixture::new();
    let expected = commitment(fixture.input());
    require_commitment(expected, expected).unwrap();
    for mutation in 0..4 {
        let mut input = fixture.input();
        match mutation {
            0 => input.association.semantic_mir_sha256[0] ^= 1,
            1 => input.association.semantic_root = 1,
            2 => input.association.registration_path = "component::other",
            3 => input.association.logical_kernel_name = "other",
            _ => unreachable!(),
        }
        rejects(expected, input);
    }
    // Change the CPU operand and both retained claims coherently, not only a cache.
    fixture.change_operand();
    rejects(expected, fixture.input());
}

#[test]
fn v2_commitment_rejects_every_full_identity_field_on_both_subjects() {
    let expected = commitment(Fixture::new().input());
    for reference in [false, true] {
        for field in 0..7 {
            let mut fixture = Fixture::new();
            let identity = if reference {
                &mut fixture.reference
            } else {
                &mut fixture.kernel
            };
            match field {
                0 => identity.def_path_hash[0] ^= 1,
                1 => identity.function_sha256[0] ^= 1,
                2 => identity.item_definition_sha256[0] ^= 1,
                3 => identity.monomorphization_sha256[0] ^= 1,
                4 => identity.generic_type_arguments_sha256[0] ^= 1,
                5 => identity.const_generic_arguments_sha256[0] ^= 1,
                6 => identity.rustc_mir_body_sha256[0] ^= 1,
                _ => unreachable!(),
            }
            rejects(expected, fixture.input());
        }
    }
}

#[test]
fn v2_does_not_accept_the_legacy_digest_omission_as_equal_cpu_content() {
    let mut fixture = Fixture::new();
    let expected = commitment(fixture.input());
    let legacy = fixture.ir.canonical_sha256_v1();
    fixture.ir.observable_output_effects[0].value =
        ReferenceValueV1::Use(ReferenceOperandV1::Constant(constant(0x3f80_0000)));
    assert_eq!(fixture.ir.canonical_sha256_v1(), legacy);
    // B1 transports value even where the legacy hash does not. This remains
    // inert content; B2 would additionally reject the incoherent replay claim.
    rejects(expected, fixture.input());
}

#[test]
fn v2_codec_path_rejects_stale_legacy_digest_and_distinct_borrowed_claims() {
    let mut fixture = Fixture::new();
    let original_digest = fixture.ir.canonical_sha256_v1();
    fixture.change_operand();
    let mut input = fixture.input();
    input.replay.effect_ir_sha256 = original_digest;
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    assert!(
        with_encoded_native_cpu_input_v1(input, &mut budget, |_, _, _| panic!(
            "stale digest exposed"
        ))
        .is_err()
    );
    assert_eq!(budget.storage(), 0);
    let mut other = fixture.ir.observable_output_effects.to_vec();
    other[0].rhs = ReferenceEffectExpressionV1::Constant(constant(0));
    let mut input = fixture.input();
    input.replay.observable_output_writes = &other;
    assert!(
        with_encoded_native_cpu_input_v1(input, &mut budget, |_, _, _| panic!(
            "foreign claims exposed"
        ))
        .is_err()
    );
    assert_eq!(budget.storage(), 0);
}
