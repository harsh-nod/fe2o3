#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![doc = include_str!("../README.md")]

use core::fmt;

const ABSENT_PROFILE_FIELD: &str = "<absent>";

/// Organization or hardware ecosystem that owns the target profile semantics.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TargetVendorV1 {
    /// AMD GPU targets.
    Amd,
    /// NVIDIA GPU targets.
    Nvidia,
    /// Intel CPU, GPU, or accelerator targets.
    Intel,
    /// Arm CPU or accelerator targets.
    Arm,
    /// The host process target.
    Host,
    /// A reviewed target whose vendor is not represented by a dedicated variant yet.
    Other,
}

impl TargetVendorV1 {
    /// Returns the canonical lowercase spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Amd => "amd",
            Self::Nvidia => "nvidia",
            Self::Intel => "intel",
            Self::Arm => "arm",
            Self::Host => "host",
            Self::Other => "other",
        }
    }
}

impl fmt::Display for TargetVendorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Instruction-set or virtual-ISA family used by a target profile.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TargetArchitectureFamilyV1 {
    /// AMDGCN GPU ISA family.
    Amdgcn,
    /// NVIDIA PTX virtual GPU ISA family.
    Nvptx,
    /// SPIR-V virtual ISA family.
    Spirv,
    /// x86-64 host or device family.
    X86_64,
    /// AArch64 host or device family.
    Aarch64,
    /// WebAssembly family.
    Wasm,
    /// A reviewed target family not represented by a dedicated variant yet.
    Other,
}

impl TargetArchitectureFamilyV1 {
    /// Returns the canonical lowercase spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Amdgcn => "amdgcn",
            Self::Nvptx => "nvptx",
            Self::Spirv => "spirv",
            Self::X86_64 => "x86_64",
            Self::Aarch64 => "aarch64",
            Self::Wasm => "wasm",
            Self::Other => "other",
        }
    }
}

impl fmt::Display for TargetArchitectureFamilyV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Runtime execution model expected by a target profile.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TargetExecutionModelV1 {
    /// SIMT/SIMD grid launch with work-items grouped into workgroups or blocks.
    GpuGrid,
    /// Native host process execution.
    CpuProcess,
    /// Queue-submitted accelerator execution whose work partitioning is backend-defined.
    AcceleratorQueue,
}

impl TargetExecutionModelV1 {
    /// Returns the canonical lowercase spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GpuGrid => "gpu-grid",
            Self::CpuProcess => "cpu-process",
            Self::AcceleratorQueue => "accelerator-queue",
        }
    }
}

impl fmt::Display for TargetExecutionModelV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Artifact format produced for a target profile.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TargetArtifactFormatV1 {
    /// AMDHSA code object.
    AmdHsaCodeObject,
    /// PTX text.
    PtxText,
    /// SPIR-V binary module.
    SpirvBinary,
    /// Native object file.
    NativeObject,
    /// WebAssembly module.
    WasmModule,
    /// Reviewed target whose artifact container is not represented by a dedicated variant yet.
    Unknown,
}

impl TargetArtifactFormatV1 {
    /// Returns the canonical lowercase spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AmdHsaCodeObject => "amd-hsa-code-object",
            Self::PtxText => "ptx-text",
            Self::SpirvBinary => "spirv-binary",
            Self::NativeObject => "native-object",
            Self::WasmModule => "wasm-module",
            Self::Unknown => "unknown",
        }
    }
}

impl fmt::Display for TargetArtifactFormatV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Tri-state feature support plus explicit unsupported state.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TargetFeatureStateV1 {
    /// The feature is not supported by this target profile.
    Unsupported,
    /// The profile intentionally leaves this feature unspecified.
    Unspecified,
    /// The feature is supported but disabled by this profile.
    Disabled,
    /// The feature is supported and enabled by this profile.
    Enabled,
}

impl TargetFeatureStateV1 {
    /// Returns the canonical lowercase spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unsupported => "unsupported",
            Self::Unspecified => "unspecified",
            Self::Disabled => "disabled",
            Self::Enabled => "enabled",
        }
    }
}

impl fmt::Display for TargetFeatureStateV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// One named target feature and its reviewed state.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TargetFeatureSpecV1 {
    name: &'static str,
    state: TargetFeatureStateV1,
}

impl TargetFeatureSpecV1 {
    /// Creates a target feature from static parts.
    ///
    /// Callers should run [`Self::validate`] directly or through
    /// [`TargetProfileSpecV1::validate`] before using the feature as canonical
    /// profile evidence.
    pub const fn new_unchecked(name: &'static str, state: TargetFeatureStateV1) -> Self {
        Self { name, state }
    }

    /// Returns the feature name.
    pub const fn name(self) -> &'static str {
        self.name
    }

    /// Returns the reviewed feature state.
    pub const fn state(self) -> TargetFeatureStateV1 {
        self.state
    }

    /// Checks that the feature name can appear in canonical profile text.
    pub fn validate(self) -> Result<(), TargetProfileValidationErrorV1> {
        validate_feature_name(self.name)
    }
}

impl fmt::Display for TargetFeatureSpecV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}:{}", self.name, self.state)
    }
}

/// Reusable target profile metadata for compiler, proof, and runtime contracts.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TargetProfileSpecV1 {
    vendor: TargetVendorV1,
    architecture_family: TargetArchitectureFamilyV1,
    architecture: &'static str,
    rustc_target: Option<&'static str>,
    llvm_target: Option<&'static str>,
    artifact_format: TargetArtifactFormatV1,
    execution_model: TargetExecutionModelV1,
    data_layout: Option<&'static str>,
    features: &'static [TargetFeatureSpecV1],
}

impl TargetProfileSpecV1 {
    /// Creates a target profile from reviewed static parts.
    ///
    /// Callers should run [`Self::validate`] before using the profile as
    /// canonical compiler input or proof evidence.
    pub const fn from_static_parts(
        vendor: TargetVendorV1,
        architecture_family: TargetArchitectureFamilyV1,
        architecture: &'static str,
        rustc_target: Option<&'static str>,
        llvm_target: Option<&'static str>,
        artifact_format: TargetArtifactFormatV1,
        execution_model: TargetExecutionModelV1,
        data_layout: Option<&'static str>,
        features: &'static [TargetFeatureSpecV1],
    ) -> Self {
        Self {
            vendor,
            architecture_family,
            architecture,
            rustc_target,
            llvm_target,
            artifact_format,
            execution_model,
            data_layout,
            features,
        }
    }

    /// Returns the target vendor.
    pub const fn vendor(self) -> TargetVendorV1 {
        self.vendor
    }

    /// Returns the target architecture family.
    pub const fn architecture_family(self) -> TargetArchitectureFamilyV1 {
        self.architecture_family
    }

    /// Returns the exact architecture spelling.
    pub const fn architecture(self) -> &'static str {
        self.architecture
    }

    /// Returns the rustc target triple, when this profile has one.
    pub const fn rustc_target(self) -> Option<&'static str> {
        self.rustc_target
    }

    /// Returns the LLVM target triple, when this profile has one.
    pub const fn llvm_target(self) -> Option<&'static str> {
        self.llvm_target
    }

    /// Returns the expected target artifact format.
    pub const fn artifact_format(self) -> TargetArtifactFormatV1 {
        self.artifact_format
    }

    /// Returns the runtime execution model.
    pub const fn execution_model(self) -> TargetExecutionModelV1 {
        self.execution_model
    }

    /// Returns the target data layout, when this profile has one.
    pub const fn data_layout(self) -> Option<&'static str> {
        self.data_layout
    }

    /// Returns all reviewed feature states in canonical order.
    pub const fn features(self) -> &'static [TargetFeatureSpecV1] {
        self.features
    }

    /// Finds one feature by its canonical name.
    pub fn feature(self, name: &str) -> Option<TargetFeatureSpecV1> {
        self.features
            .iter()
            .copied()
            .find(|feature| feature.name() == name)
    }

    /// Checks that the profile can be used as canonical target-profile evidence.
    pub fn validate(self) -> Result<(), TargetProfileValidationErrorV1> {
        validate_required_field(TargetProfileFieldV1::Architecture, self.architecture)?;
        validate_optional_field(TargetProfileFieldV1::RustcTarget, self.rustc_target)?;
        validate_optional_field(TargetProfileFieldV1::LlvmTarget, self.llvm_target)?;
        validate_optional_field(TargetProfileFieldV1::DataLayout, self.data_layout)?;

        let mut previous_name = None;
        for feature in self.features {
            feature.validate()?;
            if let Some(previous_name) = previous_name {
                if previous_name >= feature.name() {
                    return Err(TargetProfileValidationErrorV1::UnsortedFeatureName {
                        previous: previous_name,
                        current: feature.name(),
                    });
                }
            }
            previous_name = Some(feature.name());
        }

        Ok(())
    }
}

impl fmt::Display for TargetProfileSpecV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "fe2o3.target-profile.v1;vendor={};architecture-family={};architecture={};rustc-target=",
            self.vendor, self.architecture_family, self.architecture
        )?;
        write_optional(formatter, self.rustc_target)?;
        formatter.write_str(";llvm-target=")?;
        write_optional(formatter, self.llvm_target)?;
        write!(
            formatter,
            ";artifact-format={};execution-model={};data-layout=",
            self.artifact_format, self.execution_model
        )?;
        write_optional(formatter, self.data_layout)?;
        formatter.write_str(";features=")?;

        if self.features.is_empty() {
            return formatter.write_str(ABSENT_PROFILE_FIELD);
        }

        for (index, feature) in self.features.iter().enumerate() {
            if index != 0 {
                formatter.write_str(",")?;
            }
            write!(formatter, "{feature}")?;
        }
        Ok(())
    }
}

/// String field in a target profile.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TargetProfileFieldV1 {
    /// Exact architecture spelling.
    Architecture,
    /// rustc target triple.
    RustcTarget,
    /// LLVM target triple.
    LlvmTarget,
    /// Target data layout.
    DataLayout,
}

impl TargetProfileFieldV1 {
    /// Returns the canonical lowercase spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Architecture => "architecture",
            Self::RustcTarget => "rustc-target",
            Self::LlvmTarget => "llvm-target",
            Self::DataLayout => "data-layout",
        }
    }
}

impl fmt::Display for TargetProfileFieldV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Validation failure for canonical target-profile metadata.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TargetProfileValidationErrorV1 {
    /// A required field was empty.
    EmptyField {
        /// The rejected field.
        field: TargetProfileFieldV1,
    },
    /// A field contained non-ASCII bytes.
    NonAsciiField {
        /// The rejected field.
        field: TargetProfileFieldV1,
    },
    /// A field used a canonical-display sentinel value reserved for absent optional fields.
    ReservedAbsentFieldValue {
        /// The rejected field.
        field: TargetProfileFieldV1,
    },
    /// A field contained a delimiter that would make canonical profile text ambiguous.
    ReservedDelimiterField {
        /// The rejected field.
        field: TargetProfileFieldV1,
    },
    /// A feature name was empty.
    EmptyFeatureName,
    /// A feature name contained non-ASCII bytes.
    NonAsciiFeatureName {
        /// The rejected name.
        name: &'static str,
    },
    /// A feature name was not lowercase ASCII plus digits, period, underscore, or dash.
    NonCanonicalFeatureName {
        /// The rejected name.
        name: &'static str,
    },
    /// Feature names must be strictly sorted and unique for canonical profile text.
    UnsortedFeatureName {
        /// Previous feature name.
        previous: &'static str,
        /// Current feature name.
        current: &'static str,
    },
}

impl fmt::Display for TargetProfileValidationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField { field } => write!(formatter, "empty target profile field `{field}`"),
            Self::NonAsciiField { field } => {
                write!(formatter, "non-ASCII target profile field `{field}`")
            }
            Self::ReservedAbsentFieldValue { field } => write!(
                formatter,
                "target profile field `{field}` uses the reserved absent-field sentinel"
            ),
            Self::ReservedDelimiterField { field } => write!(
                formatter,
                "target profile field `{field}` uses a reserved canonical delimiter"
            ),
            Self::EmptyFeatureName => formatter.write_str("empty target feature name"),
            Self::NonAsciiFeatureName { name } => {
                write!(formatter, "non-ASCII target feature name `{name}`")
            }
            Self::NonCanonicalFeatureName { name } => {
                write!(formatter, "noncanonical target feature name `{name}`")
            }
            Self::UnsortedFeatureName { previous, current } => write!(
                formatter,
                "target feature name `{current}` is not strictly after `{previous}`"
            ),
        }
    }
}

impl core::error::Error for TargetProfileValidationErrorV1 {}

fn validate_required_field(
    field: TargetProfileFieldV1,
    value: &'static str,
) -> Result<(), TargetProfileValidationErrorV1> {
    if value.is_empty() {
        return Err(TargetProfileValidationErrorV1::EmptyField { field });
    }
    validate_present_field(field, value)
}

fn validate_optional_field(
    field: TargetProfileFieldV1,
    value: Option<&'static str>,
) -> Result<(), TargetProfileValidationErrorV1> {
    if let Some(value) = value {
        validate_required_field(field, value)?;
    }
    Ok(())
}

fn validate_present_field(
    field: TargetProfileFieldV1,
    value: &'static str,
) -> Result<(), TargetProfileValidationErrorV1> {
    if !value.is_ascii() {
        return Err(TargetProfileValidationErrorV1::NonAsciiField { field });
    }
    if value == ABSENT_PROFILE_FIELD {
        return Err(TargetProfileValidationErrorV1::ReservedAbsentFieldValue { field });
    }
    if value.bytes().any(is_reserved_field_delimiter) {
        return Err(TargetProfileValidationErrorV1::ReservedDelimiterField { field });
    }
    Ok(())
}

fn is_reserved_field_delimiter(byte: u8) -> bool {
    matches!(byte, b';' | b'\n' | b'\r')
}

fn validate_feature_name(name: &'static str) -> Result<(), TargetProfileValidationErrorV1> {
    if name.is_empty() {
        return Err(TargetProfileValidationErrorV1::EmptyFeatureName);
    }
    if !name.is_ascii() {
        return Err(TargetProfileValidationErrorV1::NonAsciiFeatureName { name });
    }
    if !name.bytes().all(is_canonical_feature_name_byte) {
        return Err(TargetProfileValidationErrorV1::NonCanonicalFeatureName { name });
    }
    Ok(())
}

fn is_canonical_feature_name_byte(byte: u8) -> bool {
    byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
}

fn write_optional(formatter: &mut fmt::Formatter<'_>, value: Option<&str>) -> fmt::Result {
    formatter.write_str(value.unwrap_or(ABSENT_PROFILE_FIELD))
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use std::string::ToString;

    const AMD_FEATURES: &[TargetFeatureSpecV1] = &[
        TargetFeatureSpecV1::new_unchecked("wavefrontsize32", TargetFeatureStateV1::Disabled),
        TargetFeatureSpecV1::new_unchecked("wavefrontsize64", TargetFeatureStateV1::Enabled),
        TargetFeatureSpecV1::new_unchecked("xnack", TargetFeatureStateV1::Disabled),
    ];
    const PTX_FEATURES: &[TargetFeatureSpecV1] = &[TargetFeatureSpecV1::new_unchecked(
        "ptx80",
        TargetFeatureStateV1::Enabled,
    )];
    const UNSORTED_FEATURES: &[TargetFeatureSpecV1] = &[
        TargetFeatureSpecV1::new_unchecked("xnack", TargetFeatureStateV1::Disabled),
        TargetFeatureSpecV1::new_unchecked("wavefrontsize64", TargetFeatureStateV1::Enabled),
    ];
    const DUPLICATE_FEATURES: &[TargetFeatureSpecV1] = &[
        TargetFeatureSpecV1::new_unchecked("xnack", TargetFeatureStateV1::Disabled),
        TargetFeatureSpecV1::new_unchecked("xnack", TargetFeatureStateV1::Enabled),
    ];

    #[test]
    fn canonical_profile_text_is_target_neutral() {
        let profile = TargetProfileSpecV1::from_static_parts(
            TargetVendorV1::Amd,
            TargetArchitectureFamilyV1::Amdgcn,
            "gfx942",
            Some("amdgcn-amd-amdhsa"),
            Some("amdgcn-amd-amdhsa"),
            TargetArtifactFormatV1::AmdHsaCodeObject,
            TargetExecutionModelV1::GpuGrid,
            Some("e-p:64:64"),
            AMD_FEATURES,
        );

        assert_eq!(profile.validate(), Ok(()));
        assert_eq!(profile.vendor(), TargetVendorV1::Amd);
        assert_eq!(
            profile.feature("xnack").unwrap().state(),
            TargetFeatureStateV1::Disabled
        );
        assert_eq!(
            profile.to_string(),
            "fe2o3.target-profile.v1;vendor=amd;architecture-family=amdgcn;architecture=gfx942;rustc-target=amdgcn-amd-amdhsa;llvm-target=amdgcn-amd-amdhsa;artifact-format=amd-hsa-code-object;execution-model=gpu-grid;data-layout=e-p:64:64;features=wavefrontsize32:disabled,wavefrontsize64:enabled,xnack:disabled"
        );
    }

    #[test]
    fn accepts_non_amd_profiles() {
        let ptx_profile = TargetProfileSpecV1::from_static_parts(
            TargetVendorV1::Nvidia,
            TargetArchitectureFamilyV1::Nvptx,
            "sm_90",
            Some("nvptx64-nvidia-cuda"),
            Some("nvptx64-nvidia-cuda"),
            TargetArtifactFormatV1::PtxText,
            TargetExecutionModelV1::GpuGrid,
            None,
            PTX_FEATURES,
        );
        let spirv_profile = TargetProfileSpecV1::from_static_parts(
            TargetVendorV1::Other,
            TargetArchitectureFamilyV1::Spirv,
            "spirv64",
            None,
            None,
            TargetArtifactFormatV1::SpirvBinary,
            TargetExecutionModelV1::AcceleratorQueue,
            None,
            &[],
        );

        assert_eq!(ptx_profile.validate(), Ok(()));
        assert_eq!(spirv_profile.validate(), Ok(()));
        assert!(spirv_profile.to_string().contains("features=<absent>"));
    }

    #[test]
    fn rejects_ambiguous_profile_fields() {
        let empty_architecture = TargetProfileSpecV1::from_static_parts(
            TargetVendorV1::Host,
            TargetArchitectureFamilyV1::X86_64,
            "",
            None,
            None,
            TargetArtifactFormatV1::NativeObject,
            TargetExecutionModelV1::CpuProcess,
            None,
            &[],
        );
        let reserved_target = TargetProfileSpecV1::from_static_parts(
            TargetVendorV1::Host,
            TargetArchitectureFamilyV1::X86_64,
            "x86_64",
            Some("x86_64-unknown-linux-gnu;mutated"),
            None,
            TargetArtifactFormatV1::NativeObject,
            TargetExecutionModelV1::CpuProcess,
            None,
            &[],
        );
        let reserved_absent = TargetProfileSpecV1::from_static_parts(
            TargetVendorV1::Host,
            TargetArchitectureFamilyV1::X86_64,
            "x86_64",
            Some(ABSENT_PROFILE_FIELD),
            None,
            TargetArtifactFormatV1::NativeObject,
            TargetExecutionModelV1::CpuProcess,
            None,
            &[],
        );

        assert_eq!(
            empty_architecture.validate(),
            Err(TargetProfileValidationErrorV1::EmptyField {
                field: TargetProfileFieldV1::Architecture
            })
        );
        assert_eq!(
            reserved_target.validate(),
            Err(TargetProfileValidationErrorV1::ReservedDelimiterField {
                field: TargetProfileFieldV1::RustcTarget
            })
        );
        assert_eq!(
            reserved_absent.validate(),
            Err(TargetProfileValidationErrorV1::ReservedAbsentFieldValue {
                field: TargetProfileFieldV1::RustcTarget
            })
        );
    }

    #[test]
    fn rejects_noncanonical_feature_names() {
        assert_eq!(
            TargetFeatureSpecV1::new_unchecked("Wave64", TargetFeatureStateV1::Enabled).validate(),
            Err(TargetProfileValidationErrorV1::NonCanonicalFeatureName { name: "Wave64" })
        );
        assert_eq!(
            TargetFeatureSpecV1::new_unchecked("xnack+", TargetFeatureStateV1::Enabled).validate(),
            Err(TargetProfileValidationErrorV1::NonCanonicalFeatureName { name: "xnack+" })
        );
        assert_eq!(
            TargetFeatureSpecV1::new_unchecked("xn\u{e1}ck", TargetFeatureStateV1::Enabled)
                .validate(),
            Err(TargetProfileValidationErrorV1::NonAsciiFeatureName { name: "xn\u{e1}ck" })
        );
    }

    #[test]
    fn feature_names_must_be_sorted_and_unique() {
        let unsorted = TargetProfileSpecV1::from_static_parts(
            TargetVendorV1::Amd,
            TargetArchitectureFamilyV1::Amdgcn,
            "gfx942",
            Some("amdgcn-amd-amdhsa"),
            Some("amdgcn-amd-amdhsa"),
            TargetArtifactFormatV1::AmdHsaCodeObject,
            TargetExecutionModelV1::GpuGrid,
            Some("e-p:64:64"),
            UNSORTED_FEATURES,
        );
        let duplicate = TargetProfileSpecV1::from_static_parts(
            TargetVendorV1::Amd,
            TargetArchitectureFamilyV1::Amdgcn,
            "gfx942",
            Some("amdgcn-amd-amdhsa"),
            Some("amdgcn-amd-amdhsa"),
            TargetArtifactFormatV1::AmdHsaCodeObject,
            TargetExecutionModelV1::GpuGrid,
            Some("e-p:64:64"),
            DUPLICATE_FEATURES,
        );

        assert_eq!(
            unsorted.validate(),
            Err(TargetProfileValidationErrorV1::UnsortedFeatureName {
                previous: "xnack",
                current: "wavefrontsize64"
            })
        );
        assert_eq!(
            duplicate.validate(),
            Err(TargetProfileValidationErrorV1::UnsortedFeatureName {
                previous: "xnack",
                current: "xnack"
            })
        );
    }
}
