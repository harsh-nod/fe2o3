//! Selection is only an authenticated original-root bootstrap, never argv or environment.
use super::*;
use fe2o3_protected_service_spawn::launch_io;
use fe2o3_runtime_protocol::NativeApplicationRootTransferV1 as Transfer;
use std::{
    os::fd::AsFd,
    time::{Duration, Instant},
};

fn application_error(reason: &'static str) -> v3::Error {
    v3::Error::Application(std::io::Error::other(reason))
}
fn transport_error(error: launch_io::Failure) -> v3::Error {
    let error = match error {
        launch_io::Failure::Io { source, .. } => std::io::Error::from(source),
        launch_io::Failure::Timeout(_) => std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            "native supervisor gate timed out",
        ),
        _ => std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "native supervisor gate transport refused",
        ),
    };
    v3::Error::Application(error)
}

fn classify_prefix(bytes: &[u8; 8], length: usize) -> std::io::Result<()> {
    if length != 104 || bytes != &Transfer::SUPERVISOR_GATE_PREFIX {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "unexpected native supervisor bootstrap input",
        ));
    }
    Ok(())
}

// A peek is not admission. The actual owned bootstrap is consumed and checks
// root SCM credentials plus both native identities after complete native startup.
// Plain recv never installs SCM_RIGHTS into the process when probing this prefix.
#[allow(unsafe_code)]
unsafe fn selected(b: &mut Budget<'_>) -> Result<bool, v3::Error> {
    b.with_prepaid_scope(v3::INPUT_STORAGE, 8, 64 * 1024, 4096, |_| {
        let fd = crate::COMPILER_EXECUTION_SUPERVISOR_BOOTSTRAP_FD_V1;
        let mut prefix = [0; 8];
        // SAFETY: the dedicated caller owns every raw startup slot exclusively;
        // recv borrows the scalar descriptor and writes only this fixed buffer.
        let n = unsafe {
            libc::recv(
                fd,
                prefix.as_mut_ptr().cast(),
                prefix.len(),
                libc::MSG_PEEK | libc::MSG_DONTWAIT | libc::MSG_TRUNC,
            )
        };
        if n < 0 {
            let error = std::io::Error::last_os_error();
            return if error.raw_os_error() == Some(libc::EAGAIN) {
                Ok(false)
            } else {
                Err(v3::Error::Application(error))
            };
        }
        classify_prefix(&prefix, n as usize).map_err(v3::Error::Application)?;
        Ok(true)
    })
}

/// Runs the original compiler entry unless an application gate was already
/// queued on the original root bootstrap. A present malformed frame rejects;
/// application selection still requires actual root authentication after intake.
/// The separate listener also rejects a late application gate in compiler mode.
/// No argument, environment variable or decoded record grants a service role.
///
/// # Safety
/// This is a one-shot dedicated binary entry with the exact same raw-descriptor,
/// single-thread, original-account and cleanup obligations as
/// `run_inherited_protected_issuer_service_v3`. The trusted root must enqueue an
/// application gate before releasing its child. No actor may race mode intake.
#[allow(unsafe_code)]
pub unsafe fn run_inherited_native_supervisor_v3(
    session: crate::ProtectedIssuerSessionLimitsV3,
    dispatch: crate::ProtectedIssuerDispatchLimitsV3,
    cleanup: &mut Cleanup,
    b: &mut Budget<'_>,
) -> Result<(), v3::Error> {
    // SAFETY: caller transfers the sole raw-slot/process contract to either branch.
    let application = match unsafe { selected(b) } {
        Ok(application) => application,
        Err(error) => {
            // SAFETY: selection only borrowed the raw slots. This sole owner
            // closes all eleven inputs even on entry-budget or framing refusal.
            drop(unsafe { crate::native_deployment_io::Sources::new() });
            return Err(error);
        }
    };
    if !application {
        return unsafe { v3::run(session, dispatch, cleanup, b) }.map(|_| ());
    }
    let deadline = Instant::now()
        .checked_add(Duration::from_secs(120))
        .ok_or_else(|| application_error("native supervisor deadline overflow"))?;
    // SAFETY: the selected branch still performs the same complete native intake.
    unsafe {
        v3::run_with(
            cleanup,
            b,
            |supervisor, listener, bootstrap, deployment, revalidate, _cleanup, b| {
                revalidate(b)?;
                supervisor.revalidate(b)?;
                let parent = rustix::process::getppid()
                    .ok_or_else(|| application_error("native supervisor root parent absent"))?;
                crate::deployment::validate_bootstrap::<true>(&bootstrap, Some(parent))
                    .map_err(io::Error::from)?;
                rustix::net::sockopt::set_socket_passcred(&bootstrap, true)
                    .map_err(|e| v3::Error::Application(e.into()))?;
                b.charge_work(launch_io::packet_receive_work(104))?;
                b.reserve_storage(launch_io::packet_receive_scratch(104) + 104)?;
                let packet = launch_io::receive_authenticated_packet::<104>(
                    bootstrap.as_fd(),
                    launch_io::MessageSender::new(parent.as_raw_pid(), 0, 0),
                )
                .map_err(transport_error)?
                .ok_or_else(|| application_error("native supervisor root gate missing"))?;
                Transfer::check_supervisor_gate(
                    &packet,
                    *supervisor.policy().identity().as_bytes(),
                    *deployment.deployment().identity().as_bytes(),
                    b,
                )
                .map_err(|e| v3::Error::Application(std::io::Error::other(e)))?;
                revalidate(b)?;
                supervisor.revalidate(b)?;
                crate::listener::native_v3::application::run(
                    supervisor,
                    listener,
                    bootstrap,
                    deployment,
                    session,
                    deadline,
                    &mut |b| revalidate(b).map_err(std::io::Error::other),
                    b,
                )
                .map_err(v3::Error::Application)
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_exact_application_prefix_selects_candidate_mode() {
        assert!(classify_prefix(&Transfer::SUPERVISOR_GATE_PREFIX, 104).is_ok());
        for n in [0, 8, 103, 105, 4096] {
            assert!(classify_prefix(&Transfer::SUPERVISOR_GATE_PREFIX, n).is_err());
        }
        let mut wrong = Transfer::SUPERVISOR_GATE_PREFIX;
        wrong[2] ^= 1;
        assert!(classify_prefix(&wrong, 104).is_err());
    }
}
