use super::*;

/// The key encoding is part of switch semantics, not an inferred cast.
#[pliron_attr(name = "gpu.switch_key_kind_v3", format, verifier = "succ")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SwitchKeyKindAttrV3 {
    /// Original untyped u64 bit-pattern keys, in original order.
    LegacyU64,
    /// Exactly typed signed eight-bit keys.
    I8,
    /// Exactly typed signed sixteen-bit keys.
    I16,
    /// Exactly typed signed thirty-two-bit keys.
    I32,
    /// Exactly typed signed sixty-four-bit keys.
    I64,
    /// Exactly typed unsigned eight-bit keys.
    U8,
    /// Exactly typed unsigned sixteen-bit keys.
    U16,
    /// Exactly typed unsigned thirty-two-bit keys.
    U32,
    /// Exactly typed unsigned sixty-four-bit keys.
    U64,
    /// Exactly typed physical Index keys, not analytical kernel.index.
    Index,
    /// A typed switch with no cases; its integer selector supplies the type.
    EmptyTyped,
}

/// Exact width-masked case bits. Signed ordering is determined by key kind.
#[pliron_attr(
    name = "gpu.switch_case_bits_v3",
    format = "`[` vec($0, CharSpace(`,`)) `]`"
)]
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct SwitchCaseBitsAttrV3(pub(super) Vec<u64>);

impl SwitchCaseBitsAttrV3 {
    /// Borrows the actual stored keys without allocating.
    pub fn bits(&self) -> &[u64] {
        &self.0
    }
    /// Returns retained vector capacity for caller-owned resource accounting.
    pub fn capacity(&self) -> usize {
        self.0.capacity()
    }
}

impl Verify for SwitchCaseBitsAttrV3 {
    fn verify(&self, _ctx: &Context) -> Result<()> {
        if self.0.len() <= MAX_SWITCH_CASES_V3 {
            Ok(())
        } else {
            verify_err!(
                Location::Unknown,
                "switch case count exceeds its wire ceiling"
            )
        }
    }
}

/// Prefix offsets into the payload segment, including zero and final length.
#[pliron_attr(
    name = "gpu.switch_successor_offsets_v3",
    format = "`[` vec($0, CharSpace(`,`)) `]`"
)]
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct SwitchSuccessorOffsetsAttrV3(pub(super) Vec<u32>);

impl SwitchSuccessorOffsetsAttrV3 {
    /// Borrows checked-on-verification payload offsets.
    pub fn offsets(&self) -> &[u32] {
        &self.0
    }
    /// Returns retained vector capacity for caller-owned resource accounting.
    pub fn capacity(&self) -> usize {
        self.0.capacity()
    }
}

impl Verify for SwitchSuccessorOffsetsAttrV3 {
    fn verify(&self, _ctx: &Context) -> Result<()> {
        if valid_offsets(&self.0) {
            Ok(())
        } else {
            verify_err!(Location::Unknown, "switch successor offsets are malformed")
        }
    }
}

pub(super) fn valid_offsets(offsets: &[u32]) -> bool {
    (2..=MAX_SWITCH_CASES_V3 + 2).contains(&offsets.len())
        && offsets.first() == Some(&0)
        && offsets.windows(2).all(|pair| {
            pair[1]
                .checked_sub(pair[0])
                .is_some_and(|length| length as usize <= MAX_SWITCH_EDGE_ARGUMENTS_V3)
        })
}

pub(super) fn selector_shape(ctx: &Context, ty: TypeHandle) -> Option<(u32, bool, bool)> {
    let ty = ty.deref(ctx);
    if ty.is::<IndexType>() {
        return Some((64, false, true));
    }
    let integer = ty.downcast_ref::<IntegerType>()?;
    if !matches!(integer.width(), 8 | 16 | 32 | 64 | 128) {
        return None;
    }
    match integer.signedness() {
        Signedness::Signed => Some((integer.width(), true, false)),
        Signedness::Unsigned => Some((integer.width(), false, false)),
        Signedness::Signless => None,
    }
}

pub(super) fn validate_keys(
    ctx: &Context,
    ty: TypeHandle,
    kind: SwitchKeyKindAttrV3,
    keys: &[u64],
) -> std::result::Result<(), SwitchErrorV3> {
    let (width, signed, index) = selector_shape(ctx, ty).ok_or(SwitchErrorV3::SelectorType)?;
    validate_switch_case_keys_v3(width, signed, index, kind, keys)
}
