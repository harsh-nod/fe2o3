use std::error::Error;
use std::fmt;

pub const TRACE_SCHEMA_VERSION_V1: u16 = 1;
pub const KERNEL_IR_WIRE_VERSION_V6: u16 = 6;
pub const KERNEL_IR_IDENTITY_POLICY_V1: u16 = 1;

pub const MAX_TRACE_BYTES_V1: u64 = 64 * 1024 * 1024;
pub const MAX_TRACE_RESIDENT_BYTES_V1: u64 = 256 * 1024 * 1024;
pub const MAX_TRACE_EVENTS_V1: u64 = 1_000_000;
pub const MAX_EVIDENCE_REFS_PER_EVENT_V1: usize = 16;
pub const MAX_PRODUCER_TEXT_BYTES_V1: usize = 128;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct OpaqueIdentityV1([u8; 32]);

impl OpaqueIdentityV1 {
    pub fn new(bytes: [u8; 32]) -> Result<Self, TraceValidationErrorV1> {
        if bytes == [0; 32] {
            return Err(TraceValidationErrorV1::ZeroIdentity);
        }
        Ok(Self(bytes))
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProducerTextV1(String);

impl ProducerTextV1 {
    pub fn new(value: impl AsRef<str>) -> Result<Self, TraceValidationErrorV1> {
        let value = value.as_ref();
        let len = value.len();
        if value.is_empty() || len > MAX_PRODUCER_TEXT_BYTES_V1 || value.contains('\0') {
            return Err(TraceValidationErrorV1::InvalidProducerText { len });
        }
        let mut owned = String::new();
        owned
            .try_reserve_exact(len)
            .map_err(|_| TraceValidationErrorV1::ValidationAllocationFailure)?;
        owned.push_str(value);
        Ok(Self(owned))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    const fn capacity(&self) -> usize {
        self.0.capacity()
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProducerKindV1 {
    CpuKirSimulator,
    KfdHardwareCollector,
    RocgdbImporter,
    RocprofImporter,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProducerIdentityV1 {
    kind: ProducerKindV1,
    name: ProducerTextV1,
    version: ProducerTextV1,
    executable: Option<OpaqueIdentityV1>,
}

impl ProducerIdentityV1 {
    pub const fn new(
        kind: ProducerKindV1,
        name: ProducerTextV1,
        version: ProducerTextV1,
        executable: Option<OpaqueIdentityV1>,
    ) -> Self {
        Self {
            kind,
            name,
            version,
            executable,
        }
    }

    pub const fn kind(&self) -> ProducerKindV1 {
        self.kind
    }

    pub const fn name(&self) -> &ProducerTextV1 {
        &self.name
    }

    pub const fn version(&self) -> &ProducerTextV1 {
        &self.version
    }

    pub const fn executable(&self) -> Option<OpaqueIdentityV1> {
        self.executable
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ExecutionKindV1 {
    CpuKirSimulation,
    KfdHardware,
    RocgdbImport,
    RocprofImport,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ContentIdentitySchemeV1 {
    RawCanonicalSha256,
    DomainSeparatedSha256,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
/// Inert producer-supplied content claim for an optional related artifact.
/// Construction validates shape only and does not authenticate the digest.
pub struct ContentIdentityV1 {
    scheme: ContentIdentitySchemeV1,
    format_version: u16,
    digest: OpaqueIdentityV1,
    canonical_len: u64,
}

impl ContentIdentityV1 {
    pub fn new(
        scheme: ContentIdentitySchemeV1,
        format_version: u16,
        digest: OpaqueIdentityV1,
        canonical_len: u64,
    ) -> Result<Self, TraceValidationErrorV1> {
        if format_version == 0 {
            return Err(TraceValidationErrorV1::ZeroFormatVersion);
        }
        if canonical_len == 0 {
            return Err(TraceValidationErrorV1::ZeroCanonicalLength);
        }
        Ok(Self {
            scheme,
            format_version,
            digest,
            canonical_len,
        })
    }

    pub const fn scheme(self) -> ContentIdentitySchemeV1 {
        self.scheme
    }

    pub const fn format_version(self) -> u16 {
        self.format_version
    }

    pub const fn digest(self) -> OpaqueIdentityV1 {
        self.digest
    }

    pub const fn canonical_len(self) -> u64 {
        self.canonical_len
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
/// Untrusted content-identity claim carried by a trace producer.
///
/// This type does not authenticate the digest, establish canonical V6 bytes,
/// or grant any compiler/runtime authority. An owning adapter must compare it
/// against an independently validated canonical V6 owner before resolving any
/// site claim.
pub struct KernelIrIdentityClaimV1 {
    wire_version: u16,
    identity_policy: u16,
    digest: OpaqueIdentityV1,
    canonical_len: u64,
}

impl KernelIrIdentityClaimV1 {
    pub fn canonical_v6_claim(
        digest: OpaqueIdentityV1,
        canonical_len: u64,
    ) -> Result<Self, TraceValidationErrorV1> {
        if canonical_len == 0 {
            return Err(TraceValidationErrorV1::ZeroCanonicalLength);
        }
        Ok(Self {
            wire_version: KERNEL_IR_WIRE_VERSION_V6,
            identity_policy: KERNEL_IR_IDENTITY_POLICY_V1,
            digest,
            canonical_len,
        })
    }

    pub const fn wire_version(self) -> u16 {
        self.wire_version
    }

    pub const fn identity_policy(self) -> u16 {
        self.identity_policy
    }

    pub const fn digest(self) -> OpaqueIdentityV1 {
        self.digest
    }

    pub const fn canonical_len(self) -> u64 {
        self.canonical_len
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum WaveWidthV1 {
    Wave32,
    Wave64,
}

impl WaveWidthV1 {
    pub const fn lanes(self) -> u16 {
        match self {
            Self::Wave32 => 32,
            Self::Wave64 => 64,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LaunchGeometryV1 {
    logical_grid: [u64; 3],
    grid_workgroups: [u32; 3],
    workgroup_size: [u32; 3],
    wave_width: WaveWidthV1,
}

impl LaunchGeometryV1 {
    pub fn new(
        grid_workgroups: [u32; 3],
        workgroup_size: [u32; 3],
        wave_width: WaveWidthV1,
    ) -> Result<Self, TraceValidationErrorV1> {
        let mut logical_grid = [0_u64; 3];
        for axis in 0..3 {
            logical_grid[axis] = u64::from(grid_workgroups[axis])
                .checked_mul(u64::from(workgroup_size[axis]))
                .ok_or(TraceValidationErrorV1::LaunchGeometryOverflow)?;
        }
        Self::new_exact(logical_grid, grid_workgroups, workgroup_size, wave_width)
    }

    pub fn new_exact(
        logical_grid: [u64; 3],
        grid_workgroups: [u32; 3],
        workgroup_size: [u32; 3],
        wave_width: WaveWidthV1,
    ) -> Result<Self, TraceValidationErrorV1> {
        if logical_grid.contains(&0) {
            return Err(TraceValidationErrorV1::ZeroLaunchDimension {
                field: LaunchDimensionFieldV1::LogicalGrid,
            });
        }
        if grid_workgroups.contains(&0) {
            return Err(TraceValidationErrorV1::ZeroLaunchDimension {
                field: LaunchDimensionFieldV1::GridWorkgroups,
            });
        }
        if workgroup_size.contains(&0) {
            return Err(TraceValidationErrorV1::ZeroLaunchDimension {
                field: LaunchDimensionFieldV1::WorkgroupSize,
            });
        }
        for dimensions in [grid_workgroups, workgroup_size] {
            dimensions
                .into_iter()
                .try_fold(1_u64, |product, value| {
                    product.checked_mul(u64::from(value))
                })
                .ok_or(TraceValidationErrorV1::LaunchGeometryOverflow)?;
        }
        for axis in 0..3 {
            let workgroup = u64::from(workgroup_size[axis]);
            let padded = u64::from(grid_workgroups[axis])
                .checked_mul(workgroup)
                .ok_or(TraceValidationErrorV1::LaunchGeometryOverflow)?;
            let prior = u64::from(grid_workgroups[axis] - 1)
                .checked_mul(workgroup)
                .ok_or(TraceValidationErrorV1::LaunchGeometryOverflow)?;
            if logical_grid[axis] > padded || logical_grid[axis] <= prior {
                return Err(TraceValidationErrorV1::LogicalGridWorkgroupMismatch { axis });
            }
        }
        Ok(Self {
            logical_grid,
            grid_workgroups,
            workgroup_size,
            wave_width,
        })
    }

    pub const fn logical_grid(self) -> [u64; 3] {
        self.logical_grid
    }

    pub const fn grid_workgroups(self) -> [u32; 3] {
        self.grid_workgroups
    }

    pub const fn workgroup_size(self) -> [u32; 3] {
        self.workgroup_size
    }

    pub const fn wave_width(self) -> WaveWidthV1 {
        self.wave_width
    }

    pub fn workitems_per_workgroup(self) -> u64 {
        self.workgroup_size.into_iter().map(u64::from).product()
    }

    pub fn waves_per_workgroup(self) -> u64 {
        self.workitems_per_workgroup()
            .div_ceil(u64::from(self.wave_width.lanes()))
    }

    /// Canonical D1-D3 row-major linearization with dimension 0 varying fastest.
    pub fn linear_workgroup(self, coordinate: [u32; 3]) -> Option<u64> {
        linearize_u32x3(coordinate, self.grid_workgroups)
    }

    /// Canonical D1-D3 row-major linearization with dimension 0 varying fastest.
    pub fn linear_local_workitem(self, coordinate: [u32; 3]) -> Option<u64> {
        linearize_u32x3(coordinate, self.workgroup_size)
    }

    pub fn valid_lane_mask(self, workgroup: [u32; 3], wave: u32) -> Option<u64> {
        self.linear_workgroup(workgroup)?;
        let wave = u64::from(wave);
        if wave >= self.waves_per_workgroup() {
            return None;
        }
        let width = u64::from(self.wave_width.lanes());
        let first_lane = wave.checked_mul(width)?;
        let mut mask = 0_u64;
        for lane in 0..width {
            let linear = first_lane.checked_add(lane)?;
            if linear >= self.workitems_per_workgroup() {
                break;
            }
            let x = linear % u64::from(self.workgroup_size[0]);
            let yz = linear / u64::from(self.workgroup_size[0]);
            let y = yz % u64::from(self.workgroup_size[1]);
            let z = yz / u64::from(self.workgroup_size[1]);
            let local = [x, y, z];
            let active = (0..3).all(|axis| {
                u64::from(workgroup[axis]) * u64::from(self.workgroup_size[axis]) + local[axis]
                    < self.logical_grid[axis]
            });
            if active {
                mask |= 1_u64 << lane;
            }
        }
        Some(mask)
    }
}

fn linearize_u32x3(coordinate: [u32; 3], extent: [u32; 3]) -> Option<u64> {
    if coordinate
        .into_iter()
        .zip(extent)
        .any(|(coordinate, extent)| coordinate >= extent)
    {
        return None;
    }
    u64::from(coordinate[2])
        .checked_mul(u64::from(extent[1]))?
        .checked_add(u64::from(coordinate[1]))?
        .checked_mul(u64::from(extent[0]))?
        .checked_add(u64::from(coordinate[0]))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LaunchDimensionFieldV1 {
    LogicalGrid,
    GridWorkgroups,
    WorkgroupSize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TraceBoundsV1 {
    max_events: u64,
    max_encoded_bytes: u64,
    max_resident_bytes: u64,
    max_evidence_refs_per_event: u16,
}

impl TraceBoundsV1 {
    pub fn new(
        max_events: u64,
        max_encoded_bytes: u64,
        max_evidence_refs_per_event: u16,
    ) -> Result<Self, TraceValidationErrorV1> {
        Self::new_with_resident(
            max_events,
            max_encoded_bytes,
            MAX_TRACE_RESIDENT_BYTES_V1,
            max_evidence_refs_per_event,
        )
    }

    pub fn new_with_resident(
        max_events: u64,
        max_encoded_bytes: u64,
        max_resident_bytes: u64,
        max_evidence_refs_per_event: u16,
    ) -> Result<Self, TraceValidationErrorV1> {
        if max_events == 0 || max_events > MAX_TRACE_EVENTS_V1 {
            return Err(TraceValidationErrorV1::EventLimitOutOfRange { max_events });
        }
        if max_encoded_bytes == 0 || max_encoded_bytes > MAX_TRACE_BYTES_V1 {
            return Err(TraceValidationErrorV1::ByteLimitOutOfRange { max_encoded_bytes });
        }
        if max_resident_bytes == 0
            || max_resident_bytes > MAX_TRACE_RESIDENT_BYTES_V1
            || max_encoded_bytes > max_resident_bytes
        {
            return Err(TraceValidationErrorV1::ResidentLimitOutOfRange { max_resident_bytes });
        }
        if max_evidence_refs_per_event == 0
            || usize::from(max_evidence_refs_per_event) > MAX_EVIDENCE_REFS_PER_EVENT_V1
        {
            return Err(TraceValidationErrorV1::EvidenceLimitOutOfRange {
                max_evidence_refs_per_event,
            });
        }
        let retained_per_event = (std::mem::size_of::<TraceEventV1>() as u64)
            .checked_add(
                u64::from(max_evidence_refs_per_event)
                    .checked_mul(std::mem::size_of::<EvidenceRefV1>() as u64)
                    .ok_or(TraceValidationErrorV1::ResidentSizeOverflow)?,
            )
            .and_then(|bytes| bytes.checked_mul(max_events))
            .ok_or(TraceValidationErrorV1::ResidentSizeOverflow)?;
        let scratch_per_event = [
            std::mem::size_of::<InvocationLifecycleIndexEntryV1>(),
            std::mem::size_of::<OperationLifecycleIndexEntryV1>(),
            std::mem::size_of::<AllocationLifecycleIndexEntryV1>(),
        ]
        .into_iter()
        .max()
        .unwrap_or(0) as u64;
        let validation_scratch = scratch_per_event
            .checked_mul(max_events)
            .ok_or(TraceValidationErrorV1::ResidentSizeOverflow)?;
        let text_bytes = (2 * MAX_PRODUCER_TEXT_BYTES_V1) as u64;
        let required_resident = retained_per_event
            .checked_add(validation_scratch.max(max_encoded_bytes))
            .and_then(|bytes| bytes.checked_add(text_bytes))
            .ok_or(TraceValidationErrorV1::ResidentSizeOverflow)?;
        if required_resident > max_resident_bytes {
            return Err(TraceValidationErrorV1::ResidentLimitExceeded {
                actual: required_resident,
                max: max_resident_bytes,
            });
        }
        Ok(Self {
            max_events,
            max_encoded_bytes,
            max_resident_bytes,
            max_evidence_refs_per_event,
        })
    }

    pub const fn max_events(self) -> u64 {
        self.max_events
    }

    pub const fn max_encoded_bytes(self) -> u64 {
        self.max_encoded_bytes
    }

    pub const fn max_resident_bytes(self) -> u64 {
        self.max_resident_bytes
    }

    pub const fn max_evidence_refs_per_event(self) -> u16 {
        self.max_evidence_refs_per_event
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TruncationReasonV1 {
    EventLimit,
    ByteLimit,
    CollectorLoss,
    ProducerFailure,
    UserStopped,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DroppedEventCountV1 {
    Known(u64),
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TraceCompletenessV1 {
    Complete,
    Truncated {
        reason: TruncationReasonV1,
        emitted_events: u64,
        dropped_events: DroppedEventCountV1,
    },
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CaptureStartBoundaryV1 {
    DispatchBeginIncluded,
    DispatchAlreadyActive,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CaptureEndBoundaryV1 {
    DispatchEndIncluded,
    DispatchContinuesAfterCapture,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CaptureBoundariesV1 {
    start: CaptureStartBoundaryV1,
    end: CaptureEndBoundaryV1,
}

impl CaptureBoundariesV1 {
    pub const FULL_DISPATCH: Self = Self {
        start: CaptureStartBoundaryV1::DispatchBeginIncluded,
        end: CaptureEndBoundaryV1::DispatchEndIncluded,
    };

    pub const fn new(start: CaptureStartBoundaryV1, end: CaptureEndBoundaryV1) -> Self {
        Self { start, end }
    }

    pub const fn start(self) -> CaptureStartBoundaryV1 {
        self.start
    }

    pub const fn end(self) -> CaptureEndBoundaryV1 {
        self.end
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraceHeaderV1 {
    producer: ProducerIdentityV1,
    execution_kind: ExecutionKindV1,
    kernel_ir_claim: KernelIrIdentityClaimV1,
    semantic_mir: Option<ContentIdentityV1>,
    lineage: Option<ContentIdentityV1>,
    artifact: Option<ContentIdentityV1>,
    dispatch: DispatchIdentityV1,
    launch: LaunchGeometryV1,
    bounds: TraceBoundsV1,
    completeness: TraceCompletenessV1,
    boundaries: CaptureBoundariesV1,
}

impl TraceHeaderV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        producer: ProducerIdentityV1,
        execution_kind: ExecutionKindV1,
        kernel_ir_claim: KernelIrIdentityClaimV1,
        semantic_mir: Option<ContentIdentityV1>,
        lineage: Option<ContentIdentityV1>,
        artifact: Option<ContentIdentityV1>,
        dispatch: DispatchIdentityV1,
        launch: LaunchGeometryV1,
        bounds: TraceBoundsV1,
        completeness: TraceCompletenessV1,
        boundaries: CaptureBoundariesV1,
    ) -> Result<Self, TraceValidationErrorV1> {
        if !producer_execution_compatible(producer.kind(), execution_kind) {
            return Err(TraceValidationErrorV1::ProducerExecutionMismatch {
                producer: producer.kind(),
                execution: execution_kind,
            });
        }
        if matches!(completeness, TraceCompletenessV1::Complete)
            && boundaries != CaptureBoundariesV1::FULL_DISPATCH
        {
            return Err(TraceValidationErrorV1::CompleteTraceRequiresFullBoundaries);
        }
        if let TraceCompletenessV1::Truncated {
            dropped_events: DroppedEventCountV1::Known(0),
            ..
        } = completeness
        {
            return Err(TraceValidationErrorV1::ZeroKnownDroppedEvents);
        }
        Ok(Self {
            producer,
            execution_kind,
            kernel_ir_claim,
            semantic_mir,
            lineage,
            artifact,
            dispatch,
            launch,
            bounds,
            completeness,
            boundaries,
        })
    }

    pub const fn producer(&self) -> &ProducerIdentityV1 {
        &self.producer
    }

    pub const fn execution_kind(&self) -> ExecutionKindV1 {
        self.execution_kind
    }

    pub const fn kernel_ir_claim(&self) -> KernelIrIdentityClaimV1 {
        self.kernel_ir_claim
    }

    pub const fn semantic_mir(&self) -> Option<ContentIdentityV1> {
        self.semantic_mir
    }

    pub const fn lineage(&self) -> Option<ContentIdentityV1> {
        self.lineage
    }

    pub const fn artifact(&self) -> Option<ContentIdentityV1> {
        self.artifact
    }

    pub const fn dispatch(&self) -> DispatchIdentityV1 {
        self.dispatch
    }

    pub const fn launch(&self) -> LaunchGeometryV1 {
        self.launch
    }

    pub const fn bounds(&self) -> TraceBoundsV1 {
        self.bounds
    }

    pub const fn completeness(&self) -> TraceCompletenessV1 {
        self.completeness
    }

    pub const fn boundaries(&self) -> CaptureBoundariesV1 {
        self.boundaries
    }
}

const fn producer_execution_compatible(
    producer: ProducerKindV1,
    execution: ExecutionKindV1,
) -> bool {
    matches!(
        (producer, execution),
        (
            ProducerKindV1::CpuKirSimulator,
            ExecutionKindV1::CpuKirSimulation
        ) | (
            ProducerKindV1::KfdHardwareCollector,
            ExecutionKindV1::KfdHardware
        ) | (
            ProducerKindV1::RocgdbImporter,
            ExecutionKindV1::RocgdbImport
        ) | (
            ProducerKindV1::RocprofImporter,
            ExecutionKindV1::RocprofImport
        )
    )
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum EvidenceKindV1 {
    Declaration,
    Proof,
    InferenceRule,
    RuntimeObservation,
    Artifact,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EvidenceRefV1 {
    kind: EvidenceKindV1,
    identity: OpaqueIdentityV1,
}

impl EvidenceRefV1 {
    pub const fn new(kind: EvidenceKindV1, identity: OpaqueIdentityV1) -> Self {
        Self { kind, identity }
    }

    pub const fn kind(self) -> EvidenceKindV1 {
        self.kind
    }

    pub const fn identity(self) -> OpaqueIdentityV1 {
        self.identity
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum UnavailableReasonV1 {
    Unsupported,
    NotCaptured,
    OptimizedOut,
    OutsideCaptureScope,
    Truncated,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum FactProvenanceV1 {
    /// Exactly one declaration reference must be present in the event evidence set.
    Declared,
    /// Exactly one proof reference must be present in the event evidence set.
    Proved,
    /// The observer is exactly the producer identified by the trace header.
    Observed,
    /// Exactly one inference-rule reference must be present in the event evidence set.
    Inferred,
    Unavailable {
        reason: UnavailableReasonV1,
    },
}

impl FactProvenanceV1 {
    fn validate_evidence(
        self,
        evidence_refs: &[EvidenceRefV1],
    ) -> Result<(), TraceValidationErrorV1> {
        let required = match self {
            Self::Declared => Some(EvidenceKindV1::Declaration),
            Self::Proved => Some(EvidenceKindV1::Proof),
            Self::Inferred => Some(EvidenceKindV1::InferenceRule),
            Self::Observed | Self::Unavailable { .. } => None,
        };
        if let Some(required) = required {
            let actual = evidence_refs
                .iter()
                .filter(|evidence| evidence.kind() == required)
                .count();
            if actual != 1 {
                return Err(TraceValidationErrorV1::ProvenanceEvidenceCardinality {
                    kind: required,
                    actual,
                });
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DispatchIdentityDomainV1 {
    TraceLocal,
    RuntimeModel,
    ImportedCollector,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DispatchIdentityV1 {
    domain: DispatchIdentityDomainV1,
    identity: OpaqueIdentityV1,
}

impl DispatchIdentityV1 {
    pub const fn new(domain: DispatchIdentityDomainV1, identity: OpaqueIdentityV1) -> Self {
        Self { domain, identity }
    }

    pub const fn domain(self) -> DispatchIdentityDomainV1 {
        self.domain
    }

    pub const fn identity(self) -> OpaqueIdentityV1 {
        self.identity
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ActiveMaskV1 {
    width: WaveWidthV1,
    bits: u64,
}

impl ActiveMaskV1 {
    pub fn new(width: WaveWidthV1, bits: u64) -> Result<Self, TraceValidationErrorV1> {
        if width == WaveWidthV1::Wave32 && bits > u64::from(u32::MAX) {
            return Err(TraceValidationErrorV1::ActiveMaskExceedsWaveWidth);
        }
        Ok(Self { width, bits })
    }

    pub const fn width(self) -> WaveWidthV1 {
        self.width
    }

    pub const fn bits(self) -> u64 {
        self.bits
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ExecutionLevelV1 {
    Dispatch,
    Workgroup {
        workgroup: [u32; 3],
    },
    Wave {
        workgroup: [u32; 3],
        wave: u32,
        active_mask: ActiveMaskV1,
    },
    Lane {
        workgroup: [u32; 3],
        wave: u32,
        lane: u16,
        logical_workitem: [u64; 3],
        active_mask: ActiveMaskV1,
    },
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExecutionScopeV1 {
    dispatch: DispatchIdentityV1,
    level: ExecutionLevelV1,
}

impl ExecutionScopeV1 {
    pub const fn dispatch(dispatch: DispatchIdentityV1) -> Self {
        Self {
            dispatch,
            level: ExecutionLevelV1::Dispatch,
        }
    }

    pub const fn workgroup(dispatch: DispatchIdentityV1, workgroup: [u32; 3]) -> Self {
        Self {
            dispatch,
            level: ExecutionLevelV1::Workgroup { workgroup },
        }
    }

    pub const fn wave(
        dispatch: DispatchIdentityV1,
        workgroup: [u32; 3],
        wave: u32,
        active_mask: ActiveMaskV1,
    ) -> Self {
        Self {
            dispatch,
            level: ExecutionLevelV1::Wave {
                workgroup,
                wave,
                active_mask,
            },
        }
    }

    pub const fn lane(
        dispatch: DispatchIdentityV1,
        workgroup: [u32; 3],
        wave: u32,
        lane: u16,
        logical_workitem: [u64; 3],
        active_mask: ActiveMaskV1,
    ) -> Self {
        Self {
            dispatch,
            level: ExecutionLevelV1::Lane {
                workgroup,
                wave,
                lane,
                logical_workitem,
                active_mask,
            },
        }
    }

    pub const fn dispatch_identity(self) -> DispatchIdentityV1 {
        self.dispatch
    }

    pub const fn level(self) -> ExecutionLevelV1 {
        self.level
    }

    pub const fn workgroup_coordinate(self) -> Option<[u32; 3]> {
        match self.level {
            ExecutionLevelV1::Dispatch => None,
            ExecutionLevelV1::Workgroup { workgroup }
            | ExecutionLevelV1::Wave { workgroup, .. }
            | ExecutionLevelV1::Lane { workgroup, .. } => Some(workgroup),
        }
    }

    pub const fn wave_ordinal(self) -> Option<u32> {
        match self.level {
            ExecutionLevelV1::Wave { wave, .. } | ExecutionLevelV1::Lane { wave, .. } => Some(wave),
            ExecutionLevelV1::Dispatch | ExecutionLevelV1::Workgroup { .. } => None,
        }
    }

    pub const fn lane_ordinal(self) -> Option<u16> {
        match self.level {
            ExecutionLevelV1::Lane { lane, .. } => Some(lane),
            _ => None,
        }
    }

    pub const fn logical_workitem(self) -> Option<[u64; 3]> {
        match self.level {
            ExecutionLevelV1::Lane {
                logical_workitem, ..
            } => Some(logical_workitem),
            _ => None,
        }
    }

    pub const fn active_mask(self) -> Option<ActiveMaskV1> {
        match self.level {
            ExecutionLevelV1::Wave { active_mask, .. }
            | ExecutionLevelV1::Lane { active_mask, .. } => Some(active_mask),
            _ => None,
        }
    }

    fn validate_for_launch(
        self,
        dispatch: DispatchIdentityV1,
        launch: LaunchGeometryV1,
    ) -> Result<(), TraceValidationErrorV1> {
        if self.dispatch != dispatch {
            return Err(TraceValidationErrorV1::DispatchIdentityMismatch);
        }
        let Some(workgroup) = self.workgroup_coordinate() else {
            return Ok(());
        };
        if launch.linear_workgroup(workgroup).is_none() {
            return Err(TraceValidationErrorV1::WorkgroupOutsideLaunch);
        }
        let Some(wave) = self.wave_ordinal() else {
            return Ok(());
        };
        let valid_mask = launch
            .valid_lane_mask(workgroup, wave)
            .ok_or(TraceValidationErrorV1::WaveOutsideWorkgroup)?;
        let mask = self
            .active_mask()
            .expect("wave and lane scopes carry masks");
        if mask.width() != launch.wave_width() {
            return Err(TraceValidationErrorV1::ActiveMaskWaveWidthMismatch);
        }
        if mask.bits() != valid_mask {
            return Err(TraceValidationErrorV1::ActiveMaskMismatch {
                expected: valid_mask,
                actual: mask.bits(),
            });
        }
        let Some(lane) = self.lane_ordinal() else {
            return Ok(());
        };
        if lane >= launch.wave_width().lanes() {
            return Err(TraceValidationErrorV1::LaneOutsideWave);
        }
        if mask.bits() & (1_u64 << lane) == 0 {
            return Err(TraceValidationErrorV1::ScopedLaneIsInactive);
        }
        let logical = self
            .logical_workitem()
            .expect("lane scopes carry logical coordinates");
        let mut local = [0_u32; 3];
        let mut derived_workgroup = [0_u32; 3];
        for dimension in 0..3 {
            let workgroup_size = u64::from(launch.workgroup_size()[dimension]);
            if logical[dimension] >= launch.logical_grid()[dimension] {
                return Err(TraceValidationErrorV1::LogicalWorkitemOutsideLaunch);
            }
            local[dimension] = u32::try_from(logical[dimension] % workgroup_size)
                .map_err(|_| TraceValidationErrorV1::LaunchGeometryOverflow)?;
            derived_workgroup[dimension] = u32::try_from(logical[dimension] / workgroup_size)
                .map_err(|_| TraceValidationErrorV1::LaunchGeometryOverflow)?;
        }
        if derived_workgroup != workgroup {
            return Err(TraceValidationErrorV1::LogicalWorkitemWorkgroupMismatch);
        }
        let linear_local = launch
            .linear_local_workitem(local)
            .ok_or(TraceValidationErrorV1::LogicalWorkitemOutsideLaunch)?;
        let width = u64::from(launch.wave_width().lanes());
        if linear_local / width != u64::from(wave) || linear_local % width != u64::from(lane) {
            return Err(TraceValidationErrorV1::LogicalWorkitemWaveLaneMismatch);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum KirSitePointV1 {
    BlockEntry,
    Operation(u64),
    Terminator,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
/// Unresolved ordinal claim. Existence and CFG/catalog binding are external.
pub struct KirSiteClaimV1 {
    function_ordinal: u64,
    block_ordinal: u64,
    point: KirSitePointV1,
}

impl KirSiteClaimV1 {
    pub const fn new(function_ordinal: u64, block_ordinal: u64, point: KirSitePointV1) -> Self {
        Self {
            function_ordinal,
            block_ordinal,
            point,
        }
    }

    pub const fn function_ordinal(self) -> u64 {
        self.function_ordinal
    }

    pub const fn block_ordinal(self) -> u64 {
        self.block_ordinal
    }

    pub const fn point(self) -> KirSitePointV1 {
        self.point
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TimestampV1 {
    LogicalStep(u64),
    Clock {
        domain: OpaqueIdentityV1,
        ticks: u64,
    },
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DispatchOutcomeV1 {
    Completed,
    Failed,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DispatchEventV1 {
    Begin,
    End(DispatchOutcomeV1),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum InvocationEventV1 {
    Begin,
    End,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum OperationEventV1 {
    Begin(OperationOccurrenceIdV1),
    End(OperationOccurrenceIdV1),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct OperationOccurrenceIdV1 {
    frame: u64,
    occurrence: u64,
}

impl OperationOccurrenceIdV1 {
    pub fn new(frame: u64, occurrence: u64) -> Result<Self, TraceValidationErrorV1> {
        if frame == 0 || occurrence == 0 {
            return Err(TraceValidationErrorV1::ZeroOperationOccurrenceIdentity);
        }
        Ok(Self { frame, occurrence })
    }

    pub const fn frame(self) -> u64 {
        self.frame
    }

    pub const fn occurrence(self) -> u64 {
        self.occurrence
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MemoryAccessKindV1 {
    Read,
    Write,
    Atomic,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AddressSpaceV1 {
    Private,
    Workgroup,
    Global,
    Constant,
    Generic,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TraceAllocationIdV1 {
    ordinal: u64,
    generation: u64,
}

impl TraceAllocationIdV1 {
    pub fn new(ordinal: u64, generation: u64) -> Result<Self, TraceValidationErrorV1> {
        if ordinal == 0 {
            return Err(TraceValidationErrorV1::ZeroAllocationIdentity);
        }
        Ok(Self {
            ordinal,
            generation,
        })
    }

    pub const fn ordinal(self) -> u64 {
        self.ordinal
    }

    pub const fn generation(self) -> u64 {
        self.generation
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MemoryFaultKindV1 {
    OutOfBounds,
    Misaligned,
    InvalidAddressSpace,
    UseAfterRelease,
    Uninitialized,
    PermissionDenied,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MemoryOutcomeV1 {
    Completed,
    Fault(MemoryFaultKindV1),
    Unavailable(UnavailableReasonV1),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MemoryEventV1 {
    kind: MemoryAccessKindV1,
    allocation: TraceAllocationIdV1,
    byte_offset: u64,
    byte_len: u64,
    address_space: AddressSpaceV1,
    outcome: MemoryOutcomeV1,
}

impl MemoryEventV1 {
    pub fn new(
        kind: MemoryAccessKindV1,
        allocation: TraceAllocationIdV1,
        byte_offset: u64,
        byte_len: u64,
        address_space: AddressSpaceV1,
        outcome: MemoryOutcomeV1,
    ) -> Result<Self, TraceValidationErrorV1> {
        if byte_len == 0 {
            return Err(TraceValidationErrorV1::ZeroMemoryAccessLength);
        }
        if byte_offset.checked_add(byte_len).is_none()
            && outcome != MemoryOutcomeV1::Fault(MemoryFaultKindV1::OutOfBounds)
        {
            return Err(TraceValidationErrorV1::MemoryRangeOverflow);
        }
        Ok(Self {
            kind,
            allocation,
            byte_offset,
            byte_len,
            address_space,
            outcome,
        })
    }

    pub const fn kind(self) -> MemoryAccessKindV1 {
        self.kind
    }

    pub const fn allocation(self) -> TraceAllocationIdV1 {
        self.allocation
    }

    pub const fn byte_offset(self) -> u64 {
        self.byte_offset
    }

    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }

    pub const fn address_space(self) -> AddressSpaceV1 {
        self.address_space
    }

    pub const fn outcome(self) -> MemoryOutcomeV1 {
        self.outcome
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum BarrierScopeV1 {
    Wave,
    Workgroup,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum BarrierActionV1 {
    Arrive,
    Release,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BarrierEventV1 {
    barrier_id: u32,
    phase: u64,
    scope: BarrierScopeV1,
    action: BarrierActionV1,
}

impl BarrierEventV1 {
    pub const fn new(
        barrier_id: u32,
        phase: u64,
        scope: BarrierScopeV1,
        action: BarrierActionV1,
    ) -> Self {
        Self {
            barrier_id,
            phase,
            scope,
            action,
        }
    }

    pub const fn barrier_id(self) -> u32 {
        self.barrier_id
    }

    pub const fn phase(self) -> u64 {
        self.phase
    }

    pub const fn scope(self) -> BarrierScopeV1 {
        self.scope
    }

    pub const fn action(self) -> BarrierActionV1 {
        self.action
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AllocationEventV1 {
    Create {
        allocation: TraceAllocationIdV1,
        byte_len: u64,
        address_space: AddressSpaceV1,
    },
    /// The allocation predates the capture, but its layout is known.
    Preexisting {
        allocation: TraceAllocationIdV1,
        byte_len: u64,
        address_space: AddressSpaceV1,
    },
    /// The allocation predates the capture and its layout is unavailable.
    UnknownLifecycle {
        allocation: TraceAllocationIdV1,
    },
    Release {
        allocation: TraceAllocationIdV1,
    },
}

impl AllocationEventV1 {
    pub const fn allocation(self) -> TraceAllocationIdV1 {
        match self {
            Self::Create { allocation, .. }
            | Self::Preexisting { allocation, .. }
            | Self::UnknownLifecycle { allocation }
            | Self::Release { allocation } => allocation,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DiagnosticKindV1 {
    Trap,
    Assert,
    Fault,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DiagnosticEventV1 {
    kind: DiagnosticKindV1,
    code: u32,
}

impl DiagnosticEventV1 {
    pub const fn new(kind: DiagnosticKindV1, code: u32) -> Self {
        Self { kind, code }
    }

    pub const fn kind(self) -> DiagnosticKindV1 {
        self.kind
    }

    pub const fn code(self) -> u32 {
        self.code
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TraceEventKindV1 {
    Dispatch(DispatchEventV1),
    Invocation(InvocationEventV1),
    BlockEnter,
    Operation(OperationEventV1),
    Branch { target_block_ordinal: u64 },
    Memory(MemoryEventV1),
    Barrier(BarrierEventV1),
    Allocation(AllocationEventV1),
    Diagnostic(DiagnosticEventV1),
}

impl TraceEventKindV1 {
    fn validate_site(self, site: Option<KirSiteClaimV1>) -> Result<(), TraceValidationErrorV1> {
        let valid = match self {
            Self::Dispatch(_) | Self::Invocation(_) => site.is_none(),
            Self::BlockEnter => {
                matches!(
                    site.map(KirSiteClaimV1::point),
                    Some(KirSitePointV1::BlockEntry)
                )
            }
            Self::Operation(_) | Self::Memory(_) | Self::Barrier(_) => {
                matches!(
                    site.map(KirSiteClaimV1::point),
                    Some(KirSitePointV1::Operation(_))
                )
            }
            Self::Branch { .. } => {
                matches!(
                    site.map(KirSiteClaimV1::point),
                    Some(KirSitePointV1::Terminator)
                )
            }
            Self::Allocation(_) | Self::Diagnostic(_) => {
                site.is_none()
                    || !matches!(
                        site.map(KirSiteClaimV1::point),
                        Some(KirSitePointV1::BlockEntry)
                    )
            }
        };
        if !valid {
            return Err(TraceValidationErrorV1::EventSiteMismatch);
        }
        Ok(())
    }

    fn validate_scope(self, scope: ExecutionScopeV1) -> Result<(), TraceValidationErrorV1> {
        let level = scope.level();
        let valid = match self {
            Self::Dispatch(_) => matches!(level, ExecutionLevelV1::Dispatch),
            Self::Invocation(_) => matches!(level, ExecutionLevelV1::Lane { .. }),
            Self::BlockEnter | Self::Operation(_) | Self::Branch { .. } | Self::Memory(_) => {
                matches!(
                    level,
                    ExecutionLevelV1::Wave { .. } | ExecutionLevelV1::Lane { .. }
                )
            }
            Self::Barrier(barrier) => match barrier.scope() {
                BarrierScopeV1::Wave => {
                    matches!(
                        level,
                        ExecutionLevelV1::Wave { .. } | ExecutionLevelV1::Lane { .. }
                    )
                }
                BarrierScopeV1::Workgroup => !matches!(level, ExecutionLevelV1::Dispatch),
            },
            Self::Allocation(_) | Self::Diagnostic(_) => true,
        };
        if !valid {
            return Err(TraceValidationErrorV1::EventScopeMismatch);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraceEventV1 {
    sequence: u64,
    timestamp: TimestampV1,
    provenance: FactProvenanceV1,
    scope: ExecutionScopeV1,
    site: Option<KirSiteClaimV1>,
    kind: TraceEventKindV1,
    evidence_refs: Vec<EvidenceRefV1>,
}

impl TraceEventV1 {
    pub fn new(
        sequence: u64,
        timestamp: TimestampV1,
        provenance: FactProvenanceV1,
        scope: ExecutionScopeV1,
        site: Option<KirSiteClaimV1>,
        kind: TraceEventKindV1,
        evidence_refs: Vec<EvidenceRefV1>,
    ) -> Result<Self, TraceValidationErrorV1> {
        if evidence_refs.len() > MAX_EVIDENCE_REFS_PER_EVENT_V1 {
            return Err(TraceValidationErrorV1::TooManyEvidenceReferences {
                actual: evidence_refs.len(),
                max: MAX_EVIDENCE_REFS_PER_EVENT_V1,
            });
        }
        let mut normalized = Vec::new();
        normalized
            .try_reserve_exact(evidence_refs.len())
            .map_err(|_| TraceValidationErrorV1::ValidationAllocationFailure)?;
        normalized.extend(evidence_refs);
        normalized.sort_unstable();
        if normalized.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(TraceValidationErrorV1::DuplicateEvidenceReference);
        }
        provenance.validate_evidence(&normalized)?;
        kind.validate_site(site)?;
        kind.validate_scope(scope)?;
        Ok(Self {
            sequence,
            timestamp,
            provenance,
            scope,
            site,
            kind,
            evidence_refs: normalized,
        })
    }

    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    pub const fn timestamp(&self) -> TimestampV1 {
        self.timestamp
    }

    pub const fn provenance(&self) -> FactProvenanceV1 {
        self.provenance
    }

    pub const fn scope(&self) -> ExecutionScopeV1 {
        self.scope
    }

    pub const fn site(&self) -> Option<KirSiteClaimV1> {
        self.site
    }

    pub const fn kind(&self) -> TraceEventKindV1 {
        self.kind
    }

    pub fn evidence_refs(&self) -> &[EvidenceRefV1] {
        &self.evidence_refs
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraceV1 {
    header: TraceHeaderV1,
    events: Vec<TraceEventV1>,
}

impl TraceV1 {
    pub fn new(
        header: TraceHeaderV1,
        events: Vec<TraceEventV1>,
    ) -> Result<Self, TraceValidationErrorV1> {
        Self::new_with_resident_reservation(header, events, 0)
    }

    /// Constructs a trace while accounting bytes retained by an adapter during validation.
    pub fn new_with_resident_reservation(
        header: TraceHeaderV1,
        events: Vec<TraceEventV1>,
        reserved_resident_bytes: u64,
    ) -> Result<Self, TraceValidationErrorV1> {
        let trace = Self { header, events };
        trace.validate_with_resident_reservation(reserved_resident_bytes)?;
        Ok(trace)
    }

    pub const fn header(&self) -> &TraceHeaderV1 {
        &self.header
    }

    pub fn events(&self) -> &[TraceEventV1] {
        &self.events
    }

    pub(crate) fn validate(&self) -> Result<(), TraceValidationErrorV1> {
        self.validate_with_resident_reservation(0)
    }

    fn validate_with_resident_reservation(
        &self,
        reserved_resident_bytes: u64,
    ) -> Result<(), TraceValidationErrorV1> {
        let event_count = u64::try_from(self.events.len())
            .map_err(|_| TraceValidationErrorV1::EventCountOverflow)?;
        if event_count > self.header.bounds.max_events {
            return Err(TraceValidationErrorV1::TooManyEvents {
                actual: event_count,
                max: self.header.bounds.max_events,
            });
        }
        let resident = ValidationResidentLedgerV1::new(self, reserved_resident_bytes)?;
        resident.ensure_temporary(self.header.bounds.max_encoded_bytes)?;
        if let TraceCompletenessV1::Truncated { emitted_events, .. } = self.header.completeness
            && emitted_events != event_count
        {
            return Err(TraceValidationErrorV1::TruncatedEventCountMismatch {
                declared: emitted_events,
                actual: event_count,
            });
        }
        self.validate_sequences()?;
        for event in &self.events {
            if event.evidence_refs.len()
                > usize::from(self.header.bounds.max_evidence_refs_per_event)
            {
                return Err(TraceValidationErrorV1::TooManyEvidenceReferences {
                    actual: event.evidence_refs.len(),
                    max: usize::from(self.header.bounds.max_evidence_refs_per_event),
                });
            }
            event
                .scope
                .validate_for_launch(self.header.dispatch, self.header.launch)?;
            event.provenance.validate_evidence(&event.evidence_refs)?;
            event.kind.validate_site(event.site)?;
            event.kind.validate_scope(event.scope)?;
        }
        self.validate_execution_lifecycle(resident)?;
        self.validate_allocation_lifecycle(resident)?;
        Ok(())
    }

    fn validate_sequences(&self) -> Result<(), TraceValidationErrorV1> {
        match self.header.completeness {
            TraceCompletenessV1::Complete => {
                for (expected, event) in (0_u64..).zip(&self.events) {
                    if event.sequence != expected {
                        return Err(TraceValidationErrorV1::CompleteSequenceMismatch {
                            expected,
                            actual: event.sequence,
                        });
                    }
                }
            }
            TraceCompletenessV1::Truncated { dropped_events, .. } => {
                let mut observed_gaps = self.events.first().map_or(0, |event| event.sequence);
                for pair in self.events.windows(2) {
                    if pair[0].sequence >= pair[1].sequence {
                        return Err(TraceValidationErrorV1::NonIncreasingSequence {
                            previous: pair[0].sequence,
                            current: pair[1].sequence,
                        });
                    }
                    observed_gaps = observed_gaps
                        .checked_add(pair[1].sequence - pair[0].sequence - 1)
                        .ok_or(TraceValidationErrorV1::SequenceGapOverflow)?;
                }
                if let DroppedEventCountV1::Known(dropped) = dropped_events
                    && dropped < observed_gaps
                {
                    return Err(TraceValidationErrorV1::DroppedEventCountTooSmall {
                        declared: dropped,
                        observed_gaps,
                    });
                }
            }
        }
        Ok(())
    }

    fn validate_execution_lifecycle(
        &self,
        resident: ValidationResidentLedgerV1,
    ) -> Result<(), TraceValidationErrorV1> {
        self.validate_dispatch_and_invocations(resident)?;
        self.validate_operation_lifecycle(resident)
    }

    fn validate_dispatch_and_invocations(
        &self,
        resident: ValidationResidentLedgerV1,
    ) -> Result<(), TraceValidationErrorV1> {
        let start = self.header.boundaries.start();
        let end = self.header.boundaries.end();
        let mut dispatch_state = match start {
            CaptureStartBoundaryV1::DispatchBeginIncluded => DispatchLifecycleStateV1::NotBegun,
            CaptureStartBoundaryV1::DispatchAlreadyActive => DispatchLifecycleStateV1::Active,
        };
        let mut invocations =
            resident.reserved_vec::<InvocationLifecycleIndexEntryV1>(self.events.len())?;
        for event in &self.events {
            if matches!(event.scope.level(), ExecutionLevelV1::Lane { .. }) {
                let key = ExecutionCoordinateKeyV1::from_scope(event.scope)
                    .expect("lane scope has a coordinate key");
                invocations.push(InvocationLifecycleIndexEntryV1 {
                    key,
                    state: InvocationLifecycleStateV1::NotSeen,
                });
            }
        }
        invocations.sort_unstable_by_key(|entry| entry.key);
        invocations.dedup_by_key(|entry| entry.key);

        for event in &self.events {
            if dispatch_state == DispatchLifecycleStateV1::Ended {
                return Err(TraceValidationErrorV1::EventAfterDispatchEnd);
            }
            match event.kind {
                TraceEventKindV1::Dispatch(DispatchEventV1::Begin) => {
                    if dispatch_state != DispatchLifecycleStateV1::NotBegun {
                        return Err(TraceValidationErrorV1::DuplicateOrUnexpectedDispatchBegin);
                    }
                    dispatch_state = DispatchLifecycleStateV1::Active;
                }
                TraceEventKindV1::Dispatch(DispatchEventV1::End(_)) => {
                    if dispatch_state != DispatchLifecycleStateV1::Active {
                        return Err(TraceValidationErrorV1::DispatchEndWithoutBegin);
                    }
                    dispatch_state = DispatchLifecycleStateV1::Ended;
                }
                _ if dispatch_state == DispatchLifecycleStateV1::NotBegun => {
                    return Err(TraceValidationErrorV1::EventBeforeDispatchBegin);
                }
                TraceEventKindV1::Invocation(invocation) => {
                    let key = ExecutionCoordinateKeyV1::from_scope(event.scope)
                        .expect("invocation scope validation requires a lane");
                    let state = invocation_state_mut(&mut invocations, key)
                        .ok_or(TraceValidationErrorV1::ValidationIndexInvariant)?;
                    match invocation {
                        InvocationEventV1::Begin => {
                            if *state != InvocationLifecycleStateV1::NotSeen {
                                return Err(TraceValidationErrorV1::DuplicateInvocationBegin);
                            }
                            *state = InvocationLifecycleStateV1::ActiveExplicit;
                        }
                        InvocationEventV1::End => match *state {
                            InvocationLifecycleStateV1::ActiveExplicit
                            | InvocationLifecycleStateV1::ActiveImplicit => {
                                *state = InvocationLifecycleStateV1::Ended;
                            }
                            InvocationLifecycleStateV1::NotSeen
                                if start == CaptureStartBoundaryV1::DispatchAlreadyActive =>
                            {
                                *state = InvocationLifecycleStateV1::Ended;
                            }
                            InvocationLifecycleStateV1::NotSeen
                            | InvocationLifecycleStateV1::Ended => {
                                return Err(TraceValidationErrorV1::InvocationEndWithoutBegin);
                            }
                        },
                    }
                }
                _ => validate_lane_is_in_invocation(event.scope, start, &mut invocations)?,
            }
        }

        match end {
            CaptureEndBoundaryV1::DispatchEndIncluded => {
                if dispatch_state != DispatchLifecycleStateV1::Ended {
                    return Err(TraceValidationErrorV1::MissingDispatchEnd);
                }
                if invocations.iter().any(|entry| {
                    matches!(
                        entry.state,
                        InvocationLifecycleStateV1::ActiveExplicit
                            | InvocationLifecycleStateV1::ActiveImplicit
                    )
                }) {
                    return Err(TraceValidationErrorV1::MissingInvocationEnd);
                }
            }
            CaptureEndBoundaryV1::DispatchContinuesAfterCapture => {
                if dispatch_state == DispatchLifecycleStateV1::Ended {
                    return Err(TraceValidationErrorV1::UnexpectedCapturedDispatchEnd);
                }
            }
        }
        if start == CaptureStartBoundaryV1::DispatchBeginIncluded
            && dispatch_state == DispatchLifecycleStateV1::NotBegun
        {
            return Err(TraceValidationErrorV1::MissingDispatchBegin);
        }
        Ok(())
    }

    fn validate_operation_lifecycle(
        &self,
        resident: ValidationResidentLedgerV1,
    ) -> Result<(), TraceValidationErrorV1> {
        let start = self.header.boundaries.start();
        let mut operations =
            resident.reserved_vec::<OperationLifecycleIndexEntryV1>(self.events.len())?;
        for event in &self.events {
            if let TraceEventKindV1::Operation(operation) = event.kind {
                let occurrence = match operation {
                    OperationEventV1::Begin(occurrence) | OperationEventV1::End(occurrence) => {
                        occurrence
                    }
                };
                operations.push(OperationLifecycleIndexEntryV1 {
                    occurrence,
                    state: OperationLifecycleStateV1::NotSeen,
                });
            }
        }
        operations.sort_unstable_by_key(|entry| entry.occurrence);
        operations.dedup_by_key(|entry| entry.occurrence);

        for event in &self.events {
            let TraceEventKindV1::Operation(operation) = event.kind else {
                continue;
            };
            let coordinate = ExecutionCoordinateKeyV1::from_scope(event.scope)
                .expect("operation scope validation requires wave or lane");
            let key = OperationLifecycleKeyV1 {
                coordinate,
                site: event.site.expect("operation events require a site claim"),
            };
            let occurrence = match operation {
                OperationEventV1::Begin(occurrence) | OperationEventV1::End(occurrence) => {
                    occurrence
                }
            };
            let state = operation_state_mut(&mut operations, occurrence)
                .ok_or(TraceValidationErrorV1::ValidationIndexInvariant)?;
            match operation {
                OperationEventV1::Begin(_) => {
                    if *state != OperationLifecycleStateV1::NotSeen {
                        return Err(TraceValidationErrorV1::DuplicateOperationOccurrence);
                    }
                    *state = OperationLifecycleStateV1::Active(key);
                }
                OperationEventV1::End(_) => match *state {
                    OperationLifecycleStateV1::Active(begin_key) => {
                        if begin_key != key {
                            return Err(TraceValidationErrorV1::OperationOccurrenceMismatch);
                        }
                        *state = OperationLifecycleStateV1::Ended;
                    }
                    OperationLifecycleStateV1::Ended => {
                        return Err(TraceValidationErrorV1::DuplicateOperationEnd);
                    }
                    OperationLifecycleStateV1::NotSeen
                        if start == CaptureStartBoundaryV1::DispatchAlreadyActive =>
                    {
                        *state = OperationLifecycleStateV1::Ended;
                    }
                    OperationLifecycleStateV1::NotSeen => {
                        return Err(TraceValidationErrorV1::OperationEndWithoutBegin);
                    }
                },
            }
        }
        if self.header.boundaries.end() == CaptureEndBoundaryV1::DispatchEndIncluded
            && operations
                .iter()
                .any(|entry| matches!(entry.state, OperationLifecycleStateV1::Active(_)))
        {
            return Err(TraceValidationErrorV1::MissingOperationEnd);
        }
        Ok(())
    }

    fn validate_allocation_lifecycle(
        &self,
        resident: ValidationResidentLedgerV1,
    ) -> Result<(), TraceValidationErrorV1> {
        let mut allocations =
            resident.reserved_vec::<AllocationLifecycleIndexEntryV1>(self.events.len())?;
        for event in &self.events {
            let allocation = match event.kind {
                TraceEventKindV1::Allocation(allocation) => allocation.allocation(),
                TraceEventKindV1::Memory(memory) => memory.allocation(),
                _ => continue,
            };
            allocations.push(AllocationLifecycleIndexEntryV1 {
                ordinal: allocation.ordinal(),
                generation: None,
                state: None,
            });
        }
        allocations.sort_unstable_by_key(|entry| entry.ordinal);
        allocations.dedup_by_key(|entry| entry.ordinal);

        for event in &self.events {
            match event.kind {
                TraceEventKindV1::Allocation(allocation) => match allocation {
                    AllocationEventV1::Create {
                        allocation,
                        byte_len,
                        address_space,
                    }
                    | AllocationEventV1::Preexisting {
                        allocation,
                        byte_len,
                        address_space,
                    } => {
                        let entry = allocation_state_mut(&mut allocations, allocation.ordinal())
                            .ok_or(TraceValidationErrorV1::ValidationIndexInvariant)?;
                        introduce_allocation(
                            entry,
                            allocation,
                            AllocationStateV1::Known {
                                byte_len,
                                address_space,
                                released: false,
                            },
                        )?;
                    }
                    AllocationEventV1::UnknownLifecycle { allocation } => {
                        let entry = allocation_state_mut(&mut allocations, allocation.ordinal())
                            .ok_or(TraceValidationErrorV1::ValidationIndexInvariant)?;
                        introduce_allocation(
                            entry,
                            allocation,
                            AllocationStateV1::Unknown { released: false },
                        )?;
                    }
                    AllocationEventV1::Release { allocation } => {
                        let entry = allocation_state_mut(&mut allocations, allocation.ordinal())
                            .ok_or(TraceValidationErrorV1::ValidationIndexInvariant)?;
                        let Some(current_generation) = entry.generation else {
                            return Err(TraceValidationErrorV1::ReleaseOfUnknownAllocation {
                                allocation,
                            });
                        };
                        if current_generation != allocation.generation() {
                            return Err(TraceValidationErrorV1::AllocationGenerationNotCurrent {
                                allocation,
                                current_generation,
                            });
                        }
                        let state = entry.state.as_mut().ok_or(
                            TraceValidationErrorV1::ReleaseOfUnknownAllocation { allocation },
                        )?;
                        if state.released() {
                            return Err(TraceValidationErrorV1::DuplicateAllocationRelease {
                                allocation,
                            });
                        }
                        state.release();
                    }
                },
                TraceEventKindV1::Memory(memory) => {
                    let entry =
                        allocation_state_mut(&mut allocations, memory.allocation().ordinal())
                            .ok_or(TraceValidationErrorV1::ValidationIndexInvariant)?;
                    validate_memory_against_allocation(entry, memory)?;
                }
                _ => {}
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
pub(crate) struct ValidationResidentLedgerV1 {
    retained: u64,
    limit: u64,
}

impl ValidationResidentLedgerV1 {
    pub(crate) fn new(
        trace: &TraceV1,
        reserved_resident_bytes: u64,
    ) -> Result<Self, TraceValidationErrorV1> {
        let mut retained = reserved_resident_bytes;
        retained = checked_add_capacity::<TraceEventV1>(retained, trace.events.capacity())?;
        retained = checked_add_capacity::<u8>(retained, trace.header.producer.name.capacity())?;
        retained = checked_add_capacity::<u8>(retained, trace.header.producer.version.capacity())?;
        for event in &trace.events {
            retained =
                checked_add_capacity::<EvidenceRefV1>(retained, event.evidence_refs.capacity())?;
        }
        let ledger = Self {
            retained,
            limit: trace.header.bounds.max_resident_bytes,
        };
        ledger.ensure_temporary(0)?;
        Ok(ledger)
    }

    pub(crate) fn ensure_temporary(self, temporary: u64) -> Result<(), TraceValidationErrorV1> {
        let actual = self
            .retained
            .checked_add(temporary)
            .ok_or(TraceValidationErrorV1::ResidentSizeOverflow)?;
        if actual > self.limit {
            return Err(TraceValidationErrorV1::ResidentLimitExceeded {
                actual,
                max: self.limit,
            });
        }
        Ok(())
    }

    fn reserved_vec<T>(self, capacity: usize) -> Result<Vec<T>, TraceValidationErrorV1> {
        let mut values = Vec::new();
        values
            .try_reserve_exact(capacity)
            .map_err(|_| TraceValidationErrorV1::ValidationAllocationFailure)?;
        let bytes = capacity_bytes::<T>(values.capacity())?;
        self.ensure_temporary(bytes)?;
        Ok(values)
    }
}

pub(crate) fn capacity_bytes<T>(capacity: usize) -> Result<u64, TraceValidationErrorV1> {
    u64::try_from(capacity)
        .ok()
        .and_then(|count| count.checked_mul(std::mem::size_of::<T>() as u64))
        .ok_or(TraceValidationErrorV1::ResidentSizeOverflow)
}

fn checked_add_capacity<T>(resident: u64, capacity: usize) -> Result<u64, TraceValidationErrorV1> {
    resident
        .checked_add(capacity_bytes::<T>(capacity)?)
        .ok_or(TraceValidationErrorV1::ResidentSizeOverflow)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DispatchLifecycleStateV1 {
    NotBegun,
    Active,
    Ended,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct ExecutionCoordinateKeyV1 {
    dispatch: DispatchIdentityV1,
    workgroup: [u32; 3],
    wave: u32,
    lane: Option<u16>,
    logical_workitem: Option<[u64; 3]>,
}

impl ExecutionCoordinateKeyV1 {
    fn from_scope(scope: ExecutionScopeV1) -> Option<Self> {
        match scope.level() {
            ExecutionLevelV1::Wave {
                workgroup, wave, ..
            } => Some(Self {
                dispatch: scope.dispatch_identity(),
                workgroup,
                wave,
                lane: None,
                logical_workitem: None,
            }),
            ExecutionLevelV1::Lane {
                workgroup,
                wave,
                lane,
                logical_workitem,
                ..
            } => Some(Self {
                dispatch: scope.dispatch_identity(),
                workgroup,
                wave,
                lane: Some(lane),
                logical_workitem: Some(logical_workitem),
            }),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct OperationLifecycleKeyV1 {
    coordinate: ExecutionCoordinateKeyV1,
    site: KirSiteClaimV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InvocationLifecycleStateV1 {
    NotSeen,
    ActiveExplicit,
    ActiveImplicit,
    Ended,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct InvocationLifecycleIndexEntryV1 {
    key: ExecutionCoordinateKeyV1,
    state: InvocationLifecycleStateV1,
}

fn invocation_state_mut(
    invocations: &mut [InvocationLifecycleIndexEntryV1],
    key: ExecutionCoordinateKeyV1,
) -> Option<&mut InvocationLifecycleStateV1> {
    let position = invocations
        .binary_search_by_key(&key, |entry| entry.key)
        .ok()?;
    Some(&mut invocations[position].state)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OperationLifecycleStateV1 {
    NotSeen,
    Active(OperationLifecycleKeyV1),
    Ended,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct OperationLifecycleIndexEntryV1 {
    occurrence: OperationOccurrenceIdV1,
    state: OperationLifecycleStateV1,
}

fn operation_state_mut(
    operations: &mut [OperationLifecycleIndexEntryV1],
    occurrence: OperationOccurrenceIdV1,
) -> Option<&mut OperationLifecycleStateV1> {
    let position = operations
        .binary_search_by_key(&occurrence, |entry| entry.occurrence)
        .ok()?;
    Some(&mut operations[position].state)
}

fn validate_lane_is_in_invocation(
    scope: ExecutionScopeV1,
    start: CaptureStartBoundaryV1,
    invocations: &mut [InvocationLifecycleIndexEntryV1],
) -> Result<(), TraceValidationErrorV1> {
    if !matches!(scope.level(), ExecutionLevelV1::Lane { .. }) {
        return Ok(());
    }
    let key = ExecutionCoordinateKeyV1::from_scope(scope).expect("lane scope has coordinate key");
    let state = invocation_state_mut(invocations, key)
        .ok_or(TraceValidationErrorV1::ValidationIndexInvariant)?;
    match *state {
        InvocationLifecycleStateV1::Ended => Err(TraceValidationErrorV1::EventAfterInvocationEnd),
        InvocationLifecycleStateV1::ActiveExplicit | InvocationLifecycleStateV1::ActiveImplicit => {
            Ok(())
        }
        InvocationLifecycleStateV1::NotSeen
            if start == CaptureStartBoundaryV1::DispatchAlreadyActive =>
        {
            *state = InvocationLifecycleStateV1::ActiveImplicit;
            Ok(())
        }
        InvocationLifecycleStateV1::NotSeen => Err(TraceValidationErrorV1::EventOutsideInvocation),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AllocationStateV1 {
    Known {
        byte_len: u64,
        address_space: AddressSpaceV1,
        released: bool,
    },
    Unknown {
        released: bool,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AllocationLifecycleIndexEntryV1 {
    ordinal: u64,
    generation: Option<u64>,
    state: Option<AllocationStateV1>,
}

fn allocation_state_mut(
    allocations: &mut [AllocationLifecycleIndexEntryV1],
    ordinal: u64,
) -> Option<&mut AllocationLifecycleIndexEntryV1> {
    let position = allocations
        .binary_search_by_key(&ordinal, |entry| entry.ordinal)
        .ok()?;
    Some(&mut allocations[position])
}

fn introduce_allocation(
    entry: &mut AllocationLifecycleIndexEntryV1,
    allocation: TraceAllocationIdV1,
    state: AllocationStateV1,
) -> Result<(), TraceValidationErrorV1> {
    let actual = allocation.generation();
    match (entry.generation, entry.state) {
        (None, None) => {
            if actual != 0 {
                return Err(
                    TraceValidationErrorV1::AllocationGenerationMustStartAtZero { allocation },
                );
            }
        }
        (Some(current), Some(current_state)) if !current_state.released() => {
            if current == actual {
                return Err(TraceValidationErrorV1::DuplicateAllocationIntroduction { allocation });
            }
            return Err(TraceValidationErrorV1::AllocationOrdinalAlreadyLive {
                ordinal: entry.ordinal,
                current_generation: current,
                attempted_generation: actual,
            });
        }
        (Some(current), Some(_)) => {
            let expected = current.checked_add(1).ok_or(
                TraceValidationErrorV1::AllocationGenerationOverflow {
                    ordinal: entry.ordinal,
                },
            )?;
            if actual != expected {
                return Err(TraceValidationErrorV1::AllocationGenerationOutOfSequence {
                    ordinal: entry.ordinal,
                    expected,
                    actual,
                });
            }
        }
        _ => return Err(TraceValidationErrorV1::ValidationIndexInvariant),
    }
    entry.generation = Some(actual);
    entry.state = Some(state);
    Ok(())
}

impl AllocationStateV1 {
    const fn released(self) -> bool {
        match self {
            Self::Known { released, .. } | Self::Unknown { released } => released,
        }
    }

    fn release(&mut self) {
        match self {
            Self::Known { released, .. } | Self::Unknown { released } => *released = true,
        }
    }
}

fn validate_memory_against_allocation(
    entry: &AllocationLifecycleIndexEntryV1,
    memory: MemoryEventV1,
) -> Result<(), TraceValidationErrorV1> {
    let current_generation =
        entry
            .generation
            .ok_or(TraceValidationErrorV1::UseOfUnknownAllocation {
                allocation: memory.allocation(),
            })?;
    if memory.allocation().generation() < current_generation {
        return if memory.outcome() == MemoryOutcomeV1::Fault(MemoryFaultKindV1::UseAfterRelease) {
            Ok(())
        } else {
            Err(TraceValidationErrorV1::MemoryOutcomeInconsistent)
        };
    }
    if memory.allocation().generation() > current_generation {
        return Err(TraceValidationErrorV1::UseOfUnknownAllocation {
            allocation: memory.allocation(),
        });
    }
    let state = entry
        .state
        .as_ref()
        .ok_or(TraceValidationErrorV1::ValidationIndexInvariant)?;
    if state.released() {
        if memory.outcome() != MemoryOutcomeV1::Fault(MemoryFaultKindV1::UseAfterRelease) {
            return Err(TraceValidationErrorV1::MemoryOutcomeInconsistent);
        }
        return Ok(());
    }
    let AllocationStateV1::Known {
        byte_len,
        address_space,
        ..
    } = *state
    else {
        if memory.outcome() == MemoryOutcomeV1::Fault(MemoryFaultKindV1::UseAfterRelease) {
            return Err(TraceValidationErrorV1::MemoryOutcomeInconsistent);
        }
        return Ok(());
    };
    let wrong_space = memory.address_space() != address_space;
    let outside = memory
        .byte_offset()
        .checked_add(memory.byte_len())
        .is_none_or(|end| end > byte_len);
    let expected_fault = if wrong_space {
        Some(MemoryFaultKindV1::InvalidAddressSpace)
    } else if outside {
        Some(MemoryFaultKindV1::OutOfBounds)
    } else {
        None
    };
    match (expected_fault, memory.outcome()) {
        (Some(expected), MemoryOutcomeV1::Fault(actual)) if expected == actual => Ok(()),
        (Some(_), _) => Err(TraceValidationErrorV1::MemoryOutcomeInconsistent),
        (None, MemoryOutcomeV1::Fault(MemoryFaultKindV1::OutOfBounds))
        | (None, MemoryOutcomeV1::Fault(MemoryFaultKindV1::InvalidAddressSpace))
        | (None, MemoryOutcomeV1::Fault(MemoryFaultKindV1::UseAfterRelease)) => {
            Err(TraceValidationErrorV1::MemoryOutcomeInconsistent)
        }
        (None, _) => Ok(()),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TraceValidationErrorV1 {
    ZeroIdentity,
    InvalidProducerText {
        len: usize,
    },
    ZeroFormatVersion,
    ZeroCanonicalLength,
    ZeroLaunchDimension {
        field: LaunchDimensionFieldV1,
    },
    LaunchGeometryOverflow,
    LogicalGridWorkgroupMismatch {
        axis: usize,
    },
    EventLimitOutOfRange {
        max_events: u64,
    },
    ByteLimitOutOfRange {
        max_encoded_bytes: u64,
    },
    ResidentLimitOutOfRange {
        max_resident_bytes: u64,
    },
    ResidentSizeOverflow,
    ResidentLimitExceeded {
        actual: u64,
        max: u64,
    },
    EvidenceLimitOutOfRange {
        max_evidence_refs_per_event: u16,
    },
    ProducerExecutionMismatch {
        producer: ProducerKindV1,
        execution: ExecutionKindV1,
    },
    CompleteTraceRequiresFullBoundaries,
    ZeroKnownDroppedEvents,
    ProvenanceEvidenceCardinality {
        kind: EvidenceKindV1,
        actual: usize,
    },
    ActiveMaskExceedsWaveWidth,
    DispatchIdentityMismatch,
    WorkgroupOutsideLaunch,
    WaveOutsideWorkgroup,
    LaneOutsideWave,
    ActiveMaskWaveWidthMismatch,
    ActiveMaskIncludesTailLane,
    ActiveMaskMismatch {
        expected: u64,
        actual: u64,
    },
    ScopedLaneIsInactive,
    LogicalWorkitemOutsideLaunch,
    LogicalWorkitemWorkgroupMismatch,
    LogicalWorkitemWaveLaneMismatch,
    ZeroOperationOccurrenceIdentity,
    ZeroAllocationIdentity,
    ZeroMemoryAccessLength,
    MemoryRangeOverflow,
    EventSiteMismatch,
    EventScopeMismatch,
    TooManyEvidenceReferences {
        actual: usize,
        max: usize,
    },
    DuplicateEvidenceReference,
    EventCountOverflow,
    TooManyEvents {
        actual: u64,
        max: u64,
    },
    TruncatedEventCountMismatch {
        declared: u64,
        actual: u64,
    },
    NonIncreasingSequence {
        previous: u64,
        current: u64,
    },
    CompleteSequenceMismatch {
        expected: u64,
        actual: u64,
    },
    SequenceGapOverflow,
    DroppedEventCountTooSmall {
        declared: u64,
        observed_gaps: u64,
    },
    DuplicateOrUnexpectedDispatchBegin,
    DispatchEndWithoutBegin,
    EventBeforeDispatchBegin,
    EventAfterDispatchEnd,
    MissingDispatchBegin,
    MissingDispatchEnd,
    UnexpectedCapturedDispatchEnd,
    DuplicateInvocationBegin,
    InvocationEndWithoutBegin,
    EventOutsideInvocation,
    EventAfterInvocationEnd,
    MissingInvocationEnd,
    DuplicateOperationBegin,
    DuplicateOperationEnd,
    OperationEndWithoutBegin,
    MissingOperationEnd,
    DuplicateOperationOccurrence,
    OperationOccurrenceMismatch,
    ValidationAllocationFailure,
    ValidationIndexInvariant,
    DuplicateAllocationIntroduction {
        allocation: TraceAllocationIdV1,
    },
    AllocationGenerationMustStartAtZero {
        allocation: TraceAllocationIdV1,
    },
    AllocationOrdinalAlreadyLive {
        ordinal: u64,
        current_generation: u64,
        attempted_generation: u64,
    },
    AllocationGenerationOutOfSequence {
        ordinal: u64,
        expected: u64,
        actual: u64,
    },
    AllocationGenerationOverflow {
        ordinal: u64,
    },
    AllocationGenerationNotCurrent {
        allocation: TraceAllocationIdV1,
        current_generation: u64,
    },
    ReleaseOfUnknownAllocation {
        allocation: TraceAllocationIdV1,
    },
    DuplicateAllocationRelease {
        allocation: TraceAllocationIdV1,
    },
    UseOfUnknownAllocation {
        allocation: TraceAllocationIdV1,
    },
    MemoryOutcomeInconsistent,
}

impl fmt::Display for TraceValidationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid semantic trace: {self:?}")
    }
}

impl Error for TraceValidationErrorV1 {}
