use super::*;

#[test]
fn metered_owner_charges_exact_byte_passes_before_work() {
    const MODULE_ID: &str = "meter";
    // Header 20, three top-level counts 12, and length-prefixed ID 4+5.
    const CANONICAL_BYTES: usize = 20 + 12 + 4 + MODULE_ID.len();
    const EXACT_HEADER_CHECK: usize = KERNEL_IR_MAGIC_V1.len() + 2;
    const DECLARED_LENGTH_PATCHES: usize = 2 * 4;
    const DECODED_TEXT_VALIDATION_AND_COPY: usize = 2 * MODULE_ID.len();
    const IDENTITY_FRAME: usize =
        4 + VERIFIED_CANONICAL_KERNEL_IR_V12_IDENTITY_DOMAIN_V1.len() + 2 + 8;
    // Sizing counts schema tokens; output, decode, canonical comparison,
    // module equality, and hashing retain five byte-volume passes. Comparison
    // also queries each schema chunk and the final length, without a second Vec.
    const COUNT_TOKENS: usize = 5 + 2 + 3;
    const COMPARISON_QUERIES: usize = (5 + 2 + 3) + 1;
    const EXACT_WORK: usize = 5 * CANONICAL_BYTES
        + COUNT_TOKENS
        + EXACT_HEADER_CHECK
        + DECLARED_LENGTH_PATCHES
        + COMPARISON_QUERIES
        + DECODED_TEXT_VALIDATION_AND_COPY
        + IDENTITY_FRAME;

    let module = Module::new(MODULE_ID);
    assert_eq!(encode_module_v12(&module).unwrap().len(), CANONICAL_BYTES);
    let mut exact = CanonicalKernelIrWorkBudgetV1::new(EXACT_WORK);
    let owner =
        VerifiedCanonicalKernelIrV12::from_module_with_work_budget_v1(module.clone(), &mut exact)
            .unwrap();
    assert_eq!(owner.canonical_bytes().len(), CANONICAL_BYTES);
    assert_eq!(exact.work(), EXACT_WORK);
    assert_eq!(exact.failed_work(), None);

    let mut one_under = CanonicalKernelIrWorkBudgetV1::new(EXACT_WORK - 1);
    assert!(matches!(
        VerifiedCanonicalKernelIrV12::from_module_with_work_budget_v1(module, &mut one_under,),
        Err(MeteredVerifiedCanonicalKernelIrErrorV12::WorkLimit(error))
            if error.actual() == EXACT_WORK && error.limit() == EXACT_WORK - 1
    ));
    assert!(one_under.work() < EXACT_WORK);
    assert_eq!(one_under.failed_work(), Some(EXACT_WORK));
}

#[test]
fn resource_metered_owner_reports_exact_empty_verifier_receipt() {
    const MODULE_ID: &str = "meter";
    const CANONICAL_BYTES: usize = 20 + 12 + 4 + MODULE_ID.len();
    const HEADER_CHECK: usize = KERNEL_IR_MAGIC_V1.len() + 2;
    const LENGTH_PATCHES: usize = 2 * 4;
    const TEXT_VALIDATION_AND_COPY: usize = 2 * MODULE_ID.len();
    const IDENTITY_FRAME: usize =
        4 + VERIFIED_CANONICAL_KERNEL_IR_V12_IDENTITY_DOMAIN_V1.len() + 2 + 8;
    const COUNT_TOKENS: usize = 5 + 2 + 3;
    const COMPARISON_QUERIES: usize = (5 + 2 + 3) + 1;
    const CANONICAL_WORK: usize = 5 * CANONICAL_BYTES
        + COUNT_TOKENS
        + HEADER_CHECK
        + LENGTH_PATCHES
        + COMPARISON_QUERIES
        + TEXT_VALIDATION_AND_COPY
        + IDENTITY_FRAME;
    // The empty verifier owns no rows. Its only work is construction of
    // the module diagnostic-location value passed through capability
    // validation.
    const VERIFICATION_WORK: usize = 5;
    const EXACT_WORK: usize = CANONICAL_WORK + VERIFICATION_WORK;

    let module = Module::new(MODULE_ID);
    let mut exact = CanonicalKernelIrWorkBudgetV1::new(EXACT_WORK);
    let (owner, receipt) = VerifiedCanonicalKernelIrV12::from_module_with_resource_budget_v1(
        module.clone(),
        &mut exact,
        0,
    )
    .unwrap();
    assert_eq!(owner.canonical_bytes().len(), CANONICAL_BYTES);
    assert_eq!(receipt.work(), VERIFICATION_WORK);
    assert_eq!(receipt.peak_storage(), 0);
    assert_eq!(exact.work(), EXACT_WORK);

    let mut one_under = CanonicalKernelIrWorkBudgetV1::new(EXACT_WORK - 1);
    assert!(matches!(
        VerifiedCanonicalKernelIrV12::from_module_with_resource_budget_v1(
            module,
            &mut one_under,
            0,
        ),
        Err(MeteredVerifiedCanonicalKernelIrErrorV12::VerificationWorkLimit {
            error,
            receipt,
        }) if error.actual() == EXACT_WORK
            && error.limit() == EXACT_WORK - 1
            && receipt.work() == VERIFICATION_WORK
            && receipt.peak_storage() == 0
    ));
    assert_eq!(one_under.failed_work(), Some(EXACT_WORK));
}

#[test]
fn resource_metered_failures_retain_exact_verifier_receipts() {
    const MESSAGE: &str = "module identity must not be empty";
    const COUNT_PASS: usize = 5 + 1 + MESSAGE.len() + 5;
    // The empty module identifier still needs reserve/construction decisions.
    const IDENTIFIER_COPY: usize = 5 + 2;
    const ROW: usize =
        std::mem::size_of::<crate::Diagnostic>().div_ceil(std::mem::size_of::<usize>());
    const MATERIALIZE_PASS: usize =
        5 + 1 + MESSAGE.len() + IDENTIFIER_COPY + (MESSAGE.len() + MESSAGE.len() + 1) + 5;
    const FINISH: usize = 1;
    const VERIFICATION_WORK: usize = COUNT_PASS + MATERIALIZE_PASS + FINISH;
    const DIAGNOSTIC_STORAGE: usize = ROW + MESSAGE.len();
    let module = Module::new("");
    let legacy = verify_module(&module).unwrap_err();

    let mut exact = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let error = VerifiedCanonicalKernelIrV12::from_module_with_resource_budget_v1(
        module.clone(),
        &mut exact,
        DIAGNOSTIC_STORAGE,
    )
    .unwrap_err();
    assert!(matches!(
        &error,
        MeteredVerifiedCanonicalKernelIrErrorV12::Verification {
            error,
            receipt,
        } if error == &legacy
            && receipt.work() == VERIFICATION_WORK
            && receipt.peak_storage() == DIAGNOSTIC_STORAGE
    ));
    assert_eq!(
        error.verification_receipt(),
        Some(CanonicalKernelIrVerificationResourceReceiptV1::new(
            VERIFICATION_WORK,
            DIAGNOSTIC_STORAGE,
        ))
    );

    // The count pass is complete. The materializing pass owns its exact
    // diagnostic row and has prepaid both formatting passes and message
    // publication before the message buffer reservation is rejected.
    const STORAGE_FAILURE_WORK: usize =
        COUNT_PASS + 5 + 1 + MESSAGE.len() + IDENTIFIER_COPY + (MESSAGE.len() + MESSAGE.len() + 1);
    let mut storage_one_under = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let error = VerifiedCanonicalKernelIrV12::from_module_with_resource_budget_v1(
        module,
        &mut storage_one_under,
        DIAGNOSTIC_STORAGE - 1,
    )
    .unwrap_err();
    assert!(
        matches!(
            &error,
            MeteredVerifiedCanonicalKernelIrErrorV12::VerificationResource {
                error: CanonicalKernelIrVerificationResourceErrorV1::Storage(error),
                receipt,
            } if error.actual() == DIAGNOSTIC_STORAGE
                && error.limit() == DIAGNOSTIC_STORAGE - 1
                && receipt.work() == STORAGE_FAILURE_WORK
                && receipt.peak_storage() == ROW
        ),
        "unexpected verifier resource receipt: {error:?}"
    );
}

#[test]
fn metered_counter_covers_rich_rejecting_wire_schema_before_verification() {
    use crate::{
        AccessMode, AddressSpace, BasicBlock, BlockId, Function, MatrixElement, MatrixOperation,
        MemoryElementType, MemoryIntrinsicOperation, MemoryLayout, Operation, OperationKind,
        ScalarType, Signature, TargetCapability, TensorLayoutContractV1, Terminator, Type,
        ValueDef, ValueId, VolatileAccessContract,
    };

    const MODULE_ID: &str = "m";
    const FUNCTION_ID: &str = "f";
    const NAMESPACE: &str = "com.example.canonical.long-prefix";
    const NAME: &str = "rich-tensor-layout-extension";
    let matrix = MatrixOperation::lds_load(ValueId(0), MatrixElement::Bf16)
        .with_declared_tensor_layout(
            TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64(),
        );
    let results = matrix
        .result_types()
        .into_iter()
        .enumerate()
        .map(|(index, ty)| ValueDef::new(ValueId(index as u32 + 1), ty))
        .collect();
    let nested_parameter = Type::pointer(
        Type::pointer(
            Type::Scalar(ScalarType::Bf16),
            AddressSpace::Workgroup,
            AccessMode::ReadWrite,
        ),
        AddressSpace::Workgroup,
        AccessMode::ReadWrite,
    );
    let mut function = Function::internal_helper(
        FUNCTION_ID,
        Signature::new(vec![nested_parameter], vec![]),
        vec![ValueId(0)],
        vec![BasicBlock {
            id: BlockId(0),
            parameters: vec![],
            operations: vec![
                Operation::new(results, OperationKind::Matrix(matrix)),
                Operation::effect_free(
                    ValueDef::new(ValueId(5), Type::Scalar(ScalarType::U32)),
                    OperationKind::MemoryIntrinsic(MemoryIntrinsicOperation::VolatileLoad {
                        pointer: ValueId(0),
                        element: MemoryElementType::Scalar(ScalarType::U32),
                        address_space: AddressSpace::Global,
                        layout: MemoryLayout::new(4, 4),
                        contract: VolatileAccessContract::rust_allocation_load(),
                    }),
                ),
            ],
            terminator: Some(Terminator::Return { values: vec![] }),
        }],
    );
    function.required_capabilities =
        std::collections::BTreeSet::from([TargetCapability::Extension {
            namespace: NAMESPACE.into(),
            name: NAME.into(),
        }]);
    let mut module = Module::new(MODULE_ID);
    module.functions.push(function);

    let canonical_bytes = encode_module_v12(&module).unwrap().len();
    const EXACT_HEADER_CHECK: usize = KERNEL_IR_MAGIC_V1.len() + 2;
    const DECLARED_LENGTH_PATCHES: usize = 2 * 4;
    const ROLE_WORK_PER_PASS: usize = 1 + FUNCTION_ID.len() + 1;
    const DECODED_TEXT_VALIDATION_AND_COPY: usize =
        2 * (MODULE_ID.len() + FUNCTION_ID.len() + NAMESPACE.len() + NAME.len());
    const CAPABILITY_WIDTH: usize = NAMESPACE.len() + NAME.len() + 2;
    const CAPABILITY_COMPARE_CLONE_INSERT: usize = 2 * CAPABILITY_WIDTH;
    const VOLATILE_INSTANCE_PAYLOAD_BYTES: usize = 7 + 12;
    const VOLATILE_INSTANCE_COPY_WORK_PER_PASS: usize =
        crate::SEMANTIC_OPERATION_INSTANCE_HEADER_BYTES_V1 + 2 * VOLATILE_INSTANCE_PAYLOAD_BYTES;
    const VOLATILE_INSTANCE_COPY_WORK: usize = 3 * VOLATILE_INSTANCE_COPY_WORK_PER_PASS;
    // The contract has three 21-chunk fragments. The matrix wrapper adds
    // twelve chunks, its operation header fourteen, and the volatile op eight.
    const FRAGMENT_CHUNKS: usize = 1 + 2 + 1 + 1 + 1 + 2 + 2 * 5 + 1 + 1 + 1;
    const CONTRACT_CHUNKS: usize = 1 + 1 + 3 * FRAGMENT_CHUNKS + 1;
    const MATRIX_CHUNKS: usize = 14 + (1 + 2 + 1 + 1 + 6 + 1) + CONTRACT_CHUNKS;
    const FUNCTION_CHUNKS: usize = (2 + 2 + 8) + 1 + 3 + 3 + MATRIX_CHUNKS + 8 + 3 + 6;
    const COUNT_TOKENS: usize = 10 + FUNCTION_CHUNKS;
    const COMPARISON_QUERIES: usize = 10 + FUNCTION_CHUNKS + 1;
    // Token sizing plus write, decode, and comparison precede semantic rejection.
    // The instance producer still runs during count, write, and comparison.
    let exact_work = 3 * canonical_bytes
        + COUNT_TOKENS
        + EXACT_HEADER_CHECK
        + DECLARED_LENGTH_PATCHES
        + COMPARISON_QUERIES
        + 3 * ROLE_WORK_PER_PASS
        + DECODED_TEXT_VALIDATION_AND_COPY
        + CAPABILITY_COMPARE_CLONE_INSERT
        + VOLATILE_INSTANCE_COPY_WORK;
    let mut exact = CanonicalKernelIrWorkBudgetV1::new(exact_work);
    assert!(matches!(
        VerifiedCanonicalKernelIrV12::from_module_with_work_budget_v1(module.clone(), &mut exact,),
        Err(MeteredVerifiedCanonicalKernelIrErrorV12::Canonical(
            VerifiedCanonicalKernelIrErrorV12::Verification(_)
        ))
    ));
    assert_eq!(exact.work(), exact_work);
    assert_eq!(exact.failed_work(), None);

    let mut one_under = CanonicalKernelIrWorkBudgetV1::new(exact_work - 1);
    assert!(matches!(
        VerifiedCanonicalKernelIrV12::from_module_with_work_budget_v1(module, &mut one_under,),
        Err(MeteredVerifiedCanonicalKernelIrErrorV12::WorkLimit(error))
            if error.actual() == exact_work && error.limit() == exact_work - 1
    ));
    assert!(one_under.work() < exact_work);
    assert_eq!(one_under.failed_work(), Some(exact_work));
}
