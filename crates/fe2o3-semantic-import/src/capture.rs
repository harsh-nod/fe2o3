use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use fe2o3_semantic_trace::{
    ContentIdentitySchemeV1, ContentIdentityV1, KernelIrIdentityClaimV1, LaunchGeometryV1,
    OpaqueIdentityV1, WaveWidthV1,
};
use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest, Sha256};

pub const CAPTURE_SCHEMA_VERSION_V1: u16 = 1;
pub const MAX_CAPTURE_BYTES_V1: u64 = 8 * 1024 * 1024;
pub const MAX_CAPTURE_DISPATCHES_V1: usize = 16_384;
pub const CAPTURE_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.semantic-capture.v1\0";
pub(crate) const RUN_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.semantic-capture.run.v1\0";

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CaptureIdentityV1([u8; 32]);

impl CaptureIdentityV1 {
    pub fn new(bytes: [u8; 32]) -> Result<Self, CaptureErrorV1> {
        if bytes == [0; 32] {
            return Err(CaptureErrorV1::ZeroIdentity);
        }
        Ok(Self(bytes))
    }

    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

impl From<OpaqueIdentityV1> for CaptureIdentityV1 {
    fn from(value: OpaqueIdentityV1) -> Self {
        Self(*value.as_bytes())
    }
}

impl Serialize for CaptureIdentityV1 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut output = [0_u8; 64];
        for (index, byte) in self.0.iter().copied().enumerate() {
            output[index * 2] = hex_digit(byte >> 4);
            output[index * 2 + 1] = hex_digit(byte & 0x0f);
        }
        serializer.serialize_str(std::str::from_utf8(&output).expect("hex is ASCII"))
    }
}

impl<'de> Deserialize<'de> for CaptureIdentityV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct IdentityVisitor;
        impl Visitor<'_> for IdentityVisitor {
            type Value = CaptureIdentityV1;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("exactly 64 lowercase hexadecimal characters")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                if value.len() != 64 {
                    return Err(E::invalid_length(value.len(), &self));
                }
                let mut bytes = [0_u8; 32];
                for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
                    bytes[index] = (parse_hex(pair[0])
                        .ok_or_else(|| E::custom("identity is not lowercase hex"))?
                        << 4)
                        | parse_hex(pair[1])
                            .ok_or_else(|| E::custom("identity is not lowercase hex"))?;
                }
                CaptureIdentityV1::new(bytes).map_err(E::custom)
            }
        }
        deserializer.deserialize_str(IdentityVisitor)
    }
}

const fn hex_digit(value: u8) -> u8 {
    if value < 10 {
        b'0' + value
    } else {
        b'a' + value - 10
    }
}

const fn parse_hex(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TruthOriginV1 {
    Declared,
    Proved,
    Observed,
    Inferred,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureUnavailableReasonV1 {
    NotRecorded,
    NotProvided,
    NotRepresented,
    OutsideCaptureScope,
    CollectorLossUnknown,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentSchemeV1 {
    RawCanonicalSha256,
    DomainSeparatedSha256,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContentIdentityRecordV1 {
    pub scheme: ContentSchemeV1,
    pub format_version: u16,
    pub digest: CaptureIdentityV1,
    pub canonical_len: u64,
}

impl From<ContentIdentityV1> for ContentIdentityRecordV1 {
    fn from(value: ContentIdentityV1) -> Self {
        Self {
            scheme: match value.scheme() {
                ContentIdentitySchemeV1::RawCanonicalSha256 => ContentSchemeV1::RawCanonicalSha256,
                ContentIdentitySchemeV1::DomainSeparatedSha256 => {
                    ContentSchemeV1::DomainSeparatedSha256
                }
            },
            format_version: value.format_version(),
            digest: value.digest().into(),
            canonical_len: value.canonical_len(),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IdentityFactV1 {
    pub origin: TruthOriginV1,
    pub value: Option<ContentIdentityRecordV1>,
    pub unavailable_reason: Option<CaptureUnavailableReasonV1>,
}

impl IdentityFactV1 {
    pub const fn declared(value: ContentIdentityRecordV1) -> Self {
        Self {
            origin: TruthOriginV1::Declared,
            value: Some(value),
            unavailable_reason: None,
        }
    }

    pub const fn unavailable(reason: CaptureUnavailableReasonV1) -> Self {
        Self {
            origin: TruthOriginV1::Unavailable,
            value: None,
            unavailable_reason: Some(reason),
        }
    }

    pub(crate) fn validate(self) -> Result<(), CaptureErrorV1> {
        match (self.origin, self.value, self.unavailable_reason) {
            (TruthOriginV1::Unavailable, None, Some(_)) => Ok(()),
            (TruthOriginV1::Unavailable, _, _) => Err(CaptureErrorV1::InvalidUnavailableFact),
            (TruthOriginV1::Declared, Some(value), None)
                if value.format_version != 0 && value.canonical_len != 0 =>
            {
                Ok(())
            }
            _ => Err(CaptureErrorV1::InvalidAvailableFact),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct KernelIrClaimRecordV1 {
    pub origin: TruthOriginV1,
    pub wire_version: u16,
    pub identity_policy: u16,
    pub digest: CaptureIdentityV1,
    pub canonical_len: u64,
}

impl From<KernelIrIdentityClaimV1> for KernelIrClaimRecordV1 {
    fn from(value: KernelIrIdentityClaimV1) -> Self {
        Self {
            origin: TruthOriginV1::Declared,
            wire_version: value.wire_version(),
            identity_policy: value.identity_policy(),
            digest: value.digest().into(),
            canonical_len: value.canonical_len(),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureSourceKindV1 {
    Rocprofv3KernelDispatchJson,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CaptureRunV1 {
    pub identity: CaptureIdentityV1,
    pub identity_origin: TruthOriginV1,
    pub source: ContentIdentityRecordV1,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CaptureDeviceV1 {
    pub identity: CaptureIdentityV1,
    pub identity_origin: TruthOriginV1,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SamplingModeV1 {
    NotSampled,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SamplingStatusV1 {
    pub origin: TruthOriginV1,
    pub mode: SamplingModeV1,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LossStateV1 {
    Unknown,
    Reported,
    NoneReported,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LossStatusV1 {
    pub origin: TruthOriginV1,
    pub state: LossStateV1,
    pub lost_records: Option<u64>,
    pub unavailable_reason: Option<CaptureUnavailableReasonV1>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CompletenessScopeV1 {
    AllStructuredDispatchRecords,
    PartialSemanticExecutionHistory,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CaptureCoverageV1 {
    pub origin: TruthOriginV1,
    pub scope: CompletenessScopeV1,
    pub source_dispatch_records: u64,
    pub captured_dispatch_records: u64,
    pub sampling: SamplingStatusV1,
    pub loss: LossStatusV1,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LaunchRecordV1 {
    pub logical_grid: [u64; 3],
    pub grid_workgroups: [u32; 3],
    pub workgroup_size: [u32; 3],
    pub wave_width: u16,
}

impl From<LaunchGeometryV1> for LaunchRecordV1 {
    fn from(value: LaunchGeometryV1) -> Self {
        Self {
            logical_grid: value.logical_grid(),
            grid_workgroups: value.grid_workgroups(),
            workgroup_size: value.workgroup_size(),
            wave_width: match value.wave_width() {
                WaveWidthV1::Wave32 => 32,
                WaveWidthV1::Wave64 => 64,
            },
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CaptureDispatchV1 {
    pub identity: CaptureIdentityV1,
    pub identity_origin: TruthOriginV1,
    pub run_identity: CaptureIdentityV1,
    pub device_identity: CaptureIdentityV1,
    pub process_index: u32,
    pub dispatch_index: u32,
    pub source_record_ordinal: u64,
    pub kernel_ir: KernelIrClaimRecordV1,
    pub artifact: IdentityFactV1,
    pub source_map: IdentityFactV1,
    pub launch_origin: TruthOriginV1,
    pub launch: LaunchRecordV1,
    pub timing_origin: TruthOriginV1,
    pub start_timestamp: u64,
    pub end_timestamp: u64,
    pub duration_ticks: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticCaptureV1 {
    pub schema_version: u16,
    pub source_kind: CaptureSourceKindV1,
    pub runs: Vec<CaptureRunV1>,
    pub devices: Vec<CaptureDeviceV1>,
    pub dispatches: Vec<CaptureDispatchV1>,
    pub coverage: CaptureCoverageV1,
}

impl SemanticCaptureV1 {
    pub fn validate(&self) -> Result<(), CaptureErrorV1> {
        if self.schema_version != CAPTURE_SCHEMA_VERSION_V1 {
            return Err(CaptureErrorV1::UnsupportedVersion(self.schema_version));
        }
        if self.runs.len() != 1 {
            return Err(CaptureErrorV1::InvalidRunCount);
        }
        if self.dispatches.is_empty() || self.dispatches.len() > MAX_CAPTURE_DISPATCHES_V1 {
            return Err(CaptureErrorV1::DispatchCountOutOfRange);
        }
        if self.devices.is_empty() || self.devices.len() > self.dispatches.len() {
            return Err(CaptureErrorV1::InvalidDeviceCount);
        }
        let run = &self.runs[0];
        if run.source.scheme != ContentSchemeV1::DomainSeparatedSha256
            || run.source.format_version != 1
            || run.source.canonical_len == 0
        {
            return Err(CaptureErrorV1::InvalidSourceIdentity);
        }
        if run.identity_origin != TruthOriginV1::Observed
            || run.identity != derive_identity(RUN_IDENTITY_DOMAIN_V1, run.source.digest, 0)?
        {
            return Err(CaptureErrorV1::StaleRunIdentity);
        }
        let device_ids: BTreeSet<_> = self.devices.iter().map(|device| device.identity).collect();
        if device_ids.len() != self.devices.len()
            || self
                .devices
                .windows(2)
                .any(|pair| pair[0].identity >= pair[1].identity)
            || self
                .devices
                .iter()
                .any(|device| device.identity_origin != TruthOriginV1::Observed)
        {
            return Err(CaptureErrorV1::InvalidDeviceIdentity);
        }
        let mut prior_selector: Option<(u32, u32)> = None;
        let mut dispatch_ids = BTreeSet::new();
        for (expected_ordinal, dispatch) in (0_u64..).zip(&self.dispatches) {
            if dispatch.run_identity != run.identity
                || !device_ids.contains(&dispatch.device_identity)
            {
                return Err(CaptureErrorV1::StaleReference);
            }
            if dispatch.identity_origin != TruthOriginV1::Observed
                || dispatch.identity
                    != derive_identity(
                        super::ROCPROF_DISPATCH_IDENTITY_DOMAIN_V1,
                        run.source.digest,
                        dispatch.source_record_ordinal,
                    )?
            {
                return Err(CaptureErrorV1::StaleDispatchIdentity);
            }
            if !dispatch_ids.insert(dispatch.identity)
                || dispatch.source_record_ordinal != expected_ordinal
            {
                return Err(CaptureErrorV1::NonCanonicalDispatchOrder);
            }
            if dispatch.dispatch_index != 0 && prior_selector.is_none() {
                return Err(CaptureErrorV1::NonCanonicalSourceSelector);
            }
            if let Some((prior_process, prior_dispatch)) = prior_selector {
                let valid = if dispatch.process_index == prior_process {
                    dispatch.dispatch_index
                        == prior_dispatch
                            .checked_add(1)
                            .ok_or(CaptureErrorV1::NonCanonicalSourceSelector)?
                } else {
                    dispatch.process_index > prior_process && dispatch.dispatch_index == 0
                };
                if !valid {
                    return Err(CaptureErrorV1::NonCanonicalSourceSelector);
                }
            }
            prior_selector = Some((dispatch.process_index, dispatch.dispatch_index));
            if dispatch.start_timestamp > dispatch.end_timestamp
                || dispatch.end_timestamp - dispatch.start_timestamp != dispatch.duration_ticks
            {
                return Err(CaptureErrorV1::InvalidTiming);
            }
            if dispatch.kernel_ir.origin != TruthOriginV1::Declared
                || dispatch.kernel_ir.wire_version != 7
                || dispatch.kernel_ir.identity_policy != 1
                || dispatch.kernel_ir.canonical_len == 0
            {
                return Err(CaptureErrorV1::InvalidKernelIrClaim);
            }
            dispatch.artifact.validate()?;
            dispatch.source_map.validate()?;
            if dispatch.launch_origin != TruthOriginV1::Observed
                || dispatch.timing_origin != TruthOriginV1::Observed
                || !matches!(dispatch.launch.wave_width, 32 | 64)
                || dispatch.launch.logical_grid.contains(&0)
                || dispatch.launch.grid_workgroups.contains(&0)
                || dispatch.launch.workgroup_size.contains(&0)
            {
                return Err(CaptureErrorV1::InvalidObservedEnvelope);
            }
        }
        let dispatch_count =
            u64::try_from(self.dispatches.len()).map_err(|_| CaptureErrorV1::SizeOverflow)?;
        if self.coverage.origin != TruthOriginV1::Declared
            || self.coverage.scope != CompletenessScopeV1::PartialSemanticExecutionHistory
            || self.coverage.source_dispatch_records != dispatch_count
            || self.coverage.captured_dispatch_records != dispatch_count
            || self.coverage.sampling
                != (SamplingStatusV1 {
                    origin: TruthOriginV1::Declared,
                    mode: SamplingModeV1::NotSampled,
                })
            || self.coverage.loss
                != (LossStatusV1 {
                    origin: TruthOriginV1::Unavailable,
                    state: LossStateV1::Unknown,
                    lost_records: None,
                    unavailable_reason: Some(CaptureUnavailableReasonV1::CollectorLossUnknown),
                })
        {
            return Err(CaptureErrorV1::InvalidCoverage);
        }
        Ok(())
    }
}

pub fn encode_capture_v1(capture: &SemanticCaptureV1) -> Result<Vec<u8>, CaptureErrorV1> {
    capture.validate()?;
    let bytes = serde_json::to_vec(capture).map_err(|_| CaptureErrorV1::JsonEncode)?;
    if bytes.len() as u64 > MAX_CAPTURE_BYTES_V1 {
        return Err(CaptureErrorV1::CaptureTooLarge {
            actual: bytes.len() as u64,
            max: MAX_CAPTURE_BYTES_V1,
        });
    }
    Ok(bytes)
}

pub fn decode_capture_v1(bytes: &[u8]) -> Result<SemanticCaptureV1, CaptureErrorV1> {
    let actual = u64::try_from(bytes.len()).map_err(|_| CaptureErrorV1::SizeOverflow)?;
    if actual == 0 || actual > MAX_CAPTURE_BYTES_V1 {
        return Err(CaptureErrorV1::CaptureTooLarge {
            actual,
            max: MAX_CAPTURE_BYTES_V1,
        });
    }
    let capture: SemanticCaptureV1 =
        serde_json::from_slice(bytes).map_err(|_| CaptureErrorV1::JsonDecode)?;
    capture.validate()?;
    if serde_json::to_vec(&capture).map_err(|_| CaptureErrorV1::JsonEncode)? != bytes {
        return Err(CaptureErrorV1::NonCanonicalEncoding);
    }
    Ok(capture)
}

pub fn capture_content_identity_v1(
    bytes: &[u8],
) -> Result<ContentIdentityRecordV1, CaptureErrorV1> {
    let _ = decode_capture_v1(bytes)?;
    let mut hasher = Sha256::new();
    hasher.update(CAPTURE_IDENTITY_DOMAIN_V1);
    hasher.update(bytes);
    Ok(ContentIdentityRecordV1 {
        scheme: ContentSchemeV1::DomainSeparatedSha256,
        format_version: CAPTURE_SCHEMA_VERSION_V1,
        digest: CaptureIdentityV1::new(hasher.finalize().into())?,
        canonical_len: u64::try_from(bytes.len()).map_err(|_| CaptureErrorV1::SizeOverflow)?,
    })
}

pub(crate) fn derive_identity(
    domain: &[u8],
    source: CaptureIdentityV1,
    ordinal: u64,
) -> Result<CaptureIdentityV1, CaptureErrorV1> {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(source.as_bytes());
    hasher.update(ordinal.to_le_bytes());
    CaptureIdentityV1::new(hasher.finalize().into())
}

#[derive(Debug)]
pub enum CaptureErrorV1 {
    ZeroIdentity,
    UnsupportedVersion(u16),
    InvalidRunCount,
    DispatchCountOutOfRange,
    InvalidDeviceCount,
    InvalidDeviceIdentity,
    InvalidSourceIdentity,
    StaleRunIdentity,
    StaleDispatchIdentity,
    StaleReference,
    NonCanonicalDispatchOrder,
    NonCanonicalSourceSelector,
    InvalidTiming,
    InvalidKernelIrClaim,
    InvalidUnavailableFact,
    InvalidAvailableFact,
    InvalidObservedEnvelope,
    InvalidCoverage,
    MissingDeviceIdentity,
    SizeOverflow,
    CaptureTooLarge { actual: u64, max: u64 },
    NonCanonicalEncoding,
    JsonEncode,
    JsonDecode,
}

impl fmt::Display for CaptureErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "semantic capture rejected: {self:?}")
    }
}

impl Error for CaptureErrorV1 {}
