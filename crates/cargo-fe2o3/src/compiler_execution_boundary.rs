use std::error::Error;
use std::fmt;
use std::process::Command;
use std::time::Duration;

use fe2o3_compiler_closure_capability::{
    CompilerExecutionClientProfileCapabilityV1, CompilerExecutionPolicyCapabilityV1,
};
use fe2o3_compiler_execution_client::{
    CompilerExecutionChildChannelErrorV1, CompilerExecutionHandoffErrorV1,
    CompilerExecutionSupervisorCredentialsV1, MAX_COMPILER_EXECUTION_SUPERVISOR_HANDOFF_TIMEOUT_V1,
    PendingCompilerExecutionChildChannelV1,
};
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionClientProfileIdentityV1, CompilerExecutionReceiptCarriageV1,
    CompilerExecutionReceiptPublicationErrorV1, CompilerExecutionServiceLaunchManifestV1,
    CompilerExecutionServiceReadyV1,
};

const CHILD_CHANNEL_TIMEOUT: Duration = Duration::from_secs(30);
const SUPERVISOR_HANDOFF_TIMEOUT: Duration = MAX_COMPILER_EXECUTION_SUPERVISOR_HANDOFF_TIMEOUT_V1;
const SUPERVISOR_READINESS_TIMEOUT: Duration = MAX_COMPILER_EXECUTION_SUPERVISOR_HANDOFF_TIMEOUT_V1;

/// One prepared, single-use path from the selected child to the fixed supervisor.
pub(crate) struct PreparedCompilerExecutionBoundaryV1 {
    profile: CompilerExecutionClientProfileCapabilityV1,
    policy: CompilerExecutionPolicyCapabilityV1,
    child_channel: PendingCompilerExecutionChildChannelV1,
}

impl PreparedCompilerExecutionBoundaryV1 {
    /// Prepares a compiler child that must inherit the exact issuer policy at FD 202.
    pub(crate) fn prepare(
        source_profile: &CompilerExecutionClientProfileCapabilityV1,
        command: &mut Command,
    ) -> Result<Self, CompilerExecutionBoundaryErrorV1> {
        Self::prepare_inner(source_profile, command, ChildPolicyExposureV1::Inherit)
    }

    /// Prepares an application verifier without exposing the issuer-policy capability.
    pub(crate) fn prepare_application_verifier(
        source_profile: &CompilerExecutionClientProfileCapabilityV1,
        command: &mut Command,
    ) -> Result<Self, CompilerExecutionBoundaryErrorV1> {
        Self::prepare_inner(
            source_profile,
            command,
            ChildPolicyExposureV1::RetainInParent,
        )
    }

    fn prepare_inner(
        source_profile: &CompilerExecutionClientProfileCapabilityV1,
        command: &mut Command,
        policy_exposure: ChildPolicyExposureV1,
    ) -> Result<Self, CompilerExecutionBoundaryErrorV1> {
        source_profile
            .revalidate()
            .map_err(CompilerExecutionBoundaryErrorV1::Profile)?;
        let profile = CompilerExecutionClientProfileCapabilityV1::from_file(
            source_profile
                .try_clone_for_transfer()
                .map_err(CompilerExecutionBoundaryErrorV1::Profile)?,
        )
        .map_err(CompilerExecutionBoundaryErrorV1::Profile)?;
        if profile.profile() != source_profile.profile() {
            return Err(CompilerExecutionBoundaryErrorV1::Evidence(
                "retained compiler-execution profile differs from broker custody".to_owned(),
            ));
        }

        let policy =
            CompilerExecutionPolicyCapabilityV1::create(profile.profile().policy().clone())
                .map_err(CompilerExecutionBoundaryErrorV1::Policy)?;
        if policy_exposure == ChildPolicyExposureV1::Inherit {
            policy
                .inherit_for_child(command)
                .map_err(CompilerExecutionBoundaryErrorV1::Policy)?;
        }
        let child_channel = PendingCompilerExecutionChildChannelV1::prepare(command)
            .map_err(CompilerExecutionBoundaryErrorV1::ChildChannel)?;

        Ok(Self {
            profile,
            policy,
            child_channel,
        })
    }

    pub(crate) fn finish(
        self,
        child_pid: u32,
    ) -> Result<ParentCompilerExecutionReadinessCustodyV1, CompilerExecutionBoundaryErrorV1> {
        let Self {
            profile,
            policy,
            child_channel,
        } = self;
        let launch = child_channel
            .finish(child_pid, CHILD_CHANNEL_TIMEOUT)
            .map_err(CompilerExecutionBoundaryErrorV1::ChildChannel)?;
        let supervisor = CompilerExecutionSupervisorCredentialsV1::new(
            profile.profile().supervisor_uid(),
            profile.profile().supervisor_gid(),
        )
        .map_err(CompilerExecutionBoundaryErrorV1::SupervisorCredentials)?;
        let pending = launch
            .transfer_to_supervisor(profile.profile(), SUPERVISOR_HANDOFF_TIMEOUT)
            .map_err(CompilerExecutionBoundaryErrorV1::SupervisorTransfer)?;
        let manifest = pending.manifest().clone();
        let readiness = pending
            .await_readiness(profile.profile(), SUPERVISOR_READINESS_TIMEOUT)
            .map_err(CompilerExecutionBoundaryErrorV1::SupervisorReadiness)?;

        ParentCompilerExecutionReadinessCustodyV1::admit(
            profile, policy, supervisor, child_pid, manifest, readiness,
        )
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum ChildPolicyExposureV1 {
    Inherit,
    RetainInParent,
}

/// Move-only parent custody proving that one exact selected child reached a ready issuer.
///
/// The value contains public, inert evidence and sealed public trust configuration only. It grants
/// no compiler, signing, linking, publication, loading, launch, or execution authority.
pub(crate) struct ParentCompilerExecutionReadinessCustodyV1 {
    profile: CompilerExecutionClientProfileCapabilityV1,
    policy: CompilerExecutionPolicyCapabilityV1,
    supervisor: CompilerExecutionSupervisorCredentialsV1,
    child_pid: u32,
    manifest: CompilerExecutionServiceLaunchManifestV1,
    readiness: CompilerExecutionServiceReadyV1,
}

impl ParentCompilerExecutionReadinessCustodyV1 {
    fn admit(
        profile: CompilerExecutionClientProfileCapabilityV1,
        policy: CompilerExecutionPolicyCapabilityV1,
        supervisor: CompilerExecutionSupervisorCredentialsV1,
        child_pid: u32,
        manifest: CompilerExecutionServiceLaunchManifestV1,
        readiness: CompilerExecutionServiceReadyV1,
    ) -> Result<Self, CompilerExecutionBoundaryErrorV1> {
        let custody = Self {
            profile,
            policy,
            supervisor,
            child_pid,
            manifest,
            readiness,
        };
        custody.revalidate()?;
        Ok(custody)
    }

    pub(crate) fn revalidate(&self) -> Result<(), CompilerExecutionBoundaryErrorV1> {
        self.profile
            .revalidate()
            .map_err(CompilerExecutionBoundaryErrorV1::Profile)?;
        self.policy
            .revalidate()
            .map_err(CompilerExecutionBoundaryErrorV1::Policy)?;

        let profile = self.profile.profile();
        if profile.policy() != self.policy.policy() {
            return Err(CompilerExecutionBoundaryErrorV1::Evidence(
                "retained compiler-execution policy differs from the fixed client profile"
                    .to_owned(),
            ));
        }
        if profile.supervisor_uid() != self.supervisor.uid()
            || profile.supervisor_gid() != self.supervisor.gid()
        {
            return Err(CompilerExecutionBoundaryErrorV1::Evidence(
                "retained supervisor credentials differ from the fixed client profile".to_owned(),
            ));
        }

        let canonical_manifest =
            CompilerExecutionServiceLaunchManifestV1::decode(self.manifest.canonical_bytes())
                .map_err(|error| {
                    CompilerExecutionBoundaryErrorV1::Evidence(format!(
                        "retained launch manifest is not canonical: {error}"
                    ))
                })?;
        if canonical_manifest != self.manifest
            || self.child_pid == 0
            || self.manifest.client().pid() != self.child_pid
            || !self.manifest.matches_policy(self.policy.policy())
            || !self
                .manifest
                .matches_external_anchor_service(profile.external_anchor_service())
        {
            return Err(CompilerExecutionBoundaryErrorV1::Evidence(
                "retained launch manifest differs from the selected child, anchor service, or policy"
                    .to_owned(),
            ));
        }
        if self.manifest.client().uid() == self.supervisor.uid() {
            return Err(CompilerExecutionBoundaryErrorV1::Evidence(
                "selected child and protected supervisor do not have distinct UIDs".to_owned(),
            ));
        }

        let canonical_readiness = CompilerExecutionServiceReadyV1::decode(
            self.readiness.canonical_bytes(),
        )
        .map_err(|error| {
            CompilerExecutionBoundaryErrorV1::Evidence(format!(
                "retained supervisor readiness is not canonical: {error}"
            ))
        })?;
        if canonical_readiness != self.readiness
            || !self.readiness.matches_launch(
                self.readiness.issuer_pid(),
                &self.manifest,
                self.policy.policy(),
            )
        {
            return Err(CompilerExecutionBoundaryErrorV1::Evidence(
                "retained supervisor readiness differs from the launch manifest or policy"
                    .to_owned(),
            ));
        }
        Ok(())
    }

    pub(crate) fn retain_through<T>(self, operation: impl FnOnce(&Self) -> T) -> T {
        operation(&self)
    }

    pub(crate) const fn profile_identity(&self) -> CompilerExecutionClientProfileIdentityV1 {
        self.profile.profile().identity()
    }

    pub(crate) const fn grants_compiler_authority(&self) -> bool {
        false
    }

    pub(crate) fn admit_receipt_transport(
        &self,
        subject: &fe2o3_artifact_transaction::InertCompilerExecutionSubjectV1,
        transport: fe2o3_artifact_transaction::RecoveredCompilerExecutionReceiptTransportV1,
    ) -> Result<CompilerExecutionReceiptCarriageV1, CompilerExecutionBoundaryErrorV1> {
        self.revalidate()?;
        let carriage =
            admit_compiler_execution_receipt_transport(&self.profile, subject, transport)?;
        self.revalidate()?;
        Ok(carriage)
    }
}

pub(crate) fn admit_compiler_execution_receipt_transport(
    profile: &CompilerExecutionClientProfileCapabilityV1,
    subject: &fe2o3_artifact_transaction::InertCompilerExecutionSubjectV1,
    transport: fe2o3_artifact_transaction::RecoveredCompilerExecutionReceiptTransportV1,
) -> Result<CompilerExecutionReceiptCarriageV1, CompilerExecutionBoundaryErrorV1> {
    profile
        .revalidate()
        .map_err(CompilerExecutionBoundaryErrorV1::Profile)?;
    let receipt = transport.receipt();
    if receipt.subject() != subject.identity()
        || receipt.length() != transport.exact_bytes().len()
        || receipt.grants_compiler_authority()
        || receipt.grants_publication_authority()
        || receipt.grants_load_authority()
        || receipt.grants_launch_authority()
    {
        return Err(CompilerExecutionBoundaryErrorV1::Evidence(
            "compiler-execution receipt transport changed its exact subject or byte length"
                .to_owned(),
        ));
    }
    let carriage = CompilerExecutionReceiptCarriageV1::decode(transport.exact_bytes())
        .map_err(CompilerExecutionBoundaryErrorV1::Receipt)?;
    validate_compiler_execution_receipt_carriage(profile, subject, &carriage)?;
    Ok(carriage)
}

pub(crate) fn validate_compiler_execution_receipt_carriage(
    profile: &CompilerExecutionClientProfileCapabilityV1,
    subject: &fe2o3_artifact_transaction::InertCompilerExecutionSubjectV1,
    carriage: &CompilerExecutionReceiptCarriageV1,
) -> Result<(), CompilerExecutionBoundaryErrorV1> {
    profile
        .revalidate()
        .map_err(CompilerExecutionBoundaryErrorV1::Profile)?;
    if carriage.policy() != profile.profile().policy()
        || carriage.request().subject() != subject
        || carriage.canonical_bytes().is_empty()
        || carriage.grants_compiler_authority()
        || carriage.grants_load_authority()
        || carriage.grants_launch_authority()
    {
        return Err(CompilerExecutionBoundaryErrorV1::Evidence(
            "compiler-execution receipt differs from the exact subject or sealed client profile"
                .to_owned(),
        ));
    }
    let canonical = CompilerExecutionReceiptCarriageV1::decode(carriage.canonical_bytes())
        .map_err(CompilerExecutionBoundaryErrorV1::Receipt)?;
    if &canonical != carriage {
        return Err(CompilerExecutionBoundaryErrorV1::Evidence(
            "compiler-execution receipt carriage is not canonical".to_owned(),
        ));
    }
    profile
        .revalidate()
        .map_err(CompilerExecutionBoundaryErrorV1::Profile)
}

#[derive(Debug)]
pub(crate) enum CompilerExecutionBoundaryErrorV1 {
    Profile(String),
    Policy(String),
    ChildChannel(CompilerExecutionChildChannelErrorV1),
    SupervisorCredentials(CompilerExecutionHandoffErrorV1),
    SupervisorTransfer(CompilerExecutionHandoffErrorV1),
    SupervisorReadiness(CompilerExecutionHandoffErrorV1),
    Receipt(CompilerExecutionReceiptPublicationErrorV1),
    Evidence(String),
}

impl CompilerExecutionBoundaryErrorV1 {
    pub(crate) const fn stage(&self) -> &'static str {
        match self {
            Self::Profile(_) => "client-profile retention",
            Self::Policy(_) => "issuer-policy installation",
            Self::ChildChannel(_) => "selected child-channel admission",
            Self::SupervisorCredentials(_) => "supervisor credential admission",
            Self::SupervisorTransfer(_) => "fixed supervisor transfer",
            Self::SupervisorReadiness(_) => "supervisor readiness",
            Self::Receipt(_) => "compiler receipt admission",
            Self::Evidence(_) => "readiness evidence admission",
        }
    }
}

impl fmt::Display for CompilerExecutionBoundaryErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} failed: ", self.stage())?;
        match self {
            Self::Profile(error) | Self::Policy(error) | Self::Evidence(error) => {
                formatter.write_str(error)
            }
            Self::ChildChannel(error) => error.fmt(formatter),
            Self::SupervisorCredentials(error)
            | Self::SupervisorTransfer(error)
            | Self::SupervisorReadiness(error) => error.fmt(formatter),
            Self::Receipt(error) => error.fmt(formatter),
        }
    }
}

impl Error for CompilerExecutionBoundaryErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ChildChannel(error) => Some(error),
            Self::SupervisorCredentials(error)
            | Self::SupervisorTransfer(error)
            | Self::SupervisorReadiness(error) => Some(error),
            Self::Receipt(error) => Some(error),
            Self::Profile(_) | Self::Policy(_) | Self::Evidence(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::process::CommandExt;
    use std::sync::Mutex;

    use ed25519_dalek::SigningKey;
    use fe2o3_compiler_closure_capability::COMPILER_EXECUTION_POLICY_CHILD_FD_V1;
    use fe2o3_compiler_execution_client::COMPILER_EXECUTION_SERVICE_CHILD_FD_V1;
    use fe2o3_compiler_execution_protocol::{
        COMPILER_EXECUTION_ISSUER_POLICY_BYTES_V1, CompilerExecutionClientProcessIdentityV1,
        CompilerExecutionExternalAnchorServiceIdentityV1, CompilerExecutionIssuerMeasurementV1,
        CompilerExecutionIssuerPolicyV1,
    };

    use super::*;

    static RESERVED_CHILD_FD_LOCK: Mutex<()> = Mutex::new(());

    fn policy(seed: u8) -> CompilerExecutionIssuerPolicyV1 {
        CompilerExecutionIssuerPolicyV1::new(
            u64::from(seed),
            CompilerExecutionIssuerMeasurementV1::new([seed + 1; 32], 123).unwrap(),
            CompilerExecutionIssuerMeasurementV1::new([seed + 2; 32], 456).unwrap(),
            SigningKey::from_bytes(&[seed; 32])
                .verifying_key()
                .to_bytes(),
            SigningKey::from_bytes(&[seed.wrapping_add(1); 32])
                .verifying_key()
                .to_bytes(),
        )
        .unwrap()
    }

    fn client_profile(seed: u8, supervisor_uid: u32) -> CompilerExecutionClientProfileCapabilityV1 {
        CompilerExecutionClientProfileCapabilityV1::create(
            fe2o3_compiler_execution_protocol::CompilerExecutionClientProfileV1::new(
                supervisor_uid,
                5_678,
                external_anchor_service(),
                policy(seed),
            )
            .unwrap(),
        )
        .unwrap()
    }

    fn evidence(
        policy: &CompilerExecutionIssuerPolicyV1,
        child_pid: u32,
        child_uid: u32,
    ) -> (
        CompilerExecutionServiceLaunchManifestV1,
        CompilerExecutionServiceReadyV1,
    ) {
        let manifest = CompilerExecutionServiceLaunchManifestV1::new(
            CompilerExecutionClientProcessIdentityV1::new(child_pid, child_uid, 1_000).unwrap(),
            external_anchor_service(),
            policy,
        );
        let readiness = CompilerExecutionServiceReadyV1::new(9_999, &manifest, policy).unwrap();
        (manifest, readiness)
    }

    fn external_anchor_service() -> CompilerExecutionExternalAnchorServiceIdentityV1 {
        CompilerExecutionExternalAnchorServiceIdentityV1::new(6_000, 7_000).unwrap()
    }

    fn admit(
        profile: CompilerExecutionClientProfileCapabilityV1,
        child_pid: u32,
        child_uid: u32,
    ) -> Result<ParentCompilerExecutionReadinessCustodyV1, CompilerExecutionBoundaryErrorV1> {
        let retained_policy = profile.profile().policy().clone();
        let policy = CompilerExecutionPolicyCapabilityV1::create(retained_policy.clone()).unwrap();
        let supervisor = CompilerExecutionSupervisorCredentialsV1::new(
            profile.profile().supervisor_uid(),
            profile.profile().supervisor_gid(),
        )
        .unwrap();
        let (manifest, readiness) = evidence(&retained_policy, child_pid, child_uid);
        ParentCompilerExecutionReadinessCustodyV1::admit(
            profile, policy, supervisor, child_pid, manifest, readiness,
        )
    }

    #[test]
    fn exact_readiness_is_retained_and_revalidated_without_authority() {
        let custody = admit(client_profile(7, 1_234), 8_765, 1_000).unwrap();
        custody.revalidate().unwrap();
        assert!(!custody.grants_compiler_authority());
        assert_ne!(custody.profile_identity().as_bytes(), &[0; 32]);
    }

    #[test]
    fn preparation_installs_exact_policy_and_child_created_service_channel() {
        let _guard = RESERVED_CHILD_FD_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let source_profile = client_profile(7, 1_234);
        let mut command = Command::new("/bin/sh");
        command.arg("-c").arg(format!(
            "test \"$(wc -c </proc/self/fd/{COMPILER_EXECUTION_POLICY_CHILD_FD_V1})\" -eq {COMPILER_EXECUTION_ISSUER_POLICY_BYTES_V1} && test \"$(stat -Lc %F /proc/self/fd/{COMPILER_EXECUTION_SERVICE_CHILD_FD_V1})\" = socket && sleep 2"
        ));
        let prepared =
            PreparedCompilerExecutionBoundaryV1::prepare(&source_profile, &mut command).unwrap();
        let PreparedCompilerExecutionBoundaryV1 {
            profile,
            policy,
            child_channel,
        } = prepared;
        let mut child = command.spawn().unwrap();
        let child_pid = child.id();
        let launch = child_channel
            .finish(child_pid, Duration::from_secs(5))
            .unwrap();
        assert_eq!(launch.client().pid(), child_pid);
        assert_eq!(launch.submitter().pid(), std::process::id());
        drop(launch);
        assert!(child.wait().unwrap().success());
        profile.revalidate().unwrap();
        policy.revalidate().unwrap();
    }

    #[test]
    fn application_verifier_gets_service_channel_without_policy_capability() {
        let _guard = RESERVED_CHILD_FD_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let source_profile = client_profile(8, 1_234);
        let mut command = Command::new("/bin/sh");
        command.arg("-c").arg(format!(
            "test ! -e /proc/self/fd/{COMPILER_EXECUTION_POLICY_CHILD_FD_V1} && test \"$(stat -Lc %F /proc/self/fd/{COMPILER_EXECUTION_SERVICE_CHILD_FD_V1})\" = socket && sleep 2"
        ));
        let prepared = PreparedCompilerExecutionBoundaryV1::prepare_application_verifier(
            &source_profile,
            &mut command,
        )
        .unwrap();
        // SAFETY: this models the production callback registered after child-channel creation.
        unsafe {
            command.pre_exec(|| {
                crate::application_exec::protect_all_nonstdio_descriptors()?;
                crate::application_exec::validate_and_expose_connected_seqpacket_descriptor(
                    COMPILER_EXECUTION_SERVICE_CHILD_FD_V1,
                )
            });
        }
        let PreparedCompilerExecutionBoundaryV1 {
            profile,
            policy,
            child_channel,
        } = prepared;
        let mut child = command.spawn().unwrap();
        let child_pid = child.id();
        let launch = child_channel
            .finish(child_pid, Duration::from_secs(5))
            .unwrap();
        assert_eq!(launch.client().pid(), child_pid);
        drop(launch);
        assert!(child.wait().unwrap().success());
        profile.revalidate().unwrap();
        policy.revalidate().unwrap();
    }

    #[test]
    fn selected_child_and_supervisor_must_have_distinct_exact_identities() {
        assert!(admit(client_profile(7, 1_234), 8_765, 1_234).is_err());

        let profile = client_profile(7, 1_234);
        let retained_policy = profile.profile().policy().clone();
        let policy = CompilerExecutionPolicyCapabilityV1::create(retained_policy.clone()).unwrap();
        let supervisor = CompilerExecutionSupervisorCredentialsV1::new(1_235, 5_678).unwrap();
        let (manifest, readiness) = evidence(&retained_policy, 8_765, 1_000);
        assert!(
            ParentCompilerExecutionReadinessCustodyV1::admit(
                profile, policy, supervisor, 8_765, manifest, readiness,
            )
            .is_err()
        );
    }

    #[test]
    fn policy_and_readiness_substitutions_fail_closed() {
        let profile = client_profile(7, 1_234);
        let policy = CompilerExecutionPolicyCapabilityV1::create(policy(8)).unwrap();
        let supervisor = CompilerExecutionSupervisorCredentialsV1::new(1_234, 5_678).unwrap();
        let (manifest, readiness) = evidence(profile.profile().policy(), 8_765, 1_000);
        assert!(
            ParentCompilerExecutionReadinessCustodyV1::admit(
                profile, policy, supervisor, 8_765, manifest, readiness,
            )
            .is_err()
        );

        let profile = client_profile(7, 1_234);
        let retained_policy = profile.profile().policy().clone();
        let policy = CompilerExecutionPolicyCapabilityV1::create(retained_policy.clone()).unwrap();
        let supervisor = CompilerExecutionSupervisorCredentialsV1::new(1_234, 5_678).unwrap();
        let (manifest, _) = evidence(&retained_policy, 8_765, 1_000);
        let (_, substituted_readiness) = evidence(&retained_policy, 8_766, 1_000);
        assert!(
            ParentCompilerExecutionReadinessCustodyV1::admit(
                profile,
                policy,
                supervisor,
                8_765,
                manifest,
                substituted_readiness,
            )
            .is_err()
        );

        let profile = client_profile(7, 1_234);
        let retained_policy = profile.profile().policy().clone();
        let policy = CompilerExecutionPolicyCapabilityV1::create(retained_policy.clone()).unwrap();
        let supervisor = CompilerExecutionSupervisorCredentialsV1::new(1_234, 5_678).unwrap();
        let substituted_manifest = CompilerExecutionServiceLaunchManifestV1::new(
            CompilerExecutionClientProcessIdentityV1::new(8_765, 1_000, 1_000).unwrap(),
            CompilerExecutionExternalAnchorServiceIdentityV1::new(6_001, 7_001).unwrap(),
            &retained_policy,
        );
        let substituted_readiness =
            CompilerExecutionServiceReadyV1::new(9_999, &substituted_manifest, &retained_policy)
                .unwrap();
        assert!(
            ParentCompilerExecutionReadinessCustodyV1::admit(
                profile,
                policy,
                supervisor,
                8_765,
                substituted_manifest,
                substituted_readiness,
            )
            .is_err()
        );
    }
}
