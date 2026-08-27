#![no_std]
#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

use core::{fmt, str::FromStr};

mod advanced_model;
mod atomic_legalizability;
mod capabilities;
mod feature_capabilities;
mod resolved_target_v2;

pub use advanced_model::{
    ADVANCED_CAPABILITY_MODEL_REVISION, AdvancedCapabilityModelIdentity,
    AdvancedCapabilityModelRevision, AdvancedCapabilityStatus,
};
pub use atomic_legalizability::{
    AtomicAddressSpace, AtomicLegalizability, AtomicOperation, StandardAtomicQuery,
};
pub use capabilities::{
    AmdTargetCapabilities, AsyncCopyInstructionSet, AsyncCopyInstructionSets, AtomicScope,
    AtomicScopes, CapabilityDerivationError, CapabilitySupport, MatrixInstructionSet,
    MatrixInstructionSets, WavefrontWidth, WavefrontWidths,
};
pub use feature_capabilities::{
    AtomicOrdering, AtomicOrderings, AtomicWidth, AtomicWidths, DeviceDiagnosticFeature, Fp8Format,
    Fp8Formats, LaunchBoundsField, LaunchBoundsMetadata, LdsTransposeInstruction,
    LdsTransposeInstructions, MfmaFamilies, MfmaFamily, MxFormat, MxFormats, WorkgroupAxis,
    WorkgroupLimits,
};
pub use resolved_target_v2::{
    AmdTargetDetectionV2, CanonicalResolvedAmdTargetBytesV2, DecodeResolvedAmdTargetV2Error,
    DetectedTargetFeatureV2, MAX_AMD_TARGET_ID_BYTES_V2, MAX_DETECTED_AMD_DEVICES_V2,
    MAX_RESOLVED_AMD_TARGET_CANONICAL_BYTES_V2, ResolveAmdTargetV2Error, ResolvedAmdTargetDigestV2,
    ResolvedAmdTargetIdentityV2, ResolvedAmdTargetSourceV2, resolve_amd_target_v2,
};

/// Exact Cargo/rustc target triple admitted by the first gfx942 production profile.
pub const PRODUCTION_GFX942_RUSTC_TARGET_V1: &str = "amdgcn-amd-amdhsa";
/// Exact configured processor admitted by the first gfx942 production profile.
pub const PRODUCTION_GFX942_DEVICE_CPU_V1: &str = "gfx942";
/// Exact code-object target ID emitted by the first gfx942 production profile.
pub const PRODUCTION_GFX942_DEVICE_TARGET_V1: &str = "gfx942:xnack-";
/// Canonical active rustc feature set required by the first gfx942 production profile.
pub const PRODUCTION_GFX942_RUSTC_FEATURES_V1: &str = "-wavefrontsize32,+wavefrontsize64,-xnack";
/// Cargo's target-scoped rustflags channel for the exact production rustc target.
pub const PRODUCTION_GFX942_CARGO_RUSTFLAGS_ENV_V1: &str =
    "CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS";
/// Parent-owned rustc arguments for production target crates.
pub const PRODUCTION_GFX942_CARGO_RUSTFLAGS_V1: &str = "-Zalways-encode-mir -Zinline-mir=yes -Zinline-mir-hint-threshold=1000 -Copt-level=2 -Ctarget-cpu=gfx942 -Ctarget-feature=-wavefrontsize32,+wavefrontsize64,-xnack";

/// Exact Cargo/rustc target triple admitted by the first gfx950 production profile.
pub const PRODUCTION_GFX950_RUSTC_TARGET_V1: &str = "amdgcn-amd-amdhsa";
/// Exact configured processor admitted by the first gfx950 production profile.
pub const PRODUCTION_GFX950_DEVICE_CPU_V1: &str = "gfx950";
/// Exact code-object target ID emitted by the first gfx950 production profile.
pub const PRODUCTION_GFX950_DEVICE_TARGET_V1: &str = "gfx950:xnack-";
/// Canonical active rustc feature set required by the first gfx950 production profile.
pub const PRODUCTION_GFX950_RUSTC_FEATURES_V1: &str = "-wavefrontsize32,+wavefrontsize64,-xnack";
/// Cargo's target-scoped rustflags channel for the exact gfx950 rustc target.
pub const PRODUCTION_GFX950_CARGO_RUSTFLAGS_ENV_V1: &str =
    "CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS";
/// Parent-owned rustc arguments for production gfx950 target crates.
pub const PRODUCTION_GFX950_CARGO_RUSTFLAGS_V1: &str = "-Zalways-encode-mir -Ctarget-cpu=gfx950 -Ctarget-feature=-wavefrontsize32,+wavefrontsize64,-xnack";

/// One exact processor profile admitted by the production AMDGPU transaction.
///
/// This selects target facts inside the single production compiler route. It
/// is not a compiler-pipeline selector.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProductionAmdTargetProfileV1 {
    Gfx942,
    Gfx950,
}

impl ProductionAmdTargetProfileV1 {
    /// Resolves an exact configured processor spelling.
    pub const fn from_cpu(cpu: &str) -> Option<Self> {
        match cpu.as_bytes() {
            b"gfx942" => Some(Self::Gfx942),
            b"gfx950" => Some(Self::Gfx950),
            _ => None,
        }
    }

    /// Resolves one exact canonical AMDHSA production target ID.
    pub const fn from_device_target(target: &str) -> Option<Self> {
        match target.as_bytes() {
            b"gfx942:xnack-" => Some(Self::Gfx942),
            b"gfx950:xnack-" => Some(Self::Gfx950),
            _ => None,
        }
    }

    /// Exact Cargo/rustc target triple.
    pub const fn rustc_target(self) -> &'static str {
        match self {
            Self::Gfx942 => PRODUCTION_GFX942_RUSTC_TARGET_V1,
            Self::Gfx950 => PRODUCTION_GFX950_RUSTC_TARGET_V1,
        }
    }

    /// Exact LLVM target CPU.
    pub const fn cpu(self) -> &'static str {
        match self {
            Self::Gfx942 => PRODUCTION_GFX942_DEVICE_CPU_V1,
            Self::Gfx950 => PRODUCTION_GFX950_DEVICE_CPU_V1,
        }
    }

    /// Exact AMDHSA target ID.
    pub const fn device_target(self) -> &'static str {
        match self {
            Self::Gfx942 => PRODUCTION_GFX942_DEVICE_TARGET_V1,
            Self::Gfx950 => PRODUCTION_GFX950_DEVICE_TARGET_V1,
        }
    }

    /// Canonical rustc target features.
    pub const fn rustc_features(self) -> &'static str {
        match self {
            Self::Gfx942 => PRODUCTION_GFX942_RUSTC_FEATURES_V1,
            Self::Gfx950 => PRODUCTION_GFX950_RUSTC_FEATURES_V1,
        }
    }

    /// Target-scoped Cargo rustflags environment variable.
    pub const fn cargo_rustflags_env(self) -> &'static str {
        match self {
            Self::Gfx942 => PRODUCTION_GFX942_CARGO_RUSTFLAGS_ENV_V1,
            Self::Gfx950 => PRODUCTION_GFX950_CARGO_RUSTFLAGS_ENV_V1,
        }
    }

    /// Parent-owned rustc arguments for target crates.
    pub const fn cargo_rustflags(self) -> &'static str {
        match self {
            Self::Gfx942 => PRODUCTION_GFX942_CARGO_RUSTFLAGS_V1,
            Self::Gfx950 => PRODUCTION_GFX950_CARGO_RUSTFLAGS_V1,
        }
    }
}

/// Concrete canonical AMDGPU processor names understood by this crate.
///
/// Membership establishes only that a spelling is recognized. It does not
/// establish device presence, backend support, or feature availability.
pub const KNOWN_PROCESSORS: &[&str] = &[
    "gfx600", "gfx601", "gfx602", "gfx700", "gfx701", "gfx702", "gfx703", "gfx704", "gfx705",
    "gfx801", "gfx802", "gfx803", "gfx805", "gfx810", "gfx900", "gfx902", "gfx904", "gfx906",
    "gfx908", "gfx909", "gfx90a", "gfx90c", "gfx942", "gfx950", "gfx1010", "gfx1011", "gfx1012",
    "gfx1013", "gfx1030", "gfx1031", "gfx1032", "gfx1033", "gfx1034", "gfx1035", "gfx1036",
    "gfx1100", "gfx1101", "gfx1102", "gfx1103", "gfx1150", "gfx1151", "gfx1152", "gfx1153",
    "gfx1154", "gfx1170", "gfx1171", "gfx1172", "gfx1200", "gfx1201", "gfx1250", "gfx1251",
    "gfx1310",
];

/// A target-ID feature whose state can constrain code-object compatibility.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AmdTargetFeature {
    /// Error-correcting code for scratchpad memory.
    SramEcc,
    /// Recoverable page faults and associated memory behavior.
    Xnack,
}

impl fmt::Display for AmdTargetFeature {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::SramEcc => "sramecc",
            Self::Xnack => "xnack",
        })
    }
}

/// An explicitly declared AMD target-ID feature state.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum FeatureState {
    Disabled,
    Enabled,
}

impl FeatureState {
    const fn suffix(self) -> char {
        match self {
            Self::Disabled => '-',
            Self::Enabled => '+',
        }
    }
}

/// A canonical concrete AMDGPU processor target with optional feature states.
///
/// This value contains parsed declaration data. It is not proof of an observed
/// device or of the target encoded in executable bytes.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AmdTargetId {
    processor: &'static str,
    sramecc: Option<FeatureState>,
    xnack: Option<FeatureState>,
}

impl AmdTargetId {
    /// Parses one canonical concrete processor and its optional feature states.
    pub fn parse(value: &str) -> Result<Self, ParseAmdTargetIdError> {
        value.parse()
    }

    /// Returns the canonical concrete processor name.
    pub const fn processor(&self) -> &'static str {
        self.processor
    }

    /// Returns the explicit SRAM ECC state, or `None` when it was omitted.
    pub const fn sramecc(&self) -> Option<FeatureState> {
        self.sramecc
    }

    /// Returns the explicit XNACK state, or `None` when it was omitted.
    pub const fn xnack(&self) -> Option<FeatureState> {
        self.xnack
    }

    /// Returns the explicit state of `feature`, or `None` when it was omitted.
    pub const fn feature(&self, feature: AmdTargetFeature) -> Option<FeatureState> {
        match feature {
            AmdTargetFeature::SramEcc => self.sramecc,
            AmdTargetFeature::Xnack => self.xnack,
        }
    }

    /// Derives canonical ISA/codegen capabilities for this target.
    pub fn capabilities(&self) -> Result<AmdTargetCapabilities, CapabilityDerivationError> {
        AmdTargetCapabilities::derive(*self)
    }

    /// Returns the exact AMDHSA code-object V4+ ELF flags for this target.
    ///
    /// This encoding is for concrete targets only. Unsupported features use
    /// the `UNSUPPORTED` encoding, omitted supported features use `ANY`, and
    /// explicit states use `OFF` or `ON`. Generic-version and reserved bits
    /// are always zero.
    pub const fn amdhsa_elf_flags_v4_plus(&self) -> u32 {
        let machine = elf_machine(self.processor);
        let xnack = encode_elf_feature(
            processor_supports_feature(self.processor, AmdTargetFeature::Xnack),
            self.xnack,
            0x100,
            0x200,
            0x300,
        );
        let sramecc = encode_elf_feature(
            processor_supports_feature(self.processor, AmdTargetFeature::SramEcc),
            self.sramecc,
            0x400,
            0x800,
            0xc00,
        );
        machine | xnack | sramecc
    }

    /// Compares an artifact declaration with an observed device declaration.
    ///
    /// `self` is the artifact side. An omitted artifact feature is compatible
    /// with either observed state. An explicit artifact state must equal an
    /// explicit observed state. This comparison does not attest either input,
    /// inspect executable bytes, or validate feature support for the processor.
    pub fn is_compatible_with_observed(&self, observed: &Self) -> bool {
        self.processor == observed.processor
            && feature_is_compatible(self.sramecc, observed.sramecc)
            && feature_is_compatible(self.xnack, observed.xnack)
    }
}

impl FromStr for AmdTargetId {
    type Err = ParseAmdTargetIdError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.is_empty() {
            return Err(ParseAmdTargetIdError::Empty);
        }
        if !value.is_ascii() {
            return Err(ParseAmdTargetIdError::NonAscii);
        }
        if value.bytes().any(|byte| byte.is_ascii_uppercase()) {
            return Err(ParseAmdTargetIdError::NonCanonicalCase);
        }

        let mut components = value.split(':');
        let processor_text = components.next().ok_or(ParseAmdTargetIdError::Empty)?;
        let processor = parse_processor(processor_text)?;
        let mut target = Self {
            processor,
            sramecc: None,
            xnack: None,
        };

        for component in components {
            let (feature, state) = parse_feature(component)?;
            if !processor_supports_feature(processor, feature) {
                return Err(ParseAmdTargetIdError::UnsupportedFeature(feature));
            }
            let slot = match feature {
                AmdTargetFeature::SramEcc => &mut target.sramecc,
                AmdTargetFeature::Xnack => &mut target.xnack,
            };
            if slot.replace(state).is_some() {
                return Err(ParseAmdTargetIdError::DuplicateFeature(feature));
            }
        }

        Ok(target)
    }
}

impl fmt::Display for AmdTargetId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.processor)?;
        write_feature(formatter, AmdTargetFeature::SramEcc, self.sramecc)?;
        write_feature(formatter, AmdTargetFeature::Xnack, self.xnack)
    }
}

/// Why an AMD target-ID string was rejected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParseAmdTargetIdError {
    /// The input was empty.
    Empty,
    /// The input contained a non-ASCII code point.
    NonAscii,
    /// The input contained an uppercase ASCII letter.
    NonCanonicalCase,
    /// The processor selected a generic architecture family.
    GenericProcessor,
    /// The processor was unknown, an alias, or otherwise noncanonical.
    UnknownProcessor,
    /// A colon was not followed by a feature modifier.
    EmptyFeature,
    /// A known feature omitted its required `+` or `-` state.
    MissingFeatureState(AmdTargetFeature),
    /// A known feature used malformed state syntax.
    InvalidFeature(AmdTargetFeature),
    /// The feature name was not recognized.
    UnknownFeature,
    /// The concrete processor does not permit this target-ID modifier.
    UnsupportedFeature(AmdTargetFeature),
    /// A feature occurred more than once, including with conflicting states.
    DuplicateFeature(AmdTargetFeature),
}

impl fmt::Display for ParseAmdTargetIdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("AMD target ID must not be empty"),
            Self::NonAscii => formatter.write_str("AMD target ID must contain only ASCII"),
            Self::NonCanonicalCase => {
                formatter.write_str("AMD target ID must use canonical lowercase spelling")
            }
            Self::GenericProcessor => {
                formatter.write_str("generic AMDGPU processors are not accepted")
            }
            Self::UnknownProcessor => {
                formatter.write_str("unknown or noncanonical AMDGPU processor")
            }
            Self::EmptyFeature => formatter.write_str("AMD target feature must not be empty"),
            Self::MissingFeatureState(feature) => {
                write!(formatter, "AMD target feature {feature} requires + or -")
            }
            Self::InvalidFeature(feature) => {
                write!(formatter, "invalid state for AMD target feature {feature}")
            }
            Self::UnknownFeature => formatter.write_str("unknown AMD target feature"),
            Self::UnsupportedFeature(feature) => {
                write!(
                    formatter,
                    "AMD target feature {feature} is unsupported by this processor"
                )
            }
            Self::DuplicateFeature(feature) => {
                write!(formatter, "duplicate AMD target feature {feature}")
            }
        }
    }
}

impl core::error::Error for ParseAmdTargetIdError {}

fn parse_processor(value: &str) -> Result<&'static str, ParseAmdTargetIdError> {
    if value.starts_with("gfx") && value.ends_with("-generic") {
        return Err(ParseAmdTargetIdError::GenericProcessor);
    }
    KNOWN_PROCESSORS
        .iter()
        .copied()
        .find(|known| *known == value)
        .ok_or(ParseAmdTargetIdError::UnknownProcessor)
}

fn parse_feature(value: &str) -> Result<(AmdTargetFeature, FeatureState), ParseAmdTargetIdError> {
    match value {
        "sramecc-" => Ok((AmdTargetFeature::SramEcc, FeatureState::Disabled)),
        "sramecc+" => Ok((AmdTargetFeature::SramEcc, FeatureState::Enabled)),
        "xnack-" => Ok((AmdTargetFeature::Xnack, FeatureState::Disabled)),
        "xnack+" => Ok((AmdTargetFeature::Xnack, FeatureState::Enabled)),
        "" => Err(ParseAmdTargetIdError::EmptyFeature),
        "sramecc" => Err(ParseAmdTargetIdError::MissingFeatureState(
            AmdTargetFeature::SramEcc,
        )),
        "xnack" => Err(ParseAmdTargetIdError::MissingFeatureState(
            AmdTargetFeature::Xnack,
        )),
        invalid if invalid.starts_with("sramecc") => Err(ParseAmdTargetIdError::InvalidFeature(
            AmdTargetFeature::SramEcc,
        )),
        invalid if invalid.starts_with("xnack") => Err(ParseAmdTargetIdError::InvalidFeature(
            AmdTargetFeature::Xnack,
        )),
        _ => Err(ParseAmdTargetIdError::UnknownFeature),
    }
}

// Pinned to llvm/include/llvm/BinaryFormat/ELF.h at LLVM revision
// 846473237377990d00b9c353f6a2c86116b52ea5.
const fn elf_machine(processor: &str) -> u32 {
    match processor.as_bytes() {
        b"gfx600" => 0x20,
        b"gfx601" => 0x21,
        b"gfx602" => 0x3a,
        b"gfx700" => 0x22,
        b"gfx701" => 0x23,
        b"gfx702" => 0x24,
        b"gfx703" => 0x25,
        b"gfx704" => 0x26,
        b"gfx705" => 0x3b,
        b"gfx801" => 0x28,
        b"gfx802" => 0x29,
        b"gfx803" => 0x2a,
        b"gfx805" => 0x3c,
        b"gfx810" => 0x2b,
        b"gfx900" => 0x2c,
        b"gfx902" => 0x2d,
        b"gfx904" => 0x2e,
        b"gfx906" => 0x2f,
        b"gfx908" => 0x30,
        b"gfx909" => 0x31,
        b"gfx90a" => 0x3f,
        b"gfx90c" => 0x32,
        b"gfx942" => 0x4c,
        b"gfx950" => 0x4f,
        b"gfx1010" => 0x33,
        b"gfx1011" => 0x34,
        b"gfx1012" => 0x35,
        b"gfx1013" => 0x42,
        b"gfx1030" => 0x36,
        b"gfx1031" => 0x37,
        b"gfx1032" => 0x38,
        b"gfx1033" => 0x39,
        b"gfx1034" => 0x3e,
        b"gfx1035" => 0x3d,
        b"gfx1036" => 0x45,
        b"gfx1100" => 0x41,
        b"gfx1101" => 0x46,
        b"gfx1102" => 0x47,
        b"gfx1103" => 0x44,
        b"gfx1150" => 0x43,
        b"gfx1151" => 0x4a,
        b"gfx1152" => 0x55,
        b"gfx1153" => 0x58,
        b"gfx1154" => 0x57,
        b"gfx1170" => 0x5d,
        b"gfx1171" => 0x5e,
        b"gfx1172" => 0x5c,
        b"gfx1200" => 0x48,
        b"gfx1201" => 0x4e,
        b"gfx1250" => 0x49,
        b"gfx1251" => 0x5a,
        b"gfx1310" => 0x50,
        _ => 0,
    }
}

const fn encode_elf_feature(
    supported: bool,
    state: Option<FeatureState>,
    any: u32,
    off: u32,
    on: u32,
) -> u32 {
    if !supported {
        return 0;
    }
    match state {
        None => any,
        Some(FeatureState::Disabled) => off,
        Some(FeatureState::Enabled) => on,
    }
}

const fn processor_supports_feature(processor: &str, feature: AmdTargetFeature) -> bool {
    match feature {
        AmdTargetFeature::SramEcc => matches!(
            processor.as_bytes(),
            b"gfx906" | b"gfx908" | b"gfx90a" | b"gfx942" | b"gfx950" | b"gfx1250" | b"gfx1251"
        ),
        AmdTargetFeature::Xnack => matches!(
            processor.as_bytes(),
            b"gfx801"
                | b"gfx810"
                | b"gfx900"
                | b"gfx902"
                | b"gfx904"
                | b"gfx906"
                | b"gfx908"
                | b"gfx909"
                | b"gfx90a"
                | b"gfx90c"
                | b"gfx942"
                | b"gfx950"
                | b"gfx1010"
                | b"gfx1011"
                | b"gfx1012"
                | b"gfx1013"
        ),
    }
}

const fn feature_is_compatible(
    artifact: Option<FeatureState>,
    observed: Option<FeatureState>,
) -> bool {
    match artifact {
        None => true,
        Some(required) => match observed {
            Some(actual) => matches!(
                (required, actual),
                (FeatureState::Disabled, FeatureState::Disabled)
                    | (FeatureState::Enabled, FeatureState::Enabled)
            ),
            None => false,
        },
    }
}

fn write_feature(
    formatter: &mut fmt::Formatter<'_>,
    feature: AmdTargetFeature,
    state: Option<FeatureState>,
) -> fmt::Result {
    if let Some(state) = state {
        write!(formatter, ":{feature}{}", state.suffix())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    extern crate std;

    use core::str::FromStr;
    use std::string::ToString;

    use super::*;

    #[test]
    fn every_known_processor_round_trips() {
        for &processor in KNOWN_PROCESSORS {
            let target = AmdTargetId::parse(processor).unwrap();
            assert_eq!(target.processor(), processor);
            assert_eq!(target.sramecc(), None);
            assert_eq!(target.xnack(), None);
            assert_eq!(target.to_string(), processor);
        }
    }

    #[test]
    fn parses_every_feature_state_combination() {
        let states = [
            None,
            Some(FeatureState::Disabled),
            Some(FeatureState::Enabled),
        ];
        for sramecc in states {
            for xnack in states {
                let mut text = "gfx942".to_string();
                append_feature(&mut text, "sramecc", sramecc);
                append_feature(&mut text, "xnack", xnack);
                let target = AmdTargetId::parse(&text).unwrap();
                assert_eq!(target.sramecc(), sramecc);
                assert_eq!(target.xnack(), xnack);
                assert_eq!(target.to_string(), text);
            }
        }
    }

    #[test]
    fn exact_gfx942_xnack_minus_identity_has_no_spelling_or_feature_alias() {
        let exact = AmdTargetId::parse("gfx942:xnack-").unwrap();
        assert_eq!(exact.to_string(), "gfx942:xnack-");
        assert_eq!(exact.processor(), "gfx942");
        assert_eq!(exact.sramecc(), None);
        assert_eq!(exact.xnack(), Some(FeatureState::Disabled));

        for different in [
            "gfx942",
            "gfx942:xnack+",
            "gfx942:sramecc+:xnack-",
            "gfx942:sramecc-:xnack-",
            "gfx942:xnack-:sramecc+",
        ] {
            let parsed = AmdTargetId::parse(different).unwrap();
            assert_ne!(parsed, exact, "unexpected exact-target alias {different}");
        }

        for invalid in [
            "gfx942:xnack-:xnack-",
            "gfx942:xnack-:xnack+",
            "gfx942:future+",
            "GFX942:xnack-",
            "gfx941",
        ] {
            assert!(
                AmdTargetId::parse(invalid).is_err(),
                "unexpectedly parsed invalid target {invalid}"
            );
        }
    }

    #[test]
    fn production_gfx942_profile_is_internally_consistent() {
        let target = AmdTargetId::parse(PRODUCTION_GFX942_DEVICE_TARGET_V1).unwrap();
        assert_eq!(target.processor(), PRODUCTION_GFX942_DEVICE_CPU_V1);
        assert_eq!(target.xnack(), Some(FeatureState::Disabled));
        assert!(PRODUCTION_GFX942_CARGO_RUSTFLAGS_V1.contains(&std::format!(
            "-Ctarget-cpu={PRODUCTION_GFX942_DEVICE_CPU_V1}"
        )));
        assert!(PRODUCTION_GFX942_CARGO_RUSTFLAGS_V1.contains(&std::format!(
            "-Ctarget-feature={PRODUCTION_GFX942_RUSTC_FEATURES_V1}"
        )));
        assert!(PRODUCTION_GFX942_CARGO_RUSTFLAGS_V1.contains("-Zinline-mir=yes"));
        assert!(PRODUCTION_GFX942_CARGO_RUSTFLAGS_V1.contains("-Copt-level=2"));
        assert_eq!(
            PRODUCTION_GFX942_CARGO_RUSTFLAGS_ENV_V1,
            "CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS"
        );
    }

    #[test]
    fn production_gfx950_profile_is_internally_consistent() {
        let target = AmdTargetId::parse(PRODUCTION_GFX950_DEVICE_TARGET_V1).unwrap();
        assert_eq!(target.to_string(), "gfx950:xnack-");
        assert_eq!(target.processor(), PRODUCTION_GFX950_DEVICE_CPU_V1);
        assert_eq!(target.sramecc(), None);
        assert_eq!(target.xnack(), Some(FeatureState::Disabled));
        assert_eq!(target.amdhsa_elf_flags_v4_plus(), 0x64f);
        assert!(PRODUCTION_GFX950_CARGO_RUSTFLAGS_V1.contains(&std::format!(
            "-Ctarget-cpu={PRODUCTION_GFX950_DEVICE_CPU_V1}"
        )));
        assert!(PRODUCTION_GFX950_CARGO_RUSTFLAGS_V1.contains(&std::format!(
            "-Ctarget-feature={PRODUCTION_GFX950_RUSTC_FEATURES_V1}"
        )));
        assert_eq!(
            PRODUCTION_GFX950_CARGO_RUSTFLAGS_ENV_V1,
            "CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS"
        );
        assert_eq!(
            PRODUCTION_GFX950_RUSTC_TARGET_V1,
            PRODUCTION_GFX942_RUSTC_TARGET_V1
        );
    }

    #[test]
    fn production_profiles_resolve_only_exact_cpu_spellings() {
        for (cpu, profile, target) in [
            (
                "gfx942",
                ProductionAmdTargetProfileV1::Gfx942,
                "gfx942:xnack-",
            ),
            (
                "gfx950",
                ProductionAmdTargetProfileV1::Gfx950,
                "gfx950:xnack-",
            ),
        ] {
            assert_eq!(ProductionAmdTargetProfileV1::from_cpu(cpu), Some(profile));
            assert_eq!(profile.cpu(), cpu);
            assert_eq!(profile.device_target(), target);
            assert_eq!(profile.rustc_target(), "amdgcn-amd-amdhsa");
            assert_eq!(
                profile.rustc_features(),
                "-wavefrontsize32,+wavefrontsize64,-xnack"
            );
        }
        for rejected in ["gfx942:xnack-", "gfx950:xnack-", "GFX950", "gfx951"] {
            assert_eq!(ProductionAmdTargetProfileV1::from_cpu(rejected), None);
        }
        for (target, profile) in [
            ("gfx942:xnack-", ProductionAmdTargetProfileV1::Gfx942),
            ("gfx950:xnack-", ProductionAmdTargetProfileV1::Gfx950),
        ] {
            assert_eq!(
                ProductionAmdTargetProfileV1::from_device_target(target),
                Some(profile)
            );
        }
        for rejected in ["gfx942", "gfx950", "gfx950:xnack+", "GFX950:xnack-"] {
            assert_eq!(
                ProductionAmdTargetProfileV1::from_device_target(rejected),
                None
            );
        }
    }

    #[test]
    fn elf_machine_mapping_matches_independent_llvm_literals() {
        const EXPECTED: &[(&str, u32)] = &[
            ("gfx600", 0x20),
            ("gfx601", 0x21),
            ("gfx602", 0x3a),
            ("gfx700", 0x22),
            ("gfx701", 0x23),
            ("gfx702", 0x24),
            ("gfx703", 0x25),
            ("gfx704", 0x26),
            ("gfx705", 0x3b),
            ("gfx801", 0x28),
            ("gfx802", 0x29),
            ("gfx803", 0x2a),
            ("gfx805", 0x3c),
            ("gfx810", 0x2b),
            ("gfx900", 0x2c),
            ("gfx902", 0x2d),
            ("gfx904", 0x2e),
            ("gfx906", 0x2f),
            ("gfx908", 0x30),
            ("gfx909", 0x31),
            ("gfx90a", 0x3f),
            ("gfx90c", 0x32),
            ("gfx942", 0x4c),
            ("gfx950", 0x4f),
            ("gfx1010", 0x33),
            ("gfx1011", 0x34),
            ("gfx1012", 0x35),
            ("gfx1013", 0x42),
            ("gfx1030", 0x36),
            ("gfx1031", 0x37),
            ("gfx1032", 0x38),
            ("gfx1033", 0x39),
            ("gfx1034", 0x3e),
            ("gfx1035", 0x3d),
            ("gfx1036", 0x45),
            ("gfx1100", 0x41),
            ("gfx1101", 0x46),
            ("gfx1102", 0x47),
            ("gfx1103", 0x44),
            ("gfx1150", 0x43),
            ("gfx1151", 0x4a),
            ("gfx1152", 0x55),
            ("gfx1153", 0x58),
            ("gfx1154", 0x57),
            ("gfx1170", 0x5d),
            ("gfx1171", 0x5e),
            ("gfx1172", 0x5c),
            ("gfx1200", 0x48),
            ("gfx1201", 0x4e),
            ("gfx1250", 0x49),
            ("gfx1251", 0x5a),
            ("gfx1310", 0x50),
        ];

        assert_eq!(EXPECTED.len(), KNOWN_PROCESSORS.len());
        for &(processor, machine) in EXPECTED {
            let flags = AmdTargetId::parse(processor)
                .unwrap()
                .amdhsa_elf_flags_v4_plus();
            assert_eq!(flags & 0xff, machine, "{processor}");
            assert_eq!(flags & 0xff00_0000, 0, "{processor}");
        }
    }

    #[test]
    fn elf_feature_flags_use_unsupported_any_off_and_on_encodings() {
        assert_eq!(
            AmdTargetId::parse("gfx1151")
                .unwrap()
                .amdhsa_elf_flags_v4_plus(),
            0x4a
        );
        assert_eq!(
            AmdTargetId::parse("gfx1010")
                .unwrap()
                .amdhsa_elf_flags_v4_plus(),
            0x133
        );
        assert_eq!(
            AmdTargetId::parse("gfx1250")
                .unwrap()
                .amdhsa_elf_flags_v4_plus(),
            0x449
        );

        for (target, expected) in [
            ("gfx942", 0x54c),
            ("gfx942:xnack-", 0x64c),
            ("gfx942:xnack+", 0x74c),
            ("gfx942:sramecc-", 0x94c),
            ("gfx942:sramecc-:xnack-", 0xa4c),
            ("gfx942:sramecc-:xnack+", 0xb4c),
            ("gfx942:sramecc+", 0xd4c),
            ("gfx942:sramecc+:xnack-", 0xe4c),
            ("gfx942:sramecc+:xnack+", 0xf4c),
        ] {
            assert_eq!(
                AmdTargetId::parse(target)
                    .unwrap()
                    .amdhsa_elf_flags_v4_plus(),
                expected,
                "{target}"
            );
        }
    }

    #[test]
    fn canonicalizes_feature_order() {
        let target = AmdTargetId::parse("gfx942:xnack-:sramecc+").unwrap();
        assert_eq!(target.to_string(), "gfx942:sramecc+:xnack-");
        assert_eq!(
            target.feature(AmdTargetFeature::SramEcc),
            Some(FeatureState::Enabled)
        );
        assert_eq!(
            target.feature(AmdTargetFeature::Xnack),
            Some(FeatureState::Disabled)
        );
    }

    #[test]
    fn rejects_generic_processors() {
        for processor in [
            "gfx9-generic",
            "gfx9-4-generic",
            "gfx10-1-generic",
            "gfx10-3-generic",
            "gfx11-generic",
            "gfx11-7-generic",
            "gfx12-generic",
            "gfx12-5-generic",
            "gfx13-generic",
        ] {
            assert_eq!(
                AmdTargetId::parse(processor),
                Err(ParseAmdTargetIdError::GenericProcessor)
            );
        }
    }

    #[test]
    fn rejects_empty_non_ascii_and_uppercase_input() {
        assert_eq!(AmdTargetId::parse(""), Err(ParseAmdTargetIdError::Empty));
        assert_eq!(
            AmdTargetId::parse("gfx90\u{e1}"),
            Err(ParseAmdTargetIdError::NonAscii)
        );
        for input in ["GFX90a", "gfx90A", "gfx90a:XNACK+"] {
            assert_eq!(
                AmdTargetId::parse(input),
                Err(ParseAmdTargetIdError::NonCanonicalCase)
            );
        }
    }

    #[test]
    fn rejects_unknown_noncanonical_and_alias_processors() {
        for input in [
            "gfx",
            "gfx90",
            "gfx999",
            "gfx942 ",
            " gfx942",
            "tahiti",
            "polaris10",
            "generic",
            "amdgcn",
        ] {
            assert_eq!(
                AmdTargetId::parse(input),
                Err(ParseAmdTargetIdError::UnknownProcessor),
                "{input}"
            );
        }
    }

    #[test]
    fn rejects_empty_missing_invalid_and_unknown_features() {
        let cases = [
            ("gfx942:", ParseAmdTargetIdError::EmptyFeature),
            ("gfx942::xnack+", ParseAmdTargetIdError::EmptyFeature),
            (
                "gfx942:xnack",
                ParseAmdTargetIdError::MissingFeatureState(AmdTargetFeature::Xnack),
            ),
            (
                "gfx942:sramecc",
                ParseAmdTargetIdError::MissingFeatureState(AmdTargetFeature::SramEcc),
            ),
            (
                "gfx942:xnack++",
                ParseAmdTargetIdError::InvalidFeature(AmdTargetFeature::Xnack),
            ),
            (
                "gfx942:sramecc=+",
                ParseAmdTargetIdError::InvalidFeature(AmdTargetFeature::SramEcc),
            ),
            (
                "gfx942:wavefrontsize64+",
                ParseAmdTargetIdError::UnknownFeature,
            ),
            ("gfx942:+", ParseAmdTargetIdError::UnknownFeature),
        ];
        for (input, expected) in cases {
            assert_eq!(AmdTargetId::parse(input), Err(expected), "{input}");
        }
    }

    #[test]
    fn feature_modifier_support_matches_the_llvm_processor_matrix() {
        const SRAMECC_PROCESSORS: &[&str] = &[
            "gfx906", "gfx908", "gfx90a", "gfx942", "gfx950", "gfx1250", "gfx1251",
        ];
        const XNACK_PROCESSORS: &[&str] = &[
            "gfx801", "gfx810", "gfx900", "gfx902", "gfx904", "gfx906", "gfx908", "gfx909",
            "gfx90a", "gfx90c", "gfx942", "gfx950", "gfx1010", "gfx1011", "gfx1012", "gfx1013",
        ];

        for &processor in KNOWN_PROCESSORS {
            for feature in [AmdTargetFeature::SramEcc, AmdTargetFeature::Xnack] {
                for state in [FeatureState::Disabled, FeatureState::Enabled] {
                    let input = std::format!("{processor}:{feature}{}", state.suffix());
                    let parsed = AmdTargetId::parse(&input);
                    let expected = match feature {
                        AmdTargetFeature::SramEcc => SRAMECC_PROCESSORS.contains(&processor),
                        AmdTargetFeature::Xnack => XNACK_PROCESSORS.contains(&processor),
                    };
                    if expected {
                        assert!(parsed.is_ok(), "expected supported target ID: {input}");
                    } else {
                        assert_eq!(
                            parsed,
                            Err(ParseAmdTargetIdError::UnsupportedFeature(feature)),
                            "expected unsupported target ID: {input}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn fixed_xnack_processors_reject_xnack_modifiers() {
        for processor in ["gfx1250", "gfx1251"] {
            for suffix in ['-', '+'] {
                assert_eq!(
                    AmdTargetId::parse(&std::format!("{processor}:xnack{suffix}")),
                    Err(ParseAmdTargetIdError::UnsupportedFeature(
                        AmdTargetFeature::Xnack
                    ))
                );
            }
        }
    }

    #[test]
    fn rejects_duplicate_features_even_when_states_conflict() {
        for (input, feature) in [
            ("gfx942:xnack+:xnack+", AmdTargetFeature::Xnack),
            ("gfx942:xnack+:xnack-", AmdTargetFeature::Xnack),
            ("gfx942:sramecc-:sramecc-", AmdTargetFeature::SramEcc),
            ("gfx942:sramecc-:sramecc+", AmdTargetFeature::SramEcc),
        ] {
            assert_eq!(
                AmdTargetId::parse(input),
                Err(ParseAmdTargetIdError::DuplicateFeature(feature)),
                "{input}"
            );
        }
    }

    #[test]
    fn compatibility_is_exhaustive_and_directional() {
        let states = [
            None,
            Some(FeatureState::Disabled),
            Some(FeatureState::Enabled),
        ];
        for artifact_sramecc in states {
            for artifact_xnack in states {
                for observed_sramecc in states {
                    for observed_xnack in states {
                        let artifact = target(artifact_sramecc, artifact_xnack);
                        let observed = target(observed_sramecc, observed_xnack);
                        let expected = compatible(artifact_sramecc, observed_sramecc)
                            && compatible(artifact_xnack, observed_xnack);
                        assert_eq!(
                            artifact.is_compatible_with_observed(&observed),
                            expected,
                            "artifact={artifact}, observed={observed}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn different_processors_are_never_compatible() {
        let artifact = AmdTargetId::parse("gfx942").unwrap();
        let observed = AmdTargetId::parse("gfx950:sramecc+:xnack-").unwrap();
        assert!(!artifact.is_compatible_with_observed(&observed));
    }

    #[test]
    fn implements_from_str_and_error_messages() {
        let parsed = AmdTargetId::from_str("gfx1151").unwrap();
        assert_eq!(parsed.processor(), "gfx1151");

        let errors = [
            ParseAmdTargetIdError::Empty,
            ParseAmdTargetIdError::NonAscii,
            ParseAmdTargetIdError::NonCanonicalCase,
            ParseAmdTargetIdError::GenericProcessor,
            ParseAmdTargetIdError::UnknownProcessor,
            ParseAmdTargetIdError::EmptyFeature,
            ParseAmdTargetIdError::MissingFeatureState(AmdTargetFeature::Xnack),
            ParseAmdTargetIdError::InvalidFeature(AmdTargetFeature::SramEcc),
            ParseAmdTargetIdError::UnknownFeature,
            ParseAmdTargetIdError::UnsupportedFeature(AmdTargetFeature::Xnack),
            ParseAmdTargetIdError::DuplicateFeature(AmdTargetFeature::Xnack),
        ];
        for error in errors {
            assert!(!error.to_string().is_empty());
        }
    }

    fn target(sramecc: Option<FeatureState>, xnack: Option<FeatureState>) -> AmdTargetId {
        AmdTargetId {
            processor: "gfx942",
            sramecc,
            xnack,
        }
    }

    fn compatible(artifact: Option<FeatureState>, observed: Option<FeatureState>) -> bool {
        artifact.is_none() || artifact == observed
    }

    fn append_feature(text: &mut std::string::String, name: &str, state: Option<FeatureState>) {
        if let Some(state) = state {
            use core::fmt::Write;
            write!(text, ":{name}{}", state.suffix()).unwrap();
        }
    }

    #[test]
    fn known_processor_table_has_unique_canonical_entries() {
        let mut sorted = KNOWN_PROCESSORS.to_vec();
        sorted.sort_unstable();
        let original_len = sorted.len();
        sorted.dedup();
        assert_eq!(sorted.len(), original_len);

        for processor in sorted {
            assert!(processor.starts_with("gfx"));
            assert!(processor.is_ascii());
            assert_eq!(processor, processor.to_ascii_lowercase());
            assert!(!processor.contains('-'));
        }
    }

    #[test]
    fn value_is_copy_and_compact() {
        fn assert_copy<T: Copy>() {}
        assert_copy::<AmdTargetId>();
        assert!(core::mem::size_of::<AmdTargetId>() <= 3 * core::mem::size_of::<usize>());
    }
}
