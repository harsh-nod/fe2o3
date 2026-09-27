//! Real child descriptors; the fixed-path protected supervisor is not booted here.
use super::*;
use std::{
    cell::Cell,
    io::IoSliceMut,
    mem::MaybeUninit,
    process::{Child, Command},
};

struct ChildGuard(Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn child() -> (ChildGuard, ChildLaunch) {
    let mut command = Command::new("/bin/sleep");
    command.arg("30");
    let channel = crate::PendingCompilerExecutionChildChannelV1::prepare(&mut command).unwrap();
    let child = ChildGuard(command.spawn().unwrap());
    let launch = channel.finish_until(child.0.id(), deadline()).unwrap();
    (child, launch)
}
fn reserve_inputs(p: &Profile, b: &mut Budget<'_>) {
    b.reserve_storage(Pending::CHILD_LAUNCH_STORAGE + p.retained_storage())
        .unwrap();
}

#[test]
fn resource_refusals_precede_connect_and_connect_refusal_restores_original_account() {
    let _lock = super::super::super::RESERVED_CHILD_FD_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let p = profile(7);
    for case in 0..7 {
        let (_child, launch) = child();
        let mut w = Work::new(match case {
            0 => 7,
            1 => LOCAL_WORK - 1,
            2 => Pending::TRANSFER_WORK - 1,
            _ => Pending::TRANSFER_WORK,
        });
        let mut b = Budget::new(&mut w, 65536);
        reserve_inputs(&p, &mut b);
        if case == 3 {
            b.release_storage(1).unwrap();
        }
        if case == 4 {
            b.reserve_storage(65536 - b.storage() - FRAME + 1).unwrap();
        }
        let floor = b.storage();
        let identity = b.work_ledger_identity_v1();
        let connected = Cell::new(false);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            transfer_with::<false>(launch, &p, deadline(), &mut b, |_, _| {
                connected.set(true);
                assert_ne!(case, 6, "injected connect unwind");
                Err(Failure::Mismatch("injected connect refusal"))
            })
        }));
        if case == 6 {
            assert!(result.is_err());
        } else if case < 5 {
            assert!(matches!(result.unwrap(), Err(Failure::Resource(_))));
        } else {
            assert!(matches!(result.unwrap(), Err(Failure::Mismatch(_))));
        }
        assert_eq!(connected.get(), case >= 5);
        assert_eq!(b.storage(), floor);
        assert!(identity == b.work_ledger_identity_v1());
        if case >= 5 {
            assert_eq!(b.work(), Pending::TRANSFER_WORK);
        }
    }
}

#[test]
fn stale_child_deadlines_and_same_uid_refuse_before_connect() {
    let _lock = super::super::super::RESERVED_CHILD_FD_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let p = profile(7);
    for case in 0..4 {
        let (mut child, launch) = child();
        let mut w = Work::new(usize::MAX);
        let mut b = Budget::new(&mut w, usize::MAX);
        reserve_inputs(&p, &mut b);
        let floor = b.storage();
        let until = match case {
            0 => Instant::now(),
            1 => Instant::now() + Duration::from_secs(121),
            _ => deadline(),
        };
        if case == 2 {
            child.0.kill().unwrap();
            child.0.wait().unwrap();
        }
        let connect = |_, _| -> Result<OwnedFd> { panic!("invalid launch reached connect") };
        let result = if case == 3 {
            transfer_with::<true>(launch, &p, until, &mut b, connect)
        } else {
            transfer_with::<false>(launch, &p, until, &mut b, connect)
        };
        match case {
            0 | 1 => assert!(matches!(result, Err(Failure::Transport(_)))),
            2 => assert!(matches!(result, Err(Failure::Child(_)))),
            _ => assert!(matches!(result, Err(Failure::Mismatch(_)))),
        }
        assert_eq!(b.storage(), floor);
    }
}

#[test]
fn finite_send_transfers_native_frame_and_two_ordered_child_descriptors() {
    let _lock = super::super::super::RESERVED_CHILD_FD_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let (_child, launch) = child();
    let p = profile(7);
    let client = launch.client();
    let mut w = Work::new(usize::MAX);
    let mut b = Budget::new(&mut w, usize::MAX);
    reserve_inputs(&p, &mut b);
    let (manifest, charge) =
        Manifest::new(client, p.external_anchor_service(), p.policy(), &mut b).unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    let (handoff, charge) = Handoff::new(launch.submitter(), manifest, &mut b).unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    let (service, pidfd) = launch.into_descriptors();
    let expected = [
        rustix::fs::fstat(&service).unwrap(),
        rustix::fs::fstat(&pidfd).unwrap(),
    ];
    let (control, peer) = pair();
    io::send(
        &control,
        handoff.canonical_bytes(),
        &[service.as_fd(), pidfd.as_fd()],
        deadline(),
    )
    .unwrap();
    drop(service);
    drop(pidfd);
    let mut payload =
        [0; fe2o3_compiler_execution_protocol::COMPILER_EXECUTION_SUPERVISOR_HANDOFF_BYTES_V3];
    let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(2))];
    let mut ancillary = net::RecvAncillaryBuffer::new(&mut space);
    let received = net::recvmsg(
        &peer,
        &mut [IoSliceMut::new(&mut payload)],
        &mut ancillary,
        net::RecvFlags::DONTWAIT | net::RecvFlags::CMSG_CLOEXEC,
    )
    .unwrap();
    assert_eq!(received.bytes, payload.len());
    assert!(
        !received
            .flags
            .intersects(net::ReturnFlags::TRUNC | net::ReturnFlags::CTRUNC)
    );
    assert_eq!(&payload, handoff.canonical_bytes());
    let mut descriptors = Vec::new();
    for message in ancillary.drain() {
        match message {
            net::RecvAncillaryMessage::ScmRights(rights) => descriptors.extend(rights),
            _ => panic!("unexpected ancillary data"),
        }
    }
    assert_eq!(descriptors.len(), 2);
    for (fd, expected) in descriptors.iter().zip(expected) {
        let actual = rustix::fs::fstat(fd).unwrap();
        assert_eq!(
            (actual.st_dev, actual.st_ino),
            (expected.st_dev, expected.st_ino)
        );
        assert!(
            rustix::io::fcntl_getfd(fd)
                .unwrap()
                .contains(rustix::io::FdFlags::CLOEXEC)
        );
    }
    assert_eq!(
        net::sockopt::socket_peercred(&descriptors[0])
            .unwrap()
            .pid
            .as_raw_nonzero()
            .get() as u32,
        client.pid()
    );
}
