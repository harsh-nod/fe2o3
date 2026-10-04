use core::{fmt, str};

use crate::{
    AmdTargetFeature, AmdTargetId, CapabilityDerivationError, FeatureState, ParseAmdTargetIdError,
    processor_supports_feature,
};

/// Maximum target-ID text accepted by the V2 resolver.
pub const MAX_AMD_TARGET_ID_BYTES_V2: usize = 64;

/// Maximum number of devices inspected by one V2 auto-detection attempt.
pub const MAX_DETECTED_AMD_DEVICES_V2: usize = 64;

/// Capacity of the canonical V2 resolved-target encoding.
pub const MAX_RESOLVED_AMD_TARGET_CANONICAL_BYTES_V2: usize = 128;

const CANONICAL_PREFIX: &str = "fe2o3-amd-resolved-target-v2{architecture=";
const CANONICAL_SRAM_ECC: &str = ";sramecc=";
const CANONICAL_XNACK: &str = ";xnack=";
const CANONICAL_SOURCE: &str = ";source=";

/// An injectable, bounded source of direct runtime target observations.
///
/// Implementations are expected to query an in-process runtime or driver API.
/// The resolver itself never executes a child process, reads command output,
/// or consults environment variables. Device indices must be stable for the
/// duration of a single resolution call.
pub trait AmdTargetDetectionV2 {
    type Error;

    /// Returns the exact number of visible AMD devices.
    fn device_count(&self) -> Result<usize, Self::Error>;

    /// Returns the exact target ID observed for `device_index`.
    fn target_id(&self, device_index: usize) -> Result<&str, Self::Error>;
}

/// How the resolved target was selected.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ResolvedAmdTargetSourceV2 {
    Detected,
    Override,
}

impl ResolvedAmdTargetSourceV2 {
    const fn canonical_name(self) -> &'static str {
        match self {
            Self::Detected => "detected",
            Self::Override => "override",
        }
    }
}

impl fmt::Display for ResolvedAmdTargetSourceV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.canonical_name())
    }
}

/// A feature state carried by the resolved target identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DetectedTargetFeatureV2 {
    /// The architecture does not expose this target-ID feature.
    Unsupported,
    /// The override intentionally left this supported feature unconstrained.
    Unspecified,
    Disabled,
    Enabled,
}

impl DetectedTargetFeatureV2 {
    const fn canonical_name(self) -> &'static str {
        match self {
            Self::Unsupported => "unsupported",
            Self::Unspecified => "unspecified",
            Self::Disabled => "disabled",
            Self::Enabled => "enabled",
        }
    }
}

impl fmt::Display for DetectedTargetFeatureV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.canonical_name())
    }
}

/// One canonical architecture, feature set, and selection source.
///
/// Construction is restricted to strict override resolution, exact device
/// detection, or decoding the canonical V2 representation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ResolvedAmdTargetIdentityV2 {
    target: AmdTargetId,
    source: ResolvedAmdTargetSourceV2,
}

impl ResolvedAmdTargetIdentityV2 {
    fn new(target: AmdTargetId, source: ResolvedAmdTargetSourceV2) -> Self {
        Self { target, source }
    }

    /// Returns the normalized target ID retained for existing target APIs.
    pub const fn target_id(self) -> AmdTargetId {
        self.target
    }

    /// Returns the concrete canonical architecture name.
    pub const fn architecture(self) -> &'static str {
        self.target.processor()
    }

    /// Returns the resolved SRAM ECC feature state.
    pub const fn sramecc(self) -> DetectedTargetFeatureV2 {
        identity_feature(self.target, AmdTargetFeature::SramEcc)
    }

    /// Returns the resolved XNACK feature state.
    pub const fn xnack(self) -> DetectedTargetFeatureV2 {
        identity_feature(self.target, AmdTargetFeature::Xnack)
    }

    /// Returns whether detection or an explicit override selected the target.
    pub const fn source(self) -> ResolvedAmdTargetSourceV2 {
        self.source
    }

    /// Emits the deterministic, versioned canonical representation.
    pub fn encode_canonical(self) -> CanonicalResolvedAmdTargetBytesV2 {
        let mut output = CanonicalResolvedAmdTargetBytesV2::new();
        output.push(CANONICAL_PREFIX);
        output.push(self.architecture());
        output.push(CANONICAL_SRAM_ECC);
        output.push(self.sramecc().canonical_name());
        output.push(CANONICAL_XNACK);
        output.push(self.xnack().canonical_name());
        output.push(CANONICAL_SOURCE);
        output.push(self.source.canonical_name());
        output.push("}");
        output
    }

    /// Returns SHA-256 over the exact canonical V2 bytes.
    pub fn canonical_digest(self) -> ResolvedAmdTargetDigestV2 {
        ResolvedAmdTargetDigestV2(sha256(self.encode_canonical().as_bytes()))
    }

    /// Decodes only the exact canonical V2 representation.
    pub fn decode_canonical(bytes: &[u8]) -> Result<Self, DecodeResolvedAmdTargetV2Error> {
        if bytes.len() > MAX_RESOLVED_AMD_TARGET_CANONICAL_BYTES_V2 {
            return Err(DecodeResolvedAmdTargetV2Error::TooLong);
        }
        let text = str::from_utf8(bytes).map_err(|_| DecodeResolvedAmdTargetV2Error::NonUtf8)?;
        let body = text
            .strip_prefix(CANONICAL_PREFIX)
            .and_then(|value| value.strip_suffix('}'))
            .ok_or(DecodeResolvedAmdTargetV2Error::InvalidStructure)?;
        let (architecture, body) = body
            .split_once(CANONICAL_SRAM_ECC)
            .ok_or(DecodeResolvedAmdTargetV2Error::InvalidStructure)?;
        let (sramecc, body) = body
            .split_once(CANONICAL_XNACK)
            .ok_or(DecodeResolvedAmdTargetV2Error::InvalidStructure)?;
        let (xnack, source) = body
            .split_once(CANONICAL_SOURCE)
            .ok_or(DecodeResolvedAmdTargetV2Error::InvalidStructure)?;

        let base = match parse_bounded_target(architecture) {
            Ok(target) => target,
            Err(BoundedTargetParseError::TooLong) => {
                return Err(DecodeResolvedAmdTargetV2Error::InvalidStructure);
            }
            Err(BoundedTargetParseError::Invalid(error)) => {
                return Err(DecodeResolvedAmdTargetV2Error::InvalidArchitecture(error));
            }
        };
        if base.processor() != architecture || base.sramecc().is_some() || base.xnack().is_some() {
            return Err(DecodeResolvedAmdTargetV2Error::InvalidStructure);
        }

        let sramecc = decode_feature_state(base, AmdTargetFeature::SramEcc, sramecc)?;
        let xnack = decode_feature_state(base, AmdTargetFeature::Xnack, xnack)?;
        let source = match source {
            "detected" => ResolvedAmdTargetSourceV2::Detected,
            "override" => ResolvedAmdTargetSourceV2::Override,
            _ => return Err(DecodeResolvedAmdTargetV2Error::InvalidSource),
        };
        if source == ResolvedAmdTargetSourceV2::Detected
            && (is_missing_detected_feature(base, AmdTargetFeature::SramEcc, sramecc)
                || is_missing_detected_feature(base, AmdTargetFeature::Xnack, xnack))
        {
            return Err(DecodeResolvedAmdTargetV2Error::IncompleteDetection);
        }

        let target = AmdTargetId {
            processor: base.processor,
            sramecc,
            xnack,
        };
        target
            .capabilities()
            .map_err(DecodeResolvedAmdTargetV2Error::InvalidCapabilities)?;
        let identity = Self::new(target, source);
        if identity.encode_canonical().as_bytes() != bytes {
            return Err(DecodeResolvedAmdTargetV2Error::NonCanonical);
        }
        Ok(identity)
    }
}

/// Fixed-capacity canonical V2 bytes.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CanonicalResolvedAmdTargetBytesV2 {
    bytes: [u8; MAX_RESOLVED_AMD_TARGET_CANONICAL_BYTES_V2],
    len: u8,
}

impl CanonicalResolvedAmdTargetBytesV2 {
    const fn new() -> Self {
        Self {
            bytes: [0; MAX_RESOLVED_AMD_TARGET_CANONICAL_BYTES_V2],
            len: 0,
        }
    }

    fn push(&mut self, value: &str) {
        let start = usize::from(self.len);
        let end = start + value.len();
        assert!(
            end <= self.bytes.len(),
            "canonical target encoding overflow"
        );
        self.bytes[start..end].copy_from_slice(value.as_bytes());
        self.len = u8::try_from(end).expect("canonical target encoding length fits u8");
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..usize::from(self.len)]
    }

    pub const fn len(&self) -> usize {
        self.len as usize
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }
}

/// A SHA-256 digest over canonical resolved-target V2 bytes.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ResolvedAmdTargetDigestV2([u8; 32]);

impl ResolvedAmdTargetDigestV2 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for ResolvedAmdTargetDigestV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

/// A failure to resolve one exact AMD target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolveAmdTargetV2Error<E> {
    OverrideTooLong,
    InvalidOverride(ParseAmdTargetIdError),
    InvalidOverrideCapabilities(CapabilityDerivationError),
    Detection(E),
    NoDevice,
    TooManyDevices {
        observed: usize,
        limit: usize,
    },
    DetectedTargetTooLong {
        device_index: usize,
    },
    InvalidDetectedTarget {
        device_index: usize,
        error: ParseAmdTargetIdError,
    },
    InvalidDetectedCapabilities {
        device_index: usize,
        error: CapabilityDerivationError,
    },
    MissingDetectedFeature {
        device_index: usize,
        feature: AmdTargetFeature,
    },
    AmbiguousDevices {
        first_device_index: usize,
        first: AmdTargetId,
        conflicting_device_index: usize,
        conflicting: AmdTargetId,
    },
}

impl<E: fmt::Display> fmt::Display for ResolveAmdTargetV2Error<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OverrideTooLong => formatter.write_str("AMD target override is too long"),
            Self::InvalidOverride(error) => {
                write!(formatter, "invalid AMD target override: {error}")
            }
            Self::InvalidOverrideCapabilities(error) => {
                write!(
                    formatter,
                    "AMD target override has invalid capabilities: {error}"
                )
            }
            Self::Detection(error) => write!(formatter, "AMD target detection failed: {error}"),
            Self::NoDevice => formatter.write_str("AMD target detection found no device"),
            Self::TooManyDevices { observed, limit } => write!(
                formatter,
                "AMD target detection found {observed} devices, exceeding limit {limit}"
            ),
            Self::DetectedTargetTooLong { device_index } => {
                write!(formatter, "detected AMD target {device_index} is too long")
            }
            Self::InvalidDetectedTarget {
                device_index,
                error,
            } => write!(
                formatter,
                "detected AMD target {device_index} is invalid: {error}"
            ),
            Self::InvalidDetectedCapabilities {
                device_index,
                error,
            } => write!(
                formatter,
                "detected AMD target {device_index} has invalid capabilities: {error}"
            ),
            Self::MissingDetectedFeature {
                device_index,
                feature,
            } => write!(
                formatter,
                "detected AMD target {device_index} omitted supported feature {feature}"
            ),
            Self::AmbiguousDevices {
                first_device_index,
                first,
                conflicting_device_index,
                conflicting,
            } => write!(
                formatter,
                "AMD devices disagree: device {first_device_index} is {first}, device {conflicting_device_index} is {conflicting}"
            ),
        }
    }
}

/// Why canonical resolved-target V2 bytes were rejected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodeResolvedAmdTargetV2Error {
    TooLong,
    NonUtf8,
    InvalidStructure,
    InvalidArchitecture(ParseAmdTargetIdError),
    InvalidFeatureState(AmdTargetFeature),
    InvalidSource,
    IncompleteDetection,
    InvalidCapabilities(CapabilityDerivationError),
    NonCanonical,
}

impl fmt::Display for DecodeResolvedAmdTargetV2Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLong => formatter.write_str("resolved AMD target identity is too long"),
            Self::NonUtf8 => formatter.write_str("resolved AMD target identity is not UTF-8"),
            Self::InvalidStructure => {
                formatter.write_str("resolved AMD target identity has invalid structure")
            }
            Self::InvalidArchitecture(error) => {
                write!(
                    formatter,
                    "resolved AMD target architecture is invalid: {error}"
                )
            }
            Self::InvalidFeatureState(feature) => {
                write!(
                    formatter,
                    "resolved AMD target feature {feature} is invalid"
                )
            }
            Self::InvalidSource => formatter.write_str("resolved AMD target source is invalid"),
            Self::IncompleteDetection => {
                formatter.write_str("detected AMD target identity omitted a supported feature")
            }
            Self::InvalidCapabilities(error) => {
                write!(
                    formatter,
                    "resolved AMD target capabilities are invalid: {error}"
                )
            }
            Self::NonCanonical => {
                formatter.write_str("resolved AMD target identity is not canonical")
            }
        }
    }
}

impl core::error::Error for DecodeResolvedAmdTargetV2Error {}

/// Resolves an explicit override or an unambiguous set of detected devices.
///
/// An override has absolute precedence. Detection is not called when an
/// override is present, including when that override is invalid.
pub fn resolve_amd_target_v2<D: AmdTargetDetectionV2>(
    explicit_override: Option<&str>,
    detection: &D,
) -> Result<ResolvedAmdTargetIdentityV2, ResolveAmdTargetV2Error<D::Error>> {
    if let Some(value) = explicit_override {
        let target = parse_bounded_target(value).map_err(|error| match error {
            BoundedTargetParseError::TooLong => ResolveAmdTargetV2Error::OverrideTooLong,
            BoundedTargetParseError::Invalid(error) => {
                ResolveAmdTargetV2Error::InvalidOverride(error)
            }
        })?;
        target
            .capabilities()
            .map_err(ResolveAmdTargetV2Error::InvalidOverrideCapabilities)?;
        return Ok(ResolvedAmdTargetIdentityV2::new(
            target,
            ResolvedAmdTargetSourceV2::Override,
        ));
    }

    let count = detection
        .device_count()
        .map_err(ResolveAmdTargetV2Error::Detection)?;
    if count == 0 {
        return Err(ResolveAmdTargetV2Error::NoDevice);
    }
    if count > MAX_DETECTED_AMD_DEVICES_V2 {
        return Err(ResolveAmdTargetV2Error::TooManyDevices {
            observed: count,
            limit: MAX_DETECTED_AMD_DEVICES_V2,
        });
    }

    let mut first = None;
    for device_index in 0..count {
        let text = detection
            .target_id(device_index)
            .map_err(ResolveAmdTargetV2Error::Detection)?;
        let target = parse_bounded_target(text).map_err(|error| match error {
            BoundedTargetParseError::TooLong => {
                ResolveAmdTargetV2Error::DetectedTargetTooLong { device_index }
            }
            BoundedTargetParseError::Invalid(error) => {
                ResolveAmdTargetV2Error::InvalidDetectedTarget {
                    device_index,
                    error,
                }
            }
        })?;
        target.capabilities().map_err(|error| {
            ResolveAmdTargetV2Error::InvalidDetectedCapabilities {
                device_index,
                error,
            }
        })?;
        for feature in [AmdTargetFeature::SramEcc, AmdTargetFeature::Xnack] {
            if processor_supports_feature(target.processor(), feature)
                && target.feature(feature).is_none()
            {
                return Err(ResolveAmdTargetV2Error::MissingDetectedFeature {
                    device_index,
                    feature,
                });
            }
        }

        if let Some((first_device_index, first_target)) = first {
            if target != first_target {
                return Err(ResolveAmdTargetV2Error::AmbiguousDevices {
                    first_device_index,
                    first: first_target,
                    conflicting_device_index: device_index,
                    conflicting: target,
                });
            }
        } else {
            first = Some((device_index, target));
        }
    }

    let (_, target) = first.expect("a nonzero bounded device count visits one target");
    Ok(ResolvedAmdTargetIdentityV2::new(
        target,
        ResolvedAmdTargetSourceV2::Detected,
    ))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BoundedTargetParseError {
    TooLong,
    Invalid(ParseAmdTargetIdError),
}

fn parse_bounded_target(value: &str) -> Result<AmdTargetId, BoundedTargetParseError> {
    if value.len() > MAX_AMD_TARGET_ID_BYTES_V2 {
        return Err(BoundedTargetParseError::TooLong);
    }
    AmdTargetId::parse(value).map_err(BoundedTargetParseError::Invalid)
}

const fn identity_feature(
    target: AmdTargetId,
    feature: AmdTargetFeature,
) -> DetectedTargetFeatureV2 {
    if !processor_supports_feature(target.processor(), feature) {
        return DetectedTargetFeatureV2::Unsupported;
    }
    match target.feature(feature) {
        None => DetectedTargetFeatureV2::Unspecified,
        Some(FeatureState::Disabled) => DetectedTargetFeatureV2::Disabled,
        Some(FeatureState::Enabled) => DetectedTargetFeatureV2::Enabled,
    }
}

fn decode_feature_state(
    target: AmdTargetId,
    feature: AmdTargetFeature,
    value: &str,
) -> Result<Option<FeatureState>, DecodeResolvedAmdTargetV2Error> {
    let supported = processor_supports_feature(target.processor(), feature);
    match (supported, value) {
        (false, "unsupported") | (true, "unspecified") => Ok(None),
        (true, "disabled") => Ok(Some(FeatureState::Disabled)),
        (true, "enabled") => Ok(Some(FeatureState::Enabled)),
        _ => Err(DecodeResolvedAmdTargetV2Error::InvalidFeatureState(feature)),
    }
}

const fn is_missing_detected_feature(
    target: AmdTargetId,
    feature: AmdTargetFeature,
    state: Option<FeatureState>,
) -> bool {
    processor_supports_feature(target.processor(), feature) && state.is_none()
}

// FIPS 180-4 SHA-256 over this module's bounded canonical representation.
fn sha256(input: &[u8]) -> [u8; 32] {
    debug_assert!(input.len() <= MAX_RESOLVED_AMD_TARGET_CANONICAL_BYTES_V2);
    const INITIAL: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];

    let mut padded = [0_u8; 192];
    padded[..input.len()].copy_from_slice(input);
    padded[input.len()] = 0x80;
    let total = (input.len() + 9).div_ceil(64) * 64;
    let bit_len = (input.len() as u64) * 8;
    padded[total - 8..total].copy_from_slice(&bit_len.to_be_bytes());

    let mut state = INITIAL;
    for block in padded[..total].chunks_exact(64) {
        let mut words = [0_u32; 64];
        for (index, word) in words[..16].iter_mut().enumerate() {
            let offset = index * 4;
            *word = u32::from_be_bytes([
                block[offset],
                block[offset + 1],
                block[offset + 2],
                block[offset + 3],
            ]);
        }
        for index in 16..64 {
            let s0 = words[index - 15].rotate_right(7)
                ^ words[index - 15].rotate_right(18)
                ^ (words[index - 15] >> 3);
            let s1 = words[index - 2].rotate_right(17)
                ^ words[index - 2].rotate_right(19)
                ^ (words[index - 2] >> 10);
            words[index] = words[index - 16]
                .wrapping_add(s0)
                .wrapping_add(words[index - 7])
                .wrapping_add(s1);
        }

        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = state;
        for index in 0..64 {
            let sum1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choice = (e & f) ^ ((!e) & g);
            let temporary1 = h
                .wrapping_add(sum1)
                .wrapping_add(choice)
                .wrapping_add(K[index])
                .wrapping_add(words[index]);
            let sum0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let temporary2 = sum0.wrapping_add(majority);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temporary1);
            d = c;
            c = b;
            b = a;
            a = temporary1.wrapping_add(temporary2);
        }
        for (slot, value) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
            *slot = slot.wrapping_add(value);
        }
    }

    let mut output = [0_u8; 32];
    for (chunk, word) in output.chunks_exact_mut(4).zip(state) {
        chunk.copy_from_slice(&word.to_be_bytes());
    }
    output
}
