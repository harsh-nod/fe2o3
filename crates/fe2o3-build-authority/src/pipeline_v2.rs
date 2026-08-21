use std::fmt;

use crate::{PipelineAllowlistV1, PipelineV1};

const KNOWN_PIPELINE_BITS_V2: u32 = 0b111;

/// A bounded pipeline selectable by Policy V2.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum PipelineV2 {
    /// The collected row-softmax V1 pipeline.
    CollectedRowSoftmax = 1,
    /// The collected tiled-GEMM V1 pipeline.
    CollectedTiledGemm = 2,
    /// The unified production V1 pipeline.
    ProductionV1 = 3,
}

impl PipelineV2 {
    pub(crate) const fn allowlist_bit(self) -> u32 {
        match self {
            Self::CollectedRowSoftmax => 1 << 0,
            Self::CollectedTiledGemm => 1 << 1,
            Self::ProductionV1 => 1 << 2,
        }
    }

    /// Returns the stable canonical wire value.
    pub const fn wire_value(self) -> u16 {
        self as u16
    }
}

impl TryFrom<u16> for PipelineV2 {
    type Error = PipelineErrorV2;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::CollectedRowSoftmax),
            2 => Ok(Self::CollectedTiledGemm),
            3 => Ok(Self::ProductionV1),
            _ => Err(PipelineErrorV2::UnknownPipeline { value }),
        }
    }
}

impl From<PipelineV1> for PipelineV2 {
    fn from(value: PipelineV1) -> Self {
        match value {
            PipelineV1::CollectedRowSoftmax => Self::CollectedRowSoftmax,
            PipelineV1::CollectedTiledGemm => Self::CollectedTiledGemm,
        }
    }
}

impl TryFrom<PipelineV2> for PipelineV1 {
    type Error = PipelineErrorV2;

    fn try_from(value: PipelineV2) -> Result<Self, Self::Error> {
        match value {
            PipelineV2::CollectedRowSoftmax => Ok(Self::CollectedRowSoftmax),
            PipelineV2::CollectedTiledGemm => Ok(Self::CollectedTiledGemm),
            PipelineV2::ProductionV1 => {
                Err(PipelineErrorV2::PipelineNotRepresentableInV1 { pipeline: value })
            }
        }
    }
}

/// A validated allowlist of Policy V2 pipelines.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PipelineAllowlistV2(u32);

impl PipelineAllowlistV2 {
    /// An allowlist containing only collected row softmax.
    pub const ROW_SOFTMAX: Self = Self(1 << 0);
    /// An allowlist containing only collected tiled GEMM.
    pub const TILED_GEMM: Self = Self(1 << 1);
    /// An allowlist containing only Production V1.
    pub const PRODUCTION_V1: Self = Self(1 << 2);
    /// An allowlist containing the two lanes representable by Policy V1.
    pub const POLICY_V1: Self = Self((1 << 0) | (1 << 1));
    /// An allowlist containing every Policy V2 pipeline.
    pub const ALL: Self = Self(KNOWN_PIPELINE_BITS_V2);

    /// Validates raw Policy V2 allowlist bits.
    pub fn from_bits(bits: u32) -> Result<Self, PipelineErrorV2> {
        if bits & !KNOWN_PIPELINE_BITS_V2 != 0 {
            return Err(PipelineErrorV2::UnknownPipelineAllowlistBits { bits });
        }
        Ok(Self(bits))
    }

    /// Returns the canonical wire bits.
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// Reports whether the pipeline is present in this allowlist.
    pub const fn allows(self, pipeline: PipelineV2) -> bool {
        self.0 & pipeline.allowlist_bit() != 0
    }
}

impl From<PipelineAllowlistV1> for PipelineAllowlistV2 {
    fn from(value: PipelineAllowlistV1) -> Self {
        Self(value.bits())
    }
}

impl TryFrom<PipelineAllowlistV2> for PipelineAllowlistV1 {
    type Error = PipelineErrorV2;

    fn try_from(value: PipelineAllowlistV2) -> Result<Self, Self::Error> {
        if value.bits() & PipelineAllowlistV2::PRODUCTION_V1.bits() != 0 {
            return Err(PipelineErrorV2::AllowlistNotRepresentableInV1 { bits: value.bits() });
        }
        PipelineAllowlistV1::from_bits(value.bits())
            .map_err(|_| PipelineErrorV2::AllowlistNotRepresentableInV1 { bits: value.bits() })
    }
}

/// Why a Pipeline V2 value or compatibility conversion was rejected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PipelineErrorV2 {
    /// The selected pipeline ID is not assigned by V2.
    UnknownPipeline {
        /// The observed pipeline ID.
        value: u16,
    },
    /// The allowlist contains bits not assigned by V2.
    UnknownPipelineAllowlistBits {
        /// The observed allowlist bits.
        bits: u32,
    },
    /// The selected pipeline has no Policy V1 representation.
    PipelineNotRepresentableInV1 {
        /// The rejected V2 pipeline.
        pipeline: PipelineV2,
    },
    /// The allowlist contains a pipeline with no Policy V1 representation.
    AllowlistNotRepresentableInV1 {
        /// The rejected V2 allowlist bits.
        bits: u32,
    },
}

impl fmt::Display for PipelineErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownPipeline { value } => {
                write!(formatter, "unknown Policy V2 selected pipeline {value}")
            }
            Self::UnknownPipelineAllowlistBits { bits } => {
                write!(formatter, "unknown Policy V2 pipeline bits {bits:#x}")
            }
            Self::PipelineNotRepresentableInV1 { pipeline } => {
                write!(
                    formatter,
                    "pipeline {pipeline:?} is not representable by Policy V1"
                )
            }
            Self::AllowlistNotRepresentableInV1 { bits } => write!(
                formatter,
                "pipeline allowlist {bits:#x} is not representable by Policy V1"
            ),
        }
    }
}

impl std::error::Error for PipelineErrorV2 {}
