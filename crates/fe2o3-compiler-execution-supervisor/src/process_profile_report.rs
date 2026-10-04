//! One bounded read per attempt, with private-pipe EOF required before admission.

use super::ChildProcessError;
use fe2o3_protected_service_profile::observations::CHILD_NAMESPACE_REPORT_BYTES;
use rustix::io::Errno;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ReportError {
    Truncated,
    Trailing,
    State,
    Io(Errno),
}

impl From<ReportError> for ChildProcessError {
    fn from(error: ReportError) -> Self {
        match error {
            ReportError::Truncated => Self::State("truncated child namespace report"),
            ReportError::Trailing => Self::State("trailing child namespace report bytes"),
            ReportError::State => Self::State("invalid child namespace report read state"),
            ReportError::Io(errno) => Self::Io {
                operation: "read child namespace report",
                errno,
            },
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum State {
    Reading,
    Complete,
    Rejected,
}

pub(super) struct ProfileReportRead {
    bytes: [u8; CHILD_NAMESPACE_REPORT_BYTES + 1],
    used: usize,
    state: State,
}

impl ProfileReportRead {
    pub(super) const fn new() -> Self {
        Self {
            bytes: [0; CHILD_NAMESPACE_REPORT_BYTES + 1],
            used: 0,
            state: State::Reading,
        }
    }

    pub(super) fn observe(
        &mut self,
        read: impl FnOnce(&mut [u8]) -> rustix::io::Result<usize>,
    ) -> Result<Option<()>, ReportError> {
        if self.state != State::Reading {
            return Err(ReportError::State);
        }
        let remaining = self.bytes.len() - self.used;
        let result = match read(&mut self.bytes[self.used..]) {
            Ok(0) if self.used == CHILD_NAMESPACE_REPORT_BYTES => {
                self.state = State::Complete;
                Ok(Some(()))
            }
            Ok(0) => Err(ReportError::Truncated),
            Ok(count) if count > remaining => Err(ReportError::State),
            Ok(count) => {
                self.used += count;
                if self.used > CHILD_NAMESPACE_REPORT_BYTES {
                    Err(ReportError::Trailing)
                } else {
                    Ok(None)
                }
            }
            Err(Errno::AGAIN | Errno::INTR) => Ok(None),
            Err(errno) => Err(ReportError::Io(errno)),
        };
        if result.is_err() {
            self.state = State::Rejected;
        }
        result
    }

    pub(super) fn report(&self) -> Result<&[u8], ChildProcessError> {
        if self.state == State::Complete {
            Ok(&self.bytes[..CHILD_NAMESPACE_REPORT_BYTES])
        } else {
            Err(ReportError::State.into())
        }
    }
}

#[cfg(test)]
#[path = "process_profile_report_tests.rs"]
mod tests;
