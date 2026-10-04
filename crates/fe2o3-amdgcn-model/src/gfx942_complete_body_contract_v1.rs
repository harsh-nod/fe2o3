//! Inert provisional boundary for the bounded complete-body model.
//! These declarations are requirements for a FUTURE source-owned emitter. They
//! do not prove a prologue, descriptor, wait, EXEC restoration or emitted ABI.
use fe2o3_kernel_ir::Gfx942OrderedProgramRegistersV1 as Registers;

/// The only logical ABI in this provisional gfx942:xnack-/Wave64 profile.
/// Rust source: writable global u32 slice, three u32 inputs, uniform u32 selector.
/// Lowered parameters: pointer64, length64, then four u32 values. No return value.
/// No raw pointer/JSON constructor grants source ownership or uniformity proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942CompleteBodyAbiV1 {
    GuardedGlobalU32OutputUniform4,
}

/// Inert target/launch claims. A live compiler adapter must independently derive
/// these from the current Instance, authenticated target and typed launch owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942CompleteBodyBoundaryV1 {
    pub abi: Gfx942CompleteBodyAbiV1,
    pub pointer_bits: u8,
    pub index_bits: u8,
    pub wave_width: u8,
    pub xnack_enabled: bool,
    pub required_workgroup: [u32; 3],
    pub maximum_workgroup: [u32; 3],
}

impl Gfx942CompleteBodyBoundaryV1 {
    pub const PROFILE: Self = Self {
        abi: Gfx942CompleteBodyAbiV1::GuardedGlobalU32OutputUniform4,
        pointer_bits: 64,
        index_bits: 64,
        wave_width: 64,
        xnack_enabled: false,
        required_workgroup: [64, 1, 1],
        maximum_workgroup: [64, 1, 1],
    };

    /// Explicit parameter layout only. Hidden AMDHSA kernargs and the final
    /// descriptor are deliberately NOT specified or inferred by this model.
    pub const EXPLICIT_PARAMETER_OFFSETS: [u8; 6] = [0, 8, 16, 20, 24, 28];
    pub const EXPLICIT_PARAMETER_BYTES: u8 = 32;
    pub const EXPLICIT_PARAMETER_ALIGNMENT: u8 = 8;
}

/// Complete declaration of this provisional body's fixed implicit boundary.
/// Extents are required binding high-waters, NOT allocated capacities, physical
/// liveness or permission to accept a matching final descriptor without replay.
///
/// Existing source-body experiment reservations retained: v0:v1 index, v2:v3
/// address, v4 pointer-high; v5:v7 reserved; s16 pointer-low, s18:s19 length,
/// s20:s21 saved EXEC. s22 for the uniform selector is a NEW provisional
/// reservation, not inherited native evidence. s17 remains reserved.
/// Author VGPR roles must be distinct v8..v63. No author SGPR role is admitted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942CompleteBodyResourcesV1 {
    pub vgpr_binding_extent: u8,
    pub fixed_sgpr_binding_extent: u8,
    pub reads_exec: bool,
    pub restores_exec: bool,
    pub clobbers_vcc: bool,
    pub clobbers_scc: bool,
    pub guarded_global_u32_store: bool,
    pub global_loads: bool,
    pub group_segment_bytes: u32,
    pub private_segment_bytes: u32,
    pub runtime_helper_calls: u8,
}

impl Gfx942CompleteBodyResourcesV1 {
    pub const FIRST_AUTHOR_VGPR: u8 = 8;
    pub const FIXED_SGPR_BINDING_EXTENT: u8 = 23;

    /// Computes required declarations, not a validated emission/effect receipt.
    /// The terminal requires index < length, one aligned u32 global write,
    /// completion of that write, unconditional EXEC restoration, then endpgm.
    /// Pointer provenance, accessible extent and external launch premises are
    /// not proved here. The future normal pipeline must discharge them.
    pub const fn required(registers: Registers) -> Self {
        Self {
            vgpr_binding_extent: registers.vgpr_high_water(),
            fixed_sgpr_binding_extent: Self::FIXED_SGPR_BINDING_EXTENT,
            reads_exec: true,
            restores_exec: true,
            clobbers_vcc: true,
            clobbers_scc: true,
            guarded_global_u32_store: true,
            global_loads: false,
            group_segment_bytes: 0,
            private_segment_bytes: 0,
            runtime_helper_calls: 0,
        }
    }
}
