//! Deterministic receipts for compiler transformations.

use core::fmt;

use crate::{
    CompilerStageV1, ObligationSetIdentityV1, SnapshotIdentityV1, TransformConfigurationIdentityV1,
    TransformIdentityV1,
};

/// Transactional outcome of one compiler transformation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum ReceiptOutcomeV1 {
    /// The transformation committed one output snapshot.
    Produced = 1,
    /// The transformation rejected its input and committed no output.
    Rejected = 2,
}

/// Why a stage receipt was rejected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StageReceiptErrorV1 {
    /// A produced receipt omitted its output snapshot.
    ProducedWithoutOutput,
    /// A produced receipt omitted its output obligation set.
    ProducedWithoutOutputObligations,
    /// A rejected receipt claimed an output snapshot.
    RejectedWithOutput,
    /// A rejected receipt claimed an output obligation set.
    RejectedWithOutputObligations,
}

impl fmt::Display for StageReceiptErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProducedWithoutOutput => {
                formatter.write_str("produced receipt must name an output snapshot")
            }
            Self::ProducedWithoutOutputObligations => {
                formatter.write_str("produced receipt must name output obligations")
            }
            Self::RejectedWithOutput => {
                formatter.write_str("rejected receipt cannot name an output snapshot")
            }
            Self::RejectedWithOutputObligations => {
                formatter.write_str("rejected receipt cannot name output obligations")
            }
        }
    }
}

impl std::error::Error for StageReceiptErrorV1 {}

/// Receipt binding one transformation input, output, and obligation state.
///
/// A receipt records commitments but does not establish that a transformation
/// ran correctly or that its obligations were discharged soundly.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StageReceiptV1 {
    sequence: u16,
    stage: CompilerStageV1,
    transform_identity: TransformIdentityV1,
    configuration_identity: TransformConfigurationIdentityV1,
    input_snapshot_identity: SnapshotIdentityV1,
    output_snapshot_identity: Option<SnapshotIdentityV1>,
    input_obligations_identity: ObligationSetIdentityV1,
    output_obligations_identity: Option<ObligationSetIdentityV1>,
    outcome: ReceiptOutcomeV1,
}

impl StageReceiptV1 {
    /// Creates a receipt and enforces transactional output shape.
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        sequence: u16,
        stage: CompilerStageV1,
        transform_identity: TransformIdentityV1,
        configuration_identity: TransformConfigurationIdentityV1,
        input_snapshot_identity: SnapshotIdentityV1,
        output_snapshot_identity: Option<SnapshotIdentityV1>,
        input_obligations_identity: ObligationSetIdentityV1,
        output_obligations_identity: Option<ObligationSetIdentityV1>,
        outcome: ReceiptOutcomeV1,
    ) -> Result<Self, StageReceiptErrorV1> {
        match outcome {
            ReceiptOutcomeV1::Produced => {
                if output_snapshot_identity.is_none() {
                    return Err(StageReceiptErrorV1::ProducedWithoutOutput);
                }
                if output_obligations_identity.is_none() {
                    return Err(StageReceiptErrorV1::ProducedWithoutOutputObligations);
                }
            }
            ReceiptOutcomeV1::Rejected => {
                if output_snapshot_identity.is_some() {
                    return Err(StageReceiptErrorV1::RejectedWithOutput);
                }
                if output_obligations_identity.is_some() {
                    return Err(StageReceiptErrorV1::RejectedWithOutputObligations);
                }
            }
        }
        Ok(Self {
            sequence,
            stage,
            transform_identity,
            configuration_identity,
            input_snapshot_identity,
            output_snapshot_identity,
            input_obligations_identity,
            output_obligations_identity,
            outcome,
        })
    }

    /// Returns the zero-based transformation sequence number.
    pub const fn sequence(self) -> u16 {
        self.sequence
    }

    /// Returns the stage produced or rejected by this transformation.
    pub const fn stage(self) -> CompilerStageV1 {
        self.stage
    }

    /// Returns the transformation commitment.
    pub const fn transform_identity(self) -> TransformIdentityV1 {
        self.transform_identity
    }

    /// Returns the transformation configuration commitment.
    pub const fn configuration_identity(self) -> TransformConfigurationIdentityV1 {
        self.configuration_identity
    }

    /// Returns the input snapshot commitment.
    pub const fn input_snapshot_identity(self) -> SnapshotIdentityV1 {
        self.input_snapshot_identity
    }

    /// Returns the committed output snapshot, absent on rejection.
    pub const fn output_snapshot_identity(self) -> Option<SnapshotIdentityV1> {
        self.output_snapshot_identity
    }

    /// Returns the input obligation-set commitment.
    pub const fn input_obligations_identity(self) -> ObligationSetIdentityV1 {
        self.input_obligations_identity
    }

    /// Returns the output obligation-set commitment, absent on rejection.
    pub const fn output_obligations_identity(self) -> Option<ObligationSetIdentityV1> {
        self.output_obligations_identity
    }

    /// Returns the transactional outcome.
    pub const fn outcome(self) -> ReceiptOutcomeV1 {
        self.outcome
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn receipt(
        output: Option<SnapshotIdentityV1>,
        obligations: Option<ObligationSetIdentityV1>,
        outcome: ReceiptOutcomeV1,
    ) -> Result<StageReceiptV1, StageReceiptErrorV1> {
        StageReceiptV1::new(
            0,
            CompilerStageV1::Mir,
            TransformIdentityV1::from_untrusted_bytes([1; 32]),
            TransformConfigurationIdentityV1::from_untrusted_bytes([2; 32]),
            SnapshotIdentityV1::from_untrusted_bytes([3; 32]),
            output,
            ObligationSetIdentityV1::from_untrusted_bytes([4; 32]),
            obligations,
            outcome,
        )
    }

    #[test]
    fn produced_receipts_require_output_and_obligations() {
        assert_eq!(
            receipt(
                None,
                Some(ObligationSetIdentityV1::from_untrusted_bytes([5; 32])),
                ReceiptOutcomeV1::Produced,
            ),
            Err(StageReceiptErrorV1::ProducedWithoutOutput)
        );
        assert_eq!(
            receipt(
                Some(SnapshotIdentityV1::from_untrusted_bytes([6; 32])),
                None,
                ReceiptOutcomeV1::Produced,
            ),
            Err(StageReceiptErrorV1::ProducedWithoutOutputObligations)
        );
    }

    #[test]
    fn rejected_receipts_cannot_claim_committed_state() {
        assert_eq!(
            receipt(
                Some(SnapshotIdentityV1::from_untrusted_bytes([6; 32])),
                None,
                ReceiptOutcomeV1::Rejected,
            ),
            Err(StageReceiptErrorV1::RejectedWithOutput)
        );
        assert_eq!(
            receipt(
                None,
                Some(ObligationSetIdentityV1::from_untrusted_bytes([5; 32])),
                ReceiptOutcomeV1::Rejected,
            ),
            Err(StageReceiptErrorV1::RejectedWithOutputObligations)
        );
        assert!(receipt(None, None, ReceiptOutcomeV1::Rejected).is_ok());
    }
}
