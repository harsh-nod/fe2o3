use core::fmt;
use std::ffi::{CStr, NulError};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HipError {
    code: fe2o3_hip_sys::hipError_t,
}

impl HipError {
    pub fn new(code: fe2o3_hip_sys::hipError_t) -> Self {
        Self { code }
    }

    pub fn code(self) -> fe2o3_hip_sys::hipError_t {
        self.code
    }

    pub fn name(self) -> String {
        let ptr = unsafe { fe2o3_hip_sys::hipGetErrorString(self.code) };
        if ptr.is_null() {
            return format!("HIP error {}", self.code);
        }
        unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned()
    }
}

impl fmt::Display for HipError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.name(), self.code)
    }
}

impl std::error::Error for HipError {}

#[derive(Debug)]
pub enum Error {
    Hip(HipError),
    NoDevice { requested: i32, count: i32 },
    Nul(NulError),
    Io(std::io::Error),
    SizeOverflow,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Hip(error) => write!(f, "{error}"),
            Self::NoDevice { requested, count } => {
                write!(
                    f,
                    "requested HIP device {requested}, but only {count} device(s) exist"
                )
            }
            Self::Nul(error) => write!(f, "string contains an interior NUL byte: {error}"),
            Self::Io(error) => write!(f, "{error}"),
            Self::SizeOverflow => write!(f, "size calculation overflowed"),
        }
    }
}

impl std::error::Error for Error {}

impl From<HipError> for Error {
    fn from(error: HipError) -> Self {
        Self::Hip(error)
    }
}

impl From<NulError> for Error {
    fn from(error: NulError) -> Self {
        Self::Nul(error)
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

pub fn check(code: fe2o3_hip_sys::hipError_t) -> Result<()> {
    if code == fe2o3_hip_sys::HIP_SUCCESS {
        Ok(())
    } else {
        Err(HipError::new(code).into())
    }
}
