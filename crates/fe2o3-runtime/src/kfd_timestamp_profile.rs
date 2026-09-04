//! Native-runtime custody for host-observed dispatch timestamp evidence.

use fe2o3_profiler_protocol::{
    KfdRuntimeProfileV1, NativeRuntimeDispatchTimestampCaptureV1,
    NativeRuntimeDispatchTimestampRecorderOutputV1, encode_kfd_runtime_profile_v1,
    encode_native_runtime_dispatch_timestamp_capture_v1,
};

/// A runtime profile and timestamp capture retained through the direct-KFD
/// recorder boundary.
///
/// Fields and construction are private so decoded or caller-produced protocol
/// records cannot regain native-runtime provenance by structural conversion.
#[derive(Debug)]
pub struct AuthenticatedKfdRuntimeDispatchTimestampsV1 {
    runtime_profile: KfdRuntimeProfileV1,
    dispatch_timestamps: NativeRuntimeDispatchTimestampRecorderOutputV1,
}

impl AuthenticatedKfdRuntimeDispatchTimestampsV1 {
    pub(crate) fn new(
        runtime_profile: KfdRuntimeProfileV1,
        dispatch_timestamps: NativeRuntimeDispatchTimestampRecorderOutputV1,
    ) -> Result<Self, String> {
        let runtime_bytes =
            encode_kfd_runtime_profile_v1(&runtime_profile).map_err(|error| error.to_string())?;
        encode_native_runtime_dispatch_timestamp_capture_v1(
            dispatch_timestamps.capture(),
            &runtime_bytes,
        )
        .map_err(|error| error.to_string())?;
        Ok(Self {
            runtime_profile,
            dispatch_timestamps,
        })
    }

    pub const fn runtime_profile(&self) -> &KfdRuntimeProfileV1 {
        &self.runtime_profile
    }

    pub const fn dispatch_timestamps(&self) -> &NativeRuntimeDispatchTimestampCaptureV1 {
        self.dispatch_timestamps.capture()
    }

    pub fn into_runtime_profile(self) -> KfdRuntimeProfileV1 {
        self.runtime_profile
    }
}
