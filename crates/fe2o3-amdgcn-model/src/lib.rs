//! AMDGPU intrinsic vocabulary and fail-closed target lowering primitives.
//!
//! The initial target-neutral lowering subset is documented in
//! `crates/dialect-amdgcn/G1_LOWERING.md`. The strict gfx942 floating-point
//! extension is documented in `crates/dialect-amdgcn/GFX942_FLOATS.md`, and the
//! source-bound assembly subset is documented in
//! `crates/dialect-amdgcn/GFX942_INLINE_ASSEMBLY.md`. None of these paths grants
//! linking, loading, or execution authority.

mod device_math;
mod lowering;
mod scalar_v2;

pub use device_math::*;
pub use lowering::*;
pub use scalar_v2::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Dim {
    X,
    Y,
    Z,
}

impl Dim {
    pub fn suffix(self) -> &'static str {
        match self {
            Self::X => "x",
            Self::Y => "y",
            Self::Z => "z",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AmdgcnIntrinsic {
    WorkItemId(Dim),
    WorkGroupId(Dim),
    GridSize(Dim),
    DispatchPtr,
    SBarrier,
    MbcntLo,
    MbcntHi,
    Ballot32,
    Ballot64,
    DsBpermute,
    MfmaF32M16N16K16Bf16,
}

impl AmdgcnIntrinsic {
    pub fn llvm_name(self) -> &'static str {
        match self {
            Self::WorkItemId(Dim::X) => "llvm.amdgcn.workitem.id.x",
            Self::WorkItemId(Dim::Y) => "llvm.amdgcn.workitem.id.y",
            Self::WorkItemId(Dim::Z) => "llvm.amdgcn.workitem.id.z",
            Self::WorkGroupId(Dim::X) => "llvm.amdgcn.workgroup.id.x",
            Self::WorkGroupId(Dim::Y) => "llvm.amdgcn.workgroup.id.y",
            Self::WorkGroupId(Dim::Z) => "llvm.amdgcn.workgroup.id.z",
            Self::GridSize(Dim::X) => "llvm.amdgcn.grid.size.x",
            Self::GridSize(Dim::Y) => "llvm.amdgcn.grid.size.y",
            Self::GridSize(Dim::Z) => "llvm.amdgcn.grid.size.z",
            Self::DispatchPtr => "llvm.amdgcn.dispatch.ptr",
            Self::SBarrier => "llvm.amdgcn.s.barrier",
            Self::MbcntLo => "llvm.amdgcn.mbcnt.lo",
            Self::MbcntHi => "llvm.amdgcn.mbcnt.hi",
            Self::Ballot32 => "llvm.amdgcn.ballot.i32",
            Self::Ballot64 => "llvm.amdgcn.ballot.i64",
            Self::DsBpermute => "llvm.amdgcn.ds.bpermute",
            Self::MfmaF32M16N16K16Bf16 => "llvm.amdgcn.mfma.f32.16x16x16bf16.1k",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AddressSpace {
    Generic,
    Global,
    Region,
    Local,
    Constant,
    Private,
    Constant32Bit,
    BufferFatPointer,
}

impl AddressSpace {
    pub fn llvm_id(self) -> u32 {
        match self {
            Self::Generic => 0,
            Self::Global => 1,
            Self::Region => 2,
            Self::Local => 3,
            Self::Constant => 4,
            Self::Private => 5,
            Self::Constant32Bit => 6,
            Self::BufferFatPointer => 7,
        }
    }
}

pub const AMDGPU_TRIPLE: &str = "amdgcn-amd-amdhsa";
