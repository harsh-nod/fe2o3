use core::fmt;

use crate::AmdTargetId;

/// Revision of the conservative advanced capability query model.
///
/// This is distinct from the canonical target-capabilities V1 text encoding.
/// Proof and admission records can bind this opaque value without interpreting
/// its integer representation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AdvancedCapabilityModelRevision(u16);

impl AdvancedCapabilityModelRevision {
    /// First reviewed advanced capability query model.
    pub const V1: Self = Self(1);

    /// Atomic admission model with evidence-correct legalizability decisions.
    pub const V2: Self = Self(2);

    /// Returns the stable numeric revision for bounded encoders.
    pub const fn get(self) -> u16 {
        self.0
    }
}

/// Current advanced capability model revision.
pub const ADVANCED_CAPABILITY_MODEL_REVISION: AdvancedCapabilityModelRevision =
    AdvancedCapabilityModelRevision::V2;

/// Review state for one advanced target capability.
///
/// `Unreviewed` means this model has no conclusion for the target. It must not
/// be interpreted as verified absence. `Unsupported` is emitted only by a
/// reviewed target profile that deliberately rejects the requested capability.
///
/// This enum is non-exhaustive so adding a more precise evidence state does not
/// break downstream proof or admission code that includes a fail-closed
/// wildcard arm.
///
/// ```compile_fail
/// use fe2o3_amd_target::AdvancedCapabilityStatus;
///
/// fn unsafe_exhaustive_match(status: AdvancedCapabilityStatus) -> bool {
///     match status {
///         AdvancedCapabilityStatus::Unreviewed => false,
///         AdvancedCapabilityStatus::Unsupported => false,
///         AdvancedCapabilityStatus::Supported => true,
///         AdvancedCapabilityStatus::RequiresRuntimeEvidence => false,
///     }
/// }
/// ```
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AdvancedCapabilityStatus {
    Unreviewed,
    Unsupported,
    Supported,
    RequiresRuntimeEvidence,
}

impl fmt::Display for AdvancedCapabilityStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Unreviewed => "unreviewed",
            Self::Unsupported => "unsupported",
            Self::Supported => "supported",
            Self::RequiresRuntimeEvidence => "runtime-evidence",
        })
    }
}

/// Revision and exact target identity for advanced capability decisions.
///
/// This value is data suitable for inclusion in a future proof or admission
/// identity. It grants no authority and does not attest a compiler or device.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AdvancedCapabilityModelIdentity {
    revision: AdvancedCapabilityModelRevision,
    target: AmdTargetId,
}

impl AdvancedCapabilityModelIdentity {
    pub(crate) const fn new(target: AmdTargetId) -> Self {
        Self::for_revision(ADVANCED_CAPABILITY_MODEL_REVISION, target)
    }

    /// Constructs an inert identity for a declared model revision and target.
    ///
    /// This supports comparison with historical decision tables. It grants no
    /// authority and does not attest that the revision admitted an operation.
    pub const fn for_revision(
        revision: AdvancedCapabilityModelRevision,
        target: AmdTargetId,
    ) -> Self {
        Self { revision, target }
    }

    /// Advanced model revision bound by this identity.
    pub const fn revision(self) -> AdvancedCapabilityModelRevision {
        self.revision
    }

    /// Exact canonical target bound by this identity.
    pub const fn target(self) -> AmdTargetId {
        self.target
    }

    /// Writes the deterministic identity encoding for this advanced model.
    pub fn encode_canonical(&self, writer: &mut impl fmt::Write) -> fmt::Result {
        write!(
            writer,
            "amd-advanced-capability-model-v{}{{target={}}}",
            self.revision.get(),
            self.target,
        )
    }
}

impl fmt::Display for AdvancedCapabilityModelIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.encode_canonical(formatter)
    }
}
