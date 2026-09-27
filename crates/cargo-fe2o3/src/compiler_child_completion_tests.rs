use super::*;
use std::{
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

// Every assertion path reaps only the process started by this test.
struct ChildGuard(Child);
impl ChildGuard {
    fn spawn(command: &mut Command) -> Self {
        Self(command.spawn().unwrap())
    }

    fn reap(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if self.0.try_wait().unwrap().is_some() {
                return;
            }
            assert!(Instant::now() < deadline, "test child did not exit");
            thread::sleep(Duration::from_millis(5));
        }
    }
}
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn waiting_child() -> ChildGuard {
    ChildGuard::spawn(
        Command::new("/bin/cat")
            .stdin(Stdio::piped())
            .stdout(Stdio::null()),
    )
}

#[test]
fn selected_child_must_exit_successfully_before_completion() {
    let mut child = waiting_child();
    let pid = child.0.id();
    assert!(matches!(
        CompletedCompilerChild::observe(&mut child.0, pid),
        Err(CompletionError::Running)
    ));
    drop(child.0.stdin.take());
    child.reap();
    assert!(CompletedCompilerChild::observe(&mut child.0, pid).is_ok());
    // std::process::Child retains the status even after the wrapper has reaped it.
    assert!(CompletedCompilerChild::observe(&mut child.0, pid).is_ok());
}

#[test]
fn successful_unrelated_child_cannot_supply_selected_child_completion() {
    let selected = waiting_child();
    let mut other = ChildGuard::spawn(&mut Command::new("/bin/true"));
    other.reap();
    for pid in [0, selected.0.id()] {
        assert!(matches!(
            CompletedCompilerChild::observe(&mut other.0, pid),
            Err(CompletionError::WrongChild)
        ));
    }
}

#[test]
fn nonzero_compiler_exit_is_not_completion() {
    let mut child = ChildGuard::spawn(Command::new("/bin/sh").args(["-c", "exit 7"]));
    child.reap();
    let pid = child.0.id();
    let Err(CompletionError::Unsuccessful(status)) =
        CompletedCompilerChild::observe(&mut child.0, pid)
    else {
        panic!("nonzero exit must be refused");
    };
    assert_eq!(status.code(), Some(7));
}

#[test]
fn signaled_compiler_exit_is_not_completion() {
    let mut child = waiting_child();
    child.0.kill().unwrap();
    child.reap();
    let pid = child.0.id();
    let Err(CompletionError::Unsuccessful(status)) =
        CompletedCompilerChild::observe(&mut child.0, pid)
    else {
        panic!("signaled exit must be refused");
    };
    assert_eq!(status.code(), None);
}
