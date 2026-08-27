use std::io;
use std::process::{Command, Output, Stdio};

pub(crate) fn capture_output(command: &mut Command) -> io::Result<Output> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    fe2o3_artifact_transaction::with_artifact_process_spawn_v1(|| command.spawn())?
        .wait_with_output()
}
