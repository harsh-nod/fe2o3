//! Wire fixtures exercise no protected source or finalizer construction.
use super::super::{
    ConditionalWorkerCompactFinalizerReplayV5 as Replay,
    codec::{Core, Schema},
};
use super::*;

fn wire(providers: usize, options: usize) -> Vec<u8> {
    let mut bytes = fixture(providers, options, true);
    bytes[..8].copy_from_slice(b"F2CCFR05");
    bytes[8..10].copy_from_slice(&5_u16.to_le_bytes());
    seal(&mut bytes);
    bytes
}
fn seal(bytes: &mut [u8]) {
    let n = bytes.len() - 32;
    let checksum = hash(Schema::Conditional.checksum_domain(), &bytes[..n]);
    bytes[n..].copy_from_slice(&checksum);
}
fn conditional(bytes: &[u8]) -> Result<Replay> {
    let mut work = Work::new(usize::MAX);
    let mut b = Budget::new(&mut work, usize::MAX);
    b.reserve_storage(bytes.len()).unwrap();
    Replay::decode_canonical(bytes, &mut b).map(|value| value.0)
}

#[test]
fn conditional_round_trip_keeps_v5_coordinates_tail_and_byte_owner() {
    let bytes = wire(1, 1);
    let value = conditional(&bytes).unwrap();
    assert_eq!(value.canonical_bytes(), bytes);
    assert_eq!(value.expected_finalization_identity(), &[1; 32]);
    assert_eq!(value.source_evidence_identity(), &[2; 32]);
    assert_eq!(value.binding_identity(), &[3; 32]);
    let coordinates = value.coordinates();
    assert_eq!(coordinates.outer_identity_coordinates(), ([4; 32], 123));
    assert_eq!(
        coordinates.attempt(),
        decode(&fixture(1, 1, true)).unwrap().attempt()
    );
    assert_eq!(
        coordinates.slot(),
        fe2o3_artifact_transaction::CompilerModuleHandoffSlotV5::Production
    );
    assert_eq!(coordinates.transaction_identity().as_bytes(), &[7; 32]);
    let tail = value.replay_view();
    assert_eq!(tail.worker.worker_build_identity(), WORKER);
    assert_eq!(tail.worker.llvm_build_identity(), LLVM);
    assert_eq!(tail.execution_limits.timeout(), Duration::from_secs(2));
    assert_eq!(tail.bootstrap_output_bound, 4096);
    assert_eq!(tail.external_providers.len(), 1);
    assert_eq!(tail.link_options.len(), 1);
    assert_eq!(tail.bootstrap_metadata.diagnostics_body(), [0; 4]);
    assert_eq!(tail.replay_metadata.diagnostics_body(), [0; 4]);
    assert!(!value.authenticates_compiler_origin());
    assert!(!value.grants_publication_authority());
    assert!(!value.grants_load_authority());
    assert!(!value.grants_launch_authority());
    assert_eq!(value.identity(), conditional(&bytes).unwrap().identity());
    let pointer = value.canonical_bytes().as_ptr();
    let owned = value.into_canonical_bytes();
    assert_eq!(owned.as_ptr(), pointer);
    assert_eq!(owned, bytes);
}

#[test]
fn families_reject_each_others_wire_and_checksum_domains() {
    let native = fixture(0, 0, true);
    let conditional_wire = wire(0, 0);
    assert!(conditional(&native).is_err());
    assert!(decode(&conditional_wire).is_err());
    assert_ne!(
        decode(&native).unwrap().identity().as_bytes(),
        conditional(&conditional_wire)
            .unwrap()
            .identity()
            .as_bytes()
    );
    let mut wrong_checksum = conditional_wire;
    reseal(&mut wrong_checksum);
    assert!(matches!(
        conditional(&wrong_checksum),
        Err(NativeWorkerCompactReplayErrorV1::Checksum)
    ));
    let mut wrong_version = wire(0, 0);
    wrong_version[8] = 1;
    seal(&mut wrong_version);
    assert!(matches!(
        conditional(&wrong_version),
        Err(NativeWorkerCompactReplayErrorV1::Version)
    ));
}

#[test]
fn shared_header_encoder_preserves_the_frozen_native_wire_and_exact_v5_fields() {
    for (bytes, schema) in [
        (fixture(2, 2, true), Schema::Native),
        (wire(2, 2), Schema::Conditional),
    ] {
        let mut value = Core::decode_owned(bytes.clone(), schema).unwrap();
        let mut encoded = Vec::new();
        value.header.encode(schema, &mut encoded);
        assert_eq!(encoded, bytes[..HEADER_BYTES]);
        value.header.verify_outer(&[4; 32], 123).unwrap();
        assert!(value.header.verify_outer(&[5; 32], 123).is_err());
        assert!(value.header.verify_outer(&[4; 32], 124).is_err());
        value.header.slot = 1;
        encoded.clear();
        value.header.encode(schema, &mut encoded);
        assert_eq!(encoded[SLOT], 1); // Unsupported slots are not normalized away.
    }
}

#[test]
fn conditional_rejects_truncation_unbound_coordinates_retired_slots_and_bad_tails() {
    let original = wire(0, 0);
    for n in 0..original.len() {
        assert!(conditional(&original[..n]).is_err(), "prefix {n}");
    }
    for range in [
        10..42,
        42..74,
        74..106,
        OUTER + 32..OUTER + 40,
        GENERATION..SESSION,
        TRANSACTION..HEADER_BYTES,
    ] {
        let mut bad = original.clone();
        bad[range].fill(0);
        seal(&mut bad);
        assert!(conditional(&bad).is_err());
    }
    for (offset, value) in [(SLOT, 1), (SLOT, 255), (PROVIDERS, 255)] {
        let mut bad = original.clone();
        bad[offset] = value;
        seal(&mut bad);
        assert!(conditional(&bad).is_err());
    }
    let mut bad = original.clone();
    bad.insert(bad.len() - 32, 0);
    seal(&mut bad);
    assert!(conditional(&bad).is_err());
    let mut bad = original;
    bad[OUTER + 32..OUTER + 40].copy_from_slice(
        &(fe2o3_artifact_transaction::MAX_COMPILER_MODULE_HANDOFF_BYTES_V5 as u64 + 1)
            .to_le_bytes(),
    );
    seal(&mut bad);
    assert!(conditional(&bad).is_err());
}

#[test]
fn every_conditional_coordinate_remains_identity_bound() {
    let original = wire(0, 0);
    let identity = conditional(&original).unwrap().identity();
    for offset in [
        10,
        42,
        74,
        OUTER,
        OUTER + 32,
        GENERATION,
        SESSION,
        INVOCATION,
        TRANSACTION,
        HEADER_BYTES,
        LIMITS,
        LIMITS + 28,
    ] {
        let mut changed = original.clone();
        changed[offset] ^= 1;
        seal(&mut changed);
        assert_ne!(
            conditional(&changed).unwrap().identity(),
            identity,
            "offset {offset}"
        );
    }
}

#[test]
fn conditional_exact_and_short_resource_quotes_keep_the_original_ledger() {
    let bytes = wire(MAX_PROVIDERS, MAX_LINK_OPTIONS);
    let quote = NativeWorkerCompactReplayResourcesV1::for_wire_length(bytes.len()).unwrap();
    for case in 0..4 {
        let floor = bytes.len() - usize::from(case == 3);
        let mut work = Work::new(19 + quote.work() - usize::from(case == 1));
        let mut b = Budget::new(
            &mut work,
            floor + quote.scratch_storage() - usize::from(case == 2),
        );
        b.reserve_storage(floor).unwrap();
        b.charge_work(19).unwrap();
        let ledger = b.work_ledger_identity_v1();
        let result = Replay::decode_canonical(&bytes, &mut b);
        assert_eq!(b.storage(), floor);
        assert!(b.work_ledger_identity_v1() == ledger);
        if case == 0 {
            let (value, storage) = result.unwrap();
            assert_eq!(value.storage(), storage);
            assert_eq!(value.replay_view().external_providers.len(), MAX_PROVIDERS);
            assert_eq!(value.replay_view().link_options.len(), MAX_LINK_OPTIONS);
            assert_eq!(b.work(), 19 + quote.work());
            assert_eq!(
                storage.retained_storage(),
                size_of::<Replay>()
                    + bytes.len()
                    + MAX_PROVIDERS * size_of::<WorkerV3ProviderReplayReferenceV1>()
                    + MAX_LINK_OPTIONS * size_of::<LinkOptionV1>()
                    + WORKER.len()
                    + LLVM.len()
                    + MAX_LINK_OPTIONS * 4
            );
            assert!(storage.retained_storage() <= quote.scratch_storage());
        } else {
            assert!(matches!(
                result,
                Err(NativeWorkerCompactReplayErrorV1::Resource(_))
            ));
        }
    }
}

#[test]
fn encoder_prepays_before_observing_inputs_and_releases_scratch_on_unwind() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    for (quota, storage) in [(0, usize::MAX), (usize::MAX, FRAME - 1)] {
        let mut work = Work::new(quota);
        let mut b = Budget::new(&mut work, storage);
        assert!(matches!(
            super::super::codec::prepare(0, Schema::Conditional, &mut b, || panic!(
                "unpaid input must not be observed"
            )),
            Err(NativeWorkerCompactReplayErrorV1::Resource(_))
        ));
        assert_eq!(b.storage(), 0);
    }
    let mut work = Work::new(usize::MAX);
    let mut b = Budget::new(&mut work, FRAME + 19);
    b.reserve_storage(19).unwrap();
    let ledger = b.work_ledger_identity_v1();
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = super::super::codec::prepare(19, Schema::Conditional, &mut b, || {
                panic!("injected input failure")
            });
        }))
        .is_err()
    );
    assert_eq!(b.storage(), 19);
    assert_eq!(b.work(), 512);
    assert!(b.work_ledger_identity_v1() == ledger);
}
