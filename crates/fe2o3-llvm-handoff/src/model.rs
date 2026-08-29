use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};
use core::fmt;

use sha2::{Digest as _, Sha256};

use crate::{HandoffDiagnosticV1, HandoffLimitV1};

pub const GFX942_AMDHSA_TARGET_TRIPLE_V1: &str = "amdgcn-amd-amdhsa";
pub const GFX942_AMDHSA_DATA_LAYOUT_V1: &str =
    fe2o3_amd_target::PRODUCTION_AMDHSA_LLVM_DATA_LAYOUT_V1;

pub const MAX_SYMBOL_BYTES_V1: usize = 128;
pub const MAX_SOURCE_PATH_BYTES_V1: usize = 512;
pub const MAX_KERNELS_V1: usize = 64;
pub const MAX_KERNEL_PARAMETERS_V1: usize = 64;
pub const MAX_PARAMETER_ATTRIBUTES_V1: usize = 16;
pub const MAX_FUNCTION_ATTRIBUTES_V1: usize = 16;
pub const MAX_MODULE_FLAGS_V1: usize = 8;
pub const MAX_NAMED_METADATA_V1: usize = 8;
pub const MAX_DEVICE_LIBRARIES_V1: usize = 16;
pub const MAX_DEVICE_LIBRARY_BYTES_V1: u64 = 128 * 1024 * 1024;
pub const MAX_ORIGINS_V1: usize = 1_024;
pub const MAX_OBLIGATIONS_V1: usize = 1_024;

const HANDOFF_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.llvm-handoff.identity.v1";
const ORIGIN_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.llvm-handoff.origin.v1";
const OBLIGATION_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.llvm-handoff.obligation.v1";

/// A nonzero external identity carried into the handoff.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct IdentityV1(pub(crate) [u8; 32]);

impl IdentityV1 {
    pub fn new(bytes: [u8; 32]) -> Result<Self, HandoffDiagnosticV1> {
        Self::named(bytes, "external")
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub(crate) fn named(bytes: [u8; 32], name: &'static str) -> Result<Self, HandoffDiagnosticV1> {
        if bytes == [0; 32] {
            return Err(HandoffDiagnosticV1::ZeroIdentity(name));
        }
        Ok(Self(bytes))
    }
}

impl fmt::Display for IdentityV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_hex(formatter, &self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HandoffIdentityV1([u8; 32]);

impl HandoffIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for HandoffIdentityV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_hex(formatter, &self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct OriginIdentityV1(pub(crate) [u8; 32]);

impl OriginIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for OriginIdentityV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_hex(formatter, &self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ObligationIdentityV1(pub(crate) [u8; 32]);

impl ObligationIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for ObligationIdentityV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_hex(formatter, &self.0)
    }
}

/// Identity boundaries established before machine lowering.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StageIdentitiesV1 {
    pub(crate) semantic: IdentityV1,
    pub(crate) schedule: IdentityV1,
    pub(crate) target_plan: IdentityV1,
}

impl StageIdentitiesV1 {
    pub fn new(
        semantic: [u8; 32],
        schedule: [u8; 32],
        target_plan: [u8; 32],
    ) -> Result<Self, HandoffDiagnosticV1> {
        Ok(Self {
            semantic: IdentityV1::named(semantic, "semantic")?,
            schedule: IdentityV1::named(schedule, "schedule")?,
            target_plan: IdentityV1::named(target_plan, "target-plan")?,
        })
    }

    pub const fn semantic(&self) -> IdentityV1 {
        self.semantic
    }

    pub const fn schedule(&self) -> IdentityV1 {
        self.schedule
    }

    pub const fn target_plan(&self) -> IdentityV1 {
        self.target_plan
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TargetFeatureV1 {
    WavefrontSize32,
    WavefrontSize64,
    Xnack,
}

impl TargetFeatureV1 {
    pub const fn canonical_name(self) -> &'static str {
        match self {
            Self::WavefrontSize32 => "wavefrontsize32",
            Self::WavefrontSize64 => "wavefrontsize64",
            Self::Xnack => "xnack",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TargetFeatureStateV1 {
    pub(crate) feature: TargetFeatureV1,
    pub(crate) enabled: bool,
}

impl TargetFeatureStateV1 {
    pub const fn new(feature: TargetFeatureV1, enabled: bool) -> Self {
        Self { feature, enabled }
    }

    pub const fn feature(self) -> TargetFeatureV1 {
        self.feature
    }

    pub const fn enabled(self) -> bool {
        self.enabled
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CodeObjectVersionV1 {
    V6,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum OptimizationLevelV1 {
    O2,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RelocationModelV1 {
    Pic,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CodeModelV1 {
    Small,
}

/// The only target-machine policy admitted by schema V1.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Gfx942TargetPolicyV1 {
    pub(crate) features: Vec<TargetFeatureStateV1>,
    pub(crate) code_object: CodeObjectVersionV1,
    pub(crate) optimization: OptimizationLevelV1,
    pub(crate) relocation: RelocationModelV1,
    pub(crate) code_model: CodeModelV1,
}

impl Gfx942TargetPolicyV1 {
    pub fn canonical() -> Self {
        Self {
            features: vec![
                TargetFeatureStateV1::new(TargetFeatureV1::WavefrontSize32, false),
                TargetFeatureStateV1::new(TargetFeatureV1::WavefrontSize64, true),
                TargetFeatureStateV1::new(TargetFeatureV1::Xnack, false),
            ],
            code_object: CodeObjectVersionV1::V6,
            optimization: OptimizationLevelV1::O2,
            relocation: RelocationModelV1::Pic,
            code_model: CodeModelV1::Small,
        }
    }

    pub const fn target_triple(&self) -> &'static str {
        GFX942_AMDHSA_TARGET_TRIPLE_V1
    }

    pub const fn data_layout(&self) -> &'static str {
        GFX942_AMDHSA_DATA_LAYOUT_V1
    }

    pub const fn cpu(&self) -> &'static str {
        "gfx942"
    }

    pub fn features(&self) -> &[TargetFeatureStateV1] {
        &self.features
    }

    pub const fn code_object_version(&self) -> CodeObjectVersionV1 {
        self.code_object
    }

    pub const fn optimization_level(&self) -> OptimizationLevelV1 {
        self.optimization
    }

    pub const fn relocation_model(&self) -> RelocationModelV1 {
        self.relocation
    }

    pub const fn code_model(&self) -> CodeModelV1 {
        self.code_model
    }

    pub(crate) fn from_parts(
        mut features: Vec<TargetFeatureStateV1>,
        code_object: CodeObjectVersionV1,
        optimization: OptimizationLevelV1,
        relocation: RelocationModelV1,
        code_model: CodeModelV1,
    ) -> Result<Self, HandoffDiagnosticV1> {
        features.sort_unstable();
        for pair in features.windows(2) {
            if pair[0].feature == pair[1].feature {
                return Err(if pair[0].enabled == pair[1].enabled {
                    HandoffDiagnosticV1::DuplicateTargetFeature
                } else {
                    HandoffDiagnosticV1::ConflictingTargetFeature
                });
            }
        }
        for required in [
            TargetFeatureV1::WavefrontSize32,
            TargetFeatureV1::WavefrontSize64,
            TargetFeatureV1::Xnack,
        ] {
            if !features.iter().any(|value| value.feature == required) {
                return Err(HandoffDiagnosticV1::MissingTargetFeature(
                    required.canonical_name(),
                ));
            }
        }
        let value = Self {
            features,
            code_object,
            optimization,
            relocation,
            code_model,
        };
        if value != Self::canonical() {
            return Err(HandoffDiagnosticV1::UnsupportedTargetPolicy);
        }
        Ok(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AddressSpaceV1 {
    Flat,
    Global,
    Region,
    Local,
    Constant,
    Private,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ScalarTypeV1 {
    I1,
    I8,
    I16,
    I32,
    I64,
    F16,
    Bf16,
    F32,
    F64,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum KernelValueTypeV1 {
    Scalar(ScalarTypeV1),
    Pointer {
        pointee: ScalarTypeV1,
        address_space: AddressSpaceV1,
    },
}

impl KernelValueTypeV1 {
    const fn is_pointer(self) -> bool {
        matches!(self, Self::Pointer { .. })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ParameterAttributeV1 {
    NoAlias,
    NoCapture,
    NonNull,
    ReadOnly,
    WriteOnly,
    Align(u16),
    Dereferenceable(u32),
}

impl ParameterAttributeV1 {
    pub(crate) const fn kind(self) -> u8 {
        match self {
            Self::NoAlias => 1,
            Self::NoCapture => 2,
            Self::NonNull => 3,
            Self::ReadOnly => 4,
            Self::WriteOnly => 5,
            Self::Align(_) => 6,
            Self::Dereferenceable(_) => 7,
        }
    }

    pub const fn canonical_name(self) -> &'static str {
        match self {
            Self::NoAlias => "noalias",
            Self::NoCapture => "nocapture",
            Self::NonNull => "nonnull",
            Self::ReadOnly => "readonly",
            Self::WriteOnly => "writeonly",
            Self::Align(_) => "align",
            Self::Dereferenceable(_) => "dereferenceable",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KernelParameterV1 {
    pub(crate) name: String,
    pub(crate) value_type: KernelValueTypeV1,
    pub(crate) attributes: Vec<ParameterAttributeV1>,
}

impl KernelParameterV1 {
    pub fn new(
        name: &str,
        value_type: KernelValueTypeV1,
        mut attributes: Vec<ParameterAttributeV1>,
    ) -> Result<Self, HandoffDiagnosticV1> {
        validate_symbol(name)?;
        check_count(
            HandoffLimitV1::ParameterAttributes,
            attributes.len(),
            MAX_PARAMETER_ATTRIBUTES_V1,
        )?;
        attributes.sort_unstable();
        for pair in attributes.windows(2) {
            if pair[0].kind() == pair[1].kind() {
                return Err(HandoffDiagnosticV1::DuplicateParameterAttribute(
                    pair[0].canonical_name(),
                ));
            }
        }
        let readonly = attributes.contains(&ParameterAttributeV1::ReadOnly);
        let writeonly = attributes.contains(&ParameterAttributeV1::WriteOnly);
        if readonly && writeonly {
            return Err(HandoffDiagnosticV1::ConflictingParameterAttributes);
        }
        for attribute in &attributes {
            if !value_type.is_pointer() {
                return Err(HandoffDiagnosticV1::AttributeRequiresPointer(
                    attribute.canonical_name(),
                ));
            }
            match attribute {
                ParameterAttributeV1::Align(value)
                    if *value == 0 || !value.is_power_of_two() || *value > 256 =>
                {
                    return Err(HandoffDiagnosticV1::InvalidParameterAttribute("align"));
                }
                ParameterAttributeV1::Dereferenceable(0) => {
                    return Err(HandoffDiagnosticV1::InvalidParameterAttribute(
                        "dereferenceable",
                    ));
                }
                _ => {}
            }
        }
        Ok(Self {
            name: name.to_string(),
            value_type,
            attributes,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn value_type(&self) -> KernelValueTypeV1 {
        self.value_type
    }

    pub fn attributes(&self) -> &[ParameterAttributeV1] {
        &self.attributes
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WorkgroupSizeRangeV1 {
    pub(crate) minimum: u16,
    pub(crate) maximum: u16,
}

impl WorkgroupSizeRangeV1 {
    /// Constructs an inclusive flat-workgroup-size range in workitems.
    ///
    /// The bounds are not wave counts and therefore need not be multiples of
    /// the target wavefront size. Wave32/wave64 selection is carried by the
    /// independent target-feature policy.
    pub fn new(minimum: u16, maximum: u16) -> Result<Self, HandoffDiagnosticV1> {
        if minimum == 0 || minimum > maximum || maximum > 1_024 {
            return Err(HandoffDiagnosticV1::InvalidWorkgroupSizeRange);
        }
        Ok(Self { minimum, maximum })
    }

    pub const fn minimum(self) -> u16 {
        self.minimum
    }

    pub const fn maximum(self) -> u16 {
        self.maximum
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WavesPerEuV1 {
    pub(crate) minimum: u8,
    pub(crate) maximum: u8,
}

impl WavesPerEuV1 {
    pub fn new(minimum: u8, maximum: u8) -> Result<Self, HandoffDiagnosticV1> {
        if minimum == 0 || minimum > maximum || maximum > 10 {
            return Err(HandoffDiagnosticV1::InvalidWavesPerEu);
        }
        Ok(Self { minimum, maximum })
    }

    pub const fn minimum(self) -> u8 {
        self.minimum
    }

    pub const fn maximum(self) -> u8 {
        self.maximum
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum FunctionAttributeV1 {
    NoUnwind,
    FlatWorkgroupSize(WorkgroupSizeRangeV1),
    WavesPerEu(WavesPerEuV1),
    DenormalFpMathF32Ieee,
    UnsafeFpMathDisabled,
    NoInfsFpMathDisabled,
    NoNansFpMathDisabled,
    NoSignedZerosFpMathDisabled,
    ApproxFuncFpMathDisabled,
    FpContractOff,
    NoCompletionAction,
    NoDefaultQueue,
    NoHeapPointer,
    NoHostcallPointer,
    NoMultigridSyncArgument,
    NoQueuePointer,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CallingConventionV1 {
    AmdGpuKernel,
}

impl CallingConventionV1 {
    pub const fn llvm_name(self) -> &'static str {
        match self {
            Self::AmdGpuKernel => "amdgpu_kernel",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum KernelReturnTypeV1 {
    Void,
}

impl FunctionAttributeV1 {
    pub fn gfx942_kernel_defaults(flat_workgroup_size: WorkgroupSizeRangeV1) -> Vec<Self> {
        vec![
            Self::NoUnwind,
            Self::FlatWorkgroupSize(flat_workgroup_size),
            Self::DenormalFpMathF32Ieee,
            Self::UnsafeFpMathDisabled,
            Self::NoInfsFpMathDisabled,
            Self::NoNansFpMathDisabled,
            Self::NoSignedZerosFpMathDisabled,
            Self::ApproxFuncFpMathDisabled,
            Self::FpContractOff,
        ]
    }

    pub(crate) const fn kind(self) -> u8 {
        match self {
            Self::NoUnwind => 1,
            Self::FlatWorkgroupSize(_) => 2,
            Self::WavesPerEu(_) => 3,
            Self::DenormalFpMathF32Ieee => 4,
            Self::UnsafeFpMathDisabled => 5,
            Self::NoInfsFpMathDisabled => 6,
            Self::NoNansFpMathDisabled => 7,
            Self::NoSignedZerosFpMathDisabled => 8,
            Self::ApproxFuncFpMathDisabled => 9,
            Self::FpContractOff => 10,
            Self::NoCompletionAction => 11,
            Self::NoDefaultQueue => 12,
            Self::NoHeapPointer => 13,
            Self::NoHostcallPointer => 14,
            Self::NoMultigridSyncArgument => 15,
            Self::NoQueuePointer => 16,
        }
    }

    pub const fn canonical_name(self) -> &'static str {
        match self {
            Self::NoUnwind => "nounwind",
            Self::FlatWorkgroupSize(_) => "amdgpu-flat-work-group-size",
            Self::WavesPerEu(_) => "amdgpu-waves-per-eu",
            Self::DenormalFpMathF32Ieee => "denormal-fp-math-f32=ieee,ieee",
            Self::UnsafeFpMathDisabled => "unsafe-fp-math=false",
            Self::NoInfsFpMathDisabled => "no-infs-fp-math=false",
            Self::NoNansFpMathDisabled => "no-nans-fp-math=false",
            Self::NoSignedZerosFpMathDisabled => "no-signed-zeros-fp-math=false",
            Self::ApproxFuncFpMathDisabled => "approx-func-fp-math=false",
            Self::FpContractOff => "fp-contract=off",
            Self::NoCompletionAction => "amdgpu-no-completion-action",
            Self::NoDefaultQueue => "amdgpu-no-default-queue",
            Self::NoHeapPointer => "amdgpu-no-heap-ptr",
            Self::NoHostcallPointer => "amdgpu-no-hostcall-ptr",
            Self::NoMultigridSyncArgument => "amdgpu-no-multigrid-sync-arg",
            Self::NoQueuePointer => "amdgpu-no-queue-ptr",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KernelEntryV1 {
    pub(crate) symbol: String,
    pub(crate) parameters: Vec<KernelParameterV1>,
    pub(crate) function_attributes: Vec<FunctionAttributeV1>,
    pub(crate) origin: OriginIdentityV1,
}

impl KernelEntryV1 {
    pub fn new(
        symbol: &str,
        parameters: Vec<KernelParameterV1>,
        mut function_attributes: Vec<FunctionAttributeV1>,
        origin: OriginIdentityV1,
    ) -> Result<Self, HandoffDiagnosticV1> {
        validate_symbol(symbol)?;
        check_count(
            HandoffLimitV1::KernelParameters,
            parameters.len(),
            MAX_KERNEL_PARAMETERS_V1,
        )?;
        check_count(
            HandoffLimitV1::FunctionAttributes,
            function_attributes.len(),
            MAX_FUNCTION_ATTRIBUTES_V1,
        )?;
        for (index, parameter) in parameters.iter().enumerate() {
            if parameters[..index]
                .iter()
                .any(|prior| prior.name == parameter.name)
            {
                return Err(HandoffDiagnosticV1::DuplicateKernelParameter(
                    parameter.name.clone(),
                ));
            }
        }
        function_attributes.sort_unstable();
        for pair in function_attributes.windows(2) {
            if pair[0].kind() == pair[1].kind() {
                return Err(HandoffDiagnosticV1::DuplicateFunctionAttribute(
                    pair[0].canonical_name(),
                ));
            }
        }
        for required in [1_u8, 2, 4, 5, 6, 7, 8, 9, 10] {
            if !function_attributes
                .iter()
                .any(|attribute| attribute.kind() == required)
            {
                return Err(HandoffDiagnosticV1::MissingFunctionAttribute(
                    function_attribute_name(required),
                ));
            }
        }
        Ok(Self {
            symbol: symbol.to_string(),
            parameters,
            function_attributes,
            origin,
        })
    }

    pub fn symbol(&self) -> &str {
        &self.symbol
    }

    pub const fn calling_convention(&self) -> CallingConventionV1 {
        CallingConventionV1::AmdGpuKernel
    }

    pub const fn return_type(&self) -> KernelReturnTypeV1 {
        KernelReturnTypeV1::Void
    }

    pub fn parameters(&self) -> &[KernelParameterV1] {
        &self.parameters
    }

    pub fn function_attributes(&self) -> &[FunctionAttributeV1] {
        &self.function_attributes
    }

    pub const fn origin(&self) -> OriginIdentityV1 {
        self.origin
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ModuleFlagV1 {
    CodeObjectVersion6,
    PicLevel2,
    WcharSize4,
}

impl ModuleFlagV1 {
    pub const fn canonical_name(self) -> &'static str {
        match self {
            Self::CodeObjectVersion6 => "amdhsa-code-object-version=6",
            Self::PicLevel2 => "PIC Level=2",
            Self::WcharSize4 => "wchar_size=4",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum NamedMetadataV1 {
    OpenClVersion2_0,
    OpenClSpirVersion2_0,
    ProducerIdentity(IdentityV1),
}

impl NamedMetadataV1 {
    pub(crate) const fn kind(self) -> u8 {
        match self {
            Self::OpenClVersion2_0 => 1,
            Self::OpenClSpirVersion2_0 => 2,
            Self::ProducerIdentity(_) => 3,
        }
    }

    pub const fn canonical_name(self) -> &'static str {
        match self {
            Self::OpenClVersion2_0 => "opencl.ocl.version=2.0",
            Self::OpenClSpirVersion2_0 => "opencl.spir.version=2.0",
            Self::ProducerIdentity(_) => "llvm.ident.sha256",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DeviceLibraryKindV1 {
    Ocml,
    Ockl,
    OpenCl,
    OclcIsaVersion942,
    OclcWavefrontSize64On,
    OclcFiniteOnlyOff,
    OclcUnsafeMathOff,
    OclcCorrectlyRoundedSqrtOn,
    OclcDazOff,
}

impl DeviceLibraryKindV1 {
    pub const fn canonical_name(self) -> &'static str {
        match self {
            Self::Ocml => "ocml",
            Self::Ockl => "ockl",
            Self::OpenCl => "opencl",
            Self::OclcIsaVersion942 => "oclc_isa_version_942",
            Self::OclcWavefrontSize64On => "oclc_wavefrontsize64_on",
            Self::OclcFiniteOnlyOff => "oclc_finite_only_off",
            Self::OclcUnsafeMathOff => "oclc_unsafe_math_off",
            Self::OclcCorrectlyRoundedSqrtOn => "oclc_correctly_rounded_sqrt_on",
            Self::OclcDazOff => "oclc_daz_opt_off",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DeviceLibraryInputV1 {
    pub(crate) kind: DeviceLibraryKindV1,
    pub(crate) sha256: IdentityV1,
    pub(crate) byte_len: u64,
}

impl DeviceLibraryInputV1 {
    pub fn new(
        kind: DeviceLibraryKindV1,
        sha256: [u8; 32],
        byte_len: u64,
    ) -> Result<Self, HandoffDiagnosticV1> {
        if byte_len == 0 || byte_len > MAX_DEVICE_LIBRARY_BYTES_V1 {
            return Err(HandoffDiagnosticV1::InvalidDeviceLibrarySize);
        }
        Ok(Self {
            kind,
            sha256: IdentityV1::named(sha256, "device-library SHA-256")?,
            byte_len,
        })
    }

    pub const fn kind(self) -> DeviceLibraryKindV1 {
        self.kind
    }

    pub const fn sha256(self) -> IdentityV1 {
        self.sha256
    }

    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleMetadataV1 {
    pub(crate) flags: Vec<ModuleFlagV1>,
    pub(crate) named: Vec<NamedMetadataV1>,
    pub(crate) device_libraries: Vec<DeviceLibraryInputV1>,
}

impl ModuleMetadataV1 {
    pub fn new(
        mut flags: Vec<ModuleFlagV1>,
        mut named: Vec<NamedMetadataV1>,
        mut device_libraries: Vec<DeviceLibraryInputV1>,
    ) -> Result<Self, HandoffDiagnosticV1> {
        check_count(
            HandoffLimitV1::ModuleFlags,
            flags.len(),
            MAX_MODULE_FLAGS_V1,
        )?;
        check_count(
            HandoffLimitV1::NamedMetadata,
            named.len(),
            MAX_NAMED_METADATA_V1,
        )?;
        check_count(
            HandoffLimitV1::DeviceLibraries,
            device_libraries.len(),
            MAX_DEVICE_LIBRARIES_V1,
        )?;
        flags.sort_unstable();
        for pair in flags.windows(2) {
            if pair[0] == pair[1] {
                return Err(HandoffDiagnosticV1::DuplicateModuleFlag(
                    pair[0].canonical_name(),
                ));
            }
        }
        for required in [ModuleFlagV1::CodeObjectVersion6, ModuleFlagV1::PicLevel2] {
            if !flags.contains(&required) {
                return Err(HandoffDiagnosticV1::MissingModuleFlag(
                    required.canonical_name(),
                ));
            }
        }
        named.sort_unstable();
        for pair in named.windows(2) {
            if pair[0].kind() == pair[1].kind() {
                return Err(HandoffDiagnosticV1::DuplicateNamedMetadata(
                    pair[0].canonical_name(),
                ));
            }
        }
        device_libraries.sort_unstable();
        for pair in device_libraries.windows(2) {
            if pair[0].kind == pair[1].kind {
                return Err(HandoffDiagnosticV1::DuplicateDeviceLibrary(
                    pair[0].kind.canonical_name(),
                ));
            }
        }
        Ok(Self {
            flags,
            named,
            device_libraries,
        })
    }

    pub fn flags(&self) -> &[ModuleFlagV1] {
        &self.flags
    }

    pub fn named_metadata(&self) -> &[NamedMetadataV1] {
        &self.named
    }

    pub fn device_libraries(&self) -> &[DeviceLibraryInputV1] {
        &self.device_libraries
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceSpanV1 {
    pub(crate) path: String,
    pub(crate) start_line: u32,
    pub(crate) start_column: u32,
    pub(crate) end_line: u32,
    pub(crate) end_column: u32,
}

impl SourceSpanV1 {
    pub fn new(
        path: &str,
        start_line: u32,
        start_column: u32,
        end_line: u32,
        end_column: u32,
    ) -> Result<Self, HandoffDiagnosticV1> {
        validate_source_path(path)?;
        if start_line == 0
            || start_column == 0
            || end_line == 0
            || end_column == 0
            || (end_line, end_column) < (start_line, start_column)
        {
            return Err(HandoffDiagnosticV1::InvalidSourceSpan);
        }
        Ok(Self {
            path: path.to_string(),
            start_line,
            start_column,
            end_line,
            end_column,
        })
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub const fn start(&self) -> (u32, u32) {
        (self.start_line, self.start_column)
    }

    pub const fn end(&self) -> (u32, u32) {
        (self.end_line, self.end_column)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum OriginKindV1 {
    RustSource,
    Mir,
    KernelIr,
    ScheduleIr,
    AmdgcnIr,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OriginV1 {
    pub(crate) identity: OriginIdentityV1,
    pub(crate) kind: OriginKindV1,
    pub(crate) source_identity: IdentityV1,
    pub(crate) span: Option<SourceSpanV1>,
}

impl OriginV1 {
    pub fn new(
        kind: OriginKindV1,
        source_identity: IdentityV1,
        span: Option<SourceSpanV1>,
    ) -> Self {
        let identity = canonical_origin_identity(kind, source_identity, span.as_ref());
        Self {
            identity,
            kind,
            source_identity,
            span,
        }
    }

    pub const fn identity(&self) -> OriginIdentityV1 {
        self.identity
    }

    pub const fn kind(&self) -> OriginKindV1 {
        self.kind
    }

    pub const fn source_identity(&self) -> IdentityV1 {
        self.source_identity
    }

    pub const fn span(&self) -> Option<&SourceSpanV1> {
        self.span.as_ref()
    }

    pub(crate) fn identity_is_valid(&self) -> bool {
        self.identity
            == canonical_origin_identity(self.kind, self.source_identity, self.span.as_ref())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ObligationKindV1 {
    PreserveKernelAbi,
    PreserveAddressSpaces,
    PreserveTargetFeatures,
    PreserveCallingConvention,
    PreserveFunctionAttributes,
    PreserveModuleMetadata,
    AuthenticateDeviceLibraries,
    MaintainOriginCoverage,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ObligationV1 {
    pub(crate) identity: ObligationIdentityV1,
    pub(crate) kind: ObligationKindV1,
    pub(crate) subject: IdentityV1,
    pub(crate) origin: OriginIdentityV1,
}

impl ObligationV1 {
    pub fn new(kind: ObligationKindV1, subject: IdentityV1, origin: OriginIdentityV1) -> Self {
        let identity = canonical_obligation_identity(kind, subject, origin);
        Self {
            identity,
            kind,
            subject,
            origin,
        }
    }

    pub const fn identity(self) -> ObligationIdentityV1 {
        self.identity
    }

    pub const fn kind(self) -> ObligationKindV1 {
        self.kind
    }

    pub const fn subject(self) -> IdentityV1 {
        self.subject
    }

    pub const fn origin(self) -> OriginIdentityV1 {
        self.origin
    }

    pub(crate) fn identity_is_valid(self) -> bool {
        self.identity == canonical_obligation_identity(self.kind, self.subject, self.origin)
    }
}

/// Unordered construction data for one checked handoff.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Gfx942HandoffInputV1 {
    pub stage_identities: StageIdentitiesV1,
    pub target: Gfx942TargetPolicyV1,
    pub kernels: Vec<KernelEntryV1>,
    pub module: ModuleMetadataV1,
    pub origins: Vec<OriginV1>,
    pub obligations: Vec<ObligationV1>,
}

/// A validated, canonicalizable gfx942 LLVM handoff.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Gfx942HandoffV1 {
    pub(crate) stage_identities: StageIdentitiesV1,
    pub(crate) target: Gfx942TargetPolicyV1,
    pub(crate) kernels: Vec<KernelEntryV1>,
    pub(crate) module: ModuleMetadataV1,
    pub(crate) origins: Vec<OriginV1>,
    pub(crate) obligations: Vec<ObligationV1>,
}

impl Gfx942HandoffV1 {
    pub fn new(mut input: Gfx942HandoffInputV1) -> Result<Self, HandoffDiagnosticV1> {
        check_nonempty_count(
            "kernels",
            HandoffLimitV1::Kernels,
            input.kernels.len(),
            MAX_KERNELS_V1,
        )?;
        check_nonempty_count(
            "origins",
            HandoffLimitV1::Origins,
            input.origins.len(),
            MAX_ORIGINS_V1,
        )?;
        check_nonempty_count(
            "obligations",
            HandoffLimitV1::Obligations,
            input.obligations.len(),
            MAX_OBLIGATIONS_V1,
        )?;
        if input.target != Gfx942TargetPolicyV1::canonical() {
            return Err(HandoffDiagnosticV1::UnsupportedTargetPolicy);
        }

        input
            .kernels
            .sort_unstable_by(|left, right| left.symbol.cmp(&right.symbol));
        for pair in input.kernels.windows(2) {
            if pair[0].symbol == pair[1].symbol {
                return Err(HandoffDiagnosticV1::DuplicateKernel(pair[0].symbol.clone()));
            }
        }

        input.origins.sort_unstable_by_key(|origin| origin.identity);
        for origin in &input.origins {
            if !origin.identity_is_valid() {
                return Err(HandoffDiagnosticV1::DuplicateOrigin);
            }
        }
        for pair in input.origins.windows(2) {
            if pair[0].identity == pair[1].identity {
                return Err(HandoffDiagnosticV1::DuplicateOrigin);
            }
        }

        input
            .obligations
            .sort_unstable_by_key(|obligation| obligation.identity);
        for obligation in &input.obligations {
            if !obligation.identity_is_valid() {
                return Err(HandoffDiagnosticV1::DuplicateObligation);
            }
        }
        for pair in input.obligations.windows(2) {
            if pair[0].identity == pair[1].identity {
                return Err(HandoffDiagnosticV1::DuplicateObligation);
            }
        }

        let origin_exists = |identity: OriginIdentityV1| {
            input
                .origins
                .binary_search_by_key(&identity, |origin| origin.identity)
                .is_ok()
        };
        if input
            .kernels
            .iter()
            .any(|kernel| !origin_exists(kernel.origin))
            || input
                .obligations
                .iter()
                .any(|obligation| !origin_exists(obligation.origin))
        {
            return Err(HandoffDiagnosticV1::MissingOriginReference);
        }

        let handoff = Self {
            stage_identities: input.stage_identities,
            target: input.target,
            kernels: input.kernels,
            module: input.module,
            origins: input.origins,
            obligations: input.obligations,
        };
        let encoded_len = crate::codec::encode_handoff_v1(&handoff).len();
        if encoded_len > crate::MAX_CANONICAL_HANDOFF_BYTES_V1 {
            return Err(HandoffDiagnosticV1::LimitExceeded {
                limit: HandoffLimitV1::CanonicalBytes,
                observed: encoded_len as u64,
                maximum: crate::MAX_CANONICAL_HANDOFF_BYTES_V1 as u64,
            });
        }
        Ok(handoff)
    }

    pub const fn stage_identities(&self) -> &StageIdentitiesV1 {
        &self.stage_identities
    }

    pub const fn target(&self) -> &Gfx942TargetPolicyV1 {
        &self.target
    }

    pub fn kernels(&self) -> &[KernelEntryV1] {
        &self.kernels
    }

    pub const fn module(&self) -> &ModuleMetadataV1 {
        &self.module
    }

    pub fn origins(&self) -> &[OriginV1] {
        &self.origins
    }

    pub fn obligations(&self) -> &[ObligationV1] {
        &self.obligations
    }

    pub fn encode_canonical(&self) -> crate::CanonicalHandoffBytesV1 {
        crate::CanonicalHandoffBytesV1::from_validated(crate::codec::encode_handoff_v1(self))
    }

    pub fn identity(&self) -> HandoffIdentityV1 {
        let bytes = self.encode_canonical();
        let mut hasher = Sha256::new();
        hasher.update((HANDOFF_IDENTITY_DOMAIN_V1.len() as u32).to_le_bytes());
        hasher.update(HANDOFF_IDENTITY_DOMAIN_V1);
        hasher.update((bytes.len() as u32).to_le_bytes());
        hasher.update(bytes.as_bytes());
        HandoffIdentityV1(hasher.finalize().into())
    }

    pub fn decode_canonical(bytes: &[u8]) -> Result<Self, crate::DecodeHandoffErrorV1> {
        crate::codec::decode_handoff_v1(bytes)
    }
}

fn canonical_origin_identity(
    kind: OriginKindV1,
    source_identity: IdentityV1,
    span: Option<&SourceSpanV1>,
) -> OriginIdentityV1 {
    let mut hasher = Sha256::new();
    hasher.update((ORIGIN_IDENTITY_DOMAIN_V1.len() as u32).to_le_bytes());
    hasher.update(ORIGIN_IDENTITY_DOMAIN_V1);
    hasher.update([origin_kind_tag(kind)]);
    hasher.update(source_identity.0);
    match span {
        None => hasher.update([0]),
        Some(span) => {
            hasher.update([1]);
            hasher.update((span.path.len() as u16).to_le_bytes());
            hasher.update(span.path.as_bytes());
            hasher.update(span.start_line.to_le_bytes());
            hasher.update(span.start_column.to_le_bytes());
            hasher.update(span.end_line.to_le_bytes());
            hasher.update(span.end_column.to_le_bytes());
        }
    }
    OriginIdentityV1(hasher.finalize().into())
}

fn canonical_obligation_identity(
    kind: ObligationKindV1,
    subject: IdentityV1,
    origin: OriginIdentityV1,
) -> ObligationIdentityV1 {
    let mut hasher = Sha256::new();
    hasher.update((OBLIGATION_IDENTITY_DOMAIN_V1.len() as u32).to_le_bytes());
    hasher.update(OBLIGATION_IDENTITY_DOMAIN_V1);
    hasher.update([obligation_kind_tag(kind)]);
    hasher.update(subject.0);
    hasher.update(origin.0);
    ObligationIdentityV1(hasher.finalize().into())
}

pub(crate) const fn origin_kind_tag(kind: OriginKindV1) -> u8 {
    match kind {
        OriginKindV1::RustSource => 1,
        OriginKindV1::Mir => 2,
        OriginKindV1::KernelIr => 3,
        OriginKindV1::ScheduleIr => 4,
        OriginKindV1::AmdgcnIr => 5,
    }
}

pub(crate) const fn obligation_kind_tag(kind: ObligationKindV1) -> u8 {
    match kind {
        ObligationKindV1::PreserveKernelAbi => 1,
        ObligationKindV1::PreserveAddressSpaces => 2,
        ObligationKindV1::PreserveTargetFeatures => 3,
        ObligationKindV1::PreserveCallingConvention => 4,
        ObligationKindV1::PreserveFunctionAttributes => 5,
        ObligationKindV1::PreserveModuleMetadata => 6,
        ObligationKindV1::AuthenticateDeviceLibraries => 7,
        ObligationKindV1::MaintainOriginCoverage => 8,
    }
}

fn validate_symbol(value: &str) -> Result<(), HandoffDiagnosticV1> {
    if value.len() > MAX_SYMBOL_BYTES_V1 {
        return Err(HandoffDiagnosticV1::LimitExceeded {
            limit: HandoffLimitV1::SymbolBytes,
            observed: value.len() as u64,
            maximum: MAX_SYMBOL_BYTES_V1 as u64,
        });
    }
    let mut bytes = value.bytes();
    let Some(first) = bytes.next() else {
        return Err(HandoffDiagnosticV1::InvalidSymbol);
    };
    if !(first.is_ascii_alphabetic() || first == b'_' || first == b'.')
        || !bytes
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'$' | b'-'))
    {
        return Err(HandoffDiagnosticV1::InvalidSymbol);
    }
    Ok(())
}

fn validate_source_path(value: &str) -> Result<(), HandoffDiagnosticV1> {
    if value.len() > MAX_SOURCE_PATH_BYTES_V1 {
        return Err(HandoffDiagnosticV1::LimitExceeded {
            limit: HandoffLimitV1::SourcePathBytes,
            observed: value.len() as u64,
            maximum: MAX_SOURCE_PATH_BYTES_V1 as u64,
        });
    }
    if value.is_empty()
        || value.starts_with('/')
        || !value.is_ascii()
        || value.bytes().any(|byte| {
            !(byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'_' | b'.' | b'-'))
        })
        || value
            .split('/')
            .any(|component| component.is_empty() || matches!(component, "." | ".."))
    {
        return Err(HandoffDiagnosticV1::InvalidSourcePath);
    }
    Ok(())
}

fn check_nonempty_count(
    name: &'static str,
    limit: HandoffLimitV1,
    observed: usize,
    maximum: usize,
) -> Result<(), HandoffDiagnosticV1> {
    if observed == 0 {
        return Err(HandoffDiagnosticV1::EmptyCollection(name));
    }
    check_count(limit, observed, maximum)
}

fn check_count(
    limit: HandoffLimitV1,
    observed: usize,
    maximum: usize,
) -> Result<(), HandoffDiagnosticV1> {
    if observed > maximum {
        return Err(HandoffDiagnosticV1::LimitExceeded {
            limit,
            observed: observed as u64,
            maximum: maximum as u64,
        });
    }
    Ok(())
}

fn function_attribute_name(kind: u8) -> &'static str {
    match kind {
        1 => "nounwind",
        2 => "amdgpu-flat-work-group-size",
        4 => "denormal-fp-math-f32=ieee,ieee",
        5 => "unsafe-fp-math=false",
        6 => "no-infs-fp-math=false",
        7 => "no-nans-fp-math=false",
        8 => "no-signed-zeros-fp-math=false",
        9 => "approx-func-fp-math=false",
        10 => "fp-contract=off",
        11 => "amdgpu-no-completion-action",
        12 => "amdgpu-no-default-queue",
        13 => "amdgpu-no-heap-ptr",
        14 => "amdgpu-no-hostcall-ptr",
        15 => "amdgpu-no-multigrid-sync-arg",
        16 => "amdgpu-no-queue-ptr",
        _ => "unknown",
    }
}

fn write_hex(formatter: &mut fmt::Formatter<'_>, bytes: &[u8]) -> fmt::Result {
    for byte in bytes {
        write!(formatter, "{byte:02x}")?;
    }
    Ok(())
}
