//! Inert content/signature tests never issue an executed owner.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

const LIMIT: usize = 8_000_000;
const SEMANTIC: &[u8] = b"inert semantic content";
const SSA: [u8; 32] = [7; 32];
const GRAPHS: [&[u8]; 4] = [b"original", b"policy11", b"licm", b"forwarded"];
const PREFIX: &[u8] = b"inert complete prefix witness";
const GENERATED: &[u8] = b"verus! { proof fn inert_shape() {} }\n";
fn input(wire: &[u8]) -> MixedMiddleEndInputV90<'_> {
    MixedMiddleEndInputV90 {
        semantic_mir: SEMANTIC,
        source_ssa_identity: &SSA,
        original: GRAPHS[0],
        prefix: GRAPHS[1],
        licm: GRAPHS[2],
        forwarded: GRAPHS[3],
        prefix_witness: PREFIX,
        generated_source: GENERATED,
        execution_receipt: wire,
    }
}
fn part(hash: &mut Sha256, bytes: &[u8]) {
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
}
fn graph(bytes: &[u8]) -> ([u8; 32], u64) {
    let domain = b"FE2O3/VERIFIED-CANONICAL-KERNEL-IR/V18\0";
    let mut hash = Sha256::new();
    hash.update((domain.len() as u32).to_le_bytes());
    hash.update(domain);
    hash.update(1u16.to_le_bytes());
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    (hash.finalize().into(), bytes.len() as u64)
}
fn expected() -> (Expected, SigningKey) {
    let key = SigningKey::from_bytes(&[21; 32]);
    let mut hash = Sha256::new();
    hash.update(b"fe2o3-generated-verus-proof-input-v3\0");
    hash.update((GENERATED.len() as u64).to_le_bytes());
    hash.update(GENERATED);
    let mut binding = Binding {
        policy: 11,
        source: [Sha256::digest(SEMANTIC).into(), SSA],
        graphs: GRAPHS.map(graph),
        statement: [0; 32],
        generated: hash.finalize().into(),
        witness: Sha256::digest(PREFIX).into(),
        context: [17; 32],
        witness_bytes: PREFIX.len() as u64,
        rounds: 2,
        census: [2, 3, 10, 5, 12, 21],
    };
    let mut hash = Sha256::new();
    for bytes in [
        b"FE2O3/ORIGINAL-MIR/POLICY11/LICM/STORE-CONSENSUS/PREDICATED-TYPED/V90\0".as_slice(),
        &binding.source[0],
        &binding.source[1],
        &binding.witness,
        &binding.context,
    ] {
        part(&mut hash, bytes);
    }
    for (digest, len) in binding.graphs {
        part(&mut hash, &digest);
        part(&mut hash, &len.to_le_bytes());
    }
    for n in binding.census {
        part(&mut hash, &n.to_le_bytes());
    }
    part(&mut hash, &binding.generated);
    binding.statement = hash.finalize().into();
    (
        Expected {
            binding,
            runtime: [18; 32],
            toolchain: [[19; 32]; 5],
            execution: [20; 32],
            key: key.verifying_key().to_bytes(),
        },
        key,
    )
}
fn sign(e: &Expected, key: &SigningKey) -> [u8; WIRE] {
    let mut wire = [0; WIRE];
    let bytes = e.unsigned();
    wire[..UNSIGNED].copy_from_slice(&bytes);
    wire[UNSIGNED..].copy_from_slice(&key.sign(&bytes).to_bytes());
    wire
}
fn run(
    input: MixedMiddleEndInputV90<'_>,
    work: usize,
    storage: usize,
) -> (Result<()>, usize, usize) {
    let mut ledger = Work::new(work);
    let mut budget = Budget::new(&mut ledger, storage);
    let result = check_inert_predicated_typed_source_receipt_v90(input, &mut budget).map(|view| {
        assert_eq!(view.canonical_bytes(), input.execution_receipt);
        assert!(!view.authenticates_execution() && !view.grants_load_or_launch_authority());
    });
    assert_eq!(budget.storage(), 0);
    (result, budget.work(), budget.peak_storage())
}

#[test]
fn predicated_inert_typed_receipt_binds_exact_fields_without_authenticating_a_self_signer() {
    let (expected, key) = expected();
    let wire = sign(&expected, &key);
    run(input(&wire), LIMIT, LIMIT).0.unwrap();
    // A public signer may claim another runtime. Only the protected receiving
    // backend can authenticate that claim; this decoder must remain inert.
    let mut other = expected;
    other.runtime = [88; 32];
    let signed = sign(&other, &key);
    let mut ledger = Work::new(LIMIT);
    let mut budget = Budget::new(&mut ledger, LIMIT);
    let view =
        check_inert_predicated_typed_source_receipt_v90(input(&signed), &mut budget).unwrap();
    assert_eq!(view.claimed_runtime_identity(), [88; 32]);
    assert!(!view.authenticates_execution());
    assert_eq!(view.graph_identities(), GRAPHS.map(graph));
    let semantic: [u8; 32] = Sha256::digest(SEMANTIC).into();
    assert_eq!(view.source_semantic_identity(), semantic);
    assert_eq!(view.claimed_census(), expected.binding.census);
}

#[test]
fn predicated_inert_typed_receipt_rejects_changed_input_every_wire_byte_and_legacy_shape() {
    let (expected, key) = expected();
    let wire = sign(&expected, &key);
    for axis in 0..8 {
        let mut changed = input(&wire);
        match axis {
            0 => changed.semantic_mir = b"foreign source",
            1 => changed.source_ssa_identity = &[2; 32],
            2 => changed.original = b"foreign original",
            3 => changed.prefix = b"foreign prefix",
            4 => changed.licm = b"foreign licm",
            5 => changed.forwarded = GRAPHS[2],
            6 => changed.prefix_witness = b"foreign witness",
            _ => changed.generated_source = b"verus! { proof fn changed() {} }\n",
        }
        assert!(run(changed, LIMIT, LIMIT).0.is_err(), "axis {axis}");
    }
    for index in 0..WIRE {
        let mut changed = wire;
        changed[index] ^= 1;
        assert!(
            run(input(&changed), LIMIT, LIMIT).0.is_err(),
            "byte {index}"
        );
    }
    for (policy, rounds) in [(10, 2), (11, 0), (11, 33)] {
        let mut changed = expected;
        changed.binding.policy = policy;
        changed.binding.rounds = rounds;
        assert!(run(input(&sign(&changed, &key)), LIMIT, LIMIT).0.is_err());
    }
    assert!(run(input(&wire[..WIRE - 1]), LIMIT, LIMIT).0.is_err());
    let mut trailing = wire.to_vec();
    trailing.push(0);
    assert!(run(input(&trailing), LIMIT, LIMIT).0.is_err());
}

#[test]
fn predicated_inert_typed_receipt_keeps_exact_work_storage_and_nested_resource_errors() {
    let (expected, key) = expected();
    let wire = sign(&expected, &key);
    let measured = run(input(&wire), LIMIT, LIMIT);
    measured.0.unwrap();
    let exact = run(input(&wire), measured.1, measured.2);
    exact.0.unwrap();
    assert_eq!((exact.1, exact.2), (measured.1, measured.2));
    let short = run(input(&wire), measured.1 - 1, measured.2);
    assert!(matches!(short.0, Err(Error::Resource(Resource::Work(e)))
        if e.actual() == measured.1 && e.limit() == measured.1 - 1));
    let short = run(input(&wire), measured.1, measured.2 - 1);
    assert!(matches!(short.0, Err(Error::Resource(Resource::Storage(e)))
        if e.actual() == measured.2 && e.limit() == measured.2 - 1));
}

fn legacy_input(wire: &[u8]) -> fe2o3_compiler_lineage::MixedMiddleEndInputV50<'_> {
    fe2o3_compiler_lineage::MixedMiddleEndInputV50 {
        semantic_mir: SEMANTIC,
        source_ssa_identity: &SSA,
        original: GRAPHS[0],
        prefix: GRAPHS[1],
        licm: GRAPHS[2],
        forwarded: GRAPHS[3],
        prefix_witness: PREFIX,
        generated_source: GENERATED,
        execution_receipt: wire,
    }
}

fn legacy_wire() -> (Vec<u8>, Expected, SigningKey) {
    let (mut legacy, key) = expected();
    let b = &mut legacy.binding;
    let mut statement = Sha256::new();
    for bytes in [
        b"FE2O3/ORIGINAL-MIR/POLICY11/LICM/STORE-CONSENSUS/TYPED/V50\0".as_slice(),
        &b.source[0],
        &b.source[1],
        &b.witness,
        &b.context,
    ] {
        part(&mut statement, bytes);
    }
    for (identity, bytes) in b.graphs {
        part(&mut statement, &identity);
        part(&mut statement, &bytes.to_le_bytes());
    }
    for n in b.census {
        part(&mut statement, &n.to_le_bytes());
    }
    part(&mut statement, &b.generated);
    b.statement = statement.finalize().into();
    // Independent historical transcript, not a call to either family encoder.
    let mut wire = b"FE2O3/MIXED/TYPED-SOURCE-TAIL/V50\0".to_vec();
    wire.extend_from_slice(&50u16.to_le_bytes());
    wire.extend_from_slice(&[4, 1]);
    wire.extend_from_slice(&11u16.to_le_bytes());
    for identity in b.source {
        wire.extend_from_slice(&identity);
    }
    for (identity, bytes) in b.graphs {
        wire.extend_from_slice(&identity);
        wire.extend_from_slice(&bytes.to_le_bytes());
    }
    for identity in [b.statement, b.generated, b.witness, b.context] {
        wire.extend_from_slice(&identity);
    }
    for n in [b.witness_bytes, b.rounds] {
        wire.extend_from_slice(&n.to_le_bytes());
    }
    for n in b.census {
        wire.extend_from_slice(&n.to_le_bytes());
    }
    wire.extend_from_slice(&legacy.runtime);
    for identity in legacy.toolchain {
        wire.extend_from_slice(&identity);
    }
    wire.extend_from_slice(&legacy.execution);
    wire.extend_from_slice(&legacy.key);
    let signature = key.sign(&wire).to_bytes();
    wire.extend_from_slice(&signature);
    (wire, legacy, key)
}

#[test]
fn predicated_inert_receipt_preserves_legacy_transcript_and_rejects_both_families() {
    let (legacy, _, _) = legacy_wire();
    let (expected, key) = expected();
    let current = sign(&expected, &key);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let old =
        crate::check_inert_typed_source_receipt_v53(legacy_input(&legacy), &mut budget).unwrap();
    assert_eq!(old.canonical_bytes(), legacy);
    assert!(!old.authenticates_execution() && !old.grants_load_or_launch_authority());
    let new =
        check_inert_predicated_typed_source_receipt_v90(input(&current), &mut budget).unwrap();
    assert_eq!(new.canonical_bytes(), current);
    assert!(!new.authenticates_execution() && !new.grants_load_or_launch_authority());
    assert_ne!(old.statement_identity(), new.statement_identity());
    assert!(check_inert_predicated_typed_source_receipt_v90(input(&legacy), &mut budget).is_err());
    assert!(
        crate::check_inert_typed_source_receipt_v53(legacy_input(&current), &mut budget).is_err()
    );
    assert_eq!(budget.storage(), 0);
}

#[test]
fn predicated_inert_receipt_refuses_resigned_cross_family_statement_substitution() {
    let (legacy, legacy_expected, key) = legacy_wire();
    // A new-family header and valid signature cannot relabel a legacy statement.
    assert!(
        run(input(&sign(&legacy_expected, &key)), LIMIT, LIMIT)
            .0
            .is_err()
    );
    let (current_expected, _) = expected();
    let current = sign(&current_expected, &key);
    let old_magic = b"FE2O3/MIXED/TYPED-SOURCE-TAIL/V50\0";
    let mut disguised = old_magic.to_vec();
    disguised.extend_from_slice(&50u16.to_le_bytes());
    disguised.extend_from_slice(&current[MAGIC.len() + 2..UNSIGNED]);
    let signature = key.sign(&disguised).to_bytes();
    disguised.extend_from_slice(&signature);
    assert_eq!(disguised.len(), legacy.len());
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    assert!(
        crate::check_inert_typed_source_receipt_v53(legacy_input(&disguised), &mut budget).is_err()
    );
    assert_eq!(budget.storage(), 0);
}

#[test]
fn predicated_inert_receipt_preserves_outer_floor_and_sticky_denials_before_retry() {
    const FLOOR: usize = 37;
    let (expected, key) = expected();
    let wire = sign(&expected, &key);
    let measured = run(input(&wire), LIMIT, LIMIT);
    measured.0.unwrap();
    for (work_limit, storage_limit) in [
        (measured.1 - 1, FLOOR + measured.2),
        (measured.1, FLOOR + measured.2 - 1),
    ] {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(FLOOR).unwrap();
        assert!(
            check_inert_predicated_typed_source_receipt_v90(input(&wire), &mut budget).is_err()
        );
        assert_eq!(budget.storage(), FLOOR);
        let debited = budget.work();
        for _ in 0..2 {
            assert!(
                check_inert_predicated_typed_source_receipt_v90(input(&wire), &mut budget).is_err()
            );
            assert_eq!((budget.work(), budget.storage()), (debited, FLOOR));
        }
    }
}
