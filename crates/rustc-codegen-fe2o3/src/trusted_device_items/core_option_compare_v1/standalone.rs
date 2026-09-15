//! Standalone rustc-private tests using the repository's pinned rustc.
//! Link with `-L native=<sysroot>/lib -C prefer-dynamic -C rpath`.
#![feature(rustc_private)]

extern crate rustc_abi;
extern crate rustc_data_structures;
extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;
extern crate rustc_span;

use rustc_abi::ExternAbi;
use rustc_hir::Safety;
use rustc_middle::mir::{
    BinOp, Body, Operand, Rvalue, StatementKind, TerminatorKind, UnwindAction,
};
use rustc_middle::ty::{Instance, InstanceKind, Ty, TyCtxt, TyKind, TypingEnv};

#[path = "../core_option_compare_v1.rs"]
mod core_option_compare_v1;
#[path = "../core_primitive_value_v1.rs"]
mod core_primitive_value_v1;
#[path = "../core_wrapping_v1.rs"]
mod core_wrapping_v1;
#[path = "../production_mir_v1.rs"]
mod production_mir_v1;
use production_mir_v1::{ProductionMirV1, production_mir_v1};
#[path = "../../test_temp_dir.rs"]
mod test_temp_dir;

mod process_execution {
    pub(crate) fn capture_output(
        command: &mut std::process::Command,
    ) -> std::io::Result<std::process::Output> {
        command.stdin(std::process::Stdio::null()).output()
    }
}
