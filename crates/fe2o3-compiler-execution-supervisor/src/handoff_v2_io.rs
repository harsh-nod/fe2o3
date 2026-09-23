//! One poll and one nonblocking receive; rights are always drained into RAII owners.
use super::handoff_v2::ProtectedIssuerHandoffErrorV2 as Error;
use crate::handoff_ancillary::Guard;
use fe2o3_compiler_execution_protocol::COMPILER_EXECUTION_SUPERVISOR_HANDOFF_BYTES_V2 as BYTES;
use rustix::{
    event::{PollFd, PollFlags, Timespec, poll},
    net::{RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, ReturnFlags, recvmsg},
};
use std::{io::IoSliceMut, os::fd::OwnedFd, time::Instant};

pub(super) const ANCILLARY_BYTES: usize = crate::handoff_ancillary::BYTES;

pub(super) fn receive(
    control: &OwnedFd,
    deadline: Instant,
) -> Result<([u8; BYTES], [OwnedFd; 2]), Error> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return Err(Error::Timeout);
    }
    let timeout = Timespec {
        tv_sec: i64::try_from(remaining.as_secs()).unwrap_or(i64::MAX),
        tv_nsec: i64::from(remaining.subsec_nanos()),
    };
    let mut descriptors = [PollFd::new(
        control,
        PollFlags::IN | PollFlags::ERR | PollFlags::HUP,
    )];
    if poll(&mut descriptors, Some(&timeout))? == 0 {
        return Err(Error::Timeout);
    }
    let events = descriptors[0].revents();
    if events.contains(PollFlags::NVAL) {
        return Err(Error::InvalidControl("descriptor became invalid"));
    }
    if !events.contains(PollFlags::IN) {
        return Err(if events.intersects(PollFlags::ERR | PollFlags::HUP) {
            Error::ControlClosed
        } else {
            Error::InvalidControl("unexpected readiness events")
        });
    }
    let mut payload = [0; BYTES];
    let mut vectors = [IoSliceMut::new(&mut payload)];
    let mut guard = Guard::new();
    let (space, armed) = guard.parts();
    let mut ancillary = RecvAncillaryBuffer::new(space);
    let received = recvmsg(
        control,
        &mut vectors,
        &mut ancillary,
        RecvFlags::DONTWAIT | RecvFlags::CMSG_CLOEXEC,
    );
    *armed = received.is_ok();
    let mut rights: [Option<OwnedFd>; 2] = [None, None];
    let mut count = 0;
    let mut invalid = false;
    for message in ancillary.drain() {
        match message {
            RecvAncillaryMessage::ScmRights(descriptors) => {
                for descriptor in descriptors {
                    if count < rights.len() {
                        rights[count] = Some(descriptor);
                    }
                    count += 1;
                }
            }
            _ => invalid = true,
        }
    }
    drop(ancillary);
    invalid |= guard.finish();
    let received = received?;
    if invalid
        || count != rights.len()
        || received.bytes != BYTES
        || received
            .flags
            .intersects(ReturnFlags::TRUNC | ReturnFlags::CTRUNC)
    {
        return Err(Error::MalformedTransfer);
    }
    Ok((
        payload,
        rights.map(|fd| fd.expect("exact rights count checked")),
    ))
}
