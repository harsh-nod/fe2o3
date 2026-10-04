use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use crate::graph::{
    COMPLETION_IDENTITY_BYTES_V1, CompletionGraphErrorV1, CompletionGraphV1, CompletionNodeIdV1,
    CompletionNodeKindV1, CompletionNodeV1, ContextIdentityV1, DeviceIdentityV1, EventIdentityV1,
    FutureIdentityV1, MAX_COMPLETION_GRAPH_NODES_V1, MAX_COMPLETION_GRAPH_STREAMS_V1,
    StreamIdentityV1,
};

/// Domain prefix of the canonical completion graph V1 wire format.
pub const COMPLETION_GRAPH_WIRE_DOMAIN_V1: &[u8] = b"FE2O3/COMPLETION-GRAPH/V1\0";
const COMPLETION_GRAPH_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/COMPLETION-GRAPH-IDENTITY/V1\0";
const GRAPH_HEADER_BYTES_V1: usize = COMPLETION_GRAPH_WIRE_DOMAIN_V1.len()
    + 4
    + 4
    + COMPLETION_IDENTITY_BYTES_V1
    + COMPLETION_IDENTITY_BYTES_V1
    + 4
    + 4;
const STREAM_WIRE_BYTES_V1: usize = COMPLETION_IDENTITY_BYTES_V1;
const NODE_WIRE_BYTES_V1: usize = 4 + 1 + 1 + 2 + 4 + 32 + 32 + 4;

/// Maximum canonical size of one completion graph V1.
pub const MAX_COMPLETION_GRAPH_BYTES_V1: usize = GRAPH_HEADER_BYTES_V1
    + MAX_COMPLETION_GRAPH_STREAMS_V1 * STREAM_WIRE_BYTES_V1
    + MAX_COMPLETION_GRAPH_NODES_V1 * NODE_WIRE_BYTES_V1;

const FUTURE_TAG_V1: u8 = 1;
const EVENT_RECORD_TAG_V1: u8 = 2;
const EVENT_WAIT_TAG_V1: u8 = 3;

/// Domain-separated SHA-256 and byte length of an exact canonical graph.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompletionGraphIdentityV1 {
    sha256: [u8; 32],
    byte_len: u64,
}

impl CompletionGraphIdentityV1 {
    pub const fn sha256(self) -> [u8; 32] {
        self.sha256
    }

    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }

    /// Checks an exact canonical byte sequence against this identity.
    ///
    /// This does not decode the bytes or authenticate their producer.
    pub fn matches_canonical_bytes(self, bytes: &[u8]) -> bool {
        self == calculate_identity(bytes)
    }
}

impl CompletionGraphV1 {
    /// Encodes this graph in its bounded canonical V1 representation.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        encode_graph(self)
    }

    /// Calculates the identity of the exact canonical graph representation.
    pub fn identity(&self) -> CompletionGraphIdentityV1 {
        calculate_identity(&self.canonical_bytes())
    }

    /// Strictly decodes one complete canonical graph representation.
    pub fn decode_canonical(bytes: &[u8]) -> Result<Self, CompletionGraphDecodeErrorV1> {
        decode_graph(bytes)
    }
}

/// Failure to strictly decode a canonical completion graph.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum CompletionGraphDecodeErrorV1 {
    TooLarge {
        actual: usize,
        maximum: usize,
    },
    Truncated,
    InvalidDomain,
    DeclaredLengthMismatch {
        declared: usize,
        actual: usize,
    },
    UnsupportedFlags(u32),
    CountBoundExceeded {
        field: &'static str,
        actual: usize,
        maximum: usize,
    },
    InvalidTag {
        field: &'static str,
        actual: u8,
    },
    NonzeroReserved {
        field: &'static str,
    },
    InvalidNodeId {
        field: &'static str,
    },
    InvalidGraph(Box<CompletionGraphErrorV1>),
    NonCanonical,
}

impl fmt::Display for CompletionGraphDecodeErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge { actual, maximum } => write!(
                formatter,
                "completion graph encoding has {actual} bytes, exceeding {maximum}"
            ),
            Self::Truncated => formatter.write_str("truncated completion graph encoding"),
            Self::InvalidDomain => formatter.write_str("invalid completion graph wire domain"),
            Self::DeclaredLengthMismatch { declared, actual } => write!(
                formatter,
                "completion graph declares {declared} bytes but contains {actual}"
            ),
            Self::UnsupportedFlags(flags) => {
                write!(formatter, "unsupported completion graph flags {flags:#x}")
            }
            Self::CountBoundExceeded {
                field,
                actual,
                maximum,
            } => write!(
                formatter,
                "completion graph {field} count {actual} exceeds {maximum}"
            ),
            Self::InvalidTag { field, actual } => {
                write!(formatter, "invalid completion graph {field} tag {actual}")
            }
            Self::NonzeroReserved { field } => {
                write!(formatter, "nonzero reserved completion graph {field}")
            }
            Self::InvalidNodeId { field } => {
                write!(formatter, "zero completion graph node id in {field}")
            }
            Self::InvalidGraph(error) => write!(formatter, "invalid completion graph: {error}"),
            Self::NonCanonical => formatter.write_str("noncanonical completion graph encoding"),
        }
    }
}

impl Error for CompletionGraphDecodeErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidGraph(error) => Some(error.as_ref()),
            _ => None,
        }
    }
}

fn encode_graph(graph: &CompletionGraphV1) -> Vec<u8> {
    let exact_size = GRAPH_HEADER_BYTES_V1
        + graph.streams.len() * STREAM_WIRE_BYTES_V1
        + graph.nodes.len() * NODE_WIRE_BYTES_V1;
    debug_assert!(exact_size <= MAX_COMPLETION_GRAPH_BYTES_V1);
    let mut encoded = Vec::with_capacity(exact_size);
    encoded.extend_from_slice(COMPLETION_GRAPH_WIRE_DOMAIN_V1);
    push_u32(&mut encoded, exact_size);
    push_u32(&mut encoded, 0);
    encoded.extend_from_slice(&graph.device().as_bytes());
    encoded.extend_from_slice(&graph.context.local_bytes());
    push_u32(&mut encoded, graph.streams.len());
    push_u32(&mut encoded, graph.nodes.len());
    for stream in &graph.streams {
        encoded.extend_from_slice(&stream.local_bytes());
    }
    for node in &graph.nodes {
        push_u32(&mut encoded, node.id().get() as usize);
        let tag = match node.kind() {
            CompletionNodeKindV1::Future(_) => FUTURE_TAG_V1,
            CompletionNodeKindV1::EventRecord { .. } => EVENT_RECORD_TAG_V1,
            CompletionNodeKindV1::EventWait { .. } => EVENT_WAIT_TAG_V1,
        };
        encoded.push(tag);
        encoded.push(u8::from(node.stream_predecessor().is_some()));
        encoded.extend_from_slice(&0_u16.to_le_bytes());
        push_u32(
            &mut encoded,
            node.stream_predecessor().map_or(0, |node| node.get()) as usize,
        );
        encoded.extend_from_slice(&node.stream().local_bytes());
        match node.kind() {
            CompletionNodeKindV1::Future(future) => {
                encoded.extend_from_slice(&future.local_bytes());
                push_u32(&mut encoded, 0);
            }
            CompletionNodeKindV1::EventRecord { event, .. } => {
                encoded.extend_from_slice(&event.local_bytes());
                push_u32(&mut encoded, 0);
            }
            CompletionNodeKindV1::EventWait {
                event, recorded_by, ..
            } => {
                encoded.extend_from_slice(&event.local_bytes());
                push_u32(&mut encoded, recorded_by.get() as usize);
            }
        }
    }
    debug_assert_eq!(encoded.len(), exact_size);
    encoded
}

fn decode_graph(bytes: &[u8]) -> Result<CompletionGraphV1, CompletionGraphDecodeErrorV1> {
    if bytes.len() > MAX_COMPLETION_GRAPH_BYTES_V1 {
        return Err(CompletionGraphDecodeErrorV1::TooLarge {
            actual: bytes.len(),
            maximum: MAX_COMPLETION_GRAPH_BYTES_V1,
        });
    }
    let mut reader = Reader::new(bytes);
    if reader.fixed::<{ COMPLETION_GRAPH_WIRE_DOMAIN_V1.len() }>()?
        != COMPLETION_GRAPH_WIRE_DOMAIN_V1
    {
        return Err(CompletionGraphDecodeErrorV1::InvalidDomain);
    }
    let declared = reader.u32()? as usize;
    if declared != bytes.len() {
        return Err(CompletionGraphDecodeErrorV1::DeclaredLengthMismatch {
            declared,
            actual: bytes.len(),
        });
    }
    let flags = reader.u32()?;
    if flags != 0 {
        return Err(CompletionGraphDecodeErrorV1::UnsupportedFlags(flags));
    }
    let device = DeviceIdentityV1::from_bytes(reader.fixed()?);
    let context = ContextIdentityV1::new(device, reader.fixed()?);
    let stream_count = reader.bounded_count("stream", MAX_COMPLETION_GRAPH_STREAMS_V1)?;
    let node_count = reader.bounded_count("node", MAX_COMPLETION_GRAPH_NODES_V1)?;
    let expected_size = GRAPH_HEADER_BYTES_V1
        .checked_add(stream_count.saturating_mul(STREAM_WIRE_BYTES_V1))
        .and_then(|size| size.checked_add(node_count.saturating_mul(NODE_WIRE_BYTES_V1)))
        .ok_or(CompletionGraphDecodeErrorV1::TooLarge {
            actual: bytes.len(),
            maximum: MAX_COMPLETION_GRAPH_BYTES_V1,
        })?;
    if expected_size != bytes.len() {
        return Err(CompletionGraphDecodeErrorV1::DeclaredLengthMismatch {
            declared: expected_size,
            actual: bytes.len(),
        });
    }

    let mut streams = Vec::with_capacity(stream_count);
    for _ in 0..stream_count {
        streams.push(StreamIdentityV1::new(context, reader.fixed()?));
    }
    let mut nodes = Vec::with_capacity(node_count);
    for _ in 0..node_count {
        let id = decode_node_id(reader.u32()?, "node identity")?;
        let kind = reader.u8()?;
        let predecessor_tag = reader.u8()?;
        if reader.u16()? != 0 {
            return Err(CompletionGraphDecodeErrorV1::NonzeroReserved {
                field: "node flags",
            });
        }
        let predecessor_raw = reader.u32()?;
        let predecessor = match predecessor_tag {
            0 if predecessor_raw == 0 => None,
            0 => {
                return Err(CompletionGraphDecodeErrorV1::NonzeroReserved {
                    field: "absent stream predecessor",
                });
            }
            1 => Some(decode_node_id(predecessor_raw, "stream predecessor")?),
            actual => {
                return Err(CompletionGraphDecodeErrorV1::InvalidTag {
                    field: "stream predecessor",
                    actual,
                });
            }
        };
        let stream = StreamIdentityV1::new(context, reader.fixed()?);
        let payload = reader.fixed()?;
        let record_raw = reader.u32()?;
        let node = match kind {
            FUTURE_TAG_V1 if record_raw == 0 => {
                CompletionNodeV1::future(id, FutureIdentityV1::new(stream, payload), predecessor)
            }
            EVENT_RECORD_TAG_V1 if record_raw == 0 => CompletionNodeV1::record_event(
                id,
                stream,
                EventIdentityV1::new(context, payload),
                predecessor,
            ),
            EVENT_WAIT_TAG_V1 => CompletionNodeV1::wait_event(
                id,
                stream,
                EventIdentityV1::new(context, payload),
                decode_node_id(record_raw, "event record")?,
                predecessor,
            ),
            FUTURE_TAG_V1 | EVENT_RECORD_TAG_V1 => {
                return Err(CompletionGraphDecodeErrorV1::NonzeroReserved {
                    field: "non-wait event record",
                });
            }
            actual => {
                return Err(CompletionGraphDecodeErrorV1::InvalidTag {
                    field: "node kind",
                    actual,
                });
            }
        };
        nodes.push(node);
    }
    if !reader.is_empty() {
        return Err(CompletionGraphDecodeErrorV1::DeclaredLengthMismatch {
            declared: bytes.len() - reader.remaining(),
            actual: bytes.len(),
        });
    }

    let graph = CompletionGraphV1::new(context, streams, nodes)
        .map_err(|error| CompletionGraphDecodeErrorV1::InvalidGraph(Box::new(error)))?;
    if graph.canonical_bytes() != bytes {
        return Err(CompletionGraphDecodeErrorV1::NonCanonical);
    }
    Ok(graph)
}

fn decode_node_id(
    raw: u32,
    field: &'static str,
) -> Result<CompletionNodeIdV1, CompletionGraphDecodeErrorV1> {
    CompletionNodeIdV1::new(raw).ok_or(CompletionGraphDecodeErrorV1::InvalidNodeId { field })
}

fn calculate_identity(bytes: &[u8]) -> CompletionGraphIdentityV1 {
    let mut digest = Sha256::new();
    digest.update(COMPLETION_GRAPH_IDENTITY_DOMAIN_V1);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    CompletionGraphIdentityV1 {
        sha256: digest.finalize().into(),
        byte_len: bytes.len() as u64,
    }
}

fn push_u32(encoded: &mut Vec<u8>, value: usize) {
    encoded.extend_from_slice(
        &u32::try_from(value)
            .expect("validated completion graph count fits u32")
            .to_le_bytes(),
    );
}

struct Reader<'a> {
    remaining: &'a [u8],
}

impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { remaining: bytes }
    }

    fn fixed<const N: usize>(&mut self) -> Result<[u8; N], CompletionGraphDecodeErrorV1> {
        let Some((value, remaining)) = self.remaining.split_at_checked(N) else {
            return Err(CompletionGraphDecodeErrorV1::Truncated);
        };
        self.remaining = remaining;
        Ok(value.try_into().expect("split length is exact"))
    }

    fn u8(&mut self) -> Result<u8, CompletionGraphDecodeErrorV1> {
        Ok(self.fixed::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16, CompletionGraphDecodeErrorV1> {
        Ok(u16::from_le_bytes(self.fixed()?))
    }

    fn u32(&mut self) -> Result<u32, CompletionGraphDecodeErrorV1> {
        Ok(u32::from_le_bytes(self.fixed()?))
    }

    fn bounded_count(
        &mut self,
        field: &'static str,
        maximum: usize,
    ) -> Result<usize, CompletionGraphDecodeErrorV1> {
        let actual = self.u32()? as usize;
        if actual > maximum {
            return Err(CompletionGraphDecodeErrorV1::CountBoundExceeded {
                field,
                actual,
                maximum,
            });
        }
        Ok(actual)
    }

    const fn is_empty(&self) -> bool {
        self.remaining.is_empty()
    }

    const fn remaining(&self) -> usize {
        self.remaining.len()
    }
}
