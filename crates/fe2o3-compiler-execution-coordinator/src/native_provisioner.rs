//! Fixed native V3 same-host installer. Installed command selection migrates later.
use crate::{
    CompilerExecutionProvisioningBundleV3 as Bundle,
    CompilerExecutionProvisioningInputsV3 as Inputs, native_activation as activation,
    native_root_source::provisioning as io,
};
use fe2o3_compiler_execution_lifecycle::CompilerExecutionProvisioningLifecycleLeaseV2 as Lease;
use fe2o3_compiler_execution_protocol::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Account,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work,
};
use std::{error::Error, fmt, mem::size_of, path::Path};

#[path = "native_provisioner_activation.rs"]
mod intake;
pub(crate) type Result<T> = std::result::Result<T, CompilerExecutionProvisioningInstallErrorV3>;
pub(crate) use CompilerExecutionProvisioningInstallErrorV3 as Failure;
const LOCAL_WORK: usize = 8 + 1024 * 1024;
const FRAME: usize = 64 * 1024;
const CONFIG: &str = "/etc/fe2o3/compiler-execution";
const IMAGES: [&str; 5] = [
    "/usr/libexec/fe2o3/fe2o3-compiler-execution-supervisor-v3",
    "/usr/libexec/fe2o3/fe2o3-static-preexec-launcher",
    "/usr/libexec/fe2o3/fe2o3-compiler-execution-issuer-conditional",
    "/usr/libexec/fe2o3/fe2o3-external-anchor-provisioning-helper-v3",
    "/usr/libexec/fe2o3/fe2o3-external-anchor-service-v3",
];
const LIMITS: [usize; 5] = [
    MAX_COMPILER_EXECUTION_SUPERVISOR_EXECUTABLE_BYTES_V3 as usize,
    MAX_COMPILER_EXECUTION_SUPERVISOR_LAUNCHER_BYTES_V3 as usize,
    MAX_COMPILER_EXECUTION_SUPERVISOR_LAUNCHER_BYTES_V3 as usize,
    MAX_COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_HELPER_BYTES_V3 as usize,
    MAX_COMPILER_EXECUTION_EXTERNAL_ANCHOR_EXECUTABLE_BYTES_V3 as usize,
];
const RECORD_PASSES: [(usize, usize); 7] = [
    (32, 3),
    (32, 3),
    (COMPILER_EXECUTION_ISSUER_POLICY_BYTES_V3, 2),
    (COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_BYTES_V3, 2),
    (COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_BYTES_V3, 2),
    (COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_BYTES_V3, 2),
    (COMPILER_EXECUTION_CLIENT_PROFILE_BYTES_V3, 2),
];

/// Provisions the fixed native V3 deployment under one original resource account.
///
/// Takes exactly one canonical nonzero generation argument. Measures five fixed
/// root-owned static images, retains exclusive deployment lifecycle custody,
/// generates or reuses two zeroizing seeds, and durably publishes five matching
/// V3 records with no-replace rename. Existing mismatches are never overwritten.
/// A partial publication is not rolled back: rerunning with the same inputs can
/// finish it. This configures trust; it does not execute a compiler or prove kernels.
/// The installed provision command remains V1 until deployment/client migration.
///
/// File, randomness and NSS calls are single attempts with fixed bounds; short
/// operations, EINTR and oversized NSS entries refuse without retry. Logical
/// budgets do not bound filesystem/NSS latency or libc-internal allocation.
/// Administrative writers must exclusively control the fixed deployment paths
/// throughout provisioning. Advisory locks exclude cooperating services and
/// installers, not privileged writers ignoring them. Path checks and temporary
/// cleanup are point-in-time observations, not atomic protection against root.
///
/// # Safety
/// Call only once on the dedicated, single-threaded main thread, with exclusive
/// control of a valid C environment and process argv. No borrowed/concurrent
/// environment pointers, argv mutation or foreign/signal-handler environment
/// access may exist. The validated bounded environment is cleared before NSS or
/// provisioning. Terminate the process on return or caught unwind; do not retry
/// this entrypoint in the same process. No descriptor activation is accepted.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_coordinator::run_compiler_execution_reference_provisioner_v3;
/// let _ = run_compiler_execution_reference_provisioner_v3();
/// ```
#[allow(unsafe_code)]
pub unsafe fn run_compiler_execution_reference_provisioner_v3() -> Result<()> {
    let (work, storage) = quota()?;
    let mut account = Account::new(Work::new(work), storage);
    account.with_budget(|b| {
        b.with_prepaid_scope(0, 8, LOCAL_WORK, FRAME, |b| {
            crate::native::require_root()?;
            crate::native_inherited::require_single_threaded()?;
            let generation = intake::generation(b)?;
            // SAFETY: the unique entrypoint supplies exclusive environment ownership.
            unsafe { activation::clear_for_provisioning(b) }?;
            let compiler = intake::service(c"fe2o3-compiler", b)?;
            let anchor = intake::service(c"fe2o3-anchor", b)?;
            let (lease, charge) = Lease::open(b)?;
            b.reserve_storage(charge.additional_storage())?;
            provision(
                &Layout {
                    config: Path::new(CONFIG),
                    listener: Path::new(COMPILER_EXECUTION_SUPERVISOR_SOCKET_PATH_V1),
                    images: IMAGES.map(Path::new),
                    limits: LIMITS,
                },
                generation,
                compiler,
                anchor,
                &mut |b| lease.revalidate(b).map_err(Into::into),
                io::Owner::ROOT,
                b,
            )
        })
    })
}

fn sum(values: &[usize]) -> Result<usize> {
    values.iter().try_fold(0usize, |sum, n| {
        sum.checked_add(*n).ok_or(Resource::Arithmetic.into())
    })
}
fn quota() -> Result<(usize, usize)> {
    let mut work = sum(&[
        LOCAL_WORK,
        activation::CAPTURE_WORK,
        intake::ARGS_WORK,
        2 * intake::ACCOUNT_WORK,
        Lease::ADMISSION_WORK,
        3 * Lease::REVALIDATION_WORK,
        6 * io::FILE_WORK,
        Bundle::WORK,
    ])?;
    let mut storage = sum(&[
        FRAME,
        activation::CAPTURE_SCRATCH,
        intake::ARGS_SCRATCH,
        2 * intake::ACCOUNT_SCRATCH,
        2 * Lease::IO_STORAGE,
        2 * io::FILE_SCRATCH,
        io::Directory::STORAGE,
        2 * io::SEED_STORAGE,
        size_of::<Inputs>(),
        Bundle::SCRATCH,
        Bundle::SCRATCH,
    ])?;
    // Sum transient envelopes and all retained source owners conservatively.
    for maximum in LIMITS {
        let (w, s) = io::Image::quota(maximum)?;
        let (rw, rs) = io::Image::revalidation_quota(maximum)?;
        work = sum(&[work, w, rw, rw])?;
        storage = sum(&[
            storage,
            s,
            rs,
            maximum,
            size_of::<io::Image>(),
            io::PATH_BYTES,
        ])?;
    }
    for (n, passes) in RECORD_PASSES {
        let record_work = io::record_work(n)?
            .checked_mul(passes)
            .ok_or(Resource::Arithmetic)?;
        work = sum(&[work, record_work])?;
        storage = sum(&[storage, io::record_scratch(n)?])?;
    }
    Ok((work, storage))
}

struct Layout<'a> {
    config: &'a Path,
    listener: &'a Path,
    images: [&'a Path; 5],
    limits: [usize; 5],
}
#[allow(clippy::too_many_arguments)]
fn provision(
    layout: &Layout<'_>,
    generation: u64,
    compiler: (u32, u32),
    anchor: (u32, u32),
    revalidate: &mut impl FnMut(&mut Budget<'_>) -> Result<()>,
    owner: io::Owner,
    b: &mut Budget<'_>,
) -> Result<()> {
    io::listener_absent(layout.listener, b)?;
    let directory = io::Directory::open(layout.config, owner, b)?;
    b.reserve_storage(io::Directory::STORAGE)?;
    revalidate(b)?;
    io::listener_absent(layout.listener, b)?;
    // Array initialization drops earlier image owners on a later refusal.
    let mut measure = |index| -> Result<io::Image<'_>> {
        let image = io::Image::measure(layout.images[index], layout.limits[index], owner, b)?;
        b.reserve_storage(image.retained_storage())?;
        Ok(image)
    };
    let images = [
        measure(0)?,
        measure(1)?,
        measure(2)?,
        measure(3)?,
        measure(4)?,
    ];
    let issuer_seed = directory.seed("issuer-signing-key-seed-v3", b)?;
    b.reserve_storage(io::SEED_STORAGE)?;
    let anchor_seed = directory.seed("anchor-signing-key-seed-v3", b)?;
    b.reserve_storage(io::SEED_STORAGE)?;
    let inputs = Inputs {
        generation,
        compiler_service_uid: compiler.0,
        compiler_service_gid: compiler.1,
        external_anchor_service: CompilerExecutionExternalAnchorServiceIdentityV1::new(
            anchor.0, anchor.1,
        )?,
        supervisor: images[0].measurement,
        launcher: images[1].measurement,
        issuer: images[2].measurement,
        anchor_helper: images[3].measurement,
        anchor_daemon: images[4].measurement,
        issuer_verifying_key: ed25519_dalek::SigningKey::from_bytes(&issuer_seed)
            .verifying_key()
            .to_bytes(),
        anchor_verifying_key: ed25519_dalek::SigningKey::from_bytes(&anchor_seed)
            .verifying_key()
            .to_bytes(),
    };
    b.reserve_storage(inputs.retained_storage())?;
    let (bundle, charge) = Bundle::new(&inputs, b)?;
    b.reserve_storage(charge.additional_storage())?;
    revalidate(b)?;
    io::listener_absent(layout.listener, b)?;
    for image in &images {
        image.revalidate(b)?;
    }
    directory.verify_seed(&issuer_seed, b)?;
    directory.verify_seed(&anchor_seed, b)?;
    directory.publish("issuer-policy-v3", bundle.policy().canonical_bytes(), b)?;
    directory.publish(
        "supervisor-deployment-v3",
        bundle.supervisor().canonical_bytes(),
        b,
    )?;
    directory.publish(
        "anchor-deployment-v3",
        bundle.anchor_deployment().canonical_bytes(),
        b,
    )?;
    directory.publish(
        "anchor-provisioning-v3",
        bundle.anchor_provisioning().canonical_bytes(),
        b,
    )?;
    directory.publish(
        "client-profile-v3",
        bundle.client_profile().canonical_bytes(),
        b,
    )?;
    directory.verify("issuer-policy-v3", bundle.policy().canonical_bytes(), b)?;
    directory.verify(
        "supervisor-deployment-v3",
        bundle.supervisor().canonical_bytes(),
        b,
    )?;
    directory.verify(
        "anchor-deployment-v3",
        bundle.anchor_deployment().canonical_bytes(),
        b,
    )?;
    directory.verify(
        "anchor-provisioning-v3",
        bundle.anchor_provisioning().canonical_bytes(),
        b,
    )?;
    directory.verify(
        "client-profile-v3",
        bundle.client_profile().canonical_bytes(),
        b,
    )?;
    directory.verify_seed(&issuer_seed, b)?;
    directory.verify_seed(&anchor_seed, b)?;
    for image in &images {
        image.revalidate(b)?;
    }
    directory.revalidate(b)?;
    revalidate(b)?;
    io::listener_absent(layout.listener, b)?;
    Ok(())
}

#[cfg(test)]
#[path = "native_provisioner_tests.rs"]
mod tests;

/// Fixed-shape native installer refusal, preserving typed component causes.
#[derive(Debug)]
#[non_exhaustive]
pub enum CompilerExecutionProvisioningInstallErrorV3 {
    /// Original account or checked arithmetic refused.
    Resource(Resource),
    /// Root identity refused before provisioning.
    Root(crate::CompilerExecutionPreparationErrorV2),
    /// Exclusive startup or bounded environment/argument observation refused.
    Activation(crate::CompilerExecutionRootDeploymentErrorV2),
    /// Root-owned source metadata or stable bytes refused.
    Source(crate::RootSourceErrorV2),
    /// Shared static ELF profile refused.
    Image(fe2o3_runtime_protocol::SealedStaticApplicationErrorV1),
    /// Native exclusive lifecycle custody refused.
    Lifecycle(fe2o3_compiler_execution_lifecycle::LifecycleLeaseErrorV2),
    /// Native public record construction refused.
    Records(crate::CompilerExecutionProvisioningErrorV3),
    /// Dedicated service identity refused.
    Service(CompilerExecutionExternalAnchorServiceIdentityErrorV1),
    /// Fixed input or observation mismatch.
    Invalid(&'static str),
    /// One nonretrying OS operation refused.
    Io {
        /// Fixed operation label.
        operation: &'static str,
        /// Kernel error number.
        source: rustix::io::Errno,
    },
}
macro_rules! errors {
    ($($source:ty => $variant:ident),+ $(,)?) => {
        $(impl From<$source> for Failure { fn from(error: $source) -> Self { Self::$variant(error) } })+
        impl Error for Failure { fn source(&self) -> Option<&(dyn Error + 'static)> { match self {
            $(Self::$variant(error) => Some(error),)+ Self::Io { source, .. } => Some(source), Self::Invalid(_) => None,
        } } }
        impl fmt::Display for Failure { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { match self {
            $(Self::$variant(error) => error.fmt(f),)+ Self::Io { operation, source } => write!(f, "{operation}: {source}"), Self::Invalid(message) => f.write_str(message),
        } } }
    };
}
errors!(Resource => Resource, crate::CompilerExecutionPreparationErrorV2 => Root,
    crate::CompilerExecutionRootDeploymentErrorV2 => Activation, crate::RootSourceErrorV2 => Source,
    fe2o3_runtime_protocol::SealedStaticApplicationErrorV1 => Image,
    fe2o3_compiler_execution_lifecycle::LifecycleLeaseErrorV2 => Lifecycle,
    crate::CompilerExecutionProvisioningErrorV3 => Records,
    CompilerExecutionExternalAnchorServiceIdentityErrorV1 => Service);
