use std::panic::{AssertUnwindSafe, catch_unwind};

use fe2o3_amdgcn_model::{AMDGPU_TRIPLE, AddressSpace};
use fe2o3_llvm_handoff::{
    AddressSpaceV1, FunctionAttributeV1, GFX942_AMDHSA_TARGET_TRIPLE_V1, Gfx942HandoffInputV1,
    Gfx942HandoffV1, Gfx942HandoffV2, Gfx942TargetPolicyV1, IdentityV1, KernelEntryV1,
    KernelParameterV1, KernelValueTypeV1, ModuleFlagV1, ModuleMetadataV1, ObligationKindV1,
    ObligationV1, OriginV1, ScalarTypeV1,
};
use fe2o3_pliron::{ContextIdentity, ensure_context_identity, require_context_identity};
use pliron::{
    builtin::{
        op_interfaces::{OneResultInterface, SingleBlockRegionInterface},
        ops::ModuleOp,
        types::FP32Type,
    },
    context::Context,
    identifier::Identifier,
    linked_list::ContainsLinkedList,
    op::Op,
    operation::{Operation, verify_operation},
    r#type::Typed,
};
use pliron_llvm::{
    attributes::FastmathFlagsAttr,
    op_interfaces::{AlignableOpInterface, FastMathFlags, FloatBinArithOpWithFastMathFlags},
    ops::{FAddOp, FuncOp, LoadOp, ReturnOp, StoreOp},
    types::{FuncType, PointerType, VoidType},
};

use crate::model::{
    CanonicalLoweringReceiptV1, ConstructionStageV1, DialectArgumentInspectionV1,
    DialectModuleInspectionErrorV1, DialectModuleInspectionV1, FunctionAttributeKindV1,
    HandoffExtractionDiagnosticV2, InputFieldV1, LoweringDiagnosticV1,
    MAX_CANONICAL_RECEIPT_BYTES_V1, MAX_DEVICE_LIBRARIES_V1, MAX_FUNCTION_ATTRIBUTES_V1,
    MAX_MODULE_FLAGS_V1, MAX_NAME_BYTES_V1, MAX_NAMED_METADATA_V1, MAX_OBLIGATIONS_V1,
    MAX_OPERATIONS_V1, MAX_PARAMETER_ATTRIBUTES_V1, MetadataKindV1, NameRejectionV1,
    ResourceKindV1, SUPPORT_MATRIX_V1, ScalarKernelHandoffDiagnosticV2, ScalarKernelModuleV1,
    SupportStatusV1, VERIFIED_DIALECT_BODY_OPERATIONS_V1, VERIFIED_DIALECT_OPERATIONS_V1,
    VerifiedDialectOperationV1, admitted_obligations_v1, admitted_operations_v1,
    function_attribute_kind, metadata_kind,
};

const RECEIPT_MAGIC_V1: &[u8] = b"fe2o3.amdgcn-pliron-llvm.receipt.v1\0";

const REQUIRED_FUNCTION_ATTRIBUTE_KINDS_V1: [FunctionAttributeKindV1; 9] = [
    FunctionAttributeKindV1::NoUnwind,
    FunctionAttributeKindV1::FlatWorkgroupSize,
    FunctionAttributeKindV1::DenormalFpMathF32Ieee,
    FunctionAttributeKindV1::UnsafeFpMathDisabled,
    FunctionAttributeKindV1::NoInfsFpMathDisabled,
    FunctionAttributeKindV1::NoNansFpMathDisabled,
    FunctionAttributeKindV1::NoSignedZerosFpMathDisabled,
    FunctionAttributeKindV1::ApproxFuncFpMathDisabled,
    FunctionAttributeKindV1::FpContractOff,
];

const REQUIRED_MODULE_FLAGS_V1: [ModuleFlagV1; 2] =
    [ModuleFlagV1::CodeObjectVersion6, ModuleFlagV1::PicLevel2];

#[derive(Clone)]
struct OwnedDialectModuleV1 {
    owner: ContextIdentity,
    module: ModuleOp,
}

/// One privately owned verified Pliron LLVM tree and its authoritative canonical handoff.
///
/// Raw Pliron context and arena handles never cross this crate boundary.
/// Durable comparison must use [`Self::receipt`] or [`Self::handoff`].
pub struct LoweredScalarKernelV1 {
    context: Context,
    module: OwnedDialectModuleV1,
    context_identity: ContextIdentity,
    module_name: String,
    handoff: Gfx942HandoffV1,
    receipt: CanonicalLoweringReceiptV1,
}

impl LoweredScalarKernelV1 {
    /// Returns non-durable process-local provenance for the owning context.
    pub const fn context_identity(&self) -> ContextIdentity {
        self.context_identity
    }

    /// Returns the canonical target, ABI, metadata, origin, and obligation handoff.
    pub const fn handoff(&self) -> &Gfx942HandoffV1 {
        &self.handoff
    }

    /// Returns deterministic fe2o3-owned structural receipt bytes.
    pub const fn receipt(&self) -> &CanonicalLoweringReceiptV1 {
        &self.receipt
    }

    /// Returns the exact dialect operation inventory committed by the receipt.
    pub const fn operation_inventory(&self) -> &'static [VerifiedDialectOperationV1; 5] {
        &VERIFIED_DIALECT_OPERATIONS_V1
    }

    /// Extracts the exact private live graph into a validated typed LLVM handoff V2.
    ///
    /// The traversal revalidates owner identity, liveness, recursive Pliron
    /// verification, symbols, types, address spaces, alignment, strict FP,
    /// def-use wiring, CFG edges, and the committed V1 evidence. Every upstream
    /// access is panic-contained, and no raw Pliron object crosses this boundary.
    pub fn extract_handoff_v2(&self) -> Result<Gfx942HandoffV2, HandoffExtractionDiagnosticV2> {
        catch_unwind(AssertUnwindSafe(|| {
            crate::extract_v2::extract_handoff_v2(
                &self.context,
                self.context_identity,
                self.module.owner,
                &self.module.module,
                &self.module_name,
                &self.handoff,
                &self.receipt,
            )
        }))
        .unwrap_or(Err(HandoffExtractionDiagnosticV2::UpstreamPanicked))
    }

    /// Revalidates private owner identity, arena liveness, recursive dialect
    /// verification, and the complete closed V1 shape before returning typed
    /// inspection facts.
    ///
    /// Every upstream access is panic-contained. No context, pointer,
    /// operation wrapper, or printer text is returned.
    pub fn inspect_dialect_module(
        &self,
    ) -> Result<DialectModuleInspectionV1, DialectModuleInspectionErrorV1> {
        catch_unwind(AssertUnwindSafe(|| self.inspect_dialect_module_inner()))
            .unwrap_or(Err(DialectModuleInspectionErrorV1::UpstreamPanicked))
    }

    fn inspect_dialect_module_inner(
        &self,
    ) -> Result<DialectModuleInspectionV1, DialectModuleInspectionErrorV1> {
        let current = require_context_identity(&self.context)
            .map_err(|_| DialectModuleInspectionErrorV1::ContextIdentityInvalid)?;
        if current != self.context_identity {
            return Err(DialectModuleInspectionErrorV1::ContextIdentityInvalid);
        }
        if self.module.owner != current {
            return Err(DialectModuleInspectionErrorV1::ForeignOwner);
        }

        let module_pointer = self.module.module.get_operation();
        let module_ref = module_pointer
            .try_deref(&self.context)
            .map_err(|_| DialectModuleInspectionErrorV1::StaleModule)?;
        drop(module_ref);
        verify_operation(module_pointer, &self.context)
            .map_err(|_| DialectModuleInspectionErrorV1::DialectVerificationFailed)?;
        inspect_live_module(&self.context, &self.module.module)
    }
}

/// Validates, constructs, and recursively verifies the first typed gfx942 scalar kernel slice.
///
/// No LLVM C API, compiler subprocess, linker, runtime API, or publication
/// authority is used or granted.
pub fn lower_scalar_kernel_v1(
    input: &ScalarKernelModuleV1,
) -> Result<LoweredScalarKernelV1, LoweringDiagnosticV1> {
    validate_input(input)?;
    let handoff = build_handoff(input)?;
    let receipt = encode_receipt(&input.module_name, &handoff)?;

    let mut context = Context::new();
    let context_identity = ensure_context_identity(&mut context).map_err(|_| {
        LoweringDiagnosticV1::ConstructionFailed(ConstructionStageV1::ContextIdentity)
    })?;
    let module = build_dialect_module(&mut context, input)?;
    verify_operation(module.get_operation(), &context).map_err(|_| {
        LoweringDiagnosticV1::ConstructionFailed(ConstructionStageV1::DialectVerification)
    })?;

    Ok(LoweredScalarKernelV1 {
        context,
        module: OwnedDialectModuleV1 {
            owner: context_identity,
            module,
        },
        context_identity,
        module_name: input.module_name.clone(),
        handoff,
        receipt,
    })
}

/// Lowers through the private live Pliron graph and returns only typed LLVM handoff V2.
///
/// This additive boundary preserves [`lower_scalar_kernel_v1`] while making V2
/// the complete semantic result. The temporary Pliron context and arena handles
/// are dropped before this function returns.
pub fn lower_scalar_kernel_v2(
    input: &ScalarKernelModuleV1,
) -> Result<Gfx942HandoffV2, ScalarKernelHandoffDiagnosticV2> {
    let lowered =
        lower_scalar_kernel_v1(input).map_err(ScalarKernelHandoffDiagnosticV2::Lowering)?;
    lowered
        .extract_handoff_v2()
        .map_err(ScalarKernelHandoffDiagnosticV2::Extraction)
}

fn inspect_live_module(
    context: &Context,
    module: &ModuleOp,
) -> Result<DialectModuleInspectionV1, DialectModuleInspectionErrorV1> {
    let module_body = module.get_body(context, 0);
    let module_operations = module_body.deref(context).iter(context).collect::<Vec<_>>();
    let [function_pointer] = module_operations.as_slice() else {
        return Err(DialectModuleInspectionErrorV1::UnexpectedModuleShape);
    };
    let function = Operation::get_op::<FuncOp>(*function_pointer, context)
        .ok_or(DialectModuleInspectionErrorV1::UnexpectedModuleShape)?;
    let function_type = function.get_type(context);
    let result_type = function_type.deref(context).result_type();
    let returns_void = result_type
        .deref(context)
        .downcast_ref::<VoidType>()
        .is_some();
    let entry = function
        .get_entry_block(context)
        .ok_or(DialectModuleInspectionErrorV1::UnexpectedModuleShape)?;
    let arguments = entry.deref(context).arguments().collect::<Vec<_>>();
    let [input, output, addend] = arguments.as_slice() else {
        return Err(DialectModuleInspectionErrorV1::UnexpectedModuleShape);
    };
    let arguments = [
        inspect_argument(context, *input)?,
        inspect_argument(context, *output)?,
        inspect_argument(context, *addend)?,
    ];
    let expected_arguments = [
        DialectArgumentInspectionV1::OpaquePointer {
            address_space: AddressSpace::Global.llvm_id(),
        },
        DialectArgumentInspectionV1::OpaquePointer {
            address_space: AddressSpace::Global.llvm_id(),
        },
        DialectArgumentInspectionV1::F32,
    ];
    if !returns_void || arguments != expected_arguments {
        return Err(DialectModuleInspectionErrorV1::UnexpectedModuleShape);
    }

    let body = entry.deref(context).iter(context).collect::<Vec<_>>();
    let [load, add, store, return_op] = body.as_slice() else {
        return Err(DialectModuleInspectionErrorV1::UnexpectedModuleShape);
    };
    if Operation::get_op::<LoadOp>(*load, context).is_none()
        || Operation::get_op::<StoreOp>(*store, context).is_none()
        || Operation::get_op::<ReturnOp>(*return_op, context).is_none()
    {
        return Err(DialectModuleInspectionErrorV1::UnexpectedModuleShape);
    }
    let add = Operation::get_op::<FAddOp>(*add, context)
        .ok_or(DialectModuleInspectionErrorV1::UnexpectedModuleShape)?;
    let strict_fast_math = add.fast_math_flags(context) == FastmathFlagsAttr::default();

    Ok(DialectModuleInspectionV1 {
        function_count: 1,
        function_operation: VerifiedDialectOperationV1::Func,
        returns_void,
        arguments,
        body_operations: VERIFIED_DIALECT_BODY_OPERATIONS_V1,
        strict_fast_math,
    })
}

fn inspect_argument(
    context: &Context,
    argument: pliron::value::Value,
) -> Result<DialectArgumentInspectionV1, DialectModuleInspectionErrorV1> {
    let value_type = argument.get_type(context);
    let value_type = value_type.deref(context);
    if let Some(pointer) = value_type.downcast_ref::<PointerType>() {
        return Ok(DialectArgumentInspectionV1::OpaquePointer {
            address_space: pointer.address_space(),
        });
    }
    if value_type.downcast_ref::<FP32Type>().is_some() {
        return Ok(DialectArgumentInspectionV1::F32);
    }
    Err(DialectModuleInspectionErrorV1::UnexpectedModuleShape)
}

fn validate_input(input: &ScalarKernelModuleV1) -> Result<(), LoweringDiagnosticV1> {
    validate_name(&input.module_name, InputFieldV1::ModuleName)?;
    validate_name(&input.kernel_symbol, InputFieldV1::KernelSymbol)?;
    validate_name(&input.input_parameter, InputFieldV1::InputParameter)?;
    validate_name(&input.output_parameter, InputFieldV1::OutputParameter)?;
    validate_name(&input.addend_parameter, InputFieldV1::AddendParameter)?;
    if input.input_parameter == input.output_parameter
        || input.input_parameter == input.addend_parameter
        || input.output_parameter == input.addend_parameter
    {
        return Err(LoweringDiagnosticV1::DuplicateParameterName);
    }

    check_limit(
        ResourceKindV1::Operations,
        input.operations.len(),
        MAX_OPERATIONS_V1,
    )?;
    for operation in &input.operations {
        if SUPPORT_MATRIX_V1.operation(*operation) == SupportStatusV1::Rejected {
            return Err(LoweringDiagnosticV1::UnsupportedOperation(*operation));
        }
    }
    if input.operations.as_slice() != admitted_operations_v1() {
        return Err(LoweringDiagnosticV1::UnsupportedOperationSequence);
    }

    if SUPPORT_MATRIX_V1.scalar_type(input.scalar_type) == SupportStatusV1::Rejected {
        return Err(LoweringDiagnosticV1::UnsupportedType(input.scalar_type));
    }
    if SUPPORT_MATRIX_V1.address_space(input.address_space) == SupportStatusV1::Rejected {
        return Err(LoweringDiagnosticV1::UnsupportedAddressSpace(
            input.address_space,
        ));
    }
    if SUPPORT_MATRIX_V1.calling_convention(input.calling_convention) == SupportStatusV1::Rejected {
        return Err(LoweringDiagnosticV1::UnsupportedCallingConvention(
            input.calling_convention,
        ));
    }
    if SUPPORT_MATRIX_V1.target_policy(input.target_policy) == SupportStatusV1::Rejected {
        return Err(LoweringDiagnosticV1::UnsupportedTargetPolicy(
            input.target_policy,
        ));
    }

    validate_function_attributes(&input.function_attributes)?;
    validate_parameter_attributes(
        &input.input_attributes,
        ResourceKindV1::InputParameterAttributes,
    )?;
    validate_parameter_attributes(
        &input.output_attributes,
        ResourceKindV1::OutputParameterAttributes,
    )?;
    validate_parameter_attributes(
        &input.addend_attributes,
        ResourceKindV1::AddendParameterAttributes,
    )?;
    validate_metadata(input)?;

    let has_span = input.origin_span.is_some();
    if SUPPORT_MATRIX_V1.origin(input.origin_kind, has_span) == SupportStatusV1::Rejected {
        return Err(LoweringDiagnosticV1::UnsupportedOrigin {
            kind: input.origin_kind,
            has_span,
        });
    }
    validate_obligations(&input.obligations)?;
    Ok(())
}

fn validate_name(value: &str, field: InputFieldV1) -> Result<(), LoweringDiagnosticV1> {
    if value.is_empty() {
        return Err(LoweringDiagnosticV1::InvalidName {
            field,
            reason: NameRejectionV1::Empty,
        });
    }
    if value.len() > MAX_NAME_BYTES_V1 {
        return Err(LoweringDiagnosticV1::InvalidName {
            field,
            reason: NameRejectionV1::TooLong,
        });
    }
    let mut bytes = value.bytes();
    let first = bytes.next().expect("non-empty name has a first byte");
    if !(first.is_ascii_alphabetic() || first == b'_') {
        return Err(LoweringDiagnosticV1::InvalidName {
            field,
            reason: NameRejectionV1::InvalidFirstByte,
        });
    }
    if !bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_') {
        return Err(LoweringDiagnosticV1::InvalidName {
            field,
            reason: NameRejectionV1::InvalidByte,
        });
    }
    Ok(())
}

fn check_limit(
    resource: ResourceKindV1,
    observed: usize,
    maximum: usize,
) -> Result<(), LoweringDiagnosticV1> {
    if observed > maximum {
        Err(LoweringDiagnosticV1::ResourceLimit {
            resource,
            observed,
            maximum,
        })
    } else {
        Ok(())
    }
}

fn validate_function_attributes(
    attributes: &[FunctionAttributeV1],
) -> Result<(), LoweringDiagnosticV1> {
    check_limit(
        ResourceKindV1::FunctionAttributes,
        attributes.len(),
        MAX_FUNCTION_ATTRIBUTES_V1,
    )?;
    for (index, attribute) in attributes.iter().copied().enumerate() {
        let kind = function_attribute_kind(attribute);
        if SUPPORT_MATRIX_V1.function_attribute(attribute) == SupportStatusV1::Rejected {
            return Err(LoweringDiagnosticV1::UnsupportedFunctionAttribute(kind));
        }
        if attributes[..index]
            .iter()
            .copied()
            .any(|prior| function_attribute_kind(prior) == kind)
        {
            return Err(LoweringDiagnosticV1::DuplicateFunctionAttribute(kind));
        }
    }
    for required in REQUIRED_FUNCTION_ATTRIBUTE_KINDS_V1 {
        if !attributes
            .iter()
            .copied()
            .any(|attribute| function_attribute_kind(attribute) == required)
        {
            return Err(LoweringDiagnosticV1::MissingFunctionAttribute(required));
        }
    }
    Ok(())
}

fn validate_parameter_attributes(
    attributes: &[fe2o3_llvm_handoff::ParameterAttributeV1],
    resource: ResourceKindV1,
) -> Result<(), LoweringDiagnosticV1> {
    check_limit(resource, attributes.len(), MAX_PARAMETER_ATTRIBUTES_V1)?;
    if let Some(attribute) = attributes.first() {
        return Err(LoweringDiagnosticV1::UnsupportedParameterAttribute(
            *attribute,
        ));
    }
    Ok(())
}

fn validate_metadata(input: &ScalarKernelModuleV1) -> Result<(), LoweringDiagnosticV1> {
    check_limit(
        ResourceKindV1::ModuleFlags,
        input.module_flags.len(),
        MAX_MODULE_FLAGS_V1,
    )?;
    for (index, flag) in input.module_flags.iter().copied().enumerate() {
        let kind = metadata_kind(flag);
        if SUPPORT_MATRIX_V1.module_flag(flag) == SupportStatusV1::Rejected {
            return Err(LoweringDiagnosticV1::UnsupportedMetadata(kind));
        }
        if input.module_flags[..index].contains(&flag) {
            return Err(LoweringDiagnosticV1::DuplicateModuleFlag(kind));
        }
    }
    for required in REQUIRED_MODULE_FLAGS_V1 {
        if !input.module_flags.contains(&required) {
            return Err(LoweringDiagnosticV1::MissingModuleFlag(metadata_kind(
                required,
            )));
        }
    }

    check_limit(
        ResourceKindV1::NamedMetadata,
        input.named_metadata.len(),
        MAX_NAMED_METADATA_V1,
    )?;
    if !input.named_metadata.is_empty() {
        return Err(LoweringDiagnosticV1::UnsupportedMetadata(
            MetadataKindV1::NamedMetadata,
        ));
    }
    check_limit(
        ResourceKindV1::DeviceLibraries,
        input.device_libraries.len(),
        MAX_DEVICE_LIBRARIES_V1,
    )?;
    if !input.device_libraries.is_empty() {
        return Err(LoweringDiagnosticV1::UnsupportedMetadata(
            MetadataKindV1::DeviceLibrary,
        ));
    }
    Ok(())
}

fn validate_obligations(obligations: &[ObligationKindV1]) -> Result<(), LoweringDiagnosticV1> {
    check_limit(
        ResourceKindV1::Obligations,
        obligations.len(),
        MAX_OBLIGATIONS_V1,
    )?;
    for (index, obligation) in obligations.iter().copied().enumerate() {
        if SUPPORT_MATRIX_V1.obligation(obligation) == SupportStatusV1::Rejected {
            return Err(LoweringDiagnosticV1::UnsupportedObligation(obligation));
        }
        if obligations[..index].contains(&obligation) {
            return Err(LoweringDiagnosticV1::DuplicateObligation(obligation));
        }
    }
    for required in admitted_obligations_v1() {
        if !obligations.contains(required) {
            return Err(LoweringDiagnosticV1::MissingObligation(*required));
        }
    }
    Ok(())
}

fn build_handoff(input: &ScalarKernelModuleV1) -> Result<Gfx942HandoffV1, LoweringDiagnosticV1> {
    if AMDGPU_TRIPLE != GFX942_AMDHSA_TARGET_TRIPLE_V1 {
        return Err(LoweringDiagnosticV1::ConstructionFailed(
            ConstructionStageV1::CanonicalHandoff,
        ));
    }
    let origin = OriginV1::new(
        input.origin_kind,
        input.origin_source_identity,
        input.origin_span.clone(),
    );
    let pointer_type = KernelValueTypeV1::Pointer {
        pointee: ScalarTypeV1::F32,
        address_space: AddressSpaceV1::Global,
    };
    let input_parameter = KernelParameterV1::new(
        &input.input_parameter,
        pointer_type,
        input.input_attributes.clone(),
    )
    .map_err(|_| canonical_handoff_failed())?;
    let output_parameter = KernelParameterV1::new(
        &input.output_parameter,
        pointer_type,
        input.output_attributes.clone(),
    )
    .map_err(|_| canonical_handoff_failed())?;
    let addend_parameter = KernelParameterV1::new(
        &input.addend_parameter,
        KernelValueTypeV1::Scalar(ScalarTypeV1::F32),
        input.addend_attributes.clone(),
    )
    .map_err(|_| canonical_handoff_failed())?;
    let kernel = KernelEntryV1::new(
        &input.kernel_symbol,
        vec![input_parameter, output_parameter, addend_parameter],
        input.function_attributes.clone(),
        origin.identity(),
    )
    .map_err(|_| canonical_handoff_failed())?;
    let module = ModuleMetadataV1::new(
        input.module_flags.clone(),
        input.named_metadata.clone(),
        input.device_libraries.clone(),
    )
    .map_err(|_| canonical_handoff_failed())?;
    let obligations = input
        .obligations
        .iter()
        .copied()
        .map(|kind| ObligationV1::new(kind, obligation_subject(input, kind), origin.identity()))
        .collect();
    Gfx942HandoffV1::new(Gfx942HandoffInputV1 {
        stage_identities: input.stage_identities,
        target: Gfx942TargetPolicyV1::canonical(),
        kernels: vec![kernel],
        module,
        origins: vec![origin],
        obligations,
    })
    .map_err(|_| canonical_handoff_failed())
}

fn obligation_subject(input: &ScalarKernelModuleV1, kind: ObligationKindV1) -> IdentityV1 {
    match kind {
        ObligationKindV1::PreserveKernelAbi | ObligationKindV1::MaintainOriginCoverage => {
            input.stage_identities.semantic()
        }
        ObligationKindV1::PreserveAddressSpaces
        | ObligationKindV1::PreserveTargetFeatures
        | ObligationKindV1::PreserveCallingConvention
        | ObligationKindV1::PreserveFunctionAttributes
        | ObligationKindV1::PreserveModuleMetadata
        | ObligationKindV1::AuthenticateDeviceLibraries => input.stage_identities.target_plan(),
    }
}

const fn canonical_handoff_failed() -> LoweringDiagnosticV1 {
    LoweringDiagnosticV1::ConstructionFailed(ConstructionStageV1::CanonicalHandoff)
}

fn build_dialect_module(
    context: &mut Context,
    input: &ScalarKernelModuleV1,
) -> Result<ModuleOp, LoweringDiagnosticV1> {
    let module_name = Identifier::try_from(input.module_name.as_str()).map_err(|_| {
        LoweringDiagnosticV1::ConstructionFailed(ConstructionStageV1::DialectVerification)
    })?;
    let kernel_name = Identifier::try_from(input.kernel_symbol.as_str()).map_err(|_| {
        LoweringDiagnosticV1::ConstructionFailed(ConstructionStageV1::DialectVerification)
    })?;

    let module = ModuleOp::new(context, module_name);
    let f32_type = FP32Type::get(context).into();
    let pointer_type = PointerType::get(context, AddressSpace::Global.llvm_id()).into();
    let void_type = VoidType::get(context).into();
    let function_type = FuncType::get(
        context,
        void_type,
        vec![pointer_type, pointer_type, f32_type],
        false,
    );
    let function = FuncOp::new(context, kernel_name, function_type);
    module.append_operation(context, function.get_operation(), 0);

    let entry = function.get_or_create_entry_block(context);
    let input_pointer = entry.deref(context).get_argument(0);
    let output_pointer = entry.deref(context).get_argument(1);
    let addend = entry.deref(context).get_argument(2);
    input_pointer.set_name(
        context,
        Some(
            Identifier::try_from(input.input_parameter.as_str()).map_err(|_| {
                LoweringDiagnosticV1::ConstructionFailed(ConstructionStageV1::DialectVerification)
            })?,
        ),
    );
    output_pointer.set_name(
        context,
        Some(
            Identifier::try_from(input.output_parameter.as_str()).map_err(|_| {
                LoweringDiagnosticV1::ConstructionFailed(ConstructionStageV1::DialectVerification)
            })?,
        ),
    );
    addend.set_name(
        context,
        Some(
            Identifier::try_from(input.addend_parameter.as_str()).map_err(|_| {
                LoweringDiagnosticV1::ConstructionFailed(ConstructionStageV1::DialectVerification)
            })?,
        ),
    );
    let load = LoadOp::new(context, input_pointer, f32_type);
    load.set_alignment(context, 4);
    let loaded = load.get_result(context);
    load.get_operation().insert_at_back(entry, context);

    let add =
        FAddOp::new_with_fast_math_flags(context, loaded, addend, FastmathFlagsAttr::default());
    let computed = add.get_result(context);
    add.get_operation().insert_at_back(entry, context);

    let store = StoreOp::new(context, computed, output_pointer);
    store.set_alignment(context, 4);
    store.get_operation().insert_at_back(entry, context);
    let return_op = ReturnOp::new(context, None);
    return_op.get_operation().insert_at_back(entry, context);
    Ok(module)
}

pub(crate) fn encode_receipt(
    module_name: &str,
    handoff: &Gfx942HandoffV1,
) -> Result<CanonicalLoweringReceiptV1, LoweringDiagnosticV1> {
    let handoff_bytes = handoff.encode_canonical();
    let module_name_len = u16::try_from(module_name.len()).map_err(|_| {
        LoweringDiagnosticV1::ConstructionFailed(ConstructionStageV1::ReceiptEncoding)
    })?;
    let handoff_len = u32::try_from(handoff_bytes.len()).map_err(|_| {
        LoweringDiagnosticV1::ConstructionFailed(ConstructionStageV1::ReceiptEncoding)
    })?;
    let capacity = RECEIPT_MAGIC_V1
        .len()
        .checked_add(2)
        .and_then(|value| value.checked_add(module_name.len()))
        .and_then(|value| value.checked_add(1 + VERIFIED_DIALECT_OPERATIONS_V1.len()))
        .and_then(|value| value.checked_add(4 + handoff_bytes.len()))
        .ok_or(LoweringDiagnosticV1::ConstructionFailed(
            ConstructionStageV1::ReceiptEncoding,
        ))?;
    check_limit(
        ResourceKindV1::ReceiptBytes,
        capacity,
        MAX_CANONICAL_RECEIPT_BYTES_V1,
    )?;
    let mut bytes = Vec::with_capacity(capacity);
    bytes.extend_from_slice(RECEIPT_MAGIC_V1);
    bytes.extend_from_slice(&module_name_len.to_le_bytes());
    bytes.extend_from_slice(module_name.as_bytes());
    bytes.push(VERIFIED_DIALECT_OPERATIONS_V1.len() as u8);
    for operation in VERIFIED_DIALECT_OPERATIONS_V1 {
        bytes.push(dialect_operation_tag(operation));
    }
    bytes.extend_from_slice(&handoff_len.to_le_bytes());
    bytes.extend_from_slice(handoff_bytes.as_bytes());
    if bytes.len() != capacity {
        return Err(LoweringDiagnosticV1::ConstructionFailed(
            ConstructionStageV1::ReceiptEncoding,
        ));
    }
    Ok(CanonicalLoweringReceiptV1 { bytes })
}

const fn dialect_operation_tag(operation: VerifiedDialectOperationV1) -> u8 {
    match operation {
        VerifiedDialectOperationV1::Func => 1,
        VerifiedDialectOperationV1::Load => 2,
        VerifiedDialectOperationV1::FAdd => 3,
        VerifiedDialectOperationV1::Store => 4,
        VerifiedDialectOperationV1::Return => 5,
    }
}

#[cfg(test)]
mod tests {
    use fe2o3_llvm_handoff::{
        Gfx942HandoffInputV1, Gfx942HandoffV1, IdentityV1, ObligationKindV1, ObligationV1,
        StageIdentitiesV1,
    };
    use pliron::{
        basic_block::BasicBlock,
        builtin::{op_interfaces::SymbolOpInterface, types::FP64Type},
        context::Ptr,
    };
    use pliron_llvm::attributes::FastmathFlags;

    use super::*;

    fn request() -> ScalarKernelModuleV1 {
        ScalarKernelModuleV1::canonical(
            "private_module",
            "private_add",
            IdentityV1::new([0x61; 32]).unwrap(),
            StageIdentitiesV1::new([0x11; 32], [0x22; 32], [0x33; 32]).unwrap(),
        )
    }

    fn private_function(lowered: &LoweredScalarKernelV1) -> FuncOp {
        let module_body = lowered.module.module.get_body(&lowered.context, 0);
        let operations = module_body
            .deref(&lowered.context)
            .iter(&lowered.context)
            .collect::<Vec<_>>();
        let [function] = operations.as_slice() else {
            panic!("expected one private function")
        };
        Operation::get_op::<FuncOp>(*function, &lowered.context)
            .expect("private operation must be llvm.func")
    }

    fn private_entry(lowered: &LoweredScalarKernelV1) -> Ptr<BasicBlock> {
        private_function(lowered)
            .get_entry_block(&lowered.context)
            .expect("private function must have an entry block")
    }

    fn private_body(lowered: &LoweredScalarKernelV1) -> Vec<Ptr<Operation>> {
        private_entry(lowered)
            .deref(&lowered.context)
            .iter(&lowered.context)
            .collect()
    }

    #[test]
    fn raw_operation_downcasts_remain_crate_private() {
        let lowered = lower_scalar_kernel_v1(&request()).unwrap();
        let context = &lowered.context;
        let module = &lowered.module.module;
        verify_operation(module.get_operation(), context).unwrap();

        let module_body = module.get_body(context, 0);
        let module_operations = module_body.deref(context).iter(context).collect::<Vec<_>>();
        let [function] = module_operations.as_slice() else {
            panic!("expected one private llvm.func")
        };
        let function = Operation::get_op::<FuncOp>(*function, context)
            .expect("private root must be llvm.func");
        let entry = function.get_entry_block(context).unwrap();
        let operations = entry.deref(context).iter(context).collect::<Vec<_>>();
        assert_eq!(operations.len(), 4);
        assert!(Operation::get_op::<LoadOp>(operations[0], context).is_some());
        assert!(Operation::get_op::<FAddOp>(operations[1], context).is_some());
        assert!(Operation::get_op::<StoreOp>(operations[2], context).is_some());
        assert!(Operation::get_op::<ReturnOp>(operations[3], context).is_some());
    }

    #[test]
    fn equal_arena_slots_do_not_transfer_module_ownership() {
        let mut owner = lower_scalar_kernel_v1(&request()).unwrap();
        let foreign = lower_scalar_kernel_v1(&request()).unwrap();
        assert_eq!(
            owner.module.module.get_operation(),
            foreign.module.module.get_operation()
        );

        owner.module = foreign.module.clone();
        assert_eq!(
            owner.inspect_dialect_module(),
            Err(DialectModuleInspectionErrorV1::ForeignOwner)
        );
        assert_eq!(
            owner.extract_handoff_v2(),
            Err(HandoffExtractionDiagnosticV2::ForeignOwner)
        );
    }

    #[test]
    fn stale_module_is_rejected_without_dereference_panic() {
        let mut lowered = lower_scalar_kernel_v1(&request()).unwrap();
        Operation::erase(lowered.module.module.get_operation(), &mut lowered.context);
        assert_eq!(
            lowered.inspect_dialect_module(),
            Err(DialectModuleInspectionErrorV1::StaleModule)
        );
        assert_eq!(
            lowered.extract_handoff_v2(),
            Err(HandoffExtractionDiagnosticV2::StaleModule)
        );
    }

    #[test]
    fn conflicting_upstream_borrow_is_contained() {
        let lowered = lower_scalar_kernel_v1(&request()).unwrap();
        let pointer = lowered.module.module.get_operation();
        let _borrow = pointer.deref_mut(&lowered.context);
        let result = catch_unwind(AssertUnwindSafe(|| lowered.inspect_dialect_module()));
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            Err(DialectModuleInspectionErrorV1::StaleModule)
        );
    }

    #[test]
    fn exact_extractor_accepts_the_admitted_private_graph() {
        let lowered = lower_scalar_kernel_v1(&request()).unwrap();
        let handoff = lowered.extract_handoff_v2().unwrap();
        assert_eq!(handoff.base(), lowered.handoff());
        assert_eq!(handoff.module().functions().len(), 1);
    }

    #[test]
    fn renamed_function_and_parameter_symbols_are_rejected() {
        let mut function_name = lower_scalar_kernel_v1(&request()).unwrap();
        private_function(&function_name).set_symbol_name(
            &mut function_name.context,
            Identifier::try_from("substituted_kernel").unwrap(),
        );
        assert_eq!(
            function_name.extract_handoff_v2(),
            Err(HandoffExtractionDiagnosticV2::SymbolMismatch)
        );

        let parameter_name = lower_scalar_kernel_v1(&request()).unwrap();
        let entry = private_entry(&parameter_name);
        let input = entry.deref(&parameter_name.context).get_argument(0);
        input.set_name(
            &parameter_name.context,
            Some(Identifier::try_from("substituted_input").unwrap()),
        );
        assert_eq!(
            parameter_name.extract_handoff_v2(),
            Err(HandoffExtractionDiagnosticV2::SymbolMismatch)
        );
    }

    #[test]
    fn same_typed_hostile_def_use_edges_are_rejected() {
        let lowered = lower_scalar_kernel_v1(&request()).unwrap();
        let entry = private_entry(&lowered);
        let output = entry.deref(&lowered.context).get_argument(1);
        let body = private_body(&lowered);
        Operation::replace_operand(body[0], &lowered.context, 0, output);
        verify_operation(lowered.module.module.get_operation(), &lowered.context).unwrap();
        assert_eq!(
            lowered.extract_handoff_v2(),
            Err(HandoffExtractionDiagnosticV2::DefUseMismatch)
        );

        let lowered = lower_scalar_kernel_v1(&request()).unwrap();
        let entry = private_entry(&lowered);
        let addend = entry.deref(&lowered.context).get_argument(2);
        let body = private_body(&lowered);
        Operation::replace_operand(body[2], &lowered.context, 0, addend);
        verify_operation(lowered.module.module.get_operation(), &lowered.context).unwrap();
        assert_eq!(
            lowered.extract_handoff_v2(),
            Err(HandoffExtractionDiagnosticV2::DefUseMismatch)
        );
    }

    #[test]
    fn coherent_wrong_value_type_is_rejected_after_dialect_verification() {
        let lowered = lower_scalar_kernel_v1(&request()).unwrap();
        let body = private_body(&lowered);
        let loaded = body[0].deref(&lowered.context).get_result(0);
        let computed = body[1].deref(&lowered.context).get_result(0);
        let f64_type = FP64Type::get(&lowered.context).into();
        loaded.set_type(&lowered.context, f64_type);
        computed.set_type(&lowered.context, f64_type);
        Operation::replace_operand(body[1], &lowered.context, 1, loaded);
        verify_operation(lowered.module.module.get_operation(), &lowered.context).unwrap();
        assert_eq!(
            lowered.extract_handoff_v2(),
            Err(HandoffExtractionDiagnosticV2::TypeMismatch)
        );
    }

    #[test]
    fn hostile_pointer_address_space_is_rejected() {
        let lowered = lower_scalar_kernel_v1(&request()).unwrap();
        let input = private_entry(&lowered)
            .deref(&lowered.context)
            .get_argument(0);
        input.set_type(
            &lowered.context,
            PointerType::get(&lowered.context, AddressSpace::Local.llvm_id()).into(),
        );
        verify_operation(lowered.module.module.get_operation(), &lowered.context).unwrap();
        assert_eq!(
            lowered.extract_handoff_v2(),
            Err(HandoffExtractionDiagnosticV2::AddressSpaceMismatch)
        );
    }

    #[test]
    fn hostile_alignment_and_fast_math_are_rejected() {
        let alignment = lower_scalar_kernel_v1(&request()).unwrap();
        let body = private_body(&alignment);
        Operation::get_op::<LoadOp>(body[0], &alignment.context)
            .unwrap()
            .set_alignment(&alignment.context, 8);
        verify_operation(alignment.module.module.get_operation(), &alignment.context).unwrap();
        assert_eq!(
            alignment.extract_handoff_v2(),
            Err(HandoffExtractionDiagnosticV2::AlignmentMismatch)
        );

        let fast_math = lower_scalar_kernel_v1(&request()).unwrap();
        let body = private_body(&fast_math);
        Operation::get_op::<FAddOp>(body[1], &fast_math.context)
            .unwrap()
            .set_fast_math_flags(&fast_math.context, FastmathFlagsAttr(FastmathFlags::NNAN));
        verify_operation(fast_math.module.module.get_operation(), &fast_math.context).unwrap();
        assert_eq!(
            fast_math.extract_handoff_v2(),
            Err(HandoffExtractionDiagnosticV2::StrictFpMismatch)
        );
    }

    #[test]
    fn hostile_terminator_edge_and_extra_operation_are_rejected() {
        let edge = lower_scalar_kernel_v1(&request()).unwrap();
        let entry = private_entry(&edge);
        let body = private_body(&edge);
        Operation::push_successor(body[3], &edge.context, entry);
        verify_operation(edge.module.module.get_operation(), &edge.context).unwrap();
        assert_eq!(
            edge.extract_handoff_v2(),
            Err(HandoffExtractionDiagnosticV2::ControlFlowMismatch)
        );

        let mut extra = lower_scalar_kernel_v1(&request()).unwrap();
        let entry = private_entry(&extra);
        let body = private_body(&extra);
        let computed = body[1].deref(&extra.context).get_result(0);
        let output = entry.deref(&extra.context).get_argument(1);
        let extra_store = StoreOp::new(&mut extra.context, computed, output);
        extra_store.set_alignment(&extra.context, 4);
        extra_store
            .get_operation()
            .insert_before(&extra.context, body[3]);
        verify_operation(extra.module.module.get_operation(), &extra.context).unwrap();
        assert_eq!(
            extra.extract_handoff_v2(),
            Err(HandoffExtractionDiagnosticV2::OperationShapeMismatch)
        );
    }

    #[test]
    fn hostile_origin_receipt_and_obligation_evidence_are_rejected() {
        let mut origin = lower_scalar_kernel_v1(&request()).unwrap();
        let mut substituted = request();
        substituted.origin_source_identity = IdentityV1::new([0x91; 32]).unwrap();
        origin.handoff = build_handoff(&substituted).unwrap();
        assert_eq!(
            origin.extract_handoff_v2(),
            Err(HandoffExtractionDiagnosticV2::EvidenceMismatch)
        );

        let mut receipt = lower_scalar_kernel_v1(&request()).unwrap();
        receipt.receipt.bytes[0] ^= 0xff;
        assert_eq!(
            receipt.extract_handoff_v2(),
            Err(HandoffExtractionDiagnosticV2::EvidenceMismatch)
        );

        let mut obligation = lower_scalar_kernel_v1(&request()).unwrap();
        let base = obligation.handoff.clone();
        let origin_identity = base.origins()[0].identity();
        let mut obligations = base.obligations().to_vec();
        let preserve_abi = obligations
            .iter_mut()
            .find(|candidate| candidate.kind() == ObligationKindV1::PreserveKernelAbi)
            .unwrap();
        *preserve_abi = ObligationV1::new(
            ObligationKindV1::PreserveKernelAbi,
            IdentityV1::new([0xa1; 32]).unwrap(),
            origin_identity,
        );
        obligation.handoff = Gfx942HandoffV1::new(Gfx942HandoffInputV1 {
            stage_identities: *base.stage_identities(),
            target: base.target().clone(),
            kernels: base.kernels().to_vec(),
            module: base.module().clone(),
            origins: base.origins().to_vec(),
            obligations,
        })
        .unwrap();
        obligation.receipt = encode_receipt(&obligation.module_name, &obligation.handoff).unwrap();
        assert_eq!(
            obligation.extract_handoff_v2(),
            Err(HandoffExtractionDiagnosticV2::EvidenceMismatch)
        );
    }

    #[test]
    fn child_operation_borrow_panic_is_contained() {
        let lowered = lower_scalar_kernel_v1(&request()).unwrap();
        let body = private_body(&lowered);
        let _borrow = body[1].deref_mut(&lowered.context);
        let result = catch_unwind(AssertUnwindSafe(|| lowered.extract_handoff_v2()));
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            Err(HandoffExtractionDiagnosticV2::UpstreamPanicked)
        );
    }
}
