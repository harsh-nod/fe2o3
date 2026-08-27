#![allow(dead_code)]

use std::io;
use std::process::{Child, Command, ExitStatus, Output, Stdio};

pub(crate) fn spawn(command: &mut Command) -> io::Result<Child> {
    fe2o3_artifact_transaction::with_artifact_process_spawn_v1(|| command.spawn())
}

pub(crate) fn status(command: &mut Command) -> io::Result<ExitStatus> {
    spawn(command)?.wait()
}

pub(crate) fn capture_output(command: &mut Command) -> io::Result<Output> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    spawn(command)?.wait_with_output()
}
