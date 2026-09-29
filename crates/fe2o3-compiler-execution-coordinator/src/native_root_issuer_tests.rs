use super::*;
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionExternalAnchorServiceIdentityV1 as Service,
    CompilerExecutionIssuerMeasurementV1 as Measurement, CompilerExecutionIssuerPolicyV3 as Policy,
};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::fs::File;

mod process {
    include!("native_root_issuer_process_tests.rs");
}

const LIMIT: usize = 1 << 30;

#[test]
fn issuer_account_scope_preserves_original_charges_and_history() {
    let floor = 256;
    let prefix = 17;
    let mut work = Work::new(prefix + LOCAL_WORK);
    let mut b = Budget::new(&mut work, floor + FRAME);
    b.charge_work(prefix).unwrap();
    b.reserve_storage(floor).unwrap();
    assert!(b.reserve_storage(FRAME + 1).is_err());
    let denial = b.failed_storage();
    let account = RequestAccount::capture(&b);
    let mut accessed = false;
    account
        .with(floor, &mut b, |b| {
            accessed = true;
            assert!(b.work_ledger_identity_v1() == account.ledger);
            assert_eq!(b as *const Budget<'_> as usize, account.address);
            assert_eq!(b.storage(), floor + FRAME);
            Ok(())
        })
        .unwrap();
    assert!(accessed);
    assert_eq!(b.storage(), floor);
    assert_eq!(b.work(), prefix + LOCAL_WORK);
    assert_eq!(b.peak_storage(), floor + FRAME);
    assert_eq!(b.failed_storage(), denial);
    assert!(NativeAttempt::<()>::ENVELOPE >= size_of::<RequestAccount>());
    assert!(FRAME >= 4 * size_of::<NativeAttempt<'_, ()>>());
}

#[test]
fn attempt_storage_counts_each_owner_once_and_growth_above_both_consumed_inputs() {
    let prepared = 4096;
    let trace = 8192;
    let root = 512;
    let issuer = prepared + 1024;
    let retained = attempt_storage::<()>(trace, root, issuer).unwrap();
    assert_eq!(
        retained,
        trace + root + issuer + NativeAttempt::<()>::ENVELOPE
    );
    let growth = retained.checked_sub(prepared + trace).unwrap();
    assert_eq!(growth, root + 1024 + NativeAttempt::<()>::ENVELOPE);
    assert_eq!(prepared + trace + growth, retained);
    assert_eq!(
        NativeAttempt::<()>::ENVELOPE,
        size_of::<(NativeAttempt<'_, ()>, Storage)>()
            - size_of::<CompilerTrace<'_, ()>>()
            - size_of::<RootSession<'_>>()
            - size_of::<ManagedIssuer<'_, ()>>()
    );
    assert!(
        NativeAttempt::<()>::ENVELOPE
            >= size_of::<RequestAccount>()
                + size_of::<Quota>()
                + size_of::<usize>()
                + size_of::<Storage>()
    );
    assert_eq!(
        ManagedIssuer::<()>::ENVELOPE,
        size_of::<(ManagedIssuer<'_, ()>, Storage)>()
            - size_of::<Child<()>>()
            - size_of::<Ready>()
            - size_of::<RootConnection<'_>>()
    );
}

#[test]
fn attempt_storage_rejects_each_overflow_without_constructing_authority() {
    for (trace, root, issuer) in [
        (usize::MAX, 1, 1),
        (1, usize::MAX, 1),
        (1, 1, usize::MAX),
        (0, 0, usize::MAX - NativeAttempt::<()>::ENVELOPE + 1),
    ] {
        assert!(matches!(
            attempt_storage::<()>(trace, root, issuer),
            Err(Error::Resource(Resource::Arithmetic))
        ));
    }
}

#[test]
fn original_validation_quota_funds_attempt_scope_and_actual_trace_session_checks() {
    let quota = NativeAttempt::<()>::original_validation_quota();
    assert_eq!(
        quota.work(),
        LOCAL_WORK + CompilerTrace::<()>::OBSERVATION_WORK + RootSession::VALIDATE_WORK
    );
    assert_eq!(
        quota.scratch(),
        FRAME + CompilerTrace::<()>::OBSERVATION_SCRATCH + RootSession::VALIDATE_SCRATCH
    );
}

#[test]
fn issuer_account_scope_rejects_funded_foreign_ledger_before_access() {
    let floor = 256;
    let mut original_work = Work::new(LOCAL_WORK);
    let mut foreign_work = Work::new(LOCAL_WORK);
    let mut original = Budget::new(&mut original_work, floor + FRAME);
    let mut foreign = Budget::new(&mut foreign_work, floor + FRAME);
    original.reserve_storage(floor).unwrap();
    foreign.reserve_storage(floor).unwrap();
    let account = RequestAccount::capture(&original);
    let mut accessed = false;
    let result = account.with(floor, &mut foreign, |_| {
        accessed = true;
        Ok(())
    });
    assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
    assert!(!accessed);
    assert_eq!(original.work(), 0);
    assert_eq!(original.storage(), floor);
    assert_eq!(foreign.work(), LOCAL_WORK);
    assert_eq!(foreign.storage(), floor);
}

#[test]
fn issuer_account_scope_requires_both_original_work_and_budget_address() {
    let floor = 256;
    let mut first_work = Work::new(3 * LOCAL_WORK);
    let mut second_work = Work::new(3 * LOCAL_WORK);
    let mut first = Budget::new(&mut first_work, floor + FRAME);
    let mut second = Budget::new(&mut second_work, floor + FRAME);
    first.reserve_storage(floor).unwrap();
    second.reserve_storage(floor).unwrap();
    let account = RequestAccount::capture(&first);
    let other_address = &second as *const Budget<'_> as usize;
    assert_ne!(account.address, other_address);
    std::mem::swap(&mut first, &mut second);
    // Same budget address now contains a different (equally funded) Work ledger.
    assert_eq!(&first as *const Budget<'_> as usize, account.address);
    assert!(first.work_ledger_identity_v1() != account.ledger);
    // Original Work ledger is still live, but at another Budget address.
    assert_eq!(&second as *const Budget<'_> as usize, other_address);
    assert!(second.work_ledger_identity_v1() == account.ledger);
    for b in [&mut first, &mut second] {
        let result = account.with::<()>(floor, b, |_| panic!("foreign account reached issuer I/O"));
        assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
        assert_eq!(b.storage(), floor);
        assert_eq!(b.work(), LOCAL_WORK);
    }
    std::mem::swap(&mut first, &mut second);
    account.with(floor, &mut first, |_| Ok(())).unwrap();
    assert_eq!(first.work(), 2 * LOCAL_WORK);
    assert_eq!(first.storage(), floor);
}

// Inert records only: no fabricated Prepared, dependency, protected admission or
// successful provisioning. These tests cannot provide production launch evidence.
fn records() -> (Policy, Manifest, [u8; READY_BYTES]) {
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    let public = |seed| {
        ed25519_dalek::SigningKey::from_bytes(&[seed; 32])
            .verifying_key()
            .to_bytes()
    };
    let (policy, c) = Policy::new(
        7,
        Measurement::new([1; 32], 11).unwrap(),
        Measurement::new([2; 32], 12).unwrap(),
        public(7),
        public(9),
        &mut b,
    )
    .unwrap();
    b.reserve_storage(c.additional_storage()).unwrap();
    let (manifest, c) = Manifest::new(
        Client::new(42, 1000, 1000).unwrap(),
        Service::new(2000, 2000).unwrap(),
        &policy,
        &mut b,
    )
    .unwrap();
    b.reserve_storage(c.additional_storage()).unwrap();
    let (ready, _) = Ready::new(77, &manifest, &policy, &mut b).unwrap();
    (policy, manifest, *ready.canonical_bytes())
}

#[test]
fn issuer_records_require_exact_pid_manifest_and_pinned_policy() {
    let (policy, manifest, bytes) = records();
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    b.reserve_storage(policy.retained_storage() + manifest.retained_storage() + bytes.len())
        .unwrap();
    let (ready, c) = Ready::decode(&bytes, &mut b).unwrap();
    assert_eq!(c.additional_storage(), READY_OWNER);
    b.reserve_storage(c.additional_storage()).unwrap();
    match_ready_records(&ready, 77, &manifest, &policy, &mut b).unwrap();
    assert!(matches!(
        match_ready_records(&ready, 78, &manifest, &policy, &mut b),
        Err(Error::Invalid(_))
    ));
    for (client, service) in [
        (
            Client::new(43, 1000, 1000).unwrap(),
            manifest.external_anchor_service(),
        ),
        (manifest.client(), Service::new(2001, 2000).unwrap()),
    ] {
        let (other, c) = Manifest::new(client, service, &policy, &mut b).unwrap();
        b.reserve_storage(c.additional_storage()).unwrap();
        assert!(matches!(
            match_ready_records(&ready, 77, &other, &policy, &mut b),
            Err(Error::Invalid(_))
        ));
        drop(other);
        b.release_storage(c.additional_storage()).unwrap();
    }
    let (other_policy, c) = Policy::new(
        8,
        policy.executable(),
        policy.runtime(),
        ed25519_dalek::SigningKey::from_bytes(&[7; 32])
            .verifying_key()
            .to_bytes(),
        ed25519_dalek::SigningKey::from_bytes(&[9; 32])
            .verifying_key()
            .to_bytes(),
        &mut b,
    )
    .unwrap();
    b.reserve_storage(c.additional_storage()).unwrap();
    assert!(matches!(
        match_ready_records(&ready, 77, &manifest, &other_policy, &mut b),
        Err(Error::Invalid(_))
    ));
}

#[test]
fn issuer_ready_decode_accounting_and_short_inputs_stay_on_original_ledger() {
    let (_, _, bytes) = records();
    for mode in 0..4 {
        let floor = bytes.len() - usize::from(mode == 1);
        let mut work = Work::new(READY_WORK - usize::from(mode == 2));
        let mut b = Budget::new(&mut work, floor + READY_SCRATCH - usize::from(mode == 3));
        b.reserve_storage(floor).unwrap();
        let ledger = b.work_ledger_identity_v1();
        let result = Ready::decode(&bytes, &mut b);
        assert_eq!(result.is_ok(), mode == 0);
        assert_eq!(b.storage(), floor);
        assert!(b.work_ledger_identity_v1() == ledger);
        if mode == 0 {
            assert_eq!(b.work(), READY_WORK);
            assert_eq!(b.peak_storage(), floor + READY_SCRATCH);
        }
        if mode == 2 {
            assert!(b.failed_work().is_some());
        }
        if mode == 3 {
            assert!(b.failed_storage().is_some());
        }
    }
    for length in [0, 88, 119, 121] {
        let mut work = Work::new(READY_WORK);
        let mut b = Budget::new(&mut work, LIMIT);
        let mut candidate = bytes.to_vec();
        candidate.resize(length, 0);
        b.reserve_storage(candidate.len()).unwrap();
        assert!(Ready::decode(&candidate, &mut b).is_err());
    }
    let mut altered = bytes;
    altered[READY_BYTES - 1] ^= 1;
    let mut work = Work::new(READY_WORK);
    let mut b = Budget::new(&mut work, LIMIT);
    b.reserve_storage(altered.len()).unwrap();
    assert!(Ready::decode(&altered, &mut b).is_err());
}

#[test]
fn issuer_staged_peer_cannot_be_replaced_by_same_credential_socket() {
    let pair = || {
        net::socketpair(
            net::AddressFamily::UNIX,
            net::SocketType::SEQPACKET,
            net::SocketFlags::CLOEXEC | net::SocketFlags::NONBLOCK,
            None,
        )
        .unwrap()
    };
    let (_client, original) = pair();
    let (_other_client, different) = pair();
    let duplicate = io::fcntl_dupfd_cloexec(&original, 0).unwrap();
    let expected = Client::new(
        rustix::process::getpid().as_raw_pid() as u32,
        rustix::process::geteuid().as_raw(),
        rustix::process::getegid().as_raw(),
    )
    .unwrap();
    staging::validate_peer(original.as_fd(), expected).unwrap();
    staging::validate_peer(different.as_fd(), expected).unwrap();
    staging::validate_duplicate(original.as_fd(), duplicate.as_fd()).unwrap();
    assert!(staging::validate_duplicate(original.as_fd(), different.as_fd()).is_err());
    io::fcntl_setfd(&duplicate, io::FdFlags::empty()).unwrap();
    assert!(staging::validate_duplicate(original.as_fd(), duplicate.as_fd()).is_err());
}

#[test]
fn issuer_staged_sources_charge_full_image_and_checked_overflow() {
    let a = staging::source_storage(11).unwrap();
    let z = staging::source_storage(1011).unwrap();
    assert_eq!(z - a, 1000);
    assert!(staging::source_storage(0).is_err());
    assert!(staging::source_storage(u64::MAX).is_err());
    assert!(payload_storage::<()>(usize::MAX, 1, 1, 1).is_err());
    let payload = size_of::<Payload<()>>() + 4096;
    let child = Child::<()>::storage_for(payload).unwrap();
    assert!(child > payload);
    assert!(Stage::storage_for_sources(a).unwrap() > a);
    assert!(Stage::spawn_retaining_scratch::<Payload<()>>(payload).unwrap() > payload);
    assert!(Child::<()>::storage_for(usize::MAX).is_err());
}

#[test]
fn issuer_prepares_actual_blocking_bootstrap_peer_without_changing_compiler_end() {
    let (client, peer) = net::socketpair(
        net::AddressFamily::UNIX,
        net::SocketType::SEQPACKET,
        net::SocketFlags::CLOEXEC,
        None,
    )
    .unwrap();
    let expected = Client::new(
        rustix::process::getpid().as_raw_pid() as u32,
        rustix::process::geteuid().as_raw(),
        rustix::process::getegid().as_raw(),
    )
    .unwrap();
    assert_eq!(fs::fcntl_getfl(&client).unwrap(), fs::OFlags::RDWR);
    assert_eq!(fs::fcntl_getfl(&peer).unwrap(), fs::OFlags::RDWR);
    assert!(staging::validate_peer(peer.as_fd(), expected).is_err());
    let original = fs::fstat(&peer).unwrap();
    let mut work = Work::new(2 * staging::PEER_PREPARE_WORK);
    let mut b = Budget::new(&mut work, FILE_STORAGE + staging::PEER_PREPARE_SCRATCH);
    b.reserve_storage(FILE_STORAGE).unwrap();
    let ledger = b.work_ledger_identity_v1();
    for attempt in 1..=2 {
        staging::prepare_peer(peer.as_fd(), &mut b).unwrap();
        staging::validate_peer(peer.as_fd(), expected).unwrap();
        assert_eq!(fs::fcntl_getfl(&client).unwrap(), fs::OFlags::RDWR);
        assert_eq!(
            fs::fcntl_getfl(&peer).unwrap(),
            fs::OFlags::RDWR | fs::OFlags::NONBLOCK
        );
        let current = fs::fstat(&peer).unwrap();
        assert_eq!(
            (original.st_dev, original.st_ino),
            (current.st_dev, current.st_ino)
        );
        assert_eq!(b.storage(), FILE_STORAGE);
        assert_eq!(b.work(), attempt * staging::PEER_PREPARE_WORK);
        assert_eq!(
            b.peak_storage(),
            FILE_STORAGE + staging::PEER_PREPARE_SCRATCH
        );
        assert!(b.work_ledger_identity_v1() == ledger);
    }
    let staged = io::fcntl_dupfd_cloexec(&peer, 256).unwrap();
    staging::validate_duplicate(peer.as_fd(), staged.as_fd()).unwrap();
    staging::validate_peer(staged.as_fd(), expected).unwrap();
}

#[test]
fn issuer_peer_preparation_refuses_short_budget_before_mutation() {
    for mode in 0..3 {
        let (_client, peer) = net::socketpair(
            net::AddressFamily::UNIX,
            net::SocketType::SEQPACKET,
            net::SocketFlags::CLOEXEC,
            None,
        )
        .unwrap();
        let floor = FILE_STORAGE - usize::from(mode == 0);
        let mut work = Work::new(staging::PEER_PREPARE_WORK - usize::from(mode == 1));
        let mut b = Budget::new(
            &mut work,
            floor + staging::PEER_PREPARE_SCRATCH - usize::from(mode == 2),
        );
        b.reserve_storage(floor).unwrap();
        let ledger = b.work_ledger_identity_v1();
        let result = staging::prepare_peer(peer.as_fd(), &mut b);
        assert!(result.is_err());
        assert_eq!(fs::fcntl_getfl(&peer).unwrap(), fs::OFlags::RDWR);
        assert_eq!(b.storage(), floor);
        assert!(b.work_ledger_identity_v1() == ledger);
        match mode {
            0 => assert!(matches!(result, Err(Error::Resource(Resource::Accounting)))),
            1 => assert!(b.failed_work().is_some()),
            _ => assert!(b.failed_storage().is_some()),
        }
    }
}

#[test]
fn issuer_peer_preparation_rejects_unexpected_status_without_clearing_it() {
    let (_client, peer) = net::socketpair(
        net::AddressFamily::UNIX,
        net::SocketType::SEQPACKET,
        net::SocketFlags::CLOEXEC,
        None,
    )
    .unwrap();
    fs::fcntl_setfl(&peer, fs::OFlags::RDWR | fs::OFlags::APPEND).unwrap();
    let original = fs::fcntl_getfl(&peer).unwrap();
    assert!(original.contains(fs::OFlags::APPEND));
    let mut work = Work::new(staging::PEER_PREPARE_WORK);
    let mut b = Budget::new(&mut work, FILE_STORAGE + staging::PEER_PREPARE_SCRATCH);
    b.reserve_storage(FILE_STORAGE).unwrap();
    assert!(matches!(
        staging::prepare_peer(peer.as_fd(), &mut b),
        Err(Error::Invalid(_))
    ));
    assert_eq!(fs::fcntl_getfl(&peer).unwrap(), original);
    assert_eq!(b.storage(), FILE_STORAGE);
    assert_eq!(b.work(), staging::PEER_PREPARE_WORK);
}

#[test]
#[allow(unsafe_code)]
fn issuer_abi_stages_exact_roles_and_all_ready_writer_aliases_must_close() {
    let channels = Channels::new().unwrap();
    let files: [File; 10] = std::array::from_fn(|_| tempfile::tempfile().unwrap());
    let sources = [
        files[0].as_fd(),
        files[1].as_fd(),
        files[2].as_fd(),
        files[3].as_fd(),
        files[4].as_fd(),
        files[5].as_fd(),
        channels.ready_writer.as_fd(),
        files[6].as_fd(),
        files[7].as_fd(),
        files[8].as_fd(),
    ];
    let bindings = staging::bindings(sources).unwrap();
    assert_eq!(
        bindings.map(|binding| binding.destination()),
        staging::DESTINATIONS
    );
    assert_eq!(staging::DESTINATIONS, [3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
    let sources_charge = 14 * FILE_STORAGE + staging::BINDINGS_STORAGE;
    let mut work = Work::new(Stage::STAGING_WORK);
    let mut b = Budget::new(&mut work, LIMIT);
    b.reserve_storage(sources_charge).unwrap();
    // SAFETY: mechanical FD-table test only. Every empty file/channel/binding is
    // prepaid and kept live. This stage is NEVER spawned or treated as admitted.
    let (stage, c) = unsafe {
        Stage::stage(
            &files[9],
            &bindings,
            channels.profile_writer.as_fd(),
            channels.gate_reader.as_fd(),
            channels.exec_writer.as_fd(),
            sources_charge,
            &mut b,
        )
    }
    .unwrap();
    assert_eq!(
        c.additional_storage(),
        Stage::storage_for_sources(sources_charge).unwrap()
    );
    b.reserve_storage(c.additional_storage()).unwrap();
    for (source, destination) in sources.into_iter().zip(staging::DESTINATIONS) {
        staging::validate_duplicate(source, stage.binding(destination).unwrap().as_fd()).unwrap();
    }
    assert!(stage.binding(13).is_none());
    assert!(stage.binding(220).is_none());
    staging::validate_duplicate(files[9].as_fd(), stage.executable().as_fd()).unwrap();
    // The only remaining writer is Stage's alias after the parent channel closes.
    let readers = channels.close_child_ends();
    assert_eq!(
        io::read(&readers.ready, &mut [0u8; 1]),
        Err(io::Errno::AGAIN)
    );
    drop(stage);
    b.release_storage(c.additional_storage()).unwrap();
    assert_eq!(io::read(&readers.ready, &mut [0u8; 1]).unwrap(), 0);
}

#[test]
fn issuer_finite_launch_quota_includes_each_nested_operation_and_output_overlap() {
    let payload = size_of::<Payload<()>>() + 65536;
    let source = staging::source_storage(11).unwrap();
    let zero = Quota {
        work: 0,
        scratch: 0,
    };
    let base = quota::launch_quota::<()>(payload, source, zero, zero, zero, zero, zero, zero, zero)
        .unwrap();
    for index in 0..7 {
        let mut inputs = [zero; 7];
        inputs[index] = Quota {
            work: 13,
            scratch: 17,
        };
        let actual = quota::launch_quota::<()>(
            payload, source, inputs[0], inputs[1], inputs[2], inputs[3], inputs[4], inputs[5],
            inputs[6],
        )
        .unwrap();
        assert_eq!(
            actual.work() - base.work(),
            if index == 5 { 26 } else { 13 }
        );
        assert_eq!(actual.scratch() - base.scratch(), 17);
    }
    assert!(
        base.work()
            >= launch_io::MAX_PIPE_WORK
                + launch_io::MAX_WORK
                + (launch_io::MAX_PIPE_LIVENESS_CHECKS + launch_io::MAX_LIVENESS_CHECKS)
                    * PlainChild::OPERATION_WORK
    );
    assert!(base.scratch() > Stage::spawn_retaining_scratch::<Payload<()>>(payload).unwrap());
    assert!(
        quota::launch_quota::<()>(usize::MAX, source, zero, zero, zero, zero, zero, zero, zero)
            .is_err()
    );
    assert!(
        quota::launch_quota::<()>(
            payload,
            usize::MAX,
            zero,
            zero,
            zero,
            zero,
            zero,
            zero,
            zero
        )
        .is_err()
    );
    let overflow = Quota {
        work: usize::MAX,
        scratch: 0,
    };
    assert!(
        quota::launch_quota::<()>(
            payload, source, overflow, zero, zero, zero, zero, zero, zero
        )
        .is_err()
    );
    assert!(
        quota::launch_quota::<()>(
            payload, source, zero, zero, zero, zero, zero, zero, overflow
        )
        .is_err()
    );
}

#[test]
fn root_startup_quote_funds_original_view_channel_and_handshake_without_resetting() {
    let zero = Quota {
        work: 0,
        scratch: 0,
    };
    let base = quota::root_startup_quota::<()>(zero, zero).unwrap();
    assert_eq!(
        base.work(),
        4 * RootChannel::WORK
            + CompilerTrace::<()>::OBSERVATION_WORK
            + Resources::<Payload<()>>::ACCESS_WORK
            + RootSession::CREATE_WORK
    );
    assert_eq!(
        base.scratch(),
        RootChannel::STORAGE
            + RootChannel::SCRATCH
            + ReadyIssuer::<()>::ENVELOPE
            + CompilerTrace::<()>::OBSERVATION_SCRATCH
            + Resources::<Payload<()>>::ACCESS_SCRATCH
            + 2 * RootSession::CREATE_SCRATCH
            + NativeAttempt::<()>::ENVELOPE
    );
    let add = Quota {
        work: 13,
        scratch: 17,
    };
    for (launch, gate) in [(add, zero), (zero, add)] {
        let q = quota::root_startup_quota::<()>(launch, gate).unwrap();
        assert_eq!(
            (q.work() - base.work(), q.scratch() - base.scratch()),
            (13, 17)
        );
    }
    for overflow in [
        Quota {
            work: usize::MAX,
            scratch: 0,
        },
        Quota {
            work: 0,
            scratch: usize::MAX,
        },
    ] {
        for (launch, gate) in [(overflow, zero), (zero, overflow)] {
            assert!(matches!(
                quota::root_startup_quota::<()>(launch, gate),
                Err(Error::Resource(Resource::Arithmetic))
            ));
        }
    }
}

#[test]
fn issuer_continuity_funds_root_validation_and_refuses_quote_overflow() {
    let zero = Quota {
        work: 0,
        scratch: 0,
    };
    let increment = Quota {
        work: 13,
        scratch: 17,
    };
    let base = quota::continuity::<()>(zero, zero).unwrap();
    for (process, root) in [(increment, zero), (zero, increment)] {
        let measured = quota::continuity::<()>(process, root).unwrap();
        assert_eq!(measured.work() - base.work(), increment.work());
        assert_eq!(measured.scratch() - base.scratch(), increment.scratch());
    }
    for overflow in [
        Quota {
            work: usize::MAX,
            scratch: 0,
        },
        Quota {
            work: 0,
            scratch: usize::MAX,
        },
    ] {
        assert!(quota::continuity::<()>(zero, overflow).is_err());
        assert!(quota::continuity::<()>(overflow, zero).is_err());
    }
}

#[test]
fn issuer_continuity_charges_the_running_image_once_through_root_validation() {
    use fe2o3_broker_authority_service::retained_issuer_image_quota_v3;
    let quote = |length| {
        let root = RootConnection::validation_quota(length).unwrap();
        quota::continuity::<()>(
            Quota {
                work: 0,
                scratch: 0,
            },
            Quota {
                work: root.work(),
                scratch: root.scratch(),
            },
        )
        .unwrap()
    };
    let small = quote(4096);
    let large = quote(8192);
    let small_image = retained_issuer_image_quota_v3(4096).unwrap();
    let large_image = retained_issuer_image_quota_v3(8192).unwrap();
    assert_eq!(
        large.work() - small.work(),
        large_image.work() - small_image.work()
    );
    assert_eq!(
        large.scratch() - small.scratch(),
        large_image.scratch() - small_image.scratch()
    );
}

#[test]
fn issuer_cleanup_funds_full_payload_on_existing_pool_with_finite_turns() {
    let payload = size_of::<Payload<()>>() + 65536;
    let one = quota::cleanup_quota::<()>(payload, 1).unwrap();
    let two = quota::cleanup_quota::<()>(payload, 2).unwrap();
    assert_eq!(
        one.additional_storage(),
        Resources::<Payload<()>>::payload_storage(payload).unwrap()
    );
    assert_eq!(two.additional_storage(), one.additional_storage());
    assert_eq!(
        two.work() - one.work(),
        Cleanup::pump_work(fe2o3_protected_service_spawn::MAX_PROTECTED_SERVICE_PROCESSES_V2)
            .unwrap()
            + Cleanup::shutdown_work()
    );
    assert!(one.work() > Cleanup::retained_launch_work::<Payload<()>>(payload).unwrap());
    assert!(quota::cleanup_quota::<()>(payload, 0).is_err());
    assert!(quota::cleanup_quota::<()>(payload, usize::MAX).is_err());
    assert!(quota::cleanup_quota::<()>(size_of::<Payload<()>>() - 1, 1).is_err());
    assert!(quota::cleanup_quota::<()>(usize::MAX, 1).is_err());
}
