//! Bounded V1 compile requests for the sole production pipeline.

use core::fmt;

use crate::{
    CompilerProfileIdentityV1, CompilerStageV1, KernelInstanceIdentityV1, ObligationSetIdentityV1,
    PipelineConfigurationIdentityV1, RequestIdentityV1, StageSnapshotV1, TargetProfileIdentityV1,
};
use crate::{MAX_EXECUTABLE_CANDIDATE_BYTES_V1, MAX_STAGE_SNAPSHOT_BYTES_V1};

/// Hard maximum number of snapshots in one V1 output.
pub const MAX_STAGE_SNAPSHOTS_V1: u16 = 64;
/// Hard maximum number of receipts in one V1 output.
pub const MAX_STAGE_RECEIPTS_V1: u16 = 256;
/// Hard maximum number of diagnostics in one V1 output.
pub const MAX_DIAGNOSTICS_V1: u16 = 256;
/// Hard maximum aggregate byte length of all V1 output snapshots.
pub const MAX_TOTAL_SNAPSHOT_BYTES_V1: u32 = 64 * 1024 * 1024;

/// Named field in [`CompileLimitsV1`].
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CompileLimitFieldV1 {
    /// Maximum stage snapshot count.
    StageSnapshots,
    /// Maximum stage receipt count.
    StageReceipts,
    /// Maximum diagnostic count.
    Diagnostics,
    /// Maximum bytes in one snapshot.
    SnapshotBytes,
    /// Maximum aggregate snapshot bytes.
    TotalSnapshotBytes,
    /// Maximum executable candidate bytes.
    CandidateBytes,
}

/// Why caller-selected compile limits were rejected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompileLimitsErrorV1 {
    /// Every caller-selected limit must be nonzero.
    Zero {
        /// Field whose value was zero.
        field: CompileLimitFieldV1,
    },
    /// A caller-selected limit exceeds its hard V1 ceiling.
    ExceedsHardMaximum {
        /// Field whose value was too large.
        field: CompileLimitFieldV1,
        /// Observed value.
        actual: u64,
        /// Hard V1 ceiling.
        maximum: u64,
    },
    /// The per-snapshot limit exceeds the aggregate snapshot limit.
    SnapshotBytesExceedTotal {
        /// Per-snapshot byte limit.
        snapshot_bytes: u32,
        /// Aggregate snapshot byte limit.
        total_snapshot_bytes: u32,
    },
}

impl fmt::Display for CompileLimitsErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Zero { field } => write!(formatter, "compile limit {field:?} must be nonzero"),
            Self::ExceedsHardMaximum {
                field,
                actual,
                maximum,
            } => write!(
                formatter,
                "compile limit {field:?} is {actual}, exceeding hard maximum {maximum}"
            ),
            Self::SnapshotBytesExceedTotal {
                snapshot_bytes,
                total_snapshot_bytes,
            } => write!(
                formatter,
                "per-snapshot limit {snapshot_bytes} exceeds aggregate limit {total_snapshot_bytes}"
            ),
        }
    }
}

impl std::error::Error for CompileLimitsErrorV1 {}

/// Caller-selected limits, each constrained by a hard V1 ceiling.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompileLimitsV1 {
    max_stage_snapshots: u16,
    max_stage_receipts: u16,
    max_diagnostics: u16,
    max_snapshot_bytes: u32,
    max_total_snapshot_bytes: u32,
    max_candidate_bytes: u32,
}

impl CompileLimitsV1 {
    /// Selects limits for one compile request.
    pub fn new(
        max_stage_snapshots: u16,
        max_stage_receipts: u16,
        max_diagnostics: u16,
        max_snapshot_bytes: u32,
        max_total_snapshot_bytes: u32,
        max_candidate_bytes: u32,
    ) -> Result<Self, CompileLimitsErrorV1> {
        check_nonzero(
            CompileLimitFieldV1::StageSnapshots,
            max_stage_snapshots as u64,
        )?;
        check_nonzero(
            CompileLimitFieldV1::StageReceipts,
            max_stage_receipts as u64,
        )?;
        check_nonzero(CompileLimitFieldV1::Diagnostics, max_diagnostics as u64)?;
        check_nonzero(
            CompileLimitFieldV1::SnapshotBytes,
            max_snapshot_bytes as u64,
        )?;
        check_nonzero(
            CompileLimitFieldV1::TotalSnapshotBytes,
            max_total_snapshot_bytes as u64,
        )?;
        check_nonzero(
            CompileLimitFieldV1::CandidateBytes,
            max_candidate_bytes as u64,
        )?;
        check_maximum(
            CompileLimitFieldV1::StageSnapshots,
            max_stage_snapshots as u64,
            MAX_STAGE_SNAPSHOTS_V1 as u64,
        )?;
        check_maximum(
            CompileLimitFieldV1::StageReceipts,
            max_stage_receipts as u64,
            MAX_STAGE_RECEIPTS_V1 as u64,
        )?;
        check_maximum(
            CompileLimitFieldV1::Diagnostics,
            max_diagnostics as u64,
            MAX_DIAGNOSTICS_V1 as u64,
        )?;
        check_maximum(
            CompileLimitFieldV1::SnapshotBytes,
            max_snapshot_bytes as u64,
            MAX_STAGE_SNAPSHOT_BYTES_V1 as u64,
        )?;
        check_maximum(
            CompileLimitFieldV1::TotalSnapshotBytes,
            max_total_snapshot_bytes as u64,
            MAX_TOTAL_SNAPSHOT_BYTES_V1 as u64,
        )?;
        check_maximum(
            CompileLimitFieldV1::CandidateBytes,
            max_candidate_bytes as u64,
            MAX_EXECUTABLE_CANDIDATE_BYTES_V1 as u64,
        )?;
        if max_snapshot_bytes > max_total_snapshot_bytes {
            return Err(CompileLimitsErrorV1::SnapshotBytesExceedTotal {
                snapshot_bytes: max_snapshot_bytes,
                total_snapshot_bytes: max_total_snapshot_bytes,
            });
        }
        Ok(Self {
            max_stage_snapshots,
            max_stage_receipts,
            max_diagnostics,
            max_snapshot_bytes,
            max_total_snapshot_bytes,
            max_candidate_bytes,
        })
    }

    /// Returns the requested snapshot count limit.
    pub const fn max_stage_snapshots(self) -> u16 {
        self.max_stage_snapshots
    }

    /// Returns the requested receipt count limit.
    pub const fn max_stage_receipts(self) -> u16 {
        self.max_stage_receipts
    }

    /// Returns the requested diagnostic count limit.
    pub const fn max_diagnostics(self) -> u16 {
        self.max_diagnostics
    }

    /// Returns the requested per-snapshot byte limit.
    pub const fn max_snapshot_bytes(self) -> u32 {
        self.max_snapshot_bytes
    }

    /// Returns the requested aggregate snapshot byte limit.
    pub const fn max_total_snapshot_bytes(self) -> u32 {
        self.max_total_snapshot_bytes
    }

    /// Returns the requested candidate byte limit.
    pub const fn max_candidate_bytes(self) -> u32 {
        self.max_candidate_bytes
    }
}

impl Default for CompileLimitsV1 {
    fn default() -> Self {
        Self {
            max_stage_snapshots: MAX_STAGE_SNAPSHOTS_V1,
            max_stage_receipts: MAX_STAGE_RECEIPTS_V1,
            max_diagnostics: MAX_DIAGNOSTICS_V1,
            max_snapshot_bytes: MAX_STAGE_SNAPSHOT_BYTES_V1 as u32,
            max_total_snapshot_bytes: MAX_TOTAL_SNAPSHOT_BYTES_V1,
            max_candidate_bytes: MAX_EXECUTABLE_CANDIDATE_BYTES_V1 as u32,
        }
    }
}

fn check_nonzero(field: CompileLimitFieldV1, value: u64) -> Result<(), CompileLimitsErrorV1> {
    if value == 0 {
        return Err(CompileLimitsErrorV1::Zero { field });
    }
    Ok(())
}

fn check_maximum(
    field: CompileLimitFieldV1,
    value: u64,
    maximum: u64,
) -> Result<(), CompileLimitsErrorV1> {
    if value > maximum {
        return Err(CompileLimitsErrorV1::ExceedsHardMaximum {
            field,
            actual: value,
            maximum,
        });
    }
    Ok(())
}

/// Why a compile request was rejected at the API boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompileRequestErrorV1 {
    /// The initial opaque snapshot must be a frontend input.
    InputStageNotFrontend {
        /// Observed initial stage.
        actual: CompilerStageV1,
    },
    /// Empty frontend input cannot identify any admitted program.
    EmptyInput,
    /// The input exceeds the request's per-snapshot byte limit.
    InputExceedsRequestedLimit {
        /// Observed byte length.
        actual: usize,
        /// Requested per-snapshot byte limit.
        maximum: u32,
    },
}

impl fmt::Display for CompileRequestErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InputStageNotFrontend { actual } => {
                write!(
                    formatter,
                    "compile input stage is {actual:?}, not FrontendInput"
                )
            }
            Self::EmptyInput => formatter.write_str("compile input snapshot must not be empty"),
            Self::InputExceedsRequestedLimit { actual, maximum } => write!(
                formatter,
                "compile input is {actual} bytes, exceeding requested limit {maximum}"
            ),
        }
    }
}

impl std::error::Error for CompileRequestErrorV1 {}

/// Complete target-neutral input to a V1 compiler driver.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileRequestV1 {
    identity: RequestIdentityV1,
    kernel_instance_identity: KernelInstanceIdentityV1,
    compiler_profile_identity: CompilerProfileIdentityV1,
    target_profile_identity: TargetProfileIdentityV1,
    pipeline_configuration_identity: PipelineConfigurationIdentityV1,
    input_obligations_identity: ObligationSetIdentityV1,
    input: StageSnapshotV1,
    limits: CompileLimitsV1,
}

impl CompileRequestV1 {
    /// Creates a bounded compile request from an opaque frontend snapshot.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        identity: RequestIdentityV1,
        kernel_instance_identity: KernelInstanceIdentityV1,
        compiler_profile_identity: CompilerProfileIdentityV1,
        target_profile_identity: TargetProfileIdentityV1,
        pipeline_configuration_identity: PipelineConfigurationIdentityV1,
        input_obligations_identity: ObligationSetIdentityV1,
        input: StageSnapshotV1,
        limits: CompileLimitsV1,
    ) -> Result<Self, CompileRequestErrorV1> {
        if input.stage() != CompilerStageV1::FrontendInput {
            return Err(CompileRequestErrorV1::InputStageNotFrontend {
                actual: input.stage(),
            });
        }
        if input.canonical_bytes().is_empty() {
            return Err(CompileRequestErrorV1::EmptyInput);
        }
        if input.canonical_bytes().len() > limits.max_snapshot_bytes as usize {
            return Err(CompileRequestErrorV1::InputExceedsRequestedLimit {
                actual: input.canonical_bytes().len(),
                maximum: limits.max_snapshot_bytes,
            });
        }
        Ok(Self {
            identity,
            kernel_instance_identity,
            compiler_profile_identity,
            target_profile_identity,
            pipeline_configuration_identity,
            input_obligations_identity,
            input,
            limits,
        })
    }

    /// Returns the declared request commitment.
    pub const fn identity(&self) -> RequestIdentityV1 {
        self.identity
    }

    /// Returns the concrete kernel instance commitment.
    pub const fn kernel_instance_identity(&self) -> KernelInstanceIdentityV1 {
        self.kernel_instance_identity
    }

    /// Returns the frontend and compiler semantic profile commitment.
    pub const fn compiler_profile_identity(&self) -> CompilerProfileIdentityV1 {
        self.compiler_profile_identity
    }

    /// Returns the target profile commitment.
    pub const fn target_profile_identity(&self) -> TargetProfileIdentityV1 {
        self.target_profile_identity
    }

    /// Returns the pipeline configuration commitment.
    pub const fn pipeline_configuration_identity(&self) -> PipelineConfigurationIdentityV1 {
        self.pipeline_configuration_identity
    }

    /// Returns the obligation-set commitment attached to the frontend input.
    pub const fn input_obligations_identity(&self) -> ObligationSetIdentityV1 {
        self.input_obligations_identity
    }

    /// Returns the opaque frontend input snapshot.
    pub const fn input(&self) -> &StageSnapshotV1 {
        &self.input
    }

    /// Returns the caller-selected resource limits.
    pub const fn limits(&self) -> CompileLimitsV1 {
        self.limits
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SnapshotFormatIdentityV1, SnapshotIdentityV1};

    fn snapshot(stage: CompilerStageV1, bytes: Vec<u8>) -> StageSnapshotV1 {
        StageSnapshotV1::new(
            stage,
            SnapshotIdentityV1::from_untrusted_bytes([1; 32]),
            SnapshotFormatIdentityV1::from_untrusted_bytes([2; 32]),
            bytes,
        )
        .unwrap()
    }

    fn request(
        input: StageSnapshotV1,
        limits: CompileLimitsV1,
    ) -> Result<CompileRequestV1, CompileRequestErrorV1> {
        CompileRequestV1::new(
            RequestIdentityV1::from_untrusted_bytes([3; 32]),
            KernelInstanceIdentityV1::from_untrusted_bytes([4; 32]),
            CompilerProfileIdentityV1::from_untrusted_bytes([5; 32]),
            TargetProfileIdentityV1::from_untrusted_bytes([6; 32]),
            PipelineConfigurationIdentityV1::from_untrusted_bytes([7; 32]),
            ObligationSetIdentityV1::from_untrusted_bytes([8; 32]),
            input,
            limits,
        )
    }

    #[test]
    fn limits_reject_zero_excessive_and_inconsistent_values() {
        assert_eq!(
            CompileLimitsV1::new(0, 1, 1, 1, 1, 1),
            Err(CompileLimitsErrorV1::Zero {
                field: CompileLimitFieldV1::StageSnapshots,
            })
        );
        assert_eq!(
            CompileLimitsV1::new(MAX_STAGE_SNAPSHOTS_V1 + 1, 1, 1, 1, 1, 1),
            Err(CompileLimitsErrorV1::ExceedsHardMaximum {
                field: CompileLimitFieldV1::StageSnapshots,
                actual: u64::from(MAX_STAGE_SNAPSHOTS_V1) + 1,
                maximum: u64::from(MAX_STAGE_SNAPSHOTS_V1),
            })
        );
        assert_eq!(
            CompileLimitsV1::new(1, 1, 1, 2, 1, 1),
            Err(CompileLimitsErrorV1::SnapshotBytesExceedTotal {
                snapshot_bytes: 2,
                total_snapshot_bytes: 1,
            })
        );
    }

    #[test]
    fn request_accepts_only_nonempty_bounded_frontend_input() {
        assert_eq!(
            request(
                snapshot(CompilerStageV1::Mir, vec![1]),
                CompileLimitsV1::default()
            ),
            Err(CompileRequestErrorV1::InputStageNotFrontend {
                actual: CompilerStageV1::Mir,
            })
        );
        assert_eq!(
            request(
                snapshot(CompilerStageV1::FrontendInput, vec![]),
                CompileLimitsV1::default(),
            ),
            Err(CompileRequestErrorV1::EmptyInput)
        );
        let limits = CompileLimitsV1::new(1, 1, 1, 1, 1, 1).unwrap();
        assert_eq!(
            request(snapshot(CompilerStageV1::FrontendInput, vec![1, 2]), limits,),
            Err(CompileRequestErrorV1::InputExceedsRequestedLimit {
                actual: 2,
                maximum: 1,
            })
        );
        let accepted = request(snapshot(CompilerStageV1::FrontendInput, vec![1]), limits).unwrap();
        assert_eq!(accepted.input().canonical_bytes(), &[1]);
    }
}
