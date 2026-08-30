use std::error::Error;
use std::fmt;

use fe2o3_broker_authority_service::sealed_static_issuer_runtime_measurement_v1;
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionAttestationErrorV1, CompilerExecutionExternalAnchorDeploymentErrorV1,
    CompilerExecutionExternalAnchorDeploymentV1,
    CompilerExecutionExternalAnchorProvisioningErrorV1,
    CompilerExecutionExternalAnchorProvisioningV1,
    CompilerExecutionExternalAnchorServiceIdentityErrorV1,
    CompilerExecutionExternalAnchorServiceIdentityV1, CompilerExecutionIssuerMeasurementV1,
    CompilerExecutionIssuerPolicyV1, CompilerExecutionSupervisorDeploymentErrorV1,
    CompilerExecutionSupervisorDeploymentV1,
};

const EXECUTABLE_ROLES_V1: [&str; 5] = [
    "protected supervisor",
    "static pre-exec launcher",
    "compiler-execution issuer",
    "external-anchor provisioning helper",
    "external-anchor daemon",
];

/// Inert inputs for one complete compiler-execution deployment record set.
///
/// This value contains no signing seed, path, descriptor, process, publication, load, launch, or
/// GPU authority. Each executable measurement is assigned to exactly one fixed role.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompilerExecutionProvisioningInputsV1 {
    generation: u64,
    compiler_service_uid: u32,
    compiler_service_gid: u32,
    external_anchor_service: CompilerExecutionExternalAnchorServiceIdentityV1,
    supervisor: CompilerExecutionIssuerMeasurementV1,
    launcher: CompilerExecutionIssuerMeasurementV1,
    issuer: CompilerExecutionIssuerMeasurementV1,
    anchor_helper: CompilerExecutionIssuerMeasurementV1,
    anchor_daemon: CompilerExecutionIssuerMeasurementV1,
    issuer_verifying_key: [u8; 32],
    anchor_verifying_key: [u8; 32],
}

impl CompilerExecutionProvisioningInputsV1 {
    /// Constructs one role-complete inert input set.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        generation: u64,
        compiler_service_uid: u32,
        compiler_service_gid: u32,
        anchor_service_uid: u32,
        anchor_service_gid: u32,
        supervisor: CompilerExecutionIssuerMeasurementV1,
        launcher: CompilerExecutionIssuerMeasurementV1,
        issuer: CompilerExecutionIssuerMeasurementV1,
        anchor_helper: CompilerExecutionIssuerMeasurementV1,
        anchor_daemon: CompilerExecutionIssuerMeasurementV1,
        issuer_verifying_key: [u8; 32],
        anchor_verifying_key: [u8; 32],
    ) -> Result<Self, CompilerExecutionProvisioningErrorV1> {
        let external_anchor_service = CompilerExecutionExternalAnchorServiceIdentityV1::new(
            anchor_service_uid,
            anchor_service_gid,
        )
        .map_err(CompilerExecutionProvisioningErrorV1::ExternalAnchorServiceIdentity)?;
        let measurements = [supervisor, launcher, issuer, anchor_helper, anchor_daemon];
        for first in 0..measurements.len() {
            for second in first + 1..measurements.len() {
                if measurements[first] == measurements[second] {
                    return Err(
                        CompilerExecutionProvisioningErrorV1::AliasedExecutableMeasurements {
                            first: EXECUTABLE_ROLES_V1[first],
                            second: EXECUTABLE_ROLES_V1[second],
                        },
                    );
                }
            }
        }
        Ok(Self {
            generation,
            compiler_service_uid,
            compiler_service_gid,
            external_anchor_service,
            supervisor,
            launcher,
            issuer,
            anchor_helper,
            anchor_daemon,
            issuer_verifying_key,
            anchor_verifying_key,
        })
    }
}

/// Complete canonical public record set for one compiler-execution deployment.
///
/// The bundle is inert. It contains public configuration and measurements but no key seed,
/// descriptor, process, compiler, publication, load, launch, or GPU authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerExecutionProvisioningBundleV1 {
    policy: CompilerExecutionIssuerPolicyV1,
    supervisor: CompilerExecutionSupervisorDeploymentV1,
    anchor_deployment: CompilerExecutionExternalAnchorDeploymentV1,
    anchor_provisioning: CompilerExecutionExternalAnchorProvisioningV1,
}

impl CompilerExecutionProvisioningBundleV1 {
    /// Constructs the only complete canonical record graph from inert provisioning inputs.
    pub fn new(
        inputs: CompilerExecutionProvisioningInputsV1,
    ) -> Result<Self, CompilerExecutionProvisioningErrorV1> {
        let policy = CompilerExecutionIssuerPolicyV1::new(
            inputs.generation,
            inputs.issuer,
            sealed_static_issuer_runtime_measurement_v1(),
            inputs.issuer_verifying_key,
            inputs.anchor_verifying_key,
        )
        .map_err(CompilerExecutionProvisioningErrorV1::Policy)?;
        let supervisor = CompilerExecutionSupervisorDeploymentV1::new(
            inputs.compiler_service_uid,
            inputs.compiler_service_gid,
            inputs.external_anchor_service,
            inputs.supervisor,
            inputs.launcher,
            &policy,
        )
        .map_err(CompilerExecutionProvisioningErrorV1::SupervisorDeployment)?;
        let anchor_deployment = CompilerExecutionExternalAnchorDeploymentV1::new(
            &supervisor,
            &policy,
            inputs.anchor_daemon,
        )
        .map_err(CompilerExecutionProvisioningErrorV1::ExternalAnchorDeployment)?;
        let anchor_provisioning = CompilerExecutionExternalAnchorProvisioningV1::new(
            &anchor_deployment,
            inputs.anchor_helper,
        )
        .map_err(CompilerExecutionProvisioningErrorV1::ExternalAnchorProvisioning)?;
        Ok(Self {
            policy,
            supervisor,
            anchor_deployment,
            anchor_provisioning,
        })
    }

    /// Returns the canonical issuer policy.
    pub const fn policy(&self) -> &CompilerExecutionIssuerPolicyV1 {
        &self.policy
    }

    /// Returns the canonical protected-supervisor deployment record.
    pub const fn supervisor(&self) -> &CompilerExecutionSupervisorDeploymentV1 {
        &self.supervisor
    }

    /// Returns the canonical external-anchor deployment record.
    pub const fn anchor_deployment(&self) -> &CompilerExecutionExternalAnchorDeploymentV1 {
        &self.anchor_deployment
    }

    /// Returns the canonical external-anchor provisioning record.
    pub const fn anchor_provisioning(&self) -> &CompilerExecutionExternalAnchorProvisioningV1 {
        &self.anchor_provisioning
    }
}

/// Stable rejection while constructing the complete public deployment record graph.
#[derive(Debug)]
#[non_exhaustive]
pub enum CompilerExecutionProvisioningErrorV1 {
    /// The external-anchor UID/GID is privileged or outside the canonical service profile.
    ExternalAnchorServiceIdentity(CompilerExecutionExternalAnchorServiceIdentityErrorV1),
    /// Two executable roles were assigned the same exact content measurement.
    AliasedExecutableMeasurements {
        /// First conflicting role.
        first: &'static str,
        /// Second conflicting role.
        second: &'static str,
    },
    /// The issuer policy is not canonical.
    Policy(CompilerExecutionAttestationErrorV1),
    /// The protected-supervisor deployment is not canonical.
    SupervisorDeployment(CompilerExecutionSupervisorDeploymentErrorV1),
    /// The external-anchor deployment is not canonical.
    ExternalAnchorDeployment(CompilerExecutionExternalAnchorDeploymentErrorV1),
    /// The external-anchor provisioning record is not canonical.
    ExternalAnchorProvisioning(CompilerExecutionExternalAnchorProvisioningErrorV1),
}

impl fmt::Display for CompilerExecutionProvisioningErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExternalAnchorServiceIdentity(error) => {
                write!(
                    formatter,
                    "invalid external-anchor service identity: {error}"
                )
            }
            Self::AliasedExecutableMeasurements { first, second } => {
                write!(formatter, "{first} and {second} have the same measurement")
            }
            Self::Policy(error) => write!(formatter, "invalid issuer policy: {error}"),
            Self::SupervisorDeployment(error) => {
                write!(formatter, "invalid supervisor deployment: {error}")
            }
            Self::ExternalAnchorDeployment(error) => {
                write!(formatter, "invalid external-anchor deployment: {error}")
            }
            Self::ExternalAnchorProvisioning(error) => {
                write!(formatter, "invalid external-anchor provisioning: {error}")
            }
        }
    }
}

impl Error for CompilerExecutionProvisioningErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ExternalAnchorServiceIdentity(error) => Some(error),
            Self::AliasedExecutableMeasurements { .. } => None,
            Self::Policy(error) => Some(error),
            Self::SupervisorDeployment(error) => Some(error),
            Self::ExternalAnchorDeployment(error) => Some(error),
            Self::ExternalAnchorProvisioning(error) => Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::SigningKey;
    use fe2o3_compiler_execution_protocol::{
        CompilerExecutionExternalAnchorDeploymentV1, CompilerExecutionExternalAnchorProvisioningV1,
        CompilerExecutionIssuerPolicyV1, CompilerExecutionSupervisorDeploymentV1,
    };

    use super::*;

    #[test]
    fn complete_bundle_is_deterministic_canonical_and_cross_bound() {
        let first = CompilerExecutionProvisioningBundleV1::new(inputs()).unwrap();
        let second = CompilerExecutionProvisioningBundleV1::new(inputs()).unwrap();
        assert_eq!(first, second);

        let policy =
            CompilerExecutionIssuerPolicyV1::decode(first.policy().canonical_bytes()).unwrap();
        let supervisor =
            CompilerExecutionSupervisorDeploymentV1::decode(first.supervisor().canonical_bytes())
                .unwrap();
        let anchor = CompilerExecutionExternalAnchorDeploymentV1::decode(
            first.anchor_deployment().canonical_bytes(),
        )
        .unwrap();
        let provisioning = CompilerExecutionExternalAnchorProvisioningV1::decode(
            first.anchor_provisioning().canonical_bytes(),
        )
        .unwrap();

        assert!(supervisor.matches_policy(&policy));
        assert!(anchor.matches_supervisor_and_policy(&supervisor, &policy));
        assert!(provisioning.matches_deployment(&anchor));
    }

    #[test]
    fn upstream_substitution_changes_every_downstream_identity() {
        let original = CompilerExecutionProvisioningBundleV1::new(inputs()).unwrap();
        let mut substituted = inputs();
        substituted.issuer = measurement(0x46, 0x4600);
        let substituted = CompilerExecutionProvisioningBundleV1::new(substituted).unwrap();
        assert_ne!(
            original.policy().identity(),
            substituted.policy().identity()
        );
        assert_ne!(
            original.supervisor().identity(),
            substituted.supervisor().identity()
        );
        assert_ne!(
            original.anchor_deployment().identity(),
            substituted.anchor_deployment().identity()
        );
        assert_ne!(
            original.anchor_provisioning().identity(),
            substituted.anchor_provisioning().identity()
        );
    }

    #[test]
    fn executable_measurements_are_role_distinct() {
        let original = inputs();
        assert!(matches!(
            CompilerExecutionProvisioningInputsV1::new(
                original.generation,
                original.compiler_service_uid,
                original.compiler_service_gid,
                original.external_anchor_service.uid(),
                original.external_anchor_service.gid(),
                original.supervisor,
                original.launcher,
                original.issuer,
                original.anchor_helper,
                original.anchor_helper,
                original.issuer_verifying_key,
                original.anchor_verifying_key,
            ),
            Err(
                CompilerExecutionProvisioningErrorV1::AliasedExecutableMeasurements {
                    first: "external-anchor provisioning helper",
                    second: "external-anchor daemon",
                }
            )
        ));
    }

    fn inputs() -> CompilerExecutionProvisioningInputsV1 {
        CompilerExecutionProvisioningInputsV1::new(
            7,
            1_001,
            1_002,
            2_001,
            2_002,
            measurement(0x11, 0x1100),
            measurement(0x22, 0x2200),
            measurement(0x33, 0x3300),
            measurement(0x44, 0x4400),
            measurement(0x55, 0x5500),
            SigningKey::from_bytes(&[0x66; 32])
                .verifying_key()
                .to_bytes(),
            SigningKey::from_bytes(&[0x77; 32])
                .verifying_key()
                .to_bytes(),
        )
        .unwrap()
    }

    fn measurement(digest: u8, byte_len: u64) -> CompilerExecutionIssuerMeasurementV1 {
        CompilerExecutionIssuerMeasurementV1::new([digest; 32], byte_len).unwrap()
    }
}
