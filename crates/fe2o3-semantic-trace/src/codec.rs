use std::error::Error;
use std::fmt;

use crate::model::*;

const TRACE_MAGIC_V1: [u8; 8] = *b"FE2O3TR1";
const MIN_EVENT_ENCODED_BYTES_V1: u64 = 57;

/// Exact canonical encoded size of one already validated event.
pub fn encoded_event_len_v1(event: &TraceEventV1) -> Result<u64, TraceEncodeErrorV1> {
    let mut encoder = Encoder::counter(MAX_TRACE_BYTES_V1);
    encode_event(&mut encoder, event)?;
    Ok(encoder.encoded_len())
}

/// Exact canonical envelope/header/count size before the first event.
pub fn encoded_trace_prefix_len_v1(header: &TraceHeaderV1) -> Result<u64, TraceEncodeErrorV1> {
    let mut encoder = Encoder::counter(header.bounds().max_encoded_bytes());
    encoder.bytes(&TRACE_MAGIC_V1)?;
    encoder.u16(TRACE_SCHEMA_VERSION_V1)?;
    encode_header(&mut encoder, header)?;
    encoder.u64(0)?;
    Ok(encoder.encoded_len())
}

pub fn encode_trace_v1(trace: &TraceV1) -> Result<Vec<u8>, TraceEncodeErrorV1> {
    trace.validate().map_err(TraceEncodeErrorV1::Validation)?;
    let limit = trace.header().bounds().max_encoded_bytes();
    let mut counter = Encoder::counter(limit);
    encode_trace(&mut counter, trace)?;
    let exact_len =
        usize::try_from(counter.encoded_len()).map_err(|_| TraceEncodeErrorV1::LengthOverflow)?;
    let mut encoder = Encoder::materializer(limit, exact_len)?;
    let output_resident = capacity_bytes::<u8>(encoder.materialized_capacity()?)
        .map_err(TraceEncodeErrorV1::Validation)?;
    ValidationResidentLedgerV1::new(trace, 0)
        .and_then(|resident| resident.ensure_temporary(output_resident))
        .map_err(TraceEncodeErrorV1::Validation)?;
    encode_trace(&mut encoder, trace)?;
    if encoder.encoded_len() != counter.encoded_len() {
        return Err(TraceEncodeErrorV1::MaterializationInvariant);
    }
    encoder.finish()
}

fn encode_trace(encoder: &mut Encoder, trace: &TraceV1) -> Result<(), TraceEncodeErrorV1> {
    encoder.bytes(&TRACE_MAGIC_V1)?;
    encoder.u16(TRACE_SCHEMA_VERSION_V1)?;
    encode_header(encoder, trace.header())?;
    encoder.u64(u64::try_from(trace.events().len()).map_err(|_| {
        TraceEncodeErrorV1::Validation(TraceValidationErrorV1::EventCountOverflow)
    })?)?;
    for event in trace.events() {
        encode_event(encoder, event)?;
    }
    Ok(())
}

pub fn decode_trace_v1(bytes: &[u8]) -> Result<TraceV1, TraceDecodeErrorV1> {
    let actual_len = u64::try_from(bytes.len()).map_err(|_| TraceDecodeErrorV1::LengthOverflow)?;
    if actual_len > MAX_TRACE_BYTES_V1 {
        return Err(TraceDecodeErrorV1::InputTooLarge {
            actual: actual_len,
            max: MAX_TRACE_BYTES_V1,
        });
    }
    let mut decoder = Decoder::new(bytes);
    if decoder.array::<8>()? != TRACE_MAGIC_V1 {
        return Err(TraceDecodeErrorV1::InvalidMagic);
    }
    let version = decoder.u16()?;
    if version != TRACE_SCHEMA_VERSION_V1 {
        return Err(TraceDecodeErrorV1::UnsupportedVersion(version));
    }
    let header = decode_header(&mut decoder)?;
    if actual_len > header.bounds().max_encoded_bytes() {
        return Err(TraceDecodeErrorV1::DeclaredByteLimitExceeded {
            actual: actual_len,
            max: header.bounds().max_encoded_bytes(),
        });
    }
    let event_count = decoder.u64()?;
    if event_count > header.bounds().max_events() || event_count > MAX_TRACE_EVENTS_V1 {
        return Err(TraceDecodeErrorV1::Validation(
            TraceValidationErrorV1::TooManyEvents {
                actual: event_count,
                max: header.bounds().max_events().min(MAX_TRACE_EVENTS_V1),
            },
        ));
    }
    let remaining =
        u64::try_from(decoder.remaining()).map_err(|_| TraceDecodeErrorV1::LengthOverflow)?;
    let maximum_possible = remaining / MIN_EVENT_ENCODED_BYTES_V1;
    if event_count > maximum_possible {
        return Err(TraceDecodeErrorV1::ImpossibleEventCount {
            declared: event_count,
            remaining_bytes: remaining,
            minimum_event_bytes: MIN_EVENT_ENCODED_BYTES_V1,
        });
    }
    let capacity = usize::try_from(event_count).map_err(|_| TraceDecodeErrorV1::LengthOverflow)?;
    let resident_events = event_count
        .checked_mul(std::mem::size_of::<TraceEventV1>() as u64)
        .ok_or(TraceDecodeErrorV1::LengthOverflow)?;
    let resident_required = resident_events
        .checked_add(header.bounds().max_encoded_bytes())
        .ok_or(TraceDecodeErrorV1::LengthOverflow)?;
    if resident_required > header.bounds().max_resident_bytes() {
        return Err(TraceDecodeErrorV1::Validation(
            TraceValidationErrorV1::ResidentLimitExceeded {
                actual: resident_required,
                max: header.bounds().max_resident_bytes(),
            },
        ));
    }
    let mut events = Vec::new();
    events
        .try_reserve_exact(capacity)
        .map_err(|_| TraceDecodeErrorV1::AllocationFailed {
            requested: capacity,
        })?;
    for _ in 0..event_count {
        events.push(decode_event(&mut decoder)?);
    }
    if decoder.remaining() != 0 {
        return Err(TraceDecodeErrorV1::TrailingBytes(decoder.remaining()));
    }
    let trace = TraceV1::new(header, events).map_err(TraceDecodeErrorV1::Validation)?;
    let canonical = encode_trace_v1(&trace).map_err(TraceDecodeErrorV1::Reencode)?;
    if canonical != bytes {
        return Err(TraceDecodeErrorV1::NonCanonicalEncoding);
    }
    Ok(trace)
}

fn encode_header(encoder: &mut Encoder, header: &TraceHeaderV1) -> Result<(), TraceEncodeErrorV1> {
    encode_producer(encoder, header.producer())?;
    encoder.u8(execution_kind_tag(header.execution_kind()))?;
    encode_kernel_ir_claim(encoder, header.kernel_ir_claim())?;
    encode_optional_content_identity(encoder, header.semantic_mir())?;
    encode_optional_content_identity(encoder, header.lineage())?;
    encode_optional_content_identity(encoder, header.artifact())?;
    encode_dispatch_identity(encoder, header.dispatch())?;
    let launch = header.launch();
    encode_u64x3(encoder, launch.logical_grid())?;
    encode_u32x3(encoder, launch.grid_workgroups())?;
    encode_u32x3(encoder, launch.workgroup_size())?;
    encoder.u8(wave_width_tag(launch.wave_width()))?;
    let bounds = header.bounds();
    encoder.u64(bounds.max_events())?;
    encoder.u64(bounds.max_encoded_bytes())?;
    encoder.u64(bounds.max_resident_bytes())?;
    encoder.u16(bounds.max_evidence_refs_per_event())?;
    encode_completeness(encoder, header.completeness())?;
    encode_capture_boundaries(encoder, header.boundaries())
}

fn decode_header(decoder: &mut Decoder<'_>) -> Result<TraceHeaderV1, TraceDecodeErrorV1> {
    let producer = decode_producer(decoder)?;
    let execution_kind = decode_execution_kind(decoder.u8()?)?;
    let kernel_ir_claim = decode_kernel_ir_claim(decoder)?;
    let semantic_mir = decode_optional_content_identity(decoder)?;
    let lineage = decode_optional_content_identity(decoder)?;
    let artifact = decode_optional_content_identity(decoder)?;
    let dispatch = decode_dispatch_identity(decoder)?;
    let launch = LaunchGeometryV1::new_exact(
        decode_u64x3(decoder)?,
        decode_u32x3(decoder)?,
        decode_u32x3(decoder)?,
        decode_wave_width(decoder.u8()?)?,
    )
    .map_err(TraceDecodeErrorV1::Validation)?;
    let bounds = TraceBoundsV1::new_with_resident(
        decoder.u64()?,
        decoder.u64()?,
        decoder.u64()?,
        decoder.u16()?,
    )
    .map_err(TraceDecodeErrorV1::Validation)?;
    let completeness = decode_completeness(decoder)?;
    let boundaries = decode_capture_boundaries(decoder)?;
    TraceHeaderV1::new(
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
    )
    .map_err(TraceDecodeErrorV1::Validation)
}

fn encode_producer(
    encoder: &mut Encoder,
    producer: &ProducerIdentityV1,
) -> Result<(), TraceEncodeErrorV1> {
    encoder.u8(producer_kind_tag(producer.kind()))?;
    encoder.text(producer.name())?;
    encoder.text(producer.version())?;
    encode_option(encoder, producer.executable(), |encoder, identity| {
        encode_identity(encoder, identity)
    })
}

fn decode_producer(decoder: &mut Decoder<'_>) -> Result<ProducerIdentityV1, TraceDecodeErrorV1> {
    let kind = decode_producer_kind(decoder.u8()?)?;
    let name = decoder.text()?;
    let version = decoder.text()?;
    let executable = decode_option(decoder, decode_identity)?;
    Ok(ProducerIdentityV1::new(kind, name, version, executable))
}

fn encode_kernel_ir_claim(
    encoder: &mut Encoder,
    identity: KernelIrIdentityClaimV1,
) -> Result<(), TraceEncodeErrorV1> {
    encoder.u16(identity.wire_version())?;
    encoder.u16(identity.identity_policy())?;
    encode_identity(encoder, identity.digest())?;
    encoder.u64(identity.canonical_len())
}

fn decode_kernel_ir_claim(
    decoder: &mut Decoder<'_>,
) -> Result<KernelIrIdentityClaimV1, TraceDecodeErrorV1> {
    let wire_version = decoder.u16()?;
    let identity_policy = decoder.u16()?;
    if wire_version != KERNEL_IR_WIRE_VERSION_V7 || identity_policy != KERNEL_IR_IDENTITY_POLICY_V1
    {
        return Err(TraceDecodeErrorV1::UnsupportedKernelIrClaim {
            wire_version,
            identity_policy,
        });
    }
    KernelIrIdentityClaimV1::canonical_v7_claim(decode_identity(decoder)?, decoder.u64()?)
        .map_err(TraceDecodeErrorV1::Validation)
}

fn encode_optional_content_identity(
    encoder: &mut Encoder,
    identity: Option<ContentIdentityV1>,
) -> Result<(), TraceEncodeErrorV1> {
    encode_option(encoder, identity, |encoder, identity| {
        encoder.u8(content_identity_scheme_tag(identity.scheme()))?;
        encoder.u16(identity.format_version())?;
        encode_identity(encoder, identity.digest())?;
        encoder.u64(identity.canonical_len())
    })
}

fn decode_optional_content_identity(
    decoder: &mut Decoder<'_>,
) -> Result<Option<ContentIdentityV1>, TraceDecodeErrorV1> {
    decode_option(decoder, |decoder| {
        ContentIdentityV1::new(
            decode_content_identity_scheme(decoder.u8()?)?,
            decoder.u16()?,
            decode_identity(decoder)?,
            decoder.u64()?,
        )
        .map_err(TraceDecodeErrorV1::Validation)
    })
}

fn encode_completeness(
    encoder: &mut Encoder,
    completeness: TraceCompletenessV1,
) -> Result<(), TraceEncodeErrorV1> {
    match completeness {
        TraceCompletenessV1::Complete => encoder.u8(0),
        TraceCompletenessV1::Truncated {
            reason,
            emitted_events,
            dropped_events,
        } => {
            encoder.u8(1)?;
            encoder.u8(truncation_reason_tag(reason))?;
            encoder.u64(emitted_events)?;
            match dropped_events {
                DroppedEventCountV1::Unknown => encoder.u8(0),
                DroppedEventCountV1::Known(count) => {
                    encoder.u8(1)?;
                    encoder.u64(count)
                }
            }
        }
    }
}

fn decode_completeness(
    decoder: &mut Decoder<'_>,
) -> Result<TraceCompletenessV1, TraceDecodeErrorV1> {
    match decoder.u8()? {
        0 => Ok(TraceCompletenessV1::Complete),
        1 => {
            let reason = decode_truncation_reason(decoder.u8()?)?;
            let emitted_events = decoder.u64()?;
            let dropped_events = match decoder.u8()? {
                0 => DroppedEventCountV1::Unknown,
                1 => DroppedEventCountV1::Known(decoder.u64()?),
                tag => return Err(unknown_tag("dropped event count", tag)),
            };
            Ok(TraceCompletenessV1::Truncated {
                reason,
                emitted_events,
                dropped_events,
            })
        }
        tag => Err(unknown_tag("trace completeness", tag)),
    }
}

fn encode_capture_boundaries(
    encoder: &mut Encoder,
    boundaries: CaptureBoundariesV1,
) -> Result<(), TraceEncodeErrorV1> {
    encoder.u8(match boundaries.start() {
        CaptureStartBoundaryV1::DispatchBeginIncluded => 0,
        CaptureStartBoundaryV1::DispatchAlreadyActive => 1,
    })?;
    encoder.u8(match boundaries.end() {
        CaptureEndBoundaryV1::DispatchEndIncluded => 0,
        CaptureEndBoundaryV1::DispatchContinuesAfterCapture => 1,
    })
}

fn decode_capture_boundaries(
    decoder: &mut Decoder<'_>,
) -> Result<CaptureBoundariesV1, TraceDecodeErrorV1> {
    let start = match decoder.u8()? {
        0 => CaptureStartBoundaryV1::DispatchBeginIncluded,
        1 => CaptureStartBoundaryV1::DispatchAlreadyActive,
        tag => return Err(unknown_tag("capture start boundary", tag)),
    };
    let end = match decoder.u8()? {
        0 => CaptureEndBoundaryV1::DispatchEndIncluded,
        1 => CaptureEndBoundaryV1::DispatchContinuesAfterCapture,
        tag => return Err(unknown_tag("capture end boundary", tag)),
    };
    Ok(CaptureBoundariesV1::new(start, end))
}

fn encode_event(encoder: &mut Encoder, event: &TraceEventV1) -> Result<(), TraceEncodeErrorV1> {
    encoder.u64(event.sequence())?;
    encode_timestamp(encoder, event.timestamp())?;
    encode_provenance(encoder, event.provenance())?;
    encode_scope(encoder, event.scope())?;
    encode_option(encoder, event.site(), encode_site)?;
    encode_event_kind(encoder, event.kind())?;
    encoder.u16(u16::try_from(event.evidence_refs().len()).map_err(|_| {
        TraceEncodeErrorV1::Validation(TraceValidationErrorV1::TooManyEvidenceReferences {
            actual: event.evidence_refs().len(),
            max: MAX_EVIDENCE_REFS_PER_EVENT_V1,
        })
    })?)?;
    for evidence in event.evidence_refs() {
        encode_evidence(encoder, *evidence)?;
    }
    Ok(())
}

fn decode_event(decoder: &mut Decoder<'_>) -> Result<TraceEventV1, TraceDecodeErrorV1> {
    let sequence = decoder.u64()?;
    let timestamp = decode_timestamp(decoder)?;
    let provenance = decode_provenance(decoder)?;
    let scope = decode_scope(decoder)?;
    let site = decode_option(decoder, decode_site)?;
    let kind = decode_event_kind(decoder)?;
    let evidence_count = usize::from(decoder.u16()?);
    if evidence_count > MAX_EVIDENCE_REFS_PER_EVENT_V1 {
        return Err(TraceDecodeErrorV1::Validation(
            TraceValidationErrorV1::TooManyEvidenceReferences {
                actual: evidence_count,
                max: MAX_EVIDENCE_REFS_PER_EVENT_V1,
            },
        ));
    }
    let mut evidence_refs = Vec::new();
    evidence_refs
        .try_reserve_exact(evidence_count)
        .map_err(|_| TraceDecodeErrorV1::AllocationFailed {
            requested: evidence_count,
        })?;
    for _ in 0..evidence_count {
        evidence_refs.push(decode_evidence(decoder)?);
    }
    TraceEventV1::new(
        sequence,
        timestamp,
        provenance,
        scope,
        site,
        kind,
        evidence_refs,
    )
    .map_err(TraceDecodeErrorV1::Validation)
}

fn encode_timestamp(
    encoder: &mut Encoder,
    timestamp: TimestampV1,
) -> Result<(), TraceEncodeErrorV1> {
    match timestamp {
        TimestampV1::LogicalStep(step) => {
            encoder.u8(0)?;
            encoder.u64(step)
        }
        TimestampV1::Clock { domain, ticks } => {
            encoder.u8(1)?;
            encode_identity(encoder, domain)?;
            encoder.u64(ticks)
        }
    }
}

fn decode_timestamp(decoder: &mut Decoder<'_>) -> Result<TimestampV1, TraceDecodeErrorV1> {
    match decoder.u8()? {
        0 => Ok(TimestampV1::LogicalStep(decoder.u64()?)),
        1 => Ok(TimestampV1::Clock {
            domain: decode_identity(decoder)?,
            ticks: decoder.u64()?,
        }),
        tag => Err(unknown_tag("timestamp", tag)),
    }
}

fn encode_provenance(
    encoder: &mut Encoder,
    provenance: FactProvenanceV1,
) -> Result<(), TraceEncodeErrorV1> {
    match provenance {
        FactProvenanceV1::Declared => encoder.u8(0),
        FactProvenanceV1::Proved => encoder.u8(1),
        FactProvenanceV1::Observed => encoder.u8(2),
        FactProvenanceV1::Inferred => encoder.u8(3),
        FactProvenanceV1::Unavailable { reason } => {
            encoder.u8(4)?;
            encoder.u8(unavailable_reason_tag(reason))
        }
    }
}

fn decode_provenance(decoder: &mut Decoder<'_>) -> Result<FactProvenanceV1, TraceDecodeErrorV1> {
    match decoder.u8()? {
        0 => Ok(FactProvenanceV1::Declared),
        1 => Ok(FactProvenanceV1::Proved),
        2 => Ok(FactProvenanceV1::Observed),
        3 => Ok(FactProvenanceV1::Inferred),
        4 => Ok(FactProvenanceV1::Unavailable {
            reason: decode_unavailable_reason(decoder.u8()?)?,
        }),
        tag => Err(unknown_tag("fact provenance", tag)),
    }
}

fn encode_scope(encoder: &mut Encoder, scope: ExecutionScopeV1) -> Result<(), TraceEncodeErrorV1> {
    encode_dispatch_identity(encoder, scope.dispatch_identity())?;
    match scope.level() {
        ExecutionLevelV1::Dispatch => encoder.u8(0),
        ExecutionLevelV1::Workgroup { workgroup } => {
            encoder.u8(1)?;
            encode_u32x3(encoder, workgroup)
        }
        ExecutionLevelV1::Wave {
            workgroup,
            wave,
            active_mask,
        } => {
            encoder.u8(2)?;
            encode_u32x3(encoder, workgroup)?;
            encoder.u32(wave)?;
            encode_active_mask(encoder, active_mask)
        }
        ExecutionLevelV1::Lane {
            workgroup,
            wave,
            lane,
            logical_workitem,
            active_mask,
        } => {
            encoder.u8(3)?;
            encode_u32x3(encoder, workgroup)?;
            encoder.u32(wave)?;
            encoder.u16(lane)?;
            encode_u64x3(encoder, logical_workitem)?;
            encode_active_mask(encoder, active_mask)
        }
    }
}

fn decode_scope(decoder: &mut Decoder<'_>) -> Result<ExecutionScopeV1, TraceDecodeErrorV1> {
    let dispatch = decode_dispatch_identity(decoder)?;
    match decoder.u8()? {
        0 => Ok(ExecutionScopeV1::dispatch(dispatch)),
        1 => Ok(ExecutionScopeV1::workgroup(
            dispatch,
            decode_u32x3(decoder)?,
        )),
        2 => Ok(ExecutionScopeV1::wave(
            dispatch,
            decode_u32x3(decoder)?,
            decoder.u32()?,
            decode_active_mask(decoder)?,
        )),
        3 => Ok(ExecutionScopeV1::lane(
            dispatch,
            decode_u32x3(decoder)?,
            decoder.u32()?,
            decoder.u16()?,
            decode_u64x3(decoder)?,
            decode_active_mask(decoder)?,
        )),
        tag => Err(unknown_tag("execution level", tag)),
    }
}

fn encode_site(encoder: &mut Encoder, site: KirSiteClaimV1) -> Result<(), TraceEncodeErrorV1> {
    encoder.u64(site.function_ordinal())?;
    encoder.u64(site.block_ordinal())?;
    match site.point() {
        KirSitePointV1::BlockEntry => encoder.u8(0),
        KirSitePointV1::Operation(ordinal) => {
            encoder.u8(1)?;
            encoder.u64(ordinal)
        }
        KirSitePointV1::Terminator => encoder.u8(2),
    }
}

fn decode_site(decoder: &mut Decoder<'_>) -> Result<KirSiteClaimV1, TraceDecodeErrorV1> {
    let function_ordinal = decoder.u64()?;
    let block_ordinal = decoder.u64()?;
    let point = match decoder.u8()? {
        0 => KirSitePointV1::BlockEntry,
        1 => KirSitePointV1::Operation(decoder.u64()?),
        2 => KirSitePointV1::Terminator,
        tag => return Err(unknown_tag("KIR site point", tag)),
    };
    Ok(KirSiteClaimV1::new(function_ordinal, block_ordinal, point))
}

fn encode_event_kind(
    encoder: &mut Encoder,
    kind: TraceEventKindV1,
) -> Result<(), TraceEncodeErrorV1> {
    match kind {
        TraceEventKindV1::Dispatch(event) => {
            encoder.u8(0)?;
            match event {
                DispatchEventV1::Begin => encoder.u8(0),
                DispatchEventV1::End(outcome) => {
                    encoder.u8(1)?;
                    encoder.u8(dispatch_outcome_tag(outcome))
                }
            }
        }
        TraceEventKindV1::Invocation(event) => {
            encoder.u8(1)?;
            encoder.u8(match event {
                InvocationEventV1::Begin => 0,
                InvocationEventV1::End => 1,
            })
        }
        TraceEventKindV1::BlockEnter => encoder.u8(2),
        TraceEventKindV1::Operation(event) => {
            encoder.u8(3)?;
            let occurrence = match event {
                OperationEventV1::Begin(occurrence) => {
                    encoder.u8(0)?;
                    occurrence
                }
                OperationEventV1::End(occurrence) => {
                    encoder.u8(1)?;
                    occurrence
                }
            };
            encoder.u64(occurrence.frame())?;
            encoder.u64(occurrence.occurrence())
        }
        TraceEventKindV1::Branch {
            target_block_ordinal,
        } => {
            encoder.u8(4)?;
            encoder.u64(target_block_ordinal)
        }
        TraceEventKindV1::Memory(event) => {
            encoder.u8(5)?;
            encode_memory_event(encoder, event)
        }
        TraceEventKindV1::Barrier(event) => {
            encoder.u8(6)?;
            encode_barrier_event(encoder, event)
        }
        TraceEventKindV1::Allocation(event) => {
            encoder.u8(7)?;
            encode_allocation_event(encoder, event)
        }
        TraceEventKindV1::Diagnostic(event) => {
            encoder.u8(8)?;
            encoder.u8(diagnostic_kind_tag(event.kind()))?;
            encoder.u32(event.code())
        }
    }
}

fn decode_event_kind(decoder: &mut Decoder<'_>) -> Result<TraceEventKindV1, TraceDecodeErrorV1> {
    match decoder.u8()? {
        0 => Ok(TraceEventKindV1::Dispatch(match decoder.u8()? {
            0 => DispatchEventV1::Begin,
            1 => DispatchEventV1::End(decode_dispatch_outcome(decoder.u8()?)?),
            tag => return Err(unknown_tag("dispatch event", tag)),
        })),
        1 => Ok(TraceEventKindV1::Invocation(match decoder.u8()? {
            0 => InvocationEventV1::Begin,
            1 => InvocationEventV1::End,
            tag => return Err(unknown_tag("invocation event", tag)),
        })),
        2 => Ok(TraceEventKindV1::BlockEnter),
        3 => {
            let phase = decoder.u8()?;
            let occurrence = OperationOccurrenceIdV1::new(decoder.u64()?, decoder.u64()?)
                .map_err(TraceDecodeErrorV1::Validation)?;
            Ok(TraceEventKindV1::Operation(match phase {
                0 => OperationEventV1::Begin(occurrence),
                1 => OperationEventV1::End(occurrence),
                tag => return Err(unknown_tag("operation event", tag)),
            }))
        }
        4 => Ok(TraceEventKindV1::Branch {
            target_block_ordinal: decoder.u64()?,
        }),
        5 => Ok(TraceEventKindV1::Memory(decode_memory_event(decoder)?)),
        6 => Ok(TraceEventKindV1::Barrier(decode_barrier_event(decoder)?)),
        7 => Ok(TraceEventKindV1::Allocation(decode_allocation_event(
            decoder,
        )?)),
        8 => Ok(TraceEventKindV1::Diagnostic(DiagnosticEventV1::new(
            decode_diagnostic_kind(decoder.u8()?)?,
            decoder.u32()?,
        ))),
        tag => Err(unknown_tag("trace event", tag)),
    }
}

fn encode_memory_event(
    encoder: &mut Encoder,
    event: MemoryEventV1,
) -> Result<(), TraceEncodeErrorV1> {
    encoder.u8(memory_access_kind_tag(event.kind()))?;
    encode_allocation_id(encoder, event.allocation())?;
    encoder.u64(event.byte_offset())?;
    encoder.u64(event.byte_len())?;
    encoder.u8(address_space_tag(event.address_space()))?;
    match event.outcome() {
        MemoryOutcomeV1::Completed => encoder.u8(0),
        MemoryOutcomeV1::Fault(fault) => {
            encoder.u8(1)?;
            encoder.u8(memory_fault_kind_tag(fault))
        }
        MemoryOutcomeV1::Unavailable(reason) => {
            encoder.u8(2)?;
            encoder.u8(unavailable_reason_tag(reason))
        }
    }
}

fn decode_memory_event(decoder: &mut Decoder<'_>) -> Result<MemoryEventV1, TraceDecodeErrorV1> {
    let kind = decode_memory_access_kind(decoder.u8()?)?;
    let allocation = decode_allocation_id(decoder)?;
    let byte_offset = decoder.u64()?;
    let byte_len = decoder.u64()?;
    let address_space = decode_address_space(decoder.u8()?)?;
    let outcome = match decoder.u8()? {
        0 => MemoryOutcomeV1::Completed,
        1 => MemoryOutcomeV1::Fault(decode_memory_fault_kind(decoder.u8()?)?),
        2 => MemoryOutcomeV1::Unavailable(decode_unavailable_reason(decoder.u8()?)?),
        tag => return Err(unknown_tag("memory outcome", tag)),
    };
    MemoryEventV1::new(
        kind,
        allocation,
        byte_offset,
        byte_len,
        address_space,
        outcome,
    )
    .map_err(TraceDecodeErrorV1::Validation)
}

fn encode_barrier_event(
    encoder: &mut Encoder,
    event: BarrierEventV1,
) -> Result<(), TraceEncodeErrorV1> {
    encoder.u32(event.barrier_id())?;
    encoder.u64(event.phase())?;
    encoder.u8(match event.scope() {
        BarrierScopeV1::Wave => 0,
        BarrierScopeV1::Workgroup => 1,
    })?;
    encoder.u8(match event.action() {
        BarrierActionV1::Arrive => 0,
        BarrierActionV1::Release => 1,
    })
}

fn decode_barrier_event(decoder: &mut Decoder<'_>) -> Result<BarrierEventV1, TraceDecodeErrorV1> {
    let barrier_id = decoder.u32()?;
    let phase = decoder.u64()?;
    let scope = match decoder.u8()? {
        0 => BarrierScopeV1::Wave,
        1 => BarrierScopeV1::Workgroup,
        tag => return Err(unknown_tag("barrier scope", tag)),
    };
    let action = match decoder.u8()? {
        0 => BarrierActionV1::Arrive,
        1 => BarrierActionV1::Release,
        tag => return Err(unknown_tag("barrier action", tag)),
    };
    Ok(BarrierEventV1::new(barrier_id, phase, scope, action))
}

fn encode_allocation_event(
    encoder: &mut Encoder,
    event: AllocationEventV1,
) -> Result<(), TraceEncodeErrorV1> {
    match event {
        AllocationEventV1::Create {
            allocation,
            byte_len,
            address_space,
        } => {
            encoder.u8(0)?;
            encode_allocation_id(encoder, allocation)?;
            encoder.u64(byte_len)?;
            encoder.u8(address_space_tag(address_space))
        }
        AllocationEventV1::Preexisting {
            allocation,
            byte_len,
            address_space,
        } => {
            encoder.u8(1)?;
            encode_allocation_id(encoder, allocation)?;
            encoder.u64(byte_len)?;
            encoder.u8(address_space_tag(address_space))
        }
        AllocationEventV1::UnknownLifecycle { allocation } => {
            encoder.u8(2)?;
            encode_allocation_id(encoder, allocation)
        }
        AllocationEventV1::Release { allocation } => {
            encoder.u8(3)?;
            encode_allocation_id(encoder, allocation)
        }
    }
}

fn decode_allocation_event(
    decoder: &mut Decoder<'_>,
) -> Result<AllocationEventV1, TraceDecodeErrorV1> {
    match decoder.u8()? {
        0 => Ok(AllocationEventV1::Create {
            allocation: decode_allocation_id(decoder)?,
            byte_len: decoder.u64()?,
            address_space: decode_address_space(decoder.u8()?)?,
        }),
        1 => Ok(AllocationEventV1::Preexisting {
            allocation: decode_allocation_id(decoder)?,
            byte_len: decoder.u64()?,
            address_space: decode_address_space(decoder.u8()?)?,
        }),
        2 => Ok(AllocationEventV1::UnknownLifecycle {
            allocation: decode_allocation_id(decoder)?,
        }),
        3 => Ok(AllocationEventV1::Release {
            allocation: decode_allocation_id(decoder)?,
        }),
        tag => Err(unknown_tag("allocation event", tag)),
    }
}

fn encode_allocation_id(
    encoder: &mut Encoder,
    allocation: TraceAllocationIdV1,
) -> Result<(), TraceEncodeErrorV1> {
    encoder.u64(allocation.ordinal())?;
    encoder.u64(allocation.generation())
}

fn decode_allocation_id(
    decoder: &mut Decoder<'_>,
) -> Result<TraceAllocationIdV1, TraceDecodeErrorV1> {
    TraceAllocationIdV1::new(decoder.u64()?, decoder.u64()?).map_err(TraceDecodeErrorV1::Validation)
}

fn encode_dispatch_identity(
    encoder: &mut Encoder,
    dispatch: DispatchIdentityV1,
) -> Result<(), TraceEncodeErrorV1> {
    encoder.u8(dispatch_identity_domain_tag(dispatch.domain()))?;
    encode_identity(encoder, dispatch.identity())
}

fn decode_dispatch_identity(
    decoder: &mut Decoder<'_>,
) -> Result<DispatchIdentityV1, TraceDecodeErrorV1> {
    Ok(DispatchIdentityV1::new(
        decode_dispatch_identity_domain(decoder.u8()?)?,
        decode_identity(decoder)?,
    ))
}

fn encode_active_mask(encoder: &mut Encoder, mask: ActiveMaskV1) -> Result<(), TraceEncodeErrorV1> {
    encoder.u8(wave_width_tag(mask.width()))?;
    encoder.u64(mask.bits())
}

fn decode_active_mask(decoder: &mut Decoder<'_>) -> Result<ActiveMaskV1, TraceDecodeErrorV1> {
    ActiveMaskV1::new(decode_wave_width(decoder.u8()?)?, decoder.u64()?)
        .map_err(TraceDecodeErrorV1::Validation)
}

fn encode_evidence(
    encoder: &mut Encoder,
    evidence: EvidenceRefV1,
) -> Result<(), TraceEncodeErrorV1> {
    encoder.u8(evidence_kind_tag(evidence.kind()))?;
    encode_identity(encoder, evidence.identity())
}

fn decode_evidence(decoder: &mut Decoder<'_>) -> Result<EvidenceRefV1, TraceDecodeErrorV1> {
    Ok(EvidenceRefV1::new(
        decode_evidence_kind(decoder.u8()?)?,
        decode_identity(decoder)?,
    ))
}

fn encode_identity(
    encoder: &mut Encoder,
    identity: OpaqueIdentityV1,
) -> Result<(), TraceEncodeErrorV1> {
    encoder.bytes(identity.as_bytes())
}

fn decode_identity(decoder: &mut Decoder<'_>) -> Result<OpaqueIdentityV1, TraceDecodeErrorV1> {
    let bytes = decoder.array::<32>()?;
    OpaqueIdentityV1::new(bytes).map_err(TraceDecodeErrorV1::Validation)
}

fn encode_option<T>(
    encoder: &mut Encoder,
    value: Option<T>,
    encode: impl FnOnce(&mut Encoder, T) -> Result<(), TraceEncodeErrorV1>,
) -> Result<(), TraceEncodeErrorV1> {
    match value {
        None => encoder.u8(0),
        Some(value) => {
            encoder.u8(1)?;
            encode(encoder, value)
        }
    }
}

fn decode_option<T>(
    decoder: &mut Decoder<'_>,
    decode: impl FnOnce(&mut Decoder<'_>) -> Result<T, TraceDecodeErrorV1>,
) -> Result<Option<T>, TraceDecodeErrorV1> {
    match decoder.u8()? {
        0 => Ok(None),
        1 => decode(decoder).map(Some),
        tag => Err(unknown_tag("option", tag)),
    }
}

fn encode_u32x3(encoder: &mut Encoder, values: [u32; 3]) -> Result<(), TraceEncodeErrorV1> {
    for value in values {
        encoder.u32(value)?;
    }
    Ok(())
}

fn encode_u64x3(encoder: &mut Encoder, values: [u64; 3]) -> Result<(), TraceEncodeErrorV1> {
    for value in values {
        encoder.u64(value)?;
    }
    Ok(())
}

fn decode_u32x3(decoder: &mut Decoder<'_>) -> Result<[u32; 3], TraceDecodeErrorV1> {
    Ok([decoder.u32()?, decoder.u32()?, decoder.u32()?])
}

fn decode_u64x3(decoder: &mut Decoder<'_>) -> Result<[u64; 3], TraceDecodeErrorV1> {
    Ok([decoder.u64()?, decoder.u64()?, decoder.u64()?])
}

macro_rules! tag_codec {
    ($encode:ident, $decode:ident, $ty:ty, $context:literal, { $($variant:path => $tag:literal),+ $(,)? }) => {
        fn $encode(value: $ty) -> u8 {
            match value { $($variant => $tag),+ }
        }

        fn $decode(tag: u8) -> Result<$ty, TraceDecodeErrorV1> {
            match tag {
                $($tag => Ok($variant)),+,
                tag => Err(unknown_tag($context, tag)),
            }
        }
    };
}

tag_codec!(producer_kind_tag, decode_producer_kind, ProducerKindV1, "producer kind", {
    ProducerKindV1::CpuKirSimulator => 0,
    ProducerKindV1::KfdHardwareCollector => 1,
    ProducerKindV1::RocgdbImporter => 2,
    ProducerKindV1::RocprofImporter => 3,
});
tag_codec!(execution_kind_tag, decode_execution_kind, ExecutionKindV1, "execution kind", {
    ExecutionKindV1::CpuKirSimulation => 0,
    ExecutionKindV1::KfdHardware => 1,
    ExecutionKindV1::RocgdbImport => 2,
    ExecutionKindV1::RocprofImport => 3,
});
tag_codec!(content_identity_scheme_tag, decode_content_identity_scheme, ContentIdentitySchemeV1, "content identity scheme", {
    ContentIdentitySchemeV1::RawCanonicalSha256 => 0,
    ContentIdentitySchemeV1::DomainSeparatedSha256 => 1,
});
tag_codec!(wave_width_tag, decode_wave_width, WaveWidthV1, "wave width", {
    WaveWidthV1::Wave32 => 0,
    WaveWidthV1::Wave64 => 1,
});
tag_codec!(truncation_reason_tag, decode_truncation_reason, TruncationReasonV1, "truncation reason", {
    TruncationReasonV1::EventLimit => 0,
    TruncationReasonV1::ByteLimit => 1,
    TruncationReasonV1::CollectorLoss => 2,
    TruncationReasonV1::ProducerFailure => 3,
    TruncationReasonV1::UserStopped => 4,
});
tag_codec!(evidence_kind_tag, decode_evidence_kind, EvidenceKindV1, "evidence kind", {
    EvidenceKindV1::Declaration => 0,
    EvidenceKindV1::Proof => 1,
    EvidenceKindV1::InferenceRule => 2,
    EvidenceKindV1::RuntimeObservation => 3,
    EvidenceKindV1::Artifact => 4,
});
tag_codec!(unavailable_reason_tag, decode_unavailable_reason, UnavailableReasonV1, "unavailable reason", {
    UnavailableReasonV1::Unsupported => 0,
    UnavailableReasonV1::NotCaptured => 1,
    UnavailableReasonV1::OptimizedOut => 2,
    UnavailableReasonV1::OutsideCaptureScope => 3,
    UnavailableReasonV1::Truncated => 4,
});
tag_codec!(dispatch_identity_domain_tag, decode_dispatch_identity_domain, DispatchIdentityDomainV1, "dispatch identity domain", {
    DispatchIdentityDomainV1::TraceLocal => 0,
    DispatchIdentityDomainV1::RuntimeModel => 1,
    DispatchIdentityDomainV1::ImportedCollector => 2,
});
tag_codec!(dispatch_outcome_tag, decode_dispatch_outcome, DispatchOutcomeV1, "dispatch outcome", {
    DispatchOutcomeV1::Completed => 0,
    DispatchOutcomeV1::Failed => 1,
    DispatchOutcomeV1::Cancelled => 2,
});
tag_codec!(memory_access_kind_tag, decode_memory_access_kind, MemoryAccessKindV1, "memory access kind", {
    MemoryAccessKindV1::Read => 0,
    MemoryAccessKindV1::Write => 1,
    MemoryAccessKindV1::Atomic => 2,
});
tag_codec!(address_space_tag, decode_address_space, AddressSpaceV1, "address space", {
    AddressSpaceV1::Private => 0,
    AddressSpaceV1::Workgroup => 1,
    AddressSpaceV1::Global => 2,
    AddressSpaceV1::Constant => 3,
    AddressSpaceV1::Generic => 4,
});
tag_codec!(memory_fault_kind_tag, decode_memory_fault_kind, MemoryFaultKindV1, "memory fault kind", {
    MemoryFaultKindV1::OutOfBounds => 0,
    MemoryFaultKindV1::Misaligned => 1,
    MemoryFaultKindV1::InvalidAddressSpace => 2,
    MemoryFaultKindV1::UseAfterRelease => 3,
    MemoryFaultKindV1::Uninitialized => 4,
    MemoryFaultKindV1::PermissionDenied => 5,
    MemoryFaultKindV1::Unknown => 6,
});
tag_codec!(diagnostic_kind_tag, decode_diagnostic_kind, DiagnosticKindV1, "diagnostic kind", {
    DiagnosticKindV1::Trap => 0,
    DiagnosticKindV1::Assert => 1,
    DiagnosticKindV1::Fault => 2,
});

fn unknown_tag(context: &'static str, tag: u8) -> TraceDecodeErrorV1 {
    TraceDecodeErrorV1::UnknownTag { context, tag }
}

struct Encoder {
    bytes: Option<Vec<u8>>,
    encoded_len: u64,
    limit: u64,
}

impl Encoder {
    fn materializer(limit: u64, exact_len: usize) -> Result<Self, TraceEncodeErrorV1> {
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(exact_len)
            .map_err(|_| TraceEncodeErrorV1::AllocationFailed {
                requested: exact_len,
            })?;
        Ok(Self {
            bytes: Some(bytes),
            encoded_len: 0,
            limit,
        })
    }

    fn counter(limit: u64) -> Self {
        Self {
            bytes: None,
            encoded_len: 0,
            limit,
        }
    }

    fn finish(self) -> Result<Vec<u8>, TraceEncodeErrorV1> {
        self.bytes
            .ok_or(TraceEncodeErrorV1::MaterializationInvariant)
    }

    const fn encoded_len(&self) -> u64 {
        self.encoded_len
    }

    fn materialized_capacity(&self) -> Result<usize, TraceEncodeErrorV1> {
        self.bytes
            .as_ref()
            .map(Vec::capacity)
            .ok_or(TraceEncodeErrorV1::MaterializationInvariant)
    }

    fn bytes(&mut self, value: &[u8]) -> Result<(), TraceEncodeErrorV1> {
        let additional =
            u64::try_from(value.len()).map_err(|_| TraceEncodeErrorV1::LengthOverflow)?;
        let next = self
            .encoded_len
            .checked_add(additional)
            .ok_or(TraceEncodeErrorV1::LengthOverflow)?;
        if next > self.limit || next > MAX_TRACE_BYTES_V1 {
            return Err(TraceEncodeErrorV1::EncodedLengthExceedsLimit {
                attempted: next,
                max: self.limit.min(MAX_TRACE_BYTES_V1),
            });
        }
        if let Some(bytes) = &mut self.bytes {
            if bytes.capacity().saturating_sub(bytes.len()) < value.len() {
                return Err(TraceEncodeErrorV1::MaterializationInvariant);
            }
            bytes.extend_from_slice(value);
        }
        self.encoded_len = next;
        Ok(())
    }

    fn u8(&mut self, value: u8) -> Result<(), TraceEncodeErrorV1> {
        self.bytes(&[value])
    }

    fn u16(&mut self, value: u16) -> Result<(), TraceEncodeErrorV1> {
        self.bytes(&value.to_le_bytes())
    }

    fn u32(&mut self, value: u32) -> Result<(), TraceEncodeErrorV1> {
        self.bytes(&value.to_le_bytes())
    }

    fn u64(&mut self, value: u64) -> Result<(), TraceEncodeErrorV1> {
        self.bytes(&value.to_le_bytes())
    }

    fn text(&mut self, value: &ProducerTextV1) -> Result<(), TraceEncodeErrorV1> {
        let bytes = value.as_str().as_bytes();
        let len = u16::try_from(bytes.len()).map_err(|_| TraceEncodeErrorV1::LengthOverflow)?;
        self.u16(len)?;
        self.bytes(bytes)
    }
}

struct Decoder<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> Decoder<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }

    fn remaining(&self) -> usize {
        self.bytes.len() - self.cursor
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8], TraceDecodeErrorV1> {
        let end = self
            .cursor
            .checked_add(len)
            .ok_or(TraceDecodeErrorV1::LengthOverflow)?;
        let value = self
            .bytes
            .get(self.cursor..end)
            .ok_or(TraceDecodeErrorV1::UnexpectedEof)?;
        self.cursor = end;
        Ok(value)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], TraceDecodeErrorV1> {
        self.take(N)?
            .try_into()
            .map_err(|_| TraceDecodeErrorV1::LengthOverflow)
    }

    fn u8(&mut self) -> Result<u8, TraceDecodeErrorV1> {
        Ok(self.array::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16, TraceDecodeErrorV1> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, TraceDecodeErrorV1> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    fn u64(&mut self) -> Result<u64, TraceDecodeErrorV1> {
        Ok(u64::from_le_bytes(self.array()?))
    }

    fn text(&mut self) -> Result<ProducerTextV1, TraceDecodeErrorV1> {
        let len = usize::from(self.u16()?);
        if len == 0 || len > MAX_PRODUCER_TEXT_BYTES_V1 {
            return Err(TraceDecodeErrorV1::Validation(
                TraceValidationErrorV1::InvalidProducerText { len },
            ));
        }
        let text =
            std::str::from_utf8(self.take(len)?).map_err(|_| TraceDecodeErrorV1::InvalidUtf8)?;
        ProducerTextV1::new(text).map_err(TraceDecodeErrorV1::Validation)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TraceEncodeErrorV1 {
    Validation(TraceValidationErrorV1),
    LengthOverflow,
    EncodedLengthExceedsLimit { attempted: u64, max: u64 },
    AllocationFailed { requested: usize },
    MaterializationInvariant,
}

impl fmt::Display for TraceEncodeErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "cannot encode semantic trace: {self:?}")
    }
}

impl Error for TraceEncodeErrorV1 {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TraceDecodeErrorV1 {
    InputTooLarge {
        actual: u64,
        max: u64,
    },
    UnexpectedEof,
    InvalidMagic,
    UnsupportedVersion(u16),
    UnsupportedKernelIrClaim {
        wire_version: u16,
        identity_policy: u16,
    },
    UnknownTag {
        context: &'static str,
        tag: u8,
    },
    InvalidUtf8,
    LengthOverflow,
    ImpossibleEventCount {
        declared: u64,
        remaining_bytes: u64,
        minimum_event_bytes: u64,
    },
    AllocationFailed {
        requested: usize,
    },
    DeclaredByteLimitExceeded {
        actual: u64,
        max: u64,
    },
    TrailingBytes(usize),
    NonCanonicalEncoding,
    Validation(TraceValidationErrorV1),
    Reencode(TraceEncodeErrorV1),
}

impl fmt::Display for TraceDecodeErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "cannot decode semantic trace: {self:?}")
    }
}

impl Error for TraceDecodeErrorV1 {}
