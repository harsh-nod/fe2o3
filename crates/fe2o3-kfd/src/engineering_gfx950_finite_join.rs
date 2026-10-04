//! Single-context engineering execution for the fixed fresh finite-join worker.
//! This is not protected runtime admission or an atomic-coherence issuer.

use super::*;

const EXTENTS: [usize; 3] = [1024, 1536, 24];
const INITIAL_STATE: [u32; 6] = [1, 3, 0, 0, 0, 0];
const WORKGROUP: [u16; 3] = [128, 1, 1];
const GRID: [u32; 3] = [256, 1, 1];
const KERNARG_BYTES: usize = 24 + 256;

/// Observations from one completed and successfully torn-down engineering call.
/// No allocation, launch, coherence, or protected capability is returned.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Gfx950EngineeringFiniteJoinResultV1 {
    pub final_state: [u32; 6],
    /// Existing queue timer: after ring reservation, before kernarg copy and
    /// publication, through confirmed completion and idle check. Excludes
    /// opening, loading, allocation, uploads, readback, and teardown.
    pub dispatch_elapsed_ns: u64,
}

/// Executes exactly one fresh epoch-1 finite join in one disposable context.
/// Caller payload is changed only after completion, validation, and full close.
///
/// # Safety
/// Call only in a dedicated disposable process with no unrelated GPU work or
/// shared Rust memory. Authorize this device and independently bind the exact
/// object digest and symbol to the compiled fixed finite-worker source and
/// grouped descriptor: input256, payload384, six AtomicU32 words, epoch1,
/// two workgroups of128, and the bounded {0,1}->2 protocol. The code must honor
/// all extents, accesses, initialization, and the COV6 ABI. Metadata alone does
/// not prove source provenance, cross-workgroup atomic coherence or visibility.
/// The caller must independently establish those obligations for this device
/// and HOST_VISIBLE_COHERENT state allocation; this function cannot issue them.
/// Any error is terminal: immediately terminate the disposable process. Live
/// or uncertain mappings are quarantined, never reclaimed on observed state
/// alone or on a timeout. No protected or XGMI authority is granted.
///
/// ```compile_fail
/// fe2o3_kfd::execute_gfx950_engineering_finite_join_unchecked_v1(
///     1, vec![], [0;32], "worker".into(), &[0.;256], &mut [0.;384], 100,
/// ).unwrap();
/// ```
pub unsafe fn execute_gfx950_engineering_finite_join_unchecked_v1(
    unique_id: u64,
    object: Vec<u8>,
    object_sha256: [u8; 32],
    symbol: String,
    input: &[f32; 256],
    payload: &mut [f32; 384],
    timeout_ms: u32,
) -> Result<Gfx950EngineeringFiniteJoinResultV1> {
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
    // SAFETY: the public caller accepts the precise trusted-code/disposable-
    // process obligations above. The coordinator retains all roots through close.
    unsafe {
        coordinate(
            transaction,
            object,
            object_sha256,
            symbol,
            input,
            payload,
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
        return Err("finite join object, symbol, digest or timeout".into());
    }
    Ok(())
}

fn validate_metadata(metadata: &KernelMetadataV1, digest: [u8; 32], symbol: &str) -> Result<()> {
    if metadata.object_sha256 != digest
        || metadata.symbol != symbol
        || metadata.kernarg_bytes as usize != KERNARG_BYTES
        || metadata.kernarg_alignment != 8
        || metadata.wavefront_size != 64
        || metadata.private_segment_bytes != 0
        || metadata.implicit_argument_offset != Some(24)
        || metadata.implicit_argument_bytes != 256
        || metadata.explicit_arguments.len() != 3
    {
        return Err("finite join physical ABI/resources".into());
    }
    for (index, argument) in metadata.explicit_arguments.iter().enumerate() {
        if argument.offset != index as u32 * 8
            || argument.bytes != 8
            || !argument.global_buffer
            || argument
                .pointee_alignment
                .is_some_and(|alignment| alignment != 4)
            || argument
                .access
                .is_some_and(|access| access != role_access(index))
        {
            return Err("finite join pointer role/offset/alignment".into());
        }
    }
    Ok(())
}

fn role_access(index: usize) -> BufferAccessV1 {
    if index == 0 {
        BufferAccessV1::Read
    } else {
        BufferAccessV1::ReadWrite
    }
}

// These are snapshots of this Context's existing buffer records, not a second
// allocation identity or any caller-issued address. They never leave this module.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct OwnedRegion {
    buffer: u64,
    base: u64,
    requested: usize,
    backing: usize,
}

fn fixups(regions: &[OwnedRegion; 3]) -> [PointerFixupV1; 3] {
    core::array::from_fn(|index| PointerFixupV1 {
        kernarg_offset: index as u32 * 8,
        buffer: regions[index].buffer,
        buffer_offset: 0,
        extent_bytes: EXTENTS[index] as u64,
        access: role_access(index),
    })
}

fn validate_regions(regions: &[OwnedRegion; 3], pointers: &[PointerFixupV1; 3]) -> Result<()> {
    for (index, region) in regions.iter().enumerate() {
        if region.buffer == 0
            || region.base == 0
            || !region.base.is_multiple_of(PAGE_BYTES as u64)
            || region.requested != EXTENTS[index]
            || region.backing < region.requested
            || !region.backing.is_multiple_of(PAGE_BYTES)
            || region.base.checked_add(region.backing as u64).is_none()
            || pointers[index] != fixups(regions)[index]
        {
            return Err("finite join owned extent/fixup".into());
        }
        for prior in &regions[..index] {
            if prior.buffer == region.buffer
                || (prior.base < region.base + region.backing as u64
                    && region.base < prior.base + prior.backing as u64)
            {
                return Err("finite join allocation alias".into());
            }
        }
    }
    Ok(())
}

fn validate_geometry(workgroup: [u16; 3], grid: [u32; 3]) -> Result<()> {
    if workgroup != WORKGROUP || grid != GRID {
        return Err("finite join requires exactly two workgroups of128".into());
    }
    Ok(())
}

fn validate_final_state(state: [u32; 6]) -> Result<()> {
    if state[0] != 1
        || state[1] != 0
        || state[2] != 7
        || state[3] != 7
        || state[4] & !63 != 0
        || state[5] != 0
        || (0..3).any(|task| !matches!((state[4] >> (2 * task)) & 3, 1 | 2))
    {
        return Err(format!(
            "finite join incomplete/invalid final state: {state:?}"
        ));
    }
    Ok(())
}

// A private adapter around existing Context operations, shared with CPU failure
// injection. It introduces no native protocol, persistent session or public API.
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
    fn initialize_state(&mut self, buffer: u64) -> Result<[u32; 6]>;
    fn prepare(
        &mut self,
        regions: &[OwnedRegion; 3],
        pointers: &[PointerFixupV1; 3],
    ) -> Result<Self::Prepared>;
    unsafe fn submit(&mut self, prepared: Self::Prepared, timeout_ms: u32)
    -> Result<Self::Pending>;
    fn complete(&mut self, pending: Self::Pending) -> Result<u64>;
    fn read(&mut self, buffer: u64, bytes: u32) -> Result<Vec<u8>>;
    fn read_state(&mut self, buffer: u64) -> Result<[u32; 6]>;
    fn close(&mut self) -> Result<()>;
    fn quarantine(self);
}

unsafe fn coordinate<T: Transaction>(
    mut transaction: T,
    object: Vec<u8>,
    digest: [u8; 32],
    symbol: String,
    input: &[f32; 256],
    payload: &mut [f32; 384],
    timeout_ms: u32,
) -> Result<Gfx950EngineeringFiniteJoinResultV1> {
    let result = (|| {
        validate_request(&object, digest, &symbol, timeout_ms)?;
        let metadata = transaction.load(object, digest, symbol.clone())?;
        validate_metadata(&metadata, digest, &symbol)?;
        let regions = [
            transaction.allocate(0)?,
            transaction.allocate(1)?,
            transaction.allocate(2)?,
        ];
        let pointers = fixups(&regions);
        validate_regions(&regions, &pointers)?;
        let input_bytes: Vec<u8> = input
            .iter()
            .flat_map(|value| value.to_bits().to_le_bytes())
            .collect();
        let payload_bytes: Vec<u8> = payload
            .iter()
            .flat_map(|value| value.to_bits().to_le_bytes())
            .collect();
        transaction.upload(regions[0].buffer, &input_bytes)?;
        transaction.upload(regions[1].buffer, &payload_bytes)?;
        if transaction.initialize_state(regions[2].buffer)? != INITIAL_STATE {
            return Err("finite join state initialization".into());
        }
        validate_geometry(WORKGROUP, GRID)?;
        let prepared = transaction.prepare(&regions, &pointers)?;
        // SAFETY: inherited trusted-code contract; the same owned transaction
        // retains code, queue, kernarg and all three roots until completion.
        let pending = unsafe { transaction.submit(prepared, timeout_ms) }?;
        let dispatch_elapsed_ns = transaction.complete(pending)?;
        if transaction.read(regions[0].buffer, EXTENTS[0] as u32)? != input_bytes {
            return Err("finite join input changed".into());
        }
        let output_bytes = transaction.read(regions[1].buffer, EXTENTS[1] as u32)?;
        if output_bytes.len() != EXTENTS[1] {
            return Err("finite join payload readback extent".into());
        }
        let final_state = transaction.read_state(regions[2].buffer)?;
        validate_final_state(final_state)?;
        let output: [f32; 384] = core::array::from_fn(|index| {
            let start = index * 4;
            f32::from_bits(u32::from_le_bytes([
                output_bytes[start],
                output_bytes[start + 1],
                output_bytes[start + 2],
                output_bytes[start + 3],
            ]))
        });
        transaction.close()?;
        *payload = output;
        Ok(Gfx950EngineeringFiniteJoinResultV1 {
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
            .ok_or("unknown finite join buffer")?;
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
            _ => Err("finite join load response".into()),
        }
    }

    fn allocate(&mut self, role: usize) -> Result<OwnedRegion> {
        if role < 2 {
            return match self.context.allocate(EXTENTS[role] as u64)? {
                ResponseV1::Allocated { buffer, .. } => self.region(buffer),
                _ => Err("finite join allocation response".into()),
            };
        }
        if role != 2 {
            return Err("finite join allocation role".into());
        }
        self.context.check_idle()?;
        if self.context.buffers.len() >= MAX_ALLOCATIONS {
            return Err("live user allocation limit".into());
        }
        let buffer = self.context.next_buffer;
        self.context.next_buffer = buffer.checked_add(1).ok_or("buffer ID exhausted")?;
        let allocation = self.context.allocate_resource(
            24,
            KfdAllocMemoryFlags::HOST_VISIBLE_COHERENT,
            |_| Ok(()),
        )?;
        self.context.buffers.insert(buffer, allocation);
        self.region(buffer)
    }

    fn upload(&mut self, buffer: u64, bytes: &[u8]) -> Result<()> {
        self.context.write(buffer, 0, bytes)
    }

    fn initialize_state(&mut self, buffer: u64) -> Result<[u32; 6]> {
        self.context.check_idle()?;
        let allocation = self
            .context
            .buffers
            .get_mut(&buffer)
            .ok_or("unknown state buffer")?;
        if allocation.requested != 24 {
            return Err("finite join state extent".into());
        }
        Backend::initialize_engineering_finite_join_state(&mut allocation.mapping)
            .map_err(explain)?;
        let state = Backend::observe_engineering_finite_join_state(&mut allocation.mapping)
            .map_err(explain)?;
        self.context.check_currentness(false)?;
        Ok(state)
    }

    fn prepare(
        &mut self,
        regions: &[OwnedRegion; 3],
        pointers: &[PointerFixupV1; 3],
    ) -> Result<Self::Prepared> {
        validate_regions(regions, pointers)?;
        for region in regions {
            if self.region(region.buffer)? != *region {
                return Err("finite join allocation changed".into());
            }
        }
        validate_geometry(WORKGROUP, GRID)?;
        self.context.prepare_dispatch(
            self.kernel.ok_or("finite join kernel missing")?,
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
        // SAFETY: same caller contract as the public one-shot; no second submission.
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

    fn read_state(&mut self, buffer: u64) -> Result<[u32; 6]> {
        self.context.check_idle()?;
        let allocation = self
            .context
            .buffers
            .get_mut(&buffer)
            .ok_or("unknown state buffer")?;
        if allocation.requested != 24 {
            return Err("finite join state extent".into());
        }
        let state = Backend::observe_engineering_finite_join_state(&mut allocation.mapping)
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
#[path = "engineering_gfx950_finite_join_tests.rs"]
mod tests;
