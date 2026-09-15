#[allow(dead_code)]
fn generated_library_arguments_typecheck<'allocation>(
    a: &'allocation [f32],
    b: &'allocation [f32],
    c: &'allocation mut [f32],
) {
    let a = fe2o3_host::__generated::GeneratedKfdReadSlice::new(a);
    let b = fe2o3_host::__generated::GeneratedKfdReadSlice::new(b);
    let c = fe2o3_host::__generated::GeneratedKfdReadWriteSlice::new(c);
    let _arguments: fe2o3_vecadd::vecadd_gpu::Arguments<'allocation> =
        fe2o3_vecadd::vecadd_gpu::Arguments::new(a, b, c);
}

#[test]
fn library_and_existing_host_target_use_the_same_kernel_body() {
    let library = include_str!("../src/lib.rs");
    let host = include_str!("../src/main.rs");
    for source in [library, host] {
        assert!(source.contains("include!(\"vecadd_body.rs\")"));
        assert!(source.contains("vecadd_kernel_body!(thread, (), production_f32_add, a, b, c)"));
    }
    assert!(library.starts_with("#![no_std]"));
}
