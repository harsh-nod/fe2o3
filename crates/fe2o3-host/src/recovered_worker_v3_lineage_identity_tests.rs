use super::super::{
    RecoveredWorkerV3EntrypointV1, WORKER_V3_HOST_LINEAGE_DOMAIN_V1 as LEGACY_DOMAIN,
    WorkerV3HostLineageEvidenceV1, WorkerV3HostLineageIdentityV1,
    derive_roster_host_lineage_identity, nominal::LINEAGE_DOMAIN as NOMINAL_DOMAIN,
};
use super::*;

// Hash-only fixture. Pins independently calculated from the frozen legacy field order
// at 99d43a3; these primitive values are not an admitted publication or signed receipt.
fn fixture() -> Preimage<'static> {
    Preimage {
        subject: ([1; 32], b"subject\0v1"),
        receipt: ([2; 32], b"receipt\0v1"),
        finalizer: [3; 32],
        record: [4; 32],
        outer: ([5; 32], 0x010203),
        capsule: ([6; 32], 0x020304),
        module: ([7; 32], 0x030405),
        receipts: std::array::from_fn(|i| ([0x10 + i as u8; 32], 0x1000 + i as u64)),
        linked: [0x30; 32],
        finalized: ([0x31; 32], 0x040506),
        code_object: [0x32; 32],
        kernel: [0x33; 32],
    }
}

fn assert_hex(actual: [u8; 32], expected: &str) {
    assert_eq!(expected.len(), 64);
    assert_eq!(
        actual,
        std::array::from_fn(|i| u8::from_str_radix(&expected[2 * i..2 * i + 2], 16).unwrap())
    );
}

#[test]
fn legacy_and_nominal_hashes_pin_identical_coordinates_under_distinct_domains() {
    let legacy = hash(LEGACY_DOMAIN, &fixture());
    let nominal = hash(NOMINAL_DOMAIN, &fixture());
    assert_hex(
        legacy,
        "90783e41def1e05e4f2f7df291a966db16ceba3b1b029c961a6122899625a65f",
    );
    assert_hex(
        nominal,
        "b87c56d4c7d19ebe4421f22091f1a0585e5560f648847ffdc9019f96a5d89d16",
    );
    assert_ne!(legacy, nominal);
}

#[test]
fn every_lineage_coordinate_and_receipt_order_changes_identity() {
    let changed = |mutate: &dyn Fn(&mut Preimage<'static>)| {
        let mut input = fixture();
        mutate(&mut input);
        for domain in [LEGACY_DOMAIN, NOMINAL_DOMAIN] {
            assert_ne!(hash(domain, &input), hash(domain, &fixture()));
        }
    };
    changed(&|p| p.subject.0[0] ^= 1);
    changed(&|p| p.subject.1 = b"subject\0v2");
    changed(&|p| p.receipt.0[0] ^= 1);
    changed(&|p| p.receipt.1 = b"receipt\0v2");
    changed(&|p| p.finalizer[0] ^= 1);
    changed(&|p| p.record[0] ^= 1);
    changed(&|p| p.outer.0[0] ^= 1);
    changed(&|p| p.outer.1 += 1);
    changed(&|p| p.capsule.0[0] ^= 1);
    changed(&|p| p.capsule.1 += 1);
    changed(&|p| p.module.0[0] ^= 1);
    changed(&|p| p.module.1 += 1);
    for i in 0..15 {
        changed(&|p| p.receipts[i].0[0] ^= 1);
        changed(&|p| p.receipts[i].1 += 1);
    }
    changed(&|p| p.receipts.swap(0, 1));
    changed(&|p| p.linked[0] ^= 1);
    changed(&|p| p.finalized.0[0] ^= 1);
    changed(&|p| p.finalized.1 += 1);
    changed(&|p| p.code_object[0] ^= 1);
    changed(&|p| p.kernel[0] ^= 1);
}

#[test]
fn singleton_roster_aggregation_preserves_legacy_and_nominal_pins() {
    for (domain, expected) in [
        (
            LEGACY_DOMAIN,
            "86b2f7d6d3835cfc79a177fdc3343f966120b2e09d055686e57c9fbbe25745df",
        ),
        (
            NOMINAL_DOMAIN,
            "2447481d5d2c9cb3b9a61f18d5818e2021516980322cbe377195c69931fdba75",
        ),
    ] {
        let input = fixture();
        let entry = RecoveredWorkerV3EntrypointV1 {
            ordinal: 0,
            descriptor_index: 0,
            physical_kernel_index: 0,
            lineage: WorkerV3HostLineageEvidenceV1 {
                identity: WorkerV3HostLineageIdentityV1(hash(domain, &input)),
                finalizer_derivation_sha256: input.finalizer,
                capsule_sha256: input.capsule.0,
                formal_memory_sha256: input.receipts[6].0,
                proof_binding_sha256: input.receipts[7].0,
                finalized_sha256: input.finalized.0,
                finalized_length: input.finalized.1,
            },
        };
        assert_hex(
            *derive_roster_host_lineage_identity(&[entry])
                .identity()
                .as_bytes(),
            expected,
        );
    }
}
