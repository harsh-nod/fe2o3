//! Explicit one-process engineering peer owner. This is not protected XGMI authority.

use super::*;
use crate::memory::KernelOutcome;
use crate::topology::{GfxTarget, HostTopologySnapshot, KfdTopologyLinkSetV1};
use std::sync::atomic::{AtomicU64, Ordering};

#[path = "engineering_gfx950_peer_performance.rs"]
mod performance;
pub use performance::Gfx950EngineeringPeerDispatchV1;

static NEXT_GROUP: AtomicU64 = AtomicU64::new(1);
const LINK_ENABLED: u32 = 1;
const LINK_NO_ATOMICS: u32 = (1 << 2) | (1 << 3);

fn group_allocation_limit(world: usize) -> Result<usize> {
    if !matches!(world, 2 | 8) {
        return Err("invalid group allocation world size".into());
    }
    MAX_ALLOCATIONS
        .checked_mul(world)
        .ok_or_else(|| "group allocation limit overflow".into())
}

/// Group-scoped identity, never a pointer or native allocation handle.
///
/// ```compile_fail
/// let _ = fe2o3_kfd::Gfx950EngineeringPeerBufferV1 {
///     group: 1, id: 1, owner: 0, bytes: 4096,
/// };
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx950EngineeringPeerBufferV1 {
    group: u64,
    id: u64,
    owner: usize,
    bytes: u64,
}

impl Gfx950EngineeringPeerBufferV1 {
    pub const fn bytes(self) -> u64 {
        self.bytes
    }
    pub const fn owner_rank(self) -> usize {
        self.owner
    }

    /// Describes an argument using this retained group identity, not a GPU VA.
    pub fn pointer(
        self,
        kernarg_offset: u32,
        buffer_offset: u64,
        extent_bytes: u64,
        access: BufferAccessV1,
    ) -> Gfx950EngineeringPeerPointerV1 {
        Gfx950EngineeringPeerPointerV1 {
            buffer: self,
            kernarg_offset,
            buffer_offset,
            extent_bytes,
            access,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Gfx950EngineeringPeerPointerV1 {
    buffer: Gfx950EngineeringPeerBufferV1,
    kernarg_offset: u32,
    buffer_offset: u64,
    extent_bytes: u64,
    access: BufferAccessV1,
}

pub struct Gfx950EngineeringPeerKernelV1 {
    group: u64,
    rank: usize,
    id: u64,
    metadata: KernelMetadataV1,
}

impl Gfx950EngineeringPeerKernelV1 {
    pub fn metadata(&self) -> &KernelMetadataV1 {
        &self.metadata
    }
    pub const fn rank(&self) -> usize {
        self.rank
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    OwnerMapped,
    PeersMapped,
    PeersUnmapped,
    Released,
    Quarantined,
}

struct PeerMapping {
    peers: Vec<u32>,
    mapped: u32,
    unmapped: u32,
    phase: Phase,
}

trait PeerTransactionBackend {
    fn check(&mut self) -> Result<()>;
    fn map(&mut self, peers: &[u32]) -> KernelOutcome<u32>;
    fn unmap(&mut self, peers: &[u32]) -> KernelOutcome<u32>;
    fn release_owner(&mut self) -> Result<()>;
}

impl PeerMapping {
    fn new(peers: Vec<u32>) -> Result<Self> {
        if peers.len() > 7 || peers.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err("peer mapping roster is not canonical".into());
        }
        Ok(Self {
            peers,
            mapped: 0,
            unmapped: 0,
            phase: Phase::OwnerMapped,
        })
    }

    fn map(&mut self, backend: &mut impl PeerTransactionBackend) -> Result<()> {
        if self.phase != Phase::OwnerMapped {
            return Err("peer mapping is not fresh".into());
        }
        let result = (|| {
            backend.check()?;
            if !self.peers.is_empty() {
                let outcome = backend.map(&self.peers);
                self.mapped = outcome.value;
                outcome.result.map_err(explain)?;
                if self.mapped as usize != self.peers.len() {
                    return Err("incomplete peer mapping".into());
                }
            }
            backend.check()?;
            self.phase = Phase::PeersMapped;
            Ok(())
        })();
        if result.is_err() {
            self.phase = Phase::Quarantined;
        }
        result
    }

    fn release(&mut self, backend: &mut impl PeerTransactionBackend) -> Result<()> {
        if self.phase != Phase::PeersMapped {
            return Err("peer mapping cannot be released".into());
        }
        let result = (|| {
            backend.check()?;
            if !self.peers.is_empty() {
                let outcome = backend.unmap(&self.peers);
                self.unmapped = outcome.value;
                outcome.result.map_err(explain)?;
                if self.unmapped as usize != self.peers.len() {
                    return Err("incomplete peer unmapping".into());
                }
            }
            self.phase = Phase::PeersUnmapped;
            backend.check()?;
            backend.release_owner()?;
            self.phase = Phase::Released;
            Ok(())
        })();
        if result.is_err() {
            self.phase = Phase::Quarantined;
        }
        result
    }
}

struct BufferRecord {
    token: Gfx950EngineeringPeerBufferV1,
    local_id: u64,
    mapping: PeerMapping,
}

struct NativeTransaction<'a> {
    contexts: &'a mut [Context],
    owner: usize,
    local_id: u64,
}

fn check_contexts(contexts: &mut [Context]) -> Result<()> {
    for context in contexts {
        context.check_currentness(true)?;
        context.check_idle()?;
    }
    Ok(())
}

impl PeerTransactionBackend for NativeTransaction<'_> {
    fn check(&mut self) -> Result<()> {
        check_contexts(self.contexts)
    }
    fn map(&mut self, peers: &[u32]) -> KernelOutcome<u32> {
        let context = &mut self.contexts[self.owner];
        let handle = context.buffers[&self.local_id].handle;
        context.backend.map_gpu_ids(handle, peers, 0)
    }
    fn unmap(&mut self, peers: &[u32]) -> KernelOutcome<u32> {
        let context = &mut self.contexts[self.owner];
        let handle = context.buffers[&self.local_id].handle;
        context.backend.unmap_gpu_ids(handle, peers, 0)
    }
    fn release_owner(&mut self) -> Result<()> {
        self.contexts[self.owner].free(self.local_id)
    }
}

#[derive(Clone, Copy, Debug)]
struct RouteFacts {
    targets: [GfxTarget; 2],
    gpus: [u32; 2],
    hives: [u64; 2],
    node_from: u32,
    node_to: u32,
    source_node: u32,
    destination_node: u32,
    io: bool,
    link_type: u32,
    flags: u32,
    bandwidth: u64,
    directional_links: usize,
}

fn validate_route(facts: RouteFacts) -> Result<()> {
    let rejection = if facts.targets != [GfxTarget::Gfx950; 2] {
        Some("target is not gfx950")
    } else if facts.gpus[0] == facts.gpus[1] {
        Some("GPU identities are not distinct")
    } else if facts.hives[0] == 0 || facts.hives[0] != facts.hives[1] {
        Some("XGMI hive is zero or differs")
    } else if facts.directional_links != 1 {
        Some("directional link is missing or ambiguous")
    } else if !facts.io {
        Some("link is not an IO observation")
    } else if facts.node_from != facts.source_node || facts.node_to != facts.destination_node {
        Some("link direction does not match endpoints")
    } else if facts.link_type != 11 {
        Some("link type is not XGMI")
    } else if facts.flags & !LINK_NO_ATOMICS != LINK_ENABLED {
        Some("link is disabled, noncoherent, peer-disabled, or has unknown flags")
    } else {
        None
    };
    if let Some(reason) = rejection {
        return Err(format!(
            "gfx950 compute peer route rejected: {reason}; nodes {}->{} flags {:#x} reported_max_bandwidth {}",
            facts.source_node, facts.destination_node, facts.flags, facts.bandwidth,
        ));
    }
    // KFD can publish an enabled XGMI route with unreported (zero) bandwidth.
    // This metric grants no accessibility, coherence, SDMA, or throughput claim.
    Ok(())
}

fn validate_routes(snapshot: &HostTopologySnapshot, gpu_ids: &[u32]) -> Result<()> {
    for &source_id in gpu_ids {
        for &destination_id in gpu_ids {
            if source_id == destination_id {
                continue;
            }
            let source = snapshot
                .topology()
                .gpu_nodes()
                .iter()
                .find(|gpu| gpu.gpu_id() == u64::from(source_id))
                .ok_or("missing peer source")?;
            let destination = snapshot
                .topology()
                .gpu_nodes()
                .iter()
                .find(|gpu| gpu.gpu_id() == u64::from(destination_id))
                .ok_or("missing peer destination")?;
            let links = source
                .io_links()
                .iter()
                .filter(|link| link.node_to() == destination.node_id())
                .collect::<Vec<_>>();
            let link = links
                .first()
                .ok_or("missing directional gfx950 peer link")?;
            validate_route(RouteFacts {
                targets: [source.target(), destination.target()],
                gpus: [source_id, destination_id],
                hives: [source.hive_id(), destination.hive_id()],
                node_from: link.node_from(),
                node_to: link.node_to(),
                source_node: source.node_id(),
                destination_node: destination.node_id(),
                io: link.set() == KfdTopologyLinkSetV1::Io,
                link_type: link.link_type(),
                flags: link.flags(),
                bandwidth: link.bandwidth().1,
                directional_links: links.len(),
            })?;
        }
    }
    Ok(())
}

fn checked_roster(unique_ids: &[u64]) -> Result<()> {
    if !matches!(unique_ids.len(), 2 | 8)
        || unique_ids.contains(&0)
        || unique_ids.iter().copied().collect::<BTreeSet<_>>().len() != unique_ids.len()
    {
        return Err("peer group requires two or eight unique physical devices".into());
    }
    Ok(())
}

fn require_peer_access(
    owner: usize,
    rank: usize,
    gpu_id: u32,
    peers: &[u32],
    access: BufferAccessV1,
) -> Result<()> {
    if rank != owner && (access != BufferAccessV1::Read || !peers.contains(&gpu_id)) {
        return Err("peer dispatch has no read-only mapping for this rank".into());
    }
    Ok(())
}

/// One serial, disposable-process owner of gfx950 contexts and peer mappings.
///
/// Peer mappings admit reads only in kernel argument binding. Owner-rank writes
/// and all host operations require complete group quiescence. No method returns
/// a native pointer/handle, and no protected gfx942 capability is created.
///
/// ```compile_fail
/// fn requires_send<T: Send>() {}
/// requires_send::<fe2o3_kfd::Gfx950EngineeringPeerGroupV1>();
/// ```
pub struct Gfx950EngineeringPeerGroupV1 {
    incarnation: u64,
    contexts: Vec<Context>,
    buffers: BTreeMap<u64, BufferRecord>,
    next_buffer: u64,
    poisoned: bool,
    closed: bool,
}

impl Gfx950EngineeringPeerGroupV1 {
    /// Opens a separate engineering group; does not change the v1 worker entry.
    ///
    /// # Safety
    /// The caller must use a dedicated disposable single-threaded process,
    /// with no independent KFD client or GPU access. Every failure is terminal:
    /// exit the process without attempting another GPU operation. Dropping an
    /// unclosed group retains native owners until process teardown.
    ///
    /// ```compile_fail
    /// fe2o3_kfd::Gfx950EngineeringPeerGroupV1::open_unchecked(&[1, 2]).unwrap();
    /// ```
    pub unsafe fn open_unchecked(unique_ids: &[u64]) -> Result<Self> {
        checked_roster(unique_ids)?;
        let incarnation = NEXT_GROUP
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(1)
            })
            .map_err(|_| "peer group identity exhausted")?;
        let mut group = Self {
            incarnation,
            contexts: Vec::new(),
            buffers: BTreeMap::new(),
            next_buffer: 1,
            poisoned: false,
            closed: false,
        };
        let result = (|| {
            for &unique_id in unique_ids {
                let device = OpenedKfd::open_default()
                    .map_err(explain)?
                    .admit_uapi()
                    .map_err(explain)?
                    .bind_gfx950_xnack_minus(DeviceSelector::UniqueId(unique_id))
                    .map_err(explain)?;
                group.contexts.push(Context::open(device)?);
            }
            check_contexts(&mut group.contexts)?;
            let snapshot = group.contexts[0].backend.engineering_peer_topology();
            if group
                .contexts
                .iter()
                .any(|context| context.backend.engineering_peer_topology() != snapshot)
            {
                return Err("peer group topology snapshots differ".into());
            }
            let gpu_ids = group
                .contexts
                .iter()
                .map(|context| context.backend.gpu_id())
                .collect::<Vec<_>>();
            if gpu_ids.iter().copied().collect::<BTreeSet<_>>().len() != gpu_ids.len() {
                return Err("duplicate peer KFD GPU identity".into());
            }
            validate_routes(snapshot, &gpu_ids)
        })();
        if let Err(error) = result {
            group.poisoned = true;
            return Err(error);
        }
        Ok(group)
    }

    fn require_active(&self) -> Result<()> {
        if self.closed || self.poisoned {
            return Err("peer group is closed or quarantined".into());
        }
        Ok(())
    }

    fn finish<T>(&mut self, result: Result<T>) -> Result<T> {
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }

    fn validate_token(&self, token: Gfx950EngineeringPeerBufferV1) -> Result<&BufferRecord> {
        let record = self
            .buffers
            .get(&token.id)
            .ok_or("unknown or retired group buffer")?;
        if token.group != self.incarnation
            || record.token != token
            || record.mapping.phase != Phase::PeersMapped
        {
            return Err("foreign or unavailable group buffer".into());
        }
        Ok(record)
    }

    /// Allocates PUBLIC VRAM on `owner`, mapping it read-only-by-binding on peers.
    /// Peer ranks must be distinct, exclude the owner, and belong to this group.
    pub fn allocate(
        &mut self,
        owner: usize,
        peers: &[usize],
        bytes: u64,
    ) -> Result<Gfx950EngineeringPeerBufferV1> {
        self.require_active()?;
        let result = (|| {
            if owner >= self.contexts.len()
                || self.buffers.len() >= group_allocation_limit(self.contexts.len())?
                || peers
                    .iter()
                    .any(|&peer| peer >= self.contexts.len() || peer == owner)
                || peers.iter().copied().collect::<BTreeSet<_>>().len() != peers.len()
            {
                return Err("invalid peer allocation roster".into());
            }
            check_contexts(&mut self.contexts)?;
            let id = self.next_buffer;
            self.next_buffer = id.checked_add(1).ok_or("peer buffer identity exhausted")?;
            let ResponseV1::Allocated {
                buffer: local_id, ..
            } = self.contexts[owner].allocate(bytes)?
            else {
                return Err("peer owner allocation response".into());
            };
            let allocation = &self.contexts[owner].buffers[&local_id];
            let end = allocation
                .va
                .checked_add(allocation.backing as u64 - 1)
                .ok_or("peer VA overflow")?;
            for &peer in peers {
                let aperture = self.contexts[peer].backend.gpuvm_aperture();
                if allocation.va < aperture.base() || end > aperture.limit() {
                    return Err("shared VA is outside peer aperture".into());
                }
            }
            let mut peer_ids = peers
                .iter()
                .map(|&peer| self.contexts[peer].backend.gpu_id())
                .collect::<Vec<_>>();
            peer_ids.sort_unstable();
            let token = Gfx950EngineeringPeerBufferV1 {
                group: self.incarnation,
                id,
                owner,
                bytes,
            };
            self.buffers.insert(
                id,
                BufferRecord {
                    token,
                    local_id,
                    mapping: PeerMapping::new(peer_ids)?,
                },
            );
            let record = self.buffers.get_mut(&id).ok_or("missing new peer record")?;
            record.mapping.map(&mut NativeTransaction {
                contexts: &mut self.contexts,
                owner,
                local_id,
            })?;
            Ok(token)
        })();
        self.finish(result)
    }

    pub fn write(
        &mut self,
        buffer: Gfx950EngineeringPeerBufferV1,
        offset: u64,
        bytes: &[u8],
    ) -> Result<()> {
        self.require_active()?;
        let result = (|| {
            if bytes.len() > MAX_TRANSFER_BYTES_V1 as usize {
                return Err("peer host write limit".into());
            }
            let record = self.validate_token(buffer)?;
            let local = record.local_id;
            check_contexts(&mut self.contexts)?;
            self.contexts[buffer.owner].write(local, offset, bytes)?;
            check_contexts(&mut self.contexts)
        })();
        self.finish(result)
    }

    pub fn read(
        &mut self,
        buffer: Gfx950EngineeringPeerBufferV1,
        offset: u64,
        bytes: u32,
    ) -> Result<Vec<u8>> {
        self.require_active()?;
        let result = (|| {
            if bytes > MAX_TRANSFER_BYTES_V1 {
                return Err("peer host read limit".into());
            }
            let local = self.validate_token(buffer)?.local_id;
            check_contexts(&mut self.contexts)?;
            let result = self.contexts[buffer.owner].read(local, offset, bytes)?;
            check_contexts(&mut self.contexts)?;
            Ok(result)
        })();
        self.finish(result)
    }

    pub fn load_kernel(
        &mut self,
        rank: usize,
        object: Vec<u8>,
        hash: [u8; 32],
        symbol: String,
    ) -> Result<Gfx950EngineeringPeerKernelV1> {
        self.require_active()?;
        let result = (|| {
            if rank >= self.contexts.len()
                || object.len() > MAX_OBJECT_BYTES_V1 as usize
                || symbol.len() > MAX_HEADER_BYTES_V1
            {
                return Err("kernel rank or object outside peer bounds".into());
            }
            check_contexts(&mut self.contexts)?;
            let ResponseV1::LoadedKernel { kernel, metadata } =
                self.contexts[rank].load(object, hash, symbol)?
            else {
                return Err("peer kernel admission response".into());
            };
            check_contexts(&mut self.contexts)?;
            Ok(Gfx950EngineeringPeerKernelV1 {
                group: self.incarnation,
                rank,
                id: kernel,
                metadata,
            })
        })();
        self.finish(result)
    }

    /// Runs one kernel synchronously, retaining every peer allocation and context.
    ///
    /// # Safety
    /// The unauthenticated machine code must obey its declared buffer access,
    /// bounds, and termination contract. This API does not prove kernel code.
    pub unsafe fn dispatch_unchecked(
        &mut self,
        kernel: &Gfx950EngineeringPeerKernelV1,
        bytes: Vec<u8>,
        workgroup: [u16; 3],
        grid: [u32; 3],
        pointers: &[Gfx950EngineeringPeerPointerV1],
        timeout_ms: u32,
    ) -> Result<u64> {
        self.require_active()?;
        let result = (|| {
            check_contexts(&mut self.contexts)?;
            let prepared =
                self.prepare_peer_dispatch(kernel, bytes, workgroup, grid, pointers, timeout_ms)?;
            // SAFETY: group retains all owners and only exposes checked read-only
            // peer bindings; no mutation/free can interleave with this &mut borrow.
            let elapsed = unsafe {
                self.contexts[kernel.rank].execute_prepared_dispatch(prepared, timeout_ms)
            }?;
            check_contexts(&mut self.contexts)?;
            Ok(elapsed)
        })();
        self.finish(result)
    }

    fn prepare_peer_dispatch(
        &mut self,
        kernel: &Gfx950EngineeringPeerKernelV1,
        bytes: Vec<u8>,
        workgroup: [u16; 3],
        grid: [u32; 3],
        pointers: &[Gfx950EngineeringPeerPointerV1],
        timeout_ms: u32,
    ) -> Result<PreparedDispatch> {
        if kernel.group != self.incarnation
            || kernel.rank >= self.contexts.len()
            || timeout_ms == 0
            || timeout_ms > 600_000
            || pointers.len() > MAX_POINTER_FIXUPS_V1
            || bytes.len() > MAX_KERNARG_BYTES_V1 as usize
        {
            return Err("peer dispatch scope or bounds".into());
        }
        let mut bindings = BTreeMap::new();
        let mut fixups = Vec::with_capacity(pointers.len());
        for pointer in pointers {
            let record = self.validate_token(pointer.buffer)?;
            require_peer_access(
                pointer.buffer.owner,
                kernel.rank,
                self.contexts[kernel.rank].backend.gpu_id(),
                &record.mapping.peers,
                pointer.access,
            )?;
            let allocation = &self.contexts[pointer.buffer.owner].buffers[&record.local_id];
            bindings.insert(
                pointer.buffer.id,
                (allocation.va, allocation.requested as u64),
            );
            fixups.push(PointerFixupV1 {
                kernarg_offset: pointer.kernarg_offset,
                buffer: pointer.buffer.id,
                buffer_offset: pointer.buffer_offset,
                extent_bytes: pointer.extent_bytes,
                access: pointer.access,
            });
        }
        self.contexts[kernel.rank].prepare_dispatch_with_peer_bindings(
            kernel.id,
            bytes,
            workgroup,
            grid,
            &fixups,
            Some(&bindings),
        )
    }

    pub fn release(&mut self, buffer: Gfx950EngineeringPeerBufferV1) -> Result<()> {
        self.require_active()?;
        let result = (|| {
            self.validate_token(buffer)?;
            let record = self
                .buffers
                .get_mut(&buffer.id)
                .ok_or("missing peer buffer")?;
            record.mapping.release(&mut NativeTransaction {
                contexts: &mut self.contexts,
                owner: buffer.owner,
                local_id: record.local_id,
            })?;
            self.buffers.remove(&buffer.id);
            Ok(())
        })();
        self.finish(result)
    }

    /// Unmaps peers before owner frees, then explicitly tears down every context.
    pub fn close(&mut self) -> Result<()> {
        self.require_active()?;
        let result = (|| {
            let tokens = self
                .buffers
                .values()
                .map(|record| record.token)
                .collect::<Vec<_>>();
            for token in tokens {
                self.release(token)?;
            }
            check_contexts(&mut self.contexts)?;
            for context in &mut self.contexts {
                context.close_inner()?;
            }
            self.closed = true;
            Ok(())
        })();
        self.finish(result)
    }
}

impl Drop for Gfx950EngineeringPeerGroupV1 {
    fn drop(&mut self) {
        if !self.closed {
            // No uncertain mapping, queue, or backing is retried or freed.
            std::mem::forget(std::mem::take(&mut self.contexts));
        }
    }
}

#[cfg(test)]
#[path = "engineering_gfx950_peer_tests.rs"]
mod tests;
