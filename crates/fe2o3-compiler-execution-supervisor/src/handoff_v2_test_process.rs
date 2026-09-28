//! Actual same-UID submitter/client and distinct-UID supervisor fixture roles.
use crate::authority_v2::tests::measured_policy;
use crate::authority_v2_test_process::*;
use crate::native_consuming_test_process::{
    Case as ConsumingCase, Family, LIFECYCLE_TIMEOUT, MeasuredImage,
};
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_SERVICE_READY_BYTES_V2 as READY_BYTES,
    CompilerExecutionClientProcessIdentityV1 as Client,
    CompilerExecutionExternalAnchorServiceIdentityV1 as Anchor,
    CompilerExecutionServiceLaunchManifestV2 as Manifest, CompilerExecutionServiceReadyV2 as Ready,
    CompilerExecutionSupervisorHandoffV2 as Handoff,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};
use std::{
    os::fd::AsFd,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

const UID: u32 = 65_532;
const FAMILY: Family = Family::V2;

#[test]
#[ignore = "private actual submitter role for the isolated native supervisor fixture"]
fn submitter_process_helper() {
    require_child_credentials("submitter", UID);
    let control = inherited_control();
    run_submitter(control, None);
}

#[test]
#[ignore = "private submitter role for the real native consuming fixture"]
fn native_consuming_submitter_process_helper() {
    require_child_credentials(FAMILY.submitter_role(), UID);
    let control = inherited_control();
    let (request, []) = receive_packet::<0>(&control, Instant::now() + IO_TIMEOUT).unwrap();
    assert_eq!(&request[..4], FAMILY.case_tag());
    let case = ConsumingCase::from_id(u32::from_le_bytes(request[4..].try_into().unwrap()));
    run_submitter(control, Some(case));
}

include!("handoff_native_submitter_tests.rs");

#[test]
#[ignore = "private live client role, spawned by the isolated submitter fixture"]
fn client_process_helper() {
    require_child_credentials("client", UID);
    let control = inherited_control();
    let (service_peer, held_peer) = pair();
    let pidfd = rustix::process::pidfd_open(
        rustix::process::getpid(),
        rustix::process::PidfdFlags::empty(),
    )
    .unwrap();
    let pid = std::process::id();
    send_packet(
        &control,
        &frame(b"CLI2", pid),
        &[service_peer.as_fd(), pidfd.as_fd()],
    )
    .unwrap();
    drop((service_peer, pidfd));
    let (mut request, []) =
        receive_packet::<0>(&control, Instant::now() + Duration::from_secs(45)).unwrap();
    if request == frame(b"FIN2", pid) {
        // Inert fixture stop byte, sent only after the submitter verified the
        // actual readiness publication. The issuer's service peer is fd 4.
        assert_eq!(
            rustix::net::send(
                &held_peer,
                &[0x01],
                rustix::net::SendFlags::DONTWAIT | rustix::net::SendFlags::NOSIGNAL,
            )
            .unwrap(),
            1
        );
        send_packet(&control, &frame(b"FIN2", pid), &[]).unwrap();
        (request, []) = receive_packet::<0>(&control, Instant::now() + IO_TIMEOUT).unwrap();
    }
    assert_eq!(request, frame(b"STOP", pid));
    drop(held_peer);
}

#[allow(unsafe_code)]
pub(crate) fn enable_pidfd(fd: &std::os::fd::OwnedFd) {
    use std::os::fd::AsRawFd;
    let enabled: libc::c_int = 1;
    // SAFETY: the socket is borrowed and the fixed integer input has the Linux ABI size.
    let result = unsafe {
        libc::setsockopt(
            fd.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PASSPIDFD,
            (&enabled as *const libc::c_int).cast(),
            std::mem::size_of_val(&enabled) as libc::socklen_t,
        )
    };
    assert_eq!(
        result,
        0,
        "SO_PASSPIDFD coverage requires kernel support: {}",
        std::io::Error::last_os_error()
    );
}

pub(crate) fn send_excess_rights(
    control: &std::os::fd::OwnedFd,
    payload: &[u8],
    source: &std::os::fd::OwnedFd,
) {
    use rustix::net::{SendAncillaryBuffer, SendAncillaryMessage, SendFlags, sendmsg};
    let rights = [source.as_fd(); 32];
    assert!(
        std::mem::size_of::<libc::cmsghdr>() + 32 * std::mem::size_of::<libc::c_int>()
            > crate::handoff_v2_io::ANCILLARY_BYTES
    );
    let mut space = [std::mem::MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(32))];
    let mut ancillary = SendAncillaryBuffer::new(&mut space);
    assert!(ancillary.push(SendAncillaryMessage::ScmRights(&rights)));
    assert_eq!(
        sendmsg(
            control,
            &[std::io::IoSlice::new(payload)],
            &mut ancillary,
            SendFlags::DONTWAIT | SendFlags::NOSIGNAL
        )
        .unwrap(),
        payload.len()
    );
}
