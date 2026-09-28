//! Public policy values only: no proof owner or accepted key from packet bytes.
use super::*;
use fe2o3_compiler_lineage::{
    MAX_NATIVE_CONDITIONAL_STORAGE_V1, read_native_conditional_policy_roster_v1,
};
use fe2o3_functional_proof::{
    FunctionalRefinementBoundaryV2 as Boundary, FunctionalRefinementImportPolicyV2 as Formula,
};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use fe2o3_pliron::ProductionRefinementStagingPolicyV2 as Effects;
use fe2o3_proof_contracts::DigestV1;
use sha2::{Digest, Sha256};

const LIMIT: usize = MAX_NATIVE_CONDITIONAL_STORAGE_V1;
const WORK: usize = 128 * 1024 * 1024;
const FLOOR: usize = 73;
const PACKET: &[u8] = b"exact inert source bytes, no policy or accepted-key fields";
const ROOTS: [(u32, [u8; 32]); 2] = [(9, [7; 32]), (2, [8; 32])];

fn digest(value: u8) -> DigestV1 {
    DigestV1::from_untrusted_bytes([value; 32])
}

fn tools(base: u8) -> VerusToolchainIdentityV2 {
    VerusToolchainIdentityV2::new(
        digest(base),
        digest(base + 1),
        digest(base + 2),
        digest(base + 3),
        digest(base + 4),
    )
    .unwrap()
}

struct Policies {
    effects: [Effects; 2],
    formulas: [Formula; 2],
}
impl Policies {
    fn new() -> Self {
        // Compressed Ed25519 base point; public policy data, never a signed proof.
        let mut key = [0x66; 32];
        key[0] = 0x58;
        let mut second_key = key;
        second_key[31] ^= 0x80;
        Self {
            effects: [
                Effects::new([digest(3), digest(1), digest(3), digest(2)], tools(10)).unwrap(),
                Effects::new([digest(6)], tools(20)).unwrap(),
            ],
            formulas: [
                Formula::new(key, tools(30), Boundary::SafeReferenceMirToLivePliron).unwrap(),
                Formula::new(
                    second_key,
                    tools(40),
                    Boundary::SafeReferenceSourceToKernelMir,
                )
                .unwrap(),
            ],
        }
    }

    fn borrowed(&self) -> [NativeConditionalRootPolicyV2<'_>; 2] {
        std::array::from_fn(|index| NativeConditionalRootPolicyV2 {
            semantic_root: ROOTS[index].0,
            effects: &self.effects[index],
            formula: &self.formulas[index],
        })
    }

    fn capture(&self, packet: &[u8], budget: &mut Budget<'_>) -> Result<Vec<u8>, Error> {
        capture(packet, &self.borrowed(), ROOTS.into_iter(), budget)
    }
}

#[test]
fn policy_capture_preserves_complete_actual_policies_and_source_order() {
    let policies = Policies::new();
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let account = budget.work_ledger_identity_v1();
    let bytes = policies.capture(PACKET, &mut budget).unwrap();
    let reserved = budget.storage();
    assert!(reserved > FLOOR + retained_storage(&bytes).unwrap());
    assert_eq!(budget.peak_storage(), reserved);
    assert!(budget.work_ledger_identity_v1() == account);
    let roster =
        read_native_conditional_policy_roster_v1(&bytes, LIMIT, |_| Ok::<_, ()>(())).unwrap();
    assert_eq!(
        roster.source_packet_sha256(),
        &<[u8; 32]>::from(Sha256::digest(PACKET))
    );
    assert_eq!(roster.source_packet_len(), PACKET.len() as u64);
    assert_eq!(roster.root_count(), 2);
    for (index, row) in roster.roots().enumerate() {
        assert_eq!(row.semantic_root(), ROOTS[index].0);
        assert_eq!(row.kernel_binding(), &ROOTS[index].1);
        assert_eq!(
            row.effect_signers(),
            policies.effects[index]
                .signer_identities()
                .iter()
                .map(|value| *value.as_bytes())
                .collect::<Vec<_>>(),
        );
        assert_eq!(
            row.effect_toolchain(),
            &toolchain(policies.effects[index].toolchain())
        );
        assert_eq!(
            row.formula_verifying_key(),
            policies.formulas[index].verifying_key()
        );
        assert_eq!(
            row.formula_toolchain(),
            &toolchain(policies.formulas[index].toolchain())
        );
        assert_eq!(
            row.formula_boundary(),
            policies.formulas[index].boundary() as u8
        );
    }
    assert_eq!(
        roster.roots().next().unwrap().effect_signers(),
        &[[1; 32], [2; 32], [3; 32]]
    );
    drop(bytes);
    assert_eq!(budget.storage(), reserved);
}

#[test]
fn policy_capture_changes_when_unused_accepted_signers_or_exact_packet_change() {
    let mut policies = Policies::new();
    let encode = |policies: &Policies, packet| {
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, LIMIT);
        policies.capture(packet, &mut budget).unwrap()
    };
    let original = encode(&policies, PACKET);
    policies.effects[0] = Effects::new(
        [digest(1), digest(2), digest(3), digest(4)],
        policies.effects[0].toolchain(),
    )
    .unwrap();
    let extended = encode(&policies, PACKET);
    assert_ne!(original, extended);
    let changed_packet = encode(&policies, b"different exact source packet");
    assert_ne!(extended, changed_packet);
    let extended =
        read_native_conditional_policy_roster_v1(&extended, LIMIT, |_| Ok::<_, ()>(())).unwrap();
    assert_eq!(extended.roots().next().unwrap().effect_signers().len(), 4);
    assert_eq!(
        extended.source_packet_sha256(),
        &<[u8; 32]>::from(Sha256::digest(PACKET))
    );
}

#[test]
fn policy_capture_exact_work_storage_and_one_short_keep_terminal_charges() {
    let policies = Policies::new();
    let run = |work_limit, storage_limit| {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(FLOOR).unwrap();
        let account = budget.work_ledger_identity_v1();
        let result = policies.capture(PACKET, &mut budget);
        assert!(budget.work_ledger_identity_v1() == account);
        assert_eq!(budget.storage(), budget.peak_storage());
        (
            result,
            budget.work(),
            budget.storage(),
            budget.failed_work(),
            budget.failed_storage(),
        )
    };
    let (result, exact_work, exact_storage, _, _) = run(WORK, LIMIT);
    let expected = result.unwrap();
    let (result, work, storage, failed_work, failed_storage) = run(exact_work, exact_storage);
    assert_eq!(result.unwrap(), expected);
    assert_eq!((work, storage), (exact_work, exact_storage));
    assert_eq!((failed_work, failed_storage), (None, None));
    let (result, work, storage, failed_work, _) = run(exact_work - 1, exact_storage);
    assert!(result.is_err());
    assert!(work < exact_work);
    assert_eq!(storage, exact_storage);
    assert_eq!(failed_work, Some(exact_work));
    let (result, _, storage, _, failed_storage) = run(exact_work, exact_storage - 1);
    assert!(matches!(result, Err(Error::Resource(Resource::Storage(_)))));
    assert!(storage >= FLOOR + SCRATCH);
    assert_eq!(failed_storage, Some(exact_storage));
}

#[test]
fn policy_capture_rejects_missing_duplicate_and_misassociated_roots() {
    let policies = Policies::new();
    for roots in [
        ROOTS[..1].to_vec(),
        vec![ROOTS[1], ROOTS[0]],
        vec![ROOTS[0], (ROOTS[1].0, ROOTS[0].1)],
    ] {
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let result = capture(PACKET, &policies.borrowed(), roots.into_iter(), &mut budget);
        assert!(result.is_err());
        assert_eq!(budget.storage(), budget.peak_storage());
        assert!(budget.storage() >= FLOOR);
    }
}

#[test]
fn policy_capture_bounds_roots_before_copying_policies() {
    let policies = Policies::new();
    let rows: Vec<_> = (0..MAX_NATIVE_CONDITIONAL_POLICY_ROSTER_ROOTS_V1 + 1)
        .map(|index| NativeConditionalRootPolicyV2 {
            semantic_root: index as u32,
            effects: &policies.effects[0],
            formula: &policies.formulas[0],
        })
        .collect();
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    assert!(
        capture(
            PACKET,
            &rows,
            (0..rows.len()).map(|index| (index as u32, [7; 32])),
            &mut budget,
        )
        .is_err()
    );
    assert_eq!(budget.storage(), FLOOR);
    assert_eq!(budget.work(), 1);
}

#[test]
fn policy_capture_accepts_full_root_and_effect_signer_bounds() {
    let mut policies = Policies::new();
    policies.effects[0] = Effects::new(
        (1..=MAX_NATIVE_CONDITIONAL_POLICY_SIGNERS_PER_ROOT_V1).map(|index| {
            let mut bytes = [0; 32];
            bytes[..4].copy_from_slice(&(index as u32).to_le_bytes());
            DigestV1::from_untrusted_bytes(bytes)
        }),
        tools(10),
    )
    .unwrap();
    let count = MAX_NATIVE_CONDITIONAL_POLICY_ROSTER_ROOTS_V1;
    let rows: Vec<_> = (0..count)
        .map(|index| NativeConditionalRootPolicyV2 {
            semantic_root: (count - index) as u32,
            effects: &policies.effects[usize::from(index != 0)],
            formula: &policies.formulas[index % 2],
        })
        .collect();
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, LIMIT);
    let bytes = capture(
        PACKET,
        &rows,
        (0..count).map(|index| ((count - index) as u32, [(index + 1) as u8; 32])),
        &mut budget,
    )
    .unwrap();
    let roster =
        read_native_conditional_policy_roster_v1(&bytes, LIMIT, |_| Ok::<_, ()>(())).unwrap();
    assert_eq!(roster.root_count(), count);
    assert_eq!(
        roster.roots().next().unwrap().effect_signers().len(),
        MAX_NATIVE_CONDITIONAL_POLICY_SIGNERS_PER_ROOT_V1
    );
    assert_eq!(roster.roots().last().unwrap().semantic_root(), 1);
}
