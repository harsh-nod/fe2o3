//! Real socketpairs and native records, not a protected fixed-path service boot.
use super::*;
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionClientProcessIdentityV1 as Process,
    CompilerExecutionExternalAnchorServiceIdentityV1 as Service,
    CompilerExecutionIssuerMeasurementV1 as Measurement, CompilerExecutionIssuerPolicyV3 as Policy,
};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use rustix::net::{self, AddressFamily, SendFlags, SocketFlags, SocketType};
use std::time::Duration;

#[path = "native_supervisor_transfer_tests.rs"]
mod transfer;

fn profile(generation: u64) -> Profile {
    let mut w = Work::new(usize::MAX);
    let mut b = Budget::new(&mut w, usize::MAX);
    let (policy, charge) = Policy::new(
        generation,
        Measurement::new([1; 32], 1).unwrap(),
        Measurement::new([2; 32], 1).unwrap(),
        ed25519_dalek::SigningKey::from_bytes(&[3; 32])
            .verifying_key()
            .to_bytes(),
        ed25519_dalek::SigningKey::from_bytes(&[4; 32])
            .verifying_key()
            .to_bytes(),
        &mut b,
    )
    .unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    Profile::new(
        rustix::process::geteuid().as_raw(),
        rustix::process::getegid().as_raw(),
        Service::new(6000, 6001).unwrap(),
        policy,
        &mut b,
    )
    .unwrap()
    .0
}
fn pair() -> (OwnedFd, OwnedFd) {
    net::socketpair(
        AddressFamily::UNIX,
        SocketType::SEQPACKET,
        SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
        None,
    )
    .unwrap()
}
fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(2)
}
fn pending(profile: &Profile, b: &mut Budget<'_>) -> (Pending, OwnedFd, [u8; READY_BYTES]) {
    let result = raw_pending(profile, b);
    net::sockopt::set_socket_passcred(&result.0.control, true).unwrap();
    result
}
fn raw_pending(profile: &Profile, b: &mut Budget<'_>) -> (Pending, OwnedFd, [u8; READY_BYTES]) {
    let client = Process::new(101, 1000, 1000).unwrap();
    let parent = Process::new(100, 1000, 1000).unwrap();
    let mut w = Work::new(usize::MAX);
    let mut setup = Budget::new(&mut w, usize::MAX);
    setup.reserve_storage(profile.retained_storage()).unwrap();
    let (manifest, charge) = Manifest::new(
        client,
        profile.external_anchor_service(),
        profile.policy(),
        &mut setup,
    )
    .unwrap();
    setup.reserve_storage(charge.additional_storage()).unwrap();
    let (handoff, charge) = Handoff::new(parent, manifest, &mut setup).unwrap();
    setup.reserve_storage(charge.additional_storage()).unwrap();
    let ready = Ready::new(200, handoff.launch_manifest(), profile.policy(), &mut setup)
        .unwrap()
        .0;
    let (control, peer) = pair();
    let pending = Pending {
        control,
        handoff,
        profile: *profile.identity().as_bytes(),
        deadline: deadline(),
        ledger: b.work_ledger_identity_v1(),
    };
    b.reserve_storage(pending.retained_storage() + profile.retained_storage())
        .unwrap();
    (pending, peer, *ready.canonical_bytes())
}
fn fixture_peer(fd: &OwnedFd, credentials: Credentials) -> Result<()> {
    Ok(super::super::validate_control(fd, credentials)?)
}
struct RealPeer;
impl ReadinessIo for RealPeer {
    fn validate(&mut self, fd: &OwnedFd, expected: Credentials) -> Result<()> {
        fixture_peer(fd, expected)
    }
    fn readiness(&mut self, fd: &OwnedFd, until: Instant) -> Result<[u8; READY_BYTES]> {
        io::readiness(fd, until)
    }
    fn eof(&mut self, fd: &OwnedFd, until: Instant) -> Result<()> {
        io::eof(fd, until)
    }
}
struct Scripted {
    bytes: [u8; READY_BYTES],
    steps: Vec<&'static str>,
    fail: Option<usize>,
    unwind: bool,
}
impl Scripted {
    fn new(bytes: [u8; READY_BYTES]) -> Self {
        Self {
            bytes,
            steps: Vec::new(),
            fail: None,
            unwind: false,
        }
    }
    fn step(&mut self, name: &'static str) -> Result<()> {
        self.steps.push(name);
        if self.fail == Some(self.steps.len()) {
            assert!(!self.unwind, "injected transport unwind");
            return Err(Failure::Mismatch("injected transport refusal"));
        }
        Ok(())
    }
}
impl ReadinessIo for Scripted {
    fn validate(&mut self, _: &OwnedFd, _: Credentials) -> Result<()> {
        self.step("validate")
    }
    fn readiness(&mut self, _: &OwnedFd, _: Instant) -> Result<[u8; READY_BYTES]> {
        self.step("readiness")?;
        Ok(self.bytes)
    }
    fn eof(&mut self, _: &OwnedFd, _: Instant) -> Result<()> {
        self.step("eof")
    }
}
fn send(fd: &OwnedFd, bytes: &[u8]) {
    net::send(fd, bytes, SendFlags::DONTWAIT | SendFlags::NOSIGNAL).unwrap();
}

#[test]
fn exact_native_readiness_and_eof_preserve_account_and_return_only_growth() {
    let p = profile(7);
    let mut w = Work::new(Pending::READINESS_WORK + 19);
    let mut b = Budget::new(&mut w, Pending::READINESS_SCRATCH + 65536);
    b.charge_work(19).unwrap();
    let (pending, peer, bytes) = pending(&p, &mut b);
    let inherited = pending.retained_storage();
    let floor = b.storage();
    let identity = b.work_ledger_identity_v1();
    send(&peer, &bytes);
    drop(peer);
    let (received, growth) = pending.finish_with(&p, &mut b, &mut RealPeer).unwrap();
    assert_eq!(received.readiness().canonical_bytes(), &bytes);
    assert_eq!(received.manifest().client().pid(), 101);
    assert_eq!(
        growth.additional_storage(),
        received.retained_storage() - inherited
    );
    assert_eq!(b.work(), 19 + Pending::READINESS_WORK);
    assert_eq!(b.storage(), floor);
    assert!(b.peak_storage() <= floor + Pending::READINESS_SCRATCH);
    assert!(identity == b.work_ledger_identity_v1());
    b.reserve_storage(growth.additional_storage()).unwrap();
    let retained = received.retained_storage();
    drop(received);
    b.release_storage(retained).unwrap();
    assert_eq!(b.storage(), p.retained_storage());
}

#[test]
fn short_entry_floor_outer_work_and_scratch_refuse_before_peer_validation() {
    let p = profile(7);
    for case in 0..4 {
        let work = match case {
            0 => 7,
            1 => LOCAL_WORK - 1,
            _ => Pending::READINESS_WORK,
        };
        let mut w = Work::new(work);
        let mut b = Budget::new(&mut w, 65536);
        let (pending, peer, bytes) = raw_pending(&p, &mut b);
        let full_floor = b.storage();
        if case == 2 {
            b.release_storage(1).unwrap();
        }
        // Consume otherwise unused capacity to leave one byte less than the outer frame.
        if case == 3 {
            b.reserve_storage(65536 - full_floor - FRAME + 1).unwrap();
        }
        let floor = b.storage();
        let identity = b.work_ledger_identity_v1();
        let mut effects = Scripted::new(bytes);
        assert!(matches!(
            pending.finish_with(&p, &mut b, &mut effects),
            Err(Failure::Resource(_))
        ));
        assert!(effects.steps.is_empty());
        assert_eq!(b.storage(), floor);
        assert!(identity == b.work_ledger_identity_v1());
        assert_eq!(
            net::recv(&peer, &mut [0; 1], net::RecvFlags::DONTWAIT)
                .unwrap()
                .0,
            0
        );
    }
}

#[test]
fn wrong_profile_and_other_ledger_refuse_before_io() {
    let p = profile(7);
    let other = profile(8);
    for different_ledger in [false, true] {
        let mut w = Work::new(usize::MAX);
        let mut b = Budget::new(&mut w, usize::MAX);
        let (pending, _peer, bytes) = raw_pending(&p, &mut b);
        let mut spare_work = Work::new(usize::MAX);
        let mut spare = Budget::new(&mut spare_work, usize::MAX);
        spare.reserve_storage(b.storage()).unwrap();
        let (profile, account) = if different_ledger {
            (&p, &mut spare)
        } else {
            (&other, &mut b)
        };
        let mut effects = Scripted::new(bytes);
        assert!(pending.finish_with(profile, account, &mut effects).is_err());
        assert!(effects.steps.is_empty());
    }
}

#[test]
fn malformed_mismatched_empty_and_trailing_packets_never_become_readiness() {
    let p = profile(7);
    for case in 0..7 {
        let mut w = Work::new(usize::MAX);
        let mut b = Budget::new(&mut w, usize::MAX);
        let (pending, peer, mut bytes) = pending(&p, &mut b);
        match case {
            0 => send(&peer, b"short"),
            1 => {
                bytes[0] ^= 1;
                send(&peer, &bytes);
            }
            2 => {
                bytes[24] ^= 1;
                send(&peer, &bytes);
            }
            3 => send(&peer, &[]),
            _ => {
                send(&peer, &bytes);
                if case == 4 {
                    send(&peer, b"trailing");
                } else {
                    send(&peer, &[]);
                    if case == 6 {
                        send(&peer, b"hidden trailing");
                    }
                }
            }
        }
        drop(peer);
        let floor = b.storage();
        assert!(
            pending.finish_with(&p, &mut b, &mut RealPeer).is_err(),
            "case {case}"
        );
        assert_eq!(b.storage(), floor);
    }
}

#[test]
fn missing_credentials_expired_deadline_and_wrong_socket_shape_refuse() {
    let p = profile(7);
    for case in 0..3 {
        let mut w = Work::new(usize::MAX);
        let mut b = Budget::new(&mut w, usize::MAX);
        let (mut pending, peer, bytes) = pending(&p, &mut b);
        if case == 0 {
            net::sockopt::set_socket_passcred(&pending.control, false).unwrap();
        }
        if case == 1 {
            pending.deadline = Instant::now();
        }
        send(&peer, &bytes);
        drop(peer);
        let result = if case == 2 {
            pending.finish_with(&p, &mut b, &mut io::System)
        } else {
            pending.finish_with(&p, &mut b, &mut RealPeer)
        };
        assert!(result.is_err());
    }
}

#[test]
fn unsolicited_rights_are_closed_even_in_the_eof_position() {
    use std::{io::IoSlice, mem::MaybeUninit};
    let p = profile(7);
    for eof in [false, true] {
        let mut w = Work::new(usize::MAX);
        let mut b = Budget::new(&mut w, usize::MAX);
        let (pending, peer, bytes) = pending(&p, &mut b);
        if eof {
            send(&peer, &bytes);
        }
        let (reader, writer) = rustix::pipe::pipe_with(
            rustix::pipe::PipeFlags::CLOEXEC | rustix::pipe::PipeFlags::NONBLOCK,
        )
        .unwrap();
        let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(1))];
        let rights = [writer.as_fd()];
        let mut ancillary = net::SendAncillaryBuffer::new(&mut space);
        assert!(ancillary.push(net::SendAncillaryMessage::ScmRights(&rights)));
        net::sendmsg(
            &peer,
            &[IoSlice::new(if eof { &[] } else { &bytes })],
            &mut ancillary,
            SendFlags::NOSIGNAL,
        )
        .unwrap();
        drop(ancillary);
        drop(writer);
        drop(peer);
        assert!(pending.finish_with(&p, &mut b, &mut RealPeer).is_err());
        assert_eq!(rustix::io::read(&reader, &mut [0]).unwrap(), 0);
    }
}

#[test]
fn unwind_and_nested_work_denial_close_control_and_restore_floor() {
    let p = profile(7);
    for panic in [false, true] {
        let mut w = Work::new(if panic {
            usize::MAX
        } else {
            LOCAL_WORK + MANIFEST_WORK + READY_WORK - 1
        });
        let mut b = Budget::new(&mut w, usize::MAX);
        let (pending, _peer, bytes) = raw_pending(&p, &mut b);
        let mut effects = Scripted::new(bytes);
        if panic {
            effects.fail = Some(1);
            effects.unwind = true;
        }
        let floor = b.storage();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            pending.finish_with(&p, &mut b, &mut effects)
        }));
        assert!(if panic {
            result.is_err()
        } else {
            result.unwrap().is_err()
        });
        assert_eq!(b.storage(), floor);
    }
}

#[test]
fn scripted_success_orders_native_decode_match_and_eof_with_exact_work() {
    let p = profile(7);
    let mut w = Work::new(Pending::READINESS_WORK);
    let mut b = Budget::new(&mut w, Pending::READINESS_SCRATCH + 65536);
    let (pending, _peer, bytes) = raw_pending(&p, &mut b);
    let inherited = pending.retained_storage();
    let floor = b.storage();
    let mut effects = Scripted::new(bytes);
    let (received, charge) = pending.finish_with(&p, &mut b, &mut effects).unwrap();
    assert_eq!(effects.steps, ["validate", "readiness", "eof"]);
    assert_eq!(b.work(), Pending::READINESS_WORK);
    assert_eq!(b.storage(), floor);
    assert!(b.peak_storage() <= floor + Pending::READINESS_SCRATCH);
    assert_eq!(
        charge.additional_storage(),
        received.retained_storage() - inherited
    );
    assert_eq!(received.readiness().canonical_bytes(), &bytes);
}

#[test]
fn every_effect_refusal_is_terminal_and_closes_the_control_endpoint() {
    let p = profile(7);
    for fail in 1..=3 {
        let mut w = Work::new(usize::MAX);
        let mut b = Budget::new(&mut w, usize::MAX);
        let (pending, peer, bytes) = raw_pending(&p, &mut b);
        let floor = b.storage();
        let mut effects = Scripted::new(bytes);
        effects.fail = Some(fail);
        assert!(pending.finish_with(&p, &mut b, &mut effects).is_err());
        assert_eq!(effects.steps.len(), fail);
        assert_eq!(b.storage(), floor);
        assert_eq!(
            net::recv(&peer, &mut [0], net::RecvFlags::DONTWAIT)
                .unwrap()
                .0,
            0
        );
    }
}

#[test]
fn canonical_wrong_native_readiness_is_rejected_before_eof() {
    let p = profile(7);
    let mut w = Work::new(usize::MAX);
    let mut b = Budget::new(&mut w, usize::MAX);
    let (pending, _peer, _) = raw_pending(&p, &mut b);
    let other = profile(8);
    let (_, _other_peer, wrong) = raw_pending(&other, &mut b);
    let floor = b.storage();
    let mut effects = Scripted::new(wrong);
    assert!(matches!(
        pending.finish_with(&p, &mut b, &mut effects),
        Err(Failure::Mismatch(_))
    ));
    assert_eq!(effects.steps, ["validate", "readiness"]);
    assert_eq!(b.storage(), floor);
}
