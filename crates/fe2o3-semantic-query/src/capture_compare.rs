use std::error::Error;
use std::fmt;
use std::io::{self, Write};

use crate::{CaptureFactV1, CapturePlanLimitationV1, CaptureToolFamilyV1};
use fe2o3_semantic_import::{
    CaptureIdentityV1, ContentIdentityRecordV1, MAX_CAPTURE_BYTES_V1, MAX_COUNTER_CAPTURE_BYTES_V2,
    SemanticCaptureV1, SemanticCounterCaptureV2, TruthOriginV1, capture_content_identity_v1,
    counter_capture_content_identity_v2, decode_capture_v1, decode_counter_capture_v2,
};
use serde::Serialize;

pub const MAX_COMPARISON_INPUT_BYTES_V1: u64 = MAX_CAPTURE_BYTES_V1 + MAX_COUNTER_CAPTURE_BYTES_V2;
pub const MAX_COMPARISON_RESPONSE_BYTES_V1: u64 = 64 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ComparisonCaptureKindV1 {
    DispatchEnvelopeV1,
    DispatchCounterV2,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ComparisonDispositionV1 {
    IdenticalCanonicalEvidenceOnly,
    UnavailableSourceBoundIdentity,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CompatibilityRequirementV1 {
    SourceSchema,
    StableEnvironmentIdentity,
    KernelIrIdentity,
    ArtifactIdentity,
    SourceMapIdentity,
    DeviceIdentity,
    CounterIdentity,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CompatibilityStatusV1 {
    Exact,
    Mismatch,
    Unavailable,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct CompatibilityFactV1 {
    pub requirement: CompatibilityRequirementV1,
    pub status: CompatibilityStatusV1,
    pub origin: TruthOriginV1,
    pub limitation: Option<&'static str>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ComparisonEvidenceV1 {
    pub baseline_capture: ContentIdentityRecordV1,
    pub candidate_capture: ContentIdentityRecordV1,
    pub supporting: Vec<CaptureIdentityV1>,
    pub contradicting: Vec<CaptureIdentityV1>,
    pub blocking: Vec<CaptureIdentityV1>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ComparisonConfidenceV1 {
    ExactCanonicalEqualityOnly,
    Unavailable,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ComparisonNextCaptureV1 {
    pub tool: CaptureToolFamilyV1,
    pub required_facts: Vec<CaptureFactV1>,
    pub limitation: &'static str,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CaptureComparisonV1 {
    pub kind: ComparisonCaptureKindV1,
    pub disposition: ComparisonDispositionV1,
    pub compatibility: Vec<CompatibilityFactV1>,
    pub deltas: Vec<ComparisonDeltaV1>,
    pub evidence: ComparisonEvidenceV1,
    pub confidence: ComparisonConfidenceV1,
    pub limitations: Vec<CapturePlanLimitationV1>,
    pub next_capture: Option<ComparisonNextCaptureV1>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct ComparisonDeltaV1 {
    pub metric: &'static str,
    pub baseline_f64_bits: u64,
    pub candidate_f64_bits: u64,
    pub delta_f64_bits: u64,
    pub origin: TruthOriginV1,
}

pub fn compare_dispatch_captures_v1(
    baseline_bytes: &[u8],
    candidate_bytes: &[u8],
) -> Result<CaptureComparisonV1, CaptureCompareErrorV1> {
    check_total(baseline_bytes, candidate_bytes)?;
    let baseline =
        decode_capture_v1(baseline_bytes).map_err(|_| CaptureCompareErrorV1::InvalidBaseline)?;
    let candidate =
        decode_capture_v1(candidate_bytes).map_err(|_| CaptureCompareErrorV1::InvalidCandidate)?;
    finish(
        ComparisonCaptureKindV1::DispatchEnvelopeV1,
        capture_content_identity_v1(baseline_bytes)
            .map_err(|_| CaptureCompareErrorV1::InvalidBaseline)?,
        capture_content_identity_v1(candidate_bytes)
            .map_err(|_| CaptureCompareErrorV1::InvalidCandidate)?,
        dispatch_compatibility(&baseline, &candidate),
    )
}

pub fn compare_counter_captures_v2(
    baseline_bytes: &[u8],
    candidate_bytes: &[u8],
) -> Result<CaptureComparisonV1, CaptureCompareErrorV1> {
    check_total(baseline_bytes, candidate_bytes)?;
    let baseline = decode_counter_capture_v2(baseline_bytes)
        .map_err(|_| CaptureCompareErrorV1::InvalidBaseline)?;
    let candidate = decode_counter_capture_v2(candidate_bytes)
        .map_err(|_| CaptureCompareErrorV1::InvalidCandidate)?;
    finish(
        ComparisonCaptureKindV1::DispatchCounterV2,
        counter_capture_content_identity_v2(baseline_bytes)
            .map_err(|_| CaptureCompareErrorV1::InvalidBaseline)?,
        counter_capture_content_identity_v2(candidate_bytes)
            .map_err(|_| CaptureCompareErrorV1::InvalidCandidate)?,
        counter_compatibility(&baseline, &candidate),
    )
}

fn dispatch_compatibility(
    left: &SemanticCaptureV1,
    right: &SemanticCaptureV1,
) -> Vec<CompatibilityFactV1> {
    let mut pairs = left.dispatches.iter().zip(&right.dispatches);
    vec![
        exact(
            CompatibilityRequirementV1::SourceSchema,
            true,
            TruthOriginV1::Proved,
        ),
        unavailable_environment(),
        exact(
            CompatibilityRequirementV1::KernelIrIdentity,
            left.dispatches.len() == right.dispatches.len()
                && pairs.clone().all(|(a, b)| a.kernel_ir == b.kernel_ir),
            TruthOriginV1::Declared,
        ),
        exact(
            CompatibilityRequirementV1::ArtifactIdentity,
            left.dispatches.len() == right.dispatches.len()
                && pairs.clone().all(|(a, b)| a.artifact == b.artifact),
            TruthOriginV1::Declared,
        ),
        exact(
            CompatibilityRequirementV1::SourceMapIdentity,
            left.dispatches.len() == right.dispatches.len()
                && pairs.all(|(a, b)| a.source_map == b.source_map),
            TruthOriginV1::Declared,
        ),
        exact(
            CompatibilityRequirementV1::DeviceIdentity,
            left.devices == right.devices,
            TruthOriginV1::Observed,
        ),
    ]
}

fn counter_compatibility(
    left: &SemanticCounterCaptureV2,
    right: &SemanticCounterCaptureV2,
) -> Vec<CompatibilityFactV1> {
    let mut pairs = left.dispatches.iter().zip(&right.dispatches);
    vec![
        exact(
            CompatibilityRequirementV1::SourceSchema,
            true,
            TruthOriginV1::Proved,
        ),
        unavailable_environment(),
        exact(
            CompatibilityRequirementV1::KernelIrIdentity,
            left.dispatches.len() == right.dispatches.len()
                && pairs.clone().all(|(a, b)| a.kernel_ir == b.kernel_ir),
            TruthOriginV1::Declared,
        ),
        exact(
            CompatibilityRequirementV1::ArtifactIdentity,
            left.dispatches.len() == right.dispatches.len()
                && pairs.clone().all(|(a, b)| a.artifact == b.artifact),
            TruthOriginV1::Declared,
        ),
        exact(
            CompatibilityRequirementV1::SourceMapIdentity,
            left.dispatches.len() == right.dispatches.len()
                && pairs.all(|(a, b)| a.source_map == b.source_map),
            TruthOriginV1::Declared,
        ),
        exact(
            CompatibilityRequirementV1::DeviceIdentity,
            left.devices == right.devices,
            TruthOriginV1::Observed,
        ),
        exact(
            CompatibilityRequirementV1::CounterIdentity,
            left.counter_definitions == right.counter_definitions,
            TruthOriginV1::Observed,
        ),
    ]
}

fn exact(
    requirement: CompatibilityRequirementV1,
    matches: bool,
    origin: TruthOriginV1,
) -> CompatibilityFactV1 {
    CompatibilityFactV1 {
        requirement,
        status: if matches {
            CompatibilityStatusV1::Exact
        } else {
            CompatibilityStatusV1::Mismatch
        },
        origin,
        limitation: matches!(origin, TruthOriginV1::Declared).then_some(
            "exact equality of caller-declared content claims; correlation is not authenticated",
        ),
    }
}
fn unavailable_environment() -> CompatibilityFactV1 {
    CompatibilityFactV1 {
        requirement: CompatibilityRequirementV1::StableEnvironmentIdentity,
        status: CompatibilityStatusV1::Unavailable,
        origin: TruthOriginV1::Unavailable,
        limitation: Some("capture records no authenticated stable environment identity"),
    }
}

fn finish(
    kind: ComparisonCaptureKindV1,
    baseline: ContentIdentityRecordV1,
    candidate: ContentIdentityRecordV1,
    compatibility: Vec<CompatibilityFactV1>,
) -> Result<CaptureComparisonV1, CaptureCompareErrorV1> {
    let identical = baseline == candidate;
    let pair = vec![baseline.digest, candidate.digest];
    Ok(CaptureComparisonV1 {
        kind,
        disposition: if identical {
            ComparisonDispositionV1::IdenticalCanonicalEvidenceOnly
        } else {
            ComparisonDispositionV1::UnavailableSourceBoundIdentity
        },
        compatibility,
        deltas: Vec::new(),
        evidence: ComparisonEvidenceV1 {
            baseline_capture: baseline,
            candidate_capture: candidate,
            supporting: if identical {
                vec![baseline.digest]
            } else {
                Vec::new()
            },
            contradicting: Vec::new(),
            blocking: if identical {
                vec![baseline.digest]
            } else {
                pair
            },
        },
        confidence: if identical {
            ComparisonConfidenceV1::ExactCanonicalEqualityOnly
        } else {
            ComparisonConfidenceV1::Unavailable
        },
        limitations: vec![
            CapturePlanLimitationV1::NoDiagnosisClaim,
            CapturePlanLimitationV1::NoPerformancePrediction,
        ],
        next_capture: Some(ComparisonNextCaptureV1 {
            tool: CaptureToolFamilyV1::Rocprofv3Counters,
            required_facts: vec![
                CaptureFactV1::StableEnvironmentIdentity,
                CaptureFactV1::StableDeviceIdentity,
                CaptureFactV1::StableCounterIdentity,
            ],
            limitation: "future capture schema must authenticate stable environment, device, and counter identities across runs; current source-bound identities cannot establish comparability",
        }),
    })
}

fn check_total(left: &[u8], right: &[u8]) -> Result<(), CaptureCompareErrorV1> {
    let total = u64::try_from(left.len())
        .ok()
        .and_then(|n| n.checked_add(u64::try_from(right.len()).ok()?))
        .ok_or(CaptureCompareErrorV1::SizeOverflow)?;
    if total == 0 || total > MAX_COMPARISON_INPUT_BYTES_V1 {
        return Err(CaptureCompareErrorV1::InputTooLarge);
    }
    Ok(())
}

pub fn encode_capture_comparison_v1(
    value: &CaptureComparisonV1,
) -> Result<Vec<u8>, CaptureCompareErrorV1> {
    let mut output = Vec::new();
    let mut writer = BoundedWriter {
        output: &mut output,
        exceeded: false,
    };
    if serde_json::to_writer(&mut writer, value).is_err() {
        return Err(if writer.exceeded {
            CaptureCompareErrorV1::ResponseTooLarge
        } else {
            CaptureCompareErrorV1::JsonEncode
        });
    }
    output.push(b'\n');
    Ok(output)
}
struct BoundedWriter<'a> {
    output: &'a mut Vec<u8>,
    exceeded: bool,
}
impl Write for BoundedWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.write_all(bytes)?;
        Ok(bytes.len())
    }
    fn write_all(&mut self, bytes: &[u8]) -> io::Result<()> {
        if self
            .output
            .len()
            .checked_add(bytes.len())
            .is_none_or(|n| n >= MAX_COMPARISON_RESPONSE_BYTES_V1 as usize)
        {
            self.exceeded = true;
            return Err(io::Error::other("comparison response limit exceeded"));
        }
        self.output.extend_from_slice(bytes);
        Ok(())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Debug)]
pub enum CaptureCompareErrorV1 {
    InvalidBaseline,
    InvalidCandidate,
    InputTooLarge,
    SizeOverflow,
    ResponseTooLarge,
    JsonEncode,
}
impl fmt::Display for CaptureCompareErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "capture comparison rejected: {self:?}")
    }
}
impl Error for CaptureCompareErrorV1 {}
