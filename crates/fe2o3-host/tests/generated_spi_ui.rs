#[test]
fn generated_spi_is_explicit_unsafe_and_type_sealed() {
    let tests = trybuild::TestCases::new();
    for path in [
        "tests/ui/generated_spi/fail/f16_bits_requires_unsafe.rs",
        "tests/ui/generated_spi/fail/kfd_invocation_cannot_clone.rs",
        "tests/ui/generated_spi/fail/kfd_invocation_fields_are_private.rs",
        "tests/ui/generated_spi/fail/kfd_invocation_has_no_unchecked_request.rs",
        "tests/ui/generated_spi/fail/packed_arguments_fields_are_private.rs",
        "tests/ui/generated_spi/fail/packed_arguments_have_no_raw_pointer.rs",
        "tests/ui/generated_spi/fail/packing_input_fields_are_private.rs",
        "tests/ui/generated_spi/fail/packing_plan_raw_fields_are_private.rs",
        "tests/ui/generated_spi/fail/slice_pointer_binding_requires_unsafe.rs",
        "tests/ui/generated_spi/fail/unsupported_generated_scalar_types.rs",
        "tests/ui/generated_spi/fail/worker_v3_application_binding_cannot_clone.rs",
        "tests/ui/generated_spi/fail/worker_v3_application_binding_fields_are_private.rs",
    ] {
        tests.compile_fail(path);
    }
}
