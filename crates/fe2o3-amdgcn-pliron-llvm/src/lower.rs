use fe2o3_amdgcn_model::{AMDGPU_TRIPLE, AddressSpace};
use fe2o3_llvm_handoff::{
    AddressSpaceV1, FunctionAttributeV1, GFX942_AMDHSA_TARGET_TRIPLE_V1, Gfx942HandoffInputV1,
    Gfx942HandoffV1, Gfx942TargetPolicyV1, IdentityV1, KernelEntryV1, KernelParameterV1,
    KernelValueTypeV1, ModuleFlagV1, ModuleMetadataV1, ObligationKindV1, ObligationV1, OriginV1,
    ScalarTypeV1,
};
use fe2o3_pliron::{ContextIdentity, ensure_context_identity};
use pliron::{
    builtin::{
        op_interfaces::{OneResultInterface, SingleBlockRegionInterface},
        ops::ModuleOp,
        types::FP32Type,
    },
    context::Context,
    identifier::Identifier,
    op::Op,
    operation::verify_operation,
};
use pliron_llvm::{
    attributes::FastmathFlagsAttr,
    op_interfaces::FloatBinArithOpWithFastMathFlags,
    ops::{FAddOp, FuncOp, LoadOp, ReturnOp, StoreOp},
    types::{FuncType, PointerType, VoidType},
};

use crate::model::{
    CanonicalLoweringReceiptV1, ConstructionStageV1, FunctionAttributeKindV1, InputFieldV1,
    LoweringDiagnosticV1, MAX_CANONICAL_RECEIPT_BYTES_V1, MAX_DEVICE_LIBRARIES_V1,
    MAX_FUNCTION_ATTRIBUTES_V1, MAX_MODULE_FLAGS_V1, MAX_NAME_BYTES_V1, MAX_NAMED_METADATA_V1,
    MAX_OBLIGATIONS_V1, MAX_OPERATIONS_V1, MAX_PARAMETER_ATTRIBUTES_V1, MetadataKindV1,
    NameRejectionV1, ResourceKindV1, SUPPORT_MATRIX_V1, ScalarKernelModuleV1, SupportStatusV1,
    VERIFIED_DIALECT_OPERATIONS_V1, VerifiedDialectOperationV1, admitted_obligations_v1,
    admitted_operations_v1, function_attribute_kind, metadata_kind,
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

/// One verified Pliron LLVM tree and its authoritative canonical handoff.
///
/// The context identity and [`ModuleOp`] arena handle are process-local
/// provenance only. Durable comparison must use [`Self::receipt`] or
/// [`Self::handoff`].
pub struct LoweredScalarKernelV1 {
    context: Context,
    module: ModuleOp,
    context_identity: ContextIdentity,
    handoff: Gfx942HandoffV1,
    receipt: CanonicalLoweringReceiptV1,
}

impl LoweredScalarKernelV1 {
    /// Returns the context containing the verified dialect tree.
    pub const fn context(&self) -> &Context {
        &self.context
    }

    /// Returns the verified root operation.
    ///
    /// Its arena pointer is valid only with [`Self::context`] and is not an
    /// artifact, compiler, or publication identity.
    pub const fn module_op(&self) -> &ModuleOp {
        &self.module
    }

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
    let receipt = encode_receipt(input, &handoff)?;

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
        module,
        context_identity,
        handoff,
        receipt,
    })
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
    let load = LoadOp::new(context, input_pointer, f32_type);
    let loaded = load.get_result(context);
    load.get_operation().insert_at_back(entry, context);

    let add =
        FAddOp::new_with_fast_math_flags(context, loaded, addend, FastmathFlagsAttr::default());
    let computed = add.get_result(context);
    add.get_operation().insert_at_back(entry, context);

    let store = StoreOp::new(context, computed, output_pointer);
    store.get_operation().insert_at_back(entry, context);
    let return_op = ReturnOp::new(context, None);
    return_op.get_operation().insert_at_back(entry, context);
    Ok(module)
}

fn encode_receipt(
    input: &ScalarKernelModuleV1,
    handoff: &Gfx942HandoffV1,
) -> Result<CanonicalLoweringReceiptV1, LoweringDiagnosticV1> {
    let handoff_bytes = handoff.encode_canonical();
    let module_name_len = u16::try_from(input.module_name.len()).map_err(|_| {
        LoweringDiagnosticV1::ConstructionFailed(ConstructionStageV1::ReceiptEncoding)
    })?;
    let handoff_len = u32::try_from(handoff_bytes.len()).map_err(|_| {
        LoweringDiagnosticV1::ConstructionFailed(ConstructionStageV1::ReceiptEncoding)
    })?;
    let capacity = RECEIPT_MAGIC_V1
        .len()
        .checked_add(2)
        .and_then(|value| value.checked_add(input.module_name.len()))
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
    bytes.extend_from_slice(input.module_name.as_bytes());
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
