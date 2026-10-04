//! One-shot twelve-root engineering execution; no protected or coherence authority.

use super::*;

const EXTENTS: [usize; 12] = [
    8192, 8192, 25_165_824, 512, 512, 580, 8192, 6144, 4096, 2_359_296, 2_359_296, 80,
];
const CACHE_WORDS: usize = 1_179_648;
const TOKEN_WORDS: usize = 512;
const INITIAL_STATE: [u32; 20] = [1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
const WORKGROUP: [u16; 3] = [64, 1, 1];
const GRID: [u32; 3] = [128, 1, 1];
const KERNARG_BYTES: usize = 96 + 256;

/// Observations after one completed dispatch and successful context teardown.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Gfx950EngineeringWaveQkvPostTasksResultV3 {
    /// Epoch, ready, done, claimed, owners, errors, and fourteen arrival counters.
    pub final_state: [u32; 20],
    /// Existing queue timer: after ring reservation, before kernarg copy and
    /// publication, through confirmed completion and idle check. Excludes
    /// context opening, loading, allocation, upload, readback, and teardown.
    pub dispatch_elapsed_ns: u64,
}

/// Execute one fixed Norm -> packed QKV -> Q/K head norm, RoPE and KV append checkpoint.
/// All five outputs remain unchanged unless completion, readback validation and
/// teardown succeed. This is separate from ordinary/protected launch admission.
///
/// # Safety
/// Use only a dedicated disposable process with no unrelated GPU work or shared
/// Rust memory. Independently bind the reviewed object digest and symbol to the
/// exact twelve-root source, descriptor, two Wave64 workgroups and 512-byte LDS.
/// The caller must establish source/ISA, System-atomic/payload coherence and
/// lifetime obligations; metadata and HOST_VISIBLE_COHERENT do not prove them.
/// Fresh epoch1 and Norm0 -> Qkv1..12 -> Post13 are required. Any error is terminal:
/// immediately exit the process; uncertain mappings are quarantined, never reused.
/// The finite polling budget does not establish progress or scheduling fairness.
/// State values do not substitute for queue completion. No protected, reuse,
/// peer-memory or XGMI authority is returned.
#[allow(clippy::too_many_arguments)]
pub unsafe fn execute_gfx950_engineering_wave_qkv_post_tasks_unchecked_v3(
    unique_id: u64,
    object: Vec<u8>,
    object_sha256: [u8; 32],
    symbol: String,
    input: &[u16; 4096],
    norm_weight: &[u16; 4096],
    qkv_weight: &[u16; 12_582_912],
    head_norm_weight: &[u16; 256],
    rotary: &[f32; 128],
    cache_metadata: &[u32; 145],
    normalized: &mut [u16; 4096],
    qkv_output: &mut [u16; 3072],
    query: &mut [u16; 2048],
    key_cache: &mut [u16; CACHE_WORDS],
    value_cache: &mut [u16; CACHE_WORDS],
    timeout_ms: u32,
) -> Result<Gfx950EngineeringWaveQkvPostTasksResultV3> {
    validate_request(&object, object_sha256, &symbol, timeout_ms)?;
    let inputs = [
        encode_words(input),
        encode_words(norm_weight),
        encode_words(qkv_weight),
        encode_words(head_norm_weight),
        rotary
            .iter()
            .flat_map(|value| value.to_bits().to_le_bytes())
            .collect(),
        cache_metadata
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect(),
    ];
    cache_slot(&inputs[5])?;
    let device = OpenedKfd::open_default()
        .map_err(explain)?
        .admit_uapi()
        .map_err(explain)?
        .bind_gfx950_xnack_minus(DeviceSelector::UniqueId(unique_id))
        .map_err(explain)?;
    let transaction = NativeTransaction {
        context: Context::open(device)?,
        kernel: None,
    };
    // SAFETY: inherited one-shot contract; the transaction retains every root.
    unsafe {
        coordinate(
            transaction,
            object,
            object_sha256,
            symbol,
            inputs.each_ref().map(Vec::as_slice),
            [normalized, qkv_output, query, key_cache, value_cache],
            timeout_ms,
        )
    }
}

fn validate_request(object: &[u8], digest: [u8; 32], symbol: &str, timeout_ms: u32) -> Result<()> {
    if object.is_empty()
        || object.len() > MAX_OBJECT_BYTES_V1 as usize
        || <[u8; 32]>::from(Sha256::digest(object)) != digest
        || symbol.is_empty()
        || symbol.len() > 256
        || symbol.as_bytes().contains(&0)
        || !(1..=600_000).contains(&timeout_ms)
    {
        return Err("wave QKV post tasks object, symbol, digest or timeout".into());
    }
    Ok(())
}

fn role_access(index: usize) -> BufferAccessV1 {
    if index < 6 {
        BufferAccessV1::Read
    } else {
        BufferAccessV1::ReadWrite
    }
}

fn role_alignment(index: usize) -> u32 {
    if matches!(index, 4 | 5 | 11) { 4 } else { 2 }
}

fn validate_metadata(metadata: &KernelMetadataV1, digest: [u8; 32], symbol: &str) -> Result<()> {
    if metadata.object_sha256 != digest
        || metadata.symbol != symbol
        || metadata.kernarg_bytes as usize != KERNARG_BYTES
        || metadata.kernarg_alignment != 8
        || metadata.wavefront_size != 64
        || metadata.private_segment_bytes != 0
        || metadata.group_segment_bytes != 512
        || metadata.implicit_argument_offset != Some(96)
        || metadata.implicit_argument_bytes != 256
        || metadata.explicit_arguments.len() != 12
    {
        return Err("wave QKV post tasks physical ABI/resources".into());
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
            return Err("wave QKV post tasks pointer role/offset/alignment".into());
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

fn fixups(regions: &[OwnedRegion; 12]) -> [PointerFixupV1; 12] {
    core::array::from_fn(|index| PointerFixupV1 {
        kernarg_offset: index as u32 * 8,
        buffer: regions[index].buffer,
        buffer_offset: 0,
        extent_bytes: EXTENTS[index] as u64,
        access: role_access(index),
    })
}

fn validate_regions(regions: &[OwnedRegion; 12], pointers: &[PointerFixupV1; 12]) -> Result<()> {
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
            return Err("wave QKV post tasks owned extent/fixup".into());
        }
        for prior in &regions[..index] {
            if prior.buffer == region.buffer
                || (prior.base < region.base + region.backing as u64
                    && region.base < prior.base + prior.backing as u64)
            {
                return Err("wave QKV post tasks allocation alias".into());
            }
        }
    }
    Ok(())
}

fn validate_geometry(workgroup: [u16; 3], grid: [u32; 3]) -> Result<()> {
    if workgroup != WORKGROUP || grid != GRID {
        return Err("wave QKV post tasks require exactly two Wave64 workgroups".into());
    }
    Ok(())
}

fn validate_final_state(state: [u32; 20]) -> Result<()> {
    if state[0] != 1
        || state[1] != 0
        || state[2] != 16383
        || state[3] != 16383
        || state[4] & !((1u32 << 28) - 1) != 0
        || state[5] != 0
        || state[6..] != [64; 14]
        || (0..14).any(|task| !matches!((state[4] >> (2 * task)) & 3, 1 | 2))
    {
        return Err(format!(
            "wave QKV post tasks incomplete/invalid final state: {state:?}"
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
    fn initialize_state(&mut self, buffer: u64) -> Result<[u32; 20]>;
    fn prepare(
        &mut self,
        regions: &[OwnedRegion; 12],
        pointers: &[PointerFixupV1; 12],
    ) -> Result<Self::Prepared>;
    unsafe fn submit(&mut self, prepared: Self::Prepared, timeout_ms: u32)
    -> Result<Self::Pending>;
    fn complete(&mut self, pending: Self::Pending) -> Result<u64>;
    fn read(&mut self, buffer: u64, bytes: u32) -> Result<Vec<u8>>;
    fn read_state(&mut self, buffer: u64) -> Result<[u32; 20]>;
    fn close(&mut self) -> Result<()>;
    fn quarantine(self);
}

fn encode_words(words: &[u16]) -> Vec<u8> {
    words.iter().flat_map(|word| word.to_le_bytes()).collect()
}

fn decode_words(bytes: &[u8], count: usize) -> Result<Box<[u16]>> {
    if count.checked_mul(2) != Some(bytes.len()) {
        return Err("wave QKV post tasks output extent".into());
    }
    Ok(bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect::<Vec<_>>()
        .into_boxed_slice())
}

fn cache_slot(metadata: &[u8]) -> Result<usize> {
    if metadata.len() != 580 {
        return Err("wave QKV post tasks metadata extent".into());
    }
    let word = |index: usize| {
        u32::from_le_bytes(
            metadata[index * 4..index * 4 + 4]
                .try_into()
                .expect("validated metadata extent"),
        )
    };
    let position = word(0) as usize;
    if position >= 2304 {
        return Err("wave QKV post tasks position".into());
    }
    let mut seen = [false; 144];
    for logical in 0..144 {
        let page = word(logical + 1) as usize;
        if page >= 144 || seen[page] {
            return Err("wave QKV post tasks page table".into());
        }
        seen[page] = true;
    }
    Ok(word(position / 16 + 1) as usize * 16 + position % 16)
}

fn validate_cache_isolation(before: &[u8], after: &[u8], slot: usize) -> Result<()> {
    let start = slot
        .checked_mul(TOKEN_WORDS * 2)
        .ok_or("cache slot overflow")?;
    let end = start
        .checked_add(TOKEN_WORDS * 2)
        .ok_or("cache end overflow")?;
    if before.len() != CACHE_WORDS * 2
        || after.len() != before.len()
        || end > before.len()
        || before[..start] != after[..start]
        || before[end..] != after[end..]
    {
        return Err("wave QKV post tasks modified cache outside the selected slot".into());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
unsafe fn coordinate<T: Transaction>(
    mut transaction: T,
    object: Vec<u8>,
    digest: [u8; 32],
    symbol: String,
    inputs: [&[u8]; 6],
    mut outputs: [&mut [u16]; 5],
    timeout_ms: u32,
) -> Result<Gfx950EngineeringWaveQkvPostTasksResultV3> {
    let result = (|| {
        validate_request(&object, digest, &symbol, timeout_ms)?;
        if inputs
            .iter()
            .enumerate()
            .any(|(index, bytes)| bytes.len() != EXTENTS[index])
            || outputs
                .iter()
                .enumerate()
                .any(|(index, words)| words.len() != EXTENTS[index + 6] / 2)
        {
            return Err("wave QKV post tasks input/output extent".into());
        }
        let slot = cache_slot(inputs[5])?;
        let metadata = transaction.load(object, digest, symbol.clone())?;
        validate_metadata(&metadata, digest, &symbol)?;
        let mut allocated = Vec::with_capacity(12);
        for role in 0..12 {
            allocated.push(transaction.allocate(role)?);
        }
        let regions: [OwnedRegion; 12] = allocated
            .try_into()
            .map_err(|_| "wave QKV post tasks allocation count")?;
        let pointers = fixups(&regions);
        validate_regions(&regions, &pointers)?;
        for index in 0..6 {
            transaction.upload(regions[index].buffer, inputs[index])?;
        }
        let initial_outputs: [Vec<u8>; 5] =
            core::array::from_fn(|index| encode_words(outputs[index]));
        for index in 0..5 {
            transaction.upload(regions[index + 6].buffer, &initial_outputs[index])?;
        }
        if transaction.initialize_state(regions[11].buffer)? != INITIAL_STATE {
            return Err("wave QKV post tasks state initialization".into());
        }
        validate_geometry(WORKGROUP, GRID)?;
        let prepared = transaction.prepare(&regions, &pointers)?;
        // SAFETY: same one-shot contract; all twelve roots remain owned.
        let pending = unsafe { transaction.submit(prepared, timeout_ms) }?;
        let dispatch_elapsed_ns = transaction.complete(pending)?;
        for index in 0..6 {
            if transaction
                .read(regions[index].buffer, EXTENTS[index] as u32)?
                .as_slice()
                != inputs[index]
            {
                return Err("wave QKV post tasks read-only input changed".into());
            }
        }
        let mut observed = Vec::with_capacity(5);
        for index in 0..5 {
            let bytes = transaction.read(regions[index + 6].buffer, EXTENTS[index + 6] as u32)?;
            if index >= 3 {
                validate_cache_isolation(&initial_outputs[index], &bytes, slot)?;
            }
            observed.push(decode_words(&bytes, EXTENTS[index + 6] / 2)?);
        }
        let final_state = transaction.read_state(regions[11].buffer)?;
        validate_final_state(final_state)?;
        transaction.close()?;
        // Nothing fallible remains: publish every caller output only after close.
        for (output, words) in outputs.iter_mut().zip(&observed) {
            output.copy_from_slice(words);
        }
        Ok(Gfx950EngineeringWaveQkvPostTasksResultV3 {
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
}

impl NativeTransaction {
    fn region(&self, buffer: u64) -> Result<OwnedRegion> {
        let allocation = self
            .context
            .buffers
            .get(&buffer)
            .ok_or("unknown wave QKV post tasks buffer")?;
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
            _ => Err("wave QKV post tasks load response".into()),
        }
    }

    fn allocate(&mut self, role: usize) -> Result<OwnedRegion> {
        if role < 11 {
            return match self.context.allocate(EXTENTS[role] as u64)? {
                ResponseV1::Allocated { buffer, .. } => self.region(buffer),
                _ => Err("wave QKV post tasks allocation response".into()),
            };
        }
        if role != 11 {
            return Err("wave QKV post tasks allocation role".into());
        }
        self.context.check_idle()?;
        if self.context.buffers.len() >= MAX_ALLOCATIONS {
            return Err("live user allocation limit".into());
        }
        let buffer = self.context.next_buffer;
        self.context.next_buffer = buffer.checked_add(1).ok_or("buffer ID exhausted")?;
        let allocation = self.context.allocate_resource(
            80,
            KfdAllocMemoryFlags::HOST_VISIBLE_COHERENT,
            |_| Ok(()),
        )?;
        self.context.buffers.insert(buffer, allocation);
        self.region(buffer)
    }

    fn upload(&mut self, buffer: u64, bytes: &[u8]) -> Result<()> {
        self.context.write(buffer, 0, bytes)
    }

    fn initialize_state(&mut self, buffer: u64) -> Result<[u32; 20]> {
        self.context.check_idle()?;
        let allocation = self
            .context
            .buffers
            .get_mut(&buffer)
            .ok_or("unknown wave state buffer")?;
        if allocation.requested != 80 {
            return Err("wave state extent".into());
        }
        Backend::initialize_engineering_wave_qkv_post_task_state_v3(&mut allocation.mapping)
            .map_err(explain)?;
        let state =
            Backend::observe_engineering_wave_qkv_post_task_state_v3(&mut allocation.mapping)
                .map_err(explain)?;
        self.context.check_currentness(false)?;
        Ok(state)
    }

    fn prepare(
        &mut self,
        regions: &[OwnedRegion; 12],
        pointers: &[PointerFixupV1; 12],
    ) -> Result<Self::Prepared> {
        validate_regions(regions, pointers)?;
        for region in regions {
            if self.region(region.buffer)? != *region {
                return Err("wave QKV post tasks allocation changed".into());
            }
        }
        validate_geometry(WORKGROUP, GRID)?;
        self.context.prepare_dispatch(
            self.kernel.ok_or("wave QKV post tasks kernel missing")?,
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
        self.context.read(buffer, 0, bytes)
    }

    fn read_state(&mut self, buffer: u64) -> Result<[u32; 20]> {
        self.context.check_idle()?;
        let allocation = self
            .context
            .buffers
            .get_mut(&buffer)
            .ok_or("unknown wave state buffer")?;
        if allocation.requested != 80 {
            return Err("wave state extent".into());
        }
        let state =
            Backend::observe_engineering_wave_qkv_post_task_state_v3(&mut allocation.mapping)
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
#[path = "engineering_gfx950_wave_qkv_post_tasks_v3_tests.rs"]
mod tests;
