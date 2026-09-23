//! The GPU target declared by an admitted, verified simulation bundle.
//! This is content custody, not compiler authentication or a hardware observation.
use crate::{
    AdmittedSimulationBundleEvidenceV1, AdmittedSimulationInputV1, SimulationInputErrorV1,
};
use fe2o3_kir_sim::SimulationKernelIrIdentityV1;

/// Closed targets supported by the existing CPU bundle admission profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BundleDeclaredGpuTargetV1 {
    Gfx942XnackOff,
    Gfx950XnackOff,
}
impl BundleDeclaredGpuTargetV1 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Gfx942XnackOff => "gfx942:xnack-",
            Self::Gfx950XnackOff => "gfx950:xnack-",
        }
    }
}

/// Immutable content bindings produced only inside verified bundle admission.
/// There is deliberately no public constructor, decoder, attach, or setter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdmittedBundleTargetV1 {
    target: BundleDeclaredGpuTargetV1,
    module: SimulationKernelIrIdentityV1,
    evidence: AdmittedSimulationBundleEvidenceV1,
    legacy_bundle_identity: [u8; 32],
}
impl AdmittedBundleTargetV1 {
    pub const fn target(self) -> BundleDeclaredGpuTargetV1 {
        self.target
    }
    pub const fn module_identity(self) -> SimulationKernelIrIdentityV1 {
        self.module
    }
    pub const fn envelope_version(self) -> u16 {
        self.evidence.envelope_version
    }
    pub const fn envelope_identity(self) -> [u8; 32] {
        self.evidence.envelope_identity
    }
    pub const fn subject_identity(self) -> [u8; 32] {
        self.evidence.subject_identity
    }
}

fn changed() -> SimulationInputErrorV1 {
    SimulationInputErrorV1 {
        stage: "simulator_admission".to_owned(),
        code: "bundle_target_binding_changed".to_owned(),
        message:
            "bundle-declared target no longer matches its original admitted module and envelope"
                .to_owned(),
    }
}
impl AdmittedSimulationInputV1 {
    /// Retains a target only after the exact bundle and embedded module were verified.
    /// V2--V4 intentionally have an outer envelope identity distinct from the legacy
    /// inner V1 schedule identity; both original bindings are retained independently.
    pub(crate) fn retain_bundle_target_v1(
        &mut self,
        target: &str,
    ) -> Result<(), SimulationInputErrorV1> {
        let target = match target {
            "gfx942:xnack-" => BundleDeclaredGpuTargetV1::Gfx942XnackOff,
            "gfx950:xnack-" => BundleDeclaredGpuTargetV1::Gfx950XnackOff,
            _ => return Err(changed()),
        };
        let evidence = self.simulation_bundle_evidence.ok_or_else(changed)?;
        let legacy_bundle_identity = self.simulation_bundle_identity.ok_or_else(changed)?;
        let expected_version = match evidence.envelope_version {
            1..=4 => 7,
            5 => 10,
            6 => 11,
            _ => return Err(changed()),
        };
        if self.bundle_target_v1.is_some()
            || self.module.identity().wire_version() != expected_version
            || self.module.identity().canonical_length() == 0
            || self.kir_sha256 != *self.module.identity().digest()
            || self.simulation_bundle_subject != Some(evidence.subject_identity)
            || evidence.envelope_identity == [0; 32]
            || evidence.subject_identity == [0; 32]
            || legacy_bundle_identity == [0; 32]
        {
            return Err(changed());
        }
        self.bundle_target_v1 = Some(AdmittedBundleTargetV1 {
            target,
            module: *self.module.identity(),
            evidence,
            legacy_bundle_identity,
        });
        Ok(())
    }

    /// Checks original private custody, not just the public `kir_sha256` field.
    /// Replacing the public module, even with another same-version module, is refused.
    /// Raw KIR never acquires a declared GPU target from its CPU simulation profile.
    pub fn retained_bundle_target_v1(
        &self,
    ) -> Result<Option<AdmittedBundleTargetV1>, SimulationInputErrorV1> {
        let Some(retained) = self.bundle_target_v1 else {
            return if self.simulation_bundle_subject.is_none()
                && self.simulation_bundle_identity.is_none()
                && self.simulation_bundle_evidence.is_none()
            {
                Ok(None)
            } else {
                Err(changed())
            };
        };
        if *self.module.identity() != retained.module
            || self.kir_sha256 != *retained.module.digest()
            || self.simulation_bundle_evidence != Some(retained.evidence)
            || self.simulation_bundle_subject != Some(retained.evidence.subject_identity)
            || self.simulation_bundle_identity != Some(retained.legacy_bundle_identity)
        {
            return Err(changed());
        }
        Ok(Some(retained))
    }
}

#[cfg(all(test, target_os = "linux"))]
#[path = "bundle_declared_target_tests.rs"]
mod tests;
