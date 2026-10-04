use super::{DeploymentVerificationErrorKindV1, DeploymentVerificationErrorV1, invalid};

const QUALIFICATION_FAULT_POINTS_V1: [QualificationFaultPointV1; 28] = [
    QualificationFaultPointV1::LoopAttached,
    QualificationFaultPointV1::BaseMounted,
    QualificationFaultPointV1::OverlayMounted,
    QualificationFaultPointV1::ProjectionRevalidated,
    QualificationFaultPointV1::SystemdVersionComplete,
    QualificationFaultPointV1::SystemdVersionRevalidated,
    QualificationFaultPointV1::SystemdSysusersComplete,
    QualificationFaultPointV1::SystemdSysusersRevalidated,
    QualificationFaultPointV1::SystemdTmpfilesComplete,
    QualificationFaultPointV1::SystemdTmpfilesRevalidated,
    QualificationFaultPointV1::SystemdUnitVerifyComplete,
    QualificationFaultPointV1::SystemdUnitVerifyRevalidated,
    QualificationFaultPointV1::SystemdPostconditionsAdmitted,
    QualificationFaultPointV1::InstalledLowerRevalidated,
    QualificationFaultPointV1::CompilerExecutionProvisioningComplete,
    QualificationFaultPointV1::CompilerExecutionProvisioningRevalidated,
    QualificationFaultPointV1::CompilerExecutionProvisioningAdmitted,
    QualificationFaultPointV1::SystemdMachineSpawned,
    QualificationFaultPointV1::SupervisorSocketMetadataAdmitted,
    QualificationFaultPointV1::ClientTransactionComplete,
    QualificationFaultPointV1::ClientTransactionRevalidated,
    QualificationFaultPointV1::SystemdMachineReady,
    QualificationFaultPointV1::SystemdMachineStopped,
    QualificationFaultPointV1::PostBootLowerRevalidated,
    QualificationFaultPointV1::OverlayUnmounted,
    QualificationFaultPointV1::BaseUnmounted,
    QualificationFaultPointV1::LoopReleased,
    QualificationFaultPointV1::StagingCleaned,
];

/// Fixed post-transition interruption point in the sole qualification transaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QualificationFaultPointV1 {
    /// The sealed base has been atomically attached to a read-only autoclear loop device.
    LoopAttached,
    /// The detached SquashFS mount has been attached to the retained base mount point.
    BaseMounted,
    /// The detached OverlayFS mount has been attached to the retained root mount point.
    OverlayMounted,
    /// Both mounts and the complete installed deployment projection have been revalidated.
    ProjectionRevalidated,
    /// The pinned systemd version command completed and its output was admitted.
    SystemdVersionComplete,
    /// Mount custody and the installed lower were revalidated after the version command.
    SystemdVersionRevalidated,
    /// The pinned systemd-sysusers command completed without standard output.
    SystemdSysusersComplete,
    /// Mount custody and the installed lower were revalidated after systemd-sysusers.
    SystemdSysusersRevalidated,
    /// The pinned systemd-tmpfiles command completed without standard output.
    SystemdTmpfilesComplete,
    /// Mount custody and the installed lower were revalidated after systemd-tmpfiles.
    SystemdTmpfilesRevalidated,
    /// The pinned offline systemd unit verifier completed without standard output.
    SystemdUnitVerifyComplete,
    /// Mount custody and the installed lower were revalidated after unit verification.
    SystemdUnitVerifyRevalidated,
    /// Exact account databases and tmpfiles-created objects passed admission.
    SystemdPostconditionsAdmitted,
    /// Mount custody and the installed lower passed the final preflight revalidation.
    InstalledLowerRevalidated,
    /// The production compiler-execution provisioner completed with no output.
    CompilerExecutionProvisioningComplete,
    /// Mount custody and the installed lower were revalidated after provisioning.
    CompilerExecutionProvisioningRevalidated,
    /// Generation, identities, key seeds, records, and executable measurements were admitted.
    CompilerExecutionProvisioningAdmitted,
    /// The exact pinned machine helper was spawned with retained descriptor custody.
    SystemdMachineSpawned,
    /// The isolated machine published the exact supervisor socket without consuming a session.
    SupervisorSocketMetadataAdmitted,
    /// The real distinct-UID client completed Recover followed by the required Cancel.
    ClientTransactionComplete,
    /// Client evidence, socket custody, provisioning, and the installed lower were revalidated.
    ClientTransactionRevalidated,
    /// The isolated machine completed the authenticated client transaction and revalidation.
    SystemdMachineReady,
    /// The isolated systemd machine completed bounded graceful shutdown.
    SystemdMachineStopped,
    /// Provisioning state, mount custody, and the installed lower passed revalidation after shutdown.
    PostBootLowerRevalidated,
    /// The disposable OverlayFS root has been unmounted.
    OverlayUnmounted,
    /// The read-only SquashFS base has been unmounted.
    BaseUnmounted,
    /// Loop-device custody has been released after both mounts were removed.
    LoopReleased,
    /// The exact staging tree has been removed and its parent synchronized.
    StagingCleaned,
}

impl QualificationFaultPointV1 {
    /// Returns all V1 points in exact transaction order.
    pub const fn all() -> &'static [Self] {
        &QUALIFICATION_FAULT_POINTS_V1
    }

    /// Returns the stable command-line and evidence spelling.
    pub const fn canonical_name(self) -> &'static str {
        match self {
            Self::LoopAttached => "loop-attached",
            Self::BaseMounted => "base-mounted",
            Self::OverlayMounted => "overlay-mounted",
            Self::ProjectionRevalidated => "projection-revalidated",
            Self::SystemdVersionComplete => "systemd-version-complete",
            Self::SystemdVersionRevalidated => "systemd-version-revalidated",
            Self::SystemdSysusersComplete => "systemd-sysusers-complete",
            Self::SystemdSysusersRevalidated => "systemd-sysusers-revalidated",
            Self::SystemdTmpfilesComplete => "systemd-tmpfiles-complete",
            Self::SystemdTmpfilesRevalidated => "systemd-tmpfiles-revalidated",
            Self::SystemdUnitVerifyComplete => "systemd-unit-verify-complete",
            Self::SystemdUnitVerifyRevalidated => "systemd-unit-verify-revalidated",
            Self::SystemdPostconditionsAdmitted => "systemd-postconditions-admitted",
            Self::InstalledLowerRevalidated => "installed-lower-revalidated",
            Self::CompilerExecutionProvisioningComplete => {
                "compiler-execution-provisioning-complete"
            }
            Self::CompilerExecutionProvisioningRevalidated => {
                "compiler-execution-provisioning-revalidated"
            }
            Self::CompilerExecutionProvisioningAdmitted => {
                "compiler-execution-provisioning-admitted"
            }
            Self::SystemdMachineSpawned => "systemd-machine-spawned",
            Self::SupervisorSocketMetadataAdmitted => "supervisor-socket-metadata-admitted",
            Self::ClientTransactionComplete => "client-transaction-complete",
            Self::ClientTransactionRevalidated => "client-transaction-revalidated",
            Self::SystemdMachineReady => "systemd-machine-ready",
            Self::SystemdMachineStopped => "systemd-machine-stopped",
            Self::PostBootLowerRevalidated => "post-boot-lower-revalidated",
            Self::OverlayUnmounted => "overlay-unmounted",
            Self::BaseUnmounted => "base-unmounted",
            Self::LoopReleased => "loop-released",
            Self::StagingCleaned => "staging-cleaned",
        }
    }

    /// Parses one exact canonical name and rejects aliases.
    pub fn from_canonical_name(name: &str) -> Option<Self> {
        Self::all()
            .iter()
            .copied()
            .find(|point| point.canonical_name() == name)
    }
}

pub(super) trait QualificationFaultHooksV1 {
    fn checkpoint(
        &mut self,
        point: QualificationFaultPointV1,
    ) -> Result<(), DeploymentVerificationErrorV1>;
}

pub(super) struct NoQualificationFaultV1;

impl QualificationFaultHooksV1 for NoQualificationFaultV1 {
    fn checkpoint(
        &mut self,
        _point: QualificationFaultPointV1,
    ) -> Result<(), DeploymentVerificationErrorV1> {
        Ok(())
    }
}

pub(super) struct InjectQualificationFaultV1 {
    point: QualificationFaultPointV1,
    fired: bool,
}

impl InjectQualificationFaultV1 {
    pub(super) const fn new(point: QualificationFaultPointV1) -> Self {
        Self {
            point,
            fired: false,
        }
    }

    pub(super) const fn fired(&self) -> bool {
        self.fired
    }
}

impl QualificationFaultHooksV1 for InjectQualificationFaultV1 {
    fn checkpoint(
        &mut self,
        point: QualificationFaultPointV1,
    ) -> Result<(), DeploymentVerificationErrorV1> {
        if !self.fired && point == self.point {
            self.fired = true;
            return Err(invalid(
                DeploymentVerificationErrorKindV1::InjectedFailure,
                format!(
                    "injected qualification interruption at {}",
                    point.canonical_name()
                ),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qualification_fault_points_are_closed_canonical_and_single_shot() {
        let names = QualificationFaultPointV1::all()
            .iter()
            .map(|point| point.canonical_name())
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            vec![
                "loop-attached",
                "base-mounted",
                "overlay-mounted",
                "projection-revalidated",
                "systemd-version-complete",
                "systemd-version-revalidated",
                "systemd-sysusers-complete",
                "systemd-sysusers-revalidated",
                "systemd-tmpfiles-complete",
                "systemd-tmpfiles-revalidated",
                "systemd-unit-verify-complete",
                "systemd-unit-verify-revalidated",
                "systemd-postconditions-admitted",
                "installed-lower-revalidated",
                "compiler-execution-provisioning-complete",
                "compiler-execution-provisioning-revalidated",
                "compiler-execution-provisioning-admitted",
                "systemd-machine-spawned",
                "supervisor-socket-metadata-admitted",
                "client-transaction-complete",
                "client-transaction-revalidated",
                "systemd-machine-ready",
                "systemd-machine-stopped",
                "post-boot-lower-revalidated",
                "overlay-unmounted",
                "base-unmounted",
                "loop-released",
                "staging-cleaned",
            ]
        );
        assert_eq!(
            QualificationFaultPointV1::from_canonical_name("systemd-tmpfiles-complete"),
            Some(QualificationFaultPointV1::SystemdTmpfilesComplete)
        );
        assert_eq!(
            QualificationFaultPointV1::from_canonical_name("SYSTEMD-TMPFILES-COMPLETE"),
            None
        );

        for selected in QualificationFaultPointV1::all() {
            let mut hooks = InjectQualificationFaultV1::new(*selected);
            let mut failures = 0;
            for observed in QualificationFaultPointV1::all() {
                failures += usize::from(hooks.checkpoint(*observed).is_err());
            }
            assert_eq!(failures, 1);
            assert!(hooks.fired());
        }
    }
}
