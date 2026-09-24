//! JSONL V1 only; prepare and encode responses before committing cursor/revision.
use super::*;
#[derive(Clone, Copy)]
enum Update {
    Cursor { sequence: u64, revision: u64 },
    Terminate { revision: u64 },
}
struct Prepared {
    response: DebugResponseV1,
    update: Option<Update>,
}
impl Prepared {
    fn plain(response: DebugResponseV1) -> Self {
        Self {
            response,
            update: None,
        }
    }
}
fn control<S: SessionView>(
    backend: &Backend<S>,
    id: u64,
    op: DebugOperationNameV1,
    sequence: u64,
) -> Prepared {
    let total = backend.session.records_len() as u64;
    if sequence > total + 1 {
        return Prepared::plain(backend.error(
            Some(id),
            Some(op),
            DebugErrorCodeV1::InvalidCursor,
            S::PROFILE.code(Code::CursorOutOfRange),
        ));
    }
    if sequence == total + 1
        && (backend.session.capture_stop().is_some()
            || backend.session.outcome() != PhysicalEntryDebugOutcomeV20::Completed)
    {
        return Prepared::plain(backend.unavailable(
            id,
            op,
            DebugCapabilityNameV1::ForwardStep,
            CapabilityUnavailableReasonV1::Truncated,
        ));
    }
    let Some(revision) = backend.revision.checked_add(1) else {
        return Prepared::plain(backend.error(
            Some(id),
            Some(op),
            DebugErrorCodeV1::ResourceLimit,
            S::PROFILE.code(Code::RevisionLimit),
        ));
    };
    let session = backend.view_at(sequence, revision, false);
    let result = DebugResultV1::Control {
        stop: Some(views::stop(sequence, backend.session.records_len())),
        snapshot: views::snapshot(backend, sequence, session),
        events_advanced: sequence.min(total).abs_diff(backend.sequence().min(total)),
    };
    Prepared {
        response: backend.ok_at(id, op, result, session),
        update: Some(Update::Cursor { sequence, revision }),
    }
}
fn prepare<S: SessionView>(backend: &Backend<S>, request: DebugRequestV1) -> Prepared {
    let id = request.request_id();
    let op = request.operation();
    if request.expected_revision() != backend.revision {
        return Prepared::plain(backend.error(
            Some(id),
            Some(op),
            DebugErrorCodeV1::StaleRevision,
            S::PROFILE.code(Code::StaleRevision),
        ));
    }
    if backend.terminated {
        return Prepared::plain(backend.error(
            Some(id),
            Some(op),
            DebugErrorCodeV1::InvalidState,
            S::PROFILE.code(Code::Terminated),
        ));
    }
    let result = match request {
        DebugRequestV1::DiscoverCapabilities { .. } => backend.ok_at(
            id,
            op,
            DebugResultV1::Capabilities {
                capabilities: views::capabilities(),
            },
            backend.view(),
        ),
        DebugRequestV1::GetState { .. } => backend.ok_at(
            id,
            op,
            DebugResultV1::State {
                snapshot: views::snapshot(backend, backend.sequence(), backend.view()),
            },
            backend.view(),
        ),
        DebugRequestV1::Step {
            direction,
            granularity,
            count,
            focus,
            ..
        } => {
            if granularity != StepGranularityV1::Event
                || focus.is_some()
                || count as usize > RECORDS
            {
                backend.unavailable(
                    id,
                    op,
                    DebugCapabilityNameV1::ForwardStep,
                    CapabilityUnavailableReasonV1::NotExposedByBackend,
                )
            } else {
                let current = backend.sequence();
                let end = backend.session.records_len() as u64 + 1;
                let sequence = match direction {
                    StepDirectionV1::Forward => current.saturating_add(u64::from(count)).min(end),
                    StepDirectionV1::Reverse => current.saturating_sub(u64::from(count)),
                };
                return control(backend, id, op, sequence);
            }
        }
        DebugRequestV1::Seek { cursor, .. } => {
            if cursor.configuration_identity != backend.configuration
                || cursor.state_revision != backend.revision
            {
                backend.error(
                    Some(id),
                    Some(op),
                    DebugErrorCodeV1::InvalidCursor,
                    S::PROFILE.code(Code::ForeignOrStaleCursor),
                )
            } else {
                return control(backend, id, op, cursor.event_sequence);
            }
        }
        DebugRequestV1::InspectValues {
            scope,
            frame,
            selector,
            page,
            ..
        } => views::values(backend, id, scope, frame, selector, page),
        DebugRequestV1::ReadMemory {
            allocation,
            byte_offset,
            byte_len,
            ..
        } => views::memory(backend, id, allocation, byte_offset, byte_len),
        DebugRequestV1::Terminate { .. } => {
            let Some(revision) = backend.revision.checked_add(1) else {
                return Prepared::plain(backend.error(
                    Some(id),
                    Some(op),
                    DebugErrorCodeV1::ResourceLimit,
                    S::PROFILE.code(Code::RevisionLimit),
                ));
            };
            return Prepared {
                response: backend.ok_at(
                    id,
                    op,
                    DebugResultV1::Terminated,
                    backend.view_at(backend.sequence(), revision, true),
                ),
                update: Some(Update::Terminate { revision }),
            };
        }
        _ => {
            let (cap, reason) = match op {
                DebugOperationNameV1::SetBreakpoints
                | DebugOperationNameV1::RemoveBreakpoints
                | DebugOperationNameV1::ListBreakpoints => (
                    DebugCapabilityNameV1::Breakpoints,
                    CapabilityUnavailableReasonV1::NotExposedByBackend,
                ),
                DebugOperationNameV1::SetWatchpoints
                | DebugOperationNameV1::RemoveWatchpoints
                | DebugOperationNameV1::ListWatchpoints => (
                    DebugCapabilityNameV1::Watchpoints,
                    CapabilityUnavailableReasonV1::NotExposedByBackend,
                ),
                DebugOperationNameV1::ResolveSource => (
                    DebugCapabilityNameV1::SourceSites,
                    CapabilityUnavailableReasonV1::RequiresAuthenticatedMap,
                ),
                DebugOperationNameV1::InspectStack => (
                    DebugCapabilityNameV1::CallStack,
                    CapabilityUnavailableReasonV1::NotExposedByBackend,
                ),
                DebugOperationNameV1::InspectScope => (
                    DebugCapabilityNameV1::HierarchyInspection,
                    CapabilityUnavailableReasonV1::NotExposedByBackend,
                ),
                DebugOperationNameV1::QueryEvents | DebugOperationNameV1::ExportTrace => (
                    DebugCapabilityNameV1::SemanticTrace,
                    CapabilityUnavailableReasonV1::NotExposedByBackend,
                ),
                _ => (
                    DebugCapabilityNameV1::Pause,
                    CapabilityUnavailableReasonV1::ReadOnlyBackend,
                ),
            };
            backend.unavailable(id, op, cap, reason)
        }
    };
    Prepared::plain(result)
}
fn encode<S: SessionView>(
    response: &DebugResponseV1,
    limits: ProtocolLimitsV1,
) -> Result<Vec<u8>, &'static str> {
    encode_response_line_v1(response, limits).map_err(|_| S::PROFILE.code(Code::ResponseEncoding))
}
pub(super) fn respond<S: SessionView, W: Write>(
    backend: &mut Backend<S>,
    request: DebugRequestV1,
    writer: &mut W,
    limits: ProtocolLimitsV1,
) -> Result<(), &'static str> {
    let id = request.request_id();
    let op = request.operation();
    let prepared = prepare(backend, request);
    let mut bytes = match encode::<S>(&prepared.response, limits) {
        Ok(bytes) => bytes,
        Err(_) => {
            // No cursor/revision changed; refuse before applying any prepared update.
            let error = backend.error(
                Some(id),
                Some(op),
                DebugErrorCodeV1::ResponseTooLarge,
                S::PROFILE.code(Code::ResponseTooLarge),
            );
            let fallback = encode::<S>(&error, limits)?;
            return writer
                .write_all(&fallback)
                .map_err(|_| S::PROFILE.code(Code::OutputFailed));
        }
    };
    if let Some(update) = prepared.update {
        match update {
            Update::Cursor { sequence, revision } => {
                let navigation = if sequence == 0 {
                    backend.session.rewind()
                } else {
                    backend.session.seek((sequence - 1) as usize)
                };
                if matches!(navigation, Navigation::Unavailable | Navigation::Incomplete) {
                    let error = backend.error(
                        Some(id),
                        Some(op),
                        DebugErrorCodeV1::ResourceLimit,
                        S::PROFILE.code(Code::NavigationUnavailable),
                    );
                    // Drop the rejected positive response before allocating its error replacement.
                    drop(bytes);
                    bytes = encode::<S>(&error, limits)?;
                } else {
                    backend.revision = revision;
                }
            }
            Update::Terminate { revision } => {
                backend.revision = revision;
                backend.terminated = true;
            }
        }
    }
    // External output failure terminates the process/session; it does not grant
    // target-side effects. No claim of reversible external stream publication.
    writer
        .write_all(&bytes)
        .map_err(|_| S::PROFILE.code(Code::OutputFailed))
}
pub(super) fn run<S: SessionView, R: BufRead, W: Write>(
    backend: &mut Backend<S>,
    reader: &mut R,
    writer: &mut W,
    limits: ProtocolLimitsV1,
) -> Result<(), &'static str> {
    for _ in 0..COMMANDS {
        // Charge a bounded complete command allowance before its frame/parser/reply
        // allocations. Includes EOF and terminal no-op requests; no budget reset.
        if backend.session.charge_query_work(QUERY_WORK).is_err() {
            let response = backend.error(
                None,
                None,
                DebugErrorCodeV1::ResourceLimit,
                S::PROFILE.code(Code::QueryWorkLimit),
            );
            writer
                .write_all(&encode::<S>(&response, limits)?)
                .map_err(|_| S::PROFILE.code(Code::OutputFailed))?;
            return Err(S::PROFILE.code(Code::QueryWorkLimit));
        }
        let request = match read_request_line_v1(reader, limits) {
            Ok(Some(request)) => request,
            Ok(None) => {
                writer
                    .flush()
                    .map_err(|_| S::PROFILE.code(Code::OutputFailed))?;
                return Ok(());
            }
            Err(_) => {
                let response = backend.error(
                    None,
                    None,
                    DebugErrorCodeV1::InvalidRequest,
                    S::PROFILE.code(Code::UnsupportedOrInvalidRequest),
                );
                writer
                    .write_all(&encode::<S>(&response, limits)?)
                    .map_err(|_| S::PROFILE.code(Code::OutputFailed))?;
                return Err(S::PROFILE.code(Code::ProtocolRefused));
            }
        };
        respond(backend, request, writer, limits)?;
        writer
            .flush()
            .map_err(|_| S::PROFILE.code(Code::OutputFailed))?;
        if backend.terminated {
            return Ok(());
        }
    }
    let response = backend.error(
        None,
        None,
        DebugErrorCodeV1::ResourceLimit,
        S::PROFILE.code(Code::CommandLimit),
    );
    writer
        .write_all(&encode::<S>(&response, limits)?)
        .map_err(|_| S::PROFILE.code(Code::OutputFailed))?;
    Err(S::PROFILE.code(Code::CommandLimit))
}
