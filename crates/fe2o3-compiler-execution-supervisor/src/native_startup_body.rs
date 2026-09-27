use crate::{ProvisionedStaticExecutableMeasurementV1 as Provisioned, root_checks};
use fe2o3_broker_authority_service::{
    ProtectedExternalAnchorServiceAdmissionV2 as Anchor,
    ProtectedExternalAnchorServiceErrorV2 as AnchorError,
};
use fe2o3_compiler_execution_lifecycle::{
    CompilerExecutionServiceLifecycleLeaseV2 as Lease, LifecycleLeaseErrorV2 as LeaseError,
};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
use fe2o3_protected_static_executable::ProtectedStaticExecutableOwnerV1 as Owner;

pub(super) const INPUT_STORAGE: usize = 6 * FILE_STORAGE
    + 2 * (FILE_STORAGE + IMAGE_MAX)
    + Policy::FILE_STORAGE
    + Deployment::FILE_STORAGE
    + Key::FILE_STORAGE;
type Result<T> = std::result::Result<T, Error>;

/// Native inherited startup failure, with bounded native stage errors and no fallback.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// Original-ledger refusal, including complete incoming ownership.
    Resource(Resource),
    /// Fixed descriptor or finite readiness transport refused.
    Io(io::Error),
    /// Native policy, contextual deployment or service-key admission refused.
    Capability(CapabilityError),
    /// Complete protected process or namespace profile refused.
    Profile(ProfileError),
    /// Exact running image admission or continuity refused.
    Image(ImageError),
    /// Actual root's crash-retained shared lifecycle lease refused.
    Lifecycle(LeaseError),
    /// Persistent deployment-guard custody refused before listener activation.
    Cleanup(crate::ProtectedIssuerCleanupErrorV2),
    /// Native program admission refused.
    Program(ProgramError),
    /// Native external-anchor endpoint/pidfd admission refused.
    Anchor(AnchorError),
    /// Native supervisor binding or continuity refused.
    Supervisor(SupervisorError),
    /// Native listener activation or continuity refused.
    Service(ServiceError),
    /// Native readiness construction refused.
    Ready(ReadyError),
    /// Configured credentials or another fixed startup invariant is invalid.
    Invalid(&'static str),
}
macro_rules! from_error {
    ($type:ty, $variant:ident) => {
        impl From<$type> for Error {
            fn from(e: $type) -> Self {
                Self::$variant(e)
            }
        }
    };
}
from_error!(Resource, Resource);
from_error!(io::Error, Io);
from_error!(CapabilityError, Capability);
from_error!(ProfileError, Profile);
from_error!(ImageError, Image);
from_error!(LeaseError, Lifecycle);
from_error!(crate::ProtectedIssuerCleanupErrorV2, Cleanup);
from_error!(ProgramError, Program);
from_error!(AnchorError, Anchor);
from_error!(SupervisorError, Supervisor);
from_error!(ServiceError, Service);
from_error!(ReadyError, Ready);
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Io(e) => e.fmt(f),
            Self::Capability(e) => e.fmt(f),
            Self::Profile(e) => e.fmt(f),
            Self::Image(e) => e.fmt(f),
            Self::Lifecycle(e) => e.fmt(f),
            Self::Cleanup(e) => e.fmt(f),
            Self::Program(e) => e.fmt(f),
            Self::Anchor(e) => e.fmt(f),
            Self::Supervisor(e) => e.fmt(f),
            Self::Service(e) => e.fmt(f),
            Self::Ready(e) => e.fmt(f),
            Self::Invalid(s) => f.write_str(s),
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(e) => Some(e),
            Self::Io(e) => Some(e),
            Self::Capability(e) => Some(e),
            Self::Profile(e) => Some(e),
            Self::Image(e) => Some(e),
            Self::Lifecycle(e) => Some(e),
            Self::Cleanup(e) => Some(e),
            Self::Program(e) => Some(e),
            Self::Anchor(e) => Some(e),
            Self::Supervisor(e) => Some(e),
            Self::Service(e) => Some(e),
            Self::Ready(e) => Some(e),
            Self::Invalid(_) => None,
        }
    }
}

struct Context {
    policy: Policy,
    deployment: Deployment,
}
fn admit_context(policy: File, deployment: File, b: &mut Budget<'_>) -> Result<Context> {
    let (policy, charge) = Policy::from_file(policy, b)?;
    b.reserve_storage(charge.additional_storage())?;
    let (deployment, charge) = Deployment::from_file(deployment, policy.policy(), b)?;
    b.reserve_storage(charge.additional_storage())?;
    Ok(Context { policy, deployment })
}

pub(super) unsafe fn run(
    session: SessionLimits,
    dispatch: DispatchLimits,
    cleanup: &mut Cleanup,
    b: &mut Budget<'_>,
) -> Result<DispatchReport> {
    // SAFETY: the dedicated entrypoint transfers every fixed raw slot, even on refusal.
    let mut sources = unsafe { Sources::new() };
    b.with_prepaid_scope(INPUT_STORAGE, 8, NATIVE_ISSUER_STARTUP_WORK_V2,
        NATIVE_ISSUER_STARTUP_FRAME_STORAGE_V2, |b| {
            observations::require_descriptor_only_invocation().map_err(ProfileError::from)?;
            sources.validate()?;
            // SAFETY: no descriptor-owning admission has happened; the caller also
            // excludes ambient Rust owners, threads and pending cleanup custody.
            unsafe { io::close_unrelated()?; }
            let context = admit_context(
                intake(&mut sources, io::POLICY, Policy::FILE_STORAGE, b)?,
                intake(&mut sources, io::DEPLOYMENT, Deployment::FILE_STORAGE, b)?, b)?;
            let Context { policy, deployment } = context;
            let d = deployment.deployment();
            let credentials = Credentials::new(d.service_uid(), d.service_gid())
                .map_err(|_| Error::Invalid("native supervisor requires dedicated credentials"))?;
            let profile = Profile::capture(credentials, b)?;
            let owner = Owner::new(credentials.uid(), credentials.gid()).map_err(ImageError::from)?;
            let (running, charge) = Image::admit_running(measurement(d.executable(),
                fe2o3_compiler_execution_protocol::MAX_COMPILER_EXECUTION_SUPERVISOR_EXECUTABLE_BYTES_V1)?,
                owner, "native compiler supervisor", b)?;
            b.reserve_storage(charge.additional_storage())?;
            profile.revalidate(b)?;

            let bootstrap = intake(&mut sources, io::BOOTSTRAP, FILE_STORAGE, b)?.into();
            crate::deployment::validate_bootstrap::<true>(&bootstrap, rustix::process::getppid())
                .map_err(io::Error::from)?;
            let root = intake(&mut sources, io::ROOT, FILE_STORAGE, b)?;
            root_checks::inspect(&root, credentials).map_err(SupervisorError::from)?;
            let (lifecycle, charge) = Lease::admit(
                intake(&mut sources, io::LIFECYCLE, FILE_STORAGE, b)?, &root, b)?;
            b.reserve_storage(charge.additional_storage())?;
            // The cleanup pool owns a separately charged alias before any child
            // can launch. Local errors/unwind cannot release replacement exclusion.
            let (guard, charge) = lifecycle.try_clone_for_transfer(b)?;
            b.reserve_storage(charge.additional_storage())?;
            cleanup.retain_deployment_guard(guard, b)?;
            b.release_storage(charge.additional_storage())?;
            // Keep an independently charged view so each later lease check uses
            // this exact service root, even after root custody moves into Service.
            b.reserve_storage(FILE_STORAGE)?;
            let lifecycle_root = File::from(rustix::io::fcntl_dupfd_cloexec(&root, 256).map_err(|errno| io::Error::Io {
                operation: "retain native supervisor lifecycle root",
                errno,
            })?);
            let (key, charge) = Key::reissue_root_template_for_current_service(
                intake(&mut sources, io::KEY, Key::FILE_STORAGE, b)?, d, policy.policy(), b)?;
            b.reserve_storage(charge.additional_storage())?;

            let launcher_measurement = measurement(d.launcher(), IMAGE_MAX as u64)?;
            let issuer_measurement = measurement(policy.policy().executable(), IMAGE_MAX as u64)?;
            let launcher = intake(&mut sources, io::LAUNCHER, Image::file_storage(launcher_measurement)?, b)?;
            let issuer = intake(&mut sources, io::ISSUER, Image::file_storage(issuer_measurement)?, b)?;
            let expected = Provisioned::new(d.launcher().sha256(), d.launcher().byte_len())
                .map_err(ProgramError::from)?;
            let (program, charge) = Program::provision(launcher, expected, issuer, policy, b)?;
            b.reserve_storage(charge.additional_storage())?;
            let (anchor, charge) = Anchor::admit(
                intake(&mut sources, io::PEER, FILE_STORAGE, b)?.into(),
                intake(&mut sources, io::PIDFD, FILE_STORAGE, b)?.into(),
                d.external_anchor_service(), b)?;
            b.reserve_storage(charge.additional_storage())?;
            let (supervisor, charge) = Supervisor::bind(program, credentials, root, key, anchor, b)?;
            b.reserve_storage(charge.additional_storage())?;
            let (service, charge) = Service::bind(supervisor,
                intake(&mut sources, io::LISTENER, FILE_STORAGE, b)?.into(), session, b)?;
            b.reserve_storage(charge.additional_storage())?;

            let revalidate = |b: &mut Budget<'_>| -> Result<()> {
                deployment.revalidate(b)?;
                running.revalidate(b)?;
                profile.revalidate(b)?;
                lifecycle.revalidate_for_root(&lifecycle_root, b)?;
                service.revalidate(b)?;
                Ok(())
            };
            revalidate(b)?;
            let pid = u32::try_from(rustix::process::getpid().as_raw_pid())
                .map_err(|_| Error::Invalid("invalid native supervisor PID"))?;
            let (ready, charge) = Ready::new(pid, d, b)?;
            b.reserve_storage(charge.additional_storage())?;
            io::send_ready(&bootstrap, ready.canonical_bytes(), b)?;
            drop((ready, bootstrap));
            b.release_storage(charge.additional_storage() + FILE_STORAGE)?;
            revalidate(b)?;
            let report = service.run_turns(dispatch, cleanup, b);
            revalidate(b)?;
            // The persistent cleanup guard still holds the lock after these locals.
            drop(service);
            drop(lifecycle);
            Ok(report)
        })
}

const _: () = {
    assert!(Lease::FILE_STORAGE == FILE_STORAGE);
    assert!(Anchor::PAIR_STORAGE == 2 * FILE_STORAGE);
    assert!(Supervisor::ROOT_FILE_STORAGE == FILE_STORAGE);
    assert!(Service::SOCKET_STORAGE == FILE_STORAGE);
    assert!(Cleanup::GUARD_FILE_STORAGE == Lease::FILE_STORAGE);
};
