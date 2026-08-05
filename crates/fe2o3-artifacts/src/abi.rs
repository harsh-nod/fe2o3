use crate::{DigestBytes, Name, PointerWidth, ValidationError};

pub const MAX_ABI_FIELDS: usize = 64;
/// Target-neutral ceiling for a kernel argument buffer.
///
/// Runtime loaders must also enforce the selected device's tighter limit.
pub const MAX_ABI_BYTES: u64 = 1 << 20;
const MAX_ALIGNMENT: u32 = 1 << 20;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScalarType {
    I8,
    U8,
    I16,
    U16,
    I32,
    U32,
    I64,
    U64,
    F16,
    F32,
    F64,
}

impl ScalarType {
    const fn size(self) -> u64 {
        match self {
            Self::I8 | Self::U8 => 1,
            Self::I16 | Self::U16 | Self::F16 => 2,
            Self::I32 | Self::U32 | Self::F32 => 4,
            Self::I64 | Self::U64 | Self::F64 => 8,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AbiKind {
    Scalar(ScalarType),
    Pointer {
        pointee_size: u64,
        pointee_alignment: u32,
    },
    Slice {
        element_size: u64,
        element_alignment: u32,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mutability {
    Immutable,
    Mutable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Access {
    ByValue,
    ReadOnly,
    WriteOnly,
    ReadWrite,
}

impl Access {
    const fn writes(self) -> bool {
        matches!(self, Self::WriteOnly | Self::ReadWrite)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AddressSpace {
    Value,
    Global,
    Constant,
    Workgroup,
    Private,
    Generic,
}

/// Compiler-produced identity for one logical Rust kernel argument.
///
/// `rust_type` identifies the fully monomorphized Rust type. `layout` identifies
/// the rustc-reported host/device ABI layout, including scalarization and
/// padding. The bytes are opaque here; build tooling owns their versioned hash
/// domains.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TypeIdentity {
    rust_type: DigestBytes,
    layout: DigestBytes,
}

impl TypeIdentity {
    pub const fn new(rust_type: DigestBytes, layout: DigestBytes) -> Self {
        Self { rust_type, layout }
    }

    pub const fn rust_type(self) -> DigestBytes {
        self.rust_type
    }

    pub const fn layout(self) -> DigestBytes {
        self.layout
    }
}

/// Source-level ownership carried by a logical kernel argument.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArgumentOwnership {
    ByValue,
    SharedBorrow,
    UniqueBorrow,
    RawPointer,
}

/// Aliasing guarantee the compiler may rely on for an argument.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AliasClass {
    Value,
    SharedReadOnly,
    Exclusive,
    SharedAtomic,
    Unrestricted,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AbiField {
    name: Name,
    offset: u64,
    size: u64,
    alignment: u32,
    kind: AbiKind,
    mutability: Mutability,
    access: Access,
    address_space: AddressSpace,
    type_identity: TypeIdentity,
    ownership: ArgumentOwnership,
    alias_class: AliasClass,
}

impl AbiField {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        name: Name,
        offset: u64,
        size: u64,
        alignment: u32,
        kind: AbiKind,
        mutability: Mutability,
        access: Access,
        address_space: AddressSpace,
        type_identity: TypeIdentity,
        ownership: ArgumentOwnership,
        alias_class: AliasClass,
    ) -> Result<Self, ValidationError> {
        validate_alignment(alignment, "ABI field")?;
        if size == 0 || !offset.is_multiple_of(u64::from(alignment)) {
            return Err(ValidationError::InvalidLayout(
                "field size must be nonzero and offset must be aligned",
            ));
        }
        offset
            .checked_add(size)
            .ok_or(ValidationError::Overflow("ABI field end"))?;

        match kind {
            AbiKind::Scalar(scalar) => {
                if size != scalar.size() || u64::from(alignment) > size {
                    return Err(ValidationError::InvalidLayout(
                        "scalar size or alignment does not match its type",
                    ));
                }
                if mutability != Mutability::Immutable
                    || access != Access::ByValue
                    || address_space != AddressSpace::Value
                    || ownership != ArgumentOwnership::ByValue
                    || alias_class != AliasClass::Value
                {
                    return Err(ValidationError::InvalidAccess(
                        "scalars must be immutable by-value, non-aliased fields in value space",
                    ));
                }
            }
            AbiKind::Pointer {
                pointee_size,
                pointee_alignment,
            } => {
                validate_supported_reference_width(size, alignment, false)?;
                validate_referenced_layout(pointee_size, pointee_alignment, "pointee")?;
                validate_reference_access(access, address_space)?;
                validate_reference_ownership(mutability, access, ownership, alias_class)?;
            }
            AbiKind::Slice {
                element_size,
                element_alignment,
            } => {
                validate_supported_reference_width(size, alignment, true)?;
                validate_referenced_layout(element_size, element_alignment, "slice element")?;
                validate_reference_access(access, address_space)?;
                validate_reference_ownership(mutability, access, ownership, alias_class)?;
            }
        }

        Ok(Self {
            name,
            offset,
            size,
            alignment,
            kind,
            mutability,
            access,
            address_space,
            type_identity,
            ownership,
            alias_class,
        })
    }

    pub const fn name(&self) -> &Name {
        &self.name
    }

    pub const fn offset(&self) -> u64 {
        self.offset
    }

    pub const fn size(&self) -> u64 {
        self.size
    }

    pub const fn alignment(&self) -> u32 {
        self.alignment
    }

    pub const fn kind(&self) -> AbiKind {
        self.kind
    }

    pub const fn mutability(&self) -> Mutability {
        self.mutability
    }

    pub const fn access(&self) -> Access {
        self.access
    }

    pub const fn address_space(&self) -> AddressSpace {
        self.address_space
    }

    pub const fn type_identity(&self) -> TypeIdentity {
        self.type_identity
    }

    pub const fn ownership(&self) -> ArgumentOwnership {
        self.ownership
    }

    pub const fn alias_class(&self) -> AliasClass {
        self.alias_class
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AbiLayout {
    size: u64,
    alignment: u32,
    pointer_width: PointerWidth,
    fields: Vec<AbiField>,
}

impl AbiLayout {
    pub fn new(
        size: u64,
        alignment: u32,
        pointer_width: PointerWidth,
        fields: Vec<AbiField>,
    ) -> Result<Self, ValidationError> {
        validate_alignment(alignment, "ABI layout")?;
        if size > MAX_ABI_BYTES {
            return Err(ValidationError::InvalidLayout(
                "ABI size exceeds the target-neutral limit",
            ));
        }
        if fields.len() > MAX_ABI_FIELDS {
            return Err(ValidationError::TooMany {
                field: "ABI fields",
                max: MAX_ABI_FIELDS,
            });
        }
        if fields.is_empty() {
            if size != 0 || alignment != 1 {
                return Err(ValidationError::InvalidLayout(
                    "an empty ABI must have size zero and alignment one",
                ));
            }
        } else if size == 0 || !size.is_multiple_of(u64::from(alignment)) {
            return Err(ValidationError::InvalidLayout(
                "ABI size must be nonzero and a multiple of its alignment",
            ));
        }

        let mut previous_end = 0_u64;
        let mut names = Vec::with_capacity(fields.len());
        for field in &fields {
            if field.offset < previous_end {
                return Err(ValidationError::InvalidLayout(
                    "fields overlap or are not ordered by offset",
                ));
            }
            let end = field
                .offset
                .checked_add(field.size)
                .ok_or(ValidationError::Overflow("ABI field end"))?;
            if end > size || field.alignment > alignment {
                return Err(ValidationError::InvalidLayout(
                    "field lies outside the ABI or is over-aligned",
                ));
            }
            validate_field_width(field, pointer_width)?;
            previous_end = end;
            names.push(field.name.clone());
        }
        names.sort_unstable();
        if names.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(ValidationError::Duplicate { field: "ABI name" });
        }

        Ok(Self {
            size,
            alignment,
            pointer_width,
            fields,
        })
    }

    pub const fn size(&self) -> u64 {
        self.size
    }

    pub const fn alignment(&self) -> u32 {
        self.alignment
    }

    pub const fn pointer_width(&self) -> PointerWidth {
        self.pointer_width
    }

    pub fn fields(&self) -> &[AbiField] {
        &self.fields
    }
}

fn validate_alignment(value: u32, field: &'static str) -> Result<(), ValidationError> {
    if value == 0 || !value.is_power_of_two() || value > MAX_ALIGNMENT {
        return Err(ValidationError::InvalidAlignment { field, value });
    }
    Ok(())
}

fn validate_referenced_layout(
    size: u64,
    alignment: u32,
    field: &'static str,
) -> Result<(), ValidationError> {
    validate_alignment(alignment, field)?;
    if !size.is_multiple_of(u64::from(alignment)) {
        return Err(ValidationError::InvalidLayout(
            "referenced element size must respect its alignment",
        ));
    }
    Ok(())
}

fn validate_supported_reference_width(
    size: u64,
    alignment: u32,
    is_slice: bool,
) -> Result<(), ValidationError> {
    let valid = if is_slice {
        matches!((size, alignment), (8, 4) | (16, 8))
    } else {
        matches!((size, alignment), (4, 4) | (8, 8))
    };
    if valid {
        Ok(())
    } else {
        Err(ValidationError::InvalidLayout(
            "reference field does not match a supported pointer width",
        ))
    }
}

fn validate_reference_access(
    access: Access,
    address_space: AddressSpace,
) -> Result<(), ValidationError> {
    if access == Access::ByValue || address_space == AddressSpace::Value {
        return Err(ValidationError::InvalidAccess(
            "pointer and slice fields require memory access and a memory address space",
        ));
    }
    if address_space == AddressSpace::Constant && access.writes() {
        return Err(ValidationError::InvalidAccess(
            "constant address space is immutable",
        ));
    }
    Ok(())
}

fn validate_reference_ownership(
    mutability: Mutability,
    access: Access,
    ownership: ArgumentOwnership,
    alias_class: AliasClass,
) -> Result<(), ValidationError> {
    let valid = match ownership {
        ArgumentOwnership::ByValue => false,
        ArgumentOwnership::SharedBorrow => {
            mutability == Mutability::Immutable
                && ((alias_class == AliasClass::SharedReadOnly && access == Access::ReadOnly)
                    || (alias_class == AliasClass::SharedAtomic && access != Access::ByValue))
        }
        ArgumentOwnership::UniqueBorrow => {
            alias_class == AliasClass::Exclusive && mutability == Mutability::Mutable
        }
        ArgumentOwnership::RawPointer => {
            alias_class == AliasClass::Unrestricted
                && (mutability == Mutability::Mutable || !access.writes())
        }
    };
    if valid {
        Ok(())
    } else {
        Err(ValidationError::InvalidAccess(
            "reference ownership, aliasing, mutability, and access are inconsistent",
        ))
    }
}

fn validate_field_width(
    field: &AbiField,
    pointer_width: PointerWidth,
) -> Result<(), ValidationError> {
    let pointer_bytes = pointer_width.bytes();
    match field.kind {
        AbiKind::Scalar(_) => Ok(()),
        AbiKind::Pointer { .. }
            if field.size == pointer_bytes && u64::from(field.alignment) == pointer_bytes =>
        {
            Ok(())
        }
        AbiKind::Slice { .. }
            if field.size == pointer_bytes * 2 && u64::from(field.alignment) == pointer_bytes =>
        {
            Ok(())
        }
        AbiKind::Pointer { .. } => Err(ValidationError::InvalidLayout(
            "pointer field does not match target pointer width",
        )),
        AbiKind::Slice { .. } => Err(ValidationError::InvalidLayout(
            "slice field does not match target pointer width",
        )),
    }
}
