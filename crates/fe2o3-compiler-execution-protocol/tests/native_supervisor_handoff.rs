use ed25519_dalek::SigningKey;
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_ISSUER_POLICY_WORK_V2 as POLICY_WORK,
    COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_BYTES_V2 as LAUNCH_BYTES,
    COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_STORAGE_V2 as LAUNCH_STORAGE,
    COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_WORK_V2 as LAUNCH_WORK,
    COMPILER_EXECUTION_SUPERVISOR_HANDOFF_BYTES_V2 as BYTES,
    COMPILER_EXECUTION_SUPERVISOR_HANDOFF_STORAGE_V2 as STORAGE,
    COMPILER_EXECUTION_SUPERVISOR_HANDOFF_WORK_V2 as WORK,
    CompilerExecutionAttestationStorageV2 as Storage,
    CompilerExecutionClientProcessIdentityV1 as Client,
    CompilerExecutionExternalAnchorServiceIdentityV1 as Service,
    CompilerExecutionIssuerMeasurementV1 as Measurement,
    CompilerExecutionIssuerPolicyV1 as LegacyPolicy, CompilerExecutionIssuerPolicyV2 as Policy,
    CompilerExecutionServiceLaunchManifestErrorV1 as LaunchFraming,
    CompilerExecutionServiceLaunchManifestV1 as LegacyLaunch,
    CompilerExecutionServiceLaunchManifestV2 as Launch,
    CompilerExecutionSupervisorHandoffErrorV1 as Framing,
    CompilerExecutionSupervisorHandoffErrorV2 as Error,
    CompilerExecutionSupervisorHandoffV1 as Legacy,
    CompilerExecutionSupervisorHandoffV2 as Handoff,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work,
};
use sha2::{Digest, Sha256};
use std::{error::Error as _, mem::size_of};

const EXTRA: usize = 19;
const LIMIT: usize = 1_000_000;

fn client(pid: u32) -> Client {
    Client::new(pid, 1000, 1001).unwrap()
}

fn rehash(bytes: &mut [u8], prefix: usize, domain: &[u8]) {
    let mut h = Sha256::new();
    h.update(domain);
    h.update((prefix as u64).to_le_bytes());
    h.update(&bytes[..prefix]);
    bytes[prefix..].copy_from_slice(&h.finalize());
}

fn reseal(bytes: &mut [u8; BYTES]) {
    rehash(
        bytes,
        152,
        b"FE2O3/COMPILER-EXECUTION-SUPERVISOR-HANDOFF/V1\0",
    );
}

fn launch_wire(policy: &[u8; 32]) -> [u8; LAUNCH_BYTES] {
    let mut bytes = [0; LAUNCH_BYTES];
    bytes[..8].copy_from_slice(b"F2O3CEL1");
    bytes[8..10].copy_from_slice(&1u16.to_le_bytes());
    for (offset, value) in [
        (12, 112u32),
        (24, 200),
        (28, 1000),
        (32, 1001),
        (40, 6000),
        (44, 7000),
    ] {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    bytes[48..80].copy_from_slice(policy);
    rehash(
        &mut bytes,
        80,
        b"FE2O3/COMPILER-EXECUTION-SERVICE-LAUNCH-MANIFEST/V1\0",
    );
    bytes
}

fn wire(policy: &[u8; 32]) -> [u8; BYTES] {
    let mut bytes = [0; BYTES];
    bytes[..8].copy_from_slice(b"F2O3CEH1");
    bytes[8..10].copy_from_slice(&1u16.to_le_bytes());
    for (offset, value) in [(12, 184u32), (24, 100), (28, 1000), (32, 1001)] {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    bytes[40..152].copy_from_slice(&launch_wire(policy));
    reseal(&mut bytes);
    bytes
}

fn decode(bytes: &[u8]) -> Result<(Handoff, Storage), Error> {
    let input = if bytes.len() == BYTES { BYTES } else { 0 };
    let mut w = Work::new(WORK);
    let mut b = Budget::new(&mut w, input + STORAGE);
    b.reserve_storage(input).unwrap();
    let result = Handoff::decode(bytes, &mut b);
    assert_eq!(b.storage(), input);
    assert_eq!(b.peak_storage(), input + STORAGE);
    assert_eq!(b.work(), WORK);
    result
}

// Fixture admission is separate from each isolated operation budget below.
fn launch() -> Launch {
    let mut w = Work::new(LAUNCH_WORK);
    let mut b = Budget::new(&mut w, LAUNCH_BYTES + LAUNCH_STORAGE);
    b.reserve_storage(LAUNCH_BYTES).unwrap();
    Launch::decode(&launch_wire(&[0x61; 32]), &mut b).unwrap().0
}

fn assert_framing(bytes: &[u8], expected: Framing) {
    let legacy = Legacy::decode(bytes).unwrap_err();
    assert_eq!(legacy, expected);
    let error = decode(bytes).unwrap_err();
    assert_eq!(error.to_string(), legacy.to_string());
    assert_eq!(error.source().unwrap().to_string(), legacy.to_string());
    assert!(matches!(error, Error::Framing(e) if e == expected));
}

#[test]
fn independent_wire_both_wrappers_and_legacy_clone_surface_agree() {
    assert_eq!((BYTES, LAUNCH_BYTES, WORK), (184, 112, 9480));
    let bytes = wire(&[0x61; 32]);
    // Fixed footer fixtures were independently derived with Node's SHA-256.
    assert_eq!(
        &bytes[120..152],
        &[
            0xd5, 0xd4, 0xe8, 0x15, 0xd0, 0x1a, 0xa8, 0x2a, 0x5e, 0x77, 0x71, 0xda, 0x95, 0xc4,
            0x57, 0x3d, 0xbc, 0x71, 0x96, 0xb5, 0x74, 0xe2, 0x84, 0x6e, 0x34, 0xb3, 0x62, 0x8d,
            0x17, 0x1b, 0x5b, 0x27,
        ]
    );
    assert_eq!(
        &bytes[152..],
        &[
            0xae, 0x49, 0xbd, 0x3b, 0x98, 0xa2, 0x5c, 0x32, 0x5e, 0x59, 0xa8, 0x85, 0x49, 0x4e,
            0x20, 0x03, 0x53, 0x32, 0x4d, 0x19, 0x1e, 0x7d, 0xff, 0x4f, 0x7b, 0x88, 0x2d, 0x5a,
            0xca, 0xfd, 0xd5, 0xf0,
        ]
    );
    let old = Legacy::new(
        client(100),
        LegacyLaunch::decode(&launch_wire(&[0x61; 32])).unwrap(),
    )
    .unwrap();
    assert_eq!(old.canonical_bytes(), &bytes);
    assert_eq!(Legacy::decode(&bytes).unwrap(), old.clone());
    let (native, receipt) = decode(&bytes).unwrap();
    assert_eq!(native.canonical_bytes(), &bytes);
    assert_eq!(native.submitter(), old.submitter());
    assert_eq!(native.identity(), old.identity());
    assert_eq!(
        native.launch_manifest().canonical_bytes(),
        old.launch_manifest().canonical_bytes()
    );
    assert_eq!(native.launch_manifest().client(), client(200));
    assert_eq!(receipt.additional_storage(), native.retained_storage());
    assert_eq!(native.retained_storage(), size_of::<(Handoff, Storage)>());
    assert!(native.identity().matches_canonical_bytes(&bytes));
    assert!(!native.identity().matches_canonical_bytes(&bytes[..183]));
    let mut w = Work::new(WORK);
    let mut b = Budget::new(&mut w, LIMIT);
    let launch = launch();
    let inherited = launch.retained_storage();
    b.reserve_storage(inherited).unwrap();
    let (constructed, delta) = Handoff::new(client(100), launch, &mut b).unwrap();
    assert_eq!(constructed, native);
    assert_eq!(
        delta.additional_storage() + inherited,
        native.retained_storage()
    );
    assert_eq!(b.storage(), inherited);
}

#[test]
fn every_byte_mutation_and_wrong_length_has_identical_legacy_diagnostics() {
    let good = wire(&[0x61; 32]);
    for index in 0..BYTES {
        let mut bytes = good;
        bytes[index] ^= 1;
        assert_framing(&bytes, Legacy::decode(&bytes).unwrap_err());
    }
    for len in [0, 1, 183] {
        assert_framing(&good[..len], Framing::Length);
    }
    let mut extended = good.to_vec();
    extended.push(0);
    assert_framing(&extended, Framing::Length);
}

#[test]
fn independently_resealed_relationships_and_nested_errors_preserve_order() {
    let good = wire(&[0x61; 32]);
    for index in [10, 11, 16, 17, 18, 19, 20, 21, 22, 23, 36, 37, 38, 39] {
        let mut bytes = good;
        bytes[index] = 1;
        reseal(&mut bytes);
        assert_framing(&bytes, Framing::Reserved);
    }
    for (offset, value, expected) in [
        (24, 0u32, Framing::SubmitterPid),
        (24, 200, Framing::SubmitterIsClient),
        (28, 999, Framing::CredentialMismatch),
        (32, 999, Framing::CredentialMismatch),
    ] {
        let mut bytes = good;
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        reseal(&mut bytes);
        assert_framing(&bytes, expected);
    }
    // Each earlier check must win even when later fields and the footer are bad.
    let cases = [
        (vec![(0, 0), (8, 0)], Framing::Magic),
        (vec![(8, 0), (10, 1)], Framing::Version),
        (vec![(10, 1), (12, 0)], Framing::Reserved),
        (vec![(12, 0), (36, 1)], Framing::Length),
        (vec![(36, 1), (24, 0)], Framing::Reserved),
        (vec![(24, 0), (40, 0)], Framing::SubmitterPid),
        (
            vec![(24, 200), (40, 0)],
            Framing::LaunchManifest(LaunchFraming::Magic),
        ),
        (vec![(24, 200), (28, 0)], Framing::SubmitterIsClient),
        (vec![(28, 0), (152, 0)], Framing::CredentialMismatch),
    ];
    for (changes, expected) in cases {
        let mut bytes = good;
        for (offset, value) in changes {
            bytes[offset] = value;
        }
        bytes[183] ^= 1;
        assert_framing(&bytes, expected);
    }
    let mut nested_identity = good;
    nested_identity[24..28].copy_from_slice(&200u32.to_le_bytes());
    nested_identity[120] ^= 1;
    reseal(&mut nested_identity);
    assert_framing(
        &nested_identity,
        Framing::LaunchManifest(LaunchFraming::Identity),
    );
    let mut nested_policy = good;
    nested_policy[88..120].fill(0);
    rehash(
        &mut nested_policy[40..152],
        80,
        b"FE2O3/COMPILER-EXECUTION-SERVICE-LAUNCH-MANIFEST/V1\0",
    );
    reseal(&mut nested_policy);
    assert_framing(
        &nested_policy,
        Framing::LaunchManifest(LaunchFraming::PolicyIdentity),
    );
}

#[test]
fn parent_and_nested_launch_changes_bind_the_handoff_identity() {
    let good = wire(&[0x61; 32]);
    let (native, _) = decode(&good).unwrap();
    for change_launch in [false, true] {
        let mut changed = good;
        if change_launch {
            changed[40..152].copy_from_slice(&launch_wire(&[0x62; 32]));
        } else {
            changed[24..28].copy_from_slice(&101u32.to_le_bytes());
        }
        assert_framing(&changed, Framing::Identity);
        reseal(&mut changed);
        let (other, _) = decode(&changed).unwrap();
        assert_ne!(native.identity(), other.identity());
        assert!(!native.identity().matches_canonical_bytes(&changed));
    }
}

#[test]
fn construction_and_decode_exact_short_work_storage_floor_and_entry() {
    let bytes = wire(&[0x61; 32]);
    for constructing in [false, true] {
        for mode in 0..5 {
            let launch = launch();
            let input = if constructing {
                launch.retained_storage()
            } else {
                BYTES
            };
            let floor = if mode == 3 { input - 1 } else { input + EXTRA };
            let work_limit = match mode {
                1 => WORK - 1,
                4 => 7,
                _ => WORK,
            };
            let limit = floor + STORAGE - usize::from(mode == 2);
            let mut w = Work::new(work_limit);
            {
                let mut b = Budget::new(&mut w, limit);
                b.reserve_storage(floor).unwrap();
                let result = if constructing {
                    Handoff::new(client(100), launch, &mut b)
                } else {
                    Handoff::decode(&bytes, &mut b)
                };
                assert_eq!(b.storage(), floor);
                match mode {
                    0 => {
                        let (owner, charge) = result.unwrap();
                        assert_eq!(owner.canonical_bytes(), &bytes);
                        assert_eq!(b.work(), WORK);
                        assert_eq!(b.peak_storage(), floor + STORAGE);
                        assert_eq!(
                            charge.additional_storage(),
                            owner.retained_storage() - if constructing { input } else { 0 }
                        );
                        b.reserve_storage(charge.additional_storage()).unwrap();
                        assert_eq!(
                            b.storage(),
                            EXTRA + owner.retained_storage() + if constructing { 0 } else { BYTES }
                        );
                    }
                    1 | 4 => {
                        assert!(matches!(result, Err(Error::Resource(Resource::Work(_)))));
                        assert_eq!(b.work(), if mode == 1 { 8 } else { 0 });
                        assert_eq!(b.peak_storage(), floor);
                    }
                    2 => {
                        assert!(matches!(result, Err(Error::Resource(Resource::Storage(_)))));
                        assert_eq!(b.work(), WORK);
                        assert_eq!(b.failed_storage(), Some(floor + STORAGE));
                        assert_eq!(b.peak_storage(), floor);
                    }
                    3 => {
                        assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
                        assert_eq!(b.work(), 8);
                        assert_eq!(b.peak_storage(), floor);
                    }
                    _ => unreachable!(),
                }
            }
            assert_eq!(
                w.failed_work(),
                match mode {
                    1 => Some(WORK),
                    4 => Some(8),
                    _ => None,
                }
            );
        }
    }
}

#[test]
fn semantic_constructor_errors_consume_launch_but_preserve_prepaid_floor() {
    for (submitter, expected) in [
        (
            Client::new(200, 999, 999).unwrap(),
            Framing::SubmitterIsClient,
        ),
        (
            Client::new(100, 999, 1001).unwrap(),
            Framing::CredentialMismatch,
        ),
        (
            Client::new(100, 1000, 999).unwrap(),
            Framing::CredentialMismatch,
        ),
    ] {
        assert_eq!(
            Legacy::new(
                submitter,
                LegacyLaunch::decode(&launch_wire(&[0x61; 32])).unwrap()
            )
            .unwrap_err(),
            expected
        );
        let launch = launch();
        let inherited = launch.retained_storage();
        let mut w = Work::new(WORK);
        let mut b = Budget::new(&mut w, inherited + EXTRA + STORAGE);
        b.reserve_storage(inherited + EXTRA).unwrap();
        assert!(
            matches!(Handoff::new(submitter, launch, &mut b), Err(Error::Framing(e)) if e == expected)
        );
        assert_eq!(b.storage(), inherited + EXTRA);
        assert_eq!(b.work(), WORK);
        b.release_storage(inherited).unwrap();
        assert_eq!(b.storage(), EXTRA);
    }
}

#[test]
fn malformed_lengths_need_no_input_floor_but_still_pay_complete_quota() {
    for bytes in [&[][..], &[0u8; BYTES - 1][..], &[0u8; BYTES + 1][..]] {
        for short in [false, true] {
            let mut w = Work::new(WORK);
            let mut b = Budget::new(&mut w, STORAGE - usize::from(short));
            let result = Handoff::decode(bytes, &mut b);
            if short {
                assert!(matches!(result, Err(Error::Resource(Resource::Storage(_)))));
            } else {
                assert!(matches!(result, Err(Error::Framing(Framing::Length))));
            }
            assert_eq!(b.storage(), 0);
            assert_eq!(b.work(), WORK);
        }
    }
    let mut w = Work::new(WORK);
    let mut b = Budget::new(&mut w, STORAGE);
    assert!(matches!(
        Handoff::decode(&[0; BYTES], &mut b),
        Err(Error::Resource(Resource::Accounting))
    ));
    assert_eq!(b.work(), 8);
    assert_eq!(b.peak_storage(), 0);
}

#[test]
fn same_ledger_native_chain_and_legacy_wire_never_admit_a_native_policy() {
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    b.reserve_storage(EXTRA).unwrap();
    let retained = {
        let (policy, charge) = Policy::new(
            7,
            Measurement::new([0x61; 32], 12345).unwrap(),
            Measurement::new([0x62; 32], 67890).unwrap(),
            SigningKey::from_bytes(&[0x51; 32])
                .verifying_key()
                .to_bytes(),
            SigningKey::from_bytes(&[0x52; 32])
                .verifying_key()
                .to_bytes(),
            &mut b,
        )
        .unwrap();
        b.reserve_storage(charge.additional_storage()).unwrap();
        let (launch, charge) = Launch::new(
            client(200),
            Service::new(6000, 7000).unwrap(),
            &policy,
            &mut b,
        )
        .unwrap();
        b.reserve_storage(charge.additional_storage()).unwrap();
        let inherited = launch.retained_storage();
        let floor = b.storage();
        let (handoff, growth) = Handoff::new(client(100), launch, &mut b).unwrap();
        assert_eq!(b.storage(), floor);
        assert_eq!(
            inherited + growth.additional_storage(),
            handoff.retained_storage()
        );
        b.reserve_storage(growth.additional_storage()).unwrap();
        assert!(
            handoff
                .launch_manifest()
                .matches_policy(&policy, &mut b)
                .unwrap()
        );
        b.reserve_storage(BYTES).unwrap();
        let bytes = *handoff.canonical_bytes();
        assert_eq!(bytes, wire(policy.identity().as_bytes()));
        let floor = b.storage();
        let (decoded, charge) = Handoff::decode(&bytes, &mut b).unwrap();
        assert_eq!(b.storage(), floor);
        b.reserve_storage(charge.additional_storage()).unwrap();
        assert_eq!(decoded, handoff);
        let legacy_policy = LegacyPolicy::new(
            7,
            policy.executable(),
            policy.runtime(),
            *policy.verifying_key(),
            *policy.external_anchor_verifying_key(),
        )
        .unwrap();
        b.reserve_storage(BYTES).unwrap();
        let legacy_wire = wire(legacy_policy.identity().as_bytes());
        let (legacy_bound, charge) = Handoff::decode(&legacy_wire, &mut b).unwrap();
        b.reserve_storage(charge.additional_storage()).unwrap();
        assert!(
            !legacy_bound
                .launch_manifest()
                .matches_policy(&policy, &mut b)
                .unwrap()
        );
        assert_ne!(handoff.identity(), legacy_bound.identity());
        assert_eq!(b.work(), POLICY_WORK + 3 * LAUNCH_WORK + 3 * WORK);
        let work = b.work();
        let floor = b.storage();
        for _ in 0..10 {
            assert_eq!(handoff.submitter(), client(100));
            assert_eq!(handoff.launch_manifest().client(), client(200));
            assert_eq!(handoff.canonical_bytes(), &bytes);
            assert_eq!(handoff.identity(), decoded.identity());
        }
        assert_eq!((b.work(), b.storage()), (work, floor));
        b.storage() - EXTRA
    };
    b.release_storage(retained).unwrap();
    assert_eq!(b.storage(), EXTRA);
}

#[test]
fn cumulative_work_and_sticky_denials_survive_success_and_semantic_refusal() {
    let bytes = wire(&[0x61; 32]);
    let total_work = LAUNCH_WORK + 3 * WORK;
    let mut w = Work::new(total_work);
    {
        let mut b = Budget::new(&mut w, LIMIT);
        assert!(b.charge_work(total_work + 1).is_err());
        assert!(b.reserve_storage(LIMIT + 1).is_err());
        b.reserve_storage(EXTRA + BYTES + LAUNCH_BYTES).unwrap();
        let retained = {
            let (launch, charge) = Launch::decode(&bytes[40..152], &mut b).unwrap();
            b.reserve_storage(charge.additional_storage()).unwrap();
            let (owner, charge) = Handoff::new(client(100), launch, &mut b).unwrap();
            b.reserve_storage(charge.additional_storage()).unwrap();
            let (decoded, charge) = Handoff::decode(&bytes, &mut b).unwrap();
            b.reserve_storage(charge.additional_storage()).unwrap();
            assert_eq!(owner, decoded);
            let floor = b.storage();
            // A wrong-length failure does not need another input reservation.
            assert!(matches!(
                Handoff::decode(&[], &mut b),
                Err(Error::Framing(Framing::Length))
            ));
            assert_eq!(b.storage(), floor);
            assert_eq!(b.work(), total_work);
            assert_eq!(b.failed_storage(), Some(LIMIT + 1));
            assert!(b.reserve_storage(LIMIT + 2).is_err());
            assert!(matches!(
                Handoff::decode(&bytes, &mut b),
                Err(Error::Resource(Resource::Work(_)))
            ));
            assert_eq!(b.failed_storage(), Some(LIMIT + 1));
            b.storage() - EXTRA
        };
        b.release_storage(retained).unwrap();
        assert_eq!(b.storage(), EXTRA);
    }
    assert_eq!(w.work(), total_work);
    assert_eq!(w.failed_work(), Some(total_work + 1));
}
