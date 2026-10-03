//! Closed execution projection of the captured Rust `fill_write_only` entry.
//!
//! All 14 instructions, including the EXEC-zero path, are accounted for. This
//! models architectural values and sparse stores, not scheduling or native
//! memory visibility. The ISA interpretation and AMDHSA initial-state contract
//! remain reviewed assumptions, not results of the arithmetic proof. There is
//! no source/compiler refinement, authenticated memory snapshot, or authority.

use fe2o3_hsaco::{KernelDescriptorBinding, inspect_and_bind_kernel_descriptors};
use std::{error::Error, fmt};

include!("gfx942_fill_wave_v1/body.rs");

macro_rules! exec_expr {
    ($body:expr) => {
        $body
    };
}

/// Exact words of the 68-byte function, excluding ELF section padding.
const WORDS: [u32; 17] = [
    0xc00a_0100,
    0x0000_0000, // s_load_dwordx4 s[4:7], s[0:1], 0
    0xbe83_0080, // s_mov_b32 s3, 0
    0x8e80_8602, // s_lshl_b64 s[0:1], s[2:3], 6
    0x2800_0000, // v_or_b32 v0, s0, v0
    0x7e02_0201, // v_mov_b32 v1, s1
    0xbf8c_c07f, // s_waitcnt lgkmcnt(0)
    0x7dd8_0006, // v_cmp_gt_u64 vcc, s[6:7], v[0:1]
    0xbe80_206a, // s_and_saveexec_b64 s[0:1], vcc
    0xbf88_0006, // s_cbranch_execz +6 dwords, to offset0x40
    0x7e04_0204, // v_mov_b32 v2, s4
    0x7e06_0205, // v_mov_b32 v3, s5
    0xd208_0002,
    0x0409_0500, // v_lshl_add_u64 v[2:3], v[0:1], 2, v[2:3]
    0xdc70_8000,
    0x007f_0002, // global_store_dword v[2:3], v0, off
    0xbf81_0000, // s_endpgm
];

/// A borrowed exact code object and its selected descriptor. Not a certificate.
#[derive(Debug)]
pub struct Gfx942FillKernelV1<'a> {
    hsaco: &'a [u8],
    binding: KernelDescriptorBinding,
}

impl<'a> Gfx942FillKernelV1<'a> {
    pub fn inspect(hsaco: &'a [u8], kernel_index: usize) -> Result<Self, Gfx942FillErrorV1> {
        use Gfx942FillErrorV1 as E;
        let inspected = inspect_and_bind_kernel_descriptors(hsaco).map_err(|_| E::CodeObject)?;
        let binding = *inspected
            .bindings()
            .get(kernel_index)
            .ok_or(E::KernelIndex)?;
        let layout = inspected
            .gfx942_initial_register_layout_v1(kernel_index)
            .map_err(|_| E::EntryLayout)?;
        let descriptor = binding.descriptor();
        let kernel = &inspected.inspection().kernels()[kernel_index];
        if layout.kernarg_pointer_sgprs() != Some([0, 1])
            || layout.workgroup_id_sgprs() != [Some(2), None, None]
            || layout.workitem_id_vgprs() != [Some(0), None, None]
            || layout.workgroup_info_sgpr().is_some()
            || layout.user_sgpr_count() != 2
            || descriptor.kernarg_size() != 16
            || descriptor.group_segment_fixed_size() != 0
            || kernel.required_workgroup_size() != Some([64, 1, 1])
            // Close over MODE/resource configuration as well as operand layout.
            || descriptor.compute_pgm_rsrc1() != 0x00af_0040
            || descriptor.compute_pgm_rsrc2() != 0x84
            || descriptor.compute_pgm_rsrc3() != 0
        {
            return Err(E::EntryLayout);
        }
        let start = usize::try_from(binding.entry_file_offset()).map_err(|_| E::CodeExtent)?;
        let end = start.checked_add(68).ok_or(E::CodeExtent)?;
        if binding.entry_size() != 68 {
            return Err(E::CodeExtent);
        }
        let code = hsaco.get(start..end).ok_or(E::CodeExtent)?;
        for (index, word) in WORDS.iter().enumerate() {
            if code[index * 4..index * 4 + 4] != word.to_le_bytes() {
                return Err(E::Instruction { word: index });
            }
        }
        Ok(Self { hsaco, binding })
    }

    pub const fn code_object(&self) -> &'a [u8] {
        self.hsaco
    }

    pub const fn binding(&self) -> KernelDescriptorBinding {
        self.binding
    }

    /// Executes a caller-supplied projection, after validating its spatial bounds.
    ///
    /// The caller must establish the actual dispatch's ordinary wave64 mode
    /// (VSKIP and GPR indexing disabled, 64-bit global addressing), initial
    /// register values, and stable readable kernarg bytes. `output_base/bytes`
    /// describe a distinct writable allocation; this function cannot authenticate
    /// its backing or ownership. This checks this wave's coordinates and its
    /// 64-thread workgroup size, not dispatch-wide dimensions or coverage.
    /// Incoming EXEC may be partial or empty.
    pub fn execute(
        &self,
        state: Gfx942FillWaveStateV1,
        kernarg: &[u8; 16],
        output_base: u64,
        output_bytes: u64,
        workgroup_size: [u32; 3],
        workgroup_yz: [u32; 2],
    ) -> Result<Gfx942FillWaveExecutionV1, Gfx942FillErrorV1> {
        use Gfx942FillErrorV1 as E;
        if workgroup_size != [64, 1, 1] || workgroup_yz != [0, 0] {
            return Err(E::Geometry);
        }
        let address = pair(state.sgprs[0], state.sgprs[1]);
        let kernarg_end = address.checked_add(16).ok_or(E::KernargRegion)?;
        if !address.is_multiple_of(8) {
            return Err(E::KernargRegion);
        }
        let words = decode_kernarg(kernarg);
        let pointer = pair(words[0], words[1]);
        let count = pair(words[2], words[3]);
        let bytes = count.checked_mul(4).ok_or(E::OutputRegion)?;
        let end = pointer.checked_add(bytes).ok_or(E::OutputRegion)?;
        if pointer != output_base || bytes != output_bytes || !pointer.is_multiple_of(4) {
            return Err(E::OutputRegion);
        }
        if bytes != 0 && pointer < kernarg_end && address < end {
            return Err(E::OverlappingRegions);
        }
        let mut seen = 0u64;
        for (lane, registers) in state.vgprs.iter().enumerate() {
            if state.exec_mask & (1u64 << lane) != 0 {
                let local = registers[0];
                if local >= 64 {
                    return Err(E::LocalId);
                }
                let bit = 1u64 << local;
                if seen & bit != 0 {
                    return Err(E::DuplicateLocalId);
                }
                seen |= bit;
            }
        }
        Ok(execute_wave(state, kernarg))
    }
}

/// Only the registers touched by this closed entry. All other registers are
/// outside the projection; arbitrary initial values in these arrays are retained
/// until the corresponding instruction writes them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942FillWaveStateV1 {
    pub sgprs: [u32; 8],
    pub vgprs: [[u32; 4]; 64],
    pub exec_mask: u64,
    pub vcc: u64,
    pub scc: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942FillStoreV1 {
    pub address: u64,
    pub value: u32,
}

/// Sparse architectural write effects, not host-visible native completion.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Gfx942FillWaveExecutionV1 {
    pub state: Gfx942FillWaveStateV1,
    pub kernarg_read_address: u64,
    pub stores: [Option<Gfx942FillStoreV1>; 64],
    pub retired_instructions: u8,
    /// Modeled relative next-PC offset after END at0x40, not a hardware PC read.
    pub terminal_pc: u64,
}

impl Gfx942FillWaveExecutionV1 {
    /// Observes the modeled four-byte little-endian updates over an arbitrary
    /// initial memory byte. Outside the store intervals, returns `before`.
    pub fn byte_after(&self, address: u64, before: u8) -> u8 {
        gfx942_fill_byte_body_v1!(exec_expr, self, address, before, lane, byte, [])
    }
}

fn pair(low: u32, high: u32) -> u64 {
    gfx942_fill_pair_body_v1!(low, high)
}

fn decode_kernarg(bytes: &[u8; 16]) -> [u32; 4] {
    gfx942_fill_decode_body_v1!(bytes)
}

fn word(a: u8, b: u8, c: u8, d: u8) -> u32 {
    gfx942_fill_word_body_v1!(a, b, c, d)
}

fn index_words(low: u32, high: u32, local: u32) -> [u32; 2] {
    gfx942_fill_index_body_v1!(low, high, local)
}

fn execute_wave(mut state: Gfx942FillWaveStateV1, kernarg: &[u8; 16]) -> Gfx942FillWaveExecutionV1 {
    gfx942_fill_wave_body_v1!(
        exec_expr,
        state,
        kernarg,
        lane,
        stores,
        incoming,
        words,
        length,
        [],
        [],
        [],
        [],
        [],
        [],
        [],
        [],
        []
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942FillErrorV1 {
    CodeObject,
    KernelIndex,
    EntryLayout,
    CodeExtent,
    Instruction { word: usize },
    Geometry,
    KernargRegion,
    OutputRegion,
    OverlappingRegions,
    LocalId,
    DuplicateLocalId,
}

impl fmt::Display for Gfx942FillErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "gfx942 fill model rejected input: {self:?}")
    }
}
impl Error for Gfx942FillErrorV1 {}

#[cfg(test)]
mod tests;

mod dispatch;
pub use dispatch::{Gfx942FillDispatchErrorV1, Gfx942FillDispatchV1};
