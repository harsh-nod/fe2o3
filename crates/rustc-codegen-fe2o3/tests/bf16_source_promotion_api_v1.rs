//! External API visibility/type check only; no synthetic frontend authority.
#![feature(rustc_private)]
#![cfg(target_os = "linux")]
#[test]
fn source_only_bf16_drivers_are_public_and_have_closed_signatures() {
    let _: fn(&[String], &std::path::Path) -> Result<(), String> =
        rustc_codegen_fe2o3::run_bf16_tile_source_inspection_driver_v1;
    let _: fn(&[String], &std::path::Path, &std::path::Path) -> Result<(), String> =
        rustc_codegen_fe2o3::run_bf16_tile_source_promotion_driver_v1;
}
