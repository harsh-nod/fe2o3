use std::io;
use std::process::{Child, Command};

pub(crate) fn spawn(command: &mut Command) -> io::Result<Child> {
    fe2o3_artifact_transaction::with_artifact_process_spawn_v1(|| command.spawn())
}
