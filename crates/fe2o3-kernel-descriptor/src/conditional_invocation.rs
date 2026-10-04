//! Shared inert conditional invocation schema, also re-exported by artifacts.
//! It lives here to avoid the artifacts -> transaction -> ffi -> descriptor cycle.
pub use crate::conditional_invocation_codec_v1::*;
pub use crate::conditional_invocation_codec_v2::*;
pub use crate::conditional_invocation_v1::*;
pub use crate::conditional_invocation_v2::*;
