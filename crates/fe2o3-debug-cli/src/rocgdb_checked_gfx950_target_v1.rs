//! Read-only gfx950 checked-device/artifact companion.
//! No queue, launch, attach, telemetry, correlation, native-stop or register API.
//! Revalidation checks the same retained inputs; caller load-base admission is
//! not upgraded into evidence that a loader actually used that address.
use crate::rocgdb_mi_v4::RocgdbCodeObjectBindingV4;
use fe2o3_amd_target::{AmdTargetId, FeatureState};
use fe2o3_debug_protocol::{LiveGpuContentIdentityV3, OpaqueIdentityV1};
use fe2o3_kfd::CheckedGfx950XnackMinusDevice;
use sha2::{Digest, Sha256};

/// Exact rejection class; no native addresses, paths or raw driver errors.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RocgdbCheckedGfx950ArtifactErrorV1 {
    InputBound,
    DeviceNotCurrent,
    ArtifactInspection,
    ArtifactTarget,
    KernelSelection,
    KernelWaveWidth,
    ArtifactIdentity,
    CodeBinding,
    ReinspectionMismatch,
}
impl std::fmt::Display for RocgdbCheckedGfx950ArtifactErrorV1 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "checked gfx950 observation rejected: {self:?}")
    }
}
impl std::error::Error for RocgdbCheckedGfx950ArtifactErrorV1 {}

/// Move-only borrowed observation companion; not execution admission.
///
/// Safe Rust cannot replace the checked device, bytes or selected kernel name
/// while this owner exists. No public fields, Clone, Deserialize, raw device or
/// correlation gateway exists. The copied content identity is always inert.
///
/// ```compile_fail
/// use fe2o3_debug_cli::rocgdb_checked_gfx950_target_v1::RocgdbCheckedGfx950ArtifactV1;
/// let forged = RocgdbCheckedGfx950ArtifactV1 {};
/// ```
///
/// ```compile_fail
/// use fe2o3_debug_cli::rocgdb_checked_gfx950_target_v1::RocgdbCheckedGfx950ArtifactV1;
/// fn needs_clone<T: Clone>() {}
/// needs_clone::<RocgdbCheckedGfx950ArtifactV1<'static>>();
/// ```
pub struct RocgdbCheckedGfx950ArtifactV1<'a> {
    device: &'a mut CheckedGfx950XnackMinusDevice,
    artifact_bytes: &'a [u8],
    kernel_name: &'a str,
    load_base: u64,
    artifact: LiveGpuContentIdentityV3,
    code: RocgdbCodeObjectBindingV4,
}

impl<'a> RocgdbCheckedGfx950ArtifactV1<'a> {
    /// Inspect bounded retained bytes and exactly one selected kernel. The
    /// explicit load base remains caller admission, never inferred loader truth.
    /// Rechecks full observable device currentness before and after inspection.
    pub fn inspect(
        device: &'a mut CheckedGfx950XnackMinusDevice,
        artifact_bytes: &'a [u8],
        kernel_name: &'a str,
        load_base: u64,
    ) -> Result<Self, RocgdbCheckedGfx950ArtifactErrorV1> {
        require_input_bounds(artifact_bytes.len(), kernel_name)?;
        require_current(device)?;
        let inspected = inspect_artifact(artifact_bytes, kernel_name, load_base);
        // Even a failed artifact inspection does not skip the closing device
        // fence. Neither failure returns a partially checked companion.
        require_current(device)?;
        let inspected = inspected?;
        Ok(Self {
            device,
            artifact_bytes,
            kernel_name,
            load_base,
            artifact: inspected.artifact,
            code: inspected.code,
        })
    }

    /// Inert content identity. It cannot reconstruct this borrowed companion
    /// and does not imply the device is still current when this getter is read.
    pub const fn artifact(&self) -> LiveGpuContentIdentityV3 {
        self.artifact
    }

    /// Reinspect the SAME immutable artifact/name/base, compare the complete
    /// content and selected-entry binding, and fence the actual checked device
    /// before and after. This performs no GPU dispatch or debugger operation.
    /// Success establishes these observations only at the completed checks,
    /// not absence of future/external device changes or execution authority.
    pub fn revalidate(&mut self) -> Result<(), RocgdbCheckedGfx950ArtifactErrorV1> {
        require_input_bounds(self.artifact_bytes.len(), self.kernel_name)?;
        require_current(self.device)?;
        let inspected = inspect_artifact(self.artifact_bytes, self.kernel_name, self.load_base);
        require_current(self.device)?;
        let inspected = inspected?;
        require_same_inspection(self.artifact, self.code, &inspected)
    }
}

fn require_current(
    device: &mut CheckedGfx950XnackMinusDevice,
) -> Result<(), RocgdbCheckedGfx950ArtifactErrorV1> {
    device
        .check_observable_currentness()
        .map_err(|_| RocgdbCheckedGfx950ArtifactErrorV1::DeviceNotCurrent)
}

#[derive(Debug)]
struct InspectedArtifact {
    artifact: LiveGpuContentIdentityV3,
    code: RocgdbCodeObjectBindingV4,
}
fn require_same_inspection(
    artifact: LiveGpuContentIdentityV3,
    code: RocgdbCodeObjectBindingV4,
    actual: &InspectedArtifact,
) -> Result<(), RocgdbCheckedGfx950ArtifactErrorV1> {
    if actual.artifact != artifact || actual.code != code {
        return Err(RocgdbCheckedGfx950ArtifactErrorV1::ReinspectionMismatch);
    }
    Ok(())
}

fn require_input_bounds(
    bytes: usize,
    kernel_name: &str,
) -> Result<(), RocgdbCheckedGfx950ArtifactErrorV1> {
    if bytes == 0
        || bytes > fe2o3_hsaco::MAX_HSACO_BYTES
        || kernel_name.is_empty()
        || kernel_name.len() > fe2o3_hsaco::MAX_MESSAGEPACK_STRING_BYTES
        || kernel_name
            .bytes()
            .any(|byte| byte == 0 || byte.is_ascii_control())
    {
        return Err(RocgdbCheckedGfx950ArtifactErrorV1::InputBound);
    }
    Ok(())
}
fn require_profile(
    target: AmdTargetId,
    wave: u32,
) -> Result<(), RocgdbCheckedGfx950ArtifactErrorV1> {
    // Exact closed profile: missing/any/positive XNACK is not minus.
    // No new explicit SRAM-ECC feature claim is accepted.
    if target.processor() != "gfx950"
        || target.xnack() != Some(FeatureState::Disabled)
        || target.sramecc().is_some()
    {
        return Err(RocgdbCheckedGfx950ArtifactErrorV1::ArtifactTarget);
    }
    if wave != 64 {
        return Err(RocgdbCheckedGfx950ArtifactErrorV1::KernelWaveWidth);
    }
    Ok(())
}
fn inspect_artifact(
    bytes: &[u8],
    kernel_name: &str,
    load_base: u64,
) -> Result<InspectedArtifact, RocgdbCheckedGfx950ArtifactErrorV1> {
    require_input_bounds(bytes.len(), kernel_name)?;
    let inspected = fe2o3_hsaco::inspect_and_bind_kernel_descriptors(bytes)
        .map_err(|_| RocgdbCheckedGfx950ArtifactErrorV1::ArtifactInspection)?;
    let mut selected = inspected
        .inspection()
        .kernels()
        .iter()
        .enumerate()
        .filter(|(_, kernel)| kernel.name() == kernel_name);
    let (index, kernel) = selected
        .next()
        .ok_or(RocgdbCheckedGfx950ArtifactErrorV1::KernelSelection)?;
    if selected.next().is_some() {
        return Err(RocgdbCheckedGfx950ArtifactErrorV1::KernelSelection);
    }
    require_profile(inspected.inspection().target(), kernel.wavefront_size())?;
    let binding = inspected
        .bindings()
        .get(index)
        .copied()
        .filter(|binding| binding.kernel_index() == index)
        .ok_or(RocgdbCheckedGfx950ArtifactErrorV1::KernelSelection)?;
    let digest = OpaqueIdentityV1::new(Sha256::digest(bytes).into())
        .map_err(|_| RocgdbCheckedGfx950ArtifactErrorV1::ArtifactIdentity)?;
    let artifact = LiveGpuContentIdentityV3 {
        digest,
        canonical_bytes: bytes.len() as u64,
    };
    let code = RocgdbCodeObjectBindingV4::new(
        artifact,
        load_base,
        binding.entry_address(),
        binding.entry_size(),
    )
    .map_err(|_| RocgdbCheckedGfx950ArtifactErrorV1::CodeBinding)?;
    Ok(InspectedArtifact { artifact, code })
}

#[cfg(test)]
#[path = "rocgdb_checked_gfx950_target_v1_tests.rs"]
mod tests;
