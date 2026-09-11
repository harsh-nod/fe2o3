//! Bounded IPC for isolated, explicitly unauthenticated gfx950 engineering workers.
//!
//! Each direction carries a four-byte little-endian JSON-header length, the
//! exact UTF-8 JSON header, then the exact binary payload length named by that
//! header. Headers never carry pointers. The parent must validate the ready
//! identity and each response before sending the next command to that worker.
//! Distinct workers may have commands outstanding concurrently.

use std::io::{self, Read, Write};

use serde::{Deserialize, Serialize, de::DeserializeOwned};

pub const PROTOCOL_VERSION_V1: u32 = 1;
pub const TARGET_V1: &str = "gfx950:xnack-";
pub const MAX_HEADER_BYTES_V1: usize = 65_536;
pub const MAX_TRANSFER_BYTES_V1: u32 = 4 * 1024 * 1024;
pub const MAX_OBJECT_BYTES_V1: u32 = 64 * 1024 * 1024;
pub const MAX_KERNARG_BYTES_V1: u32 = 65_536;
pub const MAX_POINTER_FIXUPS_V1: usize = 256;
pub const MAX_SEQUENCE_DISPATCHES_V1: usize = 16;
pub const MAX_ORDERED_BATCH_DISPATCHES_V1: usize = 16;
/// Conservative dispatch budget when hardware read-pointer reports never advance.
pub const MAX_UNRETIRED_RING_PACKETS_V1: u64 = 131_072;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BufferAccessV1 {
    Read,
    Write,
    ReadWrite,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PointerFixupV1 {
    pub kernarg_offset: u32,
    pub buffer: u64,
    pub buffer_offset: u64,
    pub extent_bytes: u64,
    pub access: BufferAccessV1,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SequenceDispatchV1 {
    pub kernel: u64,
    pub payload_bytes: u32,
    pub workgroup: [u16; 3],
    pub grid: [u32; 3],
    pub pointers: Vec<PointerFixupV1>,
    pub timeout_ms: u32,
}

/// One member of a dependent ordered batch. Its deadline belongs to the batch,
/// not to an individual kernel; no per-kernel timing is returned.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OrderedBatchDispatchV1 {
    pub kernel: u64,
    pub payload_bytes: u32,
    pub workgroup: [u16; 3],
    pub grid: [u32; 3],
    pub pointers: Vec<PointerFixupV1>,
}

fn ordered_batch_payload_bytes(
    dispatches: &[OrderedBatchDispatchV1],
    timeout_ms: u32,
) -> io::Result<usize> {
    if !(1..=MAX_ORDERED_BATCH_DISPATCHES_V1).contains(&dispatches.len())
        || !(1..=600_000).contains(&timeout_ms)
    {
        return Err(invalid("engineering ordered batch count or deadline"));
    }
    dispatches.iter().try_fold(0_usize, |total, dispatch| {
        if dispatch.payload_bytes > MAX_KERNARG_BYTES_V1
            || dispatch.pointers.len() > MAX_POINTER_FIXUPS_V1
        {
            return Err(invalid("engineering ordered batch dispatch limits"));
        }
        total
            .checked_add(dispatch.payload_bytes as usize)
            .filter(|bytes| *bytes <= MAX_TRANSFER_BYTES_V1 as usize)
            .ok_or_else(|| invalid("engineering ordered batch payload limit"))
    })
}

fn sequence_payload_bytes(dispatches: &[SequenceDispatchV1]) -> io::Result<usize> {
    if dispatches.is_empty() || dispatches.len() > MAX_SEQUENCE_DISPATCHES_V1 {
        return Err(invalid("engineering sequence length"));
    }
    let mut bytes = 0_u32;
    let mut timeout = 0_u32;
    for dispatch in dispatches {
        if dispatch.payload_bytes > MAX_KERNARG_BYTES_V1
            || dispatch.pointers.len() > MAX_POINTER_FIXUPS_V1
            || dispatch.timeout_ms == 0
        {
            return Err(invalid("engineering sequence dispatch limits"));
        }
        bytes = bytes
            .checked_add(dispatch.payload_bytes)
            .filter(|bytes| *bytes <= MAX_TRANSFER_BYTES_V1)
            .ok_or_else(|| invalid("engineering sequence payload limit"))?;
        timeout = timeout
            .checked_add(dispatch.timeout_ms)
            .filter(|timeout| *timeout <= 600_000)
            .ok_or_else(|| invalid("engineering sequence timeout limit"))?;
    }
    Ok(bytes as usize)
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum CommandV1 {
    /// Explicit engineering policy, accepted once before user resources exist.
    ConfigurePerformance {
        cache_kernel_admission: bool,
        operational_currentness: bool,
        profile: bool,
    },
    PerformanceSnapshot,
    /// Retires a completed queue and rebuilds its private resources.
    RolloverQueue {
        expected_epoch: u64,
        expected_completed_packets: u64,
    },
    /// Concatenated kernargs; no allocation or lifetime transition between items.
    DispatchSequence {
        dispatches: Vec<SequenceDispatchV1>,
    },
    /// Explicit single-queue, 1..16 dependent packets, with one aggregate
    /// publication-to-observed-completion deadline and no per-kernel timings.
    DispatchOrderedBatch {
        dispatches: Vec<OrderedBatchDispatchV1>,
        timeout_ms: u32,
    },
    Allocate {
        bytes: u64,
    },
    Free {
        buffer: u64,
    },
    Write {
        buffer: u64,
        offset: u64,
        payload_bytes: u32,
    },
    Read {
        buffer: u64,
        offset: u64,
        bytes: u32,
    },
    LoadKernel {
        payload_bytes: u32,
        object_sha256: [u8; 32],
        symbol: String,
    },
    Dispatch {
        kernel: u64,
        payload_bytes: u32,
        workgroup: [u16; 3],
        grid: [u32; 3],
        pointers: Vec<PointerFixupV1>,
        timeout_ms: u32,
    },
    Close,
}

impl CommandV1 {
    /// Checks framing limits before allocating or reading any binary payload.
    pub fn payload_bytes(&self) -> io::Result<usize> {
        let length = match self {
            Self::DispatchSequence { dispatches } => return sequence_payload_bytes(dispatches),
            Self::DispatchOrderedBatch {
                dispatches,
                timeout_ms,
            } => {
                return ordered_batch_payload_bytes(dispatches, *timeout_ms);
            }
            Self::Write { payload_bytes, .. } if *payload_bytes <= MAX_TRANSFER_BYTES_V1 => {
                *payload_bytes
            }
            Self::LoadKernel {
                payload_bytes,
                symbol,
                ..
            } if *payload_bytes > 0
                && *payload_bytes <= MAX_OBJECT_BYTES_V1
                && !symbol.is_empty()
                && symbol.len() <= 256
                && symbol
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_') =>
            {
                *payload_bytes
            }
            Self::Dispatch {
                payload_bytes,
                pointers,
                timeout_ms,
                ..
            } if *payload_bytes <= MAX_KERNARG_BYTES_V1
                && pointers.len() <= MAX_POINTER_FIXUPS_V1
                && (1..=600_000).contains(timeout_ms) =>
            {
                *payload_bytes
            }
            Self::Read { bytes, .. } if *bytes <= MAX_TRANSFER_BYTES_V1 => 0,
            Self::Allocate { .. }
            | Self::Free { .. }
            | Self::ConfigurePerformance { .. }
            | Self::PerformanceSnapshot
            | Self::RolloverQueue { .. }
            | Self::Close => 0,
            _ => return Err(invalid("engineering frame limits")),
        };
        Ok(length as usize)
    }
}

/// Cumulative worker-side wall-clock observations, not GPU timestamp queries.
/// Counters start at explicit configuration; snapshot excludes its own command.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PerformanceCountersV1 {
    pub commands: u64,
    pub command_ns: u64,
    pub full_currentness_checks: u64,
    pub full_currentness_ns: u64,
    pub operational_currentness_checks: u64,
    pub operational_currentness_ns: u64,
    pub kernel_admissions: u64,
    pub kernel_admission_ns: u64,
    pub dispatches: u64,
    pub dispatch_prepare_ns: u64,
    pub dispatch_publish_ns: u64,
    pub dispatch_wait_ns: u64,
    pub completion_polls: u64,
    pub reads: u64,
    pub read_bytes: u64,
    pub read_ns: u64,
    pub writes: u64,
    pub write_bytes: u64,
    pub write_ns: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KernelMetadataV1 {
    pub symbol: String,
    pub object_sha256: [u8; 32],
    pub kernarg_bytes: u32,
    pub kernarg_alignment: u32,
    pub group_segment_bytes: u32,
    pub private_segment_bytes: u32,
    pub wavefront_size: u32,
    pub implicit_argument_offset: Option<u32>,
    pub implicit_argument_bytes: u32,
    pub explicit_arguments: Vec<ExplicitArgumentV1>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExplicitArgumentV1 {
    pub offset: u32,
    pub bytes: u32,
    pub global_buffer: bool,
    pub pointee_alignment: Option<u32>,
    pub access: Option<BufferAccessV1>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum ResponseV1 {
    /// Every retained signal completed and the selected exit currentness/idle
    /// fence passed. Time is aggregate host wall time, not GPU/kernel time.
    DispatchOrderedBatchCompleted {
        completed_dispatches: u32,
        elapsed_ns: u64,
    },
    DispatchSequenceCompleted {
        elapsed_ns: Vec<u64>,
    },
    DispatchSequenceFailed {
        completed_dispatches: u32,
        attempted_dispatches: u32,
        elapsed_ns: Vec<u64>,
        message: String,
        fatal: bool,
    },
    QueueRolledOver {
        retired_packets: u64,
        queue_epoch: u64,
    },
    PerformanceConfigured,
    PerformanceSnapshot {
        counters: PerformanceCountersV1,
    },
    Ready {
        protocol: u32,
        target: String,
        device_unique_id: u64,
        authority: String,
    },
    Allocated {
        buffer: u64,
        bytes: u64,
    },
    Freed,
    Written,
    Read {
        payload_bytes: u32,
    },
    LoadedKernel {
        kernel: u64,
        metadata: KernelMetadataV1,
    },
    Dispatched {
        elapsed_ns: u64,
    },
    Closed,
    Error {
        message: String,
        fatal: bool,
    },
}

/// Reads one bounded JSON header, distinguishing clean stream EOF from truncation.
pub fn read_header_v1<T: DeserializeOwned + Serialize>(
    input: &mut impl Read,
) -> io::Result<Option<T>> {
    let mut prefix = [0_u8; 4];
    loop {
        match input.read(&mut prefix[..1]) {
            Ok(0) => return Ok(None),
            Ok(_) => break,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }
    input.read_exact(&mut prefix[1..])?;
    let length = u32::from_le_bytes(prefix) as usize;
    if length == 0 || length > MAX_HEADER_BYTES_V1 {
        return Err(invalid("engineering header length"));
    }
    let mut bytes = vec![0; length];
    input.read_exact(&mut bytes)?;
    let header: T =
        serde_json::from_slice(&bytes).map_err(|_| invalid("engineering header JSON"))?;
    let incoming: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|_| invalid("engineering header JSON"))?;
    let recognized =
        serde_json::to_value(&header).map_err(|_| invalid("engineering header encoding"))?;
    if incoming != recognized {
        return Err(invalid("engineering header contains unrecognized fields"));
    }
    Ok(Some(header))
}

/// Writes a bounded header only. The caller then writes its exact binary payload
/// and flushes once; this separation allows all ranks to be submitted first.
pub fn write_header_v1(output: &mut impl Write, header: &impl Serialize) -> io::Result<()> {
    let bytes = serde_json::to_vec(header).map_err(|_| invalid("engineering header encoding"))?;
    if bytes.is_empty() || bytes.len() > MAX_HEADER_BYTES_V1 {
        return Err(invalid("engineering header length"));
    }
    output.write_all(&(bytes.len() as u32).to_le_bytes())?;
    output.write_all(&bytes)
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn sequences_have_exact_cumulative_payload_and_time_bounds() {
        let dispatch = SequenceDispatchV1 {
            kernel: 1,
            payload_bytes: MAX_KERNARG_BYTES_V1,
            workgroup: [64, 1, 1],
            grid: [64, 1, 1],
            pointers: Vec::new(),
            timeout_ms: 1,
        };
        assert!(sequence_payload_bytes(&[]).is_err());
        assert_eq!(
            sequence_payload_bytes(&vec![dispatch.clone(); 16]).unwrap(),
            16 * MAX_KERNARG_BYTES_V1 as usize
        );
        assert!(sequence_payload_bytes(&vec![dispatch.clone(); 17]).is_err());
        let mut excessive = dispatch.clone();
        excessive.payload_bytes += 1;
        assert!(sequence_payload_bytes(&[excessive]).is_err());
        excessive = dispatch.clone();
        excessive.timeout_ms = 0;
        assert!(sequence_payload_bytes(&[excessive]).is_err());
        excessive = dispatch.clone();
        excessive.timeout_ms = 300_001;
        assert!(sequence_payload_bytes(&[excessive.clone(), excessive]).is_err());
        excessive = dispatch.clone();
        excessive.timeout_ms = u32::MAX;
        assert!(sequence_payload_bytes(&[excessive]).is_err());
        let command = CommandV1::DispatchSequence {
            dispatches: vec![dispatch],
        };
        let mut bytes = Vec::new();
        write_header_v1(&mut bytes, &command).unwrap();
        assert_eq!(
            read_header_v1::<CommandV1>(&mut Cursor::new(bytes)).unwrap(),
            Some(command)
        );
    }

    #[test]
    fn failed_sequence_reports_completed_and_attempted_counts_separately() {
        let response = ResponseV1::DispatchSequenceFailed {
            completed_dispatches: 1,
            attempted_dispatches: 2,
            elapsed_ns: vec![7],
            message: "uncertain second publication".into(),
            fatal: true,
        };
        let mut bytes = Vec::new();
        write_header_v1(&mut bytes, &response).unwrap();
        assert_eq!(
            read_header_v1::<ResponseV1>(&mut Cursor::new(bytes)).unwrap(),
            Some(response)
        );
    }

    #[test]
    fn performance_commands_are_explicit_bounded_and_exact() {
        for command in [
            CommandV1::ConfigurePerformance {
                cache_kernel_admission: true,
                operational_currentness: false,
                profile: true,
            },
            CommandV1::PerformanceSnapshot,
            CommandV1::RolloverQueue {
                expected_epoch: 4,
                expected_completed_packets: MAX_UNRETIRED_RING_PACKETS_V1,
            },
        ] {
            assert_eq!(command.payload_bytes().unwrap(), 0);
            let mut bytes = Vec::new();
            write_header_v1(&mut bytes, &command).unwrap();
            assert_eq!(
                read_header_v1::<CommandV1>(&mut Cursor::new(bytes)).unwrap(),
                Some(command)
            );
        }
        let response = ResponseV1::PerformanceSnapshot {
            counters: PerformanceCountersV1::default(),
        };
        let mut bytes = Vec::new();
        write_header_v1(&mut bytes, &response).unwrap();
        assert_eq!(
            read_header_v1::<ResponseV1>(&mut Cursor::new(bytes)).unwrap(),
            Some(response)
        );
    }

    #[test]
    fn exact_header_round_trip_does_not_consume_binary_payload() {
        let command = CommandV1::Write {
            buffer: 7,
            offset: 11,
            payload_bytes: 3,
        };
        let mut bytes = Vec::new();
        write_header_v1(&mut bytes, &command).unwrap();
        bytes.extend_from_slice(&[5, 6, 7]);
        let mut stream = Cursor::new(bytes);
        assert_eq!(
            read_header_v1::<CommandV1>(&mut stream).unwrap(),
            Some(command)
        );
        let mut payload = [0; 3];
        stream.read_exact(&mut payload).unwrap();
        assert_eq!(payload, [5, 6, 7]);
        assert_eq!(read_header_v1::<CommandV1>(&mut stream).unwrap(), None);
    }

    #[test]
    fn malformed_and_oversized_headers_fail_before_payload() {
        for bytes in [
            vec![1],
            vec![0, 0, 0, 0],
            (MAX_HEADER_BYTES_V1 as u32 + 1).to_le_bytes().to_vec(),
            vec![2, 0, 0, 0, b'{'],
        ] {
            assert!(read_header_v1::<CommandV1>(&mut Cursor::new(bytes)).is_err());
        }
        let mut bytes = Vec::new();
        write_header_v1(
            &mut bytes,
            &serde_json::json!({ "op": "close", "extra": 1 }),
        )
        .unwrap();
        assert!(read_header_v1::<CommandV1>(&mut Cursor::new(bytes)).is_err());
        for header in [
            r#"{"op":"close","op":"close"}"#,
            r#"{"op":"allocate","bytes":1,"bytes":2}"#,
            r#"{"op":"allocate","bytes":-1}"#,
            r#"{"op":"unknown"}"#,
        ] {
            let mut bytes = (header.len() as u32).to_le_bytes().to_vec();
            bytes.extend_from_slice(header.as_bytes());
            assert!(
                read_header_v1::<CommandV1>(&mut Cursor::new(bytes)).is_err(),
                "{header}"
            );
        }
    }

    #[test]
    fn payload_and_symbol_limits_are_closed() {
        assert!(
            CommandV1::Write {
                buffer: 1,
                offset: 0,
                payload_bytes: MAX_TRANSFER_BYTES_V1 + 1
            }
            .payload_bytes()
            .is_err()
        );
        assert!(
            CommandV1::Read {
                buffer: 1,
                offset: 0,
                bytes: MAX_TRANSFER_BYTES_V1 + 1
            }
            .payload_bytes()
            .is_err()
        );
        for symbol in ["", "../kernel", "kernel\0"] {
            assert!(
                CommandV1::LoadKernel {
                    payload_bytes: 1,
                    object_sha256: [0; 32],
                    symbol: symbol.into()
                }
                .payload_bytes()
                .is_err()
            );
        }
        assert!(
            CommandV1::Dispatch {
                kernel: 1,
                payload_bytes: 0,
                workgroup: [1; 3],
                grid: [1; 3],
                pointers: vec![],
                timeout_ms: 0
            }
            .payload_bytes()
            .is_err()
        );
    }
}
