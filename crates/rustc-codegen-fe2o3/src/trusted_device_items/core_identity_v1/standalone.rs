//! Pinned rustc-only tests, without workspace or sysroot builds.
//! Link with `-C prefer-dynamic -C rpath -L native=<sysroot>/lib` and set
//! `LD_LIBRARY_PATH=<sysroot>/lib` when running. Output is below 20 MB.
#![feature(rustc_private)]

extern crate rustc_abi;
extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;
extern crate rustc_span;

#[path = "../core_identity_v1.rs"]
mod core_identity_v1;

mod process_execution {
    pub(crate) fn capture_output(
        command: &mut std::process::Command,
    ) -> std::io::Result<std::process::Output> {
        command.stdin(std::process::Stdio::null()).output()
    }
}
