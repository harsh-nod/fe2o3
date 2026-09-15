//! Small pinned-rustc test runner for the arithmetic authenticator. Compile
//! directly to avoid building the workspace. From the repository root:
//! ```text
//! arithmetic_sysroot=$(rustc --print sysroot)
//! arithmetic_tests=$(mktemp -d /tmp/fe2o3-core-arithmetic.XXXXXX)
//! rustc --edition=2024 --test -C prefer-dynamic -C rpath -L native="$arithmetic_sysroot/lib" crates/rustc-codegen-fe2o3/src/trusted_device_items/core_checked_mul_v1_tests/standalone.rs -o "$arithmetic_tests/tests"
//! LD_LIBRARY_PATH="$arithmetic_sysroot/lib" "$arithmetic_tests/tests" --test-threads=1
//! ```
#![feature(rustc_private)]

extern crate rustc_abi;
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

#[path = "../core_checked_mul_v1.rs"]
mod core_checked_mul_v1;

// The production capture wrapper also owns an artifact-spawn guard. These
// no-codegen tests create no artifacts; only the subprocess plumbing is local.
mod process_execution {
    pub(crate) fn capture_output(
        command: &mut std::process::Command,
    ) -> std::io::Result<std::process::Output> {
        command.stdin(std::process::Stdio::null()).output()
    }
}
