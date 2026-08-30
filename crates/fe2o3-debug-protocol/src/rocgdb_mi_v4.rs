//! Redacted result records for exact ROCgdb/KFD stopped-state correlation.

use serde::{Deserialize, Serialize};

use crate::{
    LiveGpuAvailabilityV3, LiveGpuContentIdentityV3, LiveGpuRelativePcV3, LiveGpuSourceSpanV3,
    OpaqueIdentityV1,
};

pub const MAX_ROCGDB_MI_LANES_V4: usize = 64;
pub const ROCGDB_MI_NATIVE_CLI_RESPONSE_SCHEMA_V4: &str = "fe2o3-rocgdb-kfd-native-response-v4";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RocgdbMiNativeCliResponseSchemaV4 {
    #[serde(rename = "fe2o3-rocgdb-kfd-native-response-v4")]
    V4,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RocgdbMiNativeUnavailableReasonV4 {
    RocgdbSpawnFailed,
    StructuredCommandsUnavailable,
    DirectKfdDeviceUnavailable,
    TargetLaunchFailed,
    CooperativeTelemetryUnavailable,
    TargetExitedBeforePublication,
    NativePublicationNotObserved,
    GpuStoppedStateUnavailable,
    CorrelationRejected,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RocgdbMiNativeProbeV4 {
    pub structured_mi_commands: bool,
    pub direct_kfd_device_admitted: bool,
    pub cooperative_v2_declaration: bool,
    pub cooperative_v2_publication: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum RocgdbMiNativeCliResultV4 {
    Available {
        probe: RocgdbMiNativeProbeV4,
        stopped_state: Box<RocgdbMiNativeStoppedStateV4>,
    },
    Unavailable {
        probe: RocgdbMiNativeProbeV4,
        reason: RocgdbMiNativeUnavailableReasonV4,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RocgdbMiNativeCliResponseV4 {
    pub schema: RocgdbMiNativeCliResponseSchemaV4,
    pub result: RocgdbMiNativeCliResultV4,
}

impl RocgdbMiNativeCliResponseV4 {
    pub fn validate(&self) -> Result<(), RocgdbMiNativeProtocolErrorV4> {
        if let RocgdbMiNativeCliResultV4::Available {
            probe,
            stopped_state,
        } = &self.result
        {
            if !probe.structured_mi_commands
                || !probe.direct_kfd_device_admitted
                || !probe.cooperative_v2_declaration
                || !probe.cooperative_v2_publication
            {
                return Err(RocgdbMiNativeProtocolErrorV4::InvalidProbe);
            }
            stopped_state.validate()?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RocgdbMiNativeCorrelationOriginV4 {
    /// Observed by the target's direct-KFD queue owner at publication time.
    TargetKfdPublicationObservation,
    /// Observed by ROCgdb from one structured MI result tuple.
    RocgdbStructuredObservation,
    /// Explicit load base joined to the exact inspector-derived kernel entry range.
    ExplicitCodeObjectAdmission,
    /// Exact equality or bounded arithmetic over retained observations.
    Correlated,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RocgdbMiWorkgroupCoordinateV4 {
    pub x: u32,
    pub y: u32,
    pub z: u32,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RocgdbMiWorkitemCoordinateV4 {
    pub x: u32,
    pub y: u32,
    pub z: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RocgdbMiNativeLaneV4 {
    pub lane_identity: OpaqueIdentityV1,
    pub lane_index: u16,
    pub workitem: RocgdbMiWorkitemCoordinateV4,
    pub active: LiveGpuAvailabilityV3<bool>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RocgdbMiNativeStoppedStateV4 {
    pub association_identity: OpaqueIdentityV1,
    pub queue_occurrence_identity: OpaqueIdentityV1,
    pub process_instance_identity: OpaqueIdentityV1,
    pub dispatch_identity: OpaqueIdentityV1,
    pub artifact: LiveGpuContentIdentityV3,
    pub grid: [u32; 3],
    pub workgroup: [u32; 3],
    pub workgroup_coordinate: RocgdbMiWorkgroupCoordinateV4,
    pub wave_identity: OpaqueIdentityV1,
    pub wave_ordinal: u32,
    pub workgroup_thread_index: u32,
    pub lanes: Vec<RocgdbMiNativeLaneV4>,
    pub relative_pc: LiveGpuAvailabilityV3<LiveGpuRelativePcV3>,
    pub source: LiveGpuAvailabilityV3<LiveGpuSourceSpanV3>,
    /// Registers require a subsequent structured register command bound to this stop.
    pub registers: LiveGpuAvailabilityV3<OpaqueIdentityV1>,
    /// Memory requires a subsequent relative read bound to an admitted allocation.
    pub memory: LiveGpuAvailabilityV3<OpaqueIdentityV1>,
    pub origins: Vec<RocgdbMiNativeCorrelationOriginV4>,
}

impl RocgdbMiNativeStoppedStateV4 {
    pub fn validate(&self) -> Result<(), RocgdbMiNativeProtocolErrorV4> {
        let wave_width = self.lanes.len();
        if !matches!(wave_width, 32 | 64) || wave_width > MAX_ROCGDB_MI_LANES_V4 {
            return Err(RocgdbMiNativeProtocolErrorV4::InvalidWaveWidth);
        }
        if self.grid.contains(&0)
            || self.workgroup.contains(&0)
            || self
                .workgroup
                .iter()
                .zip(self.grid)
                .any(|(workgroup, grid)| *workgroup > grid)
        {
            return Err(RocgdbMiNativeProtocolErrorV4::InvalidGeometry);
        }
        for (expected, lane) in self.lanes.iter().enumerate() {
            if usize::from(lane.lane_index) != expected {
                return Err(RocgdbMiNativeProtocolErrorV4::InvalidLane);
            }
        }
        if self.origins
            != [
                RocgdbMiNativeCorrelationOriginV4::TargetKfdPublicationObservation,
                RocgdbMiNativeCorrelationOriginV4::RocgdbStructuredObservation,
                RocgdbMiNativeCorrelationOriginV4::ExplicitCodeObjectAdmission,
                RocgdbMiNativeCorrelationOriginV4::Correlated,
            ]
        {
            return Err(RocgdbMiNativeProtocolErrorV4::InvalidOrigins);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RocgdbMiNativeProtocolErrorV4 {
    InvalidWaveWidth,
    InvalidGeometry,
    InvalidLane,
    InvalidOrigins,
    InvalidProbe,
}
