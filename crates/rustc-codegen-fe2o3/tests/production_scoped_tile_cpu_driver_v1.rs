#![deny(warnings)]

#[path = "production_scoped_tile_cpu_driver_v1/source_contract.rs"]
mod source_contract;

#[cfg(target_os = "linux")]
#[path = "production_scoped_tile_cpu_driver_v1/linux.rs"]
mod linux;

#[test]
#[ignore = "requires the pinned nightly rust-src component and AMD source target; CPU only"]
fn ordinary_mixed_tile_source_executes_public_cpu_cli_paths() {
    #[cfg(target_os = "linux")]
    linux::run_ordinary_source_cpu_paths();
    #[cfg(not(target_os = "linux"))]
    panic!("ordinary diagnostic source CPU workflow requires a Linux host");
}
