//! Bounded, opaque stage snapshots.

use core::fmt;

use crate::{SnapshotFormatIdentityV1, SnapshotIdentityV1};

/// Hard maximum byte length of one V1 stage snapshot.
pub const MAX_STAGE_SNAPSHOT_BYTES_V1: usize = 16 * 1024 * 1024;

/// Stable stage vocabulary for the Rust-first compiler ladder.
///
/// These names do not expose implementation objects from any compiler
/// framework. A stage payload is meaningful only under its explicit format
/// identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum CompilerStageV1 {
    /// Target-neutral input produced by a checked frontend adapter.
    FrontendInput = 1,
    /// Admitted monomorphized Rust behavior and source origins.
    Mir = 2,
    /// Structured algorithm, indexing, and numerical semantics.
    Kernel = 3,
    /// Non-executable scheduling and mapping plan.
    Schedule = 4,
    /// Distributed regions, masks, and physical tile layouts.
    Tile = 5,
    /// Target-neutral executable SIMT representation.
    Gpu = 6,
    /// AMDGPU-selected semantics and target legalization.
    Amdgcn = 7,
    /// In-memory LLVM representation at the export boundary.
    Llvm = 8,
    /// Deterministically emitted target object.
    Object = 9,
    /// Finalized HSACO candidate before publication or loading.
    Hsaco = 10,
}

/// Why a stage snapshot was rejected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StageSnapshotErrorV1 {
    /// The opaque payload exceeds the hard V1 limit.
    PayloadTooLarge {
        /// Observed payload length.
        actual: usize,
        /// Maximum admitted payload length.
        maximum: usize,
    },
}

impl fmt::Display for StageSnapshotErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PayloadTooLarge { actual, maximum } => write!(
                formatter,
                "stage snapshot is {actual} bytes, exceeding the {maximum}-byte limit"
            ),
        }
    }
}

impl std::error::Error for StageSnapshotErrorV1 {}

/// Opaque canonical bytes for one compiler stage.
///
/// The identity and format commitments are supplied by the producer. This API
/// checks framing limits but does not hash, parse, or authenticate the bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StageSnapshotV1 {
    stage: CompilerStageV1,
    identity: SnapshotIdentityV1,
    format_identity: SnapshotFormatIdentityV1,
    canonical_bytes: Vec<u8>,
}

impl StageSnapshotV1 {
    /// Creates a bounded opaque snapshot.
    pub fn new(
        stage: CompilerStageV1,
        identity: SnapshotIdentityV1,
        format_identity: SnapshotFormatIdentityV1,
        canonical_bytes: Vec<u8>,
    ) -> Result<Self, StageSnapshotErrorV1> {
        if canonical_bytes.len() > MAX_STAGE_SNAPSHOT_BYTES_V1 {
            return Err(StageSnapshotErrorV1::PayloadTooLarge {
                actual: canonical_bytes.len(),
                maximum: MAX_STAGE_SNAPSHOT_BYTES_V1,
            });
        }
        Ok(Self {
            stage,
            identity,
            format_identity,
            canonical_bytes,
        })
    }

    /// Returns the semantic stage.
    pub const fn stage(&self) -> CompilerStageV1 {
        self.stage
    }

    /// Returns the producer-supplied snapshot commitment.
    pub const fn identity(&self) -> SnapshotIdentityV1 {
        self.identity
    }

    /// Returns the producer-supplied format commitment.
    pub const fn format_identity(&self) -> SnapshotFormatIdentityV1 {
        self.format_identity
    }

    /// Borrows the opaque canonical bytes.
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_preserves_stage_format_identity_and_bytes() {
        let snapshot = StageSnapshotV1::new(
            CompilerStageV1::Gpu,
            SnapshotIdentityV1::from_untrusted_bytes([1; 32]),
            SnapshotFormatIdentityV1::from_untrusted_bytes([2; 32]),
            vec![3, 4, 5],
        )
        .unwrap();

        assert_eq!(snapshot.stage(), CompilerStageV1::Gpu);
        assert_eq!(snapshot.identity().as_bytes(), &[1; 32]);
        assert_eq!(snapshot.format_identity().as_bytes(), &[2; 32]);
        assert_eq!(snapshot.canonical_bytes(), &[3, 4, 5]);
    }

    #[test]
    fn snapshot_rejects_payload_above_hard_limit() {
        let result = StageSnapshotV1::new(
            CompilerStageV1::Mir,
            SnapshotIdentityV1::from_untrusted_bytes([1; 32]),
            SnapshotFormatIdentityV1::from_untrusted_bytes([2; 32]),
            vec![0; MAX_STAGE_SNAPSHOT_BYTES_V1 + 1],
        );

        assert_eq!(
            result,
            Err(StageSnapshotErrorV1::PayloadTooLarge {
                actual: MAX_STAGE_SNAPSHOT_BYTES_V1 + 1,
                maximum: MAX_STAGE_SNAPSHOT_BYTES_V1,
            })
        );
    }
}
