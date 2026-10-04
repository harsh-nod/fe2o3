//! One-shot eleven-root engineering execution; no protected or coherence authority.

use super::*;

const EXTENTS: [usize; 11] = [
    8192, 8192, 50_331_648, 50_331_648, 50_331_648, 8192, 12_288, 12_288, 12_288, 16_384, 44,
];
const WEIGHT_WORDS: usize = 25_165_824;
const SYMBOL: &str = "ferric_qwen3_claimed_mlp_bf16_f32_v1";
const INITIAL_STATE: [u32; 11] = [1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0];
const WORKGROUP: [u16; 3] = [64, 1, 1];
const GRID: [u32; 3] = [128, 1, 1];
const KERNARG_BYTES: usize = 88 + 256;

// Bit-preserving transport is shared; no attention or cache policy is reused.
use super::wave_qkv_attention_output_tasks_v5::{
    decode_f32, decode_words, encode_f32, encode_words,
};

/// Observations after one completed dispatch and successful context teardown.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Gfx950EngineeringWaveMlpTasksResultV1 {
    /// Epoch, ready, done, claimed, owners, errors, and five arrival counters.
    pub final_state: [u32; 11],
    /// Existing queue timer: after ring reservation, before kernarg copy and
    /// publication, through confirmed completion and idle check. Excludes
    /// context opening, loading, allocation, upload, readback, and teardown.
    pub dispatch_elapsed_ns: u64,
}

/// Execute Norm -> Gate/Up -> SwiGLU -> FP32 Down partial.
/// All four BF16 outputs and the FP32 partial remain unchanged unless completion,
/// readback validation and teardown succeed. Numerical acceptance is a separate
/// caller obligation; this is not ordinary or protected launch admission.
///
/// # Safety
/// Use only a dedicated disposable process with no unrelated GPU work or shared
/// Rust memory. Independently bind the reviewed object digest to the exact
/// eleven-root source, descriptor, two Wave64 workgroups and 512-byte LDS.
/// Establish the source/ISA, System-atomic/payload coherence and lifetime
/// obligations; metadata and HOST_VISIBLE_COHERENT alone do not prove them.
/// Fresh epoch1 and Norm0 -> Gate1/Up2 -> SwiGLU3 -> Down4 are required. Any error
/// is terminal: immediately exit the process; uncertain mappings are quarantined,
/// never reused. Finite polling does not establish progress or scheduling fairness.
/// State values do not substitute for queue completion. No protected, reuse,
/// peer-memory or XGMI authority is returned.
#[allow(clippy::too_many_arguments)]
pub unsafe fn execute_gfx950_engineering_wave_mlp_tasks_unchecked_v1(
    unique_id: u64,
    object: Vec<u8>,
    object_sha256: [u8; 32],
    symbol: String,
    input: &[u16; 4096],
    norm_weight: &[u16; 4096],
    gate_weight: &[u16],
    up_weight: &[u16],
    down_weight: &[u16],
    normalized: &mut [u16; 4096],
    gate: &mut [u16; 6144],
    up: &mut [u16; 6144],
    activation: &mut [u16; 6144],
    down_partial: &mut [f32; 4096],
    timeout_ms: u32,
) -> Result<Gfx950EngineeringWaveMlpTasksResultV1> {
    validate_request(&object, object_sha256, &symbol, timeout_ms)?;
    validate_weight_extents([gate_weight.len(), up_weight.len(), down_weight.len()])?;
    let inputs = [
        encode_words(input),
        encode_words(norm_weight),
        encode_words(gate_weight),
        encode_words(up_weight),
        encode_words(down_weight),
    ];
    let device = OpenedKfd::open_default()
        .map_err(explain)?
        .admit_uapi()
        .map_err(explain)?
        .bind_gfx950_xnack_minus(DeviceSelector::UniqueId(unique_id))
        .map_err(explain)?;
    let transaction = NativeTransaction {
        context: Context::open(device)?,
        kernel: None,
        state_buffer: None,
    };
    // SAFETY: inherited one-shot contract; the transaction retains every root.
    unsafe {
        coordinate(
            transaction,
            object,
            object_sha256,
            symbol,
            inputs.each_ref().map(Vec::as_slice),
            [normalized, gate, up, activation],
            down_partial,
            timeout_ms,
        )
    }
}

fn validate_weight_extents(lengths: [usize; 3]) -> Result<()> {
    if lengths != [WEIGHT_WORDS; 3] {
        return Err("wave MLP weight extent".into());
    }
    Ok(())
}

pub(super) fn validate_request(
    object: &[u8],
    digest: [u8; 32],
    symbol: &str,
    timeout_ms: u32,
) -> Result<()> {
    if object.is_empty()
        || object.len() > MAX_OBJECT_BYTES_V1 as usize
        || <[u8; 32]>::from(Sha256::digest(object)) != digest
        || symbol != SYMBOL
        || symbol.len() > 256
        || symbol.as_bytes().contains(&0)
        || !(1..=600_000).contains(&timeout_ms)
    {
        return Err("wave MLP tasks object, symbol, digest or timeout".into());
    }
    Ok(())
}

pub(super) fn role_access(index: usize) -> BufferAccessV1 {
    if index < 5 {
        BufferAccessV1::Read
    } else {
        BufferAccessV1::ReadWrite
    }
}

fn role_alignment(index: usize) -> u32 {
    if matches!(index, 9 | 10) { 4 } else { 2 }
}

pub(super) fn validate_metadata(
    metadata: &KernelMetadataV1,
    digest: [u8; 32],
    symbol: &str,
) -> Result<()> {
    if metadata.object_sha256 != digest
        || metadata.symbol != symbol
        || metadata.kernarg_bytes as usize != KERNARG_BYTES
        || metadata.kernarg_alignment != 8
        || metadata.wavefront_size != 64
        || metadata.private_segment_bytes != 0
        || metadata.group_segment_bytes != 512
        || metadata.implicit_argument_offset != Some(88)
        || metadata.implicit_argument_bytes != 256
        || metadata.explicit_arguments.len() != 11
    {
        return Err("wave MLP tasks physical ABI/resources".into());
    }
    for (index, argument) in metadata.explicit_arguments.iter().enumerate() {
        if argument.offset != index as u32 * 8
            || argument.bytes != 8
            || !argument.global_buffer
            || argument
                .pointee_alignment
                .is_some_and(|value| value != role_alignment(index))
            || argument
                .access
                .is_some_and(|access| access != role_access(index))
        {
            return Err("wave MLP tasks pointer role/offset/alignment".into());
        }
    }
    Ok(())
}

// Snapshots of this Context's existing buffer records, never caller addresses.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct OwnedRegion {
    buffer: u64,
    base: u64,
    requested: usize,
    backing: usize,
}

fn fixups(regions: &[OwnedRegion; 11]) -> [PointerFixupV1; 11] {
    core::array::from_fn(|index| PointerFixupV1 {
        kernarg_offset: index as u32 * 8,
        buffer: regions[index].buffer,
        buffer_offset: 0,
        extent_bytes: EXTENTS[index] as u64,
        access: role_access(index),
    })
}

fn validate_regions(regions: &[OwnedRegion; 11], pointers: &[PointerFixupV1; 11]) -> Result<()> {
    let expected = fixups(regions);
    for (index, region) in regions.iter().enumerate() {
        if region.buffer == 0
            || region.base == 0
            || !region.base.is_multiple_of(PAGE_BYTES as u64)
            || region.requested != EXTENTS[index]
            || region.backing < region.requested
            || !region.backing.is_multiple_of(PAGE_BYTES)
            || region.base.checked_add(region.backing as u64).is_none()
            || pointers[index] != expected[index]
        {
            return Err("wave MLP tasks owned extent/fixup".into());
        }
        for prior in &regions[..index] {
            if prior.buffer == region.buffer
                || (prior.base < region.base + region.backing as u64
                    && region.base < prior.base + prior.backing as u64)
            {
                return Err("wave MLP tasks allocation alias".into());
            }
        }
    }
    Ok(())
}

fn validate_geometry(workgroup: [u16; 3], grid: [u32; 3]) -> Result<()> {
    if workgroup != WORKGROUP || grid != GRID {
        return Err("wave MLP tasks require exactly two Wave64 workgroups".into());
    }
    Ok(())
}

pub(super) fn validate_final_state(state: [u32; 11]) -> Result<()> {
    if state[0] != 1
        || state[1] != 0
        || state[2] != 31
        || state[3] != 31
        || state[5] != 0
        || state[6..] != [64; 5]
        || (state[4] & !0x3ff) != 0
        || (0..5).any(|task| !matches!((state[4] >> (2 * task)) & 3, 1 | 2))
    {
        return Err(format!(
            "wave MLP tasks incomplete/invalid final state: {state:?}"
        ));
    }
    Ok(())
}

// Private test adapter around actual Context operations, not a runtime protocol.
trait Transaction {
    type Prepared;
    type Pending;
    fn load(
        &mut self,
        object: Vec<u8>,
        digest: [u8; 32],
        symbol: String,
    ) -> Result<KernelMetadataV1>;
    fn allocate(&mut self, role: usize) -> Result<OwnedRegion>;
    fn upload(&mut self, buffer: u64, bytes: &[u8]) -> Result<()>;
    fn initialize_state(&mut self, buffer: u64) -> Result<[u32; 11]>;
    fn prepare(
        &mut self,
        regions: &[OwnedRegion; 11],
        pointers: &[PointerFixupV1; 11],
    ) -> Result<Self::Prepared>;
    unsafe fn submit(&mut self, prepared: Self::Prepared, timeout_ms: u32)
    -> Result<Self::Pending>;
    fn complete(&mut self, pending: Self::Pending) -> Result<u64>;
    fn read(&mut self, buffer: u64, bytes: u32) -> Result<Vec<u8>>;
    fn read_state(&mut self, buffer: u64) -> Result<[u32; 11]>;
    fn close(&mut self) -> Result<()>;
    fn quarantine(self);
}

#[allow(clippy::too_many_arguments)]
unsafe fn coordinate<T: Transaction>(
    mut transaction: T,
    object: Vec<u8>,
    digest: [u8; 32],
    symbol: String,
    inputs: [&[u8]; 5],
    mut outputs: [&mut [u16]; 4],
    down_partial: &mut [f32],
    timeout_ms: u32,
) -> Result<Gfx950EngineeringWaveMlpTasksResultV1> {
    let result = (|| {
        validate_request(&object, digest, &symbol, timeout_ms)?;
        if inputs
            .iter()
            .enumerate()
            .any(|(index, bytes)| bytes.len() != EXTENTS[index])
            || outputs
                .iter()
                .enumerate()
                .any(|(index, words)| words.len() != EXTENTS[index + 5] / 2)
            || down_partial.len() != 4096
        {
            return Err("wave MLP tasks input/output extent".into());
        }
        let metadata = transaction.load(object, digest, symbol.clone())?;
        validate_metadata(&metadata, digest, &symbol)?;
        let mut allocated = Vec::with_capacity(11);
        for role in 0..11 {
            allocated.push(transaction.allocate(role)?);
        }
        let regions: [OwnedRegion; 11] = allocated
            .try_into()
            .map_err(|_| "wave MLP tasks allocation count")?;
        let pointers = fixups(&regions);
        validate_regions(&regions, &pointers)?;
        for index in 0..5 {
            transaction.upload(regions[index].buffer, inputs[index])?;
        }
        let initial_outputs: [Vec<u8>; 4] =
            core::array::from_fn(|index| encode_words(outputs[index]));
        for index in 0..4 {
            transaction.upload(regions[index + 5].buffer, &initial_outputs[index])?;
        }
        transaction.upload(regions[9].buffer, &encode_f32(down_partial))?;
        if transaction.initialize_state(regions[10].buffer)? != INITIAL_STATE {
            return Err("wave MLP tasks state initialization".into());
        }
        validate_geometry(WORKGROUP, GRID)?;
        let prepared = transaction.prepare(&regions, &pointers)?;
        // SAFETY: same one-shot contract; all eleven roots remain owned.
        let pending = unsafe { transaction.submit(prepared, timeout_ms) }?;
        let dispatch_elapsed_ns = transaction.complete(pending)?;
        for index in 0..5 {
            if transaction
                .read(regions[index].buffer, EXTENTS[index] as u32)?
                .as_slice()
                != inputs[index]
            {
                return Err("wave MLP tasks read-only input changed".into());
            }
        }
        let mut observed = Vec::with_capacity(4);
        for index in 0..4 {
            let bytes = transaction.read(regions[index + 5].buffer, EXTENTS[index + 5] as u32)?;
            observed.push(decode_words(&bytes, EXTENTS[index + 5] / 2)?);
        }
        let partial = decode_f32(
            &transaction.read(regions[9].buffer, EXTENTS[9] as u32)?,
            4096,
        )?;
        let final_state = transaction.read_state(regions[10].buffer)?;
        validate_final_state(final_state)?;
        transaction.close()?;
        // Nothing fallible remains: publish every caller output only after close.
        for (output, words) in outputs.iter_mut().zip(&observed) {
            output.copy_from_slice(words);
        }
        down_partial.copy_from_slice(&partial);
        Ok(Gfx950EngineeringWaveMlpTasksResultV1 {
            final_state,
            dispatch_elapsed_ns,
        })
    })();
    if result.is_err() {
        transaction.quarantine();
    }
    result
}

struct NativeTransaction {
    context: Context,
    kernel: Option<u64>,
    state_buffer: Option<u64>,
}

impl NativeTransaction {
    fn region(&self, buffer: u64) -> Result<OwnedRegion> {
        let allocation = self
            .context
            .buffers
            .get(&buffer)
            .ok_or("unknown wave MLP tasks buffer")?;
        Ok(OwnedRegion {
            buffer,
            base: allocation.va,
            requested: allocation.requested,
            backing: allocation.backing,
        })
    }
}

impl Transaction for NativeTransaction {
    type Prepared = PreparedDispatch;
    type Pending = PendingDispatch;

    fn load(
        &mut self,
        object: Vec<u8>,
        digest: [u8; 32],
        symbol: String,
    ) -> Result<KernelMetadataV1> {
        match self.context.load(object, digest, symbol)? {
            ResponseV1::LoadedKernel { kernel, metadata } => {
                self.kernel = Some(kernel);
                Ok(metadata)
            }
            _ => Err("wave MLP tasks load response".into()),
        }
    }

    fn allocate(&mut self, role: usize) -> Result<OwnedRegion> {
        if role < 10 {
            return match self.context.allocate(EXTENTS[role] as u64)? {
                ResponseV1::Allocated { buffer, .. } => self.region(buffer),
                _ => Err("wave MLP tasks allocation response".into()),
            };
        }
        if role != 10 || self.state_buffer.is_some() {
            return Err("wave MLP tasks allocation role".into());
        }
        self.context.check_idle()?;
        if self.context.buffers.len() >= MAX_ALLOCATIONS {
            return Err("live user allocation limit".into());
        }
        let buffer = self.context.next_buffer;
        self.context.next_buffer = buffer.checked_add(1).ok_or("buffer ID exhausted")?;
        let allocation = self.context.allocate_resource(
            44,
            KfdAllocMemoryFlags::HOST_VISIBLE_COHERENT,
            |_| Ok(()),
        )?;
        self.context.buffers.insert(buffer, allocation);
        self.state_buffer = Some(buffer);
        self.region(buffer)
    }

    fn upload(&mut self, buffer: u64, bytes: &[u8]) -> Result<()> {
        if self.state_buffer == Some(buffer) {
            return Err("atomic MLP state cannot use byte uploads".into());
        }
        self.context.write(buffer, 0, bytes)
    }

    fn initialize_state(&mut self, buffer: u64) -> Result<[u32; 11]> {
        if self.state_buffer != Some(buffer) {
            return Err("MLP state must be the owned coherent allocation".into());
        }
        self.context.check_idle()?;
        let allocation = self
            .context
            .buffers
            .get_mut(&buffer)
            .ok_or("unknown wave state buffer")?;
        if allocation.requested != 44 {
            return Err("wave state extent".into());
        }
        Backend::initialize_engineering_wave_mlp_task_state_v1(&mut allocation.mapping)
            .map_err(explain)?;
        let state = Backend::observe_engineering_wave_mlp_task_state_v1(&mut allocation.mapping)
            .map_err(explain)?;
        self.context.check_currentness(false)?;
        Ok(state)
    }

    fn prepare(
        &mut self,
        regions: &[OwnedRegion; 11],
        pointers: &[PointerFixupV1; 11],
    ) -> Result<Self::Prepared> {
        validate_regions(regions, pointers)?;
        for region in regions {
            if self.region(region.buffer)? != *region {
                return Err("wave MLP tasks allocation changed".into());
            }
        }
        validate_geometry(WORKGROUP, GRID)?;
        self.context.prepare_dispatch(
            self.kernel.ok_or("wave MLP tasks kernel missing")?,
            vec![0; KERNARG_BYTES],
            WORKGROUP,
            GRID,
            pointers,
        )
    }

    unsafe fn submit(
        &mut self,
        prepared: Self::Prepared,
        timeout_ms: u32,
    ) -> Result<Self::Pending> {
        // SAFETY: inherited public caller contract; no second submission exists.
        unsafe { self.context.publish_prepared_dispatch(prepared, timeout_ms) }
    }

    fn complete(&mut self, mut pending: Self::Pending) -> Result<u64> {
        loop {
            if let Some(elapsed) = self.context.poll_pending_dispatch(&mut pending)? {
                return Ok(elapsed);
            }
            std::thread::sleep(Duration::from_micros(50));
        }
    }

    fn read(&mut self, buffer: u64, bytes: u32) -> Result<Vec<u8>> {
        if self.state_buffer == Some(buffer) {
            return Err("atomic MLP state cannot use byte readback".into());
        }
        self.context.read(buffer, 0, bytes)
    }

    fn read_state(&mut self, buffer: u64) -> Result<[u32; 11]> {
        if self.state_buffer != Some(buffer) {
            return Err("MLP state must be the owned coherent allocation".into());
        }
        self.context.check_idle()?;
        let allocation = self
            .context
            .buffers
            .get_mut(&buffer)
            .ok_or("unknown wave state buffer")?;
        if allocation.requested != 44 {
            return Err("wave state extent".into());
        }
        let state = Backend::observe_engineering_wave_mlp_task_state_v1(&mut allocation.mapping)
            .map_err(explain)?;
        self.context.check_currentness(false)?;
        Ok(state)
    }

    fn close(&mut self) -> Result<()> {
        self.context.close_inner()
    }
    fn quarantine(self) {
        std::mem::forget(self);
    }
}

#[cfg(test)]
#[path = "engineering_gfx950_wave_mlp_tasks_v1_tests.rs"]
mod tests;
