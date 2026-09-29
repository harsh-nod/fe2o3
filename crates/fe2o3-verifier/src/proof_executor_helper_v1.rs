//! Closed bootstrap for the separate proof process. No proof/launch authority.
//!
//! The caller, not numeric root or these records, supplies host-root provenance
//! and outside whole-domain custody. The helper independently reopens approval,
//! measures its sealed image and opens a process-affine proof-runtime lease.
//! Only startup and shutdown are implemented here; proof RPC remains disabled.

use crate::retained_functional_refinement_runtime_v1::{
    RetainedFunctionalRefinementRuntimeErrorKindV1 as RuntimeKind,
    RetainedFunctionalRefinementRuntimeErrorV1 as RuntimeError,
    RetainedGeneratedVerusRuntimeBackendV1 as Runtime, open_retained_generated_verus_runtime_v1,
};
use fe2o3_build_authority::{
    COMPILER_RUNTIME_MANIFEST_MAX_FILE_BYTES_V1 as IMAGE_MAX, CompilerRuntimeRoleV1 as Role,
};
use fe2o3_compiler_closure_capability::{
    ApprovedCompilerPolicyV1 as Approval, CompilerApprovalErrorV1 as ApprovalError,
    RetainedCompilerRuntimeErrorV1 as InventoryError, RetainedCompilerRuntimeV1 as Inventory,
};
use fe2o3_compiler_execution_protocol::{
    ProofExecutorBootstrapErrorV1 as RecordError, ProofExecutorBootstrapKindV1 as Kind,
    ProofExecutorBootstrapRecordV1 as Record,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work,
};
use fe2o3_protected_service_profile::{
    ProtectedServiceCredentialProfileV1 as Credentials,
    ProtectedServiceNamespaceSetV2 as Namespaces, ProtectedServiceProcessProfileV2 as Profile,
    ProtectedServiceProfileErrorV2 as ProfileError, observations, require_owned_sigchld_v2,
};
use fe2o3_protected_service_spawn::launch_io;
use fe2o3_protected_static_executable::{
    ProtectedStaticExecutableErrorV2 as ImageError,
    ProtectedStaticExecutableMeasurementV1 as Measurement,
    ProtectedStaticExecutableOwnerV1 as Owner, ProtectedStaticExecutableV2 as Image,
};
use std::{fmt, mem::size_of, os::fd::AsFd, path::Path, time::Duration};

#[path = "proof_executor_helper_io_v1.rs"]
mod io;
type Result<T> = std::result::Result<T, ProofExecutorHelperErrorV1>;

const RUNTIME_ROOT: &str = "/opt/fe2o3/verus-runtime-v2/functional-refinement-0.2026.08.02-b677dd5";
// One dedicated-process ledger, never reset between exchanges. These are
// logical ceilings, not RSS/latency guarantees. The older runtime lease uses
// its own finite inventory bounds, not this ledger; do not claim full metering.
const WORK: usize = 1 << 44;
const STORAGE: usize = 1 << 35;
const FRAME: usize = 128 * 1024
    + launch_io::ATTEMPT_SCRATCH
    + observations::DESCRIPTOR_INVOCATION_SCRATCH
    + 8 * size_of::<Context>();

/// Terminal helper refusal. No variant contains an authority or a proof result.
#[derive(Debug)]
#[non_exhaustive]
pub enum ProofExecutorHelperErrorV1 {
    /// The original startup ledger refused.
    Resource(Resource),
    /// Fixed root approval refused.
    Approval(ApprovalError),
    /// Immutable compiler inventory refused.
    Inventory(InventoryError),
    /// Protected process or namespace observation refused.
    Profile(ProfileError),
    /// Descriptor-only invocation refused.
    Observation(observations::Error),
    /// Sealed running image refused.
    Image(ImageError),
    /// Fresh proof-runtime lease refused; no allocated diagnostic crosses here.
    Runtime(RuntimeKind),
    /// The fixed inert bootstrap record refused.
    Record(RecordError),
    /// The shared bounded transport refused.
    Transport(launch_io::Failure),
    /// One non-retrying descriptor observation failed.
    Io(rustix::io::Errno),
    /// A fixed startup or association invariant did not hold.
    Invalid(&'static str),
}
use ProofExecutorHelperErrorV1 as Error;
macro_rules! errors {
    ($($ty:ty => $variant:ident),+ $(,)?) => {$(
        impl From<$ty> for Error {
            fn from(e: $ty) -> Self { Self::$variant(e) }
        }
    )+};
}
errors!(Resource => Resource, ApprovalError => Approval, InventoryError => Inventory,
    ProfileError => Profile, observations::Error => Observation, ImageError => Image,
    RecordError => Record, launch_io::Failure => Transport, rustix::io::Errno => Io);
impl From<RuntimeError> for Error {
    fn from(e: RuntimeError) -> Self {
        Self::Runtime(e.kind())
    }
}
impl From<launch_io::Error<Error>> for Error {
    fn from(e: launch_io::Error<Error>) -> Self {
        match e {
            launch_io::Error::Failure(e) => e.into(),
            launch_io::Error::Observer(e) => e,
        }
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "proof helper bootstrap refused: {self:?}")
    }
}
impl std::error::Error for Error {}

struct Context {
    inventory: Inventory,
    image: Image,
    profile: Profile,
    namespaces: Namespaces,
    runtime: Runtime,
}
impl Context {
    fn open(b: &mut Budget<'_>) -> Result<Self> {
        let (approval, charge) = Approval::from_production_policy(b)?;
        b.reserve_storage(charge.retained_storage())?;
        let (inventory, charge) = Inventory::from_production_runtime(approval, b)?;
        b.reserve_storage(charge.additional_storage())?;
        b.charge_work(4 * fe2o3_build_authority::COMPILER_RUNTIME_MANIFEST_MAX_BYTES_V1)?;
        let mut helpers = inventory
            .manifest()
            .entries()
            .filter(|e| e.role == Role::ProofExecutorHelper);
        let entry = helpers
            .next()
            .ok_or(Error::Invalid("proof helper role absent"))?;
        if helpers.next().is_some() {
            return Err(Error::Invalid("proof helper role is not unique"));
        }
        let measurement = Measurement::new(entry.sha256, entry.length, IMAGE_MAX)
            .map_err(|_| Error::Invalid("invalid proof helper measurement"))?;
        drop(helpers);
        // Observed credentials are confinement configuration, NOT a separately
        // approved proof-role identity. The root attempt owner must supply that.
        let credentials = Credentials::new(
            rustix::process::geteuid().as_raw(),
            rustix::process::getegid().as_raw(),
        )
        .map_err(|_| Error::Invalid("proof helper requires nonroot credentials"))?;
        let (profile, charge) = Profile::capture(credentials, b)?;
        b.reserve_storage(charge.additional_storage())?;
        require_owned_sigchld_v2(b)?;
        let (namespaces, charge) = Namespaces::capture_self(b)?;
        b.reserve_storage(charge.additional_storage())?;
        let owner = Owner::new(credentials.uid(), credentials.gid())
            .map_err(|_| Error::Invalid("invalid proof helper image owner"))?;
        let (image, charge) = Image::admit_running(measurement, owner, "proof executor helper", b)?;
        b.reserve_storage(charge.additional_storage())?;
        let runtime = open_retained_generated_verus_runtime_v1(Path::new(RUNTIME_ROOT))?;
        if runtime.identity() != *inventory.manifest().proof_runtime_identity() {
            return Err(Error::Invalid("proof runtime identity mismatch"));
        }
        let context = Self {
            inventory,
            image,
            profile,
            namespaces,
            runtime,
        };
        context.revalidate(b)?;
        Ok(context)
    }
    fn revalidate(&self, b: &mut Budget<'_>) -> Result<()> {
        self.profile.revalidate_current(b)?;
        self.namespaces.revalidate_self(b)?;
        require_owned_sigchld_v2(b)?;
        self.inventory.revalidate(b)?;
        self.image.revalidate(b)?;
        self.runtime.revalidate()?;
        Ok(())
    }
}

/// Executes the descriptor-only helper bootstrap, returning no positive authority.
///
/// The sole inherited role is FD 3: an unnamed nonblocking Unix SEQPACKET with
/// SO_PASSCRED enabled on both endpoints before clone. The actual root parent
/// sends Initial and Finish records; responses identify this actual helper PID.
/// All unrelated descriptors, including stdio, are closed. No keys, paths,
/// command-line configuration, generated proof inputs or executable FDs are read.
///
/// # Safety
/// Call exactly once as the main of the dedicated secure-start static binary,
/// before creating any Rust descriptor owner, thread or child. FD 3 and all other
/// raw descriptors must be exclusively disposable here, even on refusal. The
/// creator must be the independently admitted host-root coordinator, retain
/// actual clone/pidfd/user-namespace custody and all executable backing, and have
/// an outside whole-domain custodian until retirement. Numeric root observed
/// inside this process does not establish those obligations. Never retry this
/// entry or continue normal application work after it returns or unwinds.
#[allow(unsafe_code)]
pub unsafe fn run_inherited_proof_executor_helper_v1() -> Result<()> {
    // Acquire raw cleanup custody before any operation can refuse or reuse FD 3.
    let source = unsafe { io::Source::take() };
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.with_prepaid_scope(
        0,
        8,
        io::WORK + observations::DESCRIPTOR_INVOCATION_WORK,
        FRAME,
        |b| {
            source.validate()?;
            observations::require_descriptor_only_invocation()?;
            // SAFETY: dedicated-process contract, before other descriptor owners.
            unsafe {
                io::close_unrelated()?;
            }
            let bootstrap = source.into_owned()?;
            let parent = io::Parent::capture(&bootstrap)?;
            let context = Context::open(b)?;
            let mut observer = io::Observer {
                parent: &parent,
                budget: b,
            };
            observer.check()?;
            let deadline = launch_io::bounded_deadline(Duration::from_secs(120))?;
            let (bytes, _) = launch_io::receive_ready_from::<88, false, _>(
                bootstrap.as_fd(),
                parent.sender(),
                &mut observer,
                deadline,
            )?;
            let (initial, charge) = Record::decode(&bytes, observer.budget)?;
            observer
                .budget
                .reserve_storage(charge.additional_storage())?;
            let session = *initial.session();
            let runtime = context.runtime.identity();
            if !initial.matches_association(
                Kind::Initial,
                parent.pid(),
                session,
                runtime,
                observer.budget,
            )? {
                return Err(Error::Invalid("initial proof helper association mismatch"));
            }
            drop(initial);
            observer
                .budget
                .release_storage(charge.additional_storage())?;
            for (outgoing, incoming) in [(Kind::Ready, Some(Kind::Finish)), (Kind::Finished, None)]
            {
                context.revalidate(observer.budget)?;
                observer.check()?;
                let (record, charge) = Record::new(
                    outgoing,
                    std::process::id(),
                    session,
                    runtime,
                    observer.budget,
                )?;
                observer
                    .budget
                    .reserve_storage(charge.additional_storage())?;
                launch_io::send_ready(
                    bootstrap.as_fd(),
                    record.canonical_bytes(),
                    &mut observer,
                    deadline,
                )?;
                drop(record);
                observer
                    .budget
                    .release_storage(charge.additional_storage())?;
                if let Some(kind) = incoming {
                    let (bytes, _) = launch_io::receive_ready_from::<88, false, _>(
                        bootstrap.as_fd(),
                        parent.sender(),
                        &mut observer,
                        deadline,
                    )?;
                    let (record, charge) = Record::decode(&bytes, observer.budget)?;
                    observer
                        .budget
                        .reserve_storage(charge.additional_storage())?;
                    if !record.matches_association(
                        kind,
                        parent.pid(),
                        session,
                        runtime,
                        observer.budget,
                    )? {
                        return Err(Error::Invalid("finish proof helper association mismatch"));
                    }
                    drop(record);
                    observer
                        .budget
                        .release_storage(charge.additional_storage())?;
                }
            }
            observer.check()?;
            Ok(())
        },
    )
}
