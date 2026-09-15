//! Pinned rustc-only tests; no workspace or sysroot build.
//! Link with `-C prefer-dynamic -C rpath -L native=<sysroot>/lib` and run
//! with `LD_LIBRARY_PATH=<sysroot>/lib`. The executable is below 20 MB.
#![feature(rustc_private)]

extern crate rustc_abi;
extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;
extern crate rustc_span;

#[path = "../core_result_map_v1.rs"]
mod core_result_map_v1;

mod process_execution {
    pub(crate) fn capture_output(
        command: &mut std::process::Command,
    ) -> std::io::Result<std::process::Output> {
        command.stdin(std::process::Stdio::null()).output()
    }
}
