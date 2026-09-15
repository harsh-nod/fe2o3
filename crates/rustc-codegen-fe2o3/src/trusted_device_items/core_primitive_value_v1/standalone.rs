//! Run with the repository's pinned rustc, without building the workspace:
//! Add `-L native=<sysroot>/lib` when linking and put that directory in
//! `LD_LIBRARY_PATH` when running. The executable is approximately 3 MB.
//! `rustc --edition=2024 --test -C prefer-dynamic -C rpath standalone.rs -o /tmp/primitive-value-tests`
#![feature(rustc_private)]

extern crate rustc_abi;
extern crate rustc_data_structures;
extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;
extern crate rustc_span;

#[path = "../core_primitive_value_v1.rs"]
mod core_primitive_value_v1;

mod process_execution {
    pub(crate) fn capture_output(
        command: &mut std::process::Command,
    ) -> std::io::Result<std::process::Output> {
        command.stdin(std::process::Stdio::null()).output()
    }
}
