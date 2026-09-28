//! Family decoding over the shared bounded descriptor transport.
use fe2o3_external_anchor_provisioner::{
    EXTERNAL_ANCHOR_PROVISIONING_READY_BYTES_V1 as READY_BYTES,
    ExternalAnchorProvisioningReadyV1 as Ready,
};
pub(crate) use fe2o3_protected_service_spawn::launch_io::{
    ATTEMPT_SCRATCH, Boundary, Error, Failure, MAX_LIVENESS_CHECKS, MAX_WORK, Observer,
    await_exec_eof, await_profile_ready, bounded_deadline, release_child,
};
use std::{
    os::fd::{BorrowedFd, OwnedFd},
    time::Instant,
};

pub(crate) fn receive_ready<O: Observer>(
    bootstrap: BorrowedFd<'_>,
    observer: &mut O,
    deadline: Instant,
) -> Result<(Ready, OwnedFd), Error<O::Error>> {
    let (bytes, fd) =
        fe2o3_protected_service_spawn::launch_io::receive_ready::<READY_BYTES, true, _>(
            bootstrap, observer, deadline,
        )?;
    decode(bytes, fd).map_err(Error::Failure)
}

fn decode(bytes: [u8; READY_BYTES], fd: Option<OwnedFd>) -> Result<(Ready, OwnedFd), Failure> {
    let ready = Ready::decode(&bytes).map_err(|_| Failure::MalformedReadyTransfer)?;
    Ok((ready, fd.ok_or(Failure::MalformedReadyTransfer)?))
}

#[cfg(test)]
#[path = "launch_io_tests.rs"]
mod tests;
