use std::{error::Error, fmt};

use fe2o3_amdgcn_model::AddressSpace;
use fe2o3_llvm_handoff::{
    DeviceLibraryInputV1, FunctionAttributeV1, IdentityV1, MAX_CANONICAL_HANDOFF_BYTES_V1,
    ModuleFlagV1, NamedMetadataV1, ObligationKindV1, OriginKindV1, ParameterAttributeV1,
    ScalarTypeV1, SourceSpanV1, StageIdentitiesV1, WorkgroupSizeRangeV1,
};

/// Maximum accepted byte length of each Pliron-compatible module, kernel, or parameter name.
pub const MAX_NAME_BYTES_V1: usize = 128;

/// Maximum number of source operations accepted by V1.
pub const MAX_OPERATIONS_V1: usize = 8;

/// Maximum number of function attributes accepted by V1.
pub const MAX_FUNCTION_ATTRIBUTES_V1: usize = 16;

/// Maximum number of parameter attributes accepted on one parameter by V1.
pub const MAX_PARAMETER_ATTRIBUTES_V1: usize = 16;

/// Maximum number of module flags accepted by V1.
pub const MAX_MODULE_FLAGS_V1: usize = 8;

/// Maximum number of named metadata entries accepted by V1.
pub const MAX_NAMED_METADATA_V1: usize = 8;

/// Maximum number of device-library declarations accepted by V1.
pub const MAX_DEVICE_LIBRARIES_V1: usize = 16;

/// Maximum number of preservation obligations accepted by V1.
pub const MAX_OBLIGATIONS_V1: usize = 8;

/// Maximum formatted size of every V1 diagnostic.
pub const MAX_DIAGNOSTIC_BYTES_V1: usize = 256;

/// Maximum byte length of the deterministic structural receipt.
pub const MAX_CANONICAL_RECEIPT_BYTES_V1: usize =
    MAX_CANONICAL_HANDOFF_BYTES_V1 + MAX_NAME_BYTES_V1 + 256;

/// Whether one typed surface is admitted by the V1 lane.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SupportStatusV1 {
    /// The value has a reviewed V1 lowering.
    Supported,
    /// The value is recognized but rejected by V1.
    Rejected,
}

/// One source operation in the closed scalar-kernel body vocabulary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScalarOperationV1 {
    /// Load one `f32` from the input global pointer.
    LoadInputF32,
    /// Strictly add the scalar `f32` addend.
    AddAddendF32,
    /// Store the computed `f32` through the output global pointer.
    StoreOutputF32,
    /// Return void.
    ReturnVoid,
    /// A recognized but unsupported floating-point multiply.
    MultiplyAddendF32,
    /// A recognized but unsupported function call.
    Call,
    /// A recognized but unsupported control-flow branch.
    Branch,
}

/// Calling-convention request at the source boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceCallingConventionV1 {
    /// AMDGPU kernel calling convention.
    AmdGpuKernel,
    /// Ordinary C calling convention, rejected by this lane.
    C,
}

/// Closed target-feature policy choices recognized at the source boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TargetFeaturePolicyV1 {
    /// Exact gfx942 wave64 policy with wave32 and XNACK disabled.
    Gfx942Wave64XnackMinus,
    /// gfx942 wave32 policy, rejected by this lane.
    Gfx942Wave32XnackMinus,
    /// gfx942 wave64 policy with XNACK enabled, rejected by this lane.
    Gfx942Wave64XnackPlus,
    /// A different processor family, rejected by this lane.
    OtherProcessor,
}

/// One semantic class of function attribute used in diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FunctionAttributeKindV1 {
    /// `nounwind`.
    NoUnwind,
    /// `amdgpu-flat-work-group-size`.
    FlatWorkgroupSize,
    /// `amdgpu-waves-per-eu`.
    WavesPerEu,
    /// Strict f32 denormal handling.
    DenormalFpMathF32Ieee,
    /// Disabled unsafe floating-point math.
    UnsafeFpMathDisabled,
    /// Disabled no-infinities assumption.
    NoInfsFpMathDisabled,
    /// Disabled no-NaNs assumption.
    NoNansFpMathDisabled,
    /// Disabled no-signed-zero assumption.
    NoSignedZerosFpMathDisabled,
    /// Disabled approximate functions.
    ApproxFuncFpMathDisabled,
    /// Disabled floating-point contraction.
    FpContractOff,
}

/// One metadata family used in support queries and diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MetadataKindV1 {
    /// AMDHSA code object version 6 module flag.
    CodeObjectVersion6,
    /// Position-independent-code level 2 module flag.
    PicLevel2,
    /// LLVM `wchar_size` module flag.
    WcharSize4,
    /// Any named metadata record.
    NamedMetadata,
    /// Any device-library input declaration.
    DeviceLibrary,
}

/// One named input field used by typed diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputFieldV1 {
    /// Pliron module name.
    ModuleName,
    /// Kernel symbol.
    KernelSymbol,
    /// Input pointer parameter name.
    InputParameter,
    /// Output pointer parameter name.
    OutputParameter,
    /// Scalar addend parameter name.
    AddendParameter,
}

/// Why a bounded input name was rejected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NameRejectionV1 {
    /// The name was empty.
    Empty,
    /// The name exceeded [`MAX_NAME_BYTES_V1`].
    TooLong,
    /// The first byte was not ASCII alphabetic or underscore.
    InvalidFirstByte,
    /// A later byte was not ASCII alphanumeric or underscore.
    InvalidByte,
}

/// A bounded collection named by a resource-limit diagnostic.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResourceKindV1 {
    /// Source operation list.
    Operations,
    /// Function attribute list.
    FunctionAttributes,
    /// Input parameter attribute list.
    InputParameterAttributes,
    /// Output parameter attribute list.
    OutputParameterAttributes,
    /// Addend parameter attribute list.
    AddendParameterAttributes,
    /// Module flag list.
    ModuleFlags,
    /// Named metadata list.
    NamedMetadata,
    /// Device-library declaration list.
    DeviceLibraries,
    /// Obligation list.
    Obligations,
    /// Canonical structural receipt bytes.
    ReceiptBytes,
}

/// Stage at which construction of the reviewed representation failed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConstructionStageV1 {
    /// Process-local Pliron context provenance creation.
    ContextIdentity,
    /// Canonical handoff model construction.
    CanonicalHandoff,
    /// Recursive Pliron operation verification.
    DialectVerification,
    /// Fe2o3 receipt encoding.
    ReceiptEncoding,
}

/// Stable, typed, bounded failures returned by the V1 lane.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoweringDiagnosticV1 {
    /// A name failed the strict Pliron-compatible V1 grammar.
    InvalidName {
        /// Rejected field.
        field: InputFieldV1,
        /// Typed reason.
        reason: NameRejectionV1,
    },
    /// Two kernel parameter names are equal.
    DuplicateParameterName,
    /// A bounded input collection exceeded its hard cap.
    ResourceLimit {
        /// Limited collection.
        resource: ResourceKindV1,
        /// Observed element count.
        observed: usize,
        /// Maximum accepted count.
        maximum: usize,
    },
    /// A recognized operation is outside the V1 slice.
    UnsupportedOperation(ScalarOperationV1),
    /// Supported operations were not in the one admitted order.
    UnsupportedOperationSequence,
    /// A recognized scalar type is outside the V1 slice.
    UnsupportedType(ScalarTypeV1),
    /// A recognized AMDGPU address space is outside the V1 slice.
    UnsupportedAddressSpace(AddressSpace),
    /// A recognized calling convention is outside the V1 slice.
    UnsupportedCallingConvention(SourceCallingConventionV1),
    /// A recognized target policy is outside the V1 slice.
    UnsupportedTargetPolicy(TargetFeaturePolicyV1),
    /// A function attribute or its payload is outside the V1 slice.
    UnsupportedFunctionAttribute(FunctionAttributeKindV1),
    /// A required function attribute is absent.
    MissingFunctionAttribute(FunctionAttributeKindV1),
    /// A function attribute kind occurs more than once.
    DuplicateFunctionAttribute(FunctionAttributeKindV1),
    /// Parameter attributes are recognized but unsupported in V1.
    UnsupportedParameterAttribute(ParameterAttributeV1),
    /// A metadata family is outside the V1 slice.
    UnsupportedMetadata(MetadataKindV1),
    /// A required module flag is absent.
    MissingModuleFlag(MetadataKindV1),
    /// A module flag occurs more than once.
    DuplicateModuleFlag(MetadataKindV1),
    /// An origin kind or source span is outside the V1 slice.
    UnsupportedOrigin {
        /// Requested origin kind.
        kind: OriginKindV1,
        /// Whether the request carried a source span.
        has_span: bool,
    },
    /// An obligation kind is outside the V1 slice.
    UnsupportedObligation(ObligationKindV1),
    /// A required preservation obligation is absent.
    MissingObligation(ObligationKindV1),
    /// An obligation kind occurs more than once.
    DuplicateObligation(ObligationKindV1),
    /// A reviewed construction stage failed without exposing upstream text.
    ConstructionFailed(ConstructionStageV1),
}

impl fmt::Display for LoweringDiagnosticV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidName { field, reason } => {
                write!(formatter, "invalid {field:?}: {reason:?}")
            }
            Self::DuplicateParameterName => formatter.write_str("duplicate kernel parameter name"),
            Self::ResourceLimit {
                resource,
                observed,
                maximum,
            } => write!(
                formatter,
                "{resource:?} limit exceeded: observed {observed}, maximum {maximum}"
            ),
            Self::UnsupportedOperation(operation) => {
                write!(formatter, "unsupported V1 operation {operation:?}")
            }
            Self::UnsupportedOperationSequence => {
                formatter.write_str("unsupported V1 operation sequence")
            }
            Self::UnsupportedType(value_type) => {
                write!(formatter, "unsupported V1 scalar type {value_type:?}")
            }
            Self::UnsupportedAddressSpace(address_space) => {
                write!(formatter, "unsupported V1 address space {address_space:?}")
            }
            Self::UnsupportedCallingConvention(calling_convention) => write!(
                formatter,
                "unsupported V1 calling convention {calling_convention:?}"
            ),
            Self::UnsupportedTargetPolicy(target) => {
                write!(formatter, "unsupported V1 target policy {target:?}")
            }
            Self::UnsupportedFunctionAttribute(attribute) => {
                write!(formatter, "unsupported V1 function attribute {attribute:?}")
            }
            Self::MissingFunctionAttribute(attribute) => {
                write!(formatter, "missing V1 function attribute {attribute:?}")
            }
            Self::DuplicateFunctionAttribute(attribute) => {
                write!(formatter, "duplicate V1 function attribute {attribute:?}")
            }
            Self::UnsupportedParameterAttribute(attribute) => {
                write!(
                    formatter,
                    "unsupported V1 parameter attribute {attribute:?}"
                )
            }
            Self::UnsupportedMetadata(metadata) => {
                write!(formatter, "unsupported V1 metadata {metadata:?}")
            }
            Self::MissingModuleFlag(flag) => write!(formatter, "missing V1 module flag {flag:?}"),
            Self::DuplicateModuleFlag(flag) => {
                write!(formatter, "duplicate V1 module flag {flag:?}")
            }
            Self::UnsupportedOrigin { kind, has_span } => write!(
                formatter,
                "unsupported V1 origin {kind:?} with source span {has_span}"
            ),
            Self::UnsupportedObligation(obligation) => {
                write!(formatter, "unsupported V1 obligation {obligation:?}")
            }
            Self::MissingObligation(obligation) => {
                write!(formatter, "missing V1 obligation {obligation:?}")
            }
            Self::DuplicateObligation(obligation) => {
                write!(formatter, "duplicate V1 obligation {obligation:?}")
            }
            Self::ConstructionFailed(stage) => {
                write!(formatter, "V1 construction failed at {stage:?}")
            }
        }
    }
}

impl Error for LoweringDiagnosticV1 {}

/// Query object for the complete V1 support and rejection matrix.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SupportMatrixV1;

/// The complete support matrix for this lowering lane.
pub const SUPPORT_MATRIX_V1: SupportMatrixV1 = SupportMatrixV1;

impl SupportMatrixV1 {
    /// Reports operation support. V1 admits only the four explicit body operations.
    pub const fn operation(self, operation: ScalarOperationV1) -> SupportStatusV1 {
        match operation {
            ScalarOperationV1::LoadInputF32
            | ScalarOperationV1::AddAddendF32
            | ScalarOperationV1::StoreOutputF32
            | ScalarOperationV1::ReturnVoid => SupportStatusV1::Supported,
            ScalarOperationV1::MultiplyAddendF32
            | ScalarOperationV1::Call
            | ScalarOperationV1::Branch => SupportStatusV1::Rejected,
        }
    }

    /// Reports scalar-type support. V1 admits only `f32`; aggregate and vector types have no V1 surface.
    pub const fn scalar_type(self, value_type: ScalarTypeV1) -> SupportStatusV1 {
        match value_type {
            ScalarTypeV1::F32 => SupportStatusV1::Supported,
            ScalarTypeV1::I1
            | ScalarTypeV1::I8
            | ScalarTypeV1::I16
            | ScalarTypeV1::I32
            | ScalarTypeV1::I64
            | ScalarTypeV1::F16
            | ScalarTypeV1::Bf16
            | ScalarTypeV1::F64 => SupportStatusV1::Rejected,
        }
    }

    /// Reports AMDGPU address-space support. V1 admits only global address space 1.
    pub const fn address_space(self, address_space: AddressSpace) -> SupportStatusV1 {
        match address_space {
            AddressSpace::Global => SupportStatusV1::Supported,
            AddressSpace::Generic
            | AddressSpace::Region
            | AddressSpace::Local
            | AddressSpace::Constant
            | AddressSpace::Private
            | AddressSpace::Constant32Bit
            | AddressSpace::BufferFatPointer => SupportStatusV1::Rejected,
        }
    }

    /// Reports calling-convention support.
    pub const fn calling_convention(
        self,
        calling_convention: SourceCallingConventionV1,
    ) -> SupportStatusV1 {
        match calling_convention {
            SourceCallingConventionV1::AmdGpuKernel => SupportStatusV1::Supported,
            SourceCallingConventionV1::C => SupportStatusV1::Rejected,
        }
    }

    /// Reports target-feature-policy support.
    pub const fn target_policy(self, target: TargetFeaturePolicyV1) -> SupportStatusV1 {
        match target {
            TargetFeaturePolicyV1::Gfx942Wave64XnackMinus => SupportStatusV1::Supported,
            TargetFeaturePolicyV1::Gfx942Wave32XnackMinus
            | TargetFeaturePolicyV1::Gfx942Wave64XnackPlus
            | TargetFeaturePolicyV1::OtherProcessor => SupportStatusV1::Rejected,
        }
    }

    /// Reports function-attribute support, including the exact workgroup-size payload.
    pub fn function_attribute(self, attribute: FunctionAttributeV1) -> SupportStatusV1 {
        match attribute {
            FunctionAttributeV1::NoUnwind
            | FunctionAttributeV1::DenormalFpMathF32Ieee
            | FunctionAttributeV1::UnsafeFpMathDisabled
            | FunctionAttributeV1::NoInfsFpMathDisabled
            | FunctionAttributeV1::NoNansFpMathDisabled
            | FunctionAttributeV1::NoSignedZerosFpMathDisabled
            | FunctionAttributeV1::ApproxFuncFpMathDisabled
            | FunctionAttributeV1::FpContractOff => SupportStatusV1::Supported,
            FunctionAttributeV1::FlatWorkgroupSize(range)
                if range.minimum() == 64 && range.maximum() == 64 =>
            {
                SupportStatusV1::Supported
            }
            FunctionAttributeV1::FlatWorkgroupSize(_) | FunctionAttributeV1::WavesPerEu(_) => {
                SupportStatusV1::Rejected
            }
        }
    }

    /// Reports parameter-attribute support. V1 rejects every parameter attribute.
    pub const fn parameter_attribute(self, _attribute: ParameterAttributeV1) -> SupportStatusV1 {
        SupportStatusV1::Rejected
    }

    /// Reports module-flag support. Code object V6 and PIC level 2 are required.
    pub const fn module_flag(self, flag: ModuleFlagV1) -> SupportStatusV1 {
        match flag {
            ModuleFlagV1::CodeObjectVersion6 | ModuleFlagV1::PicLevel2 => {
                SupportStatusV1::Supported
            }
            ModuleFlagV1::WcharSize4 => SupportStatusV1::Rejected,
        }
    }

    /// Reports named-metadata support. V1 rejects all named metadata.
    pub const fn named_metadata(self, _metadata: NamedMetadataV1) -> SupportStatusV1 {
        SupportStatusV1::Rejected
    }

    /// Reports device-library declaration support. V1 rejects all device libraries.
    pub const fn device_library(self, _library: DeviceLibraryInputV1) -> SupportStatusV1 {
        SupportStatusV1::Rejected
    }

    /// Reports origin support. V1 admits one span-free AMDGCN IR origin.
    pub const fn origin(self, kind: OriginKindV1, has_span: bool) -> SupportStatusV1 {
        if matches!(kind, OriginKindV1::AmdgcnIr) && !has_span {
            SupportStatusV1::Supported
        } else {
            SupportStatusV1::Rejected
        }
    }

    /// Reports obligation support. Device-library authentication is excluded because libraries are rejected.
    pub const fn obligation(self, obligation: ObligationKindV1) -> SupportStatusV1 {
        match obligation {
            ObligationKindV1::PreserveKernelAbi
            | ObligationKindV1::PreserveAddressSpaces
            | ObligationKindV1::PreserveTargetFeatures
            | ObligationKindV1::PreserveCallingConvention
            | ObligationKindV1::PreserveFunctionAttributes
            | ObligationKindV1::PreserveModuleMetadata
            | ObligationKindV1::MaintainOriginCoverage => SupportStatusV1::Supported,
            ObligationKindV1::AuthenticateDeviceLibraries => SupportStatusV1::Rejected,
        }
    }
}

/// One closed source module request for the first typed lowering slice.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScalarKernelModuleV1 {
    /// Pliron module name. The accepted grammar is `[A-Za-z_][A-Za-z0-9_]*`.
    pub module_name: String,
    /// Kernel symbol using the same strict name grammar.
    pub kernel_symbol: String,
    /// Input pointer parameter name.
    pub input_parameter: String,
    /// Output pointer parameter name.
    pub output_parameter: String,
    /// Scalar addend parameter name.
    pub addend_parameter: String,
    /// Closed operation sequence.
    pub operations: Vec<ScalarOperationV1>,
    /// Scalar element type.
    pub scalar_type: ScalarTypeV1,
    /// AMDGPU pointer address space for both pointers.
    pub address_space: AddressSpace,
    /// Requested source calling convention.
    pub calling_convention: SourceCallingConventionV1,
    /// Requested exact target policy.
    pub target_policy: TargetFeaturePolicyV1,
    /// Kernel function attributes retained in the canonical handoff.
    pub function_attributes: Vec<FunctionAttributeV1>,
    /// Input pointer parameter attributes.
    pub input_attributes: Vec<ParameterAttributeV1>,
    /// Output pointer parameter attributes.
    pub output_attributes: Vec<ParameterAttributeV1>,
    /// Scalar addend parameter attributes.
    pub addend_attributes: Vec<ParameterAttributeV1>,
    /// LLVM module flags retained in the canonical handoff.
    pub module_flags: Vec<ModuleFlagV1>,
    /// Named metadata retained in the canonical handoff.
    pub named_metadata: Vec<NamedMetadataV1>,
    /// Device-library declarations retained in the canonical handoff.
    pub device_libraries: Vec<DeviceLibraryInputV1>,
    /// Origin kind for the complete source module.
    pub origin_kind: OriginKindV1,
    /// Stable identity from which the canonical origin identity is derived.
    pub origin_source_identity: IdentityV1,
    /// Optional source span for the origin.
    pub origin_span: Option<SourceSpanV1>,
    /// Required preservation obligations; identities are derived canonically.
    pub obligations: Vec<ObligationKindV1>,
    /// Pre-existing semantic, schedule, and target-plan identities.
    pub stage_identities: StageIdentitiesV1,
}

impl ScalarKernelModuleV1 {
    /// Creates the exact admitted request with caller-owned canonical identities.
    pub fn canonical(
        module_name: impl Into<String>,
        kernel_symbol: impl Into<String>,
        origin_source_identity: IdentityV1,
        stage_identities: StageIdentitiesV1,
    ) -> Self {
        let workgroup =
            WorkgroupSizeRangeV1::new(64, 64).expect("the static wave64 workgroup range is valid");
        Self {
            module_name: module_name.into(),
            kernel_symbol: kernel_symbol.into(),
            input_parameter: "input".to_owned(),
            output_parameter: "output".to_owned(),
            addend_parameter: "addend".to_owned(),
            operations: admitted_operations_v1().to_vec(),
            scalar_type: ScalarTypeV1::F32,
            address_space: AddressSpace::Global,
            calling_convention: SourceCallingConventionV1::AmdGpuKernel,
            target_policy: TargetFeaturePolicyV1::Gfx942Wave64XnackMinus,
            function_attributes: FunctionAttributeV1::gfx942_kernel_defaults(workgroup),
            input_attributes: Vec::new(),
            output_attributes: Vec::new(),
            addend_attributes: Vec::new(),
            module_flags: vec![ModuleFlagV1::CodeObjectVersion6, ModuleFlagV1::PicLevel2],
            named_metadata: Vec::new(),
            device_libraries: Vec::new(),
            origin_kind: OriginKindV1::AmdgcnIr,
            origin_source_identity,
            origin_span: None,
            obligations: admitted_obligations_v1().to_vec(),
            stage_identities,
        }
    }
}

/// Returns the one admitted operation sequence in canonical order.
pub const fn admitted_operations_v1() -> &'static [ScalarOperationV1; 4] {
    &[
        ScalarOperationV1::LoadInputF32,
        ScalarOperationV1::AddAddendF32,
        ScalarOperationV1::StoreOutputF32,
        ScalarOperationV1::ReturnVoid,
    ]
}

/// Returns every required preservation obligation in canonical source order.
pub const fn admitted_obligations_v1() -> &'static [ObligationKindV1; 7] {
    &[
        ObligationKindV1::PreserveKernelAbi,
        ObligationKindV1::PreserveAddressSpaces,
        ObligationKindV1::PreserveTargetFeatures,
        ObligationKindV1::PreserveCallingConvention,
        ObligationKindV1::PreserveFunctionAttributes,
        ObligationKindV1::PreserveModuleMetadata,
        ObligationKindV1::MaintainOriginCoverage,
    ]
}

/// Semantic operation kinds committed by a successful receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VerifiedDialectOperationV1 {
    /// `llvm.func`.
    Func,
    /// `llvm.load`.
    Load,
    /// `llvm.fadd` with empty fast-math flags.
    FAdd,
    /// `llvm.store`.
    Store,
    /// `llvm.return`.
    Return,
}

/// Closed inventory of real dialect operations constructed by V1.
pub const VERIFIED_DIALECT_OPERATIONS_V1: [VerifiedDialectOperationV1; 5] = [
    VerifiedDialectOperationV1::Func,
    VerifiedDialectOperationV1::Load,
    VerifiedDialectOperationV1::FAdd,
    VerifiedDialectOperationV1::Store,
    VerifiedDialectOperationV1::Return,
];

/// Body-only operation sequence required by the V1 dialect module.
pub const VERIFIED_DIALECT_BODY_OPERATIONS_V1: [VerifiedDialectOperationV1; 4] = [
    VerifiedDialectOperationV1::Load,
    VerifiedDialectOperationV1::FAdd,
    VerifiedDialectOperationV1::Store,
    VerifiedDialectOperationV1::Return,
];

/// One typed function-argument fact recovered from the live dialect module.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DialectArgumentInspectionV1 {
    /// An opaque LLVM pointer carrying its exact numeric address space.
    OpaquePointer {
        /// LLVM address-space number recovered from the pointer type.
        address_space: u32,
    },
    /// A builtin 32-bit floating-point scalar.
    F32,
}

/// Owner-checked facts recovered from the live V1 Pliron LLVM module.
///
/// This value contains no Pliron context, pointer, operation wrapper, printer
/// text, or process-local identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DialectModuleInspectionV1 {
    pub(crate) function_count: u8,
    pub(crate) function_operation: VerifiedDialectOperationV1,
    pub(crate) returns_void: bool,
    pub(crate) arguments: [DialectArgumentInspectionV1; 3],
    pub(crate) body_operations: [VerifiedDialectOperationV1; 4],
    pub(crate) strict_fast_math: bool,
}

impl DialectModuleInspectionV1 {
    /// Returns the number of top-level LLVM functions.
    pub const fn function_count(self) -> u8 {
        self.function_count
    }

    /// Returns the typed top-level function operation kind.
    pub const fn function_operation(self) -> VerifiedDialectOperationV1 {
        self.function_operation
    }

    /// Returns whether the function has the LLVM void return type.
    pub const fn returns_void(self) -> bool {
        self.returns_void
    }

    /// Returns the exact fixed argument facts in ABI order.
    pub const fn arguments(self) -> [DialectArgumentInspectionV1; 3] {
        self.arguments
    }

    /// Returns the exact body operation facts in execution order.
    pub const fn body_operations(self) -> [VerifiedDialectOperationV1; 4] {
        self.body_operations
    }

    /// Returns whether the `llvm.fadd` carries empty fast-math flags.
    pub const fn strict_fast_math(self) -> bool {
        self.strict_fast_math
    }
}

/// Bounded failure from owner-checked live dialect-module inspection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DialectModuleInspectionErrorV1 {
    /// The retained context identity is absent, corrupt, or changed.
    ContextIdentityInvalid,
    /// The private module handle belongs to a different context owner.
    ForeignOwner,
    /// The private module arena entry is no longer live.
    StaleModule,
    /// Recursive Pliron verification rejected the live module.
    DialectVerificationFailed,
    /// The live module does not have the closed V1 typed shape.
    UnexpectedModuleShape,
    /// An upstream Pliron access panicked and was contained.
    UpstreamPanicked,
}

impl fmt::Display for DialectModuleInspectionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ContextIdentityInvalid => {
                formatter.write_str("dialect inspection context identity is invalid")
            }
            Self::ForeignOwner => {
                formatter.write_str("dialect module belongs to a different context owner")
            }
            Self::StaleModule => formatter.write_str("dialect module handle is stale"),
            Self::DialectVerificationFailed => {
                formatter.write_str("dialect module verification failed")
            }
            Self::UnexpectedModuleShape => {
                formatter.write_str("dialect module has an unexpected typed shape")
            }
            Self::UpstreamPanicked => {
                formatter.write_str("dialect module inspection contained an upstream panic")
            }
        }
    }
}

impl Error for DialectModuleInspectionErrorV1 {}

/// Bounded failure from extracting the closed live Pliron graph into LLVM handoff V2.
///
/// Diagnostics classify the rejected semantic dimension without exposing
/// upstream diagnostics, printer text, arena handles, or hostile input data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HandoffExtractionDiagnosticV2 {
    /// The retained context identity is absent, corrupt, or changed.
    ContextIdentityInvalid,
    /// The private module handle belongs to a different context owner.
    ForeignOwner,
    /// The private module arena entry is no longer live.
    StaleModule,
    /// Recursive Pliron verification rejected the live module.
    DialectVerificationFailed,
    /// The module or kernel symbol does not match the committed V1 boundary.
    SymbolMismatch,
    /// The live graph has an unexpected operation, region, block, or attribute shape.
    OperationShapeMismatch,
    /// The live function signature or one SSA value has an unexpected type.
    TypeMismatch,
    /// A live pointer carries an unexpected AMDGPU address space.
    AddressSpaceMismatch,
    /// A live operand does not reference the one admitted definition.
    DefUseMismatch,
    /// A live basic block or terminator carries an unexpected CFG edge.
    ControlFlowMismatch,
    /// A live memory operation does not carry explicit four-byte alignment.
    AlignmentMismatch,
    /// The floating-point add does not carry the exact strict FP policy.
    StrictFpMismatch,
    /// V1 origins, obligations, metadata, or their receipt commitment do not match.
    EvidenceMismatch,
    /// Construction of the checked V2 handoff rejected the extracted facts.
    HandoffConstructionFailed,
    /// An upstream Pliron access panicked and was contained.
    UpstreamPanicked,
}

impl fmt::Display for HandoffExtractionDiagnosticV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::ContextIdentityInvalid => "V2 extraction context identity is invalid",
            Self::ForeignOwner => "V2 extraction module has a foreign owner",
            Self::StaleModule => "V2 extraction module handle is stale",
            Self::DialectVerificationFailed => "V2 extraction dialect verification failed",
            Self::SymbolMismatch => "V2 extraction symbol mismatch",
            Self::OperationShapeMismatch => "V2 extraction operation shape mismatch",
            Self::TypeMismatch => "V2 extraction type mismatch",
            Self::AddressSpaceMismatch => "V2 extraction address-space mismatch",
            Self::DefUseMismatch => "V2 extraction def-use mismatch",
            Self::ControlFlowMismatch => "V2 extraction control-flow mismatch",
            Self::AlignmentMismatch => "V2 extraction alignment mismatch",
            Self::StrictFpMismatch => "V2 extraction strict floating-point mismatch",
            Self::EvidenceMismatch => "V2 extraction evidence mismatch",
            Self::HandoffConstructionFailed => "V2 handoff construction failed",
            Self::UpstreamPanicked => "V2 extraction contained an upstream panic",
        };
        formatter.write_str(message)
    }
}

impl Error for HandoffExtractionDiagnosticV2 {}

/// Failure from the additive scalar-kernel-to-handoff-V2 boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScalarKernelHandoffDiagnosticV2 {
    /// The source request was rejected by the preserved V1 lowering contract.
    Lowering(LoweringDiagnosticV1),
    /// The constructed private live graph failed exact V2 extraction.
    Extraction(HandoffExtractionDiagnosticV2),
}

impl fmt::Display for ScalarKernelHandoffDiagnosticV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lowering(error) => write!(formatter, "V2 boundary lowering failed: {error}"),
            Self::Extraction(error) => write!(formatter, "V2 boundary extraction failed: {error}"),
        }
    }
}

impl Error for ScalarKernelHandoffDiagnosticV2 {}

/// Fe2o3-owned canonical structural receipt bytes.
///
/// The bytes commit the module name, reviewed dialect operation inventory, and
/// complete canonical handoff. They are deterministic across fresh Pliron
/// contexts and contain no printer output or process-local pointer identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalLoweringReceiptV1 {
    pub(crate) bytes: Vec<u8>,
}

impl CanonicalLoweringReceiptV1 {
    /// Returns the complete canonical receipt encoding.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Returns the canonical receipt byte length.
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Returns whether the receipt is empty, which is always false for constructed receipts.
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
}

pub(crate) fn function_attribute_kind(attribute: FunctionAttributeV1) -> FunctionAttributeKindV1 {
    match attribute {
        FunctionAttributeV1::NoUnwind => FunctionAttributeKindV1::NoUnwind,
        FunctionAttributeV1::FlatWorkgroupSize(_) => FunctionAttributeKindV1::FlatWorkgroupSize,
        FunctionAttributeV1::WavesPerEu(_) => FunctionAttributeKindV1::WavesPerEu,
        FunctionAttributeV1::DenormalFpMathF32Ieee => {
            FunctionAttributeKindV1::DenormalFpMathF32Ieee
        }
        FunctionAttributeV1::UnsafeFpMathDisabled => FunctionAttributeKindV1::UnsafeFpMathDisabled,
        FunctionAttributeV1::NoInfsFpMathDisabled => FunctionAttributeKindV1::NoInfsFpMathDisabled,
        FunctionAttributeV1::NoNansFpMathDisabled => FunctionAttributeKindV1::NoNansFpMathDisabled,
        FunctionAttributeV1::NoSignedZerosFpMathDisabled => {
            FunctionAttributeKindV1::NoSignedZerosFpMathDisabled
        }
        FunctionAttributeV1::ApproxFuncFpMathDisabled => {
            FunctionAttributeKindV1::ApproxFuncFpMathDisabled
        }
        FunctionAttributeV1::FpContractOff => FunctionAttributeKindV1::FpContractOff,
    }
}

pub(crate) const fn metadata_kind(flag: ModuleFlagV1) -> MetadataKindV1 {
    match flag {
        ModuleFlagV1::CodeObjectVersion6 => MetadataKindV1::CodeObjectVersion6,
        ModuleFlagV1::PicLevel2 => MetadataKindV1::PicLevel2,
        ModuleFlagV1::WcharSize4 => MetadataKindV1::WcharSize4,
    }
}
