//! Fixed eight-byte primitive descriptor and exact SSA unit roster.
use super::{Gfx942PhysicalEntryRegisterV20 as R, Gfx942PhysicalEntryShapeErrorV20 as Error};

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942PhysicalEntryOpcodeV20 {
    LoadKernargPair = 0,
    LoadKernargDword = 1,
    WaitLgkm0 = 2,
    ScalarLshl32 = 3,
    VectorAddU32 = 4,
    VectorMove32 = 5,
    ScalarCompareEqZero = 6,
    BranchScc1 = 7,
    Branch = 8,
    VectorLshlrev64 = 9,
    VectorAddCarry = 10,
    VectorAddCarryIn = 11,
    VectorCompareGtU64 = 12,
    SaveAndMaskExec = 13,
    GlobalStoreDword = 14,
    WaitVm0 = 15,
    RestoreExec = 16,
    Endpgm0 = 17,
    Fallthrough = 18,
}
impl Gfx942PhysicalEntryOpcodeV20 {
    pub const fn from_tag(tag: u8) -> Option<Self> {
        Some(match tag {
            0 => Self::LoadKernargPair,
            1 => Self::LoadKernargDword,
            2 => Self::WaitLgkm0,
            3 => Self::ScalarLshl32,
            4 => Self::VectorAddU32,
            5 => Self::VectorMove32,
            6 => Self::ScalarCompareEqZero,
            7 => Self::BranchScc1,
            8 => Self::Branch,
            9 => Self::VectorLshlrev64,
            10 => Self::VectorAddCarry,
            11 => Self::VectorAddCarryIn,
            12 => Self::VectorCompareGtU64,
            13 => Self::SaveAndMaskExec,
            14 => Self::GlobalStoreDword,
            15 => Self::WaitVm0,
            16 => Self::RestoreExec,
            17 => Self::Endpgm0,
            18 => Self::Fallthrough,
            _ => return None,
        })
    }
    pub const fn is_control(self) -> bool {
        matches!(
            self,
            Self::BranchScc1 | Self::Branch | Self::Endpgm0 | Self::Fallthrough
        )
    }
    pub const fn is_vector_definition(self) -> bool {
        matches!(
            self,
            Self::VectorAddU32
                | Self::VectorMove32
                | Self::VectorLshlrev64
                | Self::VectorAddCarry
                | Self::VectorAddCarryIn
                | Self::VectorCompareGtU64
        )
    }
}

/// Inert typed descriptor. Public fields do not bypass canonical verification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942PhysicalEntryInstructionVNext {
    pub opcode: Gfx942PhysicalEntryOpcodeV20,
    pub destination: u8,
    pub source0: u8,
    pub source1: u8,
    pub immediate: u32,
}
impl Gfx942PhysicalEntryInstructionVNext {
    pub const fn descriptor(self) -> [u8; 8] {
        let immediate = self.immediate.to_le_bytes();
        [
            self.opcode as u8,
            self.destination,
            self.source0,
            self.source1,
            immediate[0],
            immediate[1],
            immediate[2],
            immediate[3],
        ]
    }
    pub fn from_descriptor(bytes: [u8; 8]) -> Result<Self, Error> {
        let value = Self {
            opcode: Gfx942PhysicalEntryOpcodeV20::from_tag(bytes[0]).ok_or(Error::Instruction)?,
            destination: bytes[1],
            source0: bytes[2],
            source1: bytes[3],
            immediate: u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
        };
        value.validate_shape()?;
        Ok(value)
    }
    pub fn validate_shape(self) -> Result<(), Error> {
        use Gfx942PhysicalEntryOpcodeV20 as O;
        let (d, a, b, immediate) = (self.destination, self.source0, self.source1, self.immediate);
        let sgpr = |n| n < 64;
        let vgpr = |n| n < 64;
        let pair = |n: u8| n < 63 && n.is_multiple_of(2);
        let sdest = |n| (3..64).contains(&n);
        let vdest = |n| (1..64).contains(&n);
        let valid = match self.opcode {
            O::LoadKernargPair => {
                sdest(d) && pair(d) && a == 0 && b == 0 && matches!(immediate, 0 | 8)
            }
            O::LoadKernargDword => {
                sdest(d) && a == 0 && b == 0 && matches!(immediate, 16 | 20 | 24 | 28)
            }
            O::WaitLgkm0 | O::WaitVm0 | O::Endpgm0 => d == 0 && a == 0 && b == 0 && immediate == 0,
            O::ScalarLshl32 => sdest(d) && sgpr(a) && b == 0 && immediate == 6,
            O::VectorAddU32 | O::VectorAddCarry => vdest(d) && sgpr(a) && vgpr(b) && immediate == 0,
            O::VectorMove32 => vdest(d) && (sgpr(a) || a == 255) && b == 0 && immediate == 0,
            O::ScalarCompareEqZero => d == 0 && sgpr(a) && b == 0 && immediate == 0,
            O::BranchScc1 | O::Branch | O::Fallthrough => {
                d == 0 && a == 0 && b == 0 && immediate <= 255
            }
            O::VectorLshlrev64 => vdest(d) && pair(d) && pair(a) && b == 0 && immediate == 2,
            O::VectorAddCarryIn => vdest(d) && vgpr(a) && vgpr(b) && immediate == 0,
            O::VectorCompareGtU64 => d == 0 && pair(a) && pair(b) && immediate == 0,
            O::SaveAndMaskExec => sdest(d) && pair(d) && a == 0 && b == 0 && immediate == 0,
            O::GlobalStoreDword => d == 0 && pair(a) && vgpr(b) && immediate == 0,
            O::RestoreExec => d == 0 && pair(a) && b == 0 && immediate == 0,
        };
        if valid {
            Ok(())
        } else {
            Err(Error::Instruction)
        }
    }

    /// Exact prefix, explicit register units then EXEC then implicit VCC.
    /// Callers must validate shape before using register indices.
    pub const fn operand_registers(self) -> [Option<R>; 6] {
        use Gfx942PhysicalEntryOpcodeV20 as O;
        let (a, b) = (self.source0, self.source1);
        match self.opcode {
            O::LoadKernargPair | O::LoadKernargDword => {
                [Some(R::Sgpr(0)), Some(R::Sgpr(1)), None, None, None, None]
            }
            O::ScalarLshl32 | O::ScalarCompareEqZero => {
                [Some(R::Sgpr(a)), None, None, None, None, None]
            }
            O::VectorAddU32 | O::VectorAddCarry => [
                Some(R::Sgpr(a)),
                Some(R::Vgpr(b)),
                Some(R::Exec),
                None,
                None,
                None,
            ],
            O::VectorMove32 if a == 255 => [Some(R::Exec), None, None, None, None, None],
            O::VectorMove32 => [Some(R::Sgpr(a)), Some(R::Exec), None, None, None, None],
            O::VectorLshlrev64 => [
                Some(R::Vgpr(a)),
                Some(R::Vgpr(a.wrapping_add(1))),
                Some(R::Exec),
                None,
                None,
                None,
            ],
            O::VectorAddCarryIn => [
                Some(R::Vgpr(a)),
                Some(R::Vgpr(b)),
                Some(R::Exec),
                Some(R::Vcc),
                None,
                None,
            ],
            O::VectorCompareGtU64 => [
                Some(R::Sgpr(a)),
                Some(R::Sgpr(a.wrapping_add(1))),
                Some(R::Vgpr(b)),
                Some(R::Vgpr(b.wrapping_add(1))),
                Some(R::Exec),
                None,
            ],
            O::SaveAndMaskExec => [Some(R::Vcc), Some(R::Exec), None, None, None, None],
            O::GlobalStoreDword => [
                Some(R::Vgpr(a)),
                Some(R::Vgpr(a.wrapping_add(1))),
                Some(R::Vgpr(b)),
                Some(R::Exec),
                None,
                None,
            ],
            O::RestoreExec => [
                Some(R::Sgpr(a)),
                Some(R::Sgpr(a.wrapping_add(1))),
                None,
                None,
                None,
                None,
            ],
            O::BranchScc1 => [Some(R::Scc), None, None, None, None, None],
            O::WaitLgkm0 | O::WaitVm0 | O::Branch | O::Endpgm0 | O::Fallthrough => [None; 6],
        }
    }

    /// Exact result order; pair destinations are two U32 units, not one U64.
    pub const fn result_registers(self) -> [Option<R>; 4] {
        use Gfx942PhysicalEntryOpcodeV20 as O;
        let d = self.destination;
        match self.opcode {
            O::LoadKernargPair => [
                Some(R::Sgpr(d)),
                Some(R::Sgpr(d.wrapping_add(1))),
                None,
                None,
            ],
            O::LoadKernargDword => [Some(R::Sgpr(d)), None, None, None],
            O::ScalarLshl32 => [Some(R::Sgpr(d)), Some(R::Scc), None, None],
            O::VectorAddU32 | O::VectorMove32 => [Some(R::Vgpr(d)), None, None, None],
            O::ScalarCompareEqZero => [Some(R::Scc), None, None, None],
            O::VectorLshlrev64 => [
                Some(R::Vgpr(d)),
                Some(R::Vgpr(d.wrapping_add(1))),
                None,
                None,
            ],
            O::VectorAddCarry | O::VectorAddCarryIn => [Some(R::Vgpr(d)), Some(R::Vcc), None, None],
            O::VectorCompareGtU64 => [Some(R::Vcc), None, None, None],
            O::SaveAndMaskExec => [
                Some(R::Sgpr(d)),
                Some(R::Sgpr(d.wrapping_add(1))),
                Some(R::Exec),
                Some(R::Scc),
            ],
            O::RestoreExec => [Some(R::Exec), None, None, None],
            O::WaitLgkm0
            | O::WaitVm0
            | O::GlobalStoreDword
            | O::BranchScc1
            | O::Branch
            | O::Endpgm0
            | O::Fallthrough => [None; 4],
        }
    }
}
