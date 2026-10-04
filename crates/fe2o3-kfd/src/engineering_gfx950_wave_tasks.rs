//! One-shot six-root engineering execution; no protected or coherence authority.

use super::*;

const EXTENTS: [usize; 6] = [8192, 8192, 4_194_304, 8192, 1024, 36];
const INITIAL_STATE: [u32; 9] = [1, 1, 0, 0, 0, 0, 0, 0, 0];
const WORKGROUP: [u16; 3] = [64, 1, 1];
const GRID: [u32; 3] = [128, 1, 1];
const KERNARG_BYTES: usize = 48 + 256;

/// Observations after one completed dispatch and successful context teardown.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Gfx950EngineeringWaveTasksResultV1 {
    /// Epoch, ready, claimed, done, owners, error, and three arrival counters.
    pub final_state: [u32; 9],
    /// Existing queue timer: after ring reservation, before kernarg copy and
    /// publication, through confirmed completion and idle check. Excludes
    /// context opening, loading, allocation, upload, readback, and teardown.
    pub dispatch_elapsed_ns: u64,
}

/// Execute the fixed Norm4096 -> {K0..256, K256..512} engineering checkpoint.
/// Both caller outputs remain unchanged unless completion and teardown succeed.
/// This does not compare numerical outputs or grant production launch authority.
///
/// # Safety
/// Use only a dedicated disposable process with no unrelated GPU work or shared
/// Rust memory. Independently authorize the device and bind the object digest
/// and symbol to the exact compiled six-root source and descriptor. It must use
/// the declared BF16 policy, two Wave64 workgroups, all-lane task completion,
/// fresh epoch1, and the bounded Norm0 -> {Key1, Key2} protocol. The caller must
/// establish actual source/ISA, System-atomic/payload coherence and lifetime
/// obligations; neither metadata nor HOST_VISIBLE_COHERENT proves them. Actual
/// LDS requirements must pass the existing loader and independent image review;
/// this path does not guess them or force them to zero. Any error is terminal:
/// immediately terminate the disposable process. Live or uncertain mappings
/// are quarantined; observed state never substitutes for queue completion.
/// No protected, reuse, peer-memory, or XGMI authority is returned.
///
/// ```compile_fail
/// let input = [0u16; 4096];
/// let weights = vec![0u16; 2_097_152].into_boxed_slice();
/// let weights: &[u16; 2_097_152] = weights.as_ref().try_into().unwrap();
/// fe2o3_kfd::execute_gfx950_engineering_wave_tasks_unchecked_v1(
///     1, vec![], [0; 32], "worker".into(), &input, &input, weights,
///     &mut [0u16; 4096], &mut [0u16; 512], 100,
/// ).unwrap();
/// ```
#[allow(clippy::too_many_arguments)]
pub unsafe fn execute_gfx950_engineering_wave_tasks_unchecked_v1(
    unique_id: u64,
    object: Vec<u8>,
    object_sha256: [u8; 32],
    symbol: String,
    input: &[u16; 4096],
    norm_weight: &[u16; 4096],
    key_weight: &[u16; 2_097_152],
    normalized: &mut [u16; 4096],
    key_output: &mut [u16; 512],
    timeout_ms: u32,
) -> Result<Gfx950EngineeringWaveTasksResultV1> {
    validate_request(&object, object_sha256, &symbol, timeout_ms)?;
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
    // SAFETY: inherited disposable-process/source/coherence contract. The same
    // transaction owns code, queue, kernarg and all six roots through close.
    unsafe {
        coordinate(
            transaction,
            object,
            object_sha256,
            symbol,
            [
                input.as_slice(),
                norm_weight.as_slice(),
                key_weight.as_slice(),
            ],
            normalized,
            key_output,
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
        return Err("wave tasks object, symbol, digest or timeout".into());
    }
    Ok(())
}

fn role_access(index: usize) -> BufferAccessV1 {
    if index < 3 {
        BufferAccessV1::Read
    } else {
        BufferAccessV1::ReadWrite
    }
}

fn role_alignment(index: usize) -> u32 {
    if index == 5 { 4 } else { 2 }
}

fn validate_metadata(metadata: &KernelMetadataV1, digest: [u8; 32], symbol: &str) -> Result<()> {
    if metadata.object_sha256 != digest
        || metadata.symbol != symbol
        || metadata.kernarg_bytes as usize != KERNARG_BYTES
        || metadata.kernarg_alignment != 8
        || metadata.wavefront_size != 64
        || metadata.private_segment_bytes != 0
        || metadata.group_segment_bytes > 160 * 1024
        || metadata.implicit_argument_offset != Some(48)
        || metadata.implicit_argument_bytes != 256
        || metadata.explicit_arguments.len() != 6
    {
        return Err("wave tasks physical ABI/resources".into());
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
            return Err("wave tasks pointer role/offset/alignment".into());
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

fn fixups(regions: &[OwnedRegion; 6]) -> [PointerFixupV1; 6] {
    core::array::from_fn(|index| PointerFixupV1 {
        kernarg_offset: index as u32 * 8,
        buffer: regions[index].buffer,
        buffer_offset: 0,
        extent_bytes: EXTENTS[index] as u64,
        access: role_access(index),
    })
}

fn validate_regions(regions: &[OwnedRegion; 6], pointers: &[PointerFixupV1; 6]) -> Result<()> {
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
            return Err("wave tasks owned extent/fixup".into());
        }
        for prior in &regions[..index] {
            if prior.buffer == region.buffer
                || (prior.base < region.base + region.backing as u64
                    && region.base < prior.base + prior.backing as u64)
            {
                return Err("wave tasks allocation alias".into());
            }
        }
    }
    Ok(())
}

fn validate_geometry(workgroup: [u16; 3], grid: [u32; 3]) -> Result<()> {
    if workgroup != WORKGROUP || grid != GRID {
        return Err("wave tasks require exactly two Wave64 workgroups".into());
    }
    Ok(())
}

fn validate_final_state(state: [u32; 9]) -> Result<()> {
    if state[0] != 1
        || state[1] != 0
        || state[2] != 7
        || state[3] != 7
        || state[4] & !63 != 0
        || state[5] != 0
        || state[6..] != [64, 64, 64]
        || (0..3).any(|task| !matches!((state[4] >> (2 * task)) & 3, 1 | 2))
    {
        return Err(format!(
            "wave tasks incomplete/invalid final state: {state:?}"
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
    fn initialize_state(&mut self, buffer: u64) -> Result<[u32; 9]>;
    fn prepare(
        &mut self,
        regions: &[OwnedRegion; 6],
        pointers: &[PointerFixupV1; 6],
    ) -> Result<Self::Prepared>;
    unsafe fn submit(&mut self, prepared: Self::Prepared, timeout_ms: u32)
    -> Result<Self::Pending>;
    fn complete(&mut self, pending: Self::Pending) -> Result<u64>;
    fn read(&mut self, buffer: u64, bytes: u32) -> Result<Vec<u8>>;
    fn read_state(&mut self, buffer: u64) -> Result<[u32; 9]>;
    fn close(&mut self) -> Result<()>;
    fn quarantine(self);
}

fn encode_words(words: &[u16]) -> Vec<u8> {
    words.iter().flat_map(|word| word.to_le_bytes()).collect()
}

fn decode_words<const N: usize>(bytes: &[u8]) -> Result<[u16; N]> {
    if bytes.len() != N * 2 {
        return Err("wave tasks output extent".into());
    }
    Ok(core::array::from_fn(|index| {
        u16::from_le_bytes([bytes[index * 2], bytes[index * 2 + 1]])
    }))
}

#[allow(clippy::too_many_arguments)]
unsafe fn coordinate<T: Transaction>(
    mut transaction: T,
    object: Vec<u8>,
    digest: [u8; 32],
    symbol: String,
    inputs: [&[u16]; 3],
    normalized: &mut [u16; 4096],
    key_output: &mut [u16; 512],
    timeout_ms: u32,
) -> Result<Gfx950EngineeringWaveTasksResultV1> {
    let result = (|| {
        validate_request(&object, digest, &symbol, timeout_ms)?;
        if inputs
            .iter()
            .enumerate()
            .any(|(index, words)| words.len() != EXTENTS[index] / 2)
        {
            return Err("wave tasks input extent".into());
        }
        let metadata = transaction.load(object, digest, symbol.clone())?;
        validate_metadata(&metadata, digest, &symbol)?;
        let regions = [
            transaction.allocate(0)?,
            transaction.allocate(1)?,
            transaction.allocate(2)?,
            transaction.allocate(3)?,
            transaction.allocate(4)?,
            transaction.allocate(5)?,
        ];
        let pointers = fixups(&regions);
        validate_regions(&regions, &pointers)?;
        let input_bytes = inputs.map(encode_words);
        for index in 0..3 {
            transaction.upload(regions[index].buffer, &input_bytes[index])?;
        }
        transaction.upload(regions[3].buffer, &encode_words(normalized))?;
        transaction.upload(regions[4].buffer, &encode_words(key_output))?;
        if transaction.initialize_state(regions[5].buffer)? != INITIAL_STATE {
            return Err("wave tasks state initialization".into());
        }
        validate_geometry(WORKGROUP, GRID)?;
        let prepared = transaction.prepare(&regions, &pointers)?;
        // SAFETY: same one-shot contract; the transaction retains all live roots.
        let pending = unsafe { transaction.submit(prepared, timeout_ms) }?;
        let dispatch_elapsed_ns = transaction.complete(pending)?;
        for index in 0..3 {
            if transaction.read(regions[index].buffer, EXTENTS[index] as u32)? != input_bytes[index]
            {
                return Err("wave tasks read-only input changed".into());
            }
        }
        let norm_bytes = transaction.read(regions[3].buffer, EXTENTS[3] as u32)?;
        let key_bytes = transaction.read(regions[4].buffer, EXTENTS[4] as u32)?;
        let final_state = transaction.read_state(regions[5].buffer)?;
        validate_final_state(final_state)?;
        let norm = decode_words::<4096>(&norm_bytes)?;
        let key = decode_words::<512>(&key_bytes)?;
        transaction.close()?;
        *normalized = norm;
        *key_output = key;
        Ok(Gfx950EngineeringWaveTasksResultV1 {
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
            .ok_or("unknown wave tasks buffer")?;
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
            _ => Err("wave tasks load response".into()),
        }
    }

    fn allocate(&mut self, role: usize) -> Result<OwnedRegion> {
        if role < 5 {
            return match self.context.allocate(EXTENTS[role] as u64)? {
                ResponseV1::Allocated { buffer, .. } => self.region(buffer),
                _ => Err("wave tasks allocation response".into()),
            };
        }
        if role != 5 {
            return Err("wave tasks allocation role".into());
        }
        self.context.check_idle()?;
        if self.context.buffers.len() >= MAX_ALLOCATIONS {
            return Err("live user allocation limit".into());
        }
        let buffer = self.context.next_buffer;
        self.context.next_buffer = buffer.checked_add(1).ok_or("buffer ID exhausted")?;
        let allocation = self.context.allocate_resource(
            36,
            KfdAllocMemoryFlags::HOST_VISIBLE_COHERENT,
            |_| Ok(()),
        )?;
        self.context.buffers.insert(buffer, allocation);
        self.region(buffer)
    }

    fn upload(&mut self, buffer: u64, bytes: &[u8]) -> Result<()> {
        self.context.write(buffer, 0, bytes)
    }

    fn initialize_state(&mut self, buffer: u64) -> Result<[u32; 9]> {
        self.context.check_idle()?;
        let allocation = self
            .context
            .buffers
            .get_mut(&buffer)
            .ok_or("unknown wave state buffer")?;
        if allocation.requested != 36 {
            return Err("wave state extent".into());
        }
        Backend::initialize_engineering_wave_task_state(&mut allocation.mapping)
            .map_err(explain)?;
        let state = Backend::observe_engineering_wave_task_state(&mut allocation.mapping)
            .map_err(explain)?;
        self.context.check_currentness(false)?;
        Ok(state)
    }

    fn prepare(
        &mut self,
        regions: &[OwnedRegion; 6],
        pointers: &[PointerFixupV1; 6],
    ) -> Result<Self::Prepared> {
        validate_regions(regions, pointers)?;
        for region in regions {
            if self.region(region.buffer)? != *region {
                return Err("wave tasks allocation changed".into());
            }
        }
        validate_geometry(WORKGROUP, GRID)?;
        self.context.prepare_dispatch(
            self.kernel.ok_or("wave tasks kernel missing")?,
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

    fn read_state(&mut self, buffer: u64) -> Result<[u32; 9]> {
        self.context.check_idle()?;
        let allocation = self
            .context
            .buffers
            .get_mut(&buffer)
            .ok_or("unknown wave state buffer")?;
        if allocation.requested != 36 {
            return Err("wave state extent".into());
        }
        let state = Backend::observe_engineering_wave_task_state(&mut allocation.mapping)
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
#[path = "engineering_gfx950_wave_tasks_tests.rs"]
mod tests;
