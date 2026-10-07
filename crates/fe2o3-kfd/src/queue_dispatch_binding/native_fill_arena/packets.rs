//! Prepaid packet headers, initialized directly in bounded heap storage.

use super::*;

/// Exact original packet roster for the arena. Only the requested packet-header
/// payload is charged here; each packet's existing kernarg/binding allocations
/// and allocator overhead remain outside this table's payload contract.
///
/// Allocation and its original account debit precede the first initializer call.
/// Initialization visits each index once, in order, without an inline 1024-packet
/// temporary. A refused allocation does not invoke the initializer. Panic during
/// initialization disposes only the already-produced inert packet descriptors.
pub struct Gfx942NativeFillArenaPacketsV1 {
    packets: HostMetadataTableV1<Gfx942FixedDispatchPacketV1>,
}

impl Gfx942NativeFillArenaPacketsV1 {
    pub fn try_new(
        account: &ResourceCreditAccountV1,
        mut initialize: impl FnMut(usize) -> Gfx942FixedDispatchPacketV1,
    ) -> Result<Self, Gfx942DispatchBindingErrorV1> {
        let mut index = 0;
        let packets = HostMetadataTableV1::try_new(SLOTS, Some(account), || {
            let packet = initialize(index);
            index += 1;
            packet
        })
        .map_err(|_| rejected(0, "arena packet table capacity"))?;
        Ok(Self { packets })
    }

    /// Borrows the exact original descriptor roster without transferring a debit
    /// or exposing any native DATA, code or execution authority.
    pub fn packets(&self) -> &[Gfx942FixedDispatchPacketV1; SLOTS] {
        match (&*self.packets).try_into() {
            Ok(packets) => packets,
            Err(_) => std::process::abort(),
        }
    }

    pub(super) fn place_local_outputs(
        &mut self,
        total: u64,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        let mut end = 0u64;
        for (index, packet) in self.packets.iter().enumerate() {
            let [binding] = packet.buffers.as_ref() else {
                return Err(rejected(index, "arena one original local output"));
            };
            if !packet.conditional_fill
                || packet.program_index != 0
                || binding.data_index != 0
                || binding.explicit_argument_index != 0
                || binding.data_byte_offset != 0
                || binding.byte_len == 0
                || binding.completed_snapshot.is_some()
            {
                return Err(rejected(index, "arena original local output shape"));
            }
            end = end
                .checked_add(binding.byte_len)
                .ok_or_else(|| rejected(index, "arena local extent overflow"))?;
        }
        if end != total {
            return Err(rejected(0, "arena original total local extent"));
        }
        let mut offset = 0;
        for packet in self.packets.iter_mut() {
            packet.buffers[0].data_byte_offset = offset;
            offset += packet.buffers[0].byte_len;
        }
        Ok(())
    }

    pub(super) fn restore_local_outputs(&mut self) {
        for packet in self.packets.iter_mut() {
            packet.buffers[0].data_byte_offset = 0;
        }
    }
}

#[cfg(test)]
#[path = "packets_tests.rs"]
mod tests;

impl preparation::PreparationPacketsV1<SLOTS> for Gfx942NativeFillArenaPacketsV1 {
    fn as_packets(&self) -> &[Gfx942FixedDispatchPacketV1; SLOTS] {
        self.packets()
    }
}
