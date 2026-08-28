use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    CaptureDeviceV1, CaptureIdentityV1, CaptureRunV1, CaptureUnavailableReasonV1,
    CompletenessScopeV1, IdentityFactV1, KernelIrClaimRecordV1, LaunchRecordV1, LossStateV1,
    LossStatusV1, TruthOriginV1,
};

pub const COUNTER_CAPTURE_SCHEMA_VERSION_V2: u16 = 2;
pub const MAX_COUNTER_CAPTURE_BYTES_V2: u64 = 8 * 1024 * 1024;
pub const MAX_COUNTER_DEFINITIONS_V2: usize = 16_384;
pub const MAX_COUNTER_DISPATCHES_V2: usize = 16_384;
pub const MAX_COUNTER_VALUES_V2: usize = 1_000_000;
pub const MAX_COUNTER_NAME_BYTES_V2: usize = 128;
pub const COUNTER_CAPTURE_IDENTITY_DOMAIN_V2: &[u8] = b"fe2o3.semantic-counter-capture.v2\0";
pub(crate) const COUNTER_DEFINITION_IDENTITY_DOMAIN_V2: &[u8] =
    b"fe2o3.semantic-counter-capture.definition.v2\0";
pub(crate) const COUNTER_DISPATCH_IDENTITY_DOMAIN_V2: &[u8] =
    b"fe2o3.semantic-counter-capture.dispatch.v2\0";
pub(crate) const COUNTER_VALUE_IDENTITY_DOMAIN_V2: &[u8] =
    b"fe2o3.semantic-counter-capture.value.v2\0";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CounterCaptureSourceKindV2 {
    Rocprofv3DispatchCounterJson,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CounterDefinitionV2 {
    pub identity: CaptureIdentityV1,
    pub identity_origin: TruthOriginV1,
    pub source_definition_ordinal: u64,
    pub device_identity: CaptureIdentityV1,
    pub name: String,
    pub name_origin: TruthOriginV1,
    pub is_constant: bool,
    pub is_derived: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CounterValueV2 {
    pub identity: CaptureIdentityV1,
    pub origin: TruthOriginV1,
    pub counter_identity: CaptureIdentityV1,
    pub source_record_ordinal: u64,
    /// Exact IEEE-754 binary64 bits parsed from the structured JSON number.
    pub value_f64_bits: u64,
}

impl CounterValueV2 {
    pub fn value(self) -> f64 {
        f64::from_bits(self.value_f64_bits)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CounterDispatchV2 {
    pub identity: CaptureIdentityV1,
    pub identity_origin: TruthOriginV1,
    pub run_identity: CaptureIdentityV1,
    pub device_identity: CaptureIdentityV1,
    pub process_index: u32,
    pub collection_index: u32,
    pub source_collection_ordinal: u64,
    pub kernel_ir: KernelIrClaimRecordV1,
    pub artifact: IdentityFactV1,
    pub source_map: IdentityFactV1,
    pub source_and_isa_correlation: CounterCorrelationStatusV2,
    pub launch_origin: TruthOriginV1,
    pub launch: LaunchRecordV1,
    pub timing_origin: TruthOriginV1,
    pub start_timestamp: u64,
    pub end_timestamp: u64,
    pub duration_ticks: u64,
    pub values: Vec<CounterValueV2>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CounterCorrelationStatusV2 {
    UnavailableNoAuthenticatedSourceOrIsaMap,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CounterSamplingModeV2 {
    DispatchSynchronous,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CounterSamplingStatusV2 {
    pub origin: TruthOriginV1,
    pub mode: CounterSamplingModeV2,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CounterCaptureCoverageV2 {
    pub origin: TruthOriginV1,
    pub scope: CompletenessScopeV1,
    pub source_collection_records: u64,
    pub captured_collection_records: u64,
    pub source_counter_value_records: u64,
    pub captured_counter_value_records: u64,
    pub sampling: CounterSamplingStatusV2,
    pub loss: LossStatusV1,
    pub dimension_correlation: CounterDimensionCorrelationV2,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CounterDimensionCorrelationV2 {
    UnavailableRecordHasNoInstanceIdentity,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticCounterCaptureV2 {
    pub schema_version: u16,
    pub source_kind: CounterCaptureSourceKindV2,
    pub runs: Vec<CaptureRunV1>,
    pub devices: Vec<CaptureDeviceV1>,
    pub counter_definitions: Vec<CounterDefinitionV2>,
    pub dispatches: Vec<CounterDispatchV2>,
    pub coverage: CounterCaptureCoverageV2,
}

impl SemanticCounterCaptureV2 {
    pub fn validate(&self) -> Result<(), CounterCaptureErrorV2> {
        if self.schema_version != COUNTER_CAPTURE_SCHEMA_VERSION_V2 {
            return Err(CounterCaptureErrorV2::UnsupportedVersion(
                self.schema_version,
            ));
        }
        if self.runs.len() != 1 {
            return Err(CounterCaptureErrorV2::InvalidRunCount);
        }
        if self.devices.is_empty() || self.devices.len() > self.counter_definitions.len() {
            return Err(CounterCaptureErrorV2::InvalidDeviceCatalog);
        }
        if self.counter_definitions.is_empty()
            || self.counter_definitions.len() > MAX_COUNTER_DEFINITIONS_V2
        {
            return Err(CounterCaptureErrorV2::InvalidCounterCatalog);
        }
        if self.dispatches.is_empty() || self.dispatches.len() > MAX_COUNTER_DISPATCHES_V2 {
            return Err(CounterCaptureErrorV2::InvalidDispatchCount);
        }
        let run = &self.runs[0];
        if run.identity_origin != TruthOriginV1::Observed
            || run.identity
                != crate::capture::derive_identity(
                    crate::capture::RUN_IDENTITY_DOMAIN_V1,
                    run.source.digest,
                    0,
                )
                .map_err(|_| CounterCaptureErrorV2::IdentityFailure)?
        {
            return Err(CounterCaptureErrorV2::StaleRunIdentity);
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
            return Err(CounterCaptureErrorV2::InvalidDeviceCatalog);
        }
        let mut counter_ids = BTreeSet::new();
        let mut prior_definition_ordinal = None;
        for definition in &self.counter_definitions {
            if !counter_ids.insert(definition.identity)
                || !device_ids.contains(&definition.device_identity)
                || definition.identity_origin != TruthOriginV1::Observed
                || definition.identity
                    != crate::capture::derive_identity(
                        COUNTER_DEFINITION_IDENTITY_DOMAIN_V2,
                        run.source.digest,
                        definition.source_definition_ordinal,
                    )
                    .map_err(|_| CounterCaptureErrorV2::IdentityFailure)?
                || prior_definition_ordinal
                    .is_some_and(|prior| prior >= definition.source_definition_ordinal)
                || definition.name_origin != TruthOriginV1::Observed
                || definition.name.is_empty()
                || definition.name.len() > MAX_COUNTER_NAME_BYTES_V2
                || definition.name.contains('\0')
            {
                return Err(CounterCaptureErrorV2::InvalidCounterCatalog);
            }
            prior_definition_ordinal = Some(definition.source_definition_ordinal);
        }

        let mut value_count = 0_u64;
        let mut expected_value_ordinal = 0_u64;
        let mut prior_selector: Option<(u32, u32)> = None;
        let mut dispatch_ids = BTreeSet::new();
        for (expected_collection_ordinal, dispatch) in (0_u64..).zip(&self.dispatches) {
            if dispatch.run_identity != run.identity
                || !device_ids.contains(&dispatch.device_identity)
                || dispatch.identity_origin != TruthOriginV1::Observed
                || dispatch.source_collection_ordinal != expected_collection_ordinal
                || dispatch.identity
                    != crate::capture::derive_identity(
                        COUNTER_DISPATCH_IDENTITY_DOMAIN_V2,
                        run.source.digest,
                        expected_collection_ordinal,
                    )
                    .map_err(|_| CounterCaptureErrorV2::IdentityFailure)?
                || !dispatch_ids.insert(dispatch.identity)
            {
                return Err(CounterCaptureErrorV2::InvalidDispatchIdentity);
            }
            validate_selector(
                prior_selector,
                dispatch.process_index,
                dispatch.collection_index,
            )?;
            prior_selector = Some((dispatch.process_index, dispatch.collection_index));
            if dispatch.start_timestamp > dispatch.end_timestamp
                || dispatch.end_timestamp - dispatch.start_timestamp != dispatch.duration_ticks
                || dispatch.launch_origin != TruthOriginV1::Observed
                || dispatch.timing_origin != TruthOriginV1::Observed
                || dispatch.source_and_isa_correlation
                    != CounterCorrelationStatusV2::UnavailableNoAuthenticatedSourceOrIsaMap
            {
                return Err(CounterCaptureErrorV2::InvalidDispatchEnvelope);
            }
            if dispatch.kernel_ir.origin != TruthOriginV1::Declared
                || dispatch.kernel_ir.wire_version != 7
                || dispatch.kernel_ir.identity_policy != 1
                || dispatch.kernel_ir.canonical_len == 0
                || !matches!(dispatch.launch.wave_width, 32 | 64)
                || dispatch.launch.logical_grid.contains(&0)
                || dispatch.launch.grid_workgroups.contains(&0)
                || dispatch.launch.workgroup_size.contains(&0)
            {
                return Err(CounterCaptureErrorV2::InvalidDispatchEnvelope);
            }
            dispatch
                .artifact
                .validate()
                .map_err(|_| CounterCaptureErrorV2::InvalidContentClaim)?;
            dispatch
                .source_map
                .validate()
                .map_err(|_| CounterCaptureErrorV2::InvalidContentClaim)?;
            if dispatch.values.is_empty() {
                return Err(CounterCaptureErrorV2::EmptyCounterValues);
            }
            for value in &dispatch.values {
                if value.source_record_ordinal != expected_value_ordinal
                    || value.identity
                        != crate::capture::derive_identity(
                            COUNTER_VALUE_IDENTITY_DOMAIN_V2,
                            run.source.digest,
                            expected_value_ordinal,
                        )
                        .map_err(|_| CounterCaptureErrorV2::IdentityFailure)?
                    || value.origin != TruthOriginV1::Observed
                    || !counter_ids.contains(&value.counter_identity)
                    || !value.value().is_finite()
                {
                    return Err(CounterCaptureErrorV2::InvalidCounterValue);
                }
                expected_value_ordinal = expected_value_ordinal
                    .checked_add(1)
                    .ok_or(CounterCaptureErrorV2::SizeOverflow)?;
                value_count = value_count
                    .checked_add(1)
                    .ok_or(CounterCaptureErrorV2::SizeOverflow)?;
            }
        }
        if value_count
            > u64::try_from(MAX_COUNTER_VALUES_V2)
                .map_err(|_| CounterCaptureErrorV2::SizeOverflow)?
        {
            return Err(CounterCaptureErrorV2::TooManyCounterValues);
        }
        let dispatch_count = u64::try_from(self.dispatches.len())
            .map_err(|_| CounterCaptureErrorV2::SizeOverflow)?;
        if self.coverage.origin != TruthOriginV1::Declared
            || self.coverage.scope != CompletenessScopeV1::PartialSemanticExecutionHistory
            || self.coverage.source_collection_records != dispatch_count
            || self.coverage.captured_collection_records != dispatch_count
            || self.coverage.source_counter_value_records != value_count
            || self.coverage.captured_counter_value_records != value_count
            || self.coverage.sampling
                != (CounterSamplingStatusV2 {
                    origin: TruthOriginV1::Declared,
                    mode: CounterSamplingModeV2::DispatchSynchronous,
                })
            || self.coverage.loss
                != (LossStatusV1 {
                    origin: TruthOriginV1::Unavailable,
                    state: LossStateV1::Unknown,
                    lost_records: None,
                    unavailable_reason: Some(CaptureUnavailableReasonV1::CollectorLossUnknown),
                })
            || self.coverage.dimension_correlation
                != CounterDimensionCorrelationV2::UnavailableRecordHasNoInstanceIdentity
        {
            return Err(CounterCaptureErrorV2::InvalidCoverage);
        }
        Ok(())
    }
}

fn validate_selector(
    prior: Option<(u32, u32)>,
    process: u32,
    collection: u32,
) -> Result<(), CounterCaptureErrorV2> {
    match prior {
        None if collection == 0 => Ok(()),
        None => Err(CounterCaptureErrorV2::NonCanonicalSourceSelector),
        Some((prior_process, prior_collection)) if process == prior_process => {
            if collection
                == prior_collection
                    .checked_add(1)
                    .ok_or(CounterCaptureErrorV2::NonCanonicalSourceSelector)?
            {
                Ok(())
            } else {
                Err(CounterCaptureErrorV2::NonCanonicalSourceSelector)
            }
        }
        Some((prior_process, _)) if process > prior_process && collection == 0 => Ok(()),
        Some(_) => Err(CounterCaptureErrorV2::NonCanonicalSourceSelector),
    }
}

pub fn encode_counter_capture_v2(
    capture: &SemanticCounterCaptureV2,
) -> Result<Vec<u8>, CounterCaptureErrorV2> {
    capture.validate()?;
    let bytes = serde_json::to_vec(capture).map_err(|_| CounterCaptureErrorV2::JsonEncode)?;
    if bytes.len() as u64 > MAX_COUNTER_CAPTURE_BYTES_V2 {
        return Err(CounterCaptureErrorV2::CaptureTooLarge {
            actual: bytes.len() as u64,
            max: MAX_COUNTER_CAPTURE_BYTES_V2,
        });
    }
    Ok(bytes)
}

pub fn decode_counter_capture_v2(
    bytes: &[u8],
) -> Result<SemanticCounterCaptureV2, CounterCaptureErrorV2> {
    let actual = u64::try_from(bytes.len()).map_err(|_| CounterCaptureErrorV2::SizeOverflow)?;
    if actual == 0 || actual > MAX_COUNTER_CAPTURE_BYTES_V2 {
        return Err(CounterCaptureErrorV2::CaptureTooLarge {
            actual,
            max: MAX_COUNTER_CAPTURE_BYTES_V2,
        });
    }
    let capture: SemanticCounterCaptureV2 =
        serde_json::from_slice(bytes).map_err(|_| CounterCaptureErrorV2::JsonDecode)?;
    capture.validate()?;
    if serde_json::to_vec(&capture).map_err(|_| CounterCaptureErrorV2::JsonEncode)? != bytes {
        return Err(CounterCaptureErrorV2::NonCanonicalEncoding);
    }
    Ok(capture)
}

pub fn counter_capture_content_identity_v2(
    bytes: &[u8],
) -> Result<crate::ContentIdentityRecordV1, CounterCaptureErrorV2> {
    let _ = decode_counter_capture_v2(bytes)?;
    let mut hasher = Sha256::new();
    hasher.update(COUNTER_CAPTURE_IDENTITY_DOMAIN_V2);
    hasher.update(bytes);
    Ok(crate::ContentIdentityRecordV1 {
        scheme: crate::ContentSchemeV1::DomainSeparatedSha256,
        format_version: COUNTER_CAPTURE_SCHEMA_VERSION_V2,
        digest: CaptureIdentityV1::new(hasher.finalize().into())
            .map_err(|_| CounterCaptureErrorV2::IdentityFailure)?,
        canonical_len: u64::try_from(bytes.len())
            .map_err(|_| CounterCaptureErrorV2::SizeOverflow)?,
    })
}

#[derive(Debug)]
pub enum CounterCaptureErrorV2 {
    UnsupportedVersion(u16),
    InvalidRunCount,
    InvalidDeviceCatalog,
    InvalidCounterCatalog,
    InvalidDispatchCount,
    StaleRunIdentity,
    InvalidDispatchIdentity,
    NonCanonicalSourceSelector,
    InvalidDispatchEnvelope,
    InvalidContentClaim,
    EmptyCounterValues,
    InvalidCounterValue,
    TooManyCounterValues,
    InvalidCoverage,
    IdentityFailure,
    SizeOverflow,
    CaptureTooLarge { actual: u64, max: u64 },
    NonCanonicalEncoding,
    JsonEncode,
    JsonDecode,
}

impl fmt::Display for CounterCaptureErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "semantic counter capture rejected: {self:?}")
    }
}

impl Error for CounterCaptureErrorV2 {}
