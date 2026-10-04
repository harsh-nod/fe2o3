//! Process completion is separate from issuer readiness and artifact validity.
use std::{error::Error, fmt, io, process::Child};

/// A successful exit observed on the selected Child handle, not a supplied status.
/// This says nothing about proof execution or compiler/artifact authority.
pub(super) struct CompletedCompilerChild;

impl CompletedCompilerChild {
    pub(super) fn observe(child: &mut Child, selected_pid: u32) -> Result<Self, CompletionError> {
        if selected_pid == 0 || child.id() != selected_pid {
            return Err(CompletionError::WrongChild);
        }
        match child.try_wait().map_err(CompletionError::Status)? {
            Some(status) if status.success() => Ok(Self),
            Some(status) => Err(CompletionError::Unsuccessful(status)),
            None => Err(CompletionError::Running),
        }
    }
}

#[derive(Debug)]
pub(crate) enum CompletionError {
    WrongChild,
    Running,
    Unsuccessful(std::process::ExitStatus),
    Status(io::Error),
    NotObserved,
}

impl fmt::Display for CompletionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongChild => {
                f.write_str("completion handle differs from the selected compiler child")
            }
            Self::Running => f.write_str("selected compiler child is still running"),
            Self::Unsuccessful(status) => write!(f, "selected compiler child failed: {status}"),
            Self::Status(error) => write!(f, "selected compiler child status failed: {error}"),
            Self::NotObserved => {
                f.write_str("selected compiler child success has not been observed")
            }
        }
    }
}

impl Error for CompletionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Status(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
#[path = "compiler_child_completion_tests.rs"]
mod tests;
