//! Exact, redacted correlation of structured ROCgdb MI3 GPU hierarchy records.
//!
//! This module accepts only MI result records from the five documented query
//! commands. AMD `target-id` values are parsed by an exact versioned grammar;
//! `details` and stream records are never evidence. Native identifiers remain
//! private inputs and are replaced by session-scoped digests in the result.

use std::collections::BTreeSet;
use std::fmt;

use fe2o3_debug_protocol::{
    LiveGpuAvailabilityV3, LiveGpuContentIdentityV3, LiveGpuEvidenceKindV3, LiveGpuEvidenceRefV3,
    LiveGpuRelativePcV3, LiveGpuTruthOriginV3, LiveGpuTruthV3, LiveGpuUnavailableReasonV3,
    OpaqueIdentityV1, RocgdbMiNativeLaneV4, RocgdbMiNativeStoppedStateV4, RocgdbMiStoppedScopeV3,
    RocgdbMiThreadIdentityV3, RocgdbMiWaveIdentityV3, RocgdbMiWorkgroupCoordinateV4,
    RocgdbMiWorkitemCoordinateV4,
};
use fe2o3_kfd::{KfdTargetDebugTelemetryDigestV1, KfdTargetDebugTelemetryPayloadV2};
use sha2::{Digest, Sha256};

use crate::rocgdb_mi_parser_v3::{
    MiListV3, MiParserLimitsV3, MiRecordV3, MiResultsV3, MiValueV3, parse_mi_record_v3,
};

const MAX_ENTITIES_V4: usize = 4_096;
const MAX_TARGET_ID_BYTES_V4: usize = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RocgdbMiNativeCorrelationErrorV4 {
    InvalidMi,
    NotResult,
    BackendRejected,
    MissingField(&'static str),
    InvalidField(&'static str),
    UnknownField(&'static str),
    InvalidTargetId,
    CountOutOfRange,
    DuplicateEntity,
    MissingEntity,
    ProcessSubstitution,
    DeviceSubstitution,
    QueueSubstitution,
    PacketSubstitution,
    ArtifactSubstitution,
    DispatchSubstitution,
    GeometrySubstitution,
    HierarchySubstitution,
    ThreadSubstitution,
    LaneSubstitution,
    StopSubstitution,
    StaleGeneration,
    IdentityCollision,
    ProtocolRejected,
}

#[derive(Debug)]
pub enum RocgdbMiNativeQueryErrorV4 {
    Backend(crate::rocgdb_mi_v3::RocgdbMiAdapterErrorV3),
    Correlation(RocgdbMiNativeCorrelationErrorV4),
}

impl fmt::Display for RocgdbMiNativeQueryErrorV4 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Backend(error) => error.fmt(formatter),
            Self::Correlation(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for RocgdbMiNativeQueryErrorV4 {}

impl From<crate::rocgdb_mi_v3::RocgdbMiAdapterErrorV3> for RocgdbMiNativeQueryErrorV4 {
    fn from(value: crate::rocgdb_mi_v3::RocgdbMiAdapterErrorV3) -> Self {
        Self::Backend(value)
    }
}

impl From<RocgdbMiNativeCorrelationErrorV4> for RocgdbMiNativeQueryErrorV4 {
    fn from(value: RocgdbMiNativeCorrelationErrorV4) -> Self {
        Self::Correlation(value)
    }
}

impl fmt::Display for RocgdbMiNativeCorrelationErrorV4 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "ROCgdb/KFD native V4 correlation failed: {self:?}"
        )
    }
}

impl std::error::Error for RocgdbMiNativeCorrelationErrorV4 {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct KfdPublishedDispatchBindingV4 {
    pub(crate) process_instance: OpaqueIdentityV1,
    pub(crate) queue_occurrence: OpaqueIdentityV1,
    pub(crate) generation: u64,
    pub(crate) gpu_id: u32,
    pub(crate) queue_id: u32,
    pub(crate) packet_id: u64,
    pub(crate) artifact: LiveGpuContentIdentityV3,
    pub(crate) dispatch: OpaqueIdentityV1,
    pub(crate) grid: [u32; 3],
    pub(crate) workgroup: [u32; 3],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RocgdbInferiorBindingV4 {
    process_instance: OpaqueIdentityV1,
    expected_generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RocgdbDirectKfdDeviceBindingV4 {
    gpu_id: u32,
}

impl RocgdbDirectKfdDeviceBindingV4 {
    pub fn from_checked_device(device: &fe2o3_kfd::CheckedGfx942XnackMinusDevice) -> Self {
        Self {
            gpu_id: device.observation().kfd_gpu_id(),
        }
    }
}

impl RocgdbInferiorBindingV4 {
    pub fn new(
        process_instance: OpaqueIdentityV1,
        expected_generation: u64,
    ) -> Result<Self, RocgdbMiNativeCorrelationErrorV4> {
        if expected_generation == 0 {
            return Err(RocgdbMiNativeCorrelationErrorV4::StaleGeneration);
        }
        Ok(Self {
            process_instance,
            expected_generation,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RocgdbCodeObjectBindingV4 {
    artifact: LiveGpuContentIdentityV3,
    load_base: u64,
    entry_address: u64,
    entry_size: u64,
}

impl RocgdbCodeObjectBindingV4 {
    pub fn new(
        artifact: LiveGpuContentIdentityV3,
        load_base: u64,
        entry_address: u64,
        entry_size: u64,
    ) -> Result<Self, RocgdbMiNativeCorrelationErrorV4> {
        let value = Self {
            artifact,
            load_base,
            entry_address,
            entry_size,
        };
        validate_code_binding(value)?;
        Ok(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct QualifiedIdV4 {
    raw: Vec<u8>,
    inferior: u32,
    local: u32,
}

#[derive(Clone, Debug)]
struct AgentV4 {
    id: QualifiedIdV4,
    gpu_id: u32,
    evidence: OpaqueIdentityV1,
}

#[derive(Clone, Debug)]
struct QueueV4 {
    id: QualifiedIdV4,
    target_agent: u32,
    target_queue: u32,
    queue_id: u32,
    evidence: OpaqueIdentityV1,
}

#[derive(Clone, Debug)]
struct DispatchV4 {
    id: QualifiedIdV4,
    target_agent: u32,
    target_queue: u32,
    target_dispatch: u32,
    packet_id: u64,
    grid: [u32; 3],
    workgroup: [u32; 3],
    evidence: OpaqueIdentityV1,
}

#[derive(Clone, Debug)]
struct ThreadV4 {
    raw_id: Vec<u8>,
    target_agent: u32,
    target_queue: u32,
    target_dispatch: u32,
    wave: u32,
    workgroup: [u32; 3],
    wave_in_workgroup: u32,
    frame_address: Option<u64>,
    evidence: OpaqueIdentityV1,
}

struct ParsedThreadTargetV4 {
    agent: u32,
    queue: u32,
    dispatch: u32,
    wave: u32,
    workgroup: [u32; 3],
    wave_in_workgroup: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LaneStateV4 {
    Active,
    Inactive,
    Unavailable,
}

#[derive(Clone, Debug)]
struct LaneV4 {
    lane: u16,
    state: LaneStateV4,
    target_agent: u32,
    target_queue: u32,
    target_dispatch: u32,
    wave: u32,
    workgroup: [u32; 3],
    workitem: [u32; 3],
    evidence: OpaqueIdentityV1,
}

/// Stateful V4 admission. It does not own ROCgdb or KFD control authority.
pub struct RocgdbMiNativeCorrelationAdapterV4 {
    session: OpaqueIdentityV1,
    stop: Option<OpaqueIdentityV1>,
    agents: Vec<AgentV4>,
    queues: Vec<QueueV4>,
    dispatches: Vec<DispatchV4>,
    threads: Vec<ThreadV4>,
    lanes: Vec<LaneV4>,
}

impl RocgdbMiNativeCorrelationAdapterV4 {
    pub fn new(session: OpaqueIdentityV1) -> Self {
        Self {
            session,
            stop: None,
            agents: Vec::new(),
            queues: Vec::new(),
            dispatches: Vec::new(),
            threads: Vec::new(),
            lanes: Vec::new(),
        }
    }

    pub(crate) fn bind_stop_identity_v4(
        &mut self,
        stop: OpaqueIdentityV1,
    ) -> Result<(), RocgdbMiNativeCorrelationErrorV4> {
        if self.stop.replace(stop).is_some() {
            return Err(RocgdbMiNativeCorrelationErrorV4::StopSubstitution);
        }
        Ok(())
    }

    pub fn admit_agent_info(
        &mut self,
        line: &[u8],
    ) -> Result<(), RocgdbMiNativeCorrelationErrorV4> {
        let results = result_record(line)?;
        validate_top_fields(&results, &["agents", "current-agent-id"])?;
        let tuples = tuple_list(required(&results, "agents")?, "agents")?;
        require_count(tuples.len())?;
        let mut parsed = Vec::with_capacity(tuples.len());
        for tuple in tuples {
            validate_fields(
                tuple,
                &[
                    "id",
                    "state",
                    "target-id",
                    "details",
                    "architecture",
                    "device-name",
                    "name",
                    "cores",
                    "threads",
                    "location",
                ],
                &["id", "state", "target-id"],
            )?;
            if constant(tuple, "state")? != b"A" {
                continue;
            }
            let id = parse_qualified_id(constant(tuple, "id")?)?;
            let gpu_id = parse_agent_target_id(constant(tuple, "target-id")?)?;
            parsed.push(AgentV4 {
                id,
                gpu_id,
                evidence: self.evidence(b"agent-info", line)?,
            });
        }
        reject_duplicate(parsed.iter().map(|item| item.id.raw.as_slice()))?;
        self.agents = parsed;
        Ok(())
    }

    pub fn admit_queue_info(
        &mut self,
        line: &[u8],
    ) -> Result<(), RocgdbMiNativeCorrelationErrorV4> {
        let results = result_record(line)?;
        validate_top_fields(&results, &["queues", "current-queue-id"])?;
        let tuples = tuple_list(required(&results, "queues")?, "queues")?;
        require_count(tuples.len())?;
        let mut parsed = Vec::with_capacity(tuples.len());
        for tuple in tuples {
            validate_fields(
                tuple,
                &["id", "target-id", "type", "read", "write", "size", "addr"],
                &["id", "target-id", "type", "size", "addr"],
            )?;
            let id = parse_qualified_id(constant(tuple, "id")?)?;
            if constant(tuple, "type")? != b"HSA" {
                continue;
            }
            parse_decimal::<u64>(constant(tuple, "size")?)
                .filter(|value| *value != 0)
                .ok_or(RocgdbMiNativeCorrelationErrorV4::InvalidField("queue size"))?;
            parse_hex_u64(constant(tuple, "addr")?).ok_or(
                RocgdbMiNativeCorrelationErrorV4::InvalidField("queue address"),
            )?;
            if let Some(read) = optional_constant(tuple, "read") {
                parse_decimal::<u64>(read)
                    .ok_or(RocgdbMiNativeCorrelationErrorV4::InvalidField("queue read"))?;
            }
            if let Some(write) = optional_constant(tuple, "write") {
                parse_decimal::<u64>(write).ok_or(
                    RocgdbMiNativeCorrelationErrorV4::InvalidField("queue write"),
                )?;
            }
            let (target_agent, target_queue, queue_id) =
                parse_queue_target_id(constant(tuple, "target-id")?)?;
            parsed.push(QueueV4 {
                id,
                target_agent,
                target_queue,
                queue_id,
                evidence: self.evidence(b"queue-info", line)?,
            });
        }
        reject_duplicate(parsed.iter().map(|item| item.id.raw.as_slice()))?;
        self.queues = parsed;
        Ok(())
    }

    pub fn admit_dispatch_info(
        &mut self,
        line: &[u8],
    ) -> Result<(), RocgdbMiNativeCorrelationErrorV4> {
        let results = result_record(line)?;
        validate_top_fields(&results, &["dispatches", "current-dispatch-id"])?;
        let tuples = tuple_list(required(&results, "dispatches")?, "dispatches")?;
        require_count(tuples.len())?;
        let mut parsed = Vec::with_capacity(tuples.len());
        for tuple in tuples {
            // ROCm 7.2.4's deployed emitter is authoritative. Its bundled
            // manual shows an incompatible stale queue_id/fences list schema.
            validate_fields(
                tuple,
                &[
                    "id",
                    "target-id",
                    "grid",
                    "workgroup",
                    "fence",
                    "address-spaces",
                    "kernel-desc",
                    "kernel-args",
                    "completion",
                    "kernel-function",
                ],
                &[
                    "id",
                    "target-id",
                    "grid",
                    "workgroup",
                    "fence",
                    "address-spaces",
                    "kernel-desc",
                    "kernel-args",
                    "completion",
                    "kernel-function",
                ],
            )?;
            validate_deployed_fence_v4(constant(tuple, "fence")?)?;
            validate_deployed_address_spaces_v4(constant(tuple, "address-spaces")?)?;
            let id = parse_qualified_id(constant(tuple, "id")?)?;
            let (target_agent, target_queue, target_dispatch, packet_id) =
                parse_dispatch_target_id(constant(tuple, "target-id")?)?;
            let grid = dimension_constant(constant(tuple, "grid")?, "grid")?;
            let workgroup = dimension_constant(constant(tuple, "workgroup")?, "workgroup")?;
            for field in [
                "kernel-desc",
                "kernel-args",
                "completion",
                "kernel-function",
            ] {
                parse_hex_u64(constant(tuple, field)?)
                    .ok_or(RocgdbMiNativeCorrelationErrorV4::InvalidField(field))?;
            }
            validate_geometry(grid, workgroup)?;
            parsed.push(DispatchV4 {
                id,
                target_agent,
                target_queue,
                target_dispatch,
                packet_id,
                grid,
                workgroup,
                evidence: self.evidence(b"dispatch-info", line)?,
            });
        }
        reject_duplicate(parsed.iter().map(|item| item.id.raw.as_slice()))?;
        self.dispatches = parsed;
        Ok(())
    }

    pub fn admit_thread_info(
        &mut self,
        line: &[u8],
    ) -> Result<(), RocgdbMiNativeCorrelationErrorV4> {
        let results = result_record(line)?;
        validate_top_fields(&results, &["threads", "current-thread-id"])?;
        let tuples = tuple_list(required(&results, "threads")?, "threads")?;
        require_count(tuples.len())?;
        let current = constant(&results, "current-thread-id")?;
        let mut parsed = Vec::new();
        for tuple in tuples {
            validate_fields(
                tuple,
                &[
                    "id",
                    "target-id",
                    "details",
                    "name",
                    "frame",
                    "state",
                    "core",
                ],
                &["id", "target-id", "state"],
            )?;
            if constant(tuple, "state")? != b"stopped" {
                continue;
            }
            let raw_id = constant(tuple, "id")?.to_vec();
            validate_native_token(&raw_id, "thread id")?;
            if raw_id != current {
                continue;
            }
            let target = parse_thread_target_id(constant(tuple, "target-id")?)?;
            let frame_address = tuple
                .get("frame")
                .and_then(MiValueV3::as_tuple)
                .and_then(|frame| optional_constant(frame, "addr"))
                .and_then(parse_hex_u64);
            parsed.push(ThreadV4 {
                raw_id,
                target_agent: target.agent,
                target_queue: target.queue,
                target_dispatch: target.dispatch,
                wave: target.wave,
                workgroup: target.workgroup,
                wave_in_workgroup: target.wave_in_workgroup,
                frame_address,
                evidence: self.evidence(b"thread-info", line)?,
            });
        }
        reject_duplicate(parsed.iter().map(|item| item.raw_id.as_slice()))?;
        self.threads = parsed;
        Ok(())
    }

    pub fn admit_lane_info(&mut self, line: &[u8]) -> Result<(), RocgdbMiNativeCorrelationErrorV4> {
        let results = result_record(line)?;
        validate_top_fields(&results, &["lanes", "current-lane-id"])?;
        let tuples = tuple_list(required(&results, "lanes")?, "lanes")?;
        require_count(tuples.len())?;
        let mut parsed = Vec::with_capacity(tuples.len());
        for tuple in tuples {
            validate_fields(
                tuple,
                &["id", "state", "target-id", "details", "frame"],
                &["id", "state", "target-id"],
            )?;
            let lane = parse_decimal::<u16>(constant(tuple, "id")?)
                .ok_or(RocgdbMiNativeCorrelationErrorV4::InvalidField("lane id"))?;
            let state = match constant(tuple, "state")? {
                b"A" => LaneStateV4::Active,
                b"I" => LaneStateV4::Inactive,
                b"U" => LaneStateV4::Unavailable,
                _ => return Err(RocgdbMiNativeCorrelationErrorV4::InvalidField("lane state")),
            };
            let (
                target_agent,
                target_queue,
                target_dispatch,
                wave,
                target_lane,
                workgroup,
                workitem,
            ) = parse_lane_target_id(constant(tuple, "target-id")?)?;
            if u32::from(lane) != target_lane {
                return Err(RocgdbMiNativeCorrelationErrorV4::LaneSubstitution);
            }
            parsed.push(LaneV4 {
                lane,
                state,
                target_agent,
                target_queue,
                target_dispatch,
                wave,
                workgroup,
                workitem,
                evidence: self.evidence(b"lane-info", line)?,
            });
        }
        reject_duplicate(parsed.iter().map(|item| item.lane.to_le_bytes()))?;
        self.lanes = parsed;
        Ok(())
    }

    /// Returns the native selector only to the in-process MI owner, paired
    /// with redacted identities that are safe to publish. The caller must
    /// keep the V4 stop pin current while issuing every subsequent command.
    pub(crate) fn inspection_scope_v5(
        &self,
        stopped: &RocgdbMiNativeStoppedStateV4,
    ) -> Result<(Vec<u8>, RocgdbMiStoppedScopeV3), RocgdbMiNativeCorrelationErrorV4> {
        stopped
            .validate()
            .map_err(|_| RocgdbMiNativeCorrelationErrorV4::ProtocolRejected)?;
        let stop = self
            .stop
            .ok_or(RocgdbMiNativeCorrelationErrorV4::StopSubstitution)?;
        let thread = unique(self.threads.iter())
            .map_err(|_| RocgdbMiNativeCorrelationErrorV4::ThreadSubstitution)?;
        let expected_wave = self.derive(b"wave", &[&thread.evidence.as_bytes()])?;
        if stopped.wave_identity != expected_wave {
            return Err(RocgdbMiNativeCorrelationErrorV4::ThreadSubstitution);
        }
        let thread_identity = RocgdbMiThreadIdentityV3 {
            identity: self.derive(
                b"inspection-thread-v5",
                &[&thread.evidence.as_bytes(), &stop.as_bytes()],
            )?,
        };
        Ok((
            thread.raw_id.clone(),
            RocgdbMiStoppedScopeV3 {
                stop_identity: stop,
                thread: thread_identity,
                wave: RocgdbMiWaveIdentityV3 {
                    identity: stopped.wave_identity,
                    thread: thread_identity,
                },
                lane: None,
            },
        ))
    }

    /// Correlates exact V2 declaration/publication records with the admitted MI hierarchy.
    pub fn correlate_telemetry(
        &self,
        declaration: &KfdTargetDebugTelemetryPayloadV2,
        publication: &KfdTargetDebugTelemetryPayloadV2,
        direct_kfd: RocgdbDirectKfdDeviceBindingV4,
        inferior: RocgdbInferiorBindingV4,
        code: RocgdbCodeObjectBindingV4,
    ) -> Result<RocgdbMiNativeStoppedStateV4, RocgdbMiNativeCorrelationErrorV4> {
        let KfdTargetDebugTelemetryPayloadV2::DispatchDeclared {
            process_instance,
            artifact,
            dispatch,
            grid,
            workgroup,
            generation,
            ..
        } = declaration
        else {
            return Err(RocgdbMiNativeCorrelationErrorV4::DispatchSubstitution);
        };
        let KfdTargetDebugTelemetryPayloadV2::NativeDispatchPublished {
            process_instance: published_process,
            queue_occurrence,
            dispatch: published_dispatch,
            artifact: published_artifact,
            generation: published_generation,
            target_kfd_gpu_id_observation,
            target_kfd_queue_id_observation,
            target_aql_packet_id_observation,
            grid: published_grid,
            workgroup: published_workgroup,
        } = publication
        else {
            return Err(RocgdbMiNativeCorrelationErrorV4::DispatchSubstitution);
        };
        if process_instance != published_process {
            return Err(RocgdbMiNativeCorrelationErrorV4::ProcessSubstitution);
        }
        if dispatch != published_dispatch {
            return Err(RocgdbMiNativeCorrelationErrorV4::DispatchSubstitution);
        }
        if artifact.digest() != *published_artifact {
            return Err(RocgdbMiNativeCorrelationErrorV4::ArtifactSubstitution);
        }
        if generation != published_generation {
            return Err(RocgdbMiNativeCorrelationErrorV4::StaleGeneration);
        }
        if grid != published_grid || workgroup != published_workgroup {
            return Err(RocgdbMiNativeCorrelationErrorV4::GeometrySubstitution);
        }
        if *target_kfd_gpu_id_observation != direct_kfd.gpu_id {
            return Err(RocgdbMiNativeCorrelationErrorV4::DeviceSubstitution);
        }
        let opaque = |value: &KfdTargetDebugTelemetryDigestV1| {
            OpaqueIdentityV1::new(*value.as_bytes())
                .map_err(|_| RocgdbMiNativeCorrelationErrorV4::IdentityCollision)
        };
        self.correlate(
            KfdPublishedDispatchBindingV4 {
                process_instance: opaque(process_instance)?,
                queue_occurrence: opaque(queue_occurrence)?,
                generation: *generation,
                gpu_id: *target_kfd_gpu_id_observation,
                queue_id: *target_kfd_queue_id_observation,
                packet_id: *target_aql_packet_id_observation,
                artifact: LiveGpuContentIdentityV3 {
                    digest: opaque(published_artifact)?,
                    canonical_bytes: artifact.byte_length(),
                },
                dispatch: opaque(dispatch)?,
                grid: *grid,
                workgroup: *workgroup,
            },
            inferior,
            code,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn correlate(
        &self,
        kfd: KfdPublishedDispatchBindingV4,
        inferior: RocgdbInferiorBindingV4,
        code: RocgdbCodeObjectBindingV4,
    ) -> Result<RocgdbMiNativeStoppedStateV4, RocgdbMiNativeCorrelationErrorV4> {
        if kfd.generation == 0 || inferior.expected_generation != kfd.generation {
            return Err(RocgdbMiNativeCorrelationErrorV4::StaleGeneration);
        }
        if inferior.process_instance != kfd.process_instance {
            return Err(RocgdbMiNativeCorrelationErrorV4::ProcessSubstitution);
        }
        if code.artifact != kfd.artifact {
            return Err(RocgdbMiNativeCorrelationErrorV4::ArtifactSubstitution);
        }
        validate_code_binding(code)?;
        validate_geometry(kfd.grid, kfd.workgroup)?;
        let stop = self
            .stop
            .ok_or(RocgdbMiNativeCorrelationErrorV4::StopSubstitution)?;

        let agent = unique(self.agents.iter().filter(|item| item.gpu_id == kfd.gpu_id))
            .map_err(|_| RocgdbMiNativeCorrelationErrorV4::DeviceSubstitution)?;
        let queue = unique(
            self.queues
                .iter()
                .filter(|item| item.queue_id == kfd.queue_id),
        )
        .map_err(|_| RocgdbMiNativeCorrelationErrorV4::QueueSubstitution)?;
        if queue.id.inferior != agent.id.inferior
            || queue.target_agent != agent.id.local
            || queue.target_queue != queue.id.local
        {
            return Err(RocgdbMiNativeCorrelationErrorV4::HierarchySubstitution);
        }
        let dispatch = unique(
            self.dispatches
                .iter()
                .filter(|item| item.packet_id == kfd.packet_id),
        )
        .map_err(|_| RocgdbMiNativeCorrelationErrorV4::PacketSubstitution)?;
        if dispatch.id.inferior != queue.id.inferior
            || dispatch.target_agent != queue.target_agent
            || dispatch.target_queue != queue.target_queue
            || dispatch.target_dispatch != dispatch.id.local
        {
            return Err(RocgdbMiNativeCorrelationErrorV4::HierarchySubstitution);
        }
        if dispatch.grid != kfd.grid || dispatch.workgroup != kfd.workgroup {
            return Err(RocgdbMiNativeCorrelationErrorV4::GeometrySubstitution);
        }
        let thread = unique(self.threads.iter().filter(|item| {
            item.target_agent == dispatch.target_agent
                && item.target_queue == dispatch.target_queue
                && item.target_dispatch == dispatch.target_dispatch
        }))
        .map_err(|_| RocgdbMiNativeCorrelationErrorV4::ThreadSubstitution)?;
        let actual_workgroup = validate_workgroup_scope(thread, kfd.grid, kfd.workgroup)?;
        let wave_width = self.lanes.len();
        if !matches!(wave_width, 32 | 64) {
            return Err(RocgdbMiNativeCorrelationErrorV4::CountOutOfRange);
        }
        // `validate_geometry` already bounds the nominal workgroup to 1024;
        // keep this derived edge extent independently checked.
        let actual_volume = actual_workgroup
            .into_iter()
            .try_fold(1_u32, u32::checked_mul)
            .ok_or(RocgdbMiNativeCorrelationErrorV4::GeometrySubstitution)?;
        let wave_start = thread
            .wave_in_workgroup
            .checked_mul(u32::try_from(wave_width).expect("bounded wave width"))
            .filter(|start| *start < actual_volume)
            .ok_or(RocgdbMiNativeCorrelationErrorV4::ThreadSubstitution)?;
        let mut lanes = self.lanes.iter().collect::<Vec<_>>();
        lanes.sort_by_key(|lane| lane.lane);
        let mut output_lanes = Vec::with_capacity(wave_width);
        for (expected, lane) in lanes.into_iter().enumerate() {
            if usize::from(lane.lane) != expected
                || lane.target_agent != thread.target_agent
                || lane.target_queue != thread.target_queue
                || lane.target_dispatch != thread.target_dispatch
                || lane.wave != thread.wave
                || lane.workgroup != thread.workgroup
            {
                return Err(RocgdbMiNativeCorrelationErrorV4::LaneSubstitution);
            }
            let linear = linear_workitem(lane.workitem, actual_workgroup)?;
            let expected_linear = wave_start
                .checked_add(u32::from(lane.lane))
                .ok_or(RocgdbMiNativeCorrelationErrorV4::LaneSubstitution)?;
            if linear != expected_linear
                || lane.state == LaneStateV4::Active && expected_linear >= actual_volume
            {
                return Err(RocgdbMiNativeCorrelationErrorV4::LaneSubstitution);
            }
            let active = match lane.state {
                LaneStateV4::Active => observed(true, lane.evidence),
                LaneStateV4::Inactive => observed(false, lane.evidence),
                LaneStateV4::Unavailable => unavailable(LiveGpuUnavailableReasonV3::NotObserved),
            };
            output_lanes.push(RocgdbMiNativeLaneV4 {
                lane_identity: self.derive(b"lane", &[&lane.evidence.as_bytes()])?,
                lane_index: lane.lane,
                workitem: RocgdbMiWorkitemCoordinateV4 {
                    x: lane.workitem[0],
                    y: lane.workitem[1],
                    z: lane.workitem[2],
                },
                active,
            });
        }
        let kernel_entry = code
            .load_base
            .checked_add(code.entry_address)
            .ok_or(RocgdbMiNativeCorrelationErrorV4::ArtifactSubstitution)?;
        let kernel_end = kernel_entry
            .checked_add(code.entry_size)
            .ok_or(RocgdbMiNativeCorrelationErrorV4::ArtifactSubstitution)?;
        let relative_pc = match thread.frame_address {
            Some(address) if address >= kernel_entry && address < kernel_end => inferred(
                LiveGpuRelativePcV3 {
                    kernel_entry_byte_offset: address - kernel_entry,
                },
                self.derive(
                    b"relative-pc",
                    &[
                        &thread.evidence.as_bytes(),
                        &stop.as_bytes(),
                        &code.artifact.digest.as_bytes(),
                        &code.artifact.canonical_bytes.to_le_bytes(),
                        &code.load_base.to_le_bytes(),
                        &code.entry_address.to_le_bytes(),
                        &code.entry_size.to_le_bytes(),
                    ],
                )?,
            ),
            Some(_) => unavailable(LiveGpuUnavailableReasonV3::OutsideCaptureScope),
            None => unavailable(LiveGpuUnavailableReasonV3::NotCaptured),
        };
        // V4 has no exact artifact + relative-PC authenticated source-map authority yet.
        let source = unavailable(LiveGpuUnavailableReasonV3::NotCaptured);
        let association = self.derive(
            b"association",
            &[
                &kfd.process_instance.as_bytes(),
                &kfd.queue_occurrence.as_bytes(),
                &kfd.dispatch.as_bytes(),
                &agent.evidence.as_bytes(),
                &queue.evidence.as_bytes(),
                &dispatch.evidence.as_bytes(),
                &thread.evidence.as_bytes(),
                &stop.as_bytes(),
            ],
        )?;
        let output = RocgdbMiNativeStoppedStateV4 {
            association_identity: association,
            queue_occurrence_identity: self.derive(
                b"queue-occurrence-redacted",
                &[&kfd.queue_occurrence.as_bytes()],
            )?,
            process_instance_identity: self.derive(
                b"process-instance-redacted",
                &[&kfd.process_instance.as_bytes()],
            )?,
            dispatch_identity: kfd.dispatch,
            artifact: kfd.artifact,
            grid: kfd.grid,
            workgroup: kfd.workgroup,
            workgroup_coordinate: RocgdbMiWorkgroupCoordinateV4 {
                x: thread.workgroup[0],
                y: thread.workgroup[1],
                z: thread.workgroup[2],
            },
            wave_identity: self.derive(b"wave", &[&thread.evidence.as_bytes()])?,
            wave_in_workgroup: thread.wave_in_workgroup,
            lanes: output_lanes,
            relative_pc,
            source,
            registers: unavailable(LiveGpuUnavailableReasonV3::NotCaptured),
            memory: unavailable(LiveGpuUnavailableReasonV3::NotCaptured),
            origins: vec![
                fe2o3_debug_protocol::RocgdbMiNativeCorrelationOriginV4::TargetKfdPublicationObservation,
                fe2o3_debug_protocol::RocgdbMiNativeCorrelationOriginV4::RocgdbStructuredObservation,
                fe2o3_debug_protocol::RocgdbMiNativeCorrelationOriginV4::ExplicitCodeObjectAdmission,
                fe2o3_debug_protocol::RocgdbMiNativeCorrelationOriginV4::Correlated,
            ],
        };
        output
            .validate()
            .map_err(|_| RocgdbMiNativeCorrelationErrorV4::ProtocolRejected)?;
        Ok(output)
    }

    fn evidence(
        &self,
        domain: &[u8],
        line: &[u8],
    ) -> Result<OpaqueIdentityV1, RocgdbMiNativeCorrelationErrorV4> {
        self.derive(domain, &[line])
    }

    fn derive(
        &self,
        domain: &[u8],
        fields: &[&[u8]],
    ) -> Result<OpaqueIdentityV1, RocgdbMiNativeCorrelationErrorV4> {
        let mut digest = Sha256::new();
        digest.update(b"fe2o3-rocgdb-kfd-native-correlation-v4\0");
        digest.update(self.session.as_bytes());
        digest.update((domain.len() as u64).to_le_bytes());
        digest.update(domain);
        for field in fields {
            digest.update((field.len() as u64).to_le_bytes());
            digest.update(field);
        }
        OpaqueIdentityV1::new(digest.finalize().into())
            .map_err(|_| RocgdbMiNativeCorrelationErrorV4::IdentityCollision)
    }
}

fn result_record(line: &[u8]) -> Result<MiResultsV3, RocgdbMiNativeCorrelationErrorV4> {
    match parse_mi_record_v3(line, MiParserLimitsV3::default())
        .map_err(|_| RocgdbMiNativeCorrelationErrorV4::InvalidMi)?
    {
        MiRecordV3::Result { class, results, .. } if class == "done" => Ok(results),
        MiRecordV3::Result { .. } => Err(RocgdbMiNativeCorrelationErrorV4::BackendRejected),
        MiRecordV3::Async { .. } | MiRecordV3::Stream { .. } | MiRecordV3::Prompt => {
            Err(RocgdbMiNativeCorrelationErrorV4::NotResult)
        }
    }
}

fn required<'a>(
    fields: &'a MiResultsV3,
    name: &'static str,
) -> Result<&'a MiValueV3, RocgdbMiNativeCorrelationErrorV4> {
    fields
        .get(name)
        .ok_or(RocgdbMiNativeCorrelationErrorV4::MissingField(name))
}

fn constant<'a>(
    fields: &'a MiResultsV3,
    name: &'static str,
) -> Result<&'a [u8], RocgdbMiNativeCorrelationErrorV4> {
    required(fields, name)?
        .as_const()
        .ok_or(RocgdbMiNativeCorrelationErrorV4::InvalidField(name))
}

fn optional_constant<'a>(fields: &'a MiResultsV3, name: &str) -> Option<&'a [u8]> {
    fields.get(name).and_then(MiValueV3::as_const)
}

fn tuple_list<'a>(
    value: &'a MiValueV3,
    field: &'static str,
) -> Result<Vec<&'a MiResultsV3>, RocgdbMiNativeCorrelationErrorV4> {
    match value {
        MiValueV3::List(MiListV3::Values(values)) => values
            .iter()
            .map(|value| {
                value
                    .as_tuple()
                    .ok_or(RocgdbMiNativeCorrelationErrorV4::InvalidField(field))
            })
            .collect(),
        MiValueV3::List(MiListV3::Results(results)) => results
            .iter()
            .map(|(_, value)| {
                value
                    .as_tuple()
                    .ok_or(RocgdbMiNativeCorrelationErrorV4::InvalidField(field))
            })
            .collect(),
        MiValueV3::Const(_) | MiValueV3::Tuple(_) => {
            Err(RocgdbMiNativeCorrelationErrorV4::InvalidField(field))
        }
    }
}

fn validate_top_fields(
    fields: &MiResultsV3,
    allowed: &[&'static str],
) -> Result<(), RocgdbMiNativeCorrelationErrorV4> {
    for field in fields.keys() {
        if !allowed.contains(&field.as_str()) {
            return Err(RocgdbMiNativeCorrelationErrorV4::UnknownField("result"));
        }
    }
    Ok(())
}

fn validate_fields(
    fields: &MiResultsV3,
    allowed: &[&'static str],
    required_fields: &[&'static str],
) -> Result<(), RocgdbMiNativeCorrelationErrorV4> {
    for field in fields.keys() {
        if !allowed.contains(&field.as_str()) {
            return Err(RocgdbMiNativeCorrelationErrorV4::UnknownField("tuple"));
        }
    }
    for field in required_fields {
        required(fields, field)?;
    }
    Ok(())
}

fn dimension_constant(
    value: &[u8],
    field: &'static str,
) -> Result<[u32; 3], RocgdbMiNativeCorrelationErrorV4> {
    let inner = value
        .strip_prefix(b"[")
        .and_then(|value| value.strip_suffix(b"]"))
        .ok_or(RocgdbMiNativeCorrelationErrorV4::InvalidField(field))?;
    let values = inner.split(|byte| *byte == b',').collect::<Vec<_>>();
    if values.is_empty() || values.len() > 3 {
        return Err(RocgdbMiNativeCorrelationErrorV4::InvalidField(field));
    }
    let mut result = [1_u32; 3];
    for (index, value) in values.iter().enumerate() {
        result[index] = parse_decimal::<u32>(value)
            .filter(|value| *value != 0)
            .ok_or(RocgdbMiNativeCorrelationErrorV4::InvalidField(field))?;
    }
    Ok(result)
}

fn validate_deployed_fence_v4(value: &[u8]) -> Result<(), RocgdbMiNativeCorrelationErrorV4> {
    if value.is_empty() {
        return Ok(());
    }
    let mut previous = None;
    for token in value.split(|byte| *byte == b'|') {
        let rank = match token {
            b"B" => 0,
            b"Aa" | b"As" => 1,
            b"Ra" | b"Rs" => 2,
            _ => return Err(RocgdbMiNativeCorrelationErrorV4::InvalidField("fence")),
        };
        if previous.is_some_and(|previous| previous >= rank) {
            return Err(RocgdbMiNativeCorrelationErrorV4::InvalidField("fence"));
        }
        previous = Some(rank);
    }
    Ok(())
}

fn validate_deployed_address_spaces_v4(
    value: &[u8],
) -> Result<(), RocgdbMiNativeCorrelationErrorV4> {
    let value =
        value
            .strip_prefix(b"Shared(")
            .ok_or(RocgdbMiNativeCorrelationErrorV4::InvalidField(
                "address-spaces",
            ))?;
    let close = value.iter().position(|byte| *byte == b')').ok_or(
        RocgdbMiNativeCorrelationErrorV4::InvalidField("address-spaces"),
    )?;
    let (shared, suffix) = value.split_at(close);
    let private = suffix
        .strip_prefix(b"), Private(")
        .and_then(|value| value.strip_suffix(b")"))
        .ok_or(RocgdbMiNativeCorrelationErrorV4::InvalidField(
            "address-spaces",
        ))?;
    for size in [shared, private] {
        parse_decimal::<u64>(size).ok_or(RocgdbMiNativeCorrelationErrorV4::InvalidField(
            "address-spaces",
        ))?;
    }
    Ok(())
}

fn validate_geometry(
    grid: [u32; 3],
    workgroup: [u32; 3],
) -> Result<(), RocgdbMiNativeCorrelationErrorV4> {
    if grid.contains(&0)
        || workgroup.contains(&0)
        || grid
            .iter()
            .zip(workgroup)
            .any(|(grid, workgroup)| *grid < workgroup)
    {
        return Err(RocgdbMiNativeCorrelationErrorV4::GeometrySubstitution);
    }
    workgroup
        .into_iter()
        .try_fold(1_u32, u32::checked_mul)
        .filter(|volume| *volume <= 1_024)
        .ok_or(RocgdbMiNativeCorrelationErrorV4::GeometrySubstitution)?;
    Ok(())
}

fn validate_code_binding(
    code: RocgdbCodeObjectBindingV4,
) -> Result<(), RocgdbMiNativeCorrelationErrorV4> {
    let entry = code
        .load_base
        .checked_add(code.entry_address)
        .ok_or(RocgdbMiNativeCorrelationErrorV4::ArtifactSubstitution)?;
    entry
        .checked_add(code.entry_size)
        .ok_or(RocgdbMiNativeCorrelationErrorV4::ArtifactSubstitution)?;
    if code.artifact.canonical_bytes == 0 || code.entry_size == 0 {
        return Err(RocgdbMiNativeCorrelationErrorV4::ArtifactSubstitution);
    }
    Ok(())
}

fn validate_workgroup_scope(
    thread: &ThreadV4,
    grid: [u32; 3],
    workgroup: [u32; 3],
) -> Result<[u32; 3], RocgdbMiNativeCorrelationErrorV4> {
    let mut actual = [0_u32; 3];
    for axis in 0..3 {
        let start = thread.workgroup[axis]
            .checked_mul(workgroup[axis])
            .ok_or(RocgdbMiNativeCorrelationErrorV4::ThreadSubstitution)?;
        if start >= grid[axis] {
            return Err(RocgdbMiNativeCorrelationErrorV4::ThreadSubstitution);
        }
        actual[axis] = workgroup[axis].min(grid[axis] - start);
    }
    Ok(actual)
}

fn linear_workitem(
    workitem: [u32; 3],
    workgroup: [u32; 3],
) -> Result<u32, RocgdbMiNativeCorrelationErrorV4> {
    if workitem[0] >= workgroup[0] || workitem[1] >= workgroup[1] {
        return Err(RocgdbMiNativeCorrelationErrorV4::LaneSubstitution);
    }
    workitem[2]
        .checked_mul(workgroup[1])
        .and_then(|value| value.checked_add(workitem[1]))
        .and_then(|value| value.checked_mul(workgroup[0]))
        .and_then(|value| value.checked_add(workitem[0]))
        .ok_or(RocgdbMiNativeCorrelationErrorV4::LaneSubstitution)
}

fn parse_qualified_id(bytes: &[u8]) -> Result<QualifiedIdV4, RocgdbMiNativeCorrelationErrorV4> {
    let Some(dot) = bytes.iter().position(|byte| *byte == b'.') else {
        return Err(RocgdbMiNativeCorrelationErrorV4::InvalidField(
            "qualified id",
        ));
    };
    if bytes[dot + 1..].contains(&b'.') {
        return Err(RocgdbMiNativeCorrelationErrorV4::InvalidField(
            "qualified id",
        ));
    }
    let inferior = parse_decimal::<u32>(&bytes[..dot]).ok_or(
        RocgdbMiNativeCorrelationErrorV4::InvalidField("qualified id"),
    )?;
    let local = parse_decimal::<u32>(&bytes[dot + 1..]).ok_or(
        RocgdbMiNativeCorrelationErrorV4::InvalidField("qualified id"),
    )?;
    if inferior == 0 || local == 0 {
        return Err(RocgdbMiNativeCorrelationErrorV4::InvalidField(
            "qualified id",
        ));
    }
    Ok(QualifiedIdV4 {
        raw: bytes.to_vec(),
        inferior,
        local,
    })
}

fn parse_agent_target_id(bytes: &[u8]) -> Result<u32, RocgdbMiNativeCorrelationErrorV4> {
    parse_wrapped_decimal(bytes, b"AMDGPU Agent (GPUID ", b")")
}

fn parse_queue_target_id(
    bytes: &[u8],
) -> Result<(u32, u32, u32), RocgdbMiNativeCorrelationErrorV4> {
    let mut parser = TargetIdParserV4::new(bytes, b"AMDGPU Queue ")?;
    let agent = parser.decimal()?;
    parser.literal(b":")?;
    let queue = parser.decimal()?;
    parser.literal(b" (QID ")?;
    let qid = parser.decimal()?;
    parser.finish(b")")?;
    Ok((agent, queue, qid))
}

fn parse_dispatch_target_id(
    bytes: &[u8],
) -> Result<(u32, u32, u32, u64), RocgdbMiNativeCorrelationErrorV4> {
    let mut parser = TargetIdParserV4::new(bytes, b"AMDGPU Dispatch ")?;
    let agent = parser.decimal()?;
    parser.literal(b":")?;
    let queue = parser.decimal()?;
    parser.literal(b":")?;
    let dispatch = parser.decimal()?;
    parser.literal(b" (PKID ")?;
    let packet = parser.decimal::<u64>()?;
    parser.finish(b")")?;
    Ok((agent, queue, dispatch, packet))
}

fn parse_thread_target_id(
    bytes: &[u8],
) -> Result<ParsedThreadTargetV4, RocgdbMiNativeCorrelationErrorV4> {
    let mut parser = TargetIdParserV4::new(bytes, b"AMDGPU Wave ")?;
    let agent = parser.decimal()?;
    parser.literal(b":")?;
    let queue = parser.decimal()?;
    parser.literal(b":")?;
    let dispatch = parser.decimal()?;
    parser.literal(b":")?;
    let wave = parser.decimal()?;
    parser.literal(b" (")?;
    let workgroup = parser.triplet(b')')?;
    parser.literal(b"/")?;
    let index = parser.decimal()?;
    parser.finish(b"")?;
    Ok(ParsedThreadTargetV4 {
        agent,
        queue,
        dispatch,
        wave,
        workgroup,
        wave_in_workgroup: index,
    })
}

type ParsedLaneTargetV4 = (u32, u32, u32, u32, u32, [u32; 3], [u32; 3]);

fn parse_lane_target_id(
    bytes: &[u8],
) -> Result<ParsedLaneTargetV4, RocgdbMiNativeCorrelationErrorV4> {
    let mut parser = TargetIdParserV4::new(bytes, b"AMDGPU Lane ")?;
    let agent = parser.decimal()?;
    parser.literal(b":")?;
    let queue = parser.decimal()?;
    parser.literal(b":")?;
    let dispatch = parser.decimal()?;
    parser.literal(b":")?;
    let wave = parser.decimal()?;
    parser.literal(b"/")?;
    let lane = parser.decimal()?;
    parser.literal(b" (")?;
    let workgroup = parser.triplet(b')')?;
    parser.literal(b"[")?;
    let workitem = parser.triplet(b']')?;
    parser.finish(b"")?;
    Ok((agent, queue, dispatch, wave, lane, workgroup, workitem))
}

struct TargetIdParserV4<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> TargetIdParserV4<'a> {
    fn new(bytes: &'a [u8], prefix: &[u8]) -> Result<Self, RocgdbMiNativeCorrelationErrorV4> {
        if bytes.len() > MAX_TARGET_ID_BYTES_V4 || !bytes.starts_with(prefix) {
            return Err(RocgdbMiNativeCorrelationErrorV4::InvalidTargetId);
        }
        Ok(Self {
            bytes,
            cursor: prefix.len(),
        })
    }

    fn literal(&mut self, literal: &[u8]) -> Result<(), RocgdbMiNativeCorrelationErrorV4> {
        if !self.bytes[self.cursor..].starts_with(literal) {
            return Err(RocgdbMiNativeCorrelationErrorV4::InvalidTargetId);
        }
        self.cursor += literal.len();
        Ok(())
    }

    fn decimal<T: std::str::FromStr>(&mut self) -> Result<T, RocgdbMiNativeCorrelationErrorV4> {
        let start = self.cursor;
        while self.bytes.get(self.cursor).is_some_and(u8::is_ascii_digit) {
            self.cursor += 1;
        }
        parse_decimal(&self.bytes[start..self.cursor])
            .ok_or(RocgdbMiNativeCorrelationErrorV4::InvalidTargetId)
    }

    fn triplet(&mut self, close: u8) -> Result<[u32; 3], RocgdbMiNativeCorrelationErrorV4> {
        let x = self.decimal()?;
        self.literal(b",")?;
        let y = self.decimal()?;
        self.literal(b",")?;
        let z = self.decimal()?;
        if self.bytes.get(self.cursor) != Some(&close) {
            return Err(RocgdbMiNativeCorrelationErrorV4::InvalidTargetId);
        }
        self.cursor += 1;
        Ok([x, y, z])
    }

    fn finish(&mut self, suffix: &[u8]) -> Result<(), RocgdbMiNativeCorrelationErrorV4> {
        self.literal(suffix)?;
        if self.cursor != self.bytes.len() {
            return Err(RocgdbMiNativeCorrelationErrorV4::InvalidTargetId);
        }
        Ok(())
    }
}

fn parse_wrapped_decimal(
    bytes: &[u8],
    prefix: &[u8],
    suffix: &[u8],
) -> Result<u32, RocgdbMiNativeCorrelationErrorV4> {
    let mut parser = TargetIdParserV4::new(bytes, prefix)?;
    let value = parser.decimal()?;
    parser.finish(suffix)?;
    Ok(value)
}

fn parse_decimal<T: std::str::FromStr>(bytes: &[u8]) -> Option<T> {
    if bytes.is_empty()
        || (bytes.len() > 1 && bytes[0] == b'0')
        || !bytes.iter().all(u8::is_ascii_digit)
    {
        return None;
    }
    std::str::from_utf8(bytes).ok()?.parse().ok()
}

fn parse_hex_u64(bytes: &[u8]) -> Option<u64> {
    let digits = bytes.strip_prefix(b"0x")?;
    if digits.is_empty()
        || digits.len() > 16
        || !digits
            .iter()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        || (digits.len() != 16 && digits.len() > 1 && digits[0] == b'0')
    {
        return None;
    }
    u64::from_str_radix(std::str::from_utf8(digits).ok()?, 16).ok()
}

fn validate_native_token(
    token: &[u8],
    field: &'static str,
) -> Result<(), RocgdbMiNativeCorrelationErrorV4> {
    if token.is_empty()
        || token.len() > 128
        || !token
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
    {
        return Err(RocgdbMiNativeCorrelationErrorV4::InvalidField(field));
    }
    Ok(())
}

fn require_count(count: usize) -> Result<(), RocgdbMiNativeCorrelationErrorV4> {
    if count > MAX_ENTITIES_V4 {
        Err(RocgdbMiNativeCorrelationErrorV4::CountOutOfRange)
    } else {
        Ok(())
    }
}

fn reject_duplicate<I, T>(values: I) -> Result<(), RocgdbMiNativeCorrelationErrorV4>
where
    I: IntoIterator<Item = T>,
    T: Ord,
{
    let mut seen = BTreeSet::new();
    for value in values {
        if !seen.insert(value) {
            return Err(RocgdbMiNativeCorrelationErrorV4::DuplicateEntity);
        }
    }
    Ok(())
}

fn unique<T>(mut values: impl Iterator<Item = T>) -> Result<T, ()> {
    let first = values.next().ok_or(())?;
    if values.next().is_some() {
        return Err(());
    }
    Ok(first)
}

fn observed<T>(value: T, evidence: OpaqueIdentityV1) -> LiveGpuAvailabilityV3<T> {
    LiveGpuAvailabilityV3::Available {
        value,
        truth: LiveGpuTruthV3 {
            origin: LiveGpuTruthOriginV3::Observed,
            evidence: vec![LiveGpuEvidenceRefV3 {
                kind: LiveGpuEvidenceKindV3::RuntimeObservation,
                identity: evidence,
            }],
        },
    }
}

fn inferred<T>(value: T, evidence: OpaqueIdentityV1) -> LiveGpuAvailabilityV3<T> {
    LiveGpuAvailabilityV3::Available {
        value,
        truth: LiveGpuTruthV3 {
            origin: LiveGpuTruthOriginV3::Inferred,
            evidence: vec![LiveGpuEvidenceRefV3 {
                kind: LiveGpuEvidenceKindV3::InferenceRule,
                identity: evidence,
            }],
        },
    }
}

fn unavailable<T>(reason: LiveGpuUnavailableReasonV3) -> LiveGpuAvailabilityV3<T> {
    LiveGpuAvailabilityV3::Unavailable {
        reason,
        truth: LiveGpuTruthV3 {
            origin: LiveGpuTruthOriginV3::Unavailable,
            evidence: Vec::new(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(seed: u8) -> OpaqueIdentityV1 {
        OpaqueIdentityV1::new([seed; 32]).unwrap()
    }

    fn fixture(lanes: usize) -> RocgdbMiNativeCorrelationAdapterV4 {
        let mut adapter = RocgdbMiNativeCorrelationAdapterV4::new(identity(1));
        adapter.bind_stop_identity_v4(identity(9)).unwrap();
        adapter
            .admit_agent_info(b"1^done,agents=[{id=\"1.1\",state=\"A\",target-id=\"AMDGPU Agent (GPUID 35090)\",architecture=\"gfx942\",name=\"MI300X\",cores=\"304\",threads=\"19456\",location=\"0000:01:00.0\"}]\n")
            .unwrap();
        adapter
            .admit_queue_info(b"2^done,queues=[{id=\"1.2\",target-id=\"AMDGPU Queue 1:2 (QID 7)\",type=\"HSA\",read=\"41\",write=\"42\",size=\"4096\",addr=\"0x0000000000001000\"}]\n")
            .unwrap();
        adapter
            .admit_dispatch_info(b"3^done,dispatches=[{id=\"1.3\",target-id=\"AMDGPU Dispatch 1:2:3 (PKID 41)\",grid=\"[64,1,1]\",workgroup=\"[64,1,1]\",fence=\"B|As|Rs\",address-spaces=\"Shared(0), Private(0)\",kernel-desc=\"0x0000000000002000\",kernel-args=\"0x0000000000003000\",completion=\"0x0000000000000000\",kernel-function=\"0x0000000000001000\"}]\n")
            .unwrap();
        adapter
            .admit_thread_info(b"4^done,threads=[{id=\"9\",target-id=\"AMDGPU Wave 1:2:3:4 (0,0,0)/0\",frame={level=\"0\",addr=\"0x0000000000001010\",func=\"kernel\",args=[]},state=\"stopped\"}],current-thread-id=\"9\"\n")
            .unwrap();
        let lane_tuples = (0..lanes)
            .map(|lane| {
                format!(
                    "{{id=\"{lane}\",state=\"{}\",target-id=\"AMDGPU Lane 1:2:3:4/{lane} (0,0,0)[{lane},0,0]\"}}",
                    if lane % 2 == 0 { "A" } else { "I" }
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        adapter
            .admit_lane_info(format!("5^done,lanes=[{lane_tuples}]\n").as_bytes())
            .unwrap();
        adapter
    }

    fn kfd() -> KfdPublishedDispatchBindingV4 {
        KfdPublishedDispatchBindingV4 {
            process_instance: identity(2),
            queue_occurrence: identity(3),
            generation: 7,
            gpu_id: 35_090,
            queue_id: 7,
            packet_id: 41,
            artifact: LiveGpuContentIdentityV3 {
                digest: identity(4),
                canonical_bytes: 0x100,
            },
            dispatch: identity(5),
            grid: [64, 1, 1],
            workgroup: [64, 1, 1],
        }
    }

    fn declared_v2() -> KfdTargetDebugTelemetryPayloadV2 {
        KfdTargetDebugTelemetryPayloadV2::DispatchDeclared {
            process_instance: KfdTargetDebugTelemetryDigestV1::from_bytes([2; 32]).unwrap(),
            executable: fe2o3_kfd::KfdTargetDebugArtifactIdentityV1::new(
                KfdTargetDebugTelemetryDigestV1::from_bytes([8; 32]).unwrap(),
                64,
            )
            .unwrap(),
            artifact: fe2o3_kfd::KfdTargetDebugArtifactIdentityV1::new(
                KfdTargetDebugTelemetryDigestV1::from_bytes([4; 32]).unwrap(),
                0x100,
            )
            .unwrap(),
            dispatch: KfdTargetDebugTelemetryDigestV1::from_bytes([5; 32]).unwrap(),
            kernel: KfdTargetDebugTelemetryDigestV1::from_bytes([6; 32]).unwrap(),
            logical_queue: KfdTargetDebugTelemetryDigestV1::from_bytes([7; 32]).unwrap(),
            grid: [64, 1, 1],
            workgroup: [64, 1, 1],
            dynamic_shared_memory_bytes: 0,
            generation: 7,
        }
    }

    fn published_v2() -> KfdTargetDebugTelemetryPayloadV2 {
        KfdTargetDebugTelemetryPayloadV2::NativeDispatchPublished {
            process_instance: KfdTargetDebugTelemetryDigestV1::from_bytes([2; 32]).unwrap(),
            queue_occurrence: KfdTargetDebugTelemetryDigestV1::from_bytes([3; 32]).unwrap(),
            dispatch: KfdTargetDebugTelemetryDigestV1::from_bytes([5; 32]).unwrap(),
            artifact: KfdTargetDebugTelemetryDigestV1::from_bytes([4; 32]).unwrap(),
            generation: 7,
            target_kfd_gpu_id_observation: 35_090,
            target_kfd_queue_id_observation: 7,
            target_aql_packet_id_observation: 41,
            grid: [64, 1, 1],
            workgroup: [64, 1, 1],
        }
    }

    fn correlate(
        adapter: &RocgdbMiNativeCorrelationAdapterV4,
        kfd: KfdPublishedDispatchBindingV4,
    ) -> Result<RocgdbMiNativeStoppedStateV4, RocgdbMiNativeCorrelationErrorV4> {
        adapter.correlate(
            kfd,
            RocgdbInferiorBindingV4 {
                process_instance: identity(2),
                expected_generation: 7,
            },
            RocgdbCodeObjectBindingV4 {
                artifact: kfd.artifact,
                load_base: 0x1000,
                entry_address: 0,
                entry_size: 0x100,
            },
        )
    }

    #[test]
    fn exact_hierarchy_correlates_and_redacts_native_identifiers() {
        let native = kfd();
        let output = correlate(&fixture(64), native).unwrap();
        assert_eq!(output.lanes.len(), 64);
        assert!(matches!(
            output.lanes[0].active,
            LiveGpuAvailabilityV3::Available { value: true, .. }
        ));
        assert!(matches!(
            output.lanes[1].active,
            LiveGpuAvailabilityV3::Available { value: false, .. }
        ));
        let json = serde_json::to_string(&output).unwrap();
        assert_ne!(output.process_instance_identity, native.process_instance);
        for private in [
            "35090",
            "\"queue_id\"",
            "\"packet_id\"",
            "0x0000000000001010",
            "0x0000000000001000",
        ] {
            assert!(
                !json.contains(private),
                "leaked private field {private}: {json}"
            );
        }
    }

    #[test]
    fn public_query_joins_exact_v2_records_and_rejects_coherent_resealing() {
        let adapter = fixture(64);
        let inferior = RocgdbInferiorBindingV4::new(identity(2), 7).unwrap();
        let code = RocgdbCodeObjectBindingV4::new(
            LiveGpuContentIdentityV3 {
                digest: identity(4),
                canonical_bytes: 0x100,
            },
            0x1000,
            0,
            0x100,
        )
        .unwrap();
        assert!(
            adapter
                .correlate_telemetry(
                    &declared_v2(),
                    &published_v2(),
                    RocgdbDirectKfdDeviceBindingV4 { gpu_id: 35_090 },
                    inferior,
                    code,
                )
                .is_ok()
        );
        assert_eq!(
            adapter.correlate_telemetry(
                &declared_v2(),
                &published_v2(),
                RocgdbDirectKfdDeviceBindingV4 { gpu_id: 1 },
                inferior,
                code,
            ),
            Err(RocgdbMiNativeCorrelationErrorV4::DeviceSubstitution)
        );
        let KfdTargetDebugTelemetryPayloadV2::NativeDispatchPublished {
            queue_occurrence,
            dispatch,
            artifact,
            generation,
            target_kfd_gpu_id_observation,
            target_kfd_queue_id_observation,
            target_aql_packet_id_observation,
            grid,
            workgroup,
            ..
        } = published_v2()
        else {
            unreachable!()
        };
        let changed_process = KfdTargetDebugTelemetryPayloadV2::NativeDispatchPublished {
            process_instance: KfdTargetDebugTelemetryDigestV1::from_bytes([99; 32]).unwrap(),
            queue_occurrence,
            dispatch,
            artifact,
            generation,
            target_kfd_gpu_id_observation,
            target_kfd_queue_id_observation,
            target_aql_packet_id_observation,
            grid,
            workgroup,
        };
        assert_eq!(
            adapter.correlate_telemetry(
                &declared_v2(),
                &changed_process,
                RocgdbDirectKfdDeviceBindingV4 { gpu_id: 35_090 },
                RocgdbInferiorBindingV4::new(identity(2), 7).unwrap(),
                RocgdbCodeObjectBindingV4::new(
                    LiveGpuContentIdentityV3 {
                        digest: identity(4),
                        canonical_bytes: 0x100,
                    },
                    0x1000,
                    0,
                    0x100,
                )
                .unwrap(),
            ),
            Err(RocgdbMiNativeCorrelationErrorV4::ProcessSubstitution)
        );
    }

    #[test]
    fn rejects_every_publication_identity_axis_substitution() {
        let adapter = fixture(64);
        let base = kfd();
        let cases = [
            (
                KfdPublishedDispatchBindingV4 { gpu_id: 1, ..base },
                RocgdbMiNativeCorrelationErrorV4::DeviceSubstitution,
            ),
            (
                KfdPublishedDispatchBindingV4 {
                    queue_id: 8,
                    ..base
                },
                RocgdbMiNativeCorrelationErrorV4::QueueSubstitution,
            ),
            (
                KfdPublishedDispatchBindingV4 {
                    grid: [65, 1, 1],
                    ..base
                },
                RocgdbMiNativeCorrelationErrorV4::GeometrySubstitution,
            ),
            (
                KfdPublishedDispatchBindingV4 {
                    workgroup: [32, 1, 1],
                    ..base
                },
                RocgdbMiNativeCorrelationErrorV4::GeometrySubstitution,
            ),
        ];
        for (changed, expected) in cases {
            assert_eq!(correlate(&adapter, changed), Err(expected));
        }
        let wrong_code = adapter.correlate(
            base,
            RocgdbInferiorBindingV4 {
                process_instance: identity(2),
                expected_generation: 7,
            },
            RocgdbCodeObjectBindingV4 {
                artifact: LiveGpuContentIdentityV3 {
                    digest: identity(99),
                    canonical_bytes: 0x100,
                },
                load_base: 0x1000,
                entry_address: 0,
                entry_size: 0x100,
            },
        );
        assert_eq!(
            wrong_code,
            Err(RocgdbMiNativeCorrelationErrorV4::ArtifactSubstitution)
        );
    }

    #[test]
    fn relative_pc_evidence_binds_every_code_admission_field_and_source_stays_unavailable() {
        let adapter = fixture(64);
        let base = kfd();
        let output = |entry_address| {
            adapter
                .correlate(
                    base,
                    RocgdbInferiorBindingV4 {
                        process_instance: identity(2),
                        expected_generation: 7,
                    },
                    RocgdbCodeObjectBindingV4 {
                        artifact: base.artifact,
                        load_base: 0x1000,
                        entry_address,
                        entry_size: 0x80,
                    },
                )
                .unwrap()
        };
        let first = output(0);
        let second = output(8);
        let evidence = |value: &LiveGpuAvailabilityV3<LiveGpuRelativePcV3>| match value {
            LiveGpuAvailabilityV3::Available { truth, .. } => truth.evidence.clone(),
            _ => panic!("relative PC must be available"),
        };
        assert_ne!(evidence(&first.relative_pc), evidence(&second.relative_pc));
        assert!(matches!(
            first.source,
            LiveGpuAvailabilityV3::Unavailable {
                reason: LiveGpuUnavailableReasonV3::NotCaptured,
                ..
            }
        ));
    }

    #[test]
    fn rejects_cross_process_stale_generation_and_queue_occurrence_reuse() {
        let adapter = fixture(64);
        let base = kfd();
        assert_eq!(
            adapter.correlate(
                base,
                RocgdbInferiorBindingV4 {
                    process_instance: identity(88),
                    expected_generation: 7
                },
                RocgdbCodeObjectBindingV4 {
                    artifact: base.artifact,
                    load_base: 0x1000,
                    entry_address: 0,
                    entry_size: 0x100
                },
            ),
            Err(RocgdbMiNativeCorrelationErrorV4::ProcessSubstitution)
        );
        assert_eq!(
            adapter.correlate(
                base,
                RocgdbInferiorBindingV4 {
                    process_instance: identity(2),
                    expected_generation: 8
                },
                RocgdbCodeObjectBindingV4 {
                    artifact: base.artifact,
                    load_base: 0x1000,
                    entry_address: 0,
                    entry_size: 0x100
                },
            ),
            Err(RocgdbMiNativeCorrelationErrorV4::StaleGeneration)
        );
        let changed = KfdPublishedDispatchBindingV4 {
            queue_occurrence: identity(77),
            ..base
        };
        let first = correlate(&adapter, base).unwrap();
        let second = correlate(&adapter, changed).unwrap();
        assert_ne!(
            first.queue_occurrence_identity,
            second.queue_occurrence_identity
        );
        assert_ne!(first.association_identity, second.association_identity);
    }

    #[test]
    fn rejects_malformed_hierarchy_threads_lanes_and_stream_records() {
        let mut adapter = RocgdbMiNativeCorrelationAdapterV4::new(identity(1));
        adapter.bind_stop_identity_v4(identity(8)).unwrap();
        assert_eq!(
            adapter.bind_stop_identity_v4(identity(9)),
            Err(RocgdbMiNativeCorrelationErrorV4::StopSubstitution)
        );
        for line in [
            b"~\"AMDGPU Agent (GPUID 35090)\"\n".as_slice(),
            b"1^done,agents=[{id=\"1.1\",state=\"A\",target-id=\"AMDGPU Agent (GPUID 035090)\"}]\n",
            b"1^done,agents=[{id=\"1.1\",state=\"A\",target-id=\"AMDGPU Agent GPUID 35090\"}]\n",
        ] {
            assert!(adapter.admit_agent_info(line).is_err());
        }
        let mut bad_lane = fixture(64);
        bad_lane.lanes[1].workitem = [2, 0, 0];
        assert_eq!(
            correlate(&bad_lane, kfd()),
            Err(RocgdbMiNativeCorrelationErrorV4::LaneSubstitution)
        );
        let mut bad_thread = fixture(64);
        bad_thread.threads[0].target_dispatch = 99;
        assert_eq!(
            correlate(&bad_thread, kfd()),
            Err(RocgdbMiNativeCorrelationErrorV4::ThreadSubstitution)
        );
    }

    #[test]
    fn deployed_rocgdb_16_3_queue_and_dispatch_schema_is_exact() {
        let queue = b"2^done,queues=[{id=\"1.2\",target-id=\"AMDGPU Queue 1:2 (QID 7)\",type=\"HSA\",read=\"41\",write=\"42\",size=\"4096\",addr=\"0x00007ffff7ee0000\"}],current-queue-id=\"1.2\"\n";
        let dispatch = b"3^done,dispatches=[{id=\"1.3\",target-id=\"AMDGPU Dispatch 1:2:3 (PKID 41)\",grid=\"[1024,1,1]\",workgroup=\"[256,1,1]\",fence=\"B|As|Rs\",address-spaces=\"Shared(0), Private(0)\",kernel-desc=\"0x00007ffde7e00740\",kernel-args=\"0x00007fffeff00000\",completion=\"0x0000000000000000\",kernel-function=\"0x00007ffde7e01000\"}],current-dispatch-id=\"1.3\"\n";
        let mut adapter = RocgdbMiNativeCorrelationAdapterV4::new(identity(1));
        adapter.bind_stop_identity_v4(identity(9)).unwrap();
        adapter.admit_queue_info(queue).unwrap();
        adapter.admit_dispatch_info(dispatch).unwrap();

        let stale_documented = b"3^done,dispatches=[{id=\"1.3\",queue_id=\"1.2\",target-id=\"AMDGPU Dispatch 1:2:3 (PKID 41)\",grid=\"[1024,1,1]\",workgroup=\"[256,1,1]\",fences=[{name=\"Barrier\",abbrev=\"B\"}],address-spaces=[{name=\"Shared\",size=\"0\"},{name=\"Private\",size=\"0\"}],kernel-desc=\"0x00007ffde7e00740\",kernel-args=\"0x00007fffeff00000\",completion=\"0x0000000000000000\",kernel-function=\"0x00007ffde7e01000\"}]\n";
        let bad_dispatches = [
            stale_documented.as_slice(),
            b"3^done,dispatches=[{id=\"1.3\",target-id=\"AMDGPU Dispatch 1:2:3 (PKID 41)\",grid=[\"1024\",\"1\",\"1\"],workgroup=\"[256,1,1]\",fence=\"\",address-spaces=\"Shared(0), Private(0)\",kernel-desc=\"0x00007ffde7e00740\",kernel-args=\"0x00007fffeff00000\",completion=\"0x0000000000000000\",kernel-function=\"0x00007ffde7e01000\"}]\n",
            b"3^done,dispatches=[{id=\"1.3\",target-id=\"AMDGPU Dispatch 1:2:3 (PKID 41)\",grid=\"[1024,01,1]\",workgroup=\"[256,1,1]\",fence=\"\",address-spaces=\"Shared(0), Private(0)\",kernel-desc=\"0x00007ffde7e00740\",kernel-args=\"0x00007fffeff00000\",completion=\"0x0000000000000000\",kernel-function=\"0x00007ffde7e01000\"}]\n",
            b"3^done,dispatches=[{id=\"1.3\",target-id=\"AMDGPU Dispatch 1:2:3 (PKID 41)\",grid=\"[1024,1,1,1]\",workgroup=\"[256,1,1]\",fence=\"\",address-spaces=\"Shared(0), Private(0)\",kernel-desc=\"0x00007ffde7e00740\",kernel-args=\"0x00007fffeff00000\",completion=\"0x0000000000000000\",kernel-function=\"0x00007ffde7e01000\"}]\n",
            b"3^done,dispatches=[{id=\"1.3\",target-id=\"AMDGPU Dispatch 1:2:3 (PKID 41)\",grid=\"[1024,1,1]\",workgroup=\"[256,1,1]\",fence=\"Rs|B\",address-spaces=\"Shared(0), Private(0)\",kernel-desc=\"0x00007ffde7e00740\",kernel-args=\"0x00007fffeff00000\",completion=\"0x0000000000000000\",kernel-function=\"0x00007ffde7e01000\"}]\n",
            b"3^done,dispatches=[{id=\"1.3\",target-id=\"AMDGPU Dispatch 1:2:3 (PKID 41)\",grid=\"[1024,1,1]\",workgroup=\"[256,1,1]\",fence=\"\",address-spaces=\"Private(0), Shared(0)\",kernel-desc=\"0x00007ffde7e00740\",kernel-args=\"0x00007fffeff00000\",completion=\"0x0000000000000000\",kernel-function=\"0x00007ffde7e01000\"}]\n",
            b"3^done,dispatches=[{id=\"1.3\",target-id=\"AMDGPU Dispatch 1:2:3 (PKID 41)\",grid=\"[1024,1,1]\",workgroup=\"[256,1,1]\",fence=\"\",address-spaces=\"Shared(0), Private(0)\",kernel-desc=\"0x00007FFDE7E00740\",kernel-args=\"0x00007fffeff00000\",completion=\"0x0000000000000000\",kernel-function=\"0x00007ffde7e01000\"}]\n",
            b"3^done,dispatches=[{id=\"1.3\",target-id=\"AMDGPU Dispatch 1:2:3 (PKID 41)\",grid=\"[1024,1,1]\",workgroup=\"[256,1,1]\",fence=\"\",address-spaces=\"Shared(0), Private(0)\",kernel-desc=\"0x00007ffde7e00740\",kernel-args=\"0x00007fffeff00000\",completion=\"0x0000000000000000\",kernel-function=\"0x00007ffde7e01000\",unknown=\"x\"}]\n",
            b"3^done,dispatches=[{id=\"1.3\",id=\"1.4\",target-id=\"AMDGPU Dispatch 1:2:3 (PKID 41)\",grid=\"[1024,1,1]\",workgroup=\"[256,1,1]\",fence=\"\",address-spaces=\"Shared(0), Private(0)\",kernel-desc=\"0x00007ffde7e00740\",kernel-args=\"0x00007fffeff00000\",completion=\"0x0000000000000000\",kernel-function=\"0x00007ffde7e01000\"}]\n",
        ];
        for line in bad_dispatches {
            assert!(
                adapter.admit_dispatch_info(line).is_err(),
                "accepted {line:?}"
            );
        }

        for line in [
            b"2^done,queues=[{id=\"1.2\",agent-id=\"1.1\",target-id=\"AMDGPU Queue 1:2 (QID 7)\",type=\"HSA\",size=\"4096\",addr=\"0x00007ffff7ee0000\"}]\n".as_slice(),
            b"2^done,queues=[{id=\"1.2\",target-id=\"AMDGPU Queue 1:2 (QID 7)\",type=\"HSA\",size=\"4096\",addr=\"0x00007FFFF7EE0000\"}]\n",
            b"2^done,queues=[{id=\"1.2\",target-id=\"AMDGPU Queue 1:2 (QID 7)\",type=\"HSA\",size=\"4096\",addr=\"0x00000000000001000\"}]\n",
            b"2^done,queues=[{id=\"1.2\",target-id=\"AMDGPU Queue 1:2 (QID 7)\",type=\"HSA\",size=\"4096\",addr=\"0x00007ffff7ee0000\",unknown=\"x\"}]\n",
        ] {
            assert!(adapter.admit_queue_info(line).is_err(), "accepted {line:?}");
        }
    }

    #[test]
    fn wave_one_correlates_partial_multidimensional_edge_workgroup() {
        let mut adapter = RocgdbMiNativeCorrelationAdapterV4::new(identity(1));
        adapter.bind_stop_identity_v4(identity(9)).unwrap();
        adapter
            .admit_agent_info(b"1^done,agents=[{id=\"1.1\",state=\"A\",target-id=\"AMDGPU Agent (GPUID 35090)\",architecture=\"gfx942\",name=\"MI300X\",cores=\"304\",threads=\"19456\",location=\"0000:01:00.0\"}]\n")
            .unwrap();
        adapter
            .admit_queue_info(b"2^done,queues=[{id=\"1.2\",target-id=\"AMDGPU Queue 1:2 (QID 7)\",type=\"HSA\",read=\"41\",write=\"42\",size=\"4096\",addr=\"0x0000000000001000\"}]\n")
            .unwrap();
        adapter
            .admit_dispatch_info(b"3^done,dispatches=[{id=\"1.3\",target-id=\"AMDGPU Dispatch 1:2:3 (PKID 41)\",grid=\"[30,15,1]\",workgroup=\"[16,8,1]\",fence=\"\",address-spaces=\"Shared(0), Private(0)\",kernel-desc=\"0x0000000000002000\",kernel-args=\"0x0000000000003000\",completion=\"0x0000000000000000\",kernel-function=\"0x0000000000001000\"}]\n")
            .unwrap();
        adapter
            .admit_thread_info(b"4^done,threads=[{id=\"9\",target-id=\"AMDGPU Wave 1:2:3:4 (1,1,0)/1\",frame={level=\"0\",addr=\"0x0000000000001010\",func=\"kernel\",args=[]},state=\"stopped\"}],current-thread-id=\"9\"\n")
            .unwrap();
        let lanes = (0_u32..64)
            .map(|lane| {
                let flat = 64 + lane;
                let x = flat % 14;
                let y = (flat / 14) % 7;
                let z = flat / 98;
                let state = if flat < 98 { "A" } else { "I" };
                format!(
                    "{{id=\"{lane}\",state=\"{state}\",target-id=\"AMDGPU Lane 1:2:3:4/{lane} (1,1,0)[{x},{y},{z}]\"}}"
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        adapter
            .admit_lane_info(format!("5^done,lanes=[{lanes}]\n").as_bytes())
            .unwrap();
        let native = KfdPublishedDispatchBindingV4 {
            grid: [30, 15, 1],
            workgroup: [16, 8, 1],
            ..kfd()
        };
        let output = correlate(&adapter, native).unwrap();
        assert_eq!(output.wave_in_workgroup, 1);
        assert_eq!(
            output.workgroup_coordinate,
            RocgdbMiWorkgroupCoordinateV4 { x: 1, y: 1, z: 0 }
        );
        assert!(matches!(
            output.lanes[33].active,
            LiveGpuAvailabilityV3::Available { value: true, .. }
        ));
        assert!(matches!(
            output.lanes[34].active,
            LiveGpuAvailabilityV3::Available { value: false, .. }
        ));
    }

    #[test]
    fn enforces_exact_lane_cardinality_and_u_state_is_unavailable() {
        assert_eq!(
            correlate(&fixture(31), kfd()),
            Err(RocgdbMiNativeCorrelationErrorV4::CountOutOfRange)
        );
        let mut adapter = fixture(32);
        adapter.lanes[0].state = LaneStateV4::Unavailable;
        let output = correlate(
            &adapter,
            KfdPublishedDispatchBindingV4 {
                grid: [64, 1, 1],
                workgroup: [64, 1, 1],
                ..kfd()
            },
        )
        .unwrap();
        assert!(matches!(
            output.lanes[0].active,
            LiveGpuAvailabilityV3::Unavailable {
                reason: LiveGpuUnavailableReasonV3::NotObserved,
                ..
            }
        ));
    }
}
