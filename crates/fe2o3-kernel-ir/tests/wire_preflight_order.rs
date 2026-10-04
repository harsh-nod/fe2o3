use fe2o3_kernel_ir::{
    CanonicalKernelIrWorkBudgetV1, Function, FunctionRole, KERNEL_IR_VERSION_V12,
    KernelIrEncodeError, MAX_FUNCTIONS_V1, MAX_TEXT_BYTES_V1,
    MeteredVerifiedCanonicalKernelIrErrorV12, Module, Signature, VerifiedCanonicalKernelIrErrorV12,
    VerifiedCanonicalKernelIrV12, encode_module_v12,
};

fn invalid_role() -> Function {
    let mut function = Function::declaration("f", Signature::new(vec![], vec![]));
    function.role = FunctionRole::DeviceFfiExport;
    function
}

fn assert_rejecting_prefix(module: Module, expected: KernelIrEncodeError, exact_work: usize) {
    assert_eq!(encode_module_v12(&module).unwrap_err(), expected);
    let mut budget = CanonicalKernelIrWorkBudgetV1::new(exact_work);
    assert!(matches!(
        VerifiedCanonicalKernelIrV12::from_module_with_work_budget_v1(module, &mut budget),
        Err(MeteredVerifiedCanonicalKernelIrErrorV12::Canonical(
            VerifiedCanonicalKernelIrErrorV12::Encode(error),
        )) if error == expected
    ));
    assert_eq!(budget.work(), exact_work);
    assert_eq!(budget.failed_work(), None);
}

#[test]
fn metered_module_id_limit_precedes_invalid_function_role() {
    let mut module = Module::new("m".repeat(MAX_TEXT_BYTES_V1 + 1));
    module.functions.push(invalid_role());
    assert_rejecting_prefix(
        module,
        KernelIrEncodeError::LimitExceeded {
            field: "module ID",
            actual: MAX_TEXT_BYTES_V1 + 1,
            max: MAX_TEXT_BYTES_V1,
        },
        5,
    );
}

#[test]
fn metered_module_function_count_limit_precedes_invalid_function_role() {
    const MODULE_ID: &str = "m";
    let mut module = Module::new(MODULE_ID);
    module.functions = vec![invalid_role(); MAX_FUNCTIONS_V1 + 1];
    // Five header tokens and two ID tokens precede the rejected count.
    const EXACT_WORK: usize = 5 + 2;
    assert_rejecting_prefix(
        module,
        KernelIrEncodeError::LimitExceeded {
            field: "module functions",
            actual: MAX_FUNCTIONS_V1 + 1,
            max: MAX_FUNCTIONS_V1,
        },
        EXACT_WORK,
    );
}

#[test]
fn metered_function_role_validation_keeps_exact_admission_boundary() {
    const MODULE_ID: &str = "m";
    const PREFIX: usize = 5 + 2 + 2;
    const ROLE_CENSUS: usize = 1;
    const ROLE_COMPARISONS: usize = "f".len() + 1;
    const EXACT_WORK: usize = PREFIX + ROLE_CENSUS + ROLE_COMPARISONS;
    let mut module = Module::new(MODULE_ID);
    module.functions.push(invalid_role());
    assert_rejecting_prefix(
        module.clone(),
        KernelIrEncodeError::UnsupportedInVersion {
            version: KERNEL_IR_VERSION_V12,
            feature: "device-FFI export function roles",
        },
        EXACT_WORK,
    );
    let mut budget = CanonicalKernelIrWorkBudgetV1::new(EXACT_WORK - 1);
    assert!(matches!(
        VerifiedCanonicalKernelIrV12::from_module_with_work_budget_v1(module, &mut budget),
        Err(MeteredVerifiedCanonicalKernelIrErrorV12::WorkLimit(error))
            if error.actual() == EXACT_WORK && error.limit() == EXACT_WORK - 1
    ));
    assert_eq!(budget.work(), PREFIX + ROLE_CENSUS);
    assert_eq!(budget.failed_work(), Some(EXACT_WORK));
}
