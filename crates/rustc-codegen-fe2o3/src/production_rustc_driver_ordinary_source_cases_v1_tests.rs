//! Source selections for the ordinary Rust checked-output test harness.

use super::{
    guarded_loop_read, masked_shift_source, numeric_cast_source, saturating_source, shift_source,
};

pub(super) enum OrdinarySourceCase {
    GuardedLoopRead(guarded_loop_read::Case),
    NominalPolicy4ErasedControl,
    ConditionalDescriptorPair,
    ReferenceFill,
    ProofFill,
    MaskedShift(masked_shift_source::Config),
    ConstantShift(shift_source::Config),
    ScalarBorrowPolicy5,
    RetainedScalarBorrowPolicy5,
    ScalarBorrowPolicy5Barrier,
    NumericCast(numeric_cast_source::Config),
    F32Exp,
    RetainedF32Exp,
    SaturatingInteger(saturating_source::Config),
    Fill,
    UnannotatedFill,
    Vecadd,
    WrappedFill,
    RetainedWrappedFill,
    SharedUnitHelper,
    PrivateUnitHelper,
    RetainedPrivateUnitHelper,
    F32Negate,
    F32Divide,
    RetainedF32Negate,
    RetainedF32Divide,
}
