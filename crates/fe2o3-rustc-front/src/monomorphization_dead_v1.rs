//! Versioned, fail-closed policy records for monomorphization-dead branches.
//!
//! This module describes portable folding mechanics and canonical observations.
//! Public values are deliberately inert: constructing one does not prove that a
//! compiler observed the recorded MIR, and grants no authority to omit code or
//! analysis. A compiler integration must derive its own private observation.

use std::collections::BTreeSet;
use std::fmt;

use sha2::{Digest as _, Sha256};

pub const CONSTANT_FOLD_POLICY_VERSION_V1: u16 = 1;
pub const MAX_DEAD_BRANCH_DECISIONS_V1: usize = 4096;
pub const MAX_DEAD_SUCCESSORS_PER_BRANCH_V1: usize = 256;

const EVIDENCE_FORMAT_VERSION_V1: u16 = 1;
const EVIDENCE_DOMAIN_V1: [u8; 8] = *b"FE2MDBE\0";

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FixedWidthIntegerV1 {
    width: u16,
    signed: bool,
    bits: u128,
}

impl FixedWidthIntegerV1 {
    pub const fn new(width: u16, signed: bool, bits: u128) -> Result<Self, ConstantFoldFailureV1> {
        if !supported_width(width) {
            return Err(ConstantFoldFailureV1::UnsupportedIntegerWidth(width));
        }
        if width == 1 && signed {
            return Err(ConstantFoldFailureV1::SignedBoolean);
        }
        if width < 128 && bits >= (1_u128 << width) {
            return Err(ConstantFoldFailureV1::IntegerOutOfRange { width, bits });
        }
        Ok(Self {
            width,
            signed,
            bits,
        })
    }

    pub const fn boolean(value: bool) -> Self {
        Self {
            width: 1,
            signed: false,
            bits: value as u128,
        }
    }

    pub const fn width(self) -> u16 {
        self.width
    }

    pub const fn is_signed(self) -> bool {
        self.signed
    }

    pub const fn bits(self) -> u128 {
        self.bits
    }

    fn signed_value(self) -> i128 {
        if self.width == 128 {
            return self.bits as i128;
        }
        let sign = 1_u128 << (self.width - 1);
        if self.bits & sign == 0 {
            self.bits as i128
        } else {
            (self.bits | (!0_u128 << self.width)) as i128
        }
    }

    fn from_signed(width: u16, value: i128) -> Result<Self, ConstantFoldFailureV1> {
        let (minimum, maximum) = signed_bounds(width);
        if value < minimum || value > maximum {
            return Err(ConstantFoldFailureV1::Overflow);
        }
        let bits = if width == 128 {
            value as u128
        } else {
            (value as u128) & ((1_u128 << width) - 1)
        };
        Self::new(width, true, bits)
    }

    fn from_unsigned(width: u16, value: u128) -> Result<Self, ConstantFoldFailureV1> {
        if width < 128 && value >= (1_u128 << width) {
            return Err(ConstantFoldFailureV1::Overflow);
        }
        Self::new(width, false, value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConstantFoldInputV1 {
    Known(FixedWidthIntegerV1),
    Unknown,
    Poison,
    TargetDependent,
}

impl ConstantFoldInputV1 {
    fn known(self) -> Result<FixedWidthIntegerV1, ConstantFoldFailureV1> {
        match self {
            Self::Known(value) => Ok(value),
            Self::Unknown => Err(ConstantFoldFailureV1::Unknown),
            Self::Poison => Err(ConstantFoldFailureV1::Poison),
            Self::TargetDependent => Err(ConstantFoldFailureV1::TargetDependent),
        }
    }
}

impl From<FixedWidthIntegerV1> for ConstantFoldInputV1 {
    fn from(value: FixedWidthIntegerV1) -> Self {
        Self::Known(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConstantFoldBinaryOpV1 {
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
    BitAnd,
    BitOr,
    BitXor,
    ShiftLeft,
    ShiftRight,
    Equal,
    NotEqual,
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConstantFoldFailureV1 {
    UnsupportedPolicyVersion(u16),
    UnsupportedIntegerWidth(u16),
    SignedBoolean,
    IntegerOutOfRange { width: u16, bits: u128 },
    TypeMismatch,
    Unknown,
    Poison,
    TargetDependent,
    Overflow,
    DivisionByZero,
    InvalidShift { width: u16, amount: u128 },
}

impl fmt::Display for ConstantFoldFailureV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedPolicyVersion(version) => {
                write!(
                    formatter,
                    "unsupported constant-fold policy version {version}"
                )
            }
            Self::UnsupportedIntegerWidth(width) => {
                write!(formatter, "unsupported fixed integer width {width}")
            }
            Self::SignedBoolean => formatter.write_str("one-bit integers must be unsigned"),
            Self::IntegerOutOfRange { width, bits } => {
                write!(formatter, "integer bits {bits:#034x} exceed width {width}")
            }
            Self::TypeMismatch => {
                formatter.write_str("constant-fold operands have different fixed-width types")
            }
            Self::Unknown => formatter.write_str("constant-fold input is unknown"),
            Self::Poison => formatter.write_str("constant-fold input is poison"),
            Self::TargetDependent => {
                formatter.write_str("constant-fold input depends on target layout")
            }
            Self::Overflow => formatter.write_str("constant-fold operation overflows"),
            Self::DivisionByZero => formatter.write_str("constant-fold division by zero"),
            Self::InvalidShift { width, amount } => {
                write!(
                    formatter,
                    "constant-fold shift {amount} exceeds width {width}"
                )
            }
        }
    }
}

impl std::error::Error for ConstantFoldFailureV1 {}

pub fn fold_binary_v1(
    policy_version: u16,
    operation: ConstantFoldBinaryOpV1,
    left: ConstantFoldInputV1,
    right: ConstantFoldInputV1,
) -> Result<FixedWidthIntegerV1, ConstantFoldFailureV1> {
    require_policy_v1(policy_version)?;
    let left = left.known()?;
    let right = right.known()?;
    if left.width != right.width || left.signed != right.signed {
        return Err(ConstantFoldFailureV1::TypeMismatch);
    }

    use ConstantFoldBinaryOpV1 as Op;
    match operation {
        Op::Equal
        | Op::NotEqual
        | Op::LessThan
        | Op::LessThanOrEqual
        | Op::GreaterThan
        | Op::GreaterThanOrEqual => {
            let ordering = if left.signed {
                left.signed_value().cmp(&right.signed_value())
            } else {
                left.bits.cmp(&right.bits)
            };
            let result = match operation {
                Op::Equal => ordering.is_eq(),
                Op::NotEqual => !ordering.is_eq(),
                Op::LessThan => ordering.is_lt(),
                Op::LessThanOrEqual => !ordering.is_gt(),
                Op::GreaterThan => ordering.is_gt(),
                Op::GreaterThanOrEqual => !ordering.is_lt(),
                _ => unreachable!(),
            };
            Ok(FixedWidthIntegerV1::boolean(result))
        }
        Op::BitAnd => FixedWidthIntegerV1::new(left.width, left.signed, left.bits & right.bits),
        Op::BitOr => FixedWidthIntegerV1::new(left.width, left.signed, left.bits | right.bits),
        Op::BitXor => FixedWidthIntegerV1::new(left.width, left.signed, left.bits ^ right.bits),
        Op::ShiftLeft | Op::ShiftRight => fold_shift(operation, left, right.bits),
        Op::Add | Op::Subtract | Op::Multiply | Op::Divide | Op::Remainder => {
            fold_arithmetic(operation, left, right)
        }
    }
}

fn fold_arithmetic(
    operation: ConstantFoldBinaryOpV1,
    left: FixedWidthIntegerV1,
    right: FixedWidthIntegerV1,
) -> Result<FixedWidthIntegerV1, ConstantFoldFailureV1> {
    use ConstantFoldBinaryOpV1 as Op;
    if left.signed {
        let left_value = left.signed_value();
        let right_value = right.signed_value();
        let result = match operation {
            Op::Add => left_value.checked_add(right_value),
            Op::Subtract => left_value.checked_sub(right_value),
            Op::Multiply => left_value.checked_mul(right_value),
            Op::Divide => {
                if right_value == 0 {
                    return Err(ConstantFoldFailureV1::DivisionByZero);
                }
                left_value.checked_div(right_value)
            }
            Op::Remainder => {
                if right_value == 0 {
                    return Err(ConstantFoldFailureV1::DivisionByZero);
                }
                left_value.checked_rem(right_value)
            }
            _ => unreachable!(),
        }
        .ok_or(ConstantFoldFailureV1::Overflow)?;
        FixedWidthIntegerV1::from_signed(left.width, result)
    } else {
        let result = match operation {
            Op::Add => left.bits.checked_add(right.bits),
            Op::Subtract => left.bits.checked_sub(right.bits),
            Op::Multiply => left.bits.checked_mul(right.bits),
            Op::Divide => {
                if right.bits == 0 {
                    return Err(ConstantFoldFailureV1::DivisionByZero);
                }
                left.bits.checked_div(right.bits)
            }
            Op::Remainder => {
                if right.bits == 0 {
                    return Err(ConstantFoldFailureV1::DivisionByZero);
                }
                left.bits.checked_rem(right.bits)
            }
            _ => unreachable!(),
        }
        .ok_or(ConstantFoldFailureV1::Overflow)?;
        FixedWidthIntegerV1::from_unsigned(left.width, result)
    }
}

fn fold_shift(
    operation: ConstantFoldBinaryOpV1,
    value: FixedWidthIntegerV1,
    amount: u128,
) -> Result<FixedWidthIntegerV1, ConstantFoldFailureV1> {
    if amount >= u128::from(value.width) {
        return Err(ConstantFoldFailureV1::InvalidShift {
            width: value.width,
            amount,
        });
    }
    let amount = amount as u32;
    match operation {
        ConstantFoldBinaryOpV1::ShiftLeft => {
            let shifted = value.bits << amount;
            let round_trip = if value.signed {
                (shifted as i128) >> amount == value.signed_value()
            } else {
                shifted >> amount == value.bits
            };
            if !round_trip {
                return Err(ConstantFoldFailureV1::Overflow);
            }
            if value.signed {
                FixedWidthIntegerV1::from_signed(value.width, shifted as i128)
            } else {
                FixedWidthIntegerV1::from_unsigned(value.width, shifted)
            }
        }
        ConstantFoldBinaryOpV1::ShiftRight if value.signed => {
            FixedWidthIntegerV1::from_signed(value.width, value.signed_value() >> amount)
        }
        ConstantFoldBinaryOpV1::ShiftRight => {
            FixedWidthIntegerV1::from_unsigned(value.width, value.bits >> amount)
        }
        _ => unreachable!(),
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ConstantSwitchCaseV1 {
    value: FixedWidthIntegerV1,
    target: u32,
}

impl ConstantSwitchCaseV1 {
    pub const fn new(value: FixedWidthIntegerV1, target: u32) -> Self {
        Self { value, target }
    }

    pub const fn value(self) -> FixedWidthIntegerV1 {
        self.value
    }

    pub const fn target(self) -> u32 {
        self.target
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConstantSwitchV1 {
    branch_block: u32,
    discriminant: ConstantFoldInputV1,
    cases: Vec<ConstantSwitchCaseV1>,
    otherwise: u32,
}

impl ConstantSwitchV1 {
    pub fn new(
        branch_block: u32,
        discriminant: ConstantFoldInputV1,
        mut cases: Vec<ConstantSwitchCaseV1>,
        otherwise: u32,
    ) -> Result<Self, MonomorphizationDeadEvidenceErrorV1> {
        if cases.len() > MAX_DEAD_SUCCESSORS_PER_BRANCH_V1 {
            return Err(MonomorphizationDeadEvidenceErrorV1::TooMany {
                field: "constant switch cases",
                maximum: MAX_DEAD_SUCCESSORS_PER_BRANCH_V1,
            });
        }
        if let ConstantFoldInputV1::Known(discriminant) = discriminant {
            for case in &cases {
                if case.value.width != discriminant.width
                    || case.value.signed != discriminant.signed
                {
                    return Err(MonomorphizationDeadEvidenceErrorV1::Fold(
                        ConstantFoldFailureV1::TypeMismatch,
                    ));
                }
            }
        }
        cases.sort_unstable_by_key(|case| case.value.bits);
        if cases
            .windows(2)
            .any(|pair| pair[0].value.bits == pair[1].value.bits)
        {
            return Err(MonomorphizationDeadEvidenceErrorV1::DuplicateSwitchCase);
        }
        Ok(Self {
            branch_block,
            discriminant,
            cases,
            otherwise,
        })
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DeadBranchDecisionV1 {
    branch_block: u32,
    selected_successor: u32,
    discriminant: FixedWidthIntegerV1,
    dead_successors: Vec<u32>,
}

impl DeadBranchDecisionV1 {
    fn new(
        branch_block: u32,
        selected_successor: u32,
        discriminant: FixedWidthIntegerV1,
        mut dead_successors: Vec<u32>,
    ) -> Result<Self, MonomorphizationDeadEvidenceErrorV1> {
        dead_successors.sort_unstable();
        dead_successors.dedup();
        if dead_successors.is_empty() {
            return Err(MonomorphizationDeadEvidenceErrorV1::NoDeadSuccessor { branch_block });
        }
        if dead_successors.len() > MAX_DEAD_SUCCESSORS_PER_BRANCH_V1 {
            return Err(MonomorphizationDeadEvidenceErrorV1::TooMany {
                field: "dead branch successors",
                maximum: MAX_DEAD_SUCCESSORS_PER_BRANCH_V1,
            });
        }
        if dead_successors.binary_search(&selected_successor).is_ok() {
            return Err(
                MonomorphizationDeadEvidenceErrorV1::SelectedSuccessorIsDead { branch_block },
            );
        }
        Ok(Self {
            branch_block,
            selected_successor,
            discriminant,
            dead_successors,
        })
    }

    pub const fn branch_block(&self) -> u32 {
        self.branch_block
    }

    pub const fn selected_successor(&self) -> u32 {
        self.selected_successor
    }

    pub const fn discriminant(&self) -> FixedWidthIntegerV1 {
        self.discriminant
    }

    pub fn dead_successors(&self) -> &[u32] {
        &self.dead_successors
    }
}

pub fn prove_constant_switch_v1(
    policy_version: u16,
    switch: &ConstantSwitchV1,
) -> Result<DeadBranchDecisionV1, MonomorphizationDeadEvidenceErrorV1> {
    require_policy_v1(policy_version)?;
    let discriminant = switch.discriminant.known()?;
    let selected_successor = switch
        .cases
        .iter()
        .find_map(|case| (case.value.bits == discriminant.bits).then_some(case.target))
        .unwrap_or(switch.otherwise);
    let mut successors = switch
        .cases
        .iter()
        .map(|case| case.target)
        .collect::<BTreeSet<_>>();
    successors.insert(switch.otherwise);
    successors.remove(&selected_successor);
    DeadBranchDecisionV1::new(
        switch.branch_block,
        selected_successor,
        discriminant,
        successors.into_iter().collect(),
    )
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct DeadBranchContextV1 {
    function_identity: [u8; 32],
    cfg_identity: [u8; 32],
    source_identity: [u8; 32],
    target_identity: [u8; 32],
}

impl DeadBranchContextV1 {
    pub fn new(
        function_identity: [u8; 32],
        cfg_identity: [u8; 32],
        source_identity: [u8; 32],
        target_identity: [u8; 32],
    ) -> Result<Self, MonomorphizationDeadEvidenceErrorV1> {
        for (field, identity) in [
            ("function identity", function_identity),
            ("CFG identity", cfg_identity),
            ("source identity", source_identity),
            ("target identity", target_identity),
        ] {
            if identity == [0; 32] {
                return Err(MonomorphizationDeadEvidenceErrorV1::ZeroIdentity { field });
            }
        }
        Ok(Self {
            function_identity,
            cfg_identity,
            source_identity,
            target_identity,
        })
    }

    pub const fn function_identity(self) -> [u8; 32] {
        self.function_identity
    }

    pub const fn cfg_identity(self) -> [u8; 32] {
        self.cfg_identity
    }

    pub const fn source_identity(self) -> [u8; 32] {
        self.source_identity
    }

    pub const fn target_identity(self) -> [u8; 32] {
        self.target_identity
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct MonomorphizationDeadEvidenceIdentityV1([u8; 32]);

impl MonomorphizationDeadEvidenceIdentityV1 {
    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Canonical but inert description of policy-proven dead successors.
///
/// Anyone can construct this record. It grants no authority to skip collection,
/// panic rejection, address-space analysis, verification, or lowering.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MonomorphizationDeadEvidenceV1 {
    policy_version: u16,
    context: DeadBranchContextV1,
    decisions: Vec<DeadBranchDecisionV1>,
    canonical_bytes: Vec<u8>,
    identity: MonomorphizationDeadEvidenceIdentityV1,
}

impl MonomorphizationDeadEvidenceV1 {
    pub fn new(
        policy_version: u16,
        context: DeadBranchContextV1,
        mut decisions: Vec<DeadBranchDecisionV1>,
    ) -> Result<Self, MonomorphizationDeadEvidenceErrorV1> {
        require_policy_v1(policy_version)?;
        if decisions.len() > MAX_DEAD_BRANCH_DECISIONS_V1 {
            return Err(MonomorphizationDeadEvidenceErrorV1::TooMany {
                field: "dead branch decisions",
                maximum: MAX_DEAD_BRANCH_DECISIONS_V1,
            });
        }
        decisions.sort_unstable_by_key(DeadBranchDecisionV1::branch_block);
        if decisions
            .windows(2)
            .any(|pair| pair[0].branch_block == pair[1].branch_block)
        {
            return Err(MonomorphizationDeadEvidenceErrorV1::DuplicateBranchDecision);
        }
        let canonical_bytes = encode_evidence(policy_version, context, &decisions);
        let identity =
            MonomorphizationDeadEvidenceIdentityV1(Sha256::digest(&canonical_bytes).into());
        Ok(Self {
            policy_version,
            context,
            decisions,
            canonical_bytes,
            identity,
        })
    }

    pub const fn policy_version(&self) -> u16 {
        self.policy_version
    }

    pub const fn context(&self) -> DeadBranchContextV1 {
        self.context
    }

    pub fn decisions(&self) -> &[DeadBranchDecisionV1] {
        &self.decisions
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub const fn identity(&self) -> MonomorphizationDeadEvidenceIdentityV1 {
        self.identity
    }

    pub const fn grants_compiler_authority(&self) -> bool {
        false
    }

    pub const fn grants_panic_exclusion_authority(&self) -> bool {
        false
    }

    pub const fn grants_address_space_exclusion_authority(&self) -> bool {
        false
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MonomorphizationDeadEvidenceErrorV1 {
    Fold(ConstantFoldFailureV1),
    TooMany { field: &'static str, maximum: usize },
    DuplicateSwitchCase,
    NoDeadSuccessor { branch_block: u32 },
    SelectedSuccessorIsDead { branch_block: u32 },
    ZeroIdentity { field: &'static str },
    DuplicateBranchDecision,
}

impl fmt::Display for MonomorphizationDeadEvidenceErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fold(error) => error.fmt(formatter),
            Self::TooMany { field, maximum } => write!(formatter, "{field} exceeds {maximum}"),
            Self::DuplicateSwitchCase => {
                formatter.write_str("constant switch contains a duplicate case")
            }
            Self::NoDeadSuccessor { branch_block } => {
                write!(
                    formatter,
                    "constant switch bb{branch_block} has no dead successor"
                )
            }
            Self::SelectedSuccessorIsDead { branch_block } => write!(
                formatter,
                "constant switch bb{branch_block} marks its selected successor dead"
            ),
            Self::ZeroIdentity { field } => write!(formatter, "{field} must be measured"),
            Self::DuplicateBranchDecision => {
                formatter.write_str("dead branch decisions contain a duplicate block")
            }
        }
    }
}

impl std::error::Error for MonomorphizationDeadEvidenceErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Fold(error) => Some(error),
            _ => None,
        }
    }
}

impl From<ConstantFoldFailureV1> for MonomorphizationDeadEvidenceErrorV1 {
    fn from(value: ConstantFoldFailureV1) -> Self {
        Self::Fold(value)
    }
}

const fn supported_width(width: u16) -> bool {
    matches!(width, 1 | 8 | 16 | 32 | 64 | 128)
}

fn require_policy_v1(version: u16) -> Result<(), ConstantFoldFailureV1> {
    if version == CONSTANT_FOLD_POLICY_VERSION_V1 {
        Ok(())
    } else {
        Err(ConstantFoldFailureV1::UnsupportedPolicyVersion(version))
    }
}

const fn signed_bounds(width: u16) -> (i128, i128) {
    if width == 128 {
        (i128::MIN, i128::MAX)
    } else {
        let high = 1_i128 << (width - 1);
        (-high, high - 1)
    }
}

fn encode_evidence(
    policy_version: u16,
    context: DeadBranchContextV1,
    decisions: &[DeadBranchDecisionV1],
) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&EVIDENCE_DOMAIN_V1);
    bytes.extend_from_slice(&EVIDENCE_FORMAT_VERSION_V1.to_le_bytes());
    bytes.extend_from_slice(&policy_version.to_le_bytes());
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    bytes.extend_from_slice(&context.function_identity);
    bytes.extend_from_slice(&context.cfg_identity);
    bytes.extend_from_slice(&context.source_identity);
    bytes.extend_from_slice(&context.target_identity);
    bytes.extend_from_slice(&(decisions.len() as u32).to_le_bytes());
    for decision in decisions {
        bytes.extend_from_slice(&decision.branch_block.to_le_bytes());
        bytes.extend_from_slice(&decision.selected_successor.to_le_bytes());
        bytes.extend_from_slice(&decision.discriminant.width.to_le_bytes());
        bytes.push(u8::from(decision.discriminant.signed));
        bytes.push(0);
        bytes.extend_from_slice(&decision.discriminant.bits.to_le_bytes());
        bytes.extend_from_slice(&(decision.dead_successors.len() as u16).to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        for successor in &decision.dead_successors {
            bytes.extend_from_slice(&successor.to_le_bytes());
        }
    }
    bytes
}
