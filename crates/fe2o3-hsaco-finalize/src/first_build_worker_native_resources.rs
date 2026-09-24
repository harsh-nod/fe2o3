//! Checked native first-build Rust resource schedule, pinned to the current
//! codecs and nightly-2026-04-03 collection implementations.
//!
//! Process supervision is a SEPARATE accounting domain, not a reason to reject
//! a Rust codec quote. Its existing walltime/image/input/output admission remains
//! unchanged. In particular, timeouts are not converted to logical work: retries
//! of interrupted IO, procfs reads and accumulated descendant scans are excluded.
//!
//! `new` covers one nested V2 decode and engine preflight, then two response
//! decodes, authorized request construction, shared replay and evidence hashing.
//! Work is logical byte/row/comparison visits, not CPU instructions. Arithmetic
//! overflow or a declared hard bound refuses the quote before consume/execution.
//! Reaudit this schedule when the codecs, collection implementation or limits
//! change. In particular, the synthetic output probe must remain injective and
//! capped at inputs + 1; this schedule does not cover the old SHA retry loop.
//! No OS, allocator, LLVM, production, or backend authority is asserted.
//!
//! Integration: keep the original caller ledger and its already accepted owner
//! floor (consumed storage or the actual retained raw/recovered backing). These
//! storage amounts are ADDITIONAL logical buffer reservations, not replacements
//! for that floor or measurements of allocation capacity/RSS. Use the existing
//! `Budget::with_prepaid_scope` for a covered operation. It restores temporary
//! storage, including on failure/unwind. Returned owners are then unreserved.
//! Reserve `preflight_storage` while retaining the prepared engine. On successful
//! execution, reserve `returned_retained_storage` plus the adapter's owner shell
//! on that same ledger. `returned_buffer_storage` exposes its payload component
//! only. Original input reservations remain separate until those owners drop.
//! Never reset the parent Budget. Excess caller capacity and allocator internals
//! are outside this logical accounting; internal collection growth is included.
//!
//! The V3 aggregate guard remains authoritative and must run before consume or
//! execution. Call `first_build_worker_v3::enforce_worker_working_set_budget(
//! outer_handoff_bytes, nested, external_providers, link_options)` separately.
//! This leaf deliberately does not duplicate its 512 MiB admission policy.

use std::{mem::size_of, time::Duration};

use sha2::Sha256;

use fe2o3_compiler_ffi::{
    CompilerFfiContractV1, CompilerModuleHandoffV2, CompilerModuleSymbolRoleV1,
    MAX_COMPILER_FFI_CONTRACTS_V1, MAX_COMPILER_FFI_ENVELOPE_BYTES_V1,
    MAX_COMPILER_MODULE_BYTES_V1, MAX_COMPILER_MODULE_HANDOFF_BYTES_V2,
    MAX_COMPILER_MODULE_SYMBOL_BYTES_V1, MAX_COMPILER_MODULE_SYMBOL_MANIFEST_BYTES_V1,
    MAX_COMPILER_MODULE_SYMBOLS_V1,
};

use crate::{
    ContentIdentityV1, LinkInputV1, LinkOptionV1, MAX_LINK_INPUTS, MAX_LINK_OPTION_NAME_BYTES,
    MAX_LINK_OPTION_VALUE_BYTES, MAX_LINK_OPTIONS, MAX_WORKER_DIAGNOSTICS, MAX_WORKER_OUTPUT_BYTES,
    MAX_WORKER_REQUEST_BYTES, MAX_WORKER_RESPONSE_BYTES, MAX_WORKER_SYMBOL_BYTES,
    MAX_WORKER_SYMBOLS, MAX_WORKER_TARGET_BYTES, MAX_WORKER_TOOLCHAIN_ID_BYTES,
    MAX_WORKER_TOTAL_DIAGNOSTIC_BYTES, MAX_WORKER_TOTAL_INPUT_BYTES, MultiInputLinkPlanV1,
    ProvenanceNodeV1, WorkerExecutionLimitsV1, WorkerInputV1, WorkerOutputConstraintsV1,
    WorkerRequestV2, WorkerResponseV2,
    first_build_worker_engine::{
        ReproducibleFirstBuildEnginePreflight, ReproducibleFirstBuildEngineResult,
    },
    request_construction::DecodedCompilerModuleHandoffV2,
    worker_executor::{MAX_WORKER_STDERR_BYTES, MAX_WORKER_TIMEOUT},
    worker_protocol_v2::{
        MAX_PROVIDER_BASENAME_BYTES, MAX_PROVIDER_FILES, MAX_PROVIDER_IDENTITY_BYTES,
        MAX_WORKER_RESPONSE_REPLAY_METADATA_SHELL_BYTES_V1,
        WorkerDeviceLibraryProviderFileEvidenceV1, WorkerNativeLinkInputEvidenceV1,
    },
};

/// Additional prepaid Rust work/storage; existing source-owner floors are separate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NativeWorkerResourceQuote {
    pub(crate) preflight_storage: usize,
    pub(crate) execution_storage: usize,
    pub(crate) preflight_work: usize,
    pub(crate) execution_work: usize,
    returned_buffer_storage: usize,
    returned_retained_storage: usize,
    request_wire_bytes: usize,
    request_encoding_bytes: usize,
    response_capture_bytes: usize,
    response_decode_output_bytes: usize,
    successful_response_bytes: usize,
    response_metadata_work: usize,
    response_metadata_storage: usize,
    total_worker_timeout: Duration,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NativeWorkerResourceQuoteError {
    Arithmetic(&'static str),
    HardBound {
        component: &'static str,
        actual: usize,
        maximum: usize,
    },
    Empty(&'static str),
    HandoffExtent,
    InvalidExecutionLimits,
}

impl std::fmt::Display for NativeWorkerResourceQuoteError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Arithmetic(component) => {
                write!(formatter, "native Worker quote overflow: {component}")
            }
            Self::HardBound {
                component,
                actual,
                maximum,
            } => write!(
                formatter,
                "native Worker {component} {actual} exceeds {maximum}"
            ),
            Self::Empty(component) => write!(formatter, "native Worker {component} is empty"),
            Self::HandoffExtent => {
                formatter.write_str("native Worker nested extents exceed handoff")
            }
            Self::InvalidExecutionLimits => {
                formatter.write_str("invalid native Worker execution limits")
            }
        }
    }
}

impl std::error::Error for NativeWorkerResourceQuoteError {}

type QuoteResult<T> = Result<T, NativeWorkerResourceQuoteError>;

#[derive(Clone, Copy)]
struct Dimensions {
    handoff: usize,
    module: usize,
    envelope: usize,
    manifest: usize,
    providers: usize,
    provider_count: usize,
    option_text: usize,
    option_count: usize,
    symbols: usize,
    contracts: usize,
    output: usize,
    stdout: usize,
    stderr: usize,
    timeout: Duration,
}

impl NativeWorkerResourceQuote {
    /// Quotes the finite Rust staging, codec and replay operations before consume.
    pub(crate) fn new(
        module: &CompilerModuleHandoffV2,
        external_providers: &[WorkerInputV1],
        link_options: &[LinkOptionV1],
        exact_output_bound: &WorkerOutputConstraintsV1,
        limits: WorkerExecutionLimitsV1,
    ) -> QuoteResult<Self> {
        Self::from_sources(
            module,
            external_providers,
            link_options,
            exact_output_bound,
            limits,
        )
    }

    /// Reads actual retained lengths; accepts neither digests nor a caller work allowance.
    ///
    /// The nested handoff length is its selected wire extent, not its backing
    /// capacity or previously accepted outer owner floor. No hashing, copying, sorting,
    /// decoding, consuming, or execution occurs here. Counts are checked before
    /// walking caller collections; the walks visit at most 127 providers and 64
    /// options, inspecting two string lengths per option. Charge this small
    /// length census separately before calling, if the caller meters entry work.
    fn from_sources(
        module: &CompilerModuleHandoffV2,
        external_providers: &[WorkerInputV1],
        link_options: &[LinkOptionV1],
        exact_output_bound: &WorkerOutputConstraintsV1,
        limits: WorkerExecutionLimitsV1,
    ) -> QuoteResult<Self> {
        check_counts(external_providers.len(), link_options.len())?;
        let providers = sum(
            external_providers.iter().map(|input| input.bytes().len()),
            "provider payload",
        )?;
        let option_text = sum(
            link_options
                .iter()
                .flat_map(|option| [option.name().len(), option.value().len()]),
            "option text",
        )?;
        Self::from_dimensions(Dimensions {
            handoff: module.canonical_bytes().len(),
            module: module.module_bytes().len(),
            envelope: module.envelope().canonical_bytes().len(),
            manifest: module.symbol_manifest().canonical_bytes().len(),
            providers,
            provider_count: external_providers.len(),
            option_text,
            option_count: link_options.len(),
            symbols: module.symbol_manifest().symbol_count(),
            contracts: sum(
                [
                    module.envelope().inspection().import_count(),
                    module.envelope().inspection().export_count(),
                ],
                "envelope contract count",
            )?,
            output: usize::try_from(exact_output_bound.max_bytes())
                .map_err(|_| NativeWorkerResourceQuoteError::Arithmetic("output width"))?,
            stdout: limits.stdout_bytes(),
            stderr: limits.stderr_bytes(),
            timeout: limits.timeout(),
        })
    }

    /// Complete result buffers and metadata; add the native adapter owner shell.
    /// Excludes the original source and prepared-owner reservations.
    pub(crate) const fn returned_retained_storage(&self) -> usize {
        self.returned_retained_storage
    }

    /// One audited metadata decode; replay framing validates it once more.
    pub(crate) const fn response_metadata_resources(&self) -> (usize, usize) {
        (self.response_metadata_work, self.response_metadata_storage)
    }

    fn from_dimensions(d: Dimensions) -> QuoteResult<Self> {
        check_counts(d.provider_count, d.option_count)?;
        bound(
            "manifest symbol count",
            d.symbols,
            MAX_COMPILER_MODULE_SYMBOLS_V1,
        )?;
        bound(
            "envelope contract count",
            d.contracts,
            MAX_COMPILER_FFI_CONTRACTS_V1,
        )?;
        for (component, actual, maximum) in [
            ("handoff", d.handoff, MAX_COMPILER_MODULE_HANDOFF_BYTES_V2),
            ("module", d.module, MAX_COMPILER_MODULE_BYTES_V1),
            ("envelope", d.envelope, MAX_COMPILER_FFI_ENVELOPE_BYTES_V1),
            (
                "manifest",
                d.manifest,
                MAX_COMPILER_MODULE_SYMBOL_MANIFEST_BYTES_V1,
            ),
            ("output", d.output, MAX_WORKER_OUTPUT_BYTES),
        ] {
            bound(component, actual, maximum)?;
            if actual == 0 {
                return Err(NativeWorkerResourceQuoteError::Empty(component));
            }
        }
        if d.stdout == 0
            || d.stdout > MAX_WORKER_RESPONSE_BYTES
            || d.stderr > MAX_WORKER_STDERR_BYTES
            || d.timeout.is_zero()
            || d.timeout > MAX_WORKER_TIMEOUT
        {
            return Err(NativeWorkerResourceQuoteError::InvalidExecutionLimits);
        }
        let metadata = sum([d.envelope, d.manifest], "handoff metadata bytes")?;
        if sum([d.module, metadata], "nested handoff extent")? > d.handoff {
            return Err(NativeWorkerResourceQuoteError::HandoffExtent);
        }
        let inputs = sum([d.module, d.providers], "aggregate input payload")?;
        bound(
            "aggregate input payload",
            inputs,
            MAX_WORKER_TOTAL_INPUT_BYTES,
        )?;
        let option_bytes_per_row = sum(
            [MAX_LINK_OPTION_NAME_BYTES, MAX_LINK_OPTION_VALUE_BYTES],
            "option row bound",
        )?;
        bound(
            "option text",
            d.option_text,
            product(d.option_count, option_bytes_per_row, "option text bound")?,
        )?;

        // encode_request: magic + 15 TLV headers, fixed bodies, three symbol
        // counts and provider count. Each input has a kind, digest and u64 length.
        // Manifest rows contain a role + u32 length + text; envelope directional
        // text is also length-delimited. Three complete metadata images therefore
        // cover the three encoded symbol lists without inspecting/sorting names.
        let fixed_request = sum(
            [
                8,
                15 * 6,
                32,
                2 * MAX_WORKER_TOOLCHAIN_ID_BYTES,
                40,
                MAX_WORKER_TARGET_BYTES,
                1,
                3,
                32,
                8,
                32,
                4,
                3 * 4,
            ],
            "request framing",
        )?;
        let input_count = sum([d.provider_count, 1], "input count")?;
        let request_encoding_bytes = sum(
            [
                fixed_request,
                inputs,
                product(input_count, 41, "input framing")?,
                product(metadata, 3, "symbol wire buffers")?,
            ],
            "request encoding buffers",
        )?;
        // Field buffers are constructed BEFORE encode_request checks the total
        // request cap. Only retained, successful canonical requests may be clamped.
        let request_wire_bytes = request_encoding_bytes.min(MAX_WORKER_REQUEST_BYTES);

        // V4 is the largest response shape: magic, ten TLVs, three request/
        // envelope digests, build ID, stage, output tag/digest/u64, response digest.
        // The shared codec constant bounds diagnostics + provider + derivation.
        let response_framing = sum(
            [
                8,
                10 * 6,
                3 * 32,
                MAX_WORKER_TOOLCHAIN_ID_BYTES,
                1,
                1 + 32 + 8,
                32,
            ],
            "response framing",
        )?;
        let successful_response_bytes = sum(
            [
                response_framing,
                MAX_WORKER_RESPONSE_REPLAY_METADATA_SHELL_BYTES_V1,
                d.output,
            ],
            "successful response",
        )?
        .min(d.stdout);
        // decode_output copies and hashes its payload before decode_for_request
        // rejects a request-bound mismatch. Do not use d.output for this buffer.
        let response_decode_output_bytes = d.stdout.min(MAX_WORKER_OUTPUT_BYTES);
        let successful_output_bytes = d.output.min(response_decode_output_bytes);

        // Byte buffers only. Four payload copies cover decoded module, all_inputs,
        // candidate inputs and synthetic/replay inputs (including an extra provider
        // allowance). Two metadata images and two option-text sets cover original
        // and cloned staging. add_metadata_schedule supplies heap rows/scratch.
        let input_buffers = sum(
            [
                product(inputs, 4, "input copies")?,
                product(metadata, 2, "metadata copies")?,
                product(d.option_text, 2, "option copies")?,
            ],
            "input buffers",
        )?;
        // Candidate request + its transcript + synthetic request; encoding field
        // buffers coexist with these. encode_inputs also stages one provider.
        let preflight_storage = sum(
            [
                d.handoff,
                input_buffers,
                product(request_wire_bytes, 3, "preflight requests")?,
                request_encoding_bytes,
                d.providers,
            ],
            "preflight buffers",
        )?;
        // Both sealed requests and both transcripts remain live. For EACH worker:
        // capture and canonical response (or cloned failure capture), decoded
        // output, and original/cloned stderr. Sum both calls conservatively even
        // though some temporaries have been dropped before the second call.
        let response_buffers = sum(
            [
                capture_storage(d.stdout)?,
                response_decode_output_bytes,
                capture_storage(d.stderr)?,
            ],
            "one response buffers",
        )?;
        let execution_storage = sum(
            [
                input_buffers,
                product(request_wire_bytes, 4, "execution requests")?,
                request_encoding_bytes,
                d.providers,
                product(response_buffers, 2, "two response buffers")?,
            ],
            "execution buffers",
        )?;

        // Work units below are bytes copied or supplied to SHA-256, not cycles or
        // loop/comparison counts. Two preflight encoders and one execution encoder
        // each copy fields and final wire, stage provider bytes once more, and
        // hash the request plus its actual domain and u64 length prefix.
        let request_hash_prefix = b"FE2O3/DIRECT-LLVM-WORKER-REQUEST/V2\0".len() + 8;
        let one_request_work = sum(
            [
                product(request_encoding_bytes, 2, "request encoding copies")?,
                d.providers,
                request_wire_bytes,
                request_hash_prefix,
            ],
            "one request byte work",
        )?;
        // WorkerInputV1::new hashes twice, via calculate then from_declared.
        // Preflight creates all_inputs' module and two request modules (six
        // module hashes); execution creates only the authorized module (two).
        // A borrowed nested V2 decode may copy/hash the canonical handoff, hash
        // and UTF-8-check the module, then debug-check its identity on extraction.
        // The declared manifest hash is a separate pass. Reconstruction of the
        // nested envelope and manifest is added by nested_metadata below.
        let nested_payload_work = sum(
            [
                product(d.handoff, 2, "nested canonical copy and hash")?,
                product(d.module, 3, "nested module hash UTF-8 and debug check")?,
                d.manifest,
            ],
            "nested payload work",
        )?;
        let preflight_work = sum(
            [
                product(inputs, 4, "preflight payload copies")?,
                metadata,
                product(d.module, 6, "preflight module hashes")?,
                product(one_request_work, 2, "two request encoders")?,
                request_wire_bytes,
                nested_payload_work,
            ],
            "preflight byte work",
        )?;
        // Capture writes, possible failure clone, canonical copy and response
        // hash each visit stdout once. Output decode copies/hashes at the wire
        // limit, then executor validation rehashes accepted output. Nested
        // metadata parsing/hashing is added by response_metadata below.
        let response_hash_prefix = b"FE2O3/DIRECT-LLVM-WORKER-RESPONSE/V4\0".len() + 8;
        let one_response_work = sum(
            [
                product(d.stdout, 6, "response byte passes and capture growth")?,
                product(d.stderr, 4, "stderr byte passes and capture growth")?,
                product(
                    response_decode_output_bytes,
                    2,
                    "output decode copy and hash",
                )?,
                successful_output_bytes,
                response_hash_prefix,
            ],
            "one response byte work",
        )?;
        // Shared validate_replay_parts: two transcript hashes, one output
        // comparison and hash, one hash of all inputs, compiler payload equality,
        // stable-field comparisons (at most one request), and staged-envelope
        // hashing. Shared evidence hashing visits the four complete transcripts.
        // Plan reconstruction, symbols, request-ID and binding preimages are
        // supplied separately by add_metadata_schedule, including allocations.
        let replay_payload_work = sum(
            [
                product(request_wire_bytes, 2, "replay request hashes")?,
                product(request_hash_prefix, 2, "replay request hash prefixes")?,
                product(successful_output_bytes, 2, "replay output compare and hash")?,
                inputs,
                d.module,
                request_wire_bytes,
                d.envelope,
                b"FE2O3/STAGED-COMPILER-FFI-ENVELOPE/V1\0".len(),
                8,
                product(request_wire_bytes, 2, "evidence request hashes")?,
                product(successful_response_bytes, 2, "evidence response hashes")?,
                4 * 8,
            ],
            "shared replay and evidence payload work",
        )?;
        let execution_work = sum(
            [
                d.module,
                metadata,
                product(d.module, 2, "execution module hashes")?,
                one_request_work,
                request_wire_bytes,
                product(request_wire_bytes, 2, "two stdin payloads")?,
                product(one_response_work, 2, "two response byte passes")?,
                successful_output_bytes,
                replay_payload_work,
            ], // engine output equality
            "execution byte work",
        )?;
        let returned_buffer_storage = sum(
            [
                d.module,
                metadata,
                d.option_text,
                product(request_wire_bytes, 2, "returned transcripts")?,
                product(successful_response_bytes, 2, "returned responses")?,
                product(successful_output_bytes, 2, "returned outputs")?,
            ],
            "returned buffers",
        )?;
        let total_worker_timeout =
            d.timeout
                .checked_mul(2)
                .ok_or(NativeWorkerResourceQuoteError::Arithmetic(
                    "two worker timeouts",
                ))?;
        let mut quote = Self {
            preflight_storage,
            execution_storage,
            preflight_work,
            execution_work,
            returned_buffer_storage,
            returned_retained_storage: returned_buffer_storage,
            request_wire_bytes,
            request_encoding_bytes,
            response_capture_bytes: d.stdout,
            response_decode_output_bytes,
            successful_response_bytes,
            response_metadata_work: 0,
            response_metadata_storage: 0,
            total_worker_timeout,
        };
        quote.add_metadata_schedule(d)?;
        Ok(quote)
    }

    fn add_metadata_schedule(&mut self, d: Dimensions) -> QuoteResult<()> {
        let decode = nested_metadata(d)?;
        let request = request_metadata(d)?;
        let plan = plan_metadata(d)?;
        let response = response_metadata(d)?;
        self.response_metadata_work = response.work;
        self.response_metadata_storage = response.storage;
        let n = sum([d.provider_count, 1], "all input rows")?;
        let option_width = MAX_LINK_OPTION_NAME_BYTES + MAX_LINK_OPTION_VALUE_BYTES;
        let engine_collections = sum(
            [
                product(
                    sort_work(n, 41, size_of::<WorkerInputV1>())?,
                    2,
                    "engine input sorts",
                )?,
                sort_work(d.option_count, option_width, size_of::<LinkOptionV1>())?,
                option_work(d)?,
                product(n, 2 * 41, "engine duplicate/input-kind checks")?,
                product(d.option_count, 2 * option_width, "engine duplicate options")?,
                synthetic_probe_work(n)?,
            ],
            "engine collections",
        )?;

        // Fixed owner shells, hash states, borrowed TLV/cursor views and error
        // values. The adapter adds its own public owner shell separately. The
        // 4 * 1024 bytes are four bounded error/target formatting temporaries,
        // each at most the existing 1024-byte manifest-symbol bound.
        let frames = sum(
            [
                size_of::<ReproducibleFirstBuildEnginePreflight>(),
                size_of::<ReproducibleFirstBuildEngineResult>(),
                size_of::<DecodedCompilerModuleHandoffV2>(),
                2 * size_of::<WorkerRequestV2>(),
                2 * size_of::<WorkerResponseV2>(),
                8 * size_of::<Sha256>(),
                64 * size_of::<&[u8]>(),
                4 * MAX_COMPILER_MODULE_SYMBOL_BYTES_V1,
                2 * MAX_WORKER_TOOLCHAIN_ID_BYTES,
            ],
            "native codec frames",
        )?;
        // Two preflight requests (candidate + discarded synthetic replay), one
        // synthetic plan. Execution builds one request and one plan; replay
        // reconstructs a second plan and validates two request metadata sets.
        // Paying two complete request-metadata schedules for that borrowed
        // replay covers its three symbol decoders, unstable expected-symbol
        // sort, two request-ID hashes, and reconstructed input-kind closure.
        self.preflight_work = sum(
            [
                self.preflight_work,
                decode.work,
                engine_collections,
                product(request.work, 2, "preflight request metadata")?,
                plan.work,
                frames, // initialize/copy the bounded frames and error payloads
            ],
            "complete preflight work",
        )?;
        self.preflight_storage = sum(
            [
                self.preflight_storage,
                decode.storage,
                frames,
                product(request.storage, 2, "preflight metadata coexistence")?,
                plan.storage,
                vector_storage(n, size_of::<WorkerInputV1>())?,
            ],
            "complete preflight storage",
        )?;
        self.execution_work = sum(
            [
                self.execution_work,
                product(request.work, 3, "request and replay metadata")?,
                product(plan.work, 2, "derived and reconstructed plans")?,
                product(response.work, 2, "two response metadata decodes")?,
                plan.wire, // streamed canonical plan evidence hash
                binding_hash_work()?,
                2 * (8 + MAX_WORKER_TOOLCHAIN_ID_BYTES) + 40 + 28 + 8,
                frames,
            ],
            "complete execution work",
        )?;
        self.execution_storage = sum(
            [
                self.execution_storage,
                decode.storage,
                frames,
                product(request.storage, 3, "execution request metadata coexistence")?,
                product(plan.storage, 2, "execution plan coexistence")?,
                product(
                    response.storage,
                    2,
                    "execution response metadata coexistence",
                )?,
            ],
            "complete execution storage",
        )?;
        self.returned_retained_storage = sum(
            [
                self.returned_buffer_storage,
                decode.retained,
                frames,
                plan.retained,
                product(response.retained, 2, "returned response metadata")?,
            ],
            "returned retained storage",
        )?;
        Ok(())
    }
}

struct MetadataSchedule {
    work: usize,
    storage: usize,
    retained: usize,
    wire: usize,
}

fn nested_metadata(d: Dimensions) -> QuoteResult<MetadataSchedule> {
    // Component schedule already audited in compiler-ffi's
    // inert_semantic_compiler_module_handoff_decode_work_v4: eight complete
    // image traversals, 128 envelope visits, 320 manifest visits, and 4096 units
    // per row for the two BTreeMap searches (pinned nightly). Use the immutable
    // nested headers here, rather than estimating every count from total bytes.
    // The inherited 4 MiB fixed/error allowance covers a strict subset of that
    // decoder; its invocation/capsule variable costs are not charged here.
    let contract_pairs = product(
        d.contracts,
        d.contracts.saturating_sub(1),
        "envelope duplicate pairs",
    )?;
    let work = sum(
        [
            4 * 1024 * 1024,
            product(d.handoff, 8, "nested decoder image passes")?,
            product(d.envelope, 128, "nested envelope reconstruction")?,
            product(d.manifest, 320, "nested manifest reconstruction")?,
            product(d.symbols, 4096, "nested manifest map searches")?,
            product(
                contract_pairs,
                128 + 3 * 32 + 4,
                "nested envelope duplicate checks",
            )?,
        ],
        "nested decoder work",
    )?;
    let manifest_rows =
        vector_storage(d.symbols, size_of::<(CompilerModuleSymbolRoleV1, String)>())?;
    let contract_rows = vector_storage(d.contracts, size_of::<CompilerFfiContractV1>())?;
    let retained = sum(
        [
            // Canonical bytes plus separately retained string text; each text sum
            // is bounded by its containing validated canonical representation.
            product(d.manifest, 2, "retained manifest bytes")?,
            manifest_rows,
            product(d.envelope, 2, "retained envelope bytes")?,
            contract_rows,
            size_of::<DecodedCompilerModuleHandoffV2>(),
        ],
        "retained nested metadata",
    )?;
    let storage = sum(
        [
            retained,
            manifest_rows,
            d.manifest,
            tree_storage(d.symbols)?,
            d.envelope,
            2 * size_of::<CompilerFfiContractV1>(),
            size_of::<CompilerModuleHandoffV2>(),
            8 * size_of::<Sha256>(),
            128 * size_of::<usize>(),
        ],
        "nested decode metadata",
    )?;
    Ok(MetadataSchedule {
        work,
        storage,
        retained,
        wire: 0,
    })
}

fn request_metadata(d: Dimensions) -> QuoteResult<MetadataSchedule> {
    let n = sum([d.provider_count, 1], "request inputs")?;
    let s = d.symbols.max(d.contracts);
    let width = MAX_COMPILER_MODULE_SYMBOL_BYTES_V1;
    let text = product(s, width, "request symbol text")?;
    let symbol_rows = vector_storage(s, size_of::<String>())?;
    let ref_rows = vector_storage(s, size_of::<&str>())?;
    let input_rows = vector_storage(n, size_of::<WorkerInputV1>())?;
    let option_rows = vector_storage(d.option_count, size_of::<LinkOptionV1>())?;
    let cloned_manifest = sum(
        [
            product(d.manifest, 2, "manifest canonical and strings")?,
            vector_storage(s, size_of::<(CompilerModuleSymbolRoleV1, String)>())?,
        ],
        "cloned manifest",
    )?;
    let cloned_envelope = sum(
        [
            product(d.envelope, 2, "envelope canonical and strings")?,
            vector_storage(d.contracts, size_of::<CompilerFfiContractV1>())?,
        ],
        "cloned envelope",
    )?;
    // derive_manifest_symbol_closure scans six role views, clones at most three
    // string lists, validates them, and copies three lists into the request.
    // Both LinkSymbolClosure::new and validate_request_parts run <=3 searches
    // per symbol. Before the worker's 4096/256 limits reject, the sort can see
    // all 16384 manifest rows with 1024-byte strings.
    let searches = product(
        product(s, 6, "request symbol searches")?,
        search_steps(s),
        "request symbol search comparisons",
    )?;
    let work = sum(
        [
            sort_work(s, width, size_of::<String>())?,
            product(
                sort_work(n, 41, size_of::<WorkerInputV1>())?,
                2,
                "request input sorts",
            )?,
            product(searches, width + 1, "request symbol search bytes")?,
            // 6 clone passes + 16 text-validation/adjacency passes + 6 closure/ID
            // hash passes + 8 filtering/count/framing passes. Each list is <= s.
            product(
                product(s, 36, "request symbol passes")?,
                width + size_of::<String>() + 8,
                "request symbol visits",
            )?,
            product(option_work(d)?, 3, "request option decodes")?,
            product(
                n,
                8 * (41 + size_of::<WorkerInputV1>()),
                "request input visits",
            )?,
            cloned_manifest,
            cloned_envelope,
            d.envelope,
            binding_hash_work()?,
            8 * 32 + 3 * (8 + MAX_WORKER_TOOLCHAIN_ID_BYTES) + 15 * 6,
        ],
        "request metadata work",
    )?;
    let retained = sum(
        [
            product(
                sum([symbol_rows, text], "one symbol list")?,
                3,
                "request symbol lists",
            )?,
            input_rows,
            2 * MAX_WORKER_TOOLCHAIN_ID_BYTES,
            size_of::<WorkerRequestV2>(),
        ],
        "request retained metadata",
    )?;
    let storage = sum(
        [
            retained,
            cloned_manifest,
            cloned_envelope,
            option_rows,
            product(
                sum([symbol_rows, text], "closure list")?,
                3,
                "closure lists",
            )?,
            product(ref_rows, 2, "manifest directional views")?,
            vector_storage(n, size_of::<&WorkerInputV1>())?,
            sort_storage(s, size_of::<String>())?,
            sort_storage(n, size_of::<WorkerInputV1>())?,
        ],
        "request metadata storage",
    )?;
    Ok(MetadataSchedule {
        work,
        storage,
        retained,
        wire: 0,
    })
}

fn plan_metadata(d: Dimensions) -> QuoteResult<MetadataSchedule> {
    let n = sum([d.provider_count, 1], "plan inputs")?;
    let v = sum([n, 1], "plan nodes")?;
    // Engine plans are a star: n leaves, one output, exactly n parent edges.
    // The general 1024-node/4096-edge plan maxima are not reachable here.
    let wire = sum(
        [
            b"FE2O3/AMDGPU-MULTI-INPUT-LINK-PLAN/V1\0".len(),
            4 + MAX_WORKER_TARGET_BYTES + 4 + 4 + 40 + 4,
            product(n, 2 * 40, "plan inputs and parent identities")?,
            product(v, 40 + 4, "plan node wire")?,
            product(d.option_count, 8, "plan option lengths")?,
            d.option_text,
        ],
        "plan wire",
    )?;
    let retained = sum(
        [
            vector_storage(n, size_of::<LinkInputV1>())?,
            vector_storage(v, size_of::<ProvenanceNodeV1>())?,
            vector_storage(n, size_of::<ContentIdentityV1>())?,
            vector_storage(d.option_count, size_of::<LinkOptionV1>())?,
            d.option_text,
            size_of::<MultiInputLinkPlanV1>(),
        ],
        "plan retained metadata",
    )?;
    // Four maps: input digest lengths, provenance digest lengths, node index,
    // and DFS states. Treat each as v keys of the largest key/value shape.
    // Across validation/DFS there are <=16*v map operations: input inserts,
    // node/edge inserts, index construction/queries, DFS entry/edge/exit queries,
    // and final state checks. A search visits <=v keys; an insertion can shift
    // at most 11 key/value slots per level, with at most v+1 levels.
    let tree_step = product(
        sum([v, 1], "tree path bound")?,
        40 + 1
            + 11 * (size_of::<ContentIdentityV1>() + size_of::<usize>())
            + 4 * size_of::<usize>(),
        "tree search and shifts",
    )?;
    let tree_work = product(
        product(v, 16, "plan map operations")?,
        tree_step,
        "plan map work",
    )?;
    let work = sum(
        [
            sort_work(n, 40, size_of::<LinkInputV1>())?,
            sort_work(
                d.option_count,
                MAX_LINK_OPTION_NAME_BYTES + MAX_LINK_OPTION_VALUE_BYTES,
                size_of::<LinkOptionV1>(),
            )?,
            sort_work(n, 40, size_of::<ContentIdentityV1>())?,
            sort_work(v, 40, size_of::<ProvenanceNodeV1>())?,
            // BTreeMap::from_iter sorts the collected (identity, &node) rows too.
            sort_work(v, 40, size_of::<(ContentIdentityV1, &ProvenanceNodeV1)>())?,
            tree_work,
            product(wire, 4, "plan encode growth and hash")?,
            // Construction, clone/growth, canonical checks, DFS and equality each
            // traverse these bounded rows; 16 includes all row/payload visits.
            product(retained, 16, "plan row passes")?,
            option_work(d)?,
        ],
        "plan metadata work",
    )?;
    let storage = sum(
        [
            retained,
            product(tree_storage(v)?, 4, "four plan maps")?,
            vector_storage(v, size_of::<(ContentIdentityV1, &ProvenanceNodeV1)>())?,
            vector_storage(v, size_of::<(ContentIdentityV1, usize)>())?,
            vector_storage(n, size_of::<ContentIdentityV1>())?,
            product(wire, 2, "grown plan encoding")?,
            sort_storage(v, size_of::<ProvenanceNodeV1>())?,
            sort_storage(d.option_count, size_of::<LinkOptionV1>())?,
        ],
        "plan metadata storage",
    )?;
    Ok(MetadataSchedule {
        work,
        storage,
        retained,
        wire,
    })
}

fn response_metadata(d: Dimensions) -> QuoteResult<MetadataSchedule> {
    let n = sum([d.provider_count, 1], "response request inputs")?;
    let native = sum([MAX_LINK_INPUTS, 1], "response native input cap")?;
    // Counts are admitted/allocated BEFORE the strings are read. A truncated
    // response with a tiny stdout cap can still allocate all 4096 import rows.
    let text = sum(
        [
            product(
                MAX_WORKER_SYMBOLS,
                MAX_WORKER_SYMBOL_BYTES,
                "response import text",
            )?,
            MAX_WORKER_TOTAL_DIAGNOSTIC_BYTES,
            MAX_PROVIDER_IDENTITY_BYTES,
            product(
                MAX_PROVIDER_FILES,
                MAX_PROVIDER_BASENAME_BYTES,
                "provider file text",
            )?,
            MAX_WORKER_TOOLCHAIN_ID_BYTES,
            MAX_WORKER_TARGET_BYTES,
        ],
        "response metadata text",
    )?
    .min(d.stdout);
    let retained = sum(
        [
            text,
            vector_storage(MAX_WORKER_SYMBOLS, size_of::<String>())?,
            vector_storage(MAX_WORKER_DIAGNOSTICS, size_of::<String>())?,
            vector_storage(
                MAX_PROVIDER_FILES,
                size_of::<WorkerDeviceLibraryProviderFileEvidenceV1>(),
            )?,
            vector_storage(native, size_of::<WorkerNativeLinkInputEvidenceV1>())?,
            size_of::<WorkerResponseV2>(),
        ],
        "response retained metadata",
    )?;
    const FLAGS: [&str; 13] = [
        "ld.lld",
        "--shared",
        "-Bsymbolic",
        "--no-undefined",
        "--export-dynamic",
        "--build-id=none",
        "--nostdlib",
        "--no-dependent-libraries",
        "--fatal-warnings",
        "--threads=1",
        "--strip-debug",
        "-o",
        "@output=linked.hsaco",
    ];
    let symbols = d.symbols.min(MAX_WORKER_SYMBOLS);
    let arg_count = sum([FLAGS.len(), symbols, native], "LLD argument count")?;
    let args = sum(
        [
            sum(FLAGS.map(str::len), "LLD fixed argument text")?,
            product(
                symbols,
                b"--undefined=".len() + MAX_WORKER_SYMBOL_BYTES,
                "LLD symbol arguments",
            )?,
            // @input=, source digit, two colons, 64 hex digits and max u64 decimal.
            product(
                native,
                b"@input=".len() + 1 + 2 + 64 + 20,
                "LLD input arguments",
            )?,
        ],
        "LLD argument text",
    )?;
    let work = sum(
        [
            // UTF-8 + copy + ASCII + character validation + equal/order adjacency.
            product(text, 6, "response text passes")?,
            product(retained, 3, "response row fill growth and validation")?,
            product(
                product(
                    MAX_WORKER_SYMBOLS,
                    search_steps(MAX_WORKER_SYMBOLS),
                    "provider searches",
                )?,
                MAX_WORKER_SYMBOL_BYTES + 1,
                "provider search bytes",
            )?,
            product(
                product(
                    MAX_PROVIDER_FILES,
                    MAX_PROVIDER_FILES,
                    "provider duplicate pairs",
                )?,
                MAX_PROVIDER_BASENAME_BYTES + 1,
                "provider duplicate bytes",
            )?,
            sort_work(n, 41, size_of::<&WorkerInputV1>())?,
            product(
                native,
                4 * size_of::<WorkerNativeLinkInputEvidenceV1>(),
                "derivation row visits",
            )?,
            MAX_WORKER_RESPONSE_REPLAY_METADATA_SHELL_BYTES_V1, // provider/derivation body hashes
            // Argument formatting, growth (two passes), hashing and format scans.
            product(args, 5, "LLD text passes")?,
            product(arg_count, 4, "LLD length prefixes")?,
            product(native, 3 * 64, "LLD intermediate hex text")?,
            3 * 128 + 10 * 6, // three domain prefixes and response TLV headers
        ],
        "response metadata work",
    )?;
    let storage = sum(
        [
            retained,
            vector_storage(n, size_of::<&WorkerInputV1>())?,
            vector_storage(native, size_of::<WorkerNativeLinkInputEvidenceV1>())?,
            vector_storage(arg_count, size_of::<String>())?,
            product(args, 2, "grown LLD argument strings")?,
            product(native, 64, "LLD hex temporaries")?,
            sort_storage(n, size_of::<&WorkerInputV1>())?,
        ],
        "response metadata storage",
    )?;
    Ok(MetadataSchedule {
        work,
        storage,
        retained,
        wire: 0,
    })
}

fn option_work(d: Dimensions) -> QuoteResult<usize> {
    // Four name alternatives and at most four value alternatives, plus lexical
    // validation, equality/error copying and iteration: twelve visits per row.
    product(
        d.option_count,
        12 * (MAX_LINK_OPTION_NAME_BYTES + MAX_LINK_OPTION_VALUE_BYTES + size_of::<LinkOptionV1>()),
        "option decoding and validation",
    )
}

fn synthetic_probe_work(inputs: usize) -> QuoteResult<usize> {
    bound("synthetic input count", inputs, MAX_LINK_INPUTS)?;
    let probes = sum([inputs, 1], "synthetic candidates")?;
    sum(
        [
            b"FE2O3/FIRST-BUILD/PREFLIGHT-OUTPUT/V1\0".len(),
            product(probes, 8 + 40, "synthetic candidate writes")?,
            product(
                product(probes, inputs, "synthetic comparisons")?,
                41,
                "synthetic compare bytes",
            )?,
        ],
        "synthetic probe work",
    )
}

fn binding_hash_work() -> QuoteResult<usize> {
    // Native binding: two domains (<128 each), attempt generation/session/
    // invocation, slot+transaction, nine digest/length coordinates, invocation
    // digest, profile, six executable pins, transition version, closure digest.
    sum(
        [2 * 128, 8 + 2 * 32, 1 + 32, 9 * 40, 32, 2, 6 * 32, 8, 32],
        "native binding hash preimage",
    )
}

fn search_steps(mut count: usize) -> usize {
    let mut steps = 1;
    while count > 1 {
        count = count / 2 + count % 2;
        steps += 1;
    }
    steps
}

fn capture_storage(limit: usize) -> QuoteResult<usize> {
    // Pinned Vec<u8> growth has a minimum capacity of eight bytes. The
    // canonical response or failure clone coexists with the capture buffer.
    sum(
        [product(limit, 2, "capture capacity")?.max(8), limit],
        "capture plus copied bytes",
    )
}

fn vector_storage(count: usize, row: usize) -> QuoteResult<usize> {
    // Internal Vec collection/push growth is <= max(4, 2*len); include a
    // further four rows for empty/small temporary vectors. No caller capacity.
    product(
        sum(
            [product(count, 2, "grown row count")?, 4],
            "small row capacity",
        )?,
        row,
        "vector rows",
    )
}

fn sort_work(count: usize, key_bytes: usize, row_bytes: usize) -> QuoteResult<usize> {
    // Pinned core::slice::sort: insertion/small sorts, <=2*ilog2(n) quicksort
    // partitions with eager-drift/heapsort fallback, and <=n-1 physical merges.
    // For these Freeze rows the coarse quadratic envelope is 8*n*n comparator
    // visits and 16*n*n row moves, including pivot selection, partition copies,
    // merge copies and small-sort scratch. This is an implementation bound,
    // not a claim that the standard Rust sort API promises these constants.
    let pairs = product(count, count, "sort pairs")?;
    sum(
        [
            product(
                product(pairs, 8, "sort comparator visits")?,
                sum([key_bytes, 1], "sort comparison width")?,
                "sort comparison bytes",
            )?,
            product(
                product(pairs, 16, "sort move visits")?,
                row_bytes,
                "sort move bytes",
            )?,
        ],
        "sort work",
    )
}

fn sort_storage(count: usize, row: usize) -> QuoteResult<usize> {
    // Pinned driftsort: <=n heap rows, 4096-byte stack scratch, 66 run records
    // and depths. Reserve 32 recursion frames of 16 words (2*ilog2(16384)<=28).
    sum(
        [
            vector_storage(count, row)?,
            4096,
            66 * (2 * size_of::<usize>() + 1),
            32 * 16 * size_of::<usize>(),
        ],
        "sort scratch",
    )
}

fn tree_storage(entries: usize) -> QuoteResult<usize> {
    // Pinned alloc::collections::btree uses B=6: 11 key/value slots and 12
    // child edges. Four extra pointer words bound header/padding. At most one
    // nonempty node per key; 2*entries+1 also covers transient split/root nodes.
    let node = 11 * (size_of::<ContentIdentityV1>() + size_of::<usize>()) + 16 * size_of::<usize>();
    product(
        sum([product(entries, 2, "tree split nodes")?, 1], "tree root")?,
        node,
        "tree nodes",
    )
}

fn check_counts(providers: usize, options: usize) -> QuoteResult<()> {
    let maximum_providers =
        MAX_LINK_INPUTS
            .checked_sub(1)
            .ok_or(NativeWorkerResourceQuoteError::Arithmetic(
                "provider count cap",
            ))?;
    bound("provider count", providers, maximum_providers)?;
    bound("option count", options, MAX_LINK_OPTIONS)
}

fn bound(component: &'static str, actual: usize, maximum: usize) -> QuoteResult<()> {
    if actual > maximum {
        return Err(NativeWorkerResourceQuoteError::HardBound {
            component,
            actual,
            maximum,
        });
    }
    Ok(())
}

fn sum(values: impl IntoIterator<Item = usize>, component: &'static str) -> QuoteResult<usize> {
    values.into_iter().try_fold(0_usize, |sum, value| {
        sum.checked_add(value)
            .ok_or(NativeWorkerResourceQuoteError::Arithmetic(component))
    })
}

fn product(value: usize, copies: usize, component: &'static str) -> QuoteResult<usize> {
    value
        .checked_mul(copies)
        .ok_or(NativeWorkerResourceQuoteError::Arithmetic(component))
}

#[cfg(test)]
#[path = "first_build_worker_native_resources_tests.rs"]
mod tests;
