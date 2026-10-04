//! Fixed descriptor and exact physical-unit input/result roster for KIR22 only.
use super::Gfx942PhysicalLdsExchangeShapeErrorV22 as Error;
use crate::Gfx942PhysicalEntryRegisterV20 as R;

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942PhysicalLdsExchangeOpcodeV1 {
    LoadKernargPair = 0,
    WaitLgkm0 = 1,
    ScalarLshl32 = 2,
    VectorAddU32 = 3,
    VectorMove32 = 4,
    VectorLshlrev64 = 5,
    VectorAddCarry = 6,
    VectorAddCarryIn = 7,
    GlobalLoadDword = 8,
    WaitVm0 = 9,
    VectorCompareGtU64 = 10,
    SaveAndMaskExec = 11,
    GlobalStoreDword = 12,
    RestoreExec = 13,
    Endpgm0 = 14,
    VectorLshlrev32 = 15,
    VectorXor32 = 16,
    LdsWriteB32 = 17,
    LdsReadB32 = 18,
    WorkgroupPublishBarrier = 19,
}
impl Gfx942PhysicalLdsExchangeOpcodeV1 {
    pub const fn from_tag(tag: u8) -> Option<Self> {
        Some(match tag {
            0 => Self::LoadKernargPair,
            1 => Self::WaitLgkm0,
            2 => Self::ScalarLshl32,
            3 => Self::VectorAddU32,
            4 => Self::VectorMove32,
            5 => Self::VectorLshlrev64,
            6 => Self::VectorAddCarry,
            7 => Self::VectorAddCarryIn,
            8 => Self::GlobalLoadDword,
            9 => Self::WaitVm0,
            10 => Self::VectorCompareGtU64,
            11 => Self::SaveAndMaskExec,
            12 => Self::GlobalStoreDword,
            13 => Self::RestoreExec,
            14 => Self::Endpgm0,
            15 => Self::VectorLshlrev32,
            16 => Self::VectorXor32,
            17 => Self::LdsWriteB32,
            18 => Self::LdsReadB32,
            19 => Self::WorkgroupPublishBarrier,
            _ => return None,
        })
    }
    pub const fn is_control(self) -> bool {
        matches!(self, Self::Endpgm0)
    }
    /// All these instructions require full EXEC in the closed initial profile.
    pub const fn is_vector_definition(self) -> bool {
        matches!(
            self,
            Self::VectorAddU32
                | Self::VectorMove32
                | Self::VectorLshlrev64
                | Self::VectorAddCarry
                | Self::VectorAddCarryIn
                | Self::GlobalLoadDword
                | Self::VectorCompareGtU64
                | Self::VectorLshlrev32
                | Self::VectorXor32
                | Self::LdsReadB32
        )
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942PhysicalLdsExchangeInstructionV1 {
    pub opcode: Gfx942PhysicalLdsExchangeOpcodeV1,
    pub destination: u8,
    pub source0: u8,
    pub source1: u8,
    pub immediate: u32,
}
impl Gfx942PhysicalLdsExchangeInstructionV1 {
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
            opcode: Gfx942PhysicalLdsExchangeOpcodeV1::from_tag(bytes[0])
                .ok_or(Error::Instruction)?,
            destination: bytes[1],
            source0: bytes[2],
            source1: bytes[3],
            immediate: u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
        };
        value.validate_shape()?;
        Ok(value)
    }
    pub fn validate_shape(self) -> Result<(), Error> {
        use Gfx942PhysicalLdsExchangeOpcodeV1 as O;
        let (d, a, b, immediate) = (self.destination, self.source0, self.source1, self.immediate);
        let unit = |n| n < 64;
        let pair = |n: u8| n < 63 && n.is_multiple_of(2);
        let sdest = |n| (3..64).contains(&n);
        let vdest = |n| (1..64).contains(&n);
        let valid = match self.opcode {
            O::LoadKernargPair => {
                sdest(d) && pair(d) && a == 0 && b == 0 && matches!(immediate, 0 | 8 | 16 | 24)
            }
            O::WaitLgkm0 | O::WaitVm0 | O::Endpgm0 | O::WorkgroupPublishBarrier => {
                d == 0 && a == 0 && b == 0 && immediate == 0
            }
            O::ScalarLshl32 => sdest(d) && unit(a) && b == 0 && immediate == 7,
            O::VectorAddU32 | O::VectorAddCarry => vdest(d) && unit(a) && unit(b) && immediate == 0,
            O::VectorMove32 => vdest(d) && (unit(a) || a == 255) && b == 0 && immediate == 0,
            O::VectorLshlrev64 => vdest(d) && pair(d) && pair(a) && b == 0 && immediate == 2,
            O::VectorAddCarryIn => vdest(d) && unit(a) && unit(b) && immediate == 0,
            O::GlobalLoadDword => vdest(d) && pair(a) && b == 0 && immediate == 0,
            O::VectorCompareGtU64 => d == 0 && pair(a) && pair(b) && immediate == 0,
            O::SaveAndMaskExec => sdest(d) && pair(d) && a == 0 && b == 0 && immediate == 0,
            O::GlobalStoreDword => d == 0 && pair(a) && unit(b) && immediate == 0,
            O::RestoreExec => d == 0 && pair(a) && b == 0 && immediate == 0,
            O::VectorLshlrev32 => vdest(d) && unit(a) && b == 0 && immediate == 2,
            O::VectorXor32 => vdest(d) && unit(a) && b == 0 && immediate == 64,
            O::LdsWriteB32 => d == 0 && unit(a) && unit(b) && immediate == 0,
            O::LdsReadB32 => vdest(d) && unit(a) && b == 0 && immediate == 0,
        };
        if valid {
            Ok(())
        } else {
            Err(Error::Instruction)
        }
    }
    /// Exact prefix: explicit units, then EXEC, then implicit VCC. Shape first.
    pub const fn operand_registers(self) -> [Option<R>; 5] {
        use Gfx942PhysicalLdsExchangeOpcodeV1 as O;
        let (a, b) = (self.source0, self.source1);
        match self.opcode {
            O::LoadKernargPair => [Some(R::Sgpr(0)), Some(R::Sgpr(1)), None, None, None],
            O::ScalarLshl32 => [Some(R::Sgpr(a)), None, None, None, None],
            O::VectorAddU32 | O::VectorAddCarry => [
                Some(R::Sgpr(a)),
                Some(R::Vgpr(b)),
                Some(R::Exec),
                None,
                None,
            ],
            O::VectorMove32 if a == 255 => [Some(R::Exec), None, None, None, None],
            O::VectorMove32 => [Some(R::Sgpr(a)), Some(R::Exec), None, None, None],
            O::VectorLshlrev64 | O::GlobalLoadDword => [
                Some(R::Vgpr(a)),
                Some(R::Vgpr(a.wrapping_add(1))),
                Some(R::Exec),
                None,
                None,
            ],
            O::VectorAddCarryIn => [
                Some(R::Vgpr(a)),
                Some(R::Vgpr(b)),
                Some(R::Exec),
                Some(R::Vcc),
                None,
            ],
            O::VectorCompareGtU64 => [
                Some(R::Sgpr(a)),
                Some(R::Sgpr(a.wrapping_add(1))),
                Some(R::Vgpr(b)),
                Some(R::Vgpr(b.wrapping_add(1))),
                Some(R::Exec),
            ],
            O::SaveAndMaskExec => [Some(R::Vcc), Some(R::Exec), None, None, None],
            O::GlobalStoreDword => [
                Some(R::Vgpr(a)),
                Some(R::Vgpr(a.wrapping_add(1))),
                Some(R::Vgpr(b)),
                Some(R::Exec),
                None,
            ],
            O::RestoreExec => [
                Some(R::Sgpr(a)),
                Some(R::Sgpr(a.wrapping_add(1))),
                None,
                None,
                None,
            ],
            O::VectorLshlrev32 | O::VectorXor32 | O::LdsReadB32 => {
                [Some(R::Vgpr(a)), Some(R::Exec), None, None, None]
            }
            O::LdsWriteB32 => [
                Some(R::Vgpr(a)),
                Some(R::Vgpr(b)),
                Some(R::Exec),
                None,
                None,
            ],
            O::WaitLgkm0 | O::WaitVm0 | O::Endpgm0 | O::WorkgroupPublishBarrier => [None; 5],
        }
    }
    /// A pending load retains this same U32 SSA definition through its wait.
    pub const fn result_registers(self) -> [Option<R>; 4] {
        use Gfx942PhysicalLdsExchangeOpcodeV1 as O;
        let d = self.destination;
        match self.opcode {
            O::LoadKernargPair => [
                Some(R::Sgpr(d)),
                Some(R::Sgpr(d.wrapping_add(1))),
                None,
                None,
            ],
            O::ScalarLshl32 => [Some(R::Sgpr(d)), Some(R::Scc), None, None],
            O::VectorAddU32
            | O::VectorMove32
            | O::GlobalLoadDword
            | O::VectorLshlrev32
            | O::VectorXor32
            | O::LdsReadB32 => [Some(R::Vgpr(d)), None, None, None],
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
            | O::Endpgm0
            | O::LdsWriteB32
            | O::WorkgroupPublishBarrier => [None; 4],
        }
    }
}
