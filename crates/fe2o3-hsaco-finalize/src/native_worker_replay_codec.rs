//! Shared inert framing. Typed entrypoints select the schema, never wire input.
use super::*;

#[derive(Clone, Copy)]
pub(super) enum Schema {
    Native,
    Conditional,
}
impl Schema {
    pub(super) const fn magic(self) -> &'static [u8; 8] {
        match self {
            Self::Native => MAGIC,
            Self::Conditional => b"F2CCFR05",
        }
    }
    pub(super) const fn version(self) -> u16 {
        match self {
            Self::Native => VERSION,
            Self::Conditional => 5,
        }
    }
    pub(super) const fn checksum_domain(self) -> &'static [u8] {
        match self {
            Self::Native => CHECKSUM_DOMAIN,
            Self::Conditional => b"FE2O3/CONDITIONAL-WORKER-COMPACT-REPLAY-CHECKSUM/V5\0",
        }
    }
    const fn identity_domain(self) -> &'static [u8] {
        match self {
            Self::Native => IDENTITY_DOMAIN,
            Self::Conditional => b"FE2O3/CONDITIONAL-WORKER-COMPACT-REPLAY-IDENTITY/V5\0",
        }
    }
    const fn max_outer(self) -> usize {
        match self {
            Self::Native => MAX_COMPILER_MODULE_HANDOFF_BYTES_V4,
            Self::Conditional => fe2o3_artifact_transaction::MAX_COMPILER_MODULE_HANDOFF_BYTES_V5,
        }
    }
}

pub(super) struct Header {
    pub(super) finalization: [u8; 32],
    pub(super) source: [u8; 32],
    pub(super) binding: [u8; 32],
    pub(super) outer: ContentIdentityV1,
    pub(super) attempt: BuildAttempt,
    pub(super) slot: u8,
    pub(super) transaction: [u8; 32],
}
impl Header {
    pub(super) fn verify_outer(&self, digest: &[u8; 32], length: u64) -> Result<()> {
        if self.outer.sha256() != digest || self.outer.byte_len() != length {
            return Err(NativeWorkerCompactReplayErrorV1::Coordinates);
        }
        Ok(())
    }

    pub(super) fn encode(&self, schema: Schema, bytes: &mut Vec<u8>) {
        bytes.extend_from_slice(schema.magic());
        bytes.extend_from_slice(&schema.version().to_le_bytes());
        bytes.extend_from_slice(&self.finalization);
        bytes.extend_from_slice(&self.source);
        bytes.extend_from_slice(&self.binding);
        bytes.extend_from_slice(self.outer.sha256());
        bytes.extend_from_slice(&self.outer.byte_len().to_le_bytes());
        bytes.extend_from_slice(&self.attempt.generation().to_le_bytes());
        bytes.extend_from_slice(self.attempt.session().as_bytes());
        bytes.extend_from_slice(self.attempt.invocation().as_bytes());
        bytes.push(self.slot);
        bytes.extend_from_slice(&self.transaction);
    }
}

pub(super) struct Core {
    pub(super) identity: [u8; 32],
    pub(super) header: Header,
    pub(super) tail: DecodedCompactReplayTailV1,
    pub(super) bytes: Vec<u8>,
    pub(super) storage: NativeWorkerCompactReplayStorageV1,
}
impl Core {
    pub(super) fn decode(bytes: &[u8], schema: Schema, b: &mut Budget<'_>) -> Result<Self> {
        let quote = NativeWorkerCompactReplayResourcesV1::for_wire_length(bytes.len())?;
        b.with_prepaid_scope(bytes.len(), 8, quote.work, quote.scratch, |_| {
            let mut owned = Vec::new();
            owned
                .try_reserve_exact(bytes.len())
                .map_err(|_| Resource::Allocation)?;
            owned.extend_from_slice(bytes);
            Self::decode_owned(owned, schema)
        })
    }

    pub(super) fn decode_owned(bytes: Vec<u8>, schema: Schema) -> Result<Self> {
        check_length(bytes.len())?;
        let (body, checksum) = bytes.split_at(bytes.len() - 32);
        let mut reader = CompactReplayReaderV1::new(body);
        if reader.take(8)? != schema.magic() {
            return Err(NativeWorkerCompactReplayErrorV1::Magic);
        }
        if reader.u16()? != schema.version() {
            return Err(NativeWorkerCompactReplayErrorV1::Version);
        }
        if hash(schema.checksum_domain(), body) != checksum {
            return Err(NativeWorkerCompactReplayErrorV1::Checksum);
        }
        let finalization = reader.array()?;
        let source = reader.array()?;
        let binding = reader.array()?;
        let outer = ContentIdentityV1::from_parts(reader.array()?, reader.u64()?);
        let generation = reader.u64()?;
        let session = reader.array()?;
        let invocation = reader.array()?;
        if reader.u8()? != 0 {
            return Err(NativeWorkerCompactReplayErrorV1::Coordinates);
        }
        let transaction = reader.array()?;
        let attempt = decode_attempt(generation, session, invocation)?;
        if [finalization, source, binding, transaction].contains(&[0; 32])
            || outer.byte_len() == 0
            || outer.byte_len() > schema.max_outer() as u64
        {
            return Err(NativeWorkerCompactReplayErrorV1::Coordinates);
        }
        // Always retain derivation bodies; no legacy tail fallback.
        let tail = decode_compact_replay_tail(&mut reader, true)?;
        let storage = retained_storage(bytes.len(), &tail)?;
        Ok(Self {
            identity: hash(schema.identity_domain(), &bytes),
            header: Header {
                finalization,
                source,
                binding,
                outer,
                attempt,
                slot: 0,
                transaction,
            },
            tail,
            bytes,
            storage,
        })
    }
}

pub(super) struct Inputs<'a> {
    pub(super) header: Header,
    pub(super) requests: [&'a [u8]; 2],
    pub(super) responses: [&'a crate::WorkerResponseV2; 2],
    pub(super) worker: &'a crate::WorkerMeasurementV1,
    pub(super) limits: crate::WorkerExecutionLimitsV1,
    pub(super) options: &'a [LinkOptionV1],
}

pub(super) fn prepare<'a>(
    floor: usize,
    schema: Schema,
    b: &mut Budget<'_>,
    input: impl FnOnce() -> Inputs<'a>,
) -> Result<Core> {
    b.with_prepaid_scope(floor, 8, 512, FRAME, |b| {
        let input = input();
        let request =
            extract_worker_v3_request_replay_parts_v1(input.requests[0], input.requests[1])
                .map_err(artifact_error)?;
        let bootstrap = input.responses[0]
            .replay_metadata()
            .map_err(artifact_error)?;
        let replay = input.responses[1]
            .replay_metadata()
            .map_err(artifact_error)?;
        validate_construction_parts(
            request.bootstrap_output_bound,
            &request.external_providers,
            input.options,
            bootstrap,
            replay,
        )?;
        let n = compact_replay_encoded_length(
            HEADER_BYTES - 8 - 2 - 64,
            true,
            input.worker,
            &request.external_providers,
            input.options,
            bootstrap,
            replay,
        )?;
        let quote = NativeWorkerCompactReplayResourcesV1::for_wire_length(n)?;
        b.with_prepaid_scope(b.storage(), 8, quote.work, quote.scratch, |_| {
            let mut bytes = Vec::new();
            bytes
                .try_reserve_exact(n)
                .map_err(|_| Resource::Allocation)?;
            input.header.encode(schema, &mut bytes);
            encode_compact_replay_tail(
                &mut bytes,
                true,
                input.worker,
                input.limits,
                request.bootstrap_output_bound,
                &request.external_providers,
                input.options,
                bootstrap,
                replay,
            )?;
            let checksum = hash(schema.checksum_domain(), &bytes);
            bytes.extend_from_slice(&checksum);
            debug_assert_eq!(bytes.len(), n);
            Core::decode_owned(bytes, schema)
        })
    })
}
