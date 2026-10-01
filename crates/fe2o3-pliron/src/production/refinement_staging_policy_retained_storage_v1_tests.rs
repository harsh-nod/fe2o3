use super::super::ProductionRefinementStagingPolicyV2;
use fe2o3_functional_proof::VerusToolchainIdentityV2;
use fe2o3_proof_contracts::DigestV1;
use std::mem::size_of;

fn digest(byte: u8) -> DigestV1 {
    DigestV1::from_untrusted_bytes([byte; 32])
}

fn toolchain() -> VerusToolchainIdentityV2 {
    VerusToolchainIdentityV2::new(digest(1), digest(2), digest(3), digest(4), digest(5)).unwrap()
}

fn policy() -> ProductionRefinementStagingPolicyV2 {
    // Original caller-selected, non-authoritative constructor. Duplicate
    // input identities are intentionally collapsed by its actual BTreeSet.
    ProductionRefinementStagingPolicyV2::new([digest(6), digest(7), digest(6)], toolchain())
        .unwrap()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Refusal {
    Arithmetic,
    Bytes,
    Items,
}

fn bounded(
    owner: &ProductionRefinementStagingPolicyV2,
    mut bytes: usize,
    mut items: usize,
    max_bytes: usize,
    max_items: usize,
) -> Result<(usize, usize), Refusal> {
    if bytes > max_bytes {
        return Err(Refusal::Bytes);
    }
    if items > max_items {
        return Err(Refusal::Items);
    }
    owner.visit_retained_logical_key_payload_v1(|count, width| {
        let added = count.checked_mul(width).ok_or(Refusal::Arithmetic)?;
        let next_bytes = bytes.checked_add(added).ok_or(Refusal::Arithmetic)?;
        let next_items = items.checked_add(1).ok_or(Refusal::Arithmetic)?;
        if next_bytes > max_bytes {
            return Err(Refusal::Bytes);
        }
        if next_items > max_items {
            return Err(Refusal::Items);
        }
        bytes = next_bytes;
        items = next_items;
        Ok(())
    })?;
    Ok((bytes, items))
}

#[test]
fn original_policy_counts_unique_logical_keys_not_input_count_or_nodes() {
    let owner = policy();
    let mut events = Vec::new();
    owner
        .visit_retained_logical_key_payload_v1(|count, width| {
            events.push((count, width));
            Ok::<_, ()>(())
        })
        .unwrap();
    assert_eq!(owner.signer_identities().len(), 2);
    assert_eq!(events, vec![(2, size_of::<DigestV1>())]);
}

#[test]
fn caller_charges_policy_header_and_logical_key_payload_once() {
    let owner = policy();
    let header = size_of::<ProductionRefinementStagingPolicyV2>();
    let expected = header + 2 * size_of::<DigestV1>();
    assert_eq!(bounded(&owner, header, 1, expected, 2), Ok((expected, 2)));
}

#[test]
fn logical_policy_exact_byte_and_item_limits_refuse_one_short() {
    let owner = policy();
    let header = size_of::<ProductionRefinementStagingPolicyV2>();
    let expected = header + 2 * size_of::<DigestV1>();
    assert_eq!(
        bounded(&owner, header, 1, expected - 1, 2),
        Err(Refusal::Bytes)
    );
    assert_eq!(bounded(&owner, header, 1, expected, 1), Err(Refusal::Items));
}

#[test]
fn logical_policy_preserves_the_first_and_only_callback_refusal() {
    let owner = policy();
    let mut calls = 0;
    let result = owner.visit_retained_logical_key_payload_v1(|_, _| {
        calls += 1;
        Err::<(), _>("original refusal")
    });
    assert_eq!(result, Err("original refusal"));
    assert_eq!(calls, 1);
}

#[test]
fn logical_policy_cumulative_arithmetic_overflow_is_not_saturated() {
    let owner = policy();
    assert_eq!(
        bounded(&owner, usize::MAX, 0, usize::MAX, usize::MAX),
        Err(Refusal::Arithmetic)
    );
    assert_eq!(
        bounded(&owner, 0, usize::MAX, usize::MAX, usize::MAX),
        Err(Refusal::Arithmetic)
    );
}

#[test]
fn logical_observation_preserves_signers_toolchain_and_original_refusals() {
    let owner = policy();
    let before = owner.clone();
    owner
        .visit_retained_logical_key_payload_v1(|_, _| Ok::<_, ()>(()))
        .unwrap();
    assert_eq!(owner, before);
    assert!(owner.accepts_signer(digest(6)));
    assert!(owner.accepts_signer(digest(7)));
    assert!(!owner.accepts_signer(digest(8)));
    assert_eq!(owner.toolchain(), toolchain());
    assert!(ProductionRefinementStagingPolicyV2::new([], toolchain()).is_err());
    assert!(ProductionRefinementStagingPolicyV2::new([DigestV1::ZERO], toolchain()).is_err());
}
