//! Allocation-free process/image predicates shared by legacy and native custody.
use super::{IssuerAdmissionErrorKindV1 as Kind, ProtectedCompilerExecutionIssuerAdmissionErrorV1};
use std::{fmt, io};

#[derive(Debug)]
pub(super) struct IssuerInspectionError {
    pub(super) kind: Kind,
    message: &'static str,
    errno: Option<i32>,
}

impl IssuerInspectionError {
    pub(super) const fn new(kind: Kind, message: &'static str) -> Self {
        Self {
            kind,
            message,
            errno: None,
        }
    }

    pub(super) fn io(kind: Kind, message: &'static str, error: io::Error) -> Self {
        Self {
            kind,
            message,
            errno: Some(error.raw_os_error().unwrap_or(libc::EIO)),
        }
    }
}

impl fmt::Display for IssuerInspectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message)?;
        if let Some(errno) = self.errno {
            write!(f, ": errno {errno}")?;
        }
        Ok(())
    }
}

impl std::error::Error for IssuerInspectionError {}

impl From<IssuerInspectionError> for ProtectedCompilerExecutionIssuerAdmissionErrorV1 {
    fn from(error: IssuerInspectionError) -> Self {
        Self::Failure {
            kind: error.kind,
            message: error.message.into(),
            source: error.errno.map(io::Error::from_raw_os_error),
        }
    }
}
