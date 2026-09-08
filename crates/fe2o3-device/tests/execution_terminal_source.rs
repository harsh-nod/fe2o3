const EXECUTION_SOURCE: &str = include_str!("../src/execution.rs");

#[test]
fn compiler_issuance_terminals_are_exact_and_preserve_user_closures() {
    let matrix_diagnostic = ["fe2o3_device_subgroup_", "matrix_access_v1"].concat();
    let workgroup_diagnostic = ["fe2o3_device_workgroup_", "capability_current_v1"].concat();

    assert_eq!(EXECUTION_SOURCE.matches(&matrix_diagnostic).count(), 1);
    assert_eq!(EXECUTION_SOURCE.matches(&workgroup_diagnostic).count(), 1);

    let matrix_declaration = format!(
        "#[doc(hidden)]\n    #[inline(never)]\n    #[rustc_diagnostic_item = \"{matrix_diagnostic}\"]\n    pub fn __compiler_matrix_access("
    );
    let workgroup_declaration = format!(
        "#[doc(hidden)]\n    #[inline(never)]\n    #[rustc_diagnostic_item = \"{workgroup_diagnostic}\"]\n    pub fn __compiler_workgroup_capability_current<'workgroup>("
    );
    assert!(EXECUTION_SOURCE.contains(&matrix_declaration));
    assert!(EXECUTION_SOURCE.contains(&workgroup_declaration));

    let matrix_terminal = ["self.", "__compiler_matrix_access(epoch)"].concat();
    let workgroup_terminal = ["self.", "__compiler_workgroup_capability_current()"].concat();
    let matrix_call = EXECUTION_SOURCE.find(&matrix_terminal).unwrap();
    let matrix_closure = EXECUTION_SOURCE
        .find("operation(&matrix, &self.lane)")
        .unwrap();
    let workgroup_call = EXECUTION_SOURCE.find(&workgroup_terminal).unwrap();
    let workgroup_closure = EXECUTION_SOURCE.find("operation(workgroup)").unwrap();

    assert!(matrix_call < matrix_closure);
    assert!(workgroup_call < workgroup_closure);
    assert!(
        EXECUTION_SOURCE
            .contains("unreachable!(\"subgroup matrix access requires authenticated lowering\")")
    );
    assert!(EXECUTION_SOURCE.contains(
        "unreachable!(\"workgroup capability issuance requires authenticated lowering\")"
    ));
}

#[test]
fn reusable_phases_reuse_the_exact_barrier_terminal_and_are_lifetime_branded() {
    let barrier_diagnostic = "fe2o3_device_typed_workgroup_barrier_v1";
    assert_eq!(EXECUTION_SOURCE.matches(barrier_diagnostic).count(), 1);
    assert!(EXECUTION_SOURCE.contains("pub fn with_phase<'phase, Result>("));
    assert!(EXECUTION_SOURCE.contains("&'phase mut self"));
    assert!(EXECUTION_SOURCE.contains("DynamicPhaseEpoch<'phase>"));
    assert!(
        EXECUTION_SOURCE
            .contains("self.barrier::<WorkgroupScope, AcquireRelease, WorkgroupMemory>()")
    );
    assert!(EXECUTION_SOURCE.contains(
        "_storage: &'phase mut ReusableWorkgroupLds<'workgroup, T, ELEMENTS, KernelBrand>"
    ));
}
