//! Version-specific admission with one borrowed event/header query engine.

use fe2o3_semantic_trace::*;

use crate::{
    KernelIrClaimViewV1, QueryErrorV1, QueryLimitsV1, QueryRequestV1, QueryResponseV1,
    TraceQueryCore, identity_view, kernel_ir_view, trace_binding,
};

pub const QUERY_SCHEMA_V2: &str = "fe2o3-semantic-query-v2";

#[derive(Debug)]
pub struct TraceQuerySessionV1 {
    inner: TraceQueryCore,
}

impl TraceQuerySessionV1 {
    /// Opens and fully validates one canonical Trace V1 byte stream.
    pub fn open(bytes: &[u8], limits: QueryLimitsV1) -> Result<Self, QueryErrorV1> {
        check_input(bytes, limits)?;
        let trace = decode_trace_v1(bytes).map_err(QueryErrorV1::TraceDecode)?;
        Ok(Self {
            inner: TraceQueryCore::new(TraceQueryInput::V1(trace), bytes, limits)?,
        })
    }

    /// Adopts an already constructed and validated Trace V1 value.
    pub fn from_trace(trace: TraceV1, limits: QueryLimitsV1) -> Result<Self, QueryErrorV1> {
        let encoded = encode_trace_v1(&trace).map_err(QueryErrorV1::TraceEncode)?;
        Ok(Self {
            inner: TraceQueryCore::new(TraceQueryInput::V1(trace), &encoded, limits)?,
        })
    }

    pub const fn limits(&self) -> QueryLimitsV1 {
        self.inner.limits
    }

    pub fn query(&self, request: QueryRequestV1) -> Result<QueryResponseV1, QueryErrorV1> {
        self.inner.query(request)
    }

    pub fn encode_json(&self, response: &QueryResponseV1) -> Result<Vec<u8>, QueryErrorV1> {
        self.inner.encode_json(response)
    }

    pub fn query_json(&self, request: QueryRequestV1) -> Result<Vec<u8>, QueryErrorV1> {
        self.inner.query_json(request)
    }
}

/// Read-only queries over exact V9/V10 claims in a semantic Trace V2 envelope.
///
/// Requests, events, plans and response fields reuse the V1 query grammar.
/// Every response context uses `QUERY_SCHEMA_V2` and preserves the exact KIR
/// claim. This session does not admit V1 traces or grant execution authority.
#[derive(Debug)]
pub struct TraceQuerySessionV2 {
    inner: TraceQueryCore,
}

impl TraceQuerySessionV2 {
    pub fn open(bytes: &[u8], limits: QueryLimitsV1) -> Result<Self, QueryErrorV1> {
        check_input(bytes, limits)?;
        let trace = decode_trace_v2(bytes).map_err(QueryErrorV1::TraceDecode)?;
        Ok(Self {
            inner: TraceQueryCore::new(TraceQueryInput::V2(trace), bytes, limits)?,
        })
    }

    pub fn from_trace(trace: TraceEnvelopeV2, limits: QueryLimitsV1) -> Result<Self, QueryErrorV1> {
        let encoded = encode_trace_v2(&trace).map_err(QueryErrorV1::TraceEncode)?;
        Ok(Self {
            inner: TraceQueryCore::new(TraceQueryInput::V2(trace), &encoded, limits)?,
        })
    }

    pub const fn limits(&self) -> QueryLimitsV1 {
        self.inner.limits
    }

    pub fn query(&self, request: QueryRequestV1) -> Result<QueryResponseV1, QueryErrorV1> {
        self.inner.query(request)
    }

    pub fn encode_json(&self, response: &QueryResponseV1) -> Result<Vec<u8>, QueryErrorV1> {
        self.inner.encode_json(response)
    }

    pub fn query_json(&self, request: QueryRequestV1) -> Result<Vec<u8>, QueryErrorV1> {
        self.inner.query_json(request)
    }
}

fn check_input(bytes: &[u8], limits: QueryLimitsV1) -> Result<u64, QueryErrorV1> {
    let input_bytes = u64::try_from(bytes.len()).map_err(|_| QueryErrorV1::SizeOverflow)?;
    if input_bytes > limits.max_input_bytes() {
        return Err(QueryErrorV1::InputTooLarge {
            actual: input_bytes,
            max: limits.max_input_bytes(),
        });
    }
    Ok(input_bytes)
}

impl TraceQueryCore {
    fn new(
        trace: TraceQueryInput,
        bytes: &[u8],
        limits: QueryLimitsV1,
    ) -> Result<Self, QueryErrorV1> {
        Ok(Self {
            trace,
            input_bytes: check_input(bytes, limits)?,
            trace_binding: trace_binding(bytes),
            limits,
        })
    }
}

#[derive(Debug)]
pub(crate) enum TraceQueryInput {
    V1(TraceV1),
    V2(TraceEnvelopeV2),
}

impl TraceQueryInput {
    pub(crate) fn header(&self) -> TraceQueryHeader<'_> {
        match self {
            Self::V1(trace) => TraceQueryHeader::V1(trace.header()),
            Self::V2(trace) => TraceQueryHeader::V2(trace.header()),
        }
    }

    pub(crate) fn events(&self) -> &[TraceEventV1] {
        match self {
            Self::V1(trace) => trace.events(),
            Self::V2(trace) => trace.events(),
        }
    }

    pub(crate) fn schema(&self) -> &'static str {
        match self {
            Self::V1(_) => crate::QUERY_SCHEMA_V1,
            Self::V2(_) => QUERY_SCHEMA_V2,
        }
    }
}

pub(crate) enum TraceQueryHeader<'a> {
    V1(&'a TraceHeaderV1),
    V2(TraceHeaderViewV2<'a>),
}

impl TraceQueryHeader<'_> {
    pub(crate) fn producer(&self) -> &ProducerIdentityV1 {
        match self {
            Self::V1(header) => header.producer(),
            Self::V2(header) => header.producer(),
        }
    }

    pub(crate) fn execution_kind(&self) -> ExecutionKindV1 {
        match self {
            Self::V1(header) => header.execution_kind(),
            Self::V2(header) => header.execution_kind(),
        }
    }

    pub(crate) fn kernel_ir_claim(&self) -> KernelIrClaimViewV1 {
        match self {
            Self::V1(header) => kernel_ir_view(header.kernel_ir_claim()),
            Self::V2(header) => {
                let claim = header.kernel_ir_claim();
                KernelIrClaimViewV1 {
                    wire_version: claim.wire_version().as_u16(),
                    identity_policy: claim.identity_policy(),
                    digest: identity_view(claim.digest()),
                    canonical_len: claim.canonical_len(),
                    authenticated: false,
                }
            }
        }
    }

    pub(crate) fn semantic_mir(&self) -> Option<ContentIdentityV1> {
        match self {
            Self::V1(header) => header.semantic_mir(),
            Self::V2(header) => header.semantic_mir(),
        }
    }

    pub(crate) fn lineage(&self) -> Option<ContentIdentityV1> {
        match self {
            Self::V1(header) => header.lineage(),
            Self::V2(header) => header.lineage(),
        }
    }

    pub(crate) fn artifact(&self) -> Option<ContentIdentityV1> {
        match self {
            Self::V1(header) => header.artifact(),
            Self::V2(header) => header.artifact(),
        }
    }

    pub(crate) fn dispatch(&self) -> DispatchIdentityV1 {
        match self {
            Self::V1(header) => header.dispatch(),
            Self::V2(header) => header.dispatch(),
        }
    }

    pub(crate) fn launch(&self) -> LaunchGeometryV1 {
        match self {
            Self::V1(header) => header.launch(),
            Self::V2(header) => header.launch(),
        }
    }

    pub(crate) fn completeness(&self) -> TraceCompletenessV1 {
        match self {
            Self::V1(header) => header.completeness(),
            Self::V2(header) => header.completeness(),
        }
    }

    pub(crate) fn boundaries(&self) -> CaptureBoundariesV1 {
        match self {
            Self::V1(header) => header.boundaries(),
            Self::V2(header) => header.boundaries(),
        }
    }
}
