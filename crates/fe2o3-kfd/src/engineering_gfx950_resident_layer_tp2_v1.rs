//! Resident V5 attention prefix, ordered residual, and the closed queued MLP suffix.

use super::resident_prefix_tp2_v1 as prefix;
use super::wave_mlp_tasks_v1 as mlp_worker;
use super::wave_qkv_attention_output_tasks_v5 as v5;
use super::*;
use prefix::Transaction as _;

const NORM: &str = "ferric_qwen3_tp_batch32_wave_rmsnorm_bf16_v15";
const GEMV: &str = "ferric_qwen3_tp_wave_gemv_bf16_v3";
const SWIGLU: &str = "ferric_qwen3_tp_batch_swiglu_bf16_f32_v2";
const DOWN: &str = "ferric_qwen3_tp_wave_gemv_partial_f32_v3";
const MLP_WORKER: &str = "ferric_qwen3_claimed_mlp_bf16_f32_v1";
const WEIGHT_WORDS: usize = 25_165_824;
const PREFIX_ROOTS: usize = 14;
const FIRST_RESIDUAL: usize = 15;
const SUFFIX_BASE: usize = 16;
// Postnorm/gate/up/down weights, norm/gate/up/activation, FP32 down, final BF16.
const EXTENTS: [usize; 10] = [
    8192, 50_331_648, 50_331_648, 50_331_648, 8192, 12_288, 12_288, 12_288, 16_384, 8192,
];

#[derive(Clone, Copy)]
pub struct Gfx950EngineeringResidentLayerRankV1<'a> {
    pub prefix: Gfx950EngineeringResidentPrefixRankV1<'a>,
    pub post_norm_weight: &'a [u16; 4096],
    pub gate_weight: &'a [u16],
    pub up_weight: &'a [u16],
    pub down_weight: &'a [u16],
}

/// All arrays are staged privately until the entire two-rank group closes.
#[derive(Debug)]
pub struct Gfx950EngineeringResidentLayerObservationV1 {
    pub prefix: Gfx950EngineeringResidentPrefixObservationV1,
    pub post_norm_words: Box<[u16]>,
    pub gate_words: Box<[u16]>,
    pub up_words: Box<[u16]>,
    pub activation_words: Box<[u16]>,
    pub down_partial: Box<[f32]>,
    pub layer_output_words: Box<[u16]>,
}

#[derive(Debug)]
pub struct Gfx950EngineeringResidentLayerResultV1 {
    pub ranks: [Gfx950EngineeringResidentLayerObservationV1; 2],
    /// Host peer-round durations exclude allocation, upload and readback.
    pub producer_dispatch_ns: [u64; 2],
    pub first_residual_dispatch_ns: [u64; 2],
    pub post_norm_dispatch_ns: [u64; 2],
    pub gate_dispatch_ns: [u64; 2],
    pub up_dispatch_ns: [u64; 2],
    pub activation_dispatch_ns: [u64; 2],
    pub down_dispatch_ns: [u64; 2],
    pub final_residual_dispatch_ns: [u64; 2],
}

/// Distinct finite-worker route; no per-task timing is inferred from its dispatch.
#[derive(Debug)]
pub struct Gfx950EngineeringResidentLayerMlpWorkerResultV1 {
    pub ranks: [Gfx950EngineeringResidentLayerObservationV1; 2],
    pub mlp_final_states: [[u32; 11]; 2],
    pub producer_dispatch_ns: [u64; 2],
    pub first_residual_dispatch_ns: [u64; 2],
    /// Two measured peer dispatch durations, one finite MLP worker per rank.
    pub mlp_dispatch_ns: [u64; 2],
    pub final_residual_dispatch_ns: [u64; 2],
}

enum SuffixCompletion {
    Queued([[u64; 2]; 5]),
    Worker {
        dispatch: [u64; 2],
        states: [[u32; 11]; 2],
    },
}

// The routes share allocation, predecessor preservation and teardown. Conversion
// to the distinct public result happens before close, so no output escapes an error.
trait Route {
    const FINITE_WORKER: bool;
    type Output;
    fn result(
        ranks: [Gfx950EngineeringResidentLayerObservationV1; 2],
        producer: [u64; 2],
        first: [u64; 2],
        last: [u64; 2],
        suffix: SuffixCompletion,
    ) -> Result<Self::Output>;
}
struct Queued;
struct FiniteWorker;
impl Route for Queued {
    const FINITE_WORKER: bool = false;
    type Output = Gfx950EngineeringResidentLayerResultV1;
    fn result(
        ranks: [Gfx950EngineeringResidentLayerObservationV1; 2],
        producer: [u64; 2],
        first: [u64; 2],
        last: [u64; 2],
        suffix: SuffixCompletion,
    ) -> Result<Self::Output> {
        let SuffixCompletion::Queued(timings) = suffix else {
            return Err("resident layer queued route/result mismatch".into());
        };
        Ok(Gfx950EngineeringResidentLayerResultV1 {
            ranks,
            producer_dispatch_ns: producer,
            first_residual_dispatch_ns: first,
            post_norm_dispatch_ns: timings[0],
            gate_dispatch_ns: timings[1],
            up_dispatch_ns: timings[2],
            activation_dispatch_ns: timings[3],
            down_dispatch_ns: timings[4],
            final_residual_dispatch_ns: last,
        })
    }
}
impl Route for FiniteWorker {
    const FINITE_WORKER: bool = true;
    type Output = Gfx950EngineeringResidentLayerMlpWorkerResultV1;
    fn result(
        ranks: [Gfx950EngineeringResidentLayerObservationV1; 2],
        producer: [u64; 2],
        first: [u64; 2],
        last: [u64; 2],
        suffix: SuffixCompletion,
    ) -> Result<Self::Output> {
        let SuffixCompletion::Worker { dispatch, states } = suffix else {
            return Err("resident layer worker route/result mismatch".into());
        };
        Ok(Gfx950EngineeringResidentLayerMlpWorkerResultV1 {
            ranks,
            mlp_final_states: states,
            producer_dispatch_ns: producer,
            first_residual_dispatch_ns: first,
            mlp_dispatch_ns: dispatch,
            final_residual_dispatch_ns: last,
        })
    }
}

/// Execute the closed first-layer schedule without exporting or reuploading intermediates.
/// Numerical/model qualification is the caller's separate, fail-closed responsibility.
///
/// # Safety
/// Use a dedicated disposable process, exact authorized devices and independently
/// reviewed images, source/ISA, float modes, Exp provider, System atomics, peer
/// visibility and completion ordering. Shared-host idle observations are not a
/// reservation or a fairness guarantee. This process must exclusively own its
/// queues, mappings and allocations. Every error is terminal: exit without
/// retry or reuse. No overlapping execution or performance claim is implied.
#[allow(clippy::too_many_arguments)]
pub unsafe fn execute_gfx950_engineering_resident_layer_tp2_unchecked_v1(
    ids: [u64; 2],
    producer_object: Vec<u8>,
    producer_digest: [u8; 32],
    consumer_object: Vec<u8>,
    consumer_digest: [u8; 32],
    mlp_object: Vec<u8>,
    mlp_digest: [u8; 32],
    norm_object: Vec<u8>,
    norm_digest: [u8; 32],
    ranks: [Gfx950EngineeringResidentLayerRankV1<'_>; 2],
    timeout_ms: u32,
) -> Result<Gfx950EngineeringResidentLayerResultV1> {
    // SAFETY: forwards the unchanged queued-route contract.
    unsafe {
        execute_route::<Queued>(
            ids,
            producer_object,
            producer_digest,
            consumer_object,
            consumer_digest,
            mlp_object,
            mlp_digest,
            norm_object,
            norm_digest,
            ranks,
            timeout_ms,
        )
    }
}

/// Compose the unchanged attention prefix with one eleven-root MLP dispatch.
/// The actual first-residual allocation feeds Norm and the final residual add;
/// original FP32 down allocations remain peer-readable until both consumers close.
/// Numerical qualification and finite polling progress are not established here.
///
/// # Safety
/// The dedicated-process, reviewed-image, ownership, coherence, peer visibility,
/// completion and terminal-error obligations of the queued resident API apply.
/// Additionally review the distinct MLP worker's System atomics, five-task graph,
/// eleven-root ABI, two Wave64 workgroups and 512-byte LDS. No exclusive device
/// reservation or scheduling-fairness assumption is made.
#[allow(clippy::too_many_arguments)]
pub unsafe fn execute_gfx950_engineering_resident_layer_mlp_worker_tp2_unchecked_v1(
    ids: [u64; 2],
    producer_object: Vec<u8>,
    producer_digest: [u8; 32],
    consumer_object: Vec<u8>,
    consumer_digest: [u8; 32],
    mlp_worker_object: Vec<u8>,
    mlp_worker_digest: [u8; 32],
    ranks: [Gfx950EngineeringResidentLayerRankV1<'_>; 2],
    timeout_ms: u32,
) -> Result<Gfx950EngineeringResidentLayerMlpWorkerResultV1> {
    // SAFETY: only the separately reviewed finite-worker route is selected.
    unsafe {
        execute_route::<FiniteWorker>(
            ids,
            producer_object,
            producer_digest,
            consumer_object,
            consumer_digest,
            mlp_worker_object,
            mlp_worker_digest,
            Vec::new(),
            [0; 32],
            ranks,
            timeout_ms,
        )
    }
}

#[allow(clippy::too_many_arguments)]
unsafe fn execute_route<R: Route>(
    ids: [u64; 2],
    producer_object: Vec<u8>,
    producer_digest: [u8; 32],
    consumer_object: Vec<u8>,
    consumer_digest: [u8; 32],
    mlp_object: Vec<u8>,
    mlp_digest: [u8; 32],
    norm_object: Vec<u8>,
    norm_digest: [u8; 32],
    ranks: [Gfx950EngineeringResidentLayerRankV1<'_>; 2],
    timeout_ms: u32,
) -> Result<R::Output> {
    prefix::validate_request(
        ids,
        &producer_object,
        producer_digest,
        &consumer_object,
        consumer_digest,
        timeout_ms,
    )?;
    if R::FINITE_WORKER {
        mlp_worker::validate_request(&mlp_object, mlp_digest, MLP_WORKER, timeout_ms)?;
    } else {
        for entry in [GEMV, SWIGLU, DOWN] {
            v5::validate_request(&mlp_object, mlp_digest, entry, timeout_ms)?;
        }
        v5::validate_request(&norm_object, norm_digest, NORM, timeout_ms)?;
    }
    if ranks.iter().any(|rank| {
        [
            rank.gate_weight.len(),
            rank.up_weight.len(),
            rank.down_weight.len(),
        ] != [WEIGHT_WORDS; 3]
    }) || ranks[0].post_norm_weight != ranks[1].post_norm_weight
    {
        return Err("resident layer MLP weight extents/shared norm identity".into());
    }
    if ranks[0].prefix.input != ranks[1].prefix.input
        || ranks[0].prefix.cache_metadata != ranks[1].prefix.cache_metadata
        || ranks.iter().any(|rank| {
            rank.prefix.key_cache.len() != v5::CACHE_WORDS
                || rank.prefix.value_cache.len() != v5::CACHE_WORDS
        })
    {
        return Err("resident layer prefix input/metadata/cache roster".into());
    }
    let inputs = ranks.map(|rank| {
        [
            v5::encode_words(rank.prefix.input),
            v5::encode_words(rank.prefix.norm_weight),
            v5::encode_words(rank.prefix.qkv_weight),
            v5::encode_words(rank.prefix.head_norm_weight),
            v5::encode_f32(rank.prefix.rotary),
            rank.prefix
                .cache_metadata
                .iter()
                .flat_map(|value| value.to_le_bytes())
                .collect(),
            v5::encode_words(rank.prefix.output_weight),
        ]
    });
    let caches = ranks.map(|rank| {
        [
            v5::encode_words(rank.prefix.key_cache),
            v5::encode_words(rank.prefix.value_cache),
        ]
    });
    let weights = ranks.map(|rank| {
        [
            v5::encode_words(rank.post_norm_weight),
            v5::encode_words(rank.gate_weight),
            v5::encode_words(rank.up_weight),
            v5::encode_words(rank.down_weight),
        ]
    });
    let inputs = inputs
        .each_ref()
        .map(|row| row.each_ref().map(Vec::as_slice));
    let caches = caches
        .each_ref()
        .map(|row| row.each_ref().map(Vec::as_slice));
    let weights = weights
        .each_ref()
        .map(|row| row.each_ref().map(Vec::as_slice));
    prefix::validate_inputs(&inputs, &caches)?;
    validate_weights(&weights)?;
    // SAFETY: caller provides the complete dedicated-process engineering contract.
    let group = unsafe { Gfx950EngineeringPeerGroupV1::open_unchecked(&ids)? };
    let native = Native {
        prefix: prefix::Native::from_group(group),
        kernels: core::array::from_fn(|_| core::array::from_fn(|_| None)),
        mlp_kernels: core::array::from_fn(|_| None),
    };
    // SAFETY: only the fixed rosters and geometry below are dispatched.
    unsafe {
        coordinate_route::<_, R>(
            native,
            producer_object,
            producer_digest,
            consumer_object,
            consumer_digest,
            mlp_object,
            mlp_digest,
            norm_object,
            norm_digest,
            inputs,
            caches,
            weights,
            timeout_ms,
        )
    }
}

fn validate_weights(weights: &[[&[u8]; 4]; 2]) -> Result<()> {
    if weights[0][0] != weights[1][0]
        || weights.iter().any(|row| {
            row.iter()
                .enumerate()
                .any(|(role, bytes)| bytes.len() != EXTENTS[role])
        })
    {
        return Err("resident layer immutable MLP weight roster".into());
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Kind {
    Norm,
    Gate,
    Up,
    Activation,
    Down,
}
const STAGES: [Kind; 5] = [
    Kind::Norm,
    Kind::Gate,
    Kind::Up,
    Kind::Activation,
    Kind::Down,
];
impl Kind {
    fn entry(self) -> &'static str {
        match self {
            Self::Norm => NORM,
            Self::Gate | Self::Up => GEMV,
            Self::Activation => SWIGLU,
            Self::Down => DOWN,
        }
    }
    fn kernel_slot(self) -> usize {
        match self {
            Self::Norm => 0,
            Self::Gate | Self::Up => 1,
            Self::Activation => 2,
            Self::Down => 3,
        }
    }
    fn output(self) -> usize {
        match self {
            Self::Norm => 4,
            Self::Gate => 5,
            Self::Up => 6,
            Self::Activation => 7,
            Self::Down => 8,
        }
    }
    fn total(self) -> usize {
        match self {
            Self::Norm => 352,
            Self::Activation => 312,
            _ => 328,
        }
    }
    fn hidden(self) -> u32 {
        match self {
            Self::Norm => 96,
            Self::Activation => 56,
            _ => 72,
        }
    }
    fn slices(self) -> usize {
        if self == Self::Norm { 5 } else { 3 }
    }
    fn scalars(self) -> usize {
        match self {
            Self::Norm => 4,
            Self::Activation => 2,
            _ => 5,
        }
    }
    fn grid(self) -> [u32; 3] {
        [
            match self {
                Self::Norm => 64,
                Self::Gate | Self::Up => 393216,
                Self::Activation => 6144,
                Self::Down => 262144,
            },
            1,
            1,
        ]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Binding {
    rank: usize,
    role: usize,
    offset: u32,
    extent: u64,
    access: BufferAccessV1,
}
fn binding(rank: usize, role: usize, slot: usize, extent: usize, write: bool) -> Binding {
    Binding {
        rank,
        role,
        offset: slot as u32 * 16,
        extent: extent as u64,
        access: if write {
            BufferAccessV1::Write
        } else {
            BufferAccessV1::Read
        },
    }
}

fn bindings(kind: Kind, rank: usize) -> Vec<Binding> {
    match kind {
        Kind::Norm => vec![
            binding(rank, 15, 0, 8192, false),
            binding(rank, 15, 1, 0, false),
            binding(rank, 16, 2, 8192, false),
            binding(rank, 20, 3, 0, true),
            binding(rank, 20, 4, 8192, true),
        ],
        Kind::Gate | Kind::Up => vec![
            binding(rank, 20, 0, 8192, false),
            binding(
                rank,
                if kind == Kind::Gate { 17 } else { 18 },
                1,
                50_331_648,
                false,
            ),
            binding(
                rank,
                if kind == Kind::Gate { 21 } else { 22 },
                2,
                12_288,
                true,
            ),
        ],
        Kind::Activation => vec![
            binding(rank, 21, 0, 12_288, false),
            binding(rank, 22, 1, 12_288, false),
            binding(rank, 23, 2, 12_288, true),
        ],
        Kind::Down => vec![
            binding(rank, 23, 0, 12_288, false),
            binding(rank, 19, 1, 50_331_648, false),
            binding(rank, 24, 2, 16_384, true),
        ],
    }
}

fn final_bindings(rank: usize) -> [Binding; 10] {
    core::array::from_fn(|slot| match slot {
        0 | 1 => binding(slot, 24, slot, 16384, false),
        2..=7 => binding(0, 24, slot, 0, false),
        8 => binding(rank, 15, slot, 8192, false),
        _ => binding(rank, 25, slot, 8192, true),
    })
}

fn mlp_bindings(rank: usize) -> [Binding; 10] {
    core::array::from_fn(|index| {
        let role = if index == 0 {
            FIRST_RESIDUAL
        } else {
            SUFFIX_BASE + index - 1
        };
        Binding {
            rank,
            role,
            offset: index as u32 * 8,
            extent: if index == 0 {
                8192
            } else {
                EXTENTS[index - 1] as u64
            },
            access: mlp_worker::role_access(index),
        }
    })
}

fn stage_bytes(kind: Kind) -> Vec<u8> {
    let mut bytes = vec![0; kind.total()];
    for (slot, pointer) in bindings(kind, 0).iter().enumerate() {
        let width = if kind == Kind::Down && slot == 2 {
            4
        } else {
            2
        };
        bytes[slot * 16 + 8..slot * 16 + 16]
            .copy_from_slice(&(pointer.extent / width).to_le_bytes());
    }
    let values: &[u32] = match kind {
        Kind::Norm => &[1, 4096, 0x358637bd, 0], // epsilon = the retained source's f32 1e-6.
        Kind::Gate => &[1, 6144, 4096, 2, 4],
        Kind::Up => &[1, 6144, 4096, 2, 5],
        Kind::Activation => &[1, 2],
        Kind::Down => &[1, 4096, 6144, 2, 2],
    };
    let start = kind.slices() * 16;
    for (index, value) in values.iter().enumerate() {
        bytes[start + index * 4..start + index * 4 + 4].copy_from_slice(&value.to_le_bytes());
    }
    bytes
}

fn validate_metadata(metadata: &KernelMetadataV1, digest: [u8; 32], kind: Kind) -> Result<()> {
    if metadata.object_sha256 != digest
        || metadata.symbol != kind.entry()
        || metadata.kernarg_bytes != kind.total() as u32
        || metadata.kernarg_alignment != 8
        || metadata.wavefront_size != 64
        || metadata.group_segment_bytes != 0
        || metadata.private_segment_bytes != 0
        || metadata.implicit_argument_offset != Some(kind.hidden())
        || metadata.implicit_argument_bytes != 256
        || metadata.explicit_arguments.len() != kind.slices() * 2 + kind.scalars()
    {
        return Err("resident layer stage physical ABI/resources".into());
    }
    let pointers = bindings(kind, 0);
    for (index, argument) in metadata.explicit_arguments.iter().enumerate() {
        let pair = index < kind.slices() * 2;
        let pointer = pair && index % 2 == 0;
        let offset = if pair {
            index as u32 * 8
        } else {
            kind.slices() as u32 * 16 + (index - kind.slices() * 2) as u32 * 4
        };
        let alignment = if kind == Kind::Down && index == 4 {
            4
        } else {
            2
        };
        if argument.offset != offset
            || argument.bytes != if pair { 8 } else { 4 }
            || argument.global_buffer != pointer
            || (pointer
                && (argument
                    .pointee_alignment
                    .is_some_and(|value| value != alignment)
                    || argument
                        .access
                        .is_some_and(|value| value != pointers[index / 2].access)))
            || (!pointer && (argument.pointee_alignment.is_some() || argument.access.is_some()))
        {
            return Err("resident layer pointer/length/scalar roster".into());
        }
    }
    Ok(())
}

trait Transaction: prefix::Transaction {
    type MlpState;
    fn load_mlp_worker(
        &mut self,
        rank: usize,
        object: Vec<u8>,
        digest: [u8; 32],
    ) -> Result<KernelMetadataV1>;
    fn mlp_state(&mut self, rank: usize) -> Result<Self::MlpState>;
    fn observe_mlp(&mut self, state: &Self::MlpState) -> Result<[u32; 11]>;
    unsafe fn mlp_round(
        &mut self,
        residual: &[Self::Buffer; 2],
        suffix: &[[Self::Buffer; 10]; 2],
        states: &[Self::MlpState; 2],
        timeout: u32,
    ) -> Result<[u64; 2]>;
    fn load_stage(
        &mut self,
        rank: usize,
        kind: Kind,
        object: Vec<u8>,
        digest: [u8; 32],
    ) -> Result<KernelMetadataV1>;
    unsafe fn stage_round(
        &mut self,
        kind: Kind,
        residual: &[Self::Buffer; 2],
        suffix: &[[Self::Buffer; 10]; 2],
        timeout: u32,
    ) -> Result<[u64; 2]>;
    unsafe fn final_round(
        &mut self,
        residual: &[Self::Buffer; 2],
        suffix: &[[Self::Buffer; 10]; 2],
        timeout: u32,
    ) -> Result<[u64; 2]>;
}

fn output<T: Transaction>(tx: &mut T, buffer: T::Buffer, role: usize) -> Result<Vec<u8>> {
    let bytes = prefix::read(tx, buffer, EXTENTS[role])?;
    if role == 8 {
        if v5::decode_f32(&bytes, 4096)?
            .iter()
            .any(|value| !value.is_finite())
        {
            return Err("resident layer nonfinite down partial".into());
        }
    } else {
        prefix::finite_bf16(&bytes)?;
    }
    Ok(bytes)
}

#[allow(clippy::too_many_arguments)]
fn preserved<T: Transaction>(
    tx: &mut T,
    roots: &[[T::Buffer; PREFIX_ROOTS]; 2],
    states: &[T::State; 2],
    staged: &[prefix::Stage; 2],
    inputs: &[[&[u8]; 7]; 2],
    residual: &[T::Buffer; 2],
    first: &[Vec<u8>; 2],
    suffix: &[[T::Buffer; 10]; 2],
    weights: &[[&[u8]; 4]; 2],
    produced: &[[Vec<u8>; 2]],
) -> Result<()> {
    for rank in 0..2 {
        if tx.observe(&states[rank])? != staged[rank].state {
            return Err("resident layer changed producer state".into());
        }
        for role in 0..7 {
            prefix::unchanged(tx, roots[rank][role], inputs[rank][role])?;
        }
        for role in 7..14 {
            prefix::unchanged(tx, roots[rank][role], &staged[rank].outputs[role - 7])?;
        }
        prefix::unchanged(tx, residual[rank], &first[rank])?;
        for role in 0..4 {
            prefix::unchanged(tx, suffix[rank][role], weights[rank][role])?;
        }
        for (stage, values) in produced.iter().enumerate() {
            prefix::unchanged(tx, suffix[rank][stage + 4], &values[rank])?;
        }
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::too_many_arguments)]
unsafe fn coordinate<T: Transaction>(
    tx: T,
    producer: Vec<u8>,
    pd: [u8; 32],
    consumer: Vec<u8>,
    cd: [u8; 32],
    mlp: Vec<u8>,
    md: [u8; 32],
    norm: Vec<u8>,
    nd: [u8; 32],
    inputs: [[&[u8]; 7]; 2],
    caches: [[&[u8]; 2]; 2],
    weights: [[&[u8]; 4]; 2],
    timeout: u32,
) -> Result<Gfx950EngineeringResidentLayerResultV1> {
    // SAFETY: same fixed queued dispatch and predecessor contract.
    unsafe {
        coordinate_route::<T, Queued>(
            tx, producer, pd, consumer, cd, mlp, md, norm, nd, inputs, caches, weights, timeout,
        )
    }
}

#[allow(clippy::too_many_arguments)]
unsafe fn coordinate_route<T: Transaction, R: Route>(
    mut tx: T,
    producer: Vec<u8>,
    pd: [u8; 32],
    consumer: Vec<u8>,
    cd: [u8; 32],
    mlp: Vec<u8>,
    md: [u8; 32],
    norm: Vec<u8>,
    nd: [u8; 32],
    inputs: [[&[u8]; 7]; 2],
    caches: [[&[u8]; 2]; 2],
    weights: [[&[u8]; 4]; 2],
    timeout: u32,
) -> Result<R::Output> {
    let result = (|| {
        let slots = prefix::validate_inputs(&inputs, &caches)?;
        validate_weights(&weights)?;
        for rank in 0..2 {
            v5::validate_metadata(
                &tx.load(rank, true, producer.clone(), pd)?,
                pd,
                "ferric_qwen3_claimed_rmsnorm_qkv_attention_output_bf16_f32_v5",
            )?;
            prefix::validate_consumer(&tx.load(rank, false, consumer.clone(), cd)?, cd)?;
            if R::FINITE_WORKER {
                mlp_worker::validate_metadata(
                    &tx.load_mlp_worker(rank, mlp.clone(), md)?,
                    md,
                    MLP_WORKER,
                )?;
            } else {
                for kind in [Kind::Norm, Kind::Gate, Kind::Activation, Kind::Down] {
                    let (image, digest) = if kind == Kind::Norm {
                        (&norm, nd)
                    } else {
                        (&mlp, md)
                    };
                    validate_metadata(
                        &tx.load_stage(rank, kind, image.clone(), digest)?,
                        digest,
                        kind,
                    )?;
                }
            }
        }
        let mut unique = Vec::with_capacity(50);
        let mut root_rows = Vec::with_capacity(2);
        let mut state_rows = Vec::with_capacity(2);
        let mut first_buffers = Vec::with_capacity(2);
        let mut suffix_rows = Vec::with_capacity(2);
        let mut mlp_state_rows = Vec::with_capacity(2);
        for rank in 0..2 {
            let mut roots = Vec::with_capacity(PREFIX_ROOTS);
            for role in 0..PREFIX_ROOTS {
                let buffer = tx.allocate(rank, role, v5::EXTENTS[role], role == 13)?;
                if unique.contains(&buffer) {
                    return Err("resident layer allocation token alias".into());
                }
                unique.push(buffer);
                if role < 7 {
                    prefix::upload(&mut tx, buffer, inputs[rank][role])?;
                } else if role == 10 || role == 11 {
                    prefix::upload(&mut tx, buffer, caches[rank][role - 10])?;
                } else {
                    let poison = if role == 13 {
                        0x7fc00000u32.to_le_bytes().repeat(4096)
                    } else {
                        0x7fc0u16.to_le_bytes().repeat(v5::EXTENTS[role] / 2)
                    };
                    prefix::upload(&mut tx, buffer, &poison)?;
                }
                roots.push(buffer);
            }
            root_rows.push(
                roots
                    .try_into()
                    .map_err(|_| "resident layer prefix roster")?,
            );
            let state = tx.state(rank)?;
            if tx.observe(&state)? != core::array::from_fn(|index| u32::from(index < 2)) {
                return Err("resident layer state not fresh".into());
            }
            state_rows.push(state);
            if R::FINITE_WORKER {
                let state = tx.mlp_state(rank)?;
                if tx.observe_mlp(&state)? != [1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0] {
                    return Err("resident layer MLP state not fresh".into());
                }
                mlp_state_rows.push(state);
            }
            let residual = tx.allocate(rank, FIRST_RESIDUAL, 8192, false)?;
            if unique.contains(&residual) {
                return Err("resident layer residual alias".into());
            }
            unique.push(residual);
            prefix::upload(&mut tx, residual, &0x7fc0u16.to_le_bytes().repeat(4096))?;
            first_buffers.push(residual);
            let mut suffix = Vec::with_capacity(10);
            for (role, extent) in EXTENTS.iter().copied().enumerate() {
                let buffer = tx.allocate(rank, role + SUFFIX_BASE, extent, role == 8)?;
                if unique.contains(&buffer) {
                    return Err("resident layer suffix alias".into());
                }
                unique.push(buffer);
                if role < 4 {
                    prefix::upload(&mut tx, buffer, weights[rank][role])?;
                } else {
                    let poison = if role == 8 {
                        0x7fc00000u32.to_le_bytes().repeat(4096)
                    } else {
                        0x7fc0u16.to_le_bytes().repeat(extent / 2)
                    };
                    prefix::upload(&mut tx, buffer, &poison)?;
                }
                suffix.push(buffer);
            }
            suffix_rows.push(
                suffix
                    .try_into()
                    .map_err(|_| "resident layer suffix roster")?,
            );
        }
        let roots: [[T::Buffer; PREFIX_ROOTS]; 2] = root_rows
            .try_into()
            .map_err(|_| "resident layer rank roster")?;
        let states: [T::State; 2] = state_rows.try_into().map_err(|_| "resident layer states")?;
        let residual: [T::Buffer; 2] = first_buffers
            .try_into()
            .map_err(|_| "resident layer residuals")?;
        let suffix: [[T::Buffer; 10]; 2] = suffix_rows
            .try_into()
            .map_err(|_| "resident layer suffix ranks")?;
        let mlp_states: Option<[T::MlpState; 2]> = if R::FINITE_WORKER {
            Some(
                mlp_state_rows
                    .try_into()
                    .map_err(|_| "resident layer MLP states")?,
            )
        } else {
            None
        };
        // SAFETY: initialized independent rank roots, followed by a joined completed round.
        let producer_dispatch_ns = unsafe { tx.producers(&roots, &states, timeout)? };
        let staged = [
            prefix::stage(
                &mut tx, &roots[0], &states[0], &inputs[0], &caches[0], slots[0],
            )?,
            prefix::stage(
                &mut tx, &roots[1], &states[1], &inputs[1], &caches[1], slots[1],
            )?,
        ];
        // SAFETY: both original attention partials completed and are retained read-only.
        let first_residual_dispatch_ns = unsafe { tx.consumers(&roots, &residual, timeout)? };
        let first = [
            prefix::read(&mut tx, residual[0], 8192)?,
            prefix::read(&mut tx, residual[1], 8192)?,
        ];
        for value in &first {
            prefix::finite_bf16(value)?;
        }
        if first[0] != first[1] {
            return Err("resident layer first residual replicas differ".into());
        }
        let mut produced: Vec<[Vec<u8>; 2]> = Vec::with_capacity(5);
        let mut timings = Vec::with_capacity(5);
        let mut worker_completion = None;
        preserved(
            &mut tx, &roots, &states, &staged, &inputs, &residual, &first, &suffix, &weights,
            &produced,
        )?;
        if let Some(mlp_states) = &mlp_states {
            for state in mlp_states {
                if tx.observe_mlp(state)? != [1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0] {
                    return Err("resident layer MLP state changed before dispatch".into());
                }
            }
            // SAFETY: rank-private predecessors and fresh owner-only typed state;
            // every output allocation remains live through the final consumers.
            let dispatch = unsafe { tx.mlp_round(&residual, &suffix, mlp_states, timeout)? };
            let completed = [
                tx.observe_mlp(&mlp_states[0])?,
                tx.observe_mlp(&mlp_states[1])?,
            ];
            for state in completed {
                mlp_worker::validate_final_state(state)?;
            }
            for kind in STAGES {
                let values = [
                    output(&mut tx, suffix[0][kind.output()], kind.output())?,
                    output(&mut tx, suffix[1][kind.output()], kind.output())?,
                ];
                if kind == Kind::Norm && values[0] != values[1] {
                    return Err("resident layer norm replicas differ".into());
                }
                produced.push(values);
            }
            preserved(
                &mut tx, &roots, &states, &staged, &inputs, &residual, &first, &suffix, &weights,
                &produced,
            )?;
            if [
                tx.observe_mlp(&mlp_states[0])?,
                tx.observe_mlp(&mlp_states[1])?,
            ] != completed
            {
                return Err("resident layer MLP state changed after readback".into());
            }
            worker_completion = Some((dispatch, completed));
        } else {
            for kind in STAGES {
                // SAFETY: the closed table reads only completed resident predecessors.
                timings.push(unsafe { tx.stage_round(kind, &residual, &suffix, timeout)? });
                let values = [
                    output(&mut tx, suffix[0][kind.output()], kind.output())?,
                    output(&mut tx, suffix[1][kind.output()], kind.output())?,
                ];
                if kind == Kind::Norm && values[0] != values[1] {
                    return Err("resident layer norm replicas differ".into());
                }
                // Recheck every prior input before accepting the newly completed stage.
                preserved(
                    &mut tx, &roots, &states, &staged, &inputs, &residual, &first, &suffix,
                    &weights, &produced,
                )?;
                produced.push(values);
            }
        }
        // SAFETY: both original FP32 down allocations have completed; never reuploaded.
        let final_residual_dispatch_ns = unsafe { tx.final_round(&residual, &suffix, timeout)? };
        let final_words = [
            output(&mut tx, suffix[0][9], 9)?,
            output(&mut tx, suffix[1][9], 9)?,
        ];
        if final_words[0] != final_words[1] {
            return Err("resident layer final replicas differ".into());
        }
        preserved(
            &mut tx, &roots, &states, &staged, &inputs, &residual, &first, &suffix, &weights,
            &produced,
        )?;
        if let (Some(tokens), Some((_, completed))) = (&mlp_states, &worker_completion) {
            if [tx.observe_mlp(&tokens[0])?, tx.observe_mlp(&tokens[1])?] != *completed {
                return Err("resident layer MLP state changed after final consumers".into());
            }
        }
        let mut observations = Vec::with_capacity(2);
        for rank in 0..2 {
            let o = &staged[rank].outputs;
            observations.push(Gfx950EngineeringResidentLayerObservationV1 {
                prefix: Gfx950EngineeringResidentPrefixObservationV1 {
                    normalized_words: v5::decode_words(&o[0], 4096)?,
                    qkv_words: v5::decode_words(&o[1], 3072)?,
                    query_words: v5::decode_words(&o[2], 2048)?,
                    key_cache_words: v5::decode_words(&o[3], v5::CACHE_WORDS)?,
                    value_cache_words: v5::decode_words(&o[4], v5::CACHE_WORDS)?,
                    attention_words: v5::decode_words(&o[5], 2048)?,
                    output_partial: v5::decode_f32(&o[6], 4096)?,
                    residual_words: v5::decode_words(&first[rank], 4096)?,
                    final_state: staged[rank].state,
                },
                post_norm_words: v5::decode_words(&produced[0][rank], 4096)?,
                gate_words: v5::decode_words(&produced[1][rank], 6144)?,
                up_words: v5::decode_words(&produced[2][rank], 6144)?,
                activation_words: v5::decode_words(&produced[3][rank], 6144)?,
                down_partial: v5::decode_f32(&produced[4][rank], 4096)?,
                layer_output_words: v5::decode_words(&final_words[rank], 4096)?,
            });
        }
        let ranks = observations
            .try_into()
            .map_err(|_| "resident layer output roster")?;
        let suffix = if let Some((dispatch, states)) = worker_completion {
            SuffixCompletion::Worker { dispatch, states }
        } else {
            SuffixCompletion::Queued(
                timings
                    .try_into()
                    .map_err(|_| "resident layer timing roster")?,
            )
        };
        let result = R::result(
            ranks,
            producer_dispatch_ns,
            first_residual_dispatch_ns,
            final_residual_dispatch_ns,
            suffix,
        )?;
        tx.close()?;
        Ok(result)
    })();
    if result.is_err() {
        tx.quarantine();
    }
    result
}

struct Native {
    prefix: prefix::Native,
    kernels: [[Option<Gfx950EngineeringPeerKernelV1>; 4]; 2],
    mlp_kernels: [Option<Gfx950EngineeringPeerKernelV1>; 2],
}
impl prefix::Transaction for Native {
    type Buffer = Gfx950EngineeringPeerBufferV1;
    type State = Gfx950EngineeringPeerWaveOutputStateV5;
    fn load(
        &mut self,
        rank: usize,
        producer: bool,
        object: Vec<u8>,
        digest: [u8; 32],
    ) -> Result<KernelMetadataV1> {
        self.prefix.load(rank, producer, object, digest)
    }
    fn allocate(
        &mut self,
        rank: usize,
        role: usize,
        bytes: usize,
        peer: bool,
    ) -> Result<Self::Buffer> {
        self.prefix.allocate(rank, role, bytes, peer)
    }
    fn state(&mut self, rank: usize) -> Result<Self::State> {
        self.prefix.state(rank)
    }
    fn observe(&mut self, state: &Self::State) -> Result<[u32; 22]> {
        self.prefix.observe(state)
    }
    fn write(&mut self, buffer: Self::Buffer, offset: usize, bytes: &[u8]) -> Result<()> {
        self.prefix.write(buffer, offset, bytes)
    }
    fn read(&mut self, buffer: Self::Buffer, offset: usize, bytes: usize) -> Result<Vec<u8>> {
        self.prefix.read(buffer, offset, bytes)
    }
    unsafe fn producers(
        &mut self,
        roots: &[[Self::Buffer; PREFIX_ROOTS]; 2],
        states: &[Self::State; 2],
        timeout: u32,
    ) -> Result<[u64; 2]> {
        // SAFETY: inherited fixed independent producer contract.
        unsafe { self.prefix.producers(roots, states, timeout) }
    }
    unsafe fn consumers(
        &mut self,
        roots: &[[Self::Buffer; PREFIX_ROOTS]; 2],
        outputs: &[Self::Buffer; 2],
        timeout: u32,
    ) -> Result<[u64; 2]> {
        // SAFETY: inherited completed original attention partial contract.
        unsafe { self.prefix.consumers(roots, outputs, timeout) }
    }
    fn close(&mut self) -> Result<()> {
        self.prefix.close()
    }
    fn quarantine(self) {
        core::mem::forget(self);
    }
}

fn resolve_buffer(
    binding: Binding,
    residual: &[Gfx950EngineeringPeerBufferV1; 2],
    suffix: &[[Gfx950EngineeringPeerBufferV1; 10]; 2],
) -> Gfx950EngineeringPeerPointerV1 {
    let buffer = if binding.role == FIRST_RESIDUAL {
        residual[binding.rank]
    } else {
        suffix[binding.rank][binding.role - SUFFIX_BASE]
    };
    buffer.pointer(binding.offset, 0, binding.extent, binding.access)
}

impl Transaction for Native {
    type MlpState = Gfx950EngineeringPeerWaveMlpStateV1;
    fn load_mlp_worker(
        &mut self,
        rank: usize,
        object: Vec<u8>,
        digest: [u8; 32],
    ) -> Result<KernelMetadataV1> {
        let kernel = self
            .prefix
            .group
            .load_kernel(rank, object, digest, MLP_WORKER.into())?;
        let metadata = kernel.metadata().clone();
        self.mlp_kernels[rank] = Some(kernel);
        Ok(metadata)
    }
    fn mlp_state(&mut self, rank: usize) -> Result<Self::MlpState> {
        self.prefix.group.allocate_wave_mlp_state_v1(rank)
    }
    fn observe_mlp(&mut self, state: &Self::MlpState) -> Result<[u32; 11]> {
        self.prefix.group.observe_wave_mlp_state_v1(state)
    }
    unsafe fn mlp_round(
        &mut self,
        residual: &[Self::Buffer; 2],
        suffix: &[[Self::Buffer; 10]; 2],
        states: &[Self::MlpState; 2],
        timeout: u32,
    ) -> Result<[u64; 2]> {
        let mut commands = Vec::with_capacity(2);
        for rank in 0..2 {
            if states[rank].owner_rank() != rank {
                return Err("resident layer MLP state owner order".into());
            }
            let mut pointers: Vec<_> = mlp_bindings(rank)
                .into_iter()
                .map(|value| resolve_buffer(value, residual, suffix))
                .collect();
            pointers.push(states[rank].pointer_mlp_v1());
            commands.push(Gfx950EngineeringPeerDispatchV1 {
                kernel: self.mlp_kernels[rank]
                    .as_ref()
                    .ok_or("resident layer missing MLP worker")?,
                bytes: vec![0; 344],
                workgroup: [64, 1, 1],
                grid: [128, 1, 1],
                pointers,
                timeout_ms: timeout,
            });
        }
        // SAFETY: rank-private roots/state; peer-visible down allocations are
        // not read by another rank until this entire independent round completes.
        unsafe { self.prefix.group.dispatch_round_unchecked(commands)? }
            .try_into()
            .map_err(|_| "resident layer MLP timing roster".into())
    }
    fn load_stage(
        &mut self,
        rank: usize,
        kind: Kind,
        object: Vec<u8>,
        digest: [u8; 32],
    ) -> Result<KernelMetadataV1> {
        let kernel = self
            .prefix
            .group
            .load_kernel(rank, object, digest, kind.entry().into())?;
        let metadata = kernel.metadata().clone();
        self.kernels[rank][kind.kernel_slot()] = Some(kernel);
        Ok(metadata)
    }
    unsafe fn stage_round(
        &mut self,
        kind: Kind,
        residual: &[Self::Buffer; 2],
        suffix: &[[Self::Buffer; 10]; 2],
        timeout: u32,
    ) -> Result<[u64; 2]> {
        let mut commands = Vec::with_capacity(2);
        for rank in 0..2 {
            commands.push(Gfx950EngineeringPeerDispatchV1 {
                kernel: self.kernels[rank][kind.kernel_slot()]
                    .as_ref()
                    .ok_or("resident layer missing stage kernel")?,
                bytes: stage_bytes(kind),
                workgroup: [64, 1, 1],
                grid: kind.grid(),
                pointers: bindings(kind, rank)
                    .into_iter()
                    .map(|value| resolve_buffer(value, residual, suffix))
                    .collect(),
                timeout_ms: timeout,
            });
        }
        // SAFETY: distinct per-rank outputs and only completed immutable predecessors.
        unsafe { self.prefix.group.dispatch_round_unchecked(commands)? }
            .try_into()
            .map_err(|_| "resident layer stage timing roster".into())
    }
    unsafe fn final_round(
        &mut self,
        residual: &[Self::Buffer; 2],
        suffix: &[[Self::Buffer; 10]; 2],
        timeout: u32,
    ) -> Result<[u64; 2]> {
        let mut commands = Vec::with_capacity(2);
        for rank in 0..2 {
            commands.push(Gfx950EngineeringPeerDispatchV1 {
                kernel: self.prefix.consumers[rank]
                    .as_ref()
                    .ok_or("resident layer missing final consumer")?,
                bytes: prefix::consumer_bytes(),
                workgroup: [64, 1, 1],
                grid: [4096, 1, 1],
                pointers: final_bindings(rank)
                    .into_iter()
                    .map(|value| resolve_buffer(value, residual, suffix))
                    .collect(),
                timeout_ms: timeout,
            });
        }
        // SAFETY: both original down partials completed; peer access is read-only.
        unsafe { self.prefix.group.dispatch_round_unchecked(commands)? }
            .try_into()
            .map_err(|_| "resident layer final timing roster".into())
    }
}

#[cfg(test)]
#[path = "engineering_gfx950_resident_layer_tp2_v1_tests.rs"]
mod tests;
