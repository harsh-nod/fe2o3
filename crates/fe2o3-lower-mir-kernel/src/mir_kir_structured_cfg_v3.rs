//! Bounded structured MIR-to-KIR machine semantics used by the V3 CFG proof.
//!
//! This module is deliberately independent of production custody. It models a
//! reducible family with two diamonds, two block-argument transfers, a counted
//! loop, and direct-call depth two. Production claims require the separate
//! exact live-owner classifier; constructing or validating this model grants no
//! compiler, artifact, or launch authority.

use std::{error::Error, fmt};

use sha2::{Digest, Sha256};

/// Version of the structured machine and relation encoding.
pub const MIR_KIR_STRUCTURED_CFG_MODEL_VERSION_V3: u16 = crate::FORMAL_COMPILER_V3_CLAIM_VERSION;
/// Maximum admitted counted-loop trip count.
pub const MIR_KIR_STRUCTURED_CFG_MAX_TRIP_COUNT_V3: u8 =
    crate::FORMAL_COMPILER_V3_MODELED_MAXIMUM_LOOP_TRIP_COUNT;
/// Maximum direct-call stack depth in the model: root -> helper -> leaf.
pub const MIR_KIR_STRUCTURED_CFG_MAX_CALL_DEPTH_V3: u8 =
    crate::FORMAL_COMPILER_V3_MODELED_MAXIMUM_STACK_FRAMES;
/// Stable positive Verus theorem name.
pub const MIR_KIR_STRUCTURED_CFG_THEOREM_V3: &str = "fe2o3_mir_kir_structured_cfg_refines_v3";
/// Digest of the exact positive Verus source accepted by the pinned runner.
pub const MIR_KIR_STRUCTURED_CFG_PROOF_SHA256_V3: [u8; 32] = [
    0x21, 0xff, 0xbd, 0x4c, 0xd1, 0x93, 0xfc, 0xf5, 0x7e, 0x81, 0x27, 0xaa, 0xdd, 0x4c, 0xe7, 0xa4,
    0x78, 0xed, 0xb2, 0x3b, 0x03, 0x99, 0x94, 0xf5, 0xf6, 0x54, 0x74, 0x51, 0xb8, 0x0d, 0x90, 0x21,
];
/// Digest of the pinned Verus executable.
pub const MIR_KIR_STRUCTURED_CFG_VERUS_SHA256_V3: [u8; 32] = [
    0xad, 0x26, 0x69, 0xf5, 0x79, 0xd8, 0x98, 0xed, 0xe5, 0x3f, 0x2b, 0xf8, 0x4e, 0x80, 0xa1, 0xda,
    0xf4, 0xe3, 0x57, 0x87, 0x39, 0xb0, 0xf5, 0x80, 0x7e, 0xf2, 0x09, 0xa0, 0xc9, 0xf3, 0x82, 0xdd,
];
/// Digest of the pinned Verus/vstd/Z3 closure manifest.
pub const MIR_KIR_STRUCTURED_CFG_CLOSURE_SHA256_V3: [u8; 32] = [
    0xd2, 0x8d, 0xf3, 0xfb, 0x5e, 0x0d, 0x74, 0x76, 0x37, 0x54, 0x39, 0x33, 0xdf, 0xc3, 0x8c, 0xff,
    0x45, 0x57, 0x6d, 0xa9, 0xb9, 0x20, 0xd7, 0x55, 0xb4, 0xb7, 0xe9, 0x19, 0xe4, 0x7a, 0x60, 0x19,
];

const MODEL_DOMAIN_V3: &[u8] = b"FE2O3/MIR-KIR/STRUCTURED-CFG/MODEL/V3\0";

/// Closed unsigned scalar-width set modeled by V3.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum MirKirScalarWidthV3 {
    /// Eight-bit unsigned integer.
    U8 = 8,
    /// Sixteen-bit unsigned integer.
    U16 = 16,
    /// Thirty-two-bit unsigned integer.
    U32 = 32,
    /// Sixty-four-bit unsigned integer.
    U64 = 64,
}

impl MirKirScalarWidthV3 {
    const fn mask(self) -> u64 {
        match self {
            Self::U8 => u8::MAX as u64,
            Self::U16 => u16::MAX as u64,
            Self::U32 => u32::MAX as u64,
            Self::U64 => u64::MAX,
        }
    }
}

/// Canonical unsigned scalar value in one admitted width.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MirKirScalarValueV3 {
    width: MirKirScalarWidthV3,
    bits: u64,
}

impl MirKirScalarValueV3 {
    /// Constructs the canonical low-bit interpretation at `width`.
    pub const fn new(width: MirKirScalarWidthV3, bits: u64) -> Self {
        Self {
            width,
            bits: bits & width.mask(),
        }
    }

    /// Returns the scalar width.
    pub const fn width(self) -> MirKirScalarWidthV3 {
        self.width
    }

    /// Returns canonical unsigned bits.
    pub const fn bits(self) -> u64 {
        self.bits
    }

    /// Zero-extension, same-width identity, or truncation to another closed width.
    pub const fn cast(self, target: MirKirScalarWidthV3) -> Self {
        Self::new(target, self.bits)
    }

    /// Unsigned equality.
    pub const fn equals(self, other: Self) -> Option<bool> {
        if self.width as u8 == other.width as u8 {
            Some(self.bits == other.bits)
        } else {
            None
        }
    }

    /// Unsigned less-than.
    pub const fn unsigned_less_than(self, other: Self) -> Option<bool> {
        if self.width as u8 == other.width as u8 {
            Some(self.bits < other.bits)
        } else {
            None
        }
    }

    /// Width-specific wrapping addition.
    pub const fn wrapping_add(self, other: Self) -> Option<Self> {
        if self.width as u8 != other.width as u8 {
            return None;
        }
        Some(Self::new(self.width, self.bits.wrapping_add(other.bits)))
    }

    /// Width-specific checked addition. Overflow is not converted to wrapping.
    pub const fn checked_add(self, other: Self) -> Option<Self> {
        if self.width as u8 != other.width as u8 {
            return None;
        }
        let sum = match self.bits.checked_add(other.bits) {
            Some(sum) => sum,
            None => return None,
        };
        if sum > self.width.mask() {
            None
        } else {
            Some(Self::new(self.width, sum))
        }
    }

    /// Width-specific bitwise exclusive-or.
    pub const fn bit_xor(self, other: Self) -> Option<Self> {
        if self.width as u8 == other.width as u8 {
            Some(Self::new(self.width, self.bits ^ other.bits))
        } else {
            None
        }
    }
}

/// Closed expression operation at the first helper block.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirKirStructuredExpressionV3 {
    /// Bitwise XOR.
    BitXor,
    /// Addition modulo the selected width.
    WrappingAdd,
    /// Checked addition; overflow is an observable terminal outcome.
    CheckedAdd,
}

/// Validated parameters for the structured family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MirKirStructuredProgramV3 {
    value_width: MirKirScalarWidthV3,
    output_width: MirKirScalarWidthV3,
    expression: MirKirStructuredExpressionV3,
    fallback: MirKirScalarValueV3,
    increment: MirKirScalarValueV3,
    threshold: MirKirScalarValueV3,
    trip_count: u8,
}

impl MirKirStructuredProgramV3 {
    /// Validates the closed width relation and bounded, nonempty loop.
    pub fn try_new(
        value_width: MirKirScalarWidthV3,
        output_width: MirKirScalarWidthV3,
        expression: MirKirStructuredExpressionV3,
        fallback: MirKirScalarValueV3,
        increment: MirKirScalarValueV3,
        threshold: MirKirScalarValueV3,
        trip_count: u8,
    ) -> Result<Self, MirKirStructuredCfgErrorV3> {
        if fallback.width != value_width
            || increment.width != value_width
            || threshold.width != value_width
            || !(1..=MIR_KIR_STRUCTURED_CFG_MAX_TRIP_COUNT_V3).contains(&trip_count)
        {
            return Err(MirKirStructuredCfgErrorV3::InvalidProgram);
        }
        Ok(Self {
            value_width,
            output_width,
            expression,
            fallback,
            increment,
            threshold,
            trip_count,
        })
    }

    /// Exact transition fuel for a completed non-overflowing run.
    pub const fn required_fuel(self) -> u16 {
        14 + 2 * self.trip_count as u16
    }

    /// This model object is never production evidence.
    pub const fn grants_authority(self) -> bool {
        false
    }
}

/// Observable result at the caller continuation boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirKirStructuredObservationV3 {
    /// Both diamonds, the counted loop, and both direct calls completed.
    Returned {
        /// Leaf-cast result bound to the root call destination.
        value: MirKirScalarValueV3,
        /// First `expression == 0` edge direction.
        first_zero_edge: bool,
        /// Second unsigned-less-than edge direction.
        second_less_edge: bool,
        /// Exact loop iterations.
        iterations: u8,
        /// Exact maximum direct-call depth.
        max_call_depth: u8,
    },
    /// Checked addition overflowed before either diamond was entered.
    CheckedOverflow {
        /// Stack depth at the checked expression.
        call_depth: u8,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MirPcV3 {
    RootCall,
    Compute,
    FirstBranch,
    FirstArm,
    FirstJoin,
    LoopHeader,
    LoopBody,
    SecondBranch,
    SecondArm,
    SecondJoin,
    LeafCall,
    LeafCast,
    LeafReturn,
    HelperReturn,
    RootContinuation,
    CheckedOverflow,
    Done,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum KirPcV3 {
    RootCall,
    Expression,
    FirstCond,
    FirstEdge,
    FirstPhi,
    LoopPhi,
    LoopBackedge,
    SecondCond,
    SecondEdge,
    SecondPhi,
    LeafCall,
    LeafCast,
    LeafReturn,
    HelperReturn,
    RootContinuation,
    CheckedOverflow,
    Done,
}

#[derive(Clone, Copy)]
struct MirStateV3 {
    pc: MirPcV3,
    left: MirKirScalarValueV3,
    right: MirKirScalarValueV3,
    expression: MirKirScalarValueV3,
    first_phi: MirKirScalarValueV3,
    loop_value: MirKirScalarValueV3,
    iteration: u8,
    second_phi: MirKirScalarValueV3,
    call_destination: MirKirScalarValueV3,
    first_zero: bool,
    second_less: bool,
}

#[derive(Clone, Copy)]
struct KirStateV3 {
    pc: KirPcV3,
    left_parameter: MirKirScalarValueV3,
    right_parameter: MirKirScalarValueV3,
    expression_ssa: MirKirScalarValueV3,
    first_edge_value: MirKirScalarValueV3,
    first_block_parameter: MirKirScalarValueV3,
    loop_block_parameter: MirKirScalarValueV3,
    loop_iteration_parameter: u8,
    second_edge_value: MirKirScalarValueV3,
    second_block_parameter: MirKirScalarValueV3,
    leaf_parameter: MirKirScalarValueV3,
    leaf_result: MirKirScalarValueV3,
    helper_return: MirKirScalarValueV3,
    call_result: MirKirScalarValueV3,
    first_zero: bool,
    second_less: bool,
}

fn evaluate_expression_v3(
    expression: MirKirStructuredExpressionV3,
    left: MirKirScalarValueV3,
    right: MirKirScalarValueV3,
) -> Option<MirKirScalarValueV3> {
    match expression {
        MirKirStructuredExpressionV3::BitXor => left.bit_xor(right),
        MirKirStructuredExpressionV3::WrappingAdd => left.wrapping_add(right),
        MirKirStructuredExpressionV3::CheckedAdd => left.checked_add(right),
    }
}

fn zero_v3(width: MirKirScalarWidthV3) -> MirKirScalarValueV3 {
    MirKirScalarValueV3::new(width, 0)
}

fn observe_mir_v3(state: MirStateV3) -> Option<MirKirStructuredObservationV3> {
    match state.pc {
        MirPcV3::CheckedOverflow => {
            Some(MirKirStructuredObservationV3::CheckedOverflow { call_depth: 1 })
        }
        MirPcV3::Done => Some(MirKirStructuredObservationV3::Returned {
            value: state.call_destination,
            first_zero_edge: state.first_zero,
            second_less_edge: state.second_less,
            iterations: state.iteration,
            max_call_depth: MIR_KIR_STRUCTURED_CFG_MAX_CALL_DEPTH_V3,
        }),
        _ => None,
    }
}

fn observe_kir_v3(state: KirStateV3) -> Option<MirKirStructuredObservationV3> {
    match state.pc {
        KirPcV3::CheckedOverflow => {
            Some(MirKirStructuredObservationV3::CheckedOverflow { call_depth: 1 })
        }
        KirPcV3::Done => Some(MirKirStructuredObservationV3::Returned {
            value: state.call_result,
            first_zero_edge: state.first_zero,
            second_less_edge: state.second_less,
            iterations: state.loop_iteration_parameter,
            max_call_depth: MIR_KIR_STRUCTURED_CFG_MAX_CALL_DEPTH_V3,
        }),
        _ => None,
    }
}

/// Independently executes the semantic-MIR structured machine.
pub fn execute_mir_structured_cfg_v3(
    program: MirKirStructuredProgramV3,
    left: MirKirScalarValueV3,
    right: MirKirScalarValueV3,
    fuel: u16,
) -> Option<MirKirStructuredObservationV3> {
    if left.width != program.value_width || right.width != program.value_width {
        return None;
    }
    let mut state = MirStateV3 {
        pc: MirPcV3::RootCall,
        left,
        right,
        expression: zero_v3(program.value_width),
        first_phi: zero_v3(program.value_width),
        loop_value: zero_v3(program.value_width),
        iteration: 0,
        second_phi: zero_v3(program.value_width),
        call_destination: zero_v3(program.output_width),
        first_zero: false,
        second_less: false,
    };
    for _ in 0..fuel {
        state.pc = match state.pc {
            MirPcV3::RootCall => MirPcV3::Compute,
            MirPcV3::Compute => {
                match evaluate_expression_v3(program.expression, state.left, state.right) {
                    Some(value) => {
                        state.expression = value;
                        MirPcV3::FirstBranch
                    }
                    None if program.expression == MirKirStructuredExpressionV3::CheckedAdd => {
                        MirPcV3::CheckedOverflow
                    }
                    None => return None,
                }
            }
            MirPcV3::FirstBranch => {
                state.first_zero = state.expression.bits == 0;
                MirPcV3::FirstArm
            }
            MirPcV3::FirstArm => {
                state.first_phi = if state.first_zero {
                    state.expression
                } else {
                    program.fallback
                };
                MirPcV3::FirstJoin
            }
            MirPcV3::FirstJoin => {
                state.loop_value = state.first_phi;
                MirPcV3::LoopHeader
            }
            MirPcV3::LoopHeader => {
                if state.iteration < program.trip_count {
                    MirPcV3::LoopBody
                } else {
                    MirPcV3::SecondBranch
                }
            }
            MirPcV3::LoopBody => {
                state.loop_value = state.loop_value.wrapping_add(program.increment)?;
                state.iteration = state.iteration.checked_add(1)?;
                MirPcV3::LoopHeader
            }
            MirPcV3::SecondBranch => {
                state.second_less = state.loop_value.unsigned_less_than(program.threshold)?;
                MirPcV3::SecondArm
            }
            MirPcV3::SecondArm => {
                state.second_phi = if state.second_less {
                    state.loop_value
                } else {
                    program.fallback
                };
                MirPcV3::SecondJoin
            }
            MirPcV3::SecondJoin => MirPcV3::LeafCall,
            MirPcV3::LeafCall => MirPcV3::LeafCast,
            MirPcV3::LeafCast => {
                state.call_destination = state.second_phi.cast(program.output_width);
                MirPcV3::LeafReturn
            }
            MirPcV3::LeafReturn => MirPcV3::HelperReturn,
            MirPcV3::HelperReturn => MirPcV3::RootContinuation,
            MirPcV3::RootContinuation => MirPcV3::Done,
            MirPcV3::CheckedOverflow | MirPcV3::Done => return observe_mir_v3(state),
        };
        if matches!(state.pc, MirPcV3::CheckedOverflow | MirPcV3::Done) {
            return observe_mir_v3(state);
        }
    }
    None
}

#[derive(Clone, Copy)]
struct KirShapeV3 {
    first_branch_reversed: bool,
    first_phi_swapped: bool,
    loop_bound_delta: i8,
    second_branch_reversed: bool,
    second_phi_swapped: bool,
    leaf_callee_changed: bool,
    cast_uses_fallback: bool,
    checked_is_wrapping: bool,
}

impl KirShapeV3 {
    const EXACT: Self = Self {
        first_branch_reversed: false,
        first_phi_swapped: false,
        loop_bound_delta: 0,
        second_branch_reversed: false,
        second_phi_swapped: false,
        leaf_callee_changed: false,
        cast_uses_fallback: false,
        checked_is_wrapping: false,
    };
}

/// Independently executes the canonical-KIR structured machine.
pub fn execute_kir_structured_cfg_v3(
    program: MirKirStructuredProgramV3,
    left: MirKirScalarValueV3,
    right: MirKirScalarValueV3,
    fuel: u16,
) -> Option<MirKirStructuredObservationV3> {
    execute_kir_with_shape_v3(program, left, right, fuel, KirShapeV3::EXACT)
}

fn execute_kir_with_shape_v3(
    program: MirKirStructuredProgramV3,
    left: MirKirScalarValueV3,
    right: MirKirScalarValueV3,
    fuel: u16,
    shape: KirShapeV3,
) -> Option<MirKirStructuredObservationV3> {
    if left.width != program.value_width || right.width != program.value_width {
        return None;
    }
    let zero = zero_v3(program.value_width);
    let mut state = KirStateV3 {
        pc: KirPcV3::RootCall,
        left_parameter: left,
        right_parameter: right,
        expression_ssa: zero,
        first_edge_value: zero,
        first_block_parameter: zero,
        loop_block_parameter: zero,
        loop_iteration_parameter: 0,
        second_edge_value: zero,
        second_block_parameter: zero,
        leaf_parameter: zero,
        leaf_result: zero_v3(program.output_width),
        helper_return: zero_v3(program.output_width),
        call_result: zero_v3(program.output_width),
        first_zero: false,
        second_less: false,
    };
    for _ in 0..fuel {
        state.pc = match state.pc {
            KirPcV3::RootCall => KirPcV3::Expression,
            KirPcV3::Expression => {
                let value = if shape.checked_is_wrapping
                    && program.expression == MirKirStructuredExpressionV3::CheckedAdd
                {
                    state.left_parameter.wrapping_add(state.right_parameter)
                } else {
                    evaluate_expression_v3(
                        program.expression,
                        state.left_parameter,
                        state.right_parameter,
                    )
                };
                match value {
                    Some(value) => {
                        state.expression_ssa = value;
                        KirPcV3::FirstCond
                    }
                    None if program.expression == MirKirStructuredExpressionV3::CheckedAdd => {
                        KirPcV3::CheckedOverflow
                    }
                    None => return None,
                }
            }
            KirPcV3::FirstCond => {
                state.first_zero = (state.expression_ssa.bits == 0) ^ shape.first_branch_reversed;
                KirPcV3::FirstEdge
            }
            KirPcV3::FirstEdge => {
                state.first_edge_value = if state.first_zero ^ shape.first_phi_swapped {
                    state.expression_ssa
                } else {
                    program.fallback
                };
                KirPcV3::FirstPhi
            }
            KirPcV3::FirstPhi => {
                state.first_block_parameter = state.first_edge_value;
                state.loop_block_parameter = state.first_block_parameter;
                KirPcV3::LoopPhi
            }
            KirPcV3::LoopPhi => {
                let bound = (i16::from(program.trip_count) + i16::from(shape.loop_bound_delta))
                    .clamp(0, i16::from(MIR_KIR_STRUCTURED_CFG_MAX_TRIP_COUNT_V3))
                    as u8;
                if state.loop_iteration_parameter < bound {
                    KirPcV3::LoopBackedge
                } else {
                    KirPcV3::SecondCond
                }
            }
            KirPcV3::LoopBackedge => {
                state.loop_block_parameter =
                    state.loop_block_parameter.wrapping_add(program.increment)?;
                state.loop_iteration_parameter = state.loop_iteration_parameter.checked_add(1)?;
                KirPcV3::LoopPhi
            }
            KirPcV3::SecondCond => {
                state.second_less = state
                    .loop_block_parameter
                    .unsigned_less_than(program.threshold)?
                    ^ shape.second_branch_reversed;
                KirPcV3::SecondEdge
            }
            KirPcV3::SecondEdge => {
                state.second_edge_value = if state.second_less ^ shape.second_phi_swapped {
                    state.loop_block_parameter
                } else {
                    program.fallback
                };
                KirPcV3::SecondPhi
            }
            KirPcV3::SecondPhi => {
                state.second_block_parameter = state.second_edge_value;
                state.leaf_parameter = state.second_block_parameter;
                KirPcV3::LeafCall
            }
            KirPcV3::LeafCall => {
                if shape.leaf_callee_changed {
                    return None;
                }
                KirPcV3::LeafCast
            }
            KirPcV3::LeafCast => {
                state.leaf_result = if shape.cast_uses_fallback {
                    program.fallback.cast(program.output_width)
                } else {
                    state.leaf_parameter.cast(program.output_width)
                };
                KirPcV3::LeafReturn
            }
            KirPcV3::LeafReturn => {
                state.helper_return = state.leaf_result;
                KirPcV3::HelperReturn
            }
            KirPcV3::HelperReturn => {
                state.call_result = state.helper_return;
                KirPcV3::RootContinuation
            }
            KirPcV3::RootContinuation => KirPcV3::Done,
            KirPcV3::CheckedOverflow | KirPcV3::Done => return observe_kir_v3(state),
        };
        if matches!(state.pc, KirPcV3::CheckedOverflow | KirPcV3::Done) {
            return observe_kir_v3(state);
        }
    }
    None
}

/// Executes both independent machines and compares their observations.
pub fn mir_kir_structured_cfg_refines_v3(
    program: MirKirStructuredProgramV3,
    left: MirKirScalarValueV3,
    right: MirKirScalarValueV3,
    fuel: u16,
) -> bool {
    match (
        execute_mir_structured_cfg_v3(program, left, right, fuel),
        execute_kir_structured_cfg_v3(program, left, right, fuel),
    ) {
        (Some(mir), Some(kir)) => mir == kir,
        _ => false,
    }
}

/// Exact topology/value relation consumed by a future production classifier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MirKirStructuredRelationV3 {
    /// Direct-call targets at stack depths one and two.
    pub mir_callees: [u32; 2],
    /// Related KIR direct-call targets.
    pub kir_callees: [u32; 2],
    /// MIR edge values entering first join, loop header, and second join.
    pub mir_edge_values: [u32; 3],
    /// KIR edge values entering first join, loop header, and second join.
    pub kir_edge_values: [u32; 3],
    /// MIR block arguments for first join, loop value, and second join.
    pub mir_block_arguments: [u32; 3],
    /// KIR block parameters for first join, loop value, and second join.
    pub kir_block_parameters: [u32; 3],
    /// MIR/KIR loop bound and backedge targets.
    pub mir_loop: [u32; 2],
    /// Related KIR loop bound and backedge targets.
    pub kir_loop: [u32; 2],
    /// MIR expression result, helper return, and root destination.
    pub mir_results: [u32; 3],
    /// KIR expression SSA, helper return operand, and call result.
    pub kir_results: [u32; 3],
}

/// Checks every control/value axis of a candidate structured relation.
pub fn validate_structured_relation_v3(
    relation: &MirKirStructuredRelationV3,
) -> Result<(), MirKirStructuredCfgErrorV3> {
    if relation.mir_callees != relation.kir_callees
        || relation.mir_edge_values != relation.kir_edge_values
        || relation.mir_block_arguments != relation.kir_block_parameters
        || relation.mir_loop != relation.kir_loop
        || relation.mir_results != relation.kir_results
        || relation.mir_callees[0] == relation.mir_callees[1]
        || relation.mir_loop[0] == 0
        || relation.mir_loop[0] > u32::from(MIR_KIR_STRUCTURED_CFG_MAX_TRIP_COUNT_V3)
    {
        return Err(MirKirStructuredCfgErrorV3::RelationMismatch);
    }
    Ok(())
}

/// Fail-closed V3 model/validator error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirKirStructuredCfgErrorV3 {
    /// Widths or loop bounds are outside the closed model.
    InvalidProgram,
    /// A call, edge, block argument, loop, or result relation differs.
    RelationMismatch,
}

impl fmt::Display for MirKirStructuredCfgErrorV3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidProgram => formatter.write_str("invalid bounded structured CFG program"),
            Self::RelationMismatch => formatter.write_str("structured MIR/KIR relation mismatch"),
        }
    }
}

impl Error for MirKirStructuredCfgErrorV3 {}

/// Domain-separated identity of the model, proof, and closure.
pub fn mir_kir_structured_cfg_model_identity_v3() -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(MODEL_DOMAIN_V3);
    hash.update(MIR_KIR_STRUCTURED_CFG_MODEL_VERSION_V3.to_le_bytes());
    hash.update(MIR_KIR_STRUCTURED_CFG_MAX_TRIP_COUNT_V3.to_le_bytes());
    hash.update(MIR_KIR_STRUCTURED_CFG_MAX_CALL_DEPTH_V3.to_le_bytes());
    hash.update(MIR_KIR_STRUCTURED_CFG_THEOREM_V3.as_bytes());
    hash.update(crate::FORMAL_COMPILER_V3_CLAIM_NAME.as_bytes());
    hash.update(crate::FORMAL_COMPILER_V3_MODELED_MINIMUM_LOOP_TRIP_COUNT.to_le_bytes());
    for &(operation, tag) in crate::FORMAL_COMPILER_V3_PRODUCTION_SCALAR_OPERATIONS
        .iter()
        .chain(crate::FORMAL_COMPILER_V3_MODELED_ONLY_SCALAR_OPERATIONS)
    {
        hash.update(operation.as_bytes());
        hash.update(tag.to_le_bytes());
    }
    hash.update(MIR_KIR_STRUCTURED_CFG_PROOF_SHA256_V3);
    hash.update(MIR_KIR_STRUCTURED_CFG_VERUS_SHA256_V3);
    hash.update(MIR_KIR_STRUCTURED_CFG_CLOSURE_SHA256_V3);
    hash.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn program(
        width: MirKirScalarWidthV3,
        expression: MirKirStructuredExpressionV3,
    ) -> MirKirStructuredProgramV3 {
        MirKirStructuredProgramV3::try_new(
            width,
            MirKirScalarWidthV3::U16,
            expression,
            MirKirScalarValueV3::new(width, 7),
            MirKirScalarValueV3::new(width, 3),
            MirKirScalarValueV3::new(width, 40),
            3,
        )
        .unwrap()
    }

    #[test]
    fn every_width_executes_two_diamonds_loop_and_two_calls() {
        for width in [
            MirKirScalarWidthV3::U8,
            MirKirScalarWidthV3::U16,
            MirKirScalarWidthV3::U32,
            MirKirScalarWidthV3::U64,
        ] {
            let program = program(width, MirKirStructuredExpressionV3::BitXor);
            let left = MirKirScalarValueV3::new(width, 5);
            let right = MirKirScalarValueV3::new(width, 5);
            assert!(mir_kir_structured_cfg_refines_v3(
                program,
                left,
                right,
                program.required_fuel()
            ));
            let MirKirStructuredObservationV3::Returned {
                iterations,
                max_call_depth,
                first_zero_edge,
                ..
            } = execute_mir_structured_cfg_v3(program, left, right, program.required_fuel())
                .unwrap()
            else {
                panic!("non-overflowing XOR unexpectedly trapped")
            };
            assert_eq!(iterations, 3);
            assert_eq!(max_call_depth, 2);
            assert!(first_zero_edge);
            assert_eq!(
                execute_mir_structured_cfg_v3(program, left, right, program.required_fuel() - 1),
                None
            );
        }
    }

    #[test]
    fn checked_overflow_is_not_wrapping_addition() {
        let checked = program(
            MirKirScalarWidthV3::U8,
            MirKirStructuredExpressionV3::CheckedAdd,
        );
        let wrapping = program(
            MirKirScalarWidthV3::U8,
            MirKirStructuredExpressionV3::WrappingAdd,
        );
        let left = MirKirScalarValueV3::new(MirKirScalarWidthV3::U8, 250);
        let right = MirKirScalarValueV3::new(MirKirScalarWidthV3::U8, 10);
        assert_eq!(
            execute_mir_structured_cfg_v3(checked, left, right, checked.required_fuel()),
            Some(MirKirStructuredObservationV3::CheckedOverflow { call_depth: 1 })
        );
        assert!(matches!(
            execute_mir_structured_cfg_v3(wrapping, left, right, wrapping.required_fuel()),
            Some(MirKirStructuredObservationV3::Returned { .. })
        ));
        assert_ne!(
            execute_kir_with_shape_v3(
                checked,
                left,
                right,
                checked.required_fuel(),
                KirShapeV3 {
                    checked_is_wrapping: true,
                    ..KirShapeV3::EXACT
                }
            ),
            execute_mir_structured_cfg_v3(checked, left, right, checked.required_fuel())
        );
    }

    #[test]
    fn hostile_cfg_and_value_mutations_change_or_reject_observation() {
        let program = program(
            MirKirScalarWidthV3::U8,
            MirKirStructuredExpressionV3::BitXor,
        );
        let left = MirKirScalarValueV3::new(MirKirScalarWidthV3::U8, 1);
        let right = MirKirScalarValueV3::new(MirKirScalarWidthV3::U8, 2);
        let expected = execute_mir_structured_cfg_v3(program, left, right, program.required_fuel());
        let mutations = [
            KirShapeV3 {
                first_branch_reversed: true,
                ..KirShapeV3::EXACT
            },
            KirShapeV3 {
                first_phi_swapped: true,
                ..KirShapeV3::EXACT
            },
            KirShapeV3 {
                loop_bound_delta: -1,
                ..KirShapeV3::EXACT
            },
            KirShapeV3 {
                second_branch_reversed: true,
                ..KirShapeV3::EXACT
            },
            KirShapeV3 {
                second_phi_swapped: true,
                ..KirShapeV3::EXACT
            },
            KirShapeV3 {
                leaf_callee_changed: true,
                ..KirShapeV3::EXACT
            },
            KirShapeV3 {
                cast_uses_fallback: true,
                ..KirShapeV3::EXACT
            },
        ];
        for mutation in mutations {
            assert_ne!(
                expected,
                execute_kir_with_shape_v3(program, left, right, program.required_fuel(), mutation)
            );
        }
    }

    #[test]
    fn relation_validator_fails_closed_on_each_axis() {
        let exact = MirKirStructuredRelationV3 {
            mir_callees: [1, 2],
            kir_callees: [1, 2],
            mir_edge_values: [3, 4, 5],
            kir_edge_values: [3, 4, 5],
            mir_block_arguments: [6, 7, 8],
            kir_block_parameters: [6, 7, 8],
            mir_loop: [3, 9],
            kir_loop: [3, 9],
            mir_results: [10, 11, 12],
            kir_results: [10, 11, 12],
        };
        validate_structured_relation_v3(&exact).unwrap();
        let mut mutations = [exact; 5];
        mutations[0].kir_callees[1] = 99;
        mutations[1].kir_edge_values[0] = 99;
        mutations[2].kir_block_parameters[1] = 99;
        mutations[3].kir_loop[1] = 99;
        mutations[4].kir_results[2] = 99;
        for mutation in mutations {
            assert_eq!(
                validate_structured_relation_v3(&mutation),
                Err(MirKirStructuredCfgErrorV3::RelationMismatch)
            );
        }
    }

    #[test]
    fn scalar_cast_comparison_and_program_bounds_are_closed() {
        let wide = MirKirScalarValueV3::new(MirKirScalarWidthV3::U64, 0x1_0001);
        let narrow = wide.cast(MirKirScalarWidthV3::U8);
        assert_eq!(narrow.bits(), 1);
        assert_eq!(narrow.cast(MirKirScalarWidthV3::U32).bits(), 1);
        assert_eq!(
            narrow.equals(MirKirScalarValueV3::new(MirKirScalarWidthV3::U8, 1)),
            Some(true)
        );
        assert_eq!(
            narrow.unsigned_less_than(MirKirScalarValueV3::new(MirKirScalarWidthV3::U8, 2)),
            Some(true)
        );
        assert_eq!(
            narrow.equals(MirKirScalarValueV3::new(MirKirScalarWidthV3::U16, 1)),
            None
        );
        assert_eq!(
            MirKirStructuredProgramV3::try_new(
                MirKirScalarWidthV3::U8,
                MirKirScalarWidthV3::U8,
                MirKirStructuredExpressionV3::BitXor,
                narrow,
                narrow,
                narrow,
                0,
            ),
            Err(MirKirStructuredCfgErrorV3::InvalidProgram)
        );
    }

    #[test]
    fn model_identity_binds_proof_and_closure() {
        assert_ne!(mir_kir_structured_cfg_model_identity_v3(), [0; 32]);
        assert_eq!(
            Sha256::digest(include_bytes!("../verus/mir_kir_structured_cfg_v3.rs")).as_slice(),
            MIR_KIR_STRUCTURED_CFG_PROOF_SHA256_V3
        );
        assert_eq!(
            Sha256::digest(include_bytes!("../verus/pins/VERUS_CLOSURE_MANIFEST")).as_slice(),
            MIR_KIR_STRUCTURED_CFG_CLOSURE_SHA256_V3
        );
    }
}
