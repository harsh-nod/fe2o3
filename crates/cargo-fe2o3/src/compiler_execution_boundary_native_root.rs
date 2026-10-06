//! The original root transport joins the SAME V5 lease/token/carriage pipeline.
use super::*;
use crate::protected_compiler_handoff_v3::{
    ParentRustcInvocationCustody as Parent,
    root_intake::{RootCompleted, RootEvidence},
};
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_ISSUER_POLICY_STORAGE_V3 as POLICY_SCRATCH,
    COMPILER_EXECUTION_ISSUER_POLICY_WORK_V3 as POLICY_WORK,
    COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_STORAGE_V3 as MANIFEST_SCRATCH,
    COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_WORK_V3 as MANIFEST_WORK,
    COMPILER_EXECUTION_SERVICE_READY_STORAGE_V3 as READY_SCRATCH,
    COMPILER_EXECUTION_SERVICE_READY_WORK_V3 as READY_WORK,
    CompilerExecutionIssuerPolicyV3 as PolicyRecord,
};

pub(super) enum ProfileCustody<'b> {
    Local(Profile),
    Root(&'b Profile),
}
impl std::ops::Deref for ProfileCustody<'_> {
    type Target = Profile;
    fn deref(&self) -> &Profile {
        match self {
            Self::Local(profile) => profile,
            Self::Root(profile) => profile,
        }
    }
}

pub(super) enum Origin<'b> {
    Local(Received),
    Root(RootEvidence<'b>),
}
impl Origin<'_> {
    pub(super) fn manifest(&self) -> &Manifest {
        match self {
            Self::Local(received) => received.manifest(),
            Self::Root(evidence) => evidence.completion().manifest(),
        }
    }
    pub(super) fn readiness(&self) -> &Ready {
        match self {
            Self::Local(received) => received.readiness(),
            Self::Root(evidence) => evidence.completion().readiness(),
        }
    }
    pub(super) fn retained_storage(&self) -> usize {
        match self {
            Self::Local(received) => received.retained_storage(),
            Self::Root(evidence) => evidence.retained_storage(),
        }
    }
    pub(super) fn revalidate(&self, b: &mut Budget<'_>) -> Result<()> {
        match self {
            Self::Local(_) => Ok(()),
            Self::Root(evidence) => Ok(evidence.revalidate(b)?),
        }
    }
    pub(super) fn require_subject(&self, subject: &Subject, b: &mut Budget<'_>) -> Result<()> {
        match self {
            Self::Local(_) => Ok(()),
            Self::Root(evidence) => Ok(evidence.require_subject(subject, b)?),
        }
    }
    pub(super) fn require_output(&self, path: &Path, b: &mut Budget<'_>) -> Result<()> {
        match self {
            Self::Local(_) => Ok(()),
            Self::Root(evidence) => Ok(evidence.require_output_path(path, b)?),
        }
    }
}

impl<'b, 'work> ParentCompilerExecutionReadinessCustodyV3<'b, 'work> {
    /// Additional inert construction quote, excluding input owners and the
    /// existing downstream artifact pipeline. No account or approval is created.
    pub(crate) const ROOT_ADMISSION_WORK: usize = RootEvidence::REVALIDATION_WORK
        + POLICY_WORK
        + 2 * Policy::IO_WORK
        + Profile::IO_WORK
        + 2 * LOCAL_WORK
        + MANIFEST_WORK
        + READY_WORK;
    pub(crate) const ROOT_ADMISSION_SCRATCH: usize = RootEvidence::REVALIDATION_SCRATCH
        + Self::OWNER_STORAGE
        + Policy::FILE_STORAGE
        + Policy::IO_STORAGE
        + POLICY_SCRATCH
        + Profile::IO_STORAGE
        + MANIFEST_SCRATCH
        + READY_SCRATCH
        + 2 * FRAME;

    /// Consume actual authenticated original-root custody, retaining the funded
    /// profile borrow and original account through the existing artifact path.
    /// No Received, Child, caller terminal code, or detached proof is fabricated.
    pub(crate) fn from_original_root(completed: RootCompleted<'b, 'work>) -> Result<Self> {
        let (evidence, budget) = completed.into_parts();
        evidence.revalidate(budget)?;
        let profile = evidence.profile();
        budget.reserve_storage(Self::OWNER_STORAGE + Policy::FILE_STORAGE)?;
        let (record, storage) =
            PolicyRecord::decode(profile.profile().policy().canonical_bytes(), budget)?;
        budget.reserve_storage(storage.additional_storage())?;
        let (policy, storage) = Policy::create(record, budget)?;
        budget.reserve_storage(storage.additional_storage())?;
        let child_pid = evidence.completion().manifest().client().pid();
        validate_readiness(
            profile,
            &policy,
            child_pid,
            evidence.completion().manifest(),
            evidence.completion().readiness(),
            budget,
        )?;
        Ok(Self {
            profile: ProfileCustody::Root(profile),
            policy,
            origin: Origin::Root(evidence),
            child_pid,
            completion: None,
            budget,
        })
    }

    /// Only the private completed original-root origin can discharge runtime
    /// custody. The legacy local-child path still refuses at its original gate.
    pub(super) fn require_runtime_enforcement(&mut self, parent: &Parent) -> Result<()> {
        match &self.origin {
            Origin::Local(_) => super::require_runtime_enforcement(self.budget),
            Origin::Root(evidence) => {
                evidence.revalidate(self.budget)?;
                Ok(evidence.require_parent(parent, self.budget)?)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn original_root_constructor_consumes_its_exclusive_account_borrow() {
        let _: for<'b, 'work> fn(
            RootCompleted<'b, 'work>,
        )
            -> Result<ParentCompilerExecutionReadinessCustodyV3<'b, 'work>> =
            ParentCompilerExecutionReadinessCustodyV3::from_original_root;
    }

    #[test]
    fn root_admission_quote_covers_original_inputs_policy_and_readiness() {
        type Readiness = ParentCompilerExecutionReadinessCustodyV3<'static, 'static>;
        assert_eq!(
            Readiness::ROOT_ADMISSION_WORK,
            RootEvidence::REVALIDATION_WORK
                + POLICY_WORK
                + 2 * Policy::IO_WORK
                + Profile::IO_WORK
                + 2 * LOCAL_WORK
                + MANIFEST_WORK
                + READY_WORK
        );
        assert!(
            Readiness::ROOT_ADMISSION_SCRATCH
                >= RootEvidence::REVALIDATION_SCRATCH
                    + Readiness::OWNER_STORAGE
                    + Policy::FILE_STORAGE
        );
    }
}
