use ed25519_dalek::SigningKey;
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_BYTES_V2 as BYTES,
    COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_STORAGE_V2 as STORAGE,
    COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_WORK_V2 as WORK,
    CompilerExecutionClientProcessIdentityV1 as Client,
    CompilerExecutionExternalAnchorServiceIdentityV1 as Service,
    CompilerExecutionIssuerMeasurementV1 as Measurement,
    CompilerExecutionIssuerPolicyV1 as LegacyPolicy, CompilerExecutionIssuerPolicyV2 as Policy,
    CompilerExecutionServiceLaunchManifestErrorV1 as Framing,
    CompilerExecutionServiceLaunchManifestErrorV2 as Error,
    CompilerExecutionServiceLaunchManifestV1 as Legacy,
    CompilerExecutionServiceLaunchManifestV2 as Manifest,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work,
};
use sha2::{Digest, Sha256};

fn client() -> Client {
    Client::new(1234, 5678, 9012).unwrap()
}
fn service() -> Service {
    Service::new(6001, 7001).unwrap()
}
fn policy(generation: u64, b: &mut Budget<'_>) -> Policy {
    let (p, charge) = Policy::new(
        generation,
        Measurement::new([0x61; 32], 12345).unwrap(),
        Measurement::new([0x62; 32], 67890).unwrap(),
        SigningKey::from_bytes(&[0x51; 32])
            .verifying_key()
            .to_bytes(),
        SigningKey::from_bytes(&[0x52; 32])
            .verifying_key()
            .to_bytes(),
        b,
    )
    .unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    p
}
fn rehash(bytes: &mut [u8]) {
    let mut h = Sha256::new();
    h.update(b"FE2O3/COMPILER-EXECUTION-SERVICE-LAUNCH-MANIFEST/V1\0");
    h.update(80u64.to_le_bytes());
    h.update(&bytes[..80]);
    bytes[80..].copy_from_slice(&h.finalize());
}
fn wire(policy: &[u8; 32]) -> [u8; BYTES] {
    let mut bytes = [0; BYTES];
    bytes[..8].copy_from_slice(b"F2O3CEL1");
    bytes[8..10].copy_from_slice(&1u16.to_le_bytes());
    for (offset, value) in [
        (12, 112u32),
        (24, 1234),
        (28, 5678),
        (32, 9012),
        (40, 6001),
        (44, 7001),
    ] {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    bytes[48..80].copy_from_slice(policy);
    rehash(&mut bytes);
    bytes
}
fn decode(bytes: &[u8]) -> Result<(Manifest, usize), Error> {
    let mut w = Work::new(WORK);
    let mut b = Budget::new(&mut w, BYTES + STORAGE);
    b.reserve_storage(BYTES).unwrap();
    let result = Manifest::decode(bytes, &mut b).map(|(v, s)| (v, s.additional_storage()));
    assert_eq!(b.storage(), BYTES);
    assert_eq!(b.work(), WORK);
    result
}

#[test]
fn independent_wire_and_contextual_policy_match_preserve_both_api_families() {
    let mut w = Work::new(1_000_000);
    let mut b = Budget::new(&mut w, 1_000_000);
    let p = policy(7, &mut b);
    let other = policy(8, &mut b);
    let legacy = LegacyPolicy::new(
        7,
        p.executable(),
        p.runtime(),
        *p.verifying_key(),
        *p.external_anchor_verifying_key(),
    )
    .unwrap();
    let old = Legacy::new(client(), service(), &legacy);
    assert_eq!(old.canonical_bytes(), &wire(legacy.identity().as_bytes()));
    let floor = b.storage();
    let (m, charge) = Manifest::new(client(), service(), &p, &mut b).unwrap();
    assert_eq!(b.storage(), floor);
    assert_eq!(charge.additional_storage(), m.retained_storage());
    b.reserve_storage(charge.additional_storage()).unwrap();
    assert_eq!(m.canonical_bytes(), &wire(p.identity().as_bytes()));
    assert_eq!(m.client(), client());
    assert_eq!(m.external_anchor_service(), service());
    assert_eq!(m.policy_identity(), p.identity());
    assert!(m.matches_policy(&p, &mut b).unwrap());
    assert!(!m.matches_policy(&other, &mut b).unwrap());
    let (decoded, charge) = decode(m.canonical_bytes()).unwrap();
    assert_eq!(decoded, m);
    assert_eq!(charge, m.retained_storage());
    assert!(m.identity().matches_canonical_bytes(m.canonical_bytes()));
    let legacy_view = Legacy::decode(m.canonical_bytes()).unwrap();
    assert!(!legacy_view.matches_policy(&legacy));
    let (old_view, _) = decode(old.canonical_bytes()).unwrap();
    b.reserve_storage(old_view.retained_storage()).unwrap();
    assert!(!old_view.matches_policy(&p, &mut b).unwrap());
    assert_ne!(old.identity(), m.identity());
}

#[test]
fn every_reserved_field_and_resealed_context_error_has_identical_legacy_precedence() {
    let good = wire(&[0x61; 32]);
    let mut cases = vec![(good.to_vec(), Framing::Identity)];
    cases[0].0[80] ^= 1;
    for index in [10, 11, 16, 17, 18, 19, 20, 21, 22, 23, 36, 37, 38, 39] {
        let mut bytes = good;
        bytes[index] = 1;
        rehash(&mut bytes);
        cases.push((bytes.to_vec(), Framing::Reserved));
    }
    for (offset, len, error) in [
        (0, 1, Framing::Magic),
        (8, 2, Framing::Version),
        (12, 4, Framing::Length),
        (24, 4, Framing::ClientPid),
        (48, 32, Framing::PolicyIdentity),
    ] {
        let mut bytes = good;
        bytes[offset..offset + len].fill(0);
        rehash(&mut bytes);
        cases.push((bytes.to_vec(), error));
    }
    cases.push((good[..111].to_vec(), Framing::Length));
    let mut long = good.to_vec();
    long.push(0);
    cases.push((long, Framing::Length));
    // Invalid client wins over invalid anchor, policy and footer.
    let mut multiple = good;
    multiple[24..28].fill(0);
    multiple[40..112].fill(0);
    cases.push((multiple.to_vec(), Framing::ClientPid));
    for (bytes, expected) in cases {
        assert_eq!(Legacy::decode(&bytes).unwrap_err(), expected);
        assert!(matches!(decode(&bytes), Err(Error::Framing(e)) if e==expected));
    }
    for offset in [40, 44] {
        let mut bytes = good;
        bytes[offset..offset + 4].fill(0);
        rehash(&mut bytes);
        assert!(matches!(
            decode(&bytes),
            Err(Error::Framing(Framing::ExternalAnchorServiceIdentity(_)))
        ));
    }
}

#[test]
fn exact_and_one_short_decode_budgets_restore_floor_and_keep_denial_history() {
    let bytes = wire(&[0x61; 32]);
    for mode in 0..4 {
        let floor = BYTES - usize::from(mode == 0);
        let mut w = Work::new(WORK - usize::from(mode == 1));
        let limit = floor + STORAGE - usize::from(mode == 2);
        let mut b = Budget::new(&mut w, limit);
        b.reserve_storage(floor).unwrap();
        assert!(b.reserve_storage(limit + 1).is_err());
        let result = Manifest::decode(&bytes, &mut b);
        assert_eq!(b.storage(), floor);
        assert_eq!(b.failed_storage(), Some(floor + limit + 1));
        match mode {
            0 => {
                assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
                assert_eq!(b.work(), 8);
            }
            1 => {
                assert!(matches!(result, Err(Error::Resource(Resource::Work(_)))));
                assert_eq!(b.work(), 8);
            }
            2 => {
                assert!(matches!(result, Err(Error::Resource(Resource::Storage(_)))));
                assert_eq!(b.work(), WORK);
            }
            _ => {
                result.unwrap();
                assert_eq!(b.work(), WORK);
                assert_eq!(b.peak_storage(), limit);
            }
        }
    }
}

#[test]
fn constructor_and_match_require_the_complete_borrowed_input_floor() {
    let mut w = Work::new(1_000_000);
    let mut b = Budget::new(&mut w, 1_000_000);
    let p = policy(7, &mut b);
    let (m, _) = Manifest::new(client(), service(), &p, &mut b).unwrap();
    for (matching, floor) in [
        (false, p.retained_storage()),
        (true, p.retained_storage() + m.retained_storage()),
    ] {
        for short in [false, true] {
            let mut w = Work::new(WORK);
            let mut b = Budget::new(&mut w, floor + STORAGE);
            b.reserve_storage(floor - usize::from(short)).unwrap();
            let result = if matching {
                m.matches_policy(&p, &mut b)
            } else {
                Manifest::new(client(), service(), &p, &mut b).map(|_| true)
            };
            if short {
                assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
            } else {
                assert!(result.unwrap());
                assert_eq!(b.peak_storage(), floor + STORAGE);
            }
            assert_eq!(b.storage(), floor - usize::from(short));
        }
    }
}
