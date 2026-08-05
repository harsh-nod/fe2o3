use core::fmt;
use std::ffi::{CStr, NulError};

#[derive(Clone, Copy, Eq, PartialEq)]
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

impl fmt::Debug for HipError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl std::error::Error for HipError {}

pub enum Error {
    Hip(HipError),
    NoDevice {
        requested: i32,
        count: i32,
    },
    DeviceMismatch {
        buffer_device: i32,
        stream_device: i32,
    },
    EventDeviceMismatch {
        event_device: i32,
        stream_device: i32,
    },
    EventPairDeviceMismatch {
        start_device: i32,
        stop_device: i32,
    },
    EventPending,
    EventTimingDisabled,
    NullHostAllocation,
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
            Self::DeviceMismatch {
                buffer_device,
                stream_device,
            } => write!(
                f,
                "device buffer belongs to HIP device {buffer_device}, but the stream belongs to HIP device {stream_device}"
            ),
            Self::EventDeviceMismatch {
                event_device,
                stream_device,
            } => write!(
                f,
                "event belongs to HIP device {event_device}, but the stream belongs to HIP device {stream_device}"
            ),
            Self::EventPairDeviceMismatch {
                start_device,
                stop_device,
            } => write!(
                f,
                "start event belongs to HIP device {start_device}, but the stop event belongs to HIP device {stop_device}"
            ),
            Self::EventPending => write!(f, "event is still pending and cannot be recorded again"),
            Self::EventTimingDisabled => {
                write!(f, "elapsed time requires timing-enabled events")
            }
            Self::NullHostAllocation => write!(
                f,
                "hipHostMalloc returned a null pointer for a non-empty allocation"
            ),
            Self::Nul(error) => write!(f, "string contains an interior NUL byte: {error}"),
            Self::Io(error) => write!(f, "{error}"),
            Self::SizeOverflow => write!(f, "size calculation overflowed"),
        }
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
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

#[cfg(test)]
mod tests {
    use super::Error;

    #[test]
    fn debug_uses_display_text() {
        let error = Error::NoDevice {
            requested: 2,
            count: 1,
        };

        assert_eq!(
            format!("{error:?}"),
            "requested HIP device 2, but only 1 device(s) exist"
        );
    }

    #[test]
    fn device_mismatch_reports_both_device_ids() {
        let error = Error::DeviceMismatch {
            buffer_device: 1,
            stream_device: 3,
        };

        assert_eq!(
            error.to_string(),
            "device buffer belongs to HIP device 1, but the stream belongs to HIP device 3"
        );
    }
}
