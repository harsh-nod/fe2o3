//! Integration coverage for the closed V1 typed lowering lane.

use fe2o3_amdgcn_model::AddressSpace;
use fe2o3_amdgcn_pliron_llvm::{
    ConstructionStageV1, FunctionAttributeKindV1, InputFieldV1, LoweringDiagnosticV1,
    MAX_CANONICAL_RECEIPT_BYTES_V1, MAX_DIAGNOSTIC_BYTES_V1, MetadataKindV1, NameRejectionV1,
    SUPPORT_MATRIX_V1, ScalarKernelModuleV1, ScalarOperationV1, SourceCallingConventionV1,
    SupportStatusV1, TargetFeaturePolicyV1, VERIFIED_DIALECT_OPERATIONS_V1, lower_scalar_kernel_v1,
};
use fe2o3_llvm_handoff::{
    FunctionAttributeV1, GFX942_AMDHSA_DATA_LAYOUT_V1, GFX942_AMDHSA_TARGET_TRIPLE_V1, IdentityV1,
    ModuleFlagV1, NamedMetadataV1, ObligationKindV1, OriginKindV1, ParameterAttributeV1,
    ScalarTypeV1, StageIdentitiesV1, TargetFeatureV1, WavesPerEuV1,
};
use pliron::{
    builtin::{op_interfaces::SingleBlockRegionInterface, type_interfaces::FunctionTypeInterface},
    linked_list::ContainsLinkedList,
    op::Op,
    operation::{Operation, verify_operation},
    r#type::Typed,
};
use pliron_llvm::{
    attributes::FastmathFlagsAttr,
    op_interfaces::FastMathFlags,
    ops::{FAddOp, FuncOp, LoadOp, ReturnOp, StoreOp},
    types::PointerType,
};

fn request() -> ScalarKernelModuleV1 {
    ScalarKernelModuleV1::canonical(
        "scalar_module",
        "scalar_add",
        IdentityV1::new([0x41; 32]).unwrap(),
        StageIdentitiesV1::new([0x11; 32], [0x22; 32], [0x33; 32]).unwrap(),
    )
}

fn assert_rejected(input: &ScalarKernelModuleV1, expected: LoweringDiagnosticV1) {
    let error = lower_scalar_kernel_v1(input)
        .err()
        .expect("hostile request must be rejected");
    assert_eq!(error, expected);
    assert!(
        error.to_string().len() <= MAX_DIAGNOSTIC_BYTES_V1,
        "diagnostic exceeded its public byte bound"
    );
}

#[test]
fn fresh_contexts_build_real_verified_ops_with_deterministic_receipts() {
    let first = lower_scalar_kernel_v1(&request()).unwrap();
    let second = lower_scalar_kernel_v1(&request()).unwrap();

    assert_ne!(first.context_identity(), second.context_identity());
    assert_eq!(first.receipt(), second.receipt());
    assert!(first.receipt().len() <= MAX_CANONICAL_RECEIPT_BYTES_V1);
    assert_eq!(
        first.handoff().encode_canonical(),
        second.handoff().encode_canonical()
    );
    assert_eq!(first.operation_inventory(), &VERIFIED_DIALECT_OPERATIONS_V1);

    let context = first.context();
    verify_operation(first.module_op().get_operation(), context).unwrap();
    let module_body = first.module_op().get_body(context, 0);
    let module_operations = module_body.deref(context).iter(context).collect::<Vec<_>>();
    assert_eq!(module_operations.len(), 1);
    let function = Operation::get_op::<FuncOp>(module_operations[0], context)
        .expect("module must contain a real llvm.func");
    let function_type = function.get_type(context);
    assert_eq!(function_type.deref(context).arg_types().len(), 3);
    let entry = function
        .get_entry_block(context)
        .expect("defined function must have an entry block");
    let arguments = entry.deref(context).arguments().collect::<Vec<_>>();
    for argument in &arguments[..2] {
        let argument_type = argument.get_type(context);
        let argument_type = argument_type.deref(context);
        let pointer = argument_type
            .downcast_ref::<PointerType>()
            .expect("first two arguments must be opaque LLVM pointers");
        assert_eq!(pointer.address_space(), AddressSpace::Global.llvm_id());
    }

    let operations = entry.deref(context).iter(context).collect::<Vec<_>>();
    assert_eq!(operations.len(), 4);
    assert!(Operation::get_op::<LoadOp>(operations[0], context).is_some());
    let add = Operation::get_op::<FAddOp>(operations[1], context)
        .expect("second body operation must be a real llvm.fadd");
    assert_eq!(add.fast_math_flags(context), FastmathFlagsAttr::default());
    assert!(Operation::get_op::<StoreOp>(operations[2], context).is_some());
    assert!(Operation::get_op::<ReturnOp>(operations[3], context).is_some());
}

#[test]
fn canonical_handoff_preserves_exact_identities_and_policy() {
    let input = request();
    let lowered = lower_scalar_kernel_v1(&input).unwrap();
    let handoff = lowered.handoff();

    assert_eq!(
        handoff.stage_identities().semantic(),
        input.stage_identities.semantic()
    );
    assert_eq!(
        handoff.stage_identities().schedule(),
        input.stage_identities.schedule()
    );
    assert_eq!(
        handoff.stage_identities().target_plan(),
        input.stage_identities.target_plan()
    );
    assert_eq!(
        handoff.target().target_triple(),
        GFX942_AMDHSA_TARGET_TRIPLE_V1
    );
    assert_eq!(handoff.target().data_layout(), GFX942_AMDHSA_DATA_LAYOUT_V1);
    assert_eq!(handoff.target().cpu(), "gfx942");
    assert_eq!(
        handoff
            .target()
            .features()
            .iter()
            .map(|feature| (feature.feature(), feature.enabled()))
            .collect::<Vec<_>>(),
        vec![
            (TargetFeatureV1::WavefrontSize32, false),
            (TargetFeatureV1::WavefrontSize64, true),
            (TargetFeatureV1::Xnack, false),
        ]
    );
    assert_eq!(handoff.kernels().len(), 1);
    assert_eq!(handoff.kernels()[0].symbol(), input.kernel_symbol);
    assert_eq!(handoff.origins().len(), 1);
    assert_eq!(handoff.origins()[0].kind(), OriginKindV1::AmdgcnIr);
    assert_eq!(
        handoff.origins()[0].source_identity(),
        input.origin_source_identity
    );
    assert_eq!(handoff.obligations().len(), 7);
}

#[test]
fn accepted_set_permutations_have_one_canonical_receipt() {
    let canonical = request();
    let mut permuted = canonical.clone();
    permuted.function_attributes.reverse();
    permuted.module_flags.reverse();
    permuted.obligations.reverse();

    let canonical = lower_scalar_kernel_v1(&canonical).unwrap();
    let permuted = lower_scalar_kernel_v1(&permuted).unwrap();
    assert_eq!(canonical.receipt(), permuted.receipt());
    assert_eq!(
        canonical.handoff().identity(),
        permuted.handoff().identity()
    );
}

#[test]
fn support_matrix_is_explicit_for_every_policy_dimension() {
    assert_eq!(
        SUPPORT_MATRIX_V1.operation(ScalarOperationV1::LoadInputF32),
        SupportStatusV1::Supported
    );
    assert_eq!(
        SUPPORT_MATRIX_V1.operation(ScalarOperationV1::Call),
        SupportStatusV1::Rejected
    );
    assert_eq!(
        SUPPORT_MATRIX_V1.scalar_type(ScalarTypeV1::F32),
        SupportStatusV1::Supported
    );
    assert_eq!(
        SUPPORT_MATRIX_V1.scalar_type(ScalarTypeV1::F64),
        SupportStatusV1::Rejected
    );
    assert_eq!(
        SUPPORT_MATRIX_V1.address_space(AddressSpace::Global),
        SupportStatusV1::Supported
    );
    assert_eq!(
        SUPPORT_MATRIX_V1.address_space(AddressSpace::BufferFatPointer),
        SupportStatusV1::Rejected
    );
    assert_eq!(
        SUPPORT_MATRIX_V1.calling_convention(SourceCallingConventionV1::AmdGpuKernel),
        SupportStatusV1::Supported
    );
    assert_eq!(
        SUPPORT_MATRIX_V1.calling_convention(SourceCallingConventionV1::C),
        SupportStatusV1::Rejected
    );
    assert_eq!(
        SUPPORT_MATRIX_V1.target_policy(TargetFeaturePolicyV1::Gfx942Wave64XnackMinus),
        SupportStatusV1::Supported
    );
    assert_eq!(
        SUPPORT_MATRIX_V1.target_policy(TargetFeaturePolicyV1::Gfx942Wave64XnackPlus),
        SupportStatusV1::Rejected
    );
    assert_eq!(
        SUPPORT_MATRIX_V1.parameter_attribute(ParameterAttributeV1::NoAlias),
        SupportStatusV1::Rejected
    );
    assert_eq!(
        SUPPORT_MATRIX_V1.module_flag(ModuleFlagV1::CodeObjectVersion6),
        SupportStatusV1::Supported
    );
    assert_eq!(
        SUPPORT_MATRIX_V1.module_flag(ModuleFlagV1::WcharSize4),
        SupportStatusV1::Rejected
    );
    assert_eq!(
        SUPPORT_MATRIX_V1.origin(OriginKindV1::AmdgcnIr, false),
        SupportStatusV1::Supported
    );
    assert_eq!(
        SUPPORT_MATRIX_V1.origin(OriginKindV1::RustSource, false),
        SupportStatusV1::Rejected
    );
    assert_eq!(
        SUPPORT_MATRIX_V1.obligation(ObligationKindV1::PreserveKernelAbi),
        SupportStatusV1::Supported
    );
    assert_eq!(
        SUPPORT_MATRIX_V1.obligation(ObligationKindV1::AuthenticateDeviceLibraries),
        SupportStatusV1::Rejected
    );
}

#[test]
fn hostile_operation_is_rejected_by_name() {
    let mut input = request();
    input.operations[1] = ScalarOperationV1::Call;
    assert_rejected(
        &input,
        LoweringDiagnosticV1::UnsupportedOperation(ScalarOperationV1::Call),
    );
}

#[test]
fn hostile_type_is_rejected_by_name() {
    let mut input = request();
    input.scalar_type = ScalarTypeV1::F64;
    assert_rejected(
        &input,
        LoweringDiagnosticV1::UnsupportedType(ScalarTypeV1::F64),
    );
}

#[test]
fn hostile_address_space_is_rejected_by_name() {
    let mut input = request();
    input.address_space = AddressSpace::BufferFatPointer;
    assert_rejected(
        &input,
        LoweringDiagnosticV1::UnsupportedAddressSpace(AddressSpace::BufferFatPointer),
    );
}

#[test]
fn hostile_calling_convention_is_rejected_by_name() {
    let mut input = request();
    input.calling_convention = SourceCallingConventionV1::C;
    assert_rejected(
        &input,
        LoweringDiagnosticV1::UnsupportedCallingConvention(SourceCallingConventionV1::C),
    );
}

#[test]
fn hostile_target_features_are_rejected_by_name() {
    let mut input = request();
    input.target_policy = TargetFeaturePolicyV1::Gfx942Wave64XnackPlus;
    assert_rejected(
        &input,
        LoweringDiagnosticV1::UnsupportedTargetPolicy(TargetFeaturePolicyV1::Gfx942Wave64XnackPlus),
    );
}

#[test]
fn hostile_function_and_parameter_attributes_are_rejected_by_name() {
    let mut function = request();
    function
        .function_attributes
        .push(FunctionAttributeV1::WavesPerEu(
            WavesPerEuV1::new(1, 2).unwrap(),
        ));
    assert_rejected(
        &function,
        LoweringDiagnosticV1::UnsupportedFunctionAttribute(FunctionAttributeKindV1::WavesPerEu),
    );

    let mut parameter = request();
    parameter
        .input_attributes
        .push(ParameterAttributeV1::NoAlias);
    assert_rejected(
        &parameter,
        LoweringDiagnosticV1::UnsupportedParameterAttribute(ParameterAttributeV1::NoAlias),
    );
}

#[test]
fn hostile_metadata_is_rejected_by_name() {
    let mut flag = request();
    flag.module_flags.push(ModuleFlagV1::WcharSize4);
    assert_rejected(
        &flag,
        LoweringDiagnosticV1::UnsupportedMetadata(MetadataKindV1::WcharSize4),
    );

    let mut named = request();
    named.named_metadata.push(NamedMetadataV1::ProducerIdentity(
        IdentityV1::new([0x51; 32]).unwrap(),
    ));
    assert_rejected(
        &named,
        LoweringDiagnosticV1::UnsupportedMetadata(MetadataKindV1::NamedMetadata),
    );
}

#[test]
fn hostile_origin_and_obligation_are_rejected_by_name() {
    let mut origin = request();
    origin.origin_kind = OriginKindV1::RustSource;
    assert_rejected(
        &origin,
        LoweringDiagnosticV1::UnsupportedOrigin {
            kind: OriginKindV1::RustSource,
            has_span: false,
        },
    );

    let mut obligation = request();
    obligation.obligations[0] = ObligationKindV1::AuthenticateDeviceLibraries;
    assert_rejected(
        &obligation,
        LoweringDiagnosticV1::UnsupportedObligation(ObligationKindV1::AuthenticateDeviceLibraries),
    );
}

#[test]
fn hostile_names_and_sequences_are_rejected_without_echoing_input() {
    let mut name = request();
    name.module_name = "x".repeat(4_096);
    let error = lower_scalar_kernel_v1(&name)
        .err()
        .expect("oversized hostile name must be rejected");
    assert_eq!(
        error,
        LoweringDiagnosticV1::InvalidName {
            field: InputFieldV1::ModuleName,
            reason: NameRejectionV1::TooLong,
        }
    );
    assert!(error.to_string().len() <= MAX_DIAGNOSTIC_BYTES_V1);
    assert!(!error.to_string().contains(&name.module_name));

    let mut sequence = request();
    sequence.operations.swap(0, 1);
    assert_rejected(
        &sequence,
        LoweringDiagnosticV1::UnsupportedOperationSequence,
    );
}

#[test]
fn upstream_failures_have_bounded_non_authoritative_diagnostics() {
    let diagnostic =
        LoweringDiagnosticV1::ConstructionFailed(ConstructionStageV1::DialectVerification);
    assert!(diagnostic.to_string().len() <= MAX_DIAGNOSTIC_BYTES_V1);
    assert!(!diagnostic.to_string().contains("llvm.func"));
}
