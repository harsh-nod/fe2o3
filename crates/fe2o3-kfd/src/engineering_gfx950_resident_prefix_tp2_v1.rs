//! One-shot resident V5 producers followed by ordered v18 TP2 residual consumers.

use super::wave_qkv_attention_output_tasks_v5 as v5;
use super::*;

const PRODUCER: &str = "ferric_qwen3_claimed_rmsnorm_qkv_attention_output_bf16_f32_v5";
const CONSUMER: &str = "ferric_qwen3_tp_peer_tp2_ordered_residual_bf16_v18";
const ROLES: usize = 14;
const CONSUMER_BYTES: usize = 424;
const RESIDUAL_BYTES: usize = 8192;

/// Borrowed immutable inputs for one rank; caches include the pre-append contents.
#[derive(Clone, Copy)]
pub struct Gfx950EngineeringResidentPrefixRankV1<'a> {
    pub input: &'a [u16; 4096],
    pub norm_weight: &'a [u16; 4096],
    pub qkv_weight: &'a [u16; 12_582_912],
    pub head_norm_weight: &'a [u16; 256],
    pub rotary: &'a [f32; 128],
    pub cache_metadata: &'a [u32; 145],
    pub output_weight: &'a [u16; 8_388_608],
    pub key_cache: &'a [u16],
    pub value_cache: &'a [u16],
}

/// Readbacks published only after both producer/consumer rounds and group close.
#[derive(Debug)]
pub struct Gfx950EngineeringResidentPrefixObservationV1 {
    pub normalized_words: Box<[u16]>,
    pub qkv_words: Box<[u16]>,
    pub query_words: Box<[u16]>,
    pub key_cache_words: Box<[u16]>,
    pub value_cache_words: Box<[u16]>,
    pub attention_words: Box<[u16]>,
    pub output_partial: Box<[f32]>,
    pub residual_words: Box<[u16]>,
    pub final_state: [u32; 22],
}

#[derive(Debug)]
pub struct Gfx950EngineeringResidentPrefixResultV1 {
    pub ranks: [Gfx950EngineeringResidentPrefixObservationV1; 2],
    /// Existing peer-round host timers, excluding allocation, upload and readback.
    pub producer_dispatch_ns: [u64; 2],
    pub consumer_dispatch_ns: [u64; 2],
}

/// Retain two complete V5 root sets and consume their original FP32 allocations.
/// No source/profile authorization or numerical-model claim is returned.
///
/// # Safety
/// Run only in a dedicated disposable process with no unrelated GPU work. The
/// caller must independently review both images, target/ISA, System atomics,
/// peer visibility and the full producer-completion-before-consumer contract.
/// Metadata, idle observations and coherent allocation flags do not prove these
/// obligations. Use only the exact authorized devices after shared-host topology,
/// currentness and idle checks. These observations do not establish exclusivity.
/// This process must exclusively own its queues, mappings and allocations.
/// Independent concurrent work may cause contention or a terminal timeout. Every
/// error is terminal: exit without retry or resource reuse. Neither finite polling
/// nor the two-rank round implies fairness, simultaneous execution or performance.
#[allow(clippy::too_many_arguments)]
pub unsafe fn execute_gfx950_engineering_resident_prefix_tp2_unchecked_v1(
    unique_ids: [u64; 2],
    producer_object: Vec<u8>,
    producer_digest: [u8; 32],
    consumer_object: Vec<u8>,
    consumer_digest: [u8; 32],
    ranks: [Gfx950EngineeringResidentPrefixRankV1<'_>; 2],
    timeout_ms: u32,
) -> Result<Gfx950EngineeringResidentPrefixResultV1> {
    validate_request(
        unique_ids,
        &producer_object,
        producer_digest,
        &consumer_object,
        consumer_digest,
        timeout_ms,
    )?;
    if ranks[0].input != ranks[1].input
        || ranks[0].cache_metadata != ranks[1].cache_metadata
        || ranks.iter().any(|rank| {
            rank.key_cache.len() != v5::CACHE_WORDS || rank.value_cache.len() != v5::CACHE_WORDS
        })
    {
        return Err("resident TP2 input/metadata identity or cache extent".into());
    }
    let inputs = ranks.map(|rank| {
        [
            v5::encode_words(rank.input),
            v5::encode_words(rank.norm_weight),
            v5::encode_words(rank.qkv_weight),
            v5::encode_words(rank.head_norm_weight),
            v5::encode_f32(rank.rotary),
            rank.cache_metadata
                .iter()
                .flat_map(|word| word.to_le_bytes())
                .collect(),
            v5::encode_words(rank.output_weight),
        ]
    });
    let caches = ranks.map(|rank| {
        [
            v5::encode_words(rank.key_cache),
            v5::encode_words(rank.value_cache),
        ]
    });
    let inputs = inputs
        .each_ref()
        .map(|rank| rank.each_ref().map(Vec::as_slice));
    let caches = caches
        .each_ref()
        .map(|rank| rank.each_ref().map(Vec::as_slice));
    validate_inputs(&inputs, &caches)?;
    // SAFETY: inherited dedicated-process and externally reviewed group contract.
    let group = unsafe { Gfx950EngineeringPeerGroupV1::open_unchecked(&unique_ids)? };
    let backend = Native {
        group,
        producers: [None, None],
        consumers: [None, None],
    };
    // SAFETY: only the fixed producer and consumer rosters below are submitted.
    unsafe {
        coordinate(
            backend,
            producer_object,
            producer_digest,
            consumer_object,
            consumer_digest,
            inputs,
            caches,
            timeout_ms,
        )
    }
}

pub(super) fn validate_request(
    ids: [u64; 2],
    producer: &[u8],
    pd: [u8; 32],
    consumer: &[u8],
    cd: [u8; 32],
    timeout: u32,
) -> Result<()> {
    if ids[0] == 0 || ids[1] == 0 || ids[0] == ids[1] || !(1..=150_000).contains(&timeout) {
        return Err("resident TP2 requires two distinct devices and bounded timeout".into());
    }
    v5::validate_request(producer, pd, PRODUCER, timeout)?;
    v5::validate_request(consumer, cd, CONSUMER, timeout)
}

pub(super) fn validate_inputs(
    inputs: &[[&[u8]; 7]; 2],
    caches: &[[&[u8]; 2]; 2],
) -> Result<[usize; 2]> {
    if inputs[0][0] != inputs[1][0]
        || inputs[0][5] != inputs[1][5]
        || inputs.iter().any(|rank| {
            rank.iter()
                .enumerate()
                .any(|(role, bytes)| bytes.len() != v5::EXTENTS[role])
        })
        || caches
            .iter()
            .flatten()
            .any(|bytes| bytes.len() != v5::CACHE_WORDS * 2)
    {
        return Err("resident TP2 immutable input/cache roster".into());
    }
    Ok([v5::cache_slot(inputs[0][5])?, v5::cache_slot(inputs[1][5])?])
}

pub(super) fn validate_consumer(metadata: &KernelMetadataV1, digest: [u8; 32]) -> Result<()> {
    if metadata.object_sha256 != digest
        || metadata.symbol != CONSUMER
        || metadata.kernarg_bytes != 424
        || metadata.kernarg_alignment != 8
        || metadata.group_segment_bytes != 0
        || metadata.private_segment_bytes != 0
        || metadata.wavefront_size != 64
        || metadata.implicit_argument_offset != Some(168)
        || metadata.implicit_argument_bytes != 256
        || metadata.explicit_arguments.len() != 22
    {
        return Err("resident TP2 consumer physical ABI/resources".into());
    }
    for (index, argument) in metadata.explicit_arguments.iter().enumerate() {
        let pointer = index < 20 && index % 2 == 0;
        let expected_offset = if index < 20 {
            index as u32 * 8
        } else {
            160 + (index as u32 - 20) * 4
        };
        let alignment = if index < 16 { 4 } else { 2 };
        let access = if index == 18 {
            BufferAccessV1::Write
        } else {
            BufferAccessV1::Read
        };
        if argument.offset != expected_offset
            || argument.bytes != if index < 20 { 8 } else { 4 }
            || argument.global_buffer != pointer
            || (pointer
                && (argument
                    .pointee_alignment
                    .is_some_and(|value| value != alignment)
                    || argument.access.is_some_and(|value| value != access)))
            || (!pointer && (argument.pointee_alignment.is_some() || argument.access.is_some()))
        {
            return Err("resident TP2 consumer pointer/length/scalar roster".into());
        }
    }
    Ok(())
}

pub(super) fn consumer_bytes() -> Vec<u8> {
    let mut bytes = vec![0; CONSUMER_BYTES];
    for slot in [0usize, 1, 8, 9] {
        bytes[slot * 16 + 8..slot * 16 + 16].copy_from_slice(&4096u64.to_le_bytes());
    }
    bytes[160..164].copy_from_slice(&1u32.to_le_bytes());
    bytes[164..168].copy_from_slice(&2u32.to_le_bytes());
    bytes
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ConsumerBinding {
    rank: usize,
    role: usize,
    offset: u32,
    extent: u64,
    access: BufferAccessV1,
}

fn consumer_bindings(rank: usize) -> [ConsumerBinding; 10] {
    core::array::from_fn(|slot| match slot {
        0 | 1 => ConsumerBinding {
            rank: slot,
            role: 13,
            offset: slot as u32 * 16,
            extent: 16384,
            access: BufferAccessV1::Read,
        },
        2..=7 => ConsumerBinding {
            rank: 0,
            role: 13,
            offset: slot as u32 * 16,
            extent: 0,
            access: BufferAccessV1::Read,
        },
        8 => ConsumerBinding {
            rank,
            role: 0,
            offset: 128,
            extent: 8192,
            access: BufferAccessV1::Read,
        },
        _ => ConsumerBinding {
            rank,
            role: 15,
            offset: 144,
            extent: 8192,
            access: BufferAccessV1::Write,
        },
    })
}

// Test seam around existing peer-group transactions, not a general launch API.
pub(super) trait Transaction {
    type Buffer: Copy + Eq;
    type State;
    fn load(
        &mut self,
        rank: usize,
        producer: bool,
        object: Vec<u8>,
        digest: [u8; 32],
    ) -> Result<KernelMetadataV1>;
    fn allocate(
        &mut self,
        rank: usize,
        role: usize,
        bytes: usize,
        peer: bool,
    ) -> Result<Self::Buffer>;
    fn state(&mut self, rank: usize) -> Result<Self::State>;
    fn observe(&mut self, state: &Self::State) -> Result<[u32; 22]>;
    fn write(&mut self, buffer: Self::Buffer, offset: usize, bytes: &[u8]) -> Result<()>;
    fn read(&mut self, buffer: Self::Buffer, offset: usize, bytes: usize) -> Result<Vec<u8>>;
    unsafe fn producers(
        &mut self,
        roots: &[[Self::Buffer; ROLES]; 2],
        states: &[Self::State; 2],
        timeout: u32,
    ) -> Result<[u64; 2]>;
    unsafe fn consumers(
        &mut self,
        roots: &[[Self::Buffer; ROLES]; 2],
        outputs: &[Self::Buffer; 2],
        timeout: u32,
    ) -> Result<[u64; 2]>;
    fn close(&mut self) -> Result<()>;
    fn quarantine(self);
}

pub(super) fn upload<T: Transaction>(tx: &mut T, buffer: T::Buffer, bytes: &[u8]) -> Result<()> {
    for (index, chunk) in bytes.chunks(MAX_TRANSFER_BYTES_V1 as usize).enumerate() {
        tx.write(buffer, index * MAX_TRANSFER_BYTES_V1 as usize, chunk)?;
    }
    Ok(())
}

pub(super) fn read<T: Transaction>(tx: &mut T, buffer: T::Buffer, bytes: usize) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(bytes);
    while out.len() < bytes {
        let count = (bytes - out.len()).min(MAX_TRANSFER_BYTES_V1 as usize);
        let chunk = tx.read(buffer, out.len(), count)?;
        if chunk.len() != count {
            return Err("resident TP2 short readback".into());
        }
        out.extend_from_slice(&chunk);
    }
    Ok(out)
}

pub(super) fn unchanged<T: Transaction>(
    tx: &mut T,
    buffer: T::Buffer,
    expected: &[u8],
) -> Result<()> {
    for (index, chunk) in expected.chunks(MAX_TRANSFER_BYTES_V1 as usize).enumerate() {
        if tx.read(buffer, index * MAX_TRANSFER_BYTES_V1 as usize, chunk.len())? != chunk {
            return Err("resident TP2 immutable/read-only allocation changed".into());
        }
    }
    Ok(())
}

pub(super) fn finite_bf16(bytes: &[u8]) -> Result<()> {
    if !bytes.len().is_multiple_of(2)
        || bytes
            .chunks_exact(2)
            .any(|word| u16::from_le_bytes([word[0], word[1]]) & 0x7f80 == 0x7f80)
    {
        return Err("resident TP2 nonfinite BF16 output".into());
    }
    Ok(())
}

pub(super) struct Stage {
    pub(super) outputs: [Vec<u8>; 7],
    pub(super) state: [u32; 22],
}

pub(super) fn stage<T: Transaction>(
    tx: &mut T,
    roots: &[T::Buffer; ROLES],
    state: &T::State,
    inputs: &[&[u8]; 7],
    caches: &[&[u8]; 2],
    slot: usize,
) -> Result<Stage> {
    let state = tx.observe(state)?;
    v5::validate_final_state(state)?;
    for role in 0..7 {
        unchanged(tx, roots[role], inputs[role])?;
    }
    let mut outputs = Vec::with_capacity(7);
    for role in 7..14 {
        let bytes = read(tx, roots[role], v5::EXTENTS[role])?;
        if role == 10 || role == 11 {
            v5::validate_cache_isolation(caches[role - 10], &bytes, slot)?;
            finite_bf16(&bytes[slot * 1024..(slot + 1) * 1024])?;
        } else if role == 13 {
            if v5::decode_f32(&bytes, 4096)?
                .iter()
                .any(|value| !value.is_finite())
            {
                return Err("resident TP2 nonfinite FP32 partial".into());
            }
        } else {
            finite_bf16(&bytes)?;
        }
        outputs.push(bytes);
    }
    Ok(Stage {
        outputs: outputs
            .try_into()
            .map_err(|_| "resident TP2 output roster")?,
        state,
    })
}

#[allow(clippy::too_many_arguments)]
unsafe fn coordinate<T: Transaction>(
    mut tx: T,
    producer: Vec<u8>,
    pd: [u8; 32],
    consumer: Vec<u8>,
    cd: [u8; 32],
    inputs: [[&[u8]; 7]; 2],
    caches: [[&[u8]; 2]; 2],
    timeout: u32,
) -> Result<Gfx950EngineeringResidentPrefixResultV1> {
    let result = (|| {
        let slots = validate_inputs(&inputs, &caches)?;
        for rank in 0..2 {
            v5::validate_metadata(&tx.load(rank, true, producer.clone(), pd)?, pd, PRODUCER)?;
            validate_consumer(&tx.load(rank, false, consumer.clone(), cd)?, cd)?;
        }
        let mut all_roots = Vec::with_capacity(2);
        let mut all_states = Vec::with_capacity(2);
        let mut residuals = Vec::with_capacity(2);
        let mut unique = Vec::with_capacity(30);
        for rank in 0..2 {
            let mut roots = Vec::with_capacity(ROLES);
            for role in 0..ROLES {
                let buffer = tx.allocate(rank, role, v5::EXTENTS[role], role == 13)?;
                if unique.contains(&buffer) {
                    return Err("resident TP2 allocation token alias".into());
                }
                unique.push(buffer);
                if role < 7 {
                    upload(&mut tx, buffer, inputs[rank][role])?;
                } else if role == 10 || role == 11 {
                    upload(&mut tx, buffer, caches[rank][role - 10])?;
                } else {
                    let poison = if role == 13 {
                        0x7fc00000u32.to_le_bytes().repeat(4096)
                    } else {
                        0x7fc0u16.to_le_bytes().repeat(v5::EXTENTS[role] / 2)
                    };
                    upload(&mut tx, buffer, &poison)?;
                }
                roots.push(buffer);
            }
            let state = tx.state(rank)?;
            let initial = tx.observe(&state)?;
            if initial != core::array::from_fn(|index| u32::from(index < 2)) {
                return Err("resident TP2 state is not fresh".into());
            }
            all_states.push(state);
            all_roots.push(roots.try_into().map_err(|_| "resident TP2 root roster")?);
            let residual = tx.allocate(rank, 15, RESIDUAL_BYTES, false)?;
            if unique.contains(&residual) {
                return Err("resident TP2 residual allocation alias".into());
            }
            unique.push(residual);
            upload(&mut tx, residual, &0x7fc0u16.to_le_bytes().repeat(4096))?;
            residuals.push(residual);
        }
        let roots: [[T::Buffer; ROLES]; 2] =
            all_roots.try_into().map_err(|_| "resident TP2 ranks")?;
        let states: [T::State; 2] = all_states.try_into().map_err(|_| "resident TP2 states")?;
        let residuals: [T::Buffer; 2] =
            residuals.try_into().map_err(|_| "resident TP2 residuals")?;
        // SAFETY: exact independent per-rank producer roots and initialized states.
        let producer_dispatch_ns = unsafe { tx.producers(&roots, &states, timeout)? };
        // Both stages must validate before either consumer is published.
        let stages = [
            stage(
                &mut tx, &roots[0], &states[0], &inputs[0], &caches[0], slots[0],
            )?,
            stage(
                &mut tx, &roots[1], &states[1], &inputs[1], &caches[1], slots[1],
            )?,
        ];
        // SAFETY: original partial allocations retained, peer reads only, disjoint outputs.
        let consumer_dispatch_ns = unsafe { tx.consumers(&roots, &residuals, timeout)? };
        let mut observations = Vec::with_capacity(2);
        for rank in 0..2 {
            if tx.observe(&states[rank])? != stages[rank].state {
                return Err("resident TP2 consumer changed producer state".into());
            }
            for role in 0..7 {
                unchanged(&mut tx, roots[rank][role], inputs[rank][role])?;
            }
            for role in 7..14 {
                unchanged(&mut tx, roots[rank][role], &stages[rank].outputs[role - 7])?;
            }
            let residual = read(&mut tx, residuals[rank], RESIDUAL_BYTES)?;
            finite_bf16(&residual)?;
            let o = &stages[rank].outputs;
            observations.push(Gfx950EngineeringResidentPrefixObservationV1 {
                normalized_words: v5::decode_words(&o[0], 4096)?,
                qkv_words: v5::decode_words(&o[1], 3072)?,
                query_words: v5::decode_words(&o[2], 2048)?,
                key_cache_words: v5::decode_words(&o[3], v5::CACHE_WORDS)?,
                value_cache_words: v5::decode_words(&o[4], v5::CACHE_WORDS)?,
                attention_words: v5::decode_words(&o[5], 2048)?,
                output_partial: v5::decode_f32(&o[6], 4096)?,
                residual_words: v5::decode_words(&residual, 4096)?,
                final_state: stages[rank].state,
            });
        }
        if observations[0].residual_words != observations[1].residual_words {
            return Err("resident TP2 replicated residual outputs differ".into());
        }
        let ranks = observations
            .try_into()
            .map_err(|_| "resident TP2 output rank roster")?;
        tx.close()?;
        Ok(Gfx950EngineeringResidentPrefixResultV1 {
            ranks,
            producer_dispatch_ns,
            consumer_dispatch_ns,
        })
    })();
    if result.is_err() {
        tx.quarantine();
    }
    result
}

pub(super) struct Native {
    pub(super) group: Gfx950EngineeringPeerGroupV1,
    producers: [Option<Gfx950EngineeringPeerKernelV1>; 2],
    pub(super) consumers: [Option<Gfx950EngineeringPeerKernelV1>; 2],
}

impl Native {
    pub(super) fn from_group(group: Gfx950EngineeringPeerGroupV1) -> Self {
        Self {
            group,
            producers: [None, None],
            consumers: [None, None],
        }
    }
}

impl Transaction for Native {
    type Buffer = Gfx950EngineeringPeerBufferV1;
    type State = Gfx950EngineeringPeerWaveOutputStateV5;
    fn load(
        &mut self,
        rank: usize,
        producer: bool,
        object: Vec<u8>,
        digest: [u8; 32],
    ) -> Result<KernelMetadataV1> {
        let kernel = self.group.load_kernel(
            rank,
            object,
            digest,
            if producer { PRODUCER } else { CONSUMER }.into(),
        )?;
        let metadata = kernel.metadata().clone();
        if producer {
            self.producers[rank] = Some(kernel);
        } else {
            self.consumers[rank] = Some(kernel);
        }
        Ok(metadata)
    }
    fn allocate(
        &mut self,
        rank: usize,
        _role: usize,
        bytes: usize,
        peer: bool,
    ) -> Result<Self::Buffer> {
        let peers = [1 - rank];
        self.group
            .allocate(rank, if peer { &peers } else { &[] }, bytes as u64)
    }
    fn state(&mut self, rank: usize) -> Result<Self::State> {
        self.group.allocate_wave_output_state_v5(rank)
    }
    fn observe(&mut self, state: &Self::State) -> Result<[u32; 22]> {
        self.group.observe_wave_output_state_v5(state)
    }
    fn write(&mut self, buffer: Self::Buffer, offset: usize, bytes: &[u8]) -> Result<()> {
        self.group.write(buffer, offset as u64, bytes)
    }
    fn read(&mut self, buffer: Self::Buffer, offset: usize, bytes: usize) -> Result<Vec<u8>> {
        self.group.read(buffer, offset as u64, bytes as u32)
    }
    unsafe fn producers(
        &mut self,
        roots: &[[Self::Buffer; ROLES]; 2],
        states: &[Self::State; 2],
        timeout: u32,
    ) -> Result<[u64; 2]> {
        let mut commands = Vec::with_capacity(2);
        for rank in 0..2 {
            let mut pointers = roots[rank]
                .iter()
                .enumerate()
                .map(|(role, buffer)| {
                    buffer.pointer(
                        role as u32 * 8,
                        0,
                        v5::EXTENTS[role] as u64,
                        v5::role_access(role),
                    )
                })
                .collect::<Vec<_>>();
            if states[rank].owner_rank() != rank {
                return Err("resident TP2 state owner".into());
            }
            pointers.push(states[rank].pointer_v5());
            commands.push(Gfx950EngineeringPeerDispatchV1 {
                kernel: self.producers[rank]
                    .as_ref()
                    .ok_or("resident TP2 missing producer")?,
                bytes: vec![0; 376],
                workgroup: [64, 1, 1],
                grid: [128, 1, 1],
                pointers,
                timeout_ms: timeout,
            });
        }
        // SAFETY: caller establishes the two-rank engineering contract.
        unsafe { self.group.dispatch_round_unchecked(commands)? }
            .try_into()
            .map_err(|_| "resident TP2 producer timing roster".into())
    }
    unsafe fn consumers(
        &mut self,
        roots: &[[Self::Buffer; ROLES]; 2],
        outputs: &[Self::Buffer; 2],
        timeout: u32,
    ) -> Result<[u64; 2]> {
        let mut commands = Vec::with_capacity(2);
        for rank in 0..2 {
            // Zero-length unused slices retain valid original partial backing.
            let pointers = consumer_bindings(rank)
                .map(|binding| {
                    let buffer = if binding.role == 15 {
                        outputs[binding.rank]
                    } else {
                        roots[binding.rank][binding.role]
                    };
                    buffer.pointer(binding.offset, 0, binding.extent, binding.access)
                })
                .to_vec();
            commands.push(Gfx950EngineeringPeerDispatchV1 {
                kernel: self.consumers[rank]
                    .as_ref()
                    .ok_or("resident TP2 missing consumer")?,
                bytes: consumer_bytes(),
                workgroup: [64, 1, 1],
                grid: [4096, 1, 1],
                pointers,
                timeout_ms: timeout,
            });
        }
        // SAFETY: only completed original FP32 partials are peer-readable; writes are disjoint.
        unsafe { self.group.dispatch_round_unchecked(commands)? }
            .try_into()
            .map_err(|_| "resident TP2 consumer timing roster".into())
    }
    fn close(&mut self) -> Result<()> {
        self.group.close()
    }
    fn quarantine(self) {
        core::mem::forget(self);
    }
}

#[cfg(test)]
#[path = "engineering_gfx950_resident_prefix_tp2_v1_tests.rs"]
mod tests;
