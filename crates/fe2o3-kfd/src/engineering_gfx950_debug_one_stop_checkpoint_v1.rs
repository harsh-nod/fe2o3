//! Source-bound immutable diagnostic rendezvous. Never a permission token.
//! No imported pointer, Boolean or JSON can construct the native target owner.
use super::super::super::super::{CONTROL, CWSR, EOP, KERNARG, RING, SIGNAL};
use super::super::super::Backend;
use super::{E, contract};
use crate::memory::MemoryBackend;
use sha2::{Digest, Sha256};

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Checkpoint {
    pub(super) magic: [u8; 8],
    pub(super) version: u32,
    pub(super) bytes: u32,
    pub(super) pid: u32,
    pub(super) queue_id: u32,
    pub(super) gpu_id: u32,
    pub(super) event_id: u32,
    pub(super) device_unique_id: u64,
    pub(super) queue_epoch: u64,
    pub(super) packet_id: u64,
    pub(super) code_base: u64,
    pub(super) code_bytes: u64,
    pub(super) descriptor: u64,
    pub(super) entry: u64,
    pub(super) ring: u64,
    pub(super) ring_bytes: u64,
    pub(super) control: u64,
    pub(super) write_pointer: u64,
    pub(super) read_pointer: u64,
    pub(super) signal_base: u64,
    // AMD signal AtomicI64 value is base+8. Query17 is intentionally NOT here.
    pub(super) signal_value: u64,
    pub(super) kernarg: u64,
    pub(super) kernarg_bytes: u64,
    pub(super) output_base: u64,
    pub(super) output_logical_bytes: u64,
    pub(super) output_backing_bytes: u64,
    pub(super) output_payload: u64,
    pub(super) output_payload_bytes: u64,
    pub(super) trap_base: u64,
    pub(super) trap_bytes: u64,
    pub(super) metadata_root: u64,
    pub(super) cwsr: u64,
    pub(super) cwsr_bytes: u64,
    pub(super) eop: u64,
    pub(super) eop_bytes: u64,
    pub(super) object_sha256: [u8; 32],
    pub(super) source_sha256: [u8; 32],
    pub(super) image_sha256: [u8; 32],
    pub(super) trap_sha256: [u8; 32],
    pub(super) closure_sha256: [u8; 32],
    pub(super) packet_bytes: [u8; 64],
    pub(super) kernarg_values: [u8; 264],
    pub(super) reserved: [u64; 4],
}
pub(super) fn capture(
    base: &super::super::Inner,
    packet: &fe2o3_aql::AqlPreparedKernelDispatchV1,
    image_sha256: [u8; 32],
) -> Result<Checkpoint, E> {
    let r = base.cold.resources.get();
    let c = &r.context;
    let kernel = c
        .kernels
        .get(&1)
        .ok_or(E::Contract("one-stop sole kernel"))?;
    let closure = contract::validate_object(&kernel.object)?;
    let entry_offset = closure
        .selected_binding()
        .entry_address()
        .checked_sub(closure.envelope().plan().image_start())
        .ok_or(E::Contract("entry mapping relation"))?;
    let output = c
        .internal
        .get(contract::OUTPUT)
        .ok_or(E::Contract("output custody"))?;
    let signal =
        contract::signal_addresses(c.internal[SIGNAL].va, c.internal[SIGNAL].backing as u64)?;
    let trap = r.trap.as_ref().ok_or(E::Contract("trap custody"))?;
    let mut kernarg_values = [0; 264];
    Backend::with_bytes(&c.internal[KERNARG].mapping, 65536, |bytes| {
        kernarg_values.copy_from_slice(&bytes[..264])
    });
    contract::check_kernarg(
        &kernarg_values,
        output
            .va
            .checked_add(8)
            .ok_or(E::Contract("output payload extent"))?,
    )?;
    let result = Checkpoint {
        magic: *b"F3GSTP01",
        version: 1,
        bytes: core::mem::size_of::<Checkpoint>() as u32,
        pid: c.backend.opener_pid(),
        queue_id: c.queue_id.ok_or(E::Contract("queue custody"))?,
        gpu_id: c.backend.gpu_id(),
        event_id: c
            .event
            .as_ref()
            .ok_or(E::Contract("event custody"))?
            .event_id_observation(),
        device_unique_id: c.unique_id,
        queue_epoch: c.queue_epoch,
        packet_id: 0,
        code_base: kernel.code.va,
        code_bytes: kernel.code.backing as u64,
        descriptor: kernel
            .code
            .va
            .checked_add(kernel.descriptor_offset)
            .ok_or(E::Contract("descriptor extent"))?,
        entry: kernel
            .code
            .va
            .checked_add(entry_offset)
            .ok_or(E::Contract("entry extent"))?,
        ring: c.internal[RING].va,
        ring_bytes: c.internal[RING].backing as u64,
        control: c.internal[CONTROL].va,
        write_pointer: c.internal[CONTROL]
            .va
            .checked_add(0x38)
            .ok_or(E::Contract("write pointer extent"))?,
        read_pointer: c.internal[CONTROL]
            .va
            .checked_add(0x80)
            .ok_or(E::Contract("read pointer extent"))?,
        signal_base: signal.0,
        signal_value: signal.1,
        kernarg: c.internal[KERNARG].va,
        kernarg_bytes: 264,
        output_base: output.va,
        output_logical_bytes: 272,
        output_backing_bytes: 4096,
        output_payload: output
            .va
            .checked_add(8)
            .ok_or(E::Contract("output payload extent"))?,
        output_payload_bytes: 256,
        trap_base: trap.va,
        trap_bytes: trap.backing as u64,
        metadata_root: r
            .metadata
            .as_ref()
            .ok_or(E::Contract("metadata custody"))?
            .one_stop_active_root_observation()
            .map_err(|e| E::Native(format!("{e:?}")))?,
        cwsr: c.internal[CWSR].va,
        cwsr_bytes: c.internal[CWSR].backing as u64,
        eop: c.internal[EOP].va,
        eop_bytes: c.internal[EOP].backing as u64,
        object_sha256: contract::OBJECT_SHA256,
        source_sha256: contract::SOURCE_SHA256,
        image_sha256,
        trap_sha256: base.cold.facts.trap_sha256(),
        closure_sha256: base.closure,
        packet_bytes: packet.unpublished_packet().encode_unpublished_le(),
        kernarg_values,
        reserved: [0; 4],
    };
    let image_end = result
        .code_base
        .checked_add(result.code_bytes)
        .ok_or(E::Contract("checkpoint image extent"))?;
    if result.pid != std::process::id()
        || result.queue_epoch != 0
        || result.code_bytes > 1024 * 1024
        || result.metadata_root == 0
        || result
            .descriptor
            .checked_add(64)
            .is_none_or(|end| end > image_end)
        || result
            .entry
            .checked_add(84)
            .is_none_or(|end| end > image_end)
        || Backend::with_bytes(&kernel.code.mapping, kernel.code.backing, |bytes| {
            <[u8; 32]>::from(Sha256::digest(bytes))
        }) != image_sha256
    {
        return Err(E::Contract("current checkpoint object/custody"));
    }
    Ok(result)
}
/// Fixed source rendezvous only. It reads no decision and grants no permission.
/// The unsafe caller must arrange the real native pre-resume gate at this
/// exact function in the exact owned executable. No debugger/stop is implied.
#[unsafe(no_mangle)]
#[inline(never)]
extern "C" fn fe2o3_gfx950_one_stop_prepublication_checkpoint_v1(checkpoint: *const Checkpoint) {
    core::hint::black_box(checkpoint);
}
pub(super) fn rendezvous(checkpoint: &Checkpoint) {
    fe2o3_gfx950_one_stop_prepublication_checkpoint_v1(checkpoint);
}
