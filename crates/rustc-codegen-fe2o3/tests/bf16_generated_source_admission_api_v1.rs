#![feature(rustc_private)]
#[test]
#[cfg(target_os = "linux")]
fn generated_source_admission_driver_is_public_but_returns_no_owner() {
    let _: fn(&[String], &std::path::Path) -> Result<(), String> =
        rustc_codegen_fe2o3::run_bf16_generated_source_admission_driver_v1;
}
