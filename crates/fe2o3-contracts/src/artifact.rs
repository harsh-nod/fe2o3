/// A stable logical identity for a monomorphized kernel entry point.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct KernelIdentity {
    pub package: &'static str,
    pub symbol: &'static str,
    pub instantiation: &'static str,
}

impl KernelIdentity {
    pub const fn new(
        package: &'static str,
        symbol: &'static str,
        instantiation: &'static str,
    ) -> Self {
        Self {
            package,
            symbol,
            instantiation,
        }
    }
}

/// An opaque digest produced by build or verification tooling.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(transparent)]
pub struct ArtifactDigest([u8; 32]);

impl ArtifactDigest {
    pub const ZERO: Self = Self([0; 32]);

    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Identity of a compiler or verifier participating in artifact production.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ToolIdentity {
    pub name: &'static str,
    pub version: &'static str,
}

impl ToolIdentity {
    pub const fn new(name: &'static str, version: &'static str) -> Self {
        Self { name, version }
    }
}

/// Binds generated code to the source and contract from which it was built.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ArtifactIdentity {
    pub kernel: KernelIdentity,
    pub source_digest: ArtifactDigest,
    pub contract_digest: ArtifactDigest,
    pub executable_digest: ArtifactDigest,
    pub target: &'static str,
}

/// Identity of proof evidence associated with one exact executable artifact.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ProofIdentity {
    pub artifact: ArtifactIdentity,
    pub proof_digest: ArtifactDigest,
    pub verifier: ToolIdentity,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ProofStatus {
    Unverified,
    Checked,
    Verified,
}

/// Proof state for an artifact.
///
/// This spike exposes only the safe `unverified` constructor. Issuing stronger
/// evidence is reserved for future manifest-validation integration in this
/// crate, so application code cannot claim verification by construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProofArtifact {
    artifact: ArtifactIdentity,
    proof: Option<ProofIdentity>,
    status: ProofStatus,
}

impl ProofArtifact {
    pub const fn unverified(artifact: ArtifactIdentity) -> Self {
        Self {
            artifact,
            proof: None,
            status: ProofStatus::Unverified,
        }
    }

    pub const fn artifact(self) -> ArtifactIdentity {
        self.artifact
    }

    pub const fn proof(self) -> Option<ProofIdentity> {
        self.proof
    }

    pub const fn status(self) -> ProofStatus {
        self.status
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artifact() -> ArtifactIdentity {
        ArtifactIdentity {
            kernel: KernelIdentity::new("verus-vecadd", "vecadd", "u32"),
            source_digest: ArtifactDigest::from_bytes([1; 32]),
            contract_digest: ArtifactDigest::from_bytes([2; 32]),
            executable_digest: ArtifactDigest::from_bytes([3; 32]),
            target: "amdgcn-amd-amdhsa:gfx1100",
        }
    }

    #[test]
    fn digest_is_round_trippable_but_opaque() {
        let digest = ArtifactDigest::from_bytes([7; 32]);
        assert_eq!(digest.as_bytes(), &[7; 32]);
    }

    #[test]
    fn safe_records_start_unverified_and_without_proof_identity() {
        let record = ProofArtifact::unverified(artifact());

        assert_eq!(record.artifact(), artifact());
        assert_eq!(record.status(), ProofStatus::Unverified);
        assert_eq!(record.proof(), None);
    }
}
