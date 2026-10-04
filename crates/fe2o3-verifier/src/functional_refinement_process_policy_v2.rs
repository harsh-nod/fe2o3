//! Closed host policies for the exact retained, single-threaded Verus invocation.
//! A policy bounds process custody; it is not proof or signer authority.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum GeneratedProofProcessPolicyV2 {
    LegacySingleSolverV1,
    PinnedSingleThreadContextsV2,
}

impl GeneratedProofProcessPolicyV2 {
    pub(crate) const fn is_legacy(self) -> bool {
        matches!(self, Self::LegacySingleSolverV1)
    }

    pub(crate) const fn max_total(self) -> usize {
        match self {
            Self::LegacySingleSolverV1 => 1,
            Self::PinnedSingleThreadContextsV2 => 4_096,
        }
    }

    pub(crate) const fn max_live(self) -> usize {
        match self {
            Self::LegacySingleSolverV1 => 1,
            Self::PinnedSingleThreadContextsV2 => 2,
        }
    }

    pub(crate) const fn canonical_bytes(self) -> &'static [u8] {
        match self {
            Self::LegacySingleSolverV1 => b"FE2O3/GENERATED-PROOF/PROCESS-POLICY/V1\0single-solver;max-total=1;max-live=1;num-threads=1;direct-verifier-children;exact-retained-exec-fd-maps;authenticated-terminal",
            Self::PinnedSingleThreadContextsV2 => b"FE2O3/GENERATED-PROOF/PROCESS-POLICY/V2\0pinned-verus-b677dd5;max-total=4096;max-live=2;num-threads=1;direct-verifier-children;exact-retained-exec-fd-maps;authenticated-terminal",
        }
    }
}
