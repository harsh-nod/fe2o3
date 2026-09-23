//! Native policy-bound program custody. Supervisor/process authority is separate.
use crate::{
    IssuerProgramAdmissionErrorV1 as LegacyError,
    ProvisionedStaticExecutableMeasurementV1 as Provisioned, protected_measurement,
};
use fe2o3_compiler_closure_capability::{
    CompilerExecutionCapabilityErrorV2 as CapabilityError,
    CompilerExecutionPolicyCapabilityV2 as PolicyCapability,
};
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionIssuerPolicyV2 as Policy, SEALED_STATIC_ISSUER_RUNTIME_CLOSURE_V1,
    sealed_static_issuer_runtime_measurement_v1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_protected_static_executable::{
    ProtectedStaticExecutableErrorV2 as ImageError,
    ProtectedStaticExecutableMeasurementV1 as Measurement,
    ProtectedStaticExecutableOwnerV1 as Owner, ProtectedStaticExecutableStorageV2 as ImageStorage,
    ProtectedStaticExecutableV2 as Image,
};
use fe2o3_static_preexec_manifest::StaticPreexecObjectIdentityV1 as Object;
use std::{error::Error, fmt, fs::File, mem::size_of};

const ENTRY: usize = 8;
type Result<T> = std::result::Result<T, IssuerProgramAdmissionErrorV2>;

/// Unreserved additional storage returned by native program admission.
#[derive(Clone, Copy, Debug)]
pub struct IssuerProgramStorageV2(usize);
impl IssuerProgramStorageV2 {
    /// Preserve consumed reservations and reserve this delta before retention.
    pub const fn additional_storage(self) -> usize {
        self.0
    }
}

/// Fresh native policy plus two independently sealed static executable owners.
/// Admission does not bind service credentials, signing key, protected root,
/// anchor transport, running process, readiness or GPU authority. The native
/// supervisor and serving entrypoint must still be integrated before activation.
/// No admitted V1 program is converted or projected into this owner.
///
/// All inputs and outputs use one caller ledger. Scopes restore entry storage
/// without refunding work; retire consumed reservations after an error returns.
/// Caller-provided launcher measurement and policy must come from trusted
/// provisioning, not the requesting compiler or inherited pair agreement alone.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::AdmittedIssuerProgramV2;
/// fn duplicate(p:AdmittedIssuerProgramV2) { let _=p.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::AdmittedIssuerProgramV2;
/// fn descriptor<T:std::os::fd::AsFd>() {}
/// descriptor::<AdmittedIssuerProgramV2>();
/// ```
pub struct AdmittedIssuerProgramV2 {
    launcher: Image,
    issuer: Image,
    policy: PolicyCapability,
}
impl AdmittedIssuerProgramV2 {
    /// Outer logical work, including two fixed runtime-closure derivations,
    /// four credential syscalls and constant ownership bookkeeping.
    /// Native capability/image operations charge separately on the same ledger.
    pub const WORK: usize = ENTRY + 64 * SEALED_STATIC_ISSUER_RUNTIME_CLOSURE_V1.len() + 5 * 1024;
    /// Outer logical ownership/control scratch, not allocator/RSS/stack bounds.
    pub const SCRATCH: usize = 4 * size_of::<(Self, IssuerProgramStorageV2)>() + 4096;

    /// Consumes a prepaid native policy and both source File/image reservations.
    /// Sources may alias; retained launcher and issuer memfds must not.
    pub fn provision(
        launcher_source: File,
        launcher_expected: Provisioned,
        issuer_source: File,
        policy: PolicyCapability,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, IssuerProgramStorageV2)> {
        budget.charge_work(ENTRY)?;
        let launcher_measurement = protected_measurement(launcher_expected, "static launcher")?;
        // The issuer's size validity is checked after launcher admission, as in
        // V1. Resource preflight only accounts the caller's declared File image.
        let issuer_length = usize::try_from(policy.policy().executable().byte_len())
            .map_err(|_| Resource::Arithmetic)?;
        let issuer_floor = issuer_length
            .checked_add(size_of::<(File, ImageStorage)>())
            .ok_or(Resource::Arithmetic)?;
        let floor = policy
            .retained_storage()
            .checked_add(Image::file_storage(launcher_measurement)?)
            .and_then(|n| n.checked_add(issuer_floor))
            .ok_or(Resource::Arithmetic)?;
        budget.with_prepaid_scope(floor, 0, Self::WORK - ENTRY, Self::SCRATCH, |budget| {
            policy.revalidate(budget)?;
            require_runtime(policy.policy())?;
            let (launcher, delta) = Image::seal_source_for_owner(
                launcher_source,
                launcher_measurement,
                Owner::current(),
                "static launcher",
                budget,
            )?;
            budget.reserve_storage(delta.additional_storage())?;
            let issuer_expected = Provisioned::from_issuer_policy(policy.policy().executable())?;
            let (issuer, delta) = Image::seal_source_for_owner(
                issuer_source,
                protected_measurement(issuer_expected, "compiler issuer")?,
                Owner::current(),
                "compiler issuer",
                budget,
            )?;
            budget.reserve_storage(delta.additional_storage())?;
            let admitted = Self {
                launcher,
                issuer,
                policy,
            };
            admitted.check(budget)?;
            let delta = admitted
                .retained_storage()
                .checked_sub(floor)
                .ok_or(Resource::Accounting)?;
            Ok((admitted, IssuerProgramStorageV2(delta)))
        })
    }

    /// Revalidates native policy/runtime and both exact retained image owners.
    pub fn revalidate(&self, budget: &mut Budget<'_>) -> Result<()> {
        budget.with_prepaid_scope(
            self.retained_storage(),
            ENTRY,
            Self::WORK,
            Self::SCRATCH,
            |budget| self.check(budget),
        )
    }

    fn check(&self, budget: &mut Budget<'_>) -> Result<()> {
        self.policy.revalidate(budget)?;
        require_runtime(self.policy())?;
        if self.policy().executable().sha256() != self.issuer.measurement().sha256()
            || self.policy().executable().byte_len() != self.issuer.measurement().byte_len()
        {
            return Err(IssuerProgramAdmissionErrorV2::PolicyImageMismatch);
        }
        self.launcher.revalidate(budget)?;
        self.issuer.revalidate(budget)?;
        let l = self.launcher.object_identity();
        let i = self.issuer.object_identity();
        if l.device() == i.device() && l.inode() == i.inode() {
            return Err(IssuerProgramAdmissionErrorV2::AliasedImages);
        }
        Ok(())
    }

    /// Caller-pinned native policy, without exposing its retained descriptor.
    pub const fn policy(&self) -> &Policy {
        self.policy.policy()
    }
    /// Trusted measurement of the admitted launcher.
    pub const fn launcher_measurement(&self) -> Measurement {
        self.launcher.measurement()
    }
    /// Trusted measurement of the admitted issuer.
    pub const fn issuer_measurement(&self) -> Measurement {
        self.issuer.measurement()
    }
    /// Inert launcher object identity for the shared static descriptor manifest.
    pub fn launcher_object_identity(&self) -> Object {
        object(&self.launcher)
    }
    /// Inert issuer object identity for the shared static descriptor manifest.
    pub fn issuer_object_identity(&self) -> Object {
        object(&self.issuer)
    }

    /// Returns a separately charged CLOEXEC executable File for controlled transport.
    /// Does not meter arbitrary subsequent File operations or grant process authority.
    pub fn try_clone_launcher_for_launch(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<(File, ImageStorage)> {
        self.transport(0, budget, |b| Ok(self.launcher.try_clone_for_exec(b)?))
    }
    /// Returns a separately charged CLOEXEC issuer executable File.
    pub fn try_clone_issuer_for_launch(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<(File, ImageStorage)> {
        self.transport(0, budget, |b| Ok(self.issuer.try_clone_for_exec(b)?))
    }
    /// Revalidates the launcher File as the retained kernel object, not merely its bytes.
    pub fn revalidate_launcher_clone(&self, image: &File, budget: &mut Budget<'_>) -> Result<()> {
        self.transport(
            Image::file_storage(self.launcher.measurement())?,
            budget,
            |b| Ok(self.launcher.revalidate_exec_clone(image, b)?),
        )
    }
    /// Revalidates the issuer File as the retained kernel object, not merely its bytes.
    pub fn revalidate_issuer_clone(&self, image: &File, budget: &mut Budget<'_>) -> Result<()> {
        self.transport(
            Image::file_storage(self.issuer.measurement())?,
            budget,
            |b| Ok(self.issuer.revalidate_exec_clone(image, b)?),
        )
    }
    fn transport<T>(
        &self,
        extra: usize,
        budget: &mut Budget<'_>,
        action: impl FnOnce(&mut Budget<'_>) -> Result<T>,
    ) -> Result<T> {
        let floor = self
            .retained_storage()
            .checked_add(extra)
            .ok_or(Resource::Arithmetic)?;
        budget.with_prepaid_scope(floor, ENTRY, Self::WORK, Self::SCRATCH, action)
    }
    /// Full retained logical charge; retire only after dropping this program.
    pub fn retained_storage(&self) -> usize {
        self.launcher.retained_storage()
            + self.issuer.retained_storage()
            + self.policy.retained_storage()
    }
}

fn require_runtime(policy: &Policy) -> Result<()> {
    if policy.runtime() != sealed_static_issuer_runtime_measurement_v1() {
        return Err(IssuerProgramAdmissionErrorV2::RuntimePolicyMismatch);
    }
    Ok(())
}
fn object(image: &Image) -> Object {
    let o = image.object_identity();
    Object::new(o.device(), o.inode(), o.byte_len(), o.mode())
}
impl fmt::Debug for AdmittedIssuerProgramV2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AdmittedIssuerProgramV2")
            .field("authority", &"none")
            .field("launcher", &self.launcher_measurement())
            .field("issuer", &self.issuer_measurement())
            .field("policy", &self.policy().identity())
            .finish_non_exhaustive()
    }
}

/// Bounded native program admission failure, without fallback to V1 authority.
#[derive(Debug)]
pub enum IssuerProgramAdmissionErrorV2 {
    /// Input/outer shared-ledger refusal.
    Resource(Resource),
    /// Native sealed policy refusal.
    Policy(CapabilityError),
    /// Native sealed executable refusal.
    Image(ImageError),
    /// Trusted measurement is invalid under the existing provisioning bound.
    Measurement(LegacyError),
    /// Runtime does not match the sealed-static issuer profile.
    RuntimePolicyMismatch,
    /// Exact policy executable measurement differs from the retained issuer.
    PolicyImageMismatch,
    /// Launcher and issuer unexpectedly refer to the same retained object.
    AliasedImages,
}
impl From<Resource> for IssuerProgramAdmissionErrorV2 {
    fn from(e: Resource) -> Self {
        Self::Resource(e)
    }
}
impl From<CapabilityError> for IssuerProgramAdmissionErrorV2 {
    fn from(e: CapabilityError) -> Self {
        Self::Policy(e)
    }
}
impl From<ImageError> for IssuerProgramAdmissionErrorV2 {
    fn from(e: ImageError) -> Self {
        Self::Image(e)
    }
}
impl From<LegacyError> for IssuerProgramAdmissionErrorV2 {
    fn from(e: LegacyError) -> Self {
        Self::Measurement(e)
    }
}
impl fmt::Display for IssuerProgramAdmissionErrorV2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Policy(e) => e.fmt(f),
            Self::Image(e) => e.fmt(f),
            Self::Measurement(e) => e.fmt(f),
            Self::RuntimePolicyMismatch => f.write_str("native issuer runtime policy mismatch"),
            Self::PolicyImageMismatch => f.write_str("native issuer executable policy mismatch"),
            Self::AliasedImages => f.write_str("native issuer program has aliased retained images"),
        }
    }
}
impl Error for IssuerProgramAdmissionErrorV2 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Resource(e) => Some(e),
            Self::Policy(e) => Some(e),
            Self::Image(e) => Some(e),
            Self::Measurement(e) => Some(e),
            _ => None,
        }
    }
}

const _: () = {
    use fe2o3_compiler_closure_capability::CompilerExecutionCapabilityStorageV2 as PolicyStorage;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1 as LedgerIdentity;
    const fn envelope<T>() -> usize {
        size_of::<Result<T>>().saturating_sub(size_of::<T>())
            + size_of::<std::thread::Result<Result<T>>>().saturating_sub(size_of::<T>())
    }
    assert!(
        size_of::<LedgerIdentity>()
            + size_of::<std::result::Result<(), Resource>>()
            + 2 * size_of::<bool>()
            + 2 * size_of::<sha2::Sha256>()
            + envelope::<(AdmittedIssuerProgramV2, IssuerProgramStorageV2)>()
            + envelope::<(File, ImageStorage)>()
            + envelope::<()>()
            <= 1024
    );
    assert!(
        size_of::<(AdmittedIssuerProgramV2, IssuerProgramStorageV2)>()
            <= 2 * size_of::<(Image, ImageStorage)>()
                + size_of::<(PolicyCapability, PolicyStorage)>()
    );
    assert!(
        8 * size_of::<IssuerProgramAdmissionErrorV2>() + 64 * size_of::<usize>() + 1024 <= 4096
    );
};
