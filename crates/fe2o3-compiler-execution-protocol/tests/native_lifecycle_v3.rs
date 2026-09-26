use ed25519_dalek::SigningKey;
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_CLIENT_PROFILE_WORK_V3 as PROFILE_WORK,
    COMPILER_EXECUTION_ISSUER_POLICY_WORK_V3 as POLICY_WORK,
    COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_BYTES_V3 as LAUNCH_BYTES,
    COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_STORAGE_V3 as LAUNCH_STORAGE,
    COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_WORK_V3 as LAUNCH_WORK,
    COMPILER_EXECUTION_SERVICE_READY_BYTES_V3 as READY_BYTES,
    COMPILER_EXECUTION_SERVICE_READY_STORAGE_V3 as READY_STORAGE,
    COMPILER_EXECUTION_SERVICE_READY_WORK_V3 as READY_WORK,
    COMPILER_EXECUTION_SUPERVISOR_HANDOFF_BYTES_V3 as HANDOFF_BYTES,
    COMPILER_EXECUTION_SUPERVISOR_HANDOFF_STORAGE_V3 as HANDOFF_STORAGE,
    COMPILER_EXECUTION_SUPERVISOR_HANDOFF_WORK_V3 as HANDOFF_WORK,
    CompilerExecutionAttestationStorageV3 as Storage,
    CompilerExecutionClientProcessIdentityV1 as Client,
    CompilerExecutionClientProfileV3 as Profile,
    CompilerExecutionExternalAnchorServiceIdentityV1 as Service,
    CompilerExecutionIssuerMeasurementV1 as Measurement,
    CompilerExecutionIssuerPolicyV1 as PolicyV1, CompilerExecutionIssuerPolicyV2 as PolicyV2,
    CompilerExecutionIssuerPolicyV3 as Policy,
    CompilerExecutionServiceLaunchManifestErrorV1 as LaunchFraming,
    CompilerExecutionServiceLaunchManifestErrorV3 as LaunchError,
    CompilerExecutionServiceLaunchManifestV1 as LaunchV1,
    CompilerExecutionServiceLaunchManifestV2 as LaunchV2,
    CompilerExecutionServiceLaunchManifestV3 as Launch,
    CompilerExecutionServiceReadyErrorV1 as ReadyFraming,
    CompilerExecutionServiceReadyErrorV3 as ReadyError, CompilerExecutionServiceReadyV1 as ReadyV1,
    CompilerExecutionServiceReadyV2 as ReadyV2, CompilerExecutionServiceReadyV3 as Ready,
    CompilerExecutionSupervisorHandoffErrorV1 as HandoffFraming,
    CompilerExecutionSupervisorHandoffErrorV3 as HandoffError,
    CompilerExecutionSupervisorHandoffV1 as HandoffV1,
    CompilerExecutionSupervisorHandoffV2 as HandoffV2,
    CompilerExecutionSupervisorHandoffV3 as Handoff,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work,
};
use sha2::{Digest, Sha256};
use std::mem::size_of;

const LIMIT: usize = 1_000_000;
const PID: u32 = 5678;

fn client(pid: u32) -> Client {
    Client::new(pid, 1000, 1001).unwrap()
}
fn profile(generation: u64, budget: &mut Budget<'_>) -> Profile {
    let (policy, charge) = Policy::new(
        generation,
        Measurement::new([0x61; 32], 12345).unwrap(),
        Measurement::new([0x62; 32], 67890).unwrap(),
        SigningKey::from_bytes(&[0x51; 32])
            .verifying_key()
            .to_bytes(),
        SigningKey::from_bytes(&[0x52; 32])
            .verifying_key()
            .to_bytes(),
        budget,
    )
    .unwrap();
    budget.reserve_storage(charge.additional_storage()).unwrap();
    let (profile, delta) = Profile::new(
        1000,
        1001,
        Service::new(6000, 7000).unwrap(),
        policy,
        budget,
    )
    .unwrap();
    budget.reserve_storage(delta.additional_storage()).unwrap();
    profile
}
fn launch(profile: &Profile, budget: &mut Budget<'_>) -> Launch {
    let (launch, charge) = Launch::new(
        client(200),
        profile.external_anchor_service(),
        profile.policy(),
        budget,
    )
    .unwrap();
    budget.reserve_storage(charge.additional_storage()).unwrap();
    launch
}
fn rehash(bytes: &mut [u8], prefix: usize, domain: &[u8]) {
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update((prefix as u64).to_le_bytes());
    hash.update(&bytes[..prefix]);
    bytes[prefix..].copy_from_slice(&hash.finalize());
}

// Independent V1 wire transcripts carry actual V3 identities without relabeling.
fn launch_wire(policy: &[u8; 32]) -> [u8; 112] {
    let mut bytes = [0; 112];
    bytes[..8].copy_from_slice(b"F2O3CEL1");
    bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
    for (offset, value) in [
        (12, 112_u32),
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
fn ready_wire(launch: &[u8; 32], policy: &[u8; 32]) -> [u8; 120] {
    let mut bytes = [0; 120];
    bytes[..8].copy_from_slice(b"F2O3CER1");
    bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
    bytes[12..16].copy_from_slice(&120_u32.to_le_bytes());
    bytes[16..20].copy_from_slice(&PID.to_le_bytes());
    bytes[24..56].copy_from_slice(launch);
    bytes[56..88].copy_from_slice(policy);
    rehash(
        &mut bytes,
        88,
        b"FE2O3/COMPILER-EXECUTION-SERVICE-READY/V1\0",
    );
    bytes
}
fn handoff_wire(policy: &[u8; 32]) -> [u8; 184] {
    let mut bytes = [0; 184];
    bytes[..8].copy_from_slice(b"F2O3CEH1");
    bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
    for (offset, value) in [(12, 184_u32), (24, 100), (28, 1000), (32, 1001)] {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    bytes[40..152].copy_from_slice(&launch_wire(policy));
    rehash(
        &mut bytes,
        152,
        b"FE2O3/COMPILER-EXECUTION-SUPERVISOR-HANDOFF/V1\0",
    );
    bytes
}

#[test]
fn actual_v3_profile_launch_ready_handoff_chain_keeps_original_ledger() {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(19).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let profile = profile(7, &mut budget);
    let launch = launch(&profile, &mut budget);
    let (ready, full) = Ready::new(PID, &launch, profile.policy(), &mut budget).unwrap();
    assert_eq!(full.additional_storage(), ready.retained_storage());
    budget.reserve_storage(full.additional_storage()).unwrap();
    assert!(
        ready
            .matches_launch(PID, &launch, profile.policy(), &mut budget)
            .unwrap()
    );
    assert_eq!(
        launch.canonical_bytes(),
        &launch_wire(profile.policy().identity().as_bytes())
    );
    assert_eq!(
        ready.canonical_bytes(),
        &ready_wire(
            launch.identity().as_bytes(),
            profile.policy().identity().as_bytes()
        )
    );
    let inherited = launch.retained_storage();
    let floor = budget.storage();
    let (handoff, delta) = Handoff::new(client(100), launch, &mut budget).unwrap();
    assert_eq!(budget.storage(), floor);
    assert_eq!(
        inherited + delta.additional_storage(),
        handoff.retained_storage()
    );
    budget.reserve_storage(delta.additional_storage()).unwrap();
    let nested: &Launch = handoff.launch_manifest();
    assert_eq!(
        handoff.canonical_bytes(),
        &handoff_wire(profile.policy().identity().as_bytes())
    );
    assert!(
        nested
            .matches_policy(profile.policy(), &mut budget)
            .unwrap()
    );
    assert_eq!(nested.policy_identity(), profile.policy().identity());
    assert_eq!(
        nested.external_anchor_service(),
        profile.external_anchor_service()
    );
    let (decoded_launch, charge): (Launch, Storage) =
        Launch::decode(nested.canonical_bytes(), &mut budget).unwrap();
    assert_eq!(&decoded_launch, nested);
    assert_eq!(
        charge.additional_storage(),
        decoded_launch.retained_storage()
    );
    budget.reserve_storage(charge.additional_storage()).unwrap();
    let (decoded_ready, charge): (Ready, Storage) =
        Ready::decode(ready.canonical_bytes(), &mut budget).unwrap();
    assert_eq!(decoded_ready, ready);
    budget.reserve_storage(charge.additional_storage()).unwrap();
    let (decoded_handoff, charge): (Handoff, Storage) =
        Handoff::decode(handoff.canonical_bytes(), &mut budget).unwrap();
    assert_eq!(decoded_handoff, handoff);
    assert_eq!(charge.additional_storage(), handoff.retained_storage());
    budget.reserve_storage(charge.additional_storage()).unwrap();
    assert!(
        decoded_ready
            .matches_launch(
                PID,
                decoded_handoff.launch_manifest(),
                profile.policy(),
                &mut budget
            )
            .unwrap()
    );
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert_eq!(
        budget.work(),
        POLICY_WORK + PROFILE_WORK + 3 * LAUNCH_WORK + 4 * READY_WORK + 2 * HANDOFF_WORK
    );
    let retained = budget.storage() - 19;
    drop((
        decoded_launch,
        decoded_ready,
        decoded_handoff,
        ready,
        handoff,
        profile,
    ));
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), 19);
}

#[test]
fn identity_only_decoding_never_admits_foreign_policy_or_readiness() {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let profile = profile(7, &mut budget);
    let p = profile.policy();
    let p1 = PolicyV1::new(
        p.generation(),
        p.executable(),
        p.runtime(),
        *p.verifying_key(),
        *p.external_anchor_verifying_key(),
    )
    .unwrap();
    let (p2, charge) = PolicyV2::new(
        p.generation(),
        p.executable(),
        p.runtime(),
        *p.verifying_key(),
        *p.external_anchor_verifying_key(),
        &mut budget,
    )
    .unwrap();
    budget.reserve_storage(charge.additional_storage()).unwrap();
    let l1 = LaunchV1::new(client(200), profile.external_anchor_service(), &p1);
    let r1 = ReadyV1::new(PID, &l1, &p1).unwrap();
    let h1 = HandoffV1::new(client(100), l1).unwrap();
    budget
        .reserve_storage(size_of::<HandoffV1>() + size_of::<ReadyV1>())
        .unwrap();
    let (l2, charge) = LaunchV2::new(
        client(200),
        profile.external_anchor_service(),
        &p2,
        &mut budget,
    )
    .unwrap();
    budget.reserve_storage(charge.additional_storage()).unwrap();
    let (r2, charge) = ReadyV2::new(PID, &l2, &p2, &mut budget).unwrap();
    budget.reserve_storage(charge.additional_storage()).unwrap();
    let (h2, delta) = HandoffV2::new(client(100), l2, &mut budget).unwrap();
    budget.reserve_storage(delta.additional_storage()).unwrap();
    let native = launch(&profile, &mut budget);
    for (l, r, h) in [
        (
            h1.launch_manifest().canonical_bytes(),
            r1.canonical_bytes(),
            h1.canonical_bytes(),
        ),
        (
            h2.launch_manifest().canonical_bytes(),
            r2.canonical_bytes(),
            h2.canonical_bytes(),
        ),
    ] {
        let (opaque_launch, charge) = Launch::decode(l, &mut budget).unwrap();
        budget.reserve_storage(charge.additional_storage()).unwrap();
        let (opaque_ready, charge) = Ready::decode(r, &mut budget).unwrap();
        budget.reserve_storage(charge.additional_storage()).unwrap();
        let (opaque_handoff, charge) = Handoff::decode(h, &mut budget).unwrap();
        budget.reserve_storage(charge.additional_storage()).unwrap();
        assert_eq!(opaque_launch.canonical_bytes(), l);
        assert_eq!(opaque_ready.canonical_bytes(), r);
        assert_eq!(opaque_handoff.canonical_bytes(), h);
        assert!(!opaque_launch.matches_policy(p, &mut budget).unwrap());
        assert!(
            !opaque_handoff
                .launch_manifest()
                .matches_policy(p, &mut budget)
                .unwrap()
        );
        assert!(
            !opaque_ready
                .matches_launch(PID, &opaque_launch, p, &mut budget)
                .unwrap()
        );
        assert!(
            !opaque_ready
                .matches_launch(PID, &native, p, &mut budget)
                .unwrap()
        );
        assert!(matches!(
            Ready::new(PID, &opaque_launch, p, &mut budget),
            Err(ReadyError::Framing(ReadyFraming::PolicyMismatch))
        ));
        assert!(matches!(
            Ready::new(0, &opaque_launch, p, &mut budget),
            Err(ReadyError::Framing(ReadyFraming::IssuerPid))
        ));
        let retained = opaque_launch.retained_storage()
            + opaque_ready.retained_storage()
            + opaque_handoff.retained_storage();
        drop((opaque_launch, opaque_ready, opaque_handoff));
        budget.release_storage(retained).unwrap();
    }
    let (v2_view, charge) = LaunchV2::decode(native.canonical_bytes(), &mut budget).unwrap();
    budget.reserve_storage(charge.additional_storage()).unwrap();
    assert!(!v2_view.matches_policy(&p2, &mut budget).unwrap());
    assert!(
        !LaunchV1::decode(native.canonical_bytes())
            .unwrap()
            .matches_policy(&p1)
    );
}

#[test]
fn ready_requires_the_exact_pid_manifest_and_its_actual_v3_policy_binding() {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let first = profile(7, &mut budget);
    let other = profile(8, &mut budget);
    let l = launch(&first, &mut budget);
    let other_launch = launch(&other, &mut budget);
    let (r, charge) = Ready::new(PID, &l, first.policy(), &mut budget).unwrap();
    budget.reserve_storage(charge.additional_storage()).unwrap();
    for (pid, manifest, policy) in [
        (PID + 1, &l, first.policy()),
        (PID, &other_launch, first.policy()),
        (PID, &l, other.policy()),
        (PID, &other_launch, other.policy()),
    ] {
        let floor = budget.storage();
        let work = budget.work();
        assert!(
            !r.matches_launch(pid, manifest, policy, &mut budget)
                .unwrap()
        );
        assert_eq!(
            (budget.storage(), budget.work()),
            (floor, work + READY_WORK)
        );
    }
    budget.reserve_storage(READY_BYTES).unwrap();
    let inconsistent = ready_wire(
        other_launch.identity().as_bytes(),
        first.policy().identity().as_bytes(),
    );
    let (r, charge) = Ready::decode(&inconsistent, &mut budget).unwrap();
    budget.reserve_storage(charge.additional_storage()).unwrap();
    assert_eq!(r.launch_manifest_identity(), other_launch.identity());
    assert_eq!(r.policy_identity(), first.policy().identity());
    assert!(
        !r.matches_launch(PID, &other_launch, first.policy(), &mut budget)
            .unwrap()
    );
}

#[test]
fn all_three_v3_decoders_preserve_v1_wire_diagnostics_and_v2_quotas() {
    macro_rules! check {
        ($Owner:ident, $Legacy:ident, $Error:ident, $good:expr, $work:expr, $storage:expr) => {{
            let good = $good;
            let mut work = Work::new((2 * good.len() + 1) * $work);
            let mut budget = Budget::new(&mut work, good.len() + $storage);
            budget.reserve_storage(good.len()).unwrap();
            for offset in 0..good.len() {
                let mut bytes = good;
                bytes[offset] ^= 1;
                let expected = $Legacy::decode(&bytes).unwrap_err();
                assert!(matches!($Owner::decode(&bytes, &mut budget), Err($Error::Framing(e)) if e == expected));
                assert_eq!(budget.storage(), good.len());
            }
            for length in 0..good.len() {
                let expected = $Legacy::decode(&good[..length]).unwrap_err();
                assert!(matches!($Owner::decode(&good[..length], &mut budget), Err($Error::Framing(e)) if e == expected));
            }
            let mut extended = good.to_vec();
            extended.push(0);
            let expected = $Legacy::decode(&extended).unwrap_err();
            assert!(matches!($Owner::decode(&extended, &mut budget), Err($Error::Framing(e)) if e == expected));
            assert_eq!(budget.storage(), good.len());
        }};
    }
    check!(
        Launch,
        LaunchV1,
        LaunchError,
        launch_wire(&[0x61; 32]),
        LAUNCH_WORK,
        LAUNCH_STORAGE
    );
    check!(
        Ready,
        ReadyV1,
        ReadyError,
        ready_wire(&[0x61; 32], &[0x62; 32]),
        READY_WORK,
        READY_STORAGE
    );
    check!(
        Handoff,
        HandoffV1,
        HandoffError,
        handoff_wire(&[0x61; 32]),
        HANDOFF_WORK,
        HANDOFF_STORAGE
    );
    use fe2o3_compiler_execution_protocol::*;
    assert_eq!((LAUNCH_BYTES, READY_BYTES, HANDOFF_BYTES), (112, 120, 184));
    assert_eq!((LAUNCH_WORK, READY_WORK, HANDOFF_WORK), (3592, 3848, 9480));
    assert_eq!(
        LAUNCH_WORK,
        COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_WORK_V2
    );
    assert_eq!(READY_WORK, COMPILER_EXECUTION_SERVICE_READY_WORK_V2);
    assert_eq!(HANDOFF_WORK, COMPILER_EXECUTION_SUPERVISOR_HANDOFF_WORK_V2);
    assert_eq!(
        LAUNCH_STORAGE,
        COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_STORAGE_V2
    );
    assert_eq!(READY_STORAGE, COMPILER_EXECUTION_SERVICE_READY_STORAGE_V2);
    assert_eq!(
        HANDOFF_STORAGE,
        COMPILER_EXECUTION_SUPERVISOR_HANDOFF_STORAGE_V2
    );
}

#[test]
fn underpaid_decode_never_reaches_framing_and_preserves_denial_history() {
    macro_rules! check {
        ($Owner:ident, $Error:ident, $bytes:expr, $work:expr, $storage:expr) => {
            for case in 0..5 {
                let floor = $bytes - usize::from(case == 1);
                let work_limit = match case {
                    0 => 7,
                    2 => $work - 1,
                    _ => $work,
                };
                let limit = floor + $storage - usize::from(case == 3);
                let mut work = Work::new(work_limit);
                {
                    let mut budget = Budget::new(&mut work, limit);
                    budget.reserve_storage(floor).unwrap();
                    assert!(budget.reserve_storage(limit + 1).is_err());
                    let ledger = budget.work_ledger_identity_v1();
                    let result = $Owner::decode(&[0; $bytes], &mut budget);
                    assert!(budget.work_ledger_identity_v1() == ledger);
                    assert_eq!(budget.storage(), floor);
                    assert_eq!(budget.failed_storage(), Some(floor + limit + 1));
                    assert_eq!(
                        budget.work(),
                        match case {
                            0 => 0,
                            1 | 2 => 8,
                            _ => $work,
                        }
                    );
                    assert_eq!(
                        budget.peak_storage(),
                        floor + if case == 4 { $storage } else { 0 }
                    );
                    match case {
                        0 | 2 => {
                            assert!(matches!(result, Err($Error::Resource(Resource::Work(_)))))
                        }
                        1 => assert!(matches!(
                            result,
                            Err($Error::Resource(Resource::Accounting))
                        )),
                        3 => assert!(matches!(
                            result,
                            Err($Error::Resource(Resource::Storage(_)))
                        )),
                        _ => assert!(matches!(result, Err($Error::Framing(_)))),
                    }
                }
                assert_eq!(
                    work.failed_work(),
                    match case {
                        0 => Some(8),
                        2 => Some($work),
                        _ => None,
                    }
                );
            }
        };
    }
    check!(
        Launch,
        LaunchError,
        LAUNCH_BYTES,
        LAUNCH_WORK,
        LAUNCH_STORAGE
    );
    check!(Ready, ReadyError, READY_BYTES, READY_WORK, READY_STORAGE);
    check!(
        Handoff,
        HandoffError,
        HANDOFF_BYTES,
        HANDOFF_WORK,
        HANDOFF_STORAGE
    );
}

#[test]
fn handoff_nested_framing_precedes_relationship_and_footer_errors() {
    let mut bytes = handoff_wire(&[0x61; 32]);
    bytes[24..28].copy_from_slice(&200_u32.to_le_bytes());
    bytes[40] ^= 1;
    bytes[152..].fill(0);
    let mut work = Work::new(3 * HANDOFF_WORK);
    let mut budget = Budget::new(&mut work, HANDOFF_BYTES + HANDOFF_STORAGE);
    budget.reserve_storage(HANDOFF_BYTES).unwrap();
    assert!(matches!(
        Handoff::decode(&bytes, &mut budget),
        Err(HandoffError::Framing(HandoffFraming::LaunchManifest(
            LaunchFraming::Magic
        )))
    ));
    bytes[40] ^= 1;
    assert!(matches!(
        Handoff::decode(&bytes, &mut budget),
        Err(HandoffError::Framing(HandoffFraming::SubmitterIsClient))
    ));
    bytes[24..28].copy_from_slice(&100_u32.to_le_bytes());
    assert!(matches!(
        Handoff::decode(&bytes, &mut budget),
        Err(HandoffError::Framing(HandoffFraming::Identity))
    ));
    assert_eq!(budget.storage(), HANDOFF_BYTES);
}

fn boundaries(
    input: usize,
    quota: usize,
    scratch: usize,
    mut operation: impl FnMut(&mut Budget<'_>) -> Result<(), Resource>,
) {
    for case in 0..6 {
        let floor = match case {
            1 => input - 1,
            5 => input + 19,
            _ => input,
        };
        let limit = floor + scratch - usize::from(case == 3);
        let mut work = Work::new(match case {
            0 => 7,
            2 => quota - 1,
            _ => quota,
        });
        let mut budget = Budget::new(&mut work, limit);
        budget.reserve_storage(floor).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = operation(&mut budget);
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!(budget.storage(), floor);
        assert_eq!(
            budget.work(),
            match case {
                0 => 0,
                1 | 2 => 8,
                _ => quota,
            }
        );
        assert_eq!(
            budget.peak_storage(),
            floor + if case >= 4 { scratch } else { 0 }
        );
        match case {
            0 | 2 => assert!(matches!(result, Err(Resource::Work(_)))),
            1 => assert!(matches!(result, Err(Resource::Accounting))),
            3 => {
                assert!(matches!(result, Err(Resource::Storage(_))));
                assert_eq!(budget.failed_storage(), Some(floor + scratch));
            }
            _ => result.unwrap(),
        }
    }
}

#[test]
fn constructors_and_typed_joins_preflight_complete_borrowed_or_consumed_floors() {
    macro_rules! resource {
        ($Error:ident, $result:expr) => {
            $result.map_err(|error| match error {
                $Error::Resource(error) => error,
                error => panic!("unexpected framing refusal: {error:?}"),
            })
        };
    }
    // Fixtures are admitted independently; each isolated operation must use the
    // supplied bounded ledger, including when entry, owner floor or scratch fails.
    let mut work = Work::new(LIMIT);
    let mut fixtures = Budget::new(&mut work, LIMIT);
    let profile = profile(7, &mut fixtures);
    let p = profile.policy();
    let l = launch(&profile, &mut fixtures);
    let (r, charge) = Ready::new(PID, &l, p, &mut fixtures).unwrap();
    fixtures
        .reserve_storage(charge.additional_storage())
        .unwrap();
    boundaries(p.retained_storage(), LAUNCH_WORK, LAUNCH_STORAGE, |b| {
        resource!(
            LaunchError,
            Launch::new(client(200), profile.external_anchor_service(), p, b)
        )
        .map(|(owner, charge)| assert_eq!(charge.additional_storage(), owner.retained_storage()))
    });
    boundaries(
        l.retained_storage() + p.retained_storage(),
        LAUNCH_WORK,
        LAUNCH_STORAGE,
        |b| resource!(LaunchError, l.matches_policy(p, b)).map(|matched| assert!(matched)),
    );
    boundaries(
        l.retained_storage() + p.retained_storage(),
        READY_WORK,
        READY_STORAGE,
        |b| {
            resource!(ReadyError, Ready::new(PID, &l, p, b)).map(|(owner, charge)| {
                assert_eq!(charge.additional_storage(), owner.retained_storage())
            })
        },
    );
    boundaries(
        r.retained_storage() + l.retained_storage() + p.retained_storage(),
        READY_WORK,
        READY_STORAGE,
        |b| resource!(ReadyError, r.matches_launch(PID, &l, p, b)).map(|matched| assert!(matched)),
    );
    let inherited = l.retained_storage();
    boundaries(inherited, HANDOFF_WORK, HANDOFF_STORAGE, |b| {
        let source = launch(&profile, &mut fixtures);
        let result =
            resource!(HandoffError, Handoff::new(client(100), source, b)).map(|(owner, delta)| {
                assert_eq!(
                    inherited + delta.additional_storage(),
                    owner.retained_storage()
                )
            });
        fixtures.release_storage(inherited).unwrap();
        result
    });
}

#[test]
fn consuming_handoff_errors_keep_the_original_budget_and_launch_reservation() {
    for short_work in [false, true] {
        let setup = POLICY_WORK + PROFILE_WORK + LAUNCH_WORK;
        let mut work = Work::new(setup + HANDOFF_WORK - usize::from(short_work));
        let mut budget = Budget::new(&mut work, LIMIT);
        let profile = profile(7, &mut budget);
        let launch = launch(&profile, &mut budget);
        let inherited = launch.retained_storage();
        let floor = budget.storage();
        let ledger = budget.work_ledger_identity_v1();
        let result = Handoff::new(client(200), launch, &mut budget);
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!(budget.storage(), floor);
        if short_work {
            assert!(matches!(
                result,
                Err(HandoffError::Resource(Resource::Work(_)))
            ));
            assert_eq!(budget.work(), setup + 8);
        } else {
            assert!(matches!(
                result,
                Err(HandoffError::Framing(HandoffFraming::SubmitterIsClient))
            ));
            assert_eq!(budget.work(), setup + HANDOFF_WORK);
        }
        budget.release_storage(inherited).unwrap();
        assert_eq!(budget.storage(), profile.retained_storage());
    }
}
