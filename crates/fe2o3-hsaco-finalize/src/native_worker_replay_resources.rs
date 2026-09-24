//! Checked Rust resource schedule for native durable Worker replay.
//!
//! Covers one nested V2 decode, `reconstruct_worker_exchanges`, and
//! `recover_prepaid_native_worker_evidence_v1` on the ORIGINAL caller ledger.
//! Source/carrier recovery, receipt reconstruction and raw-HSACO derivation,
//! hashing/inspection in the artifact domain remain separately bounded/charged.
//! The shared reconstruction helper's *additional* raw-output hash is paid here.
//! No process, signing, publication, load or launch authority is conferred.
//!
//! Before quoting, the caller admits <=127 providers and matching reference
//! counts, then prepays its bounded length census. For P owned payload bytes and
//! n providers, prepare_providers reserves P + n*(sizeof(WorkerInputV1) +
//! sizeof(Vec<u8>)) and pays 3*P + 256*n work before construction. Moving each
//! payload into WorkerInputV1::new hashes it twice; comparing the declared
//! identity is fixed-size. The third pass also permits a prior identity.matches
//! check, but NOT a further payload clone. Borrowed-to-owned staging requires
//! another P bytes of work/storage. Fixed caller frames/Vec headers are separate.
//! The parent restoring scope keeps this provider reservation until drop/return;
//! existing artifact payload funding is not silently released on the move.
//!
//! Quote construction performs only bounded length/arithmetic census: the
//! existing quote visits <=127 provider lengths and two lengths per <=64 options;
//! this leaf reads at most three metadata-body lengths per direction. Charge
//! entry/census work before calling. It performs no payload copy/hash/decode.
//! Run the existing V3 aggregate working-set guard separately before replay.
//!
//! Keep source, transcript, provider and artifact-owner floors paid while
//! reserving reconstruction_storage in Budget::with_prepaid_scope. That scope
//! restores scratch on every outcome. The returned evidence then needs a NEW
//! reservation of first_build.returned_retained_storage() plus
//! sizeof(InertNativeFirstBuildWorkerEvidenceV1); source storage remains separate.
//! The closed Consumed/Replayed owner is sized using its current Rust type.
//! Caller frame headers are additional. Spare caller capacity, allocator metadata,
//! RSS and CPU instruction counts are excluded, as in the original quote.
//!
//! Reaudit alongside the shared helpers/codecs and pinned nightly collections.

use std::mem::size_of;

use fe2o3_compiler_ffi::CompilerModuleHandoffV2;

use crate::{
    ContentIdentityV1, InertNativeFirstBuildWorkerEvidenceV1, LinkInputV1, LinkOptionV1,
    MAX_WORKER_OUTPUT_BYTES, MAX_WORKER_RESPONSE_BYTES, MAX_WORKER_TOOLCHAIN_ID_BYTES,
    WorkerExecutionLimitsV1, WorkerInputKindV1, WorkerInputV1, WorkerOutputConstraintsV1,
    first_build_worker_native_resources::{
        NativeWorkerResourceQuote, NativeWorkerResourceQuoteError,
    },
    worker_protocol_v2::{
        MAX_WORKER_RESPONSE_REPLAY_METADATA_SHELL_BYTES_V1, WorkerResponseReplayMetadataV1,
    },
};

type Result<T> = std::result::Result<T, NativeWorkerResourceQuoteError>;
type KindRow = (ContentIdentityV1, WorkerInputKindV1);

// The existing engine's two 41-byte-key WorkerInput sorts dominate the two
// helper sorts (LinkInput and identity/kind). Make the row-size premise explicit.
const _: () = {
    assert!(size_of::<LinkInputV1>() <= size_of::<WorkerInputV1>());
    assert!(size_of::<KindRow>() <= size_of::<WorkerInputV1>());
    assert!(size_of::<WorkerInputKindV1>() == 1);
};

/// Actual borrowed response dimensions, not caller-selected work allowances.
/// The caller must pass the same output/build/metadata to reconstruction.
#[derive(Clone, Copy, Debug)]
pub(crate) struct NativeWorkerReplayResponseInputs<'a> {
    pub(crate) raw_output_bytes: usize,
    pub(crate) worker_build_identity_bytes: usize,
    pub(crate) bootstrap_metadata: WorkerResponseReplayMetadataV1<'a>,
    pub(crate) replay_metadata: WorkerResponseReplayMetadataV1<'a>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NativeWorkerReplayResourceQuote {
    pub(crate) reconstruction_work: usize,
    pub(crate) reconstruction_storage: usize,
    pub(crate) first_build: NativeWorkerResourceQuote,
}

impl NativeWorkerReplayResourceQuote {
    pub(crate) fn new(
        module: &CompilerModuleHandoffV2,
        providers: &[WorkerInputV1],
        options: &[LinkOptionV1],
        bootstrap_output: &WorkerOutputConstraintsV1,
        limits: WorkerExecutionLimitsV1,
        responses: NativeWorkerReplayResponseInputs<'_>,
    ) -> Result<Self> {
        // This admission also bounds counts BEFORE visiting provider/option rows.
        let first_build =
            NativeWorkerResourceQuote::new(module, providers, options, bootstrap_output, limits)?;
        let output_bound = usize::try_from(bootstrap_output.max_bytes())
            .map_err(|_| NativeWorkerResourceQuoteError::Arithmetic("replay output bound"))?;
        let bootstrap = ResponseEncoding::new(
            ResponseDimensions::from_metadata(responses.bootstrap_metadata),
            responses.raw_output_bytes,
            responses.worker_build_identity_bytes,
            output_bound,
            limits.stdout_bytes(),
        )?;
        let replay = ResponseEncoding::new(
            ResponseDimensions::from_metadata(responses.replay_metadata),
            responses.raw_output_bytes,
            responses.worker_build_identity_bytes,
            output_bound,
            limits.stdout_bytes(),
        )?;
        Self::compose(
            first_build,
            providers.len(),
            responses.raw_output_bytes,
            [bootstrap, replay],
        )
    }

    fn compose(
        first_build: NativeWorkerResourceQuote,
        provider_count: usize,
        output: usize,
        responses: [ResponseEncoding; 2],
    ) -> Result<Self> {
        // Reuse preflight + execution ONCE. Their call-count envelopes are:
        //   nested decode:              1 supplied / 1 needed
        //   request encoders:           3 supplied / 2 needed
        //   request metadata schedules: 5 supplied / 4 needed (2 build, 2 replay)
        //   plan schedules:             3 supplied / 2 needed (derive, validate)
        //   module hashes:              8 supplied / 6 needed (2+2 build, 1+1 helper)
        //   response metadata decodes:  2 supplied / 2 needed, PLUS 2 below
        //   validate_replay_parts and evidence hashing: 1 supplied / 1 needed.
        // The two engine input-sort envelopes cover derive_link_plan's presort
        // and plan_inputs_with_kinds' sort. Request metadata retains sort scratch
        // with larger WorkerInput rows; its five instances cover the two helper
        // sorts plus two request builders and replay validation. The third plan
        // instance is conservative overlap, NOT credit for response encoding.
        // Input-kind closure hashes, option cloning/validation, request-ID joins,
        // plan canonicalization/equality and four transcript hashes are already
        // in these audited request/plan/replay schedules. WorkerMeasurement's
        // two bounded String clones fit the existing frame work/retained storage.
        // Both request transcript copies are paid by the original byte schedule.
        // Unused synthetic/capture allowances are not fresh execution. Process
        // supervision (including procfs) is outside BOTH Rust codec schedules.
        let (metadata_work, metadata_storage) = first_build.response_metadata_resources();
        let count = sum([provider_count, 1], "replay input count")?;
        // plan_inputs_with_kinds adds an exact-reserved tuple Vec and kind Vec.
        // Pay identity/kind reads, tuple write/read, and final kind writes. Sort
        // row moves and closure hashing are accounted separately above.
        let kind_rows = product(count, size_of::<KindRow>(), "replay kind rows")?;
        let kinds = product(count, size_of::<WorkerInputKindV1>(), "replay kinds")?;
        let kind_work = sum(
            [
                product(count, 41, "replay kind source reads")?,
                product(kind_rows, 2, "replay kind write/read")?,
                kinds,
            ],
            "replay kind staging work",
        )?;
        let kind_storage = sum(
            [
                kind_rows,
                kinds,
                size_of::<Vec<KindRow>>(),
                size_of::<Vec<WorkerInputKindV1>>(),
            ],
            "replay kind staging storage",
        )?;
        let evidence_shell = size_of::<InertNativeFirstBuildWorkerEvidenceV1>();
        // Refuse overflow now, before recovery constructs an otherwise unusable
        // evidence owner. The recovery helper returns precisely this sum.
        sum(
            [first_build.returned_retained_storage(), evidence_shell],
            "replay retained evidence",
        )?;
        let reconstruction_work = sum(
            [
                first_build.preflight_work,
                first_build.execution_work,
                product(metadata_work, 2, "replay metadata prevalidation")?,
                responses[0].work,
                responses[1].work,
                output, // raw_identity in reconstruct_worker_exchanges
                kind_work,
                evidence_shell,
            ],
            "replay reconstruction work",
        )?;
        let reconstruction_storage = sum(
            [
                first_build.preflight_storage,
                first_build.execution_storage,
                product(metadata_storage, 2, "replay metadata prevalidation storage")?,
                responses[0].storage,
                responses[1].storage,
                kind_storage,
                evidence_shell,
            ],
            "replay reconstruction storage",
        )?;
        Ok(Self {
            reconstruction_work,
            reconstruction_storage,
            first_build,
        })
    }
}

#[derive(Clone, Copy)]
struct ResponseDimensions {
    diagnostics: usize,
    provider: Option<usize>,
    derivation: Option<usize>,
}

impl ResponseDimensions {
    fn from_metadata(metadata: WorkerResponseReplayMetadataV1<'_>) -> Self {
        Self {
            diagnostics: metadata.diagnostics_body().len(),
            provider: metadata.provider_evidence_body().map(<[u8]>::len),
            derivation: metadata.derivation_evidence_body().map(<[u8]>::len),
        }
    }
}

/// Only the encoding addition; the original quote pays full strict decoding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ResponseEncoding {
    #[cfg(test)]
    wire: usize,
    work: usize,
    storage: usize,
}

impl ResponseEncoding {
    fn new(
        d: ResponseDimensions,
        output: usize,
        build_identity: usize,
        output_bound: usize,
        stdout: usize,
    ) -> Result<Self> {
        nonempty("replay raw output", output)?;
        bound("replay raw output", output, MAX_WORKER_OUTPUT_BYTES)?;
        bound("replay bootstrap output", output, output_bound)?;
        nonempty("replay worker build identity", build_identity)?;
        bound(
            "replay worker build identity",
            build_identity,
            MAX_WORKER_TOOLCHAIN_ID_BYTES,
        )?;
        let metadata = sum(
            [
                d.diagnostics,
                d.provider.unwrap_or(0),
                d.derivation.unwrap_or(0),
            ],
            "replay response metadata",
        )?;
        bound(
            "replay response metadata",
            metadata,
            MAX_WORKER_RESPONSE_REPLAY_METADATA_SHELL_BYTES_V1,
        )?;

        // V2 has seven fields; V3 adds provider + identity; V4 adds provider
        // (empty if absent), derivation + identity. No trial decode/downgrade.
        let extension = if d.derivation.is_some() {
            3 * 6 + 32
        } else if d.provider.is_some() {
            2 * 6 + 32
        } else {
            0
        };
        let output_body = sum([1 + 32 + 8, output], "replay output body")?;
        let wire = sum(
            [
                8,
                7 * 6,
                3 * 32,
                1,
                build_identity,
                metadata,
                output_body,
                extension,
            ],
            "replay response wire",
        )?;
        bound("replay response wire", wire, MAX_WORKER_RESPONSE_BYTES)?;
        // Reconstruction itself checks only the protocol cap. Enforce capture
        // admission HERE or the original metadata/returned evidence bounds,
        // which depend on recorded stdout, would not apply to replay.
        bound("replay response stdout", wire, stdout)?;
        let hash = if extension == 0 {
            0
        } else {
            let prefix = if d.derivation.is_some() {
                b"FE2O3/DIRECT-LLVM-WORKER-RESPONSE/V4\0".len()
            } else {
                b"FE2O3/DIRECT-LLVM-WORKER-RESPONSE/V3\0".len()
            };
            // Identity hashes the preceding wire, excluding its own TLV.
            sum(
                [wire - (6 + 32), prefix, 8],
                "replay response identity hash",
            )?
        };
        Ok(Self {
            #[cfg(test)]
            wire,
            // Hash raw output, write the output body, write final wire, then
            // hash its family-specific identity. Canonical decode is NOT free:
            // it is paid by first_build, including its separate output hash/copy.
            work: sum(
                [output, output_body, wire, hash],
                "replay response encoding work",
            )?,
            // Both buffers remain live during strict decode. Sum BOTH directions
            // in compose even though bootstrap encoding drops before replay.
            storage: sum([output_body, wire], "replay response encoding storage")?,
        })
    }
}

fn sum<const N: usize>(values: [usize; N], field: &'static str) -> Result<usize> {
    values.into_iter().try_fold(0usize, |total, value| {
        total
            .checked_add(value)
            .ok_or(NativeWorkerResourceQuoteError::Arithmetic(field))
    })
}

fn product(a: usize, b: usize, field: &'static str) -> Result<usize> {
    a.checked_mul(b)
        .ok_or(NativeWorkerResourceQuoteError::Arithmetic(field))
}

fn bound(component: &'static str, actual: usize, maximum: usize) -> Result<()> {
    if actual > maximum {
        Err(NativeWorkerResourceQuoteError::HardBound {
            component,
            actual,
            maximum,
        })
    } else {
        Ok(())
    }
}

fn nonempty(component: &'static str, actual: usize) -> Result<()> {
    if actual == 0 {
        Err(NativeWorkerResourceQuoteError::Empty(component))
    } else {
        Ok(())
    }
}

#[cfg(test)]
#[path = "native_worker_replay_resources_tests.rs"]
mod tests;
