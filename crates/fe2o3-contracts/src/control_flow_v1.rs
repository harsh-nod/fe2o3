//! Target-neutral source control-flow declarations.
//!
//! These values describe the finite control-flow subset accepted by the Rust
//! frontend. They neither prove that a loop respects its bound nor authorize
//! later lowering or execution.

use core::fmt;

/// Maximum number of source loops admitted in one kernel declaration.
pub const MAX_SOURCE_LOOPS_V1: u16 = 256;
/// Maximum number of source integer switches admitted in one kernel declaration.
pub const MAX_SOURCE_INTEGER_SWITCHES_V1: u16 = 256;
/// Maximum number of canonical cases admitted in one integer switch.
pub const MAX_INTEGER_SWITCH_CASES_V1: u16 = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControlFlowContractErrorV1 {
    ZeroLoopBound,
    UnsupportedIntegerWidth(u16),
    IntegerCaseOutOfRange,
}

impl fmt::Display for ControlFlowContractErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroLoopBound => formatter.write_str("loop iteration bound must be nonzero"),
            Self::UnsupportedIntegerWidth(width) => {
                write!(formatter, "integer switch width {width} is unsupported")
            }
            Self::IntegerCaseOutOfRange => {
                formatter.write_str("integer switch case is outside its discriminant type")
            }
        }
    }
}

/// A finite source-level upper bound for one loop invocation.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(transparent)]
pub struct LoopBoundV1(u32);

impl LoopBoundV1 {
    pub const fn new(max_iterations: u32) -> Result<Self, ControlFlowContractErrorV1> {
        if max_iterations == 0 {
            return Err(ControlFlowContractErrorV1::ZeroLoopBound);
        }
        Ok(Self(max_iterations))
    }

    pub const fn max_iterations(self) -> u32 {
        self.0
    }
}

/// Fixed-width Rust integer type used as a `match` discriminant.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct IntegerSwitchTypeV1 {
    width: u16,
    signed: bool,
}

impl IntegerSwitchTypeV1 {
    pub const I8: Self = Self::new_unchecked(8, true);
    pub const I16: Self = Self::new_unchecked(16, true);
    pub const I32: Self = Self::new_unchecked(32, true);
    pub const I64: Self = Self::new_unchecked(64, true);
    pub const I128: Self = Self::new_unchecked(128, true);
    pub const U8: Self = Self::new_unchecked(8, false);
    pub const U16: Self = Self::new_unchecked(16, false);
    pub const U32: Self = Self::new_unchecked(32, false);
    pub const U64: Self = Self::new_unchecked(64, false);
    pub const U128: Self = Self::new_unchecked(128, false);

    pub const fn new(width: u16, signed: bool) -> Result<Self, ControlFlowContractErrorV1> {
        if !matches!(width, 8 | 16 | 32 | 64 | 128) {
            return Err(ControlFlowContractErrorV1::UnsupportedIntegerWidth(width));
        }
        Ok(Self { width, signed })
    }

    const fn new_unchecked(width: u16, signed: bool) -> Self {
        Self { width, signed }
    }

    pub const fn width(self) -> u16 {
        self.width
    }

    pub const fn is_signed(self) -> bool {
        self.signed
    }

    pub const fn contains(self, value: i128) -> bool {
        if self.signed {
            if self.width == 128 {
                return true;
            }
            let magnitude_bits = self.width - 1;
            let maximum = (1_i128 << magnitude_bits) - 1;
            let minimum = -(1_i128 << magnitude_bits);
            value >= minimum && value <= maximum
        } else if value < 0 {
            false
        } else if self.width == 128 {
            true
        } else {
            value < (1_i128 << self.width)
        }
    }
}

/// Canonical integer case value interpreted under an explicit switch type.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(transparent)]
pub struct IntegerSwitchCaseV1(i128);

impl IntegerSwitchCaseV1 {
    pub const fn new(
        ty: IntegerSwitchTypeV1,
        value: i128,
    ) -> Result<Self, ControlFlowContractErrorV1> {
        if !ty.contains(value) {
            return Err(ControlFlowContractErrorV1::IntegerCaseOutOfRange);
        }
        Ok(Self(value))
    }

    pub const fn value(self) -> i128 {
        self.0
    }
}

/// Structured transfer represented by the source sidecar.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum StructuredTransferKindV1 {
    Break,
    Continue,
}

/// Control flow deliberately excluded from the V1 frontend subset.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum UnsupportedControlFlowV1 {
    UnboundedLoop,
    IrreducibleGraph,
    NonIntegerMatch,
    GuardedMatchArm,
    BreakWithValue,
}
