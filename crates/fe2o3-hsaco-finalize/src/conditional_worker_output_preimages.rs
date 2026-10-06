//! Complete compact replay bytes borrowed from an actual conditional publication.
use super::{Budget, Error, Published, Resource, Result};
use crate::first_build_worker_v3::{
    OwnedWorkerV3ProviderReplayPartV1, OwnedWorkerV3RequestReplayPartsV1,
    extract_worker_v3_request_replay_parts_v1 as extract,
};
use fe2o3_artifact_transaction::{
    DurablePublishedHsacoClaimV3, ProducerIdentity,
    WorkerV3ExternalProviderPayloadsV1 as Providers, WorkerV3PublicationIntentRecordV1 as Record,
};
use sha2::{Digest, Sha256};
use std::mem::size_of;

const FRAME: usize = 4096 + 4 * size_of::<ConditionalWorkerReplayPreimagesV5<'static>>();

/// Complete non-artifact replay preimages tied to the actual published owner.
/// This cannot be made from a record, a collection of hashes, or journal bytes.
/// The borrow keeps the nominal conditional source and current lease alive;
/// it does not authenticate a compiler or grant host load/launch authority.
/// ```compile_fail
/// use fe2o3_hsaco_finalize::ConditionalWorkerReplayPreimagesV5 as P;
/// fn clone(value: P<'_>) { let _ = value.clone(); }
/// ```
pub struct ConditionalWorkerReplayPreimagesV5<'a> {
    published: &'a Published,
    providers: Providers,
    retained_storage: usize,
}

impl Published {
    /// Finite extraction quote using actual request lengths and the existing
    /// closed provider-capacity ceiling. Includes both source/result checks and
    /// the returned owner while it coexists with extraction scratch. This may
    /// conservatively refuse an oversized input under the unchanged local cap.
    pub fn replay_preimages_quote(
        &self,
    ) -> std::result::Result<crate::ConditionalWorkerOperationQuoteV5, Resource> {
        use crate::ConditionalWorkerOperationQuoteV5 as Q;
        let source = self.recovered.finalized().source();
        let (work, scratch) = extraction_quote(
            source.bootstrap_request_bytes().len(),
            source.replay_request_bytes().len(),
        )
        .map_err(|_| Resource::Arithmetic)?;
        let retained = fe2o3_artifact_transaction::MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_BYTES_V1
            .checked_add(
                fe2o3_artifact_transaction::MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_PAYLOADS_V1
                    .checked_mul(size_of::<Vec<u8>>())
                    .ok_or(Resource::Arithmetic)?,
            )
            .and_then(|n| n.checked_add(size_of::<ConditionalWorkerReplayPreimagesV5<'_>>()))
            .ok_or(Resource::Arithmetic)?;
        let record_work = source
            .recovered_handoff()
            .handoff()
            .canonical_bytes()
            .len()
            .checked_add(self.recovered.transcript().canonical_bytes().len())
            .and_then(|n| n.checked_add(32))
            .ok_or(Resource::Arithmetic)?;
        let validation = super::retained_validation_quote(&self.recovered)?;
        self.account
            .operation_quote(self.retained_storage)?
            .nested(Q::new(work, scratch))?
            .nested(validation.sequential(validation)?)?
            .nested(Q::new(record_work, retained))
    }
    /// Re-extracts providers with the existing canonical Worker request parser.
    /// Source and lease checks bracket extraction. The complete returned Rust
    /// backing is unreserved: reserve `required_retained_storage()` before
    /// retaining it, in addition to this publication's existing reservation.
    pub fn replay_preimages(
        &self,
        producer: &ProducerIdentity,
        b: &mut Budget<'_>,
    ) -> Result<ConditionalWorkerReplayPreimagesV5<'_>> {
        self.account.run(b, self.retained_storage, |b| {
            let source = self.recovered.finalized().source();
            let bootstrap = source.bootstrap_request_bytes();
            let replay = source.replay_request_bytes();
            let (work, scratch) = extraction_quote(bootstrap.len(), replay.len())?;
            b.with_prepaid_scope(self.retained_storage, 8, work, scratch, |b| {
                let current = self
                    .publication
                    .current_publication_lease()
                    .acquire_current_token()
                    .map_err(Error::Currentness)?;
                let retained = super::Retained::new(&self.recovered, producer, b)?;
                super::validate_result(&retained, producer, &self.publication, b)?;
                let parts = extract(bootstrap, replay)
                    .map_err(|_| Error::Mismatch("canonical provider replay requests"))?;
                let (providers, storage) = provider_owner(parts)?;
                b.reserve_storage(storage)?;
                let result = ConditionalWorkerReplayPreimagesV5 {
                    published: self,
                    providers,
                    retained_storage: storage,
                };
                result.validate_record(b)?;
                let retained = super::Retained::new(&self.recovered, producer, b)?;
                super::validate_result(&retained, producer, &self.publication, b)?;
                current
                    .revalidate_locked_currentness()
                    .map_err(Error::Currentness)?;
                Ok(result)
            })
        })
    }
}

fn provider_owner(parts: OwnedWorkerV3RequestReplayPartsV1) -> Result<(Providers, usize)> {
    let mut payloads = Vec::new();
    payloads
        .try_reserve_exact(parts.external_providers.len())
        .map_err(|_| Resource::Allocation)?;
    if payloads.capacity() > crate::MAX_LINK_INPUTS {
        return Err(Resource::Accounting.into());
    }
    let mut storage = size_of::<ConditionalWorkerReplayPreimagesV5<'_>>()
        .checked_add(
            payloads
                .capacity()
                .checked_mul(size_of::<Vec<u8>>())
                .ok_or(Resource::Arithmetic)?,
        )
        .ok_or(Resource::Arithmetic)?;
    for part in parts.external_providers {
        storage = storage
            .checked_add(part.bytes.capacity())
            .ok_or(Resource::Arithmetic)?;
        payloads.push(part.bytes);
    }
    let providers =
        Providers::new(payloads).map_err(|_| Error::Mismatch("provider replay owner bounds"))?;
    Ok((providers, storage))
}

impl ConditionalWorkerReplayPreimagesV5<'_> {
    pub fn revalidation_quote(
        &self,
    ) -> std::result::Result<crate::ConditionalWorkerOperationQuoteV5, Resource> {
        use crate::ConditionalWorkerOperationQuoteV5 as Q;
        let inputs = self
            .published
            .retained_storage
            .checked_add(self.retained_storage)
            .ok_or(Resource::Arithmetic)?;
        let record_work = self
            .outer_handoff()
            .len()
            .checked_add(self.transcript().len())
            .and_then(|n| n.checked_add(32))
            .ok_or(Resource::Arithmetic)?;
        self.published
            .account
            .operation_quote(inputs)?
            .nested(Q::new(8, FRAME))?
            .nested(self.published.revalidation_quote()?)?
            .nested(Q::new(record_work, 0))
    }
    pub fn publication(&self) -> &Published {
        self.published
    }
    pub fn published_claim(&self) -> &DurablePublishedHsacoClaimV3 {
        self.published.published_claim()
    }
    pub fn record(&self) -> Record {
        self.published.recovered.record()
    }
    pub fn outer_handoff(&self) -> &[u8] {
        self.published
            .recovered
            .finalized()
            .source()
            .recovered_handoff()
            .handoff()
            .canonical_bytes()
    }
    pub fn transcript(&self) -> &[u8] {
        self.published.recovered.transcript().canonical_bytes()
    }
    pub fn providers(&self) -> &Providers {
        &self.providers
    }
    pub const fn required_retained_storage(&self) -> usize {
        self.retained_storage
    }
    pub const fn grants_load_authority(&self) -> bool {
        false
    }
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }

    /// Checks actual source/lease continuity and every retained replay byte.
    pub fn revalidate(&self, producer: &ProducerIdentity, b: &mut Budget<'_>) -> Result<()> {
        let floor = self
            .published
            .retained_storage
            .checked_add(self.retained_storage)
            .ok_or(Resource::Arithmetic)?;
        self.published.account.run(b, floor, |b| {
            b.with_prepaid_scope(floor, 8, 8, FRAME, |b| {
                self.published.revalidate(producer, b)?;
                self.validate_record(b)
            })
        })
    }

    fn validate_record(&self, b: &mut Budget<'_>) -> Result<()> {
        let outer = self.outer_handoff();
        let transcript = self.transcript();
        b.charge_work(
            outer
                .len()
                .checked_add(transcript.len())
                .and_then(|n| n.checked_add(32))
                .ok_or(Resource::Arithmetic)?,
        )?;
        let actual = Coordinates {
            outer_hash: Sha256::digest(outer).into(),
            outer_len: outer.len(),
            transcript_hash: Sha256::digest(transcript).into(),
            transcript_len: transcript.len(),
            provider_hash: self.providers.canonical_sha256(),
            provider_archive_len: self.providers.canonical_length(),
            provider_count: self.providers.len(),
            provider_bytes: self.providers.payload_length(),
        };
        require_coordinates(Coordinates::from_record(self.record()), actual)
    }
}

// Covers parser hashing, request comparisons, provider copies/hashes and fixed
// temporary headers. The existing Worker parser separately enforces its wire,
// per-provider and aggregate allocation bounds before returning any owner.
fn extraction_quote(bootstrap: usize, replay: usize) -> Result<(usize, usize)> {
    let bytes = bootstrap.checked_add(replay).ok_or(Resource::Arithmetic)?;
    let headers = crate::MAX_LINK_INPUTS
        .checked_mul(size_of::<OwnedWorkerV3ProviderReplayPartV1>() + size_of::<Vec<u8>>())
        .ok_or(Resource::Arithmetic)?;
    let work = bytes
        .checked_mul(4)
        .and_then(|n| n.checked_add(8 + crate::MAX_LINK_INPUTS))
        .ok_or(Resource::Arithmetic)?;
    let scratch = bootstrap
        .checked_add(headers)
        .and_then(|n| n.checked_add(FRAME))
        .ok_or(Resource::Arithmetic)?;
    Ok((work, scratch))
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct Coordinates {
    outer_hash: [u8; 32],
    outer_len: usize,
    transcript_hash: [u8; 32],
    transcript_len: usize,
    provider_hash: [u8; 32],
    provider_archive_len: usize,
    provider_count: usize,
    provider_bytes: usize,
}
impl Coordinates {
    fn from_record(record: Record) -> Self {
        Self {
            outer_hash: record.outer_handoff_sha256(),
            outer_len: record.outer_handoff_length(),
            transcript_hash: record.transcript_sha256(),
            transcript_len: record.transcript_length(),
            provider_hash: record.external_provider_archive_sha256(),
            provider_archive_len: record.external_provider_archive_length(),
            provider_count: record.external_provider_count(),
            provider_bytes: record.external_provider_payload_length(),
        }
    }
}
fn require_coordinates(expected: Coordinates, actual: Coordinates) -> Result<()> {
    if expected != actual {
        return Err(Error::Mismatch("complete compact replay preimages"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ContentIdentityV1, WorkerInputKindV1, WorkerInputV1, WorkerOptimizationLevelV1,
        WorkerOptionsV1, WorkerOutputConstraintsV1,
        worker_protocol_v2::{
            SealedWorkerRequestV2Parts, WorkerCompilerFfiEnvelopeIdentityV2, WorkerRequestV2,
        },
    };
    use fe2o3_kernel_descriptor::{CodeObjectVersion, DeviceTargetV1};

    fn request(id: u8, changed: bool) -> WorkerRequestV2 {
        let mut providers: Vec<_> = [
            b"provider-one".as_slice(),
            b"provider-two",
            b"provider-three",
        ]
        .into_iter()
        .map(|bytes| WorkerInputV1::new(WorkerInputKindV1::LlvmBitcode, bytes.to_vec()).unwrap())
        .collect();
        providers.sort_by_key(|input| (input.identity(), input.kind()));
        WorkerRequestV2::from_sealed_parts(SealedWorkerRequestV2Parts {
            request_id: [id; 32],
            llvm_build_identity: "llvm-test".into(),
            worker_build_identity: if changed {
                "other-worker"
            } else {
                "worker-test"
            }
            .into(),
            worker_executable: ContentIdentityV1::from_parts([42; 32], 4096),
            target: DeviceTargetV1::parse("amdgcn-amd-amdhsa--gfx942").unwrap(),
            code_object_version: CodeObjectVersion::V6,
            options: WorkerOptionsV1::new(WorkerOptimizationLevelV1::O2, true, true),
            compiler_envelope: WorkerCompilerFfiEnvelopeIdentityV2::from_test_bytes([43; 32]),
            compiler_module: WorkerInputV1::new(WorkerInputKindV1::LlvmTextIr, b"module".to_vec())
                .unwrap(),
            external_providers: providers,
            import_symbols: Vec::new(),
            export_symbols: vec!["kernel".into()],
            final_symbols: vec!["kernel".into()],
            output: WorkerOutputConstraintsV1::new(4096).unwrap(),
        })
        .unwrap()
    }

    #[test]
    fn actual_parser_retains_all_providers_and_refuses_changed_request() {
        let bootstrap = request(1, false);
        let replay = request(2, false);
        let parts = extract(bootstrap.canonical_bytes(), replay.canonical_bytes()).unwrap();
        let expected: Vec<_> = parts
            .external_providers
            .iter()
            .map(|part| part.bytes.clone())
            .collect();
        let (providers, storage) = provider_owner(parts).unwrap();
        assert_eq!(providers.len(), 3);
        assert_eq!(
            providers.iter().collect::<Vec<_>>(),
            expected.iter().map(Vec::as_slice).collect::<Vec<_>>()
        );
        assert!(
            storage
                >= size_of::<ConditionalWorkerReplayPreimagesV5<'_>>()
                    + 3 * size_of::<Vec<u8>>()
                    + providers.payload_length()
        );
        assert!(
            extract(
                bootstrap.canonical_bytes(),
                request(2, true).canonical_bytes()
            )
            .is_err()
        );
        let mut corrupt = bootstrap.canonical_bytes().to_vec();
        let last = corrupt.len() - 1;
        corrupt[last] ^= 1;
        assert!(extract(&corrupt, replay.canonical_bytes()).is_err());
    }

    #[test]
    fn provider_move_retains_original_allocations_and_spare_capacity_charge() {
        let mut payload = Vec::with_capacity(257);
        payload.extend_from_slice(b"payload");
        let capacity = payload.capacity();
        let pointer = payload.as_ptr();
        let part = OwnedWorkerV3ProviderReplayPartV1 {
            kind: WorkerInputKindV1::LlvmBitcode,
            identity: ContentIdentityV1::calculate(&payload),
            bytes: payload,
        };
        let (providers, storage) = provider_owner(OwnedWorkerV3RequestReplayPartsV1 {
            bootstrap_output_bound: 4096,
            external_providers: vec![part],
        })
        .unwrap();
        assert_eq!(providers.get(0).unwrap().as_ptr(), pointer);
        assert!(
            storage
                >= size_of::<ConditionalWorkerReplayPreimagesV5<'_>>()
                    + size_of::<Vec<u8>>()
                    + capacity
        );
        assert_eq!(providers.payload_length(), 7);
    }
    #[test]
    fn every_compact_preimage_coordinate_is_required() {
        let expected = Coordinates {
            outer_hash: [1; 32],
            outer_len: 2,
            transcript_hash: [3; 32],
            transcript_len: 4,
            provider_hash: [5; 32],
            provider_archive_len: 6,
            provider_count: 7,
            provider_bytes: 8,
        };
        assert!(require_coordinates(expected, expected).is_ok());
        for index in 0..8 {
            let mut changed = expected;
            match index {
                0 => changed.outer_hash[0] ^= 1,
                1 => changed.outer_len += 1,
                2 => changed.transcript_hash[0] ^= 1,
                3 => changed.transcript_len += 1,
                4 => changed.provider_hash[0] ^= 1,
                5 => changed.provider_archive_len += 1,
                6 => changed.provider_count += 1,
                _ => changed.provider_bytes += 1,
            }
            assert!(require_coordinates(expected, changed).is_err());
        }
    }
    #[test]
    fn extraction_quotes_are_monotone_and_checked() {
        let (work, scratch) = extraction_quote(17, 31).unwrap();
        assert!(work >= 4 * 48 && scratch >= 17 + FRAME);
        assert_eq!(extraction_quote(18, 31).unwrap(), (work + 4, scratch + 1));
        assert_eq!(extraction_quote(17, 32).unwrap(), (work + 4, scratch));
        assert!(extraction_quote(usize::MAX, 1).is_err());
        assert!(extraction_quote(usize::MAX / 2, 0).is_err());
    }
}
