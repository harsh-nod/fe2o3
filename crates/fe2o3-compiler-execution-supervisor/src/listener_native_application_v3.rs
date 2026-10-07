//! One original-root native application transfer, never compiler dispatch.
use super::*;
use fe2o3_compiler_closure_capability::CompilerExecutionSupervisorDeploymentCapabilityV3 as Deployment;
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionSupervisorReadyV3 as Ready, NATIVE_APPLICATION_SUPERVISOR_SOCKET_PATH_V3,
};
use std::{io, time::Instant};

fn error(e: impl std::error::Error + Send + Sync + 'static) -> io::Error {
    io::Error::other(e)
}
fn remaining(deadline: Instant) -> io::Result<std::time::Duration> {
    let value = deadline.saturating_duration_since(Instant::now());
    if value.is_zero() {
        Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "native application supervisor deadline",
        ))
    } else {
        Ok(value)
    }
}

pub(crate) fn run<'work>(
    supervisor: Supervisor,
    listener: OwnedFd,
    root_control: OwnedFd,
    deployment: &Deployment,
    session: SessionLimits,
    deadline: Instant,
    revalidate: &mut dyn FnMut(&mut Budget<'work>) -> io::Result<()>,
    b: &mut Budget<'work>,
) -> io::Result<()> {
    b.charge_work(256 * 1024).map_err(error)?;
    revalidate(b)?;
    // Set before listen/connect: Cargo queues all three packets immediately.
    // Enabling this after the first receive cannot recover queued credentials.
    rustix::net::sockopt::set_socket_passcred(&listener, true)?;
    let filesystem = FilesystemPolicy::production(supervisor.credentials());
    let (service, charge) = Service::bind_at(
        supervisor,
        listener,
        session,
        Path::new(NATIVE_APPLICATION_SUPERVISOR_SOCKET_PATH_V3),
        filesystem,
        b,
    )
    .map_err(error)?;
    b.reserve_storage(charge.additional_storage())
        .map_err(error)?;
    revalidate(b)?;
    service.revalidate(b).map_err(error)?;
    let pid = u32::try_from(rustix::process::getpid().as_raw_pid())
        .map_err(|_| io::Error::other("native application supervisor PID"))?;
    let (ready, charge) = Ready::new(pid, deployment.deployment(), b).map_err(error)?;
    b.reserve_storage(charge.additional_storage())
        .map_err(error)?;
    // The actual service credentials called listen before private readiness.
    crate::native_deployment_io::send_ready(&root_control, ready.canonical_bytes(), b)
        .map_err(error)?;
    drop(ready);
    b.release_storage(charge.additional_storage())
        .map_err(error)?;
    let work = Wait::MAX_ATTEMPTS
        .checked_mul(accept::ATTEMPT_WORK)
        .ok_or_else(|| error(Resource::Arithmetic))?;
    b.charge_work(work).map_err(error)?;
    b.reserve_storage(crate::AcceptedNativeApplicationHandoffV3::CONTROL_STORAGE)
        .map_err(error)?;
    let control = accept::accept(
        &service.socket.descriptor,
        Wait::MAX_ATTEMPTS,
        remaining(deadline)?,
    )
    .map_err(|e| error(Error::from(e)))?;
    if !rustix::net::sockopt::socket_passcred(&control)? {
        return Err(io::Error::other(
            "native application accepted control lost credentials",
        ));
    }
    revalidate(b)?;
    service.revalidate(b).map_err(error)?;
    let (accepted, charge) = service
        .supervisor
        .accept_native_application_handoff(control, remaining(deadline)?, b)
        .map_err(error)?;
    b.reserve_storage(charge.additional_storage())
        .map_err(error)?;
    revalidate(b)?;
    let (pending, charge) =
        accepted.transfer_to_root(&service.supervisor, root_control, remaining(deadline)?, b)?;
    b.reserve_storage(charge).map_err(error)?;
    revalidate(b)?;
    let (published, charge) = pending.publish_currentness_from_root(b)?;
    b.reserve_storage(charge).map_err(error)?;
    revalidate(b)?;
    published.finish_startup(b)?;
    revalidate(b)?;
    service.revalidate(b).map_err(error)?;
    Ok(())
}
