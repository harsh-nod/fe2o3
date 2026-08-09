use std::collections::HashSet;
use std::fmt::{self, Write};

use serde::{Deserialize, Serialize};

/// A rustc-reported size and ABI alignment.
///
/// `size` is `None` only for dynamically sized Rust types. Sizes and offsets
/// are bytes; the model deliberately does not infer target layout.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MirLayout {
    pub size: Option<u64>,
    pub align: u64,
}

impl MirLayout {
    pub const fn sized(size: u64, align: u64) -> Self {
        Self {
            size: Some(size),
            align,
        }
    }

    pub const fn dynamically_sized(align: u64) -> Self {
        Self { size: None, align }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum MirScalarType {
    Bool,
    Char,
    Int { signed: bool, bits: u16 },
    Float { bits: u16 },
}

impl MirScalarType {
    fn byte_width(self) -> Option<u64> {
        match self {
            Self::Bool => Some(1),
            Self::Char => Some(4),
            Self::Int { bits, .. } if matches!(bits, 8 | 16 | 32 | 64 | 128) => {
                Some(u64::from(bits / 8))
            }
            Self::Float { bits } if matches!(bits, 16 | 32 | 64 | 128) => Some(u64::from(bits / 8)),
            Self::Int { .. } | Self::Float { .. } => None,
        }
    }

    fn write_canonical(self, output: &mut String) {
        match self {
            Self::Bool => output.push_str("bool"),
            Self::Char => output.push_str("char"),
            Self::Int { signed, bits } => {
                output.push(if signed { 'i' } else { 'u' });
                write!(output, "{bits}").expect("writing to a String cannot fail");
            }
            Self::Float { bits } => {
                write!(output, "f{bits}").expect("writing to a String cannot fail");
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum MirMutability {
    Immutable,
    Mutable,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Ord, PartialOrd, Serialize)]
pub struct MirAddressSpace(pub u32);

impl MirAddressSpace {
    pub const DEFAULT: Self = Self(0);
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MirSemanticType {
    pub layout: MirLayout,
    pub kind: MirTypeKind,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum MirTypeKind {
    Unit,
    Scalar(MirScalarType),
    RawPointer {
        pointee: Box<MirSemanticType>,
        mutability: MirMutability,
        address_space: MirAddressSpace,
    },
    Reference {
        referent: Box<MirSemanticType>,
        mutability: MirMutability,
        address_space: MirAddressSpace,
    },
    Slice {
        element: Box<MirSemanticType>,
    },
    Tuple(MirAggregateLayout),
    Array {
        element: Box<MirSemanticType>,
        length: u64,
    },
    Struct(MirStructType),
    Enum(MirEnumType),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MirField {
    /// Rust source name. Tuple fields use `None` and retain declaration order.
    pub name: Option<String>,
    pub offset: u64,
    pub ty: MirSemanticType,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MirPadding {
    pub offset: u64,
    pub size: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MirAggregateLayout {
    /// Fields are in Rust declaration order, not physical offset order.
    pub fields: Vec<MirField>,
    /// Padding is in ascending physical offset order.
    pub padding: Vec<MirPadding>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MirStructType {
    /// A monomorphized, crate-qualified rustc type identity.
    pub identity: String,
    pub aggregate: MirAggregateLayout,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MirEnumType {
    /// A monomorphized, crate-qualified rustc type identity.
    pub identity: String,
    /// The logical Rust discriminant representation.
    pub discriminant: MirScalarType,
    pub encoding: MirEnumEncoding,
    /// Variants are in ascending rustc variant-index order.
    pub variants: Vec<MirVariant>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum MirEnumEncoding {
    Uninhabited,
    Single {
        variant: u32,
    },
    Direct {
        tag_offset: u64,
        tag: MirScalarType,
    },
    Niche {
        niche_offset: u64,
        niche_bits: u16,
        untagged_variant: u32,
        niche_variants_start: u32,
        niche_variants_end: u32,
        niche_start: u128,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MirVariant {
    pub index: u32,
    pub name: String,
    /// Raw two's-complement bits in the enum's logical discriminant type.
    pub discriminant: u128,
    pub aggregate: MirAggregateLayout,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirTypeValidationError {
    path: String,
    reason: String,
}

impl MirTypeValidationError {
    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn reason(&self) -> &str {
        &self.reason
    }

    fn new(path: &str, reason: impl Into<String>) -> Self {
        Self {
            path: path.to_owned(),
            reason: reason.into(),
        }
    }
}

impl fmt::Display for MirTypeValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.path, self.reason)
    }
}

impl std::error::Error for MirTypeValidationError {}

impl MirSemanticType {
    pub fn validate(&self) -> Result<(), MirTypeValidationError> {
        self.validate_at("type")
    }

    /// Returns a versioned, unambiguous representation after validating all
    /// rustc layout evidence. Names are byte-length-prefixed.
    pub fn canonical_text(&self) -> Result<String, MirTypeValidationError> {
        self.validate()?;
        let mut output = String::from("mir.type.v1");
        self.write_canonical(&mut output);
        Ok(output)
    }

    /// Returns whether the validated Rust type has at least one valid value.
    pub fn is_inhabited(&self) -> Result<bool, MirTypeValidationError> {
        self.validate()?;
        Ok(self.is_inhabited_unchecked())
    }

    /// Returns whether `ZeroSized` denotes the type's sole valid value.
    /// Enums are deliberately excluded even when their current layout is zero
    /// sized because their validity depends on variant state.
    pub fn has_single_zero_sized_value(&self) -> Result<bool, MirTypeValidationError> {
        self.validate()?;
        Ok(self.has_single_zero_sized_value_unchecked())
    }

    fn is_inhabited_unchecked(&self) -> bool {
        match &self.kind {
            MirTypeKind::Unit | MirTypeKind::Scalar(_) | MirTypeKind::RawPointer { .. } => true,
            MirTypeKind::Reference { referent, .. } => referent.is_inhabited_unchecked(),
            MirTypeKind::Slice { .. } => true,
            MirTypeKind::Tuple(aggregate) => aggregate
                .fields
                .iter()
                .all(|field| field.ty.is_inhabited_unchecked()),
            MirTypeKind::Array { element, length } => {
                *length == 0 || element.is_inhabited_unchecked()
            }
            MirTypeKind::Struct(structure) => structure
                .aggregate
                .fields
                .iter()
                .all(|field| field.ty.is_inhabited_unchecked()),
            MirTypeKind::Enum(enum_ty) => enum_ty.variants.iter().any(|variant| {
                variant
                    .aggregate
                    .fields
                    .iter()
                    .all(|field| field.ty.is_inhabited_unchecked())
            }),
        }
    }

    fn has_single_zero_sized_value_unchecked(&self) -> bool {
        if self.layout.size != Some(0) || !self.is_inhabited_unchecked() {
            return false;
        }
        match &self.kind {
            MirTypeKind::Unit => true,
            MirTypeKind::Tuple(aggregate) => aggregate
                .fields
                .iter()
                .all(|field| field.ty.has_single_zero_sized_value_unchecked()),
            MirTypeKind::Array { element, length } => {
                *length == 0 || element.has_single_zero_sized_value_unchecked()
            }
            MirTypeKind::Struct(structure) => structure
                .aggregate
                .fields
                .iter()
                .all(|field| field.ty.has_single_zero_sized_value_unchecked()),
            MirTypeKind::Scalar(_)
            | MirTypeKind::RawPointer { .. }
            | MirTypeKind::Reference { .. }
            | MirTypeKind::Slice { .. }
            | MirTypeKind::Enum(_) => false,
        }
    }

    fn validate_at(&self, path: &str) -> Result<(), MirTypeValidationError> {
        validate_layout(self.layout, path)?;
        match &self.kind {
            MirTypeKind::Unit => {
                if self.layout != MirLayout::sized(0, 1) {
                    return Err(MirTypeValidationError::new(
                        path,
                        "unit must have size 0 and alignment 1",
                    ));
                }
            }
            MirTypeKind::Scalar(scalar) => {
                let expected = scalar.byte_width().ok_or_else(|| {
                    MirTypeValidationError::new(path, "scalar width is not supported by Rust")
                })?;
                if self.layout.size != Some(expected) {
                    return Err(MirTypeValidationError::new(
                        path,
                        format!("scalar requires size {expected}"),
                    ));
                }
            }
            MirTypeKind::RawPointer { pointee, .. } => {
                validate_pointer_layout(self.layout, path)?;
                pointee.validate_at(&format!("{path}.pointee"))?;
            }
            MirTypeKind::Reference { referent, .. } => {
                validate_pointer_layout(self.layout, path)?;
                referent.validate_at(&format!("{path}.referent"))?;
            }
            MirTypeKind::Slice { element } => {
                element.validate_at(&format!("{path}.element"))?;
                if self.layout.size.is_some() {
                    return Err(MirTypeValidationError::new(path, "slice must be unsized"));
                }
                if element.layout.size.is_none() {
                    return Err(MirTypeValidationError::new(
                        format!("{path}.element").as_str(),
                        "slice element must be sized",
                    ));
                }
                if self.layout.align != element.layout.align {
                    return Err(MirTypeValidationError::new(
                        path,
                        "slice alignment must equal its element alignment",
                    ));
                }
            }
            MirTypeKind::Tuple(aggregate) => {
                validate_aggregate(
                    self.layout,
                    aggregate,
                    AggregateKind::Tuple,
                    path,
                    &[],
                    false,
                )?;
            }
            MirTypeKind::Array { element, length } => {
                element.validate_at(&format!("{path}.element"))?;
                let element_size = element.layout.size.ok_or_else(|| {
                    MirTypeValidationError::new(
                        format!("{path}.element").as_str(),
                        "array element must be sized",
                    )
                })?;
                let expected = element_size.checked_mul(*length).ok_or_else(|| {
                    MirTypeValidationError::new(path, "array byte size overflows u64")
                })?;
                if self.layout != MirLayout::sized(expected, element.layout.align) {
                    return Err(MirTypeValidationError::new(
                        path,
                        format!(
                            "array layout must have size {expected} and alignment {}",
                            element.layout.align
                        ),
                    ));
                }
            }
            MirTypeKind::Struct(struct_ty) => {
                validate_identity(&struct_ty.identity, &format!("{path}.struct.identity"))?;
                validate_aggregate(
                    self.layout,
                    &struct_ty.aggregate,
                    AggregateKind::Struct,
                    path,
                    &[],
                    false,
                )?;
            }
            MirTypeKind::Enum(enum_ty) => self.validate_enum(enum_ty, path)?,
        }
        Ok(())
    }

    fn validate_enum(
        &self,
        enum_ty: &MirEnumType,
        path: &str,
    ) -> Result<(), MirTypeValidationError> {
        let size = self.layout.size.ok_or_else(|| {
            MirTypeValidationError::new(path, "enum must have a statically known size")
        })?;
        validate_identity(&enum_ty.identity, &format!("{path}.enum.identity"))?;
        let discriminant_bits = match enum_ty.discriminant {
            MirScalarType::Int { bits, .. } if enum_ty.discriminant.byte_width().is_some() => bits,
            _ => {
                return Err(MirTypeValidationError::new(
                    format!("{path}.enum.discriminant").as_str(),
                    "enum discriminant must be a valid integer scalar",
                ));
            }
        };
        if enum_ty.variants.is_empty() && !matches!(enum_ty.encoding, MirEnumEncoding::Uninhabited)
        {
            return Err(MirTypeValidationError::new(
                format!("{path}.enum.variants").as_str(),
                "enum must contain at least one variant",
            ));
        }

        let mut names = HashSet::new();
        let mut discriminants = HashSet::new();
        for (expected_index, variant) in enum_ty.variants.iter().enumerate() {
            let variant_path = format!("{path}.enum.variant[{expected_index}]");
            if variant.index as usize != expected_index {
                return Err(MirTypeValidationError::new(
                    &variant_path,
                    "variant indices must be contiguous and ascending from zero",
                ));
            }
            validate_identity(&variant.name, &format!("{variant_path}.name"))?;
            if !names.insert(variant.name.as_str()) {
                return Err(MirTypeValidationError::new(
                    &variant_path,
                    "variant names must be unique",
                ));
            }
            if !discriminants.insert(variant.discriminant) {
                return Err(MirTypeValidationError::new(
                    &variant_path,
                    "variant discriminants must be unique",
                ));
            }
            if discriminant_bits < 128 && variant.discriminant >= (1_u128 << discriminant_bits) {
                return Err(MirTypeValidationError::new(
                    &variant_path,
                    "variant discriminant does not fit its scalar representation",
                ));
            }
        }

        let (occupied, reserved_may_overlap) = match enum_ty.encoding {
            MirEnumEncoding::Uninhabited => {
                if !enum_ty.variants.is_empty() {
                    return Err(MirTypeValidationError::new(
                        format!("{path}.enum.encoding").as_str(),
                        "uninhabited encoding requires zero variants",
                    ));
                }
                if self.layout != MirLayout::sized(0, 1) {
                    return Err(MirTypeValidationError::new(
                        format!("{path}.enum.encoding").as_str(),
                        "uninhabited enum must have size 0 and alignment 1",
                    ));
                }
                (Vec::new(), false)
            }
            MirEnumEncoding::Single { variant } => {
                if enum_ty.variants.len() != 1 || variant != 0 {
                    return Err(MirTypeValidationError::new(
                        format!("{path}.enum.encoding").as_str(),
                        "single encoding requires exactly variant 0",
                    ));
                }
                (Vec::new(), false)
            }
            MirEnumEncoding::Direct { tag_offset, tag } => {
                let tag_size = match tag {
                    MirScalarType::Int { .. } => tag.byte_width(),
                    _ => None,
                }
                .ok_or_else(|| {
                    MirTypeValidationError::new(
                        format!("{path}.enum.encoding").as_str(),
                        "direct tag must be a valid integer scalar",
                    )
                })?;
                if enum_ty.variants.iter().any(|variant| {
                    !discriminant_fits_tag(variant.discriminant, enum_ty.discriminant, tag)
                }) {
                    return Err(MirTypeValidationError::new(
                        format!("{path}.enum.encoding").as_str(),
                        "a logical discriminant does not fit the physical direct tag",
                    ));
                }
                let tag = checked_range(tag_offset, tag_size, size, path, "direct tag")?;
                (vec![tag], false)
            }
            MirEnumEncoding::Niche {
                niche_offset,
                niche_bits,
                untagged_variant,
                niche_variants_start,
                niche_variants_end,
                niche_start,
            } => {
                if niche_bits == 0 || niche_bits > 128 || !niche_bits.is_multiple_of(8) {
                    return Err(MirTypeValidationError::new(
                        format!("{path}.enum.encoding").as_str(),
                        "niche width must be 8..=128 whole bits",
                    ));
                }
                if niche_variants_start > niche_variants_end
                    || niche_variants_end as usize >= enum_ty.variants.len()
                    || untagged_variant as usize >= enum_ty.variants.len()
                    || (niche_variants_start..=niche_variants_end).contains(&untagged_variant)
                {
                    return Err(MirTypeValidationError::new(
                        format!("{path}.enum.encoding").as_str(),
                        "niche variant range and untagged variant must be valid and disjoint",
                    ));
                }
                if niche_bits < 128 && niche_start >= (1_u128 << niche_bits) {
                    return Err(MirTypeValidationError::new(
                        format!("{path}.enum.encoding").as_str(),
                        "niche start does not fit the niche width",
                    ));
                }
                (
                    vec![checked_range(
                        niche_offset,
                        u64::from(niche_bits / 8),
                        size,
                        path,
                        "niche",
                    )?],
                    true,
                )
            }
        };

        for variant in &enum_ty.variants {
            validate_aggregate(
                self.layout,
                &variant.aggregate,
                AggregateKind::Variant,
                &format!("{path}.enum.variant[{}]", variant.index),
                &occupied,
                reserved_may_overlap,
            )?;
        }
        Ok(())
    }

    fn write_canonical(&self, output: &mut String) {
        output.push('{');
        write_layout(output, self.layout);
        output.push_str(";kind=");
        match &self.kind {
            MirTypeKind::Unit => output.push_str("unit"),
            MirTypeKind::Scalar(scalar) => {
                output.push_str("scalar(");
                scalar.write_canonical(output);
                output.push(')');
            }
            MirTypeKind::RawPointer {
                pointee,
                mutability,
                address_space,
            } => write_indirect(output, "raw", pointee, *mutability, *address_space),
            MirTypeKind::Reference {
                referent,
                mutability,
                address_space,
            } => write_indirect(output, "ref", referent, *mutability, *address_space),
            MirTypeKind::Slice { element } => {
                output.push_str("slice(");
                element.write_canonical(output);
                output.push(')');
            }
            MirTypeKind::Tuple(aggregate) => {
                output.push_str("tuple");
                write_aggregate(output, aggregate);
            }
            MirTypeKind::Array { element, length } => {
                write!(output, "array(len={length};element=")
                    .expect("writing to a String cannot fail");
                element.write_canonical(output);
                output.push(')');
            }
            MirTypeKind::Struct(struct_ty) => {
                output.push_str("struct(name=");
                write_name(output, &struct_ty.identity);
                output.push(';');
                write_aggregate(output, &struct_ty.aggregate);
                output.push(')');
            }
            MirTypeKind::Enum(enum_ty) => write_enum(output, enum_ty),
        }
        output.push('}');
    }
}

#[derive(Clone, Copy)]
enum AggregateKind {
    Tuple,
    Struct,
    Variant,
}

type ByteRange = (u64, u64);

fn validate_layout(layout: MirLayout, path: &str) -> Result<(), MirTypeValidationError> {
    if layout.align == 0 || !layout.align.is_power_of_two() {
        return Err(MirTypeValidationError::new(
            path,
            "alignment must be a nonzero power of two",
        ));
    }
    if let Some(size) = layout.size
        && !size.is_multiple_of(layout.align)
    {
        return Err(MirTypeValidationError::new(
            path,
            "sized layout must be rounded up to its alignment",
        ));
    }
    Ok(())
}

fn validate_pointer_layout(layout: MirLayout, path: &str) -> Result<(), MirTypeValidationError> {
    if layout.size == Some(0) || layout.size.is_none() {
        return Err(MirTypeValidationError::new(
            path,
            "pointer or reference layout must have a nonzero size",
        ));
    }
    Ok(())
}

fn validate_identity(identity: &str, path: &str) -> Result<(), MirTypeValidationError> {
    if identity.is_empty() {
        Err(MirTypeValidationError::new(
            path,
            "identity must not be empty",
        ))
    } else {
        Ok(())
    }
}

fn validate_aggregate(
    layout: MirLayout,
    aggregate: &MirAggregateLayout,
    kind: AggregateKind,
    path: &str,
    reserved: &[ByteRange],
    reserved_may_overlap: bool,
) -> Result<(), MirTypeValidationError> {
    let mut names = HashSet::new();
    let mut field_ranges = Vec::new();
    let mut unsized_field = None;

    for (index, field) in aggregate.fields.iter().enumerate() {
        let field_path = format!("{path}.field[{index}]");
        match kind {
            AggregateKind::Tuple if field.name.is_some() => {
                return Err(MirTypeValidationError::new(
                    &field_path,
                    "tuple fields must not have names",
                ));
            }
            AggregateKind::Struct | AggregateKind::Variant => {
                let name = field.name.as_deref().ok_or_else(|| {
                    MirTypeValidationError::new(&field_path, "named aggregate field needs a name")
                })?;
                validate_identity(name, &format!("{field_path}.name"))?;
                if !names.insert(name) {
                    return Err(MirTypeValidationError::new(
                        &field_path,
                        "field names must be unique",
                    ));
                }
            }
            AggregateKind::Tuple => {}
        }

        field.ty.validate_at(&format!("{field_path}.type"))?;
        match field.ty.layout.size {
            Some(0) => {
                if let Some(size) = layout.size
                    && field.offset > size
                {
                    return Err(MirTypeValidationError::new(
                        &field_path,
                        "zero-sized field offset exceeds aggregate size",
                    ));
                }
            }
            Some(field_size) => {
                let end = field.offset.checked_add(field_size).ok_or_else(|| {
                    MirTypeValidationError::new(&field_path, "field byte range overflows u64")
                })?;
                field_ranges.push((field.offset, end));
            }
            None => {
                if unsized_field.replace((index, field.offset)).is_some()
                    || index + 1 != aggregate.fields.len()
                    || layout.size.is_some()
                {
                    return Err(MirTypeValidationError::new(
                        &field_path,
                        "only the final field of an unsized aggregate may be unsized",
                    ));
                }
            }
        }
    }

    if layout.size.is_none() && unsized_field.is_none() {
        return Err(MirTypeValidationError::new(
            path,
            "unsized aggregate must end in an unsized field",
        ));
    }

    let covered_size = layout
        .size
        .or_else(|| unsized_field.map(|(_, offset)| offset));
    let covered_size = covered_size.ok_or_else(|| {
        MirTypeValidationError::new(path, "cannot determine the sized prefix of aggregate")
    })?;
    if field_ranges.iter().any(|&(_, end)| end > covered_size) {
        return Err(MirTypeValidationError::new(
            path,
            "field extends beyond the aggregate's sized storage",
        ));
    }

    field_ranges.sort_unstable();
    for pair in field_ranges.windows(2) {
        if pair[0].1 > pair[1].0 {
            return Err(MirTypeValidationError::new(
                path,
                "non-zero-sized aggregate fields overlap",
            ));
        }
    }
    if !reserved_may_overlap
        && reserved.iter().any(|reserved_range| {
            field_ranges
                .iter()
                .any(|field_range| ranges_overlap(*reserved_range, *field_range))
        })
    {
        return Err(MirTypeValidationError::new(
            path,
            "aggregate field overlaps reserved enum tag storage",
        ));
    }
    let mut occupied = field_ranges;
    occupied.extend_from_slice(reserved);
    occupied.sort_unstable();
    let occupied = merge_ranges(occupied);
    validate_padding(covered_size, &occupied, &aggregate.padding, path)
}

fn checked_range(
    offset: u64,
    size: u64,
    container_size: u64,
    path: &str,
    description: &str,
) -> Result<ByteRange, MirTypeValidationError> {
    let end = offset.checked_add(size).ok_or_else(|| {
        MirTypeValidationError::new(path, format!("{description} byte range overflows u64"))
    })?;
    if end > container_size {
        return Err(MirTypeValidationError::new(
            path,
            format!("{description} extends beyond its containing layout"),
        ));
    }
    Ok((offset, end))
}

fn merge_ranges(ranges: Vec<ByteRange>) -> Vec<ByteRange> {
    let mut merged: Vec<ByteRange> = Vec::with_capacity(ranges.len());
    for range in ranges {
        if let Some(last) = merged.last_mut()
            && last.1 >= range.0
        {
            last.1 = last.1.max(range.1);
            continue;
        }
        merged.push(range);
    }
    merged
}

fn ranges_overlap(left: ByteRange, right: ByteRange) -> bool {
    left.0 < right.1 && right.0 < left.1
}

fn discriminant_fits_tag(raw: u128, logical: MirScalarType, physical: MirScalarType) -> bool {
    let MirScalarType::Int {
        signed: logical_signed,
        bits: logical_bits,
    } = logical
    else {
        return false;
    };
    let MirScalarType::Int {
        signed: physical_signed,
        bits: physical_bits,
    } = physical
    else {
        return false;
    };

    if logical_signed {
        let value = sign_extend(raw, logical_bits);
        if physical_signed {
            let shift = u32::from(128 - physical_bits);
            value >= (i128::MIN >> shift) && value <= (i128::MAX >> shift)
        } else {
            value >= 0 && (value as u128) <= unsigned_max(physical_bits)
        }
    } else if physical_signed {
        raw <= (i128::MAX >> u32::from(128 - physical_bits)) as u128
    } else {
        raw <= unsigned_max(physical_bits)
    }
}

fn sign_extend(raw: u128, bits: u16) -> i128 {
    let shift = u32::from(128 - bits);
    ((raw << shift) as i128) >> shift
}

fn unsigned_max(bits: u16) -> u128 {
    if bits == 128 {
        u128::MAX
    } else {
        (1_u128 << bits) - 1
    }
}

fn validate_padding(
    size: u64,
    occupied: &[ByteRange],
    padding: &[MirPadding],
    path: &str,
) -> Result<(), MirTypeValidationError> {
    let mut expected = Vec::new();
    let mut cursor = 0;
    for &(start, end) in occupied {
        if cursor < start {
            expected.push(MirPadding {
                offset: cursor,
                size: start - cursor,
            });
        }
        cursor = end;
    }
    if cursor < size {
        expected.push(MirPadding {
            offset: cursor,
            size: size - cursor,
        });
    }
    if padding != expected {
        return Err(MirTypeValidationError::new(
            format!("{path}.padding").as_str(),
            format!("padding must exactly describe physical gaps; expected {expected:?}"),
        ));
    }
    Ok(())
}

fn write_layout(output: &mut String, layout: MirLayout) {
    output.push_str("layout(size=");
    match layout.size {
        Some(size) => write!(output, "{size}").expect("writing to a String cannot fail"),
        None => output.push('?'),
    }
    write!(output, ";align={})", layout.align).expect("writing to a String cannot fail");
}

fn write_name(output: &mut String, name: &str) {
    write!(output, "{}:{name}", name.len()).expect("writing to a String cannot fail");
}

fn write_indirect(
    output: &mut String,
    kind: &str,
    pointee: &MirSemanticType,
    mutability: MirMutability,
    address_space: MirAddressSpace,
) {
    let mutability = match mutability {
        MirMutability::Immutable => "const",
        MirMutability::Mutable => "mut",
    };
    write!(
        output,
        "{kind}(mut={mutability};addrspace={};type=",
        address_space.0
    )
    .expect("writing to a String cannot fail");
    pointee.write_canonical(output);
    output.push(')');
}

fn write_aggregate(output: &mut String, aggregate: &MirAggregateLayout) {
    output.push_str("aggregate(fields=[");
    for (index, field) in aggregate.fields.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        output.push_str("field(name=");
        match &field.name {
            Some(name) => write_name(output, name),
            None => output.push('-'),
        }
        write!(output, ";offset={};type=", field.offset).expect("writing to a String cannot fail");
        field.ty.write_canonical(output);
        output.push(')');
    }
    output.push_str("];padding=[");
    for (index, padding) in aggregate.padding.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        write!(output, "{}+{}", padding.offset, padding.size)
            .expect("writing to a String cannot fail");
    }
    output.push_str("])");
}

fn write_enum(output: &mut String, enum_ty: &MirEnumType) {
    output.push_str("enum(name=");
    write_name(output, &enum_ty.identity);
    output.push_str(";discriminant=");
    enum_ty.discriminant.write_canonical(output);
    output.push_str(";encoding=");
    match enum_ty.encoding {
        MirEnumEncoding::Uninhabited => output.push_str("uninhabited"),
        MirEnumEncoding::Single { variant } => {
            write!(output, "single({variant})").expect("writing to a String cannot fail");
        }
        MirEnumEncoding::Direct { tag_offset, tag } => {
            write!(output, "direct(offset={tag_offset};tag=")
                .expect("writing to a String cannot fail");
            tag.write_canonical(output);
            output.push(')');
        }
        MirEnumEncoding::Niche {
            niche_offset,
            niche_bits,
            untagged_variant,
            niche_variants_start,
            niche_variants_end,
            niche_start,
        } => {
            write!(
                output,
                "niche(offset={niche_offset};bits={niche_bits};untagged={untagged_variant};variants={niche_variants_start}..={niche_variants_end};start={niche_start:032x})"
            )
            .expect("writing to a String cannot fail");
        }
    }
    output.push_str(";variants=[");
    for (index, variant) in enum_ty.variants.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        write!(output, "variant(index={};name=", variant.index)
            .expect("writing to a String cannot fail");
        write_name(output, &variant.name);
        write!(output, ";discriminant={:032x};", variant.discriminant)
            .expect("writing to a String cannot fail");
        write_aggregate(output, &variant.aggregate);
        output.push(')');
    }
    output.push_str("])");
}
