use crate::{AuthenticatedKernelArtifactV1, CompilerGeneratedKernelContractV1, KernelId};
use fe2o3_artifacts::{AbiField, AbiKind, AbiLayout, PointerWidth, ValidationError};
use std::fmt;

/// Compiler-generated expectation for one complete logical kernel ABI.
///
/// This is inert metadata. Constructing it does not authenticate an artifact
/// or grant load or launch authority. Only the unsafe generated SPI on
/// [`AuthenticatedKernelArtifactV1`] can turn it into a validated packing plan.
#[derive(Clone, Debug, Eq, PartialEq)]
#[doc(hidden)]
pub struct CompilerGeneratedArgumentLayoutV1 {
    layout: AbiLayout,
}

impl CompilerGeneratedArgumentLayoutV1 {
    /// Validates and records the layout emitted by a generated host adapter.
    pub fn new(
        kernarg_size: u64,
        kernarg_alignment: u32,
        pointer_width: PointerWidth,
        fields: Vec<AbiField>,
    ) -> Result<Self, GeneratedArgumentLayoutError> {
        validate_field_order(&fields)?;
        let layout = AbiLayout::new(kernarg_size, kernarg_alignment, pointer_width, fields)
            .map_err(GeneratedArgumentLayoutError::InvalidLayout)?;
        Ok(Self { layout })
    }
}

/// One physical value that a generated adapter must provide to HIP.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[doc(hidden)]
pub struct GeneratedPackingComponentV1 {
    argument_index: usize,
    kind: GeneratedPackingComponentKindV1,
    offset: u64,
    size: u64,
    alignment: u32,
}

impl GeneratedPackingComponentV1 {
    pub const fn argument_index(self) -> usize {
        self.argument_index
    }

    pub const fn kind(self) -> GeneratedPackingComponentKindV1 {
        self.kind
    }

    pub const fn offset(self) -> u64 {
        self.offset
    }

    pub const fn size(self) -> u64 {
        self.size
    }

    pub const fn alignment(self) -> u32 {
        self.alignment
    }
}

/// Physical role of one host launch parameter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[doc(hidden)]
#[non_exhaustive]
pub enum GeneratedPackingComponentKindV1 {
    Scalar,
    Pointer,
    SlicePointer,
    SliceLength,
}

/// Manifest-validated plan for packing one authenticated kernel ABI.
///
/// The plan describes field and component order only. It cannot construct a
/// kernel brand, admit aliases, load a payload, or launch a kernel. Its backing
/// storage is private, so downstream code cannot replace its contents or
/// destructure it into an apparently validated value.
#[derive(Clone, Debug, Eq, PartialEq)]
#[doc(hidden)]
pub struct GeneratedArgumentPackingPlanV1 {
    kernel_id: KernelId,
    kernarg_size: u64,
    kernarg_alignment: u32,
    pointer_width: PointerWidth,
    fields: Box<[AbiField]>,
    components: Box<[GeneratedPackingComponentV1]>,
}

impl GeneratedArgumentPackingPlanV1 {
    pub const fn kernel_id(&self) -> KernelId {
        self.kernel_id
    }

    pub const fn kernarg_size(&self) -> u64 {
        self.kernarg_size
    }

    pub const fn kernarg_alignment(&self) -> u32 {
        self.kernarg_alignment
    }

    pub const fn pointer_width(&self) -> PointerWidth {
        self.pointer_width
    }

    pub fn argument_count(&self) -> usize {
        self.fields.len()
    }

    pub fn argument(&self, index: usize) -> Option<&AbiField> {
        self.fields.get(index)
    }

    pub fn component_count(&self) -> usize {
        self.components.len()
    }

    pub fn component(&self, index: usize) -> Option<GeneratedPackingComponentV1> {
        self.components.get(index).copied()
    }
}

impl<K: CompilerGeneratedKernelContractV1> AuthenticatedKernelArtifactV1<K> {
    /// Validates compiler-generated packing metadata against this artifact's
    /// authenticated manifest ABI.
    ///
    /// # Safety
    ///
    /// `generated` must come from the trusted compiler analysis that emitted
    /// the host adapter for `K`. It must not be reconstructed from the artifact
    /// manifest being checked: that would be a self-comparison and would not
    /// establish the Rust type, layout, access, or ownership association. The
    /// caller must pack values of exactly those compiler-established Rust types
    /// and retain their resources through GPU completion.
    ///
    /// Success does not itself authorize loading or launch. Existing marker,
    /// geometry, context, alias-admission, and lifetime gates remain required.
    pub unsafe fn validate_argument_packing(
        &self,
        generated: &CompilerGeneratedArgumentLayoutV1,
    ) -> Result<GeneratedArgumentPackingPlanV1, GeneratedArgumentPackingError> {
        validate_argument_packing(
            self.identity().kernel_id(),
            self.identity().abi(),
            generated,
        )
    }
}

/// Structural defect in a compiler-generated logical argument layout.
#[derive(Clone, Debug, Eq, PartialEq)]
#[doc(hidden)]
#[non_exhaustive]
pub enum GeneratedArgumentLayoutError {
    ReorderedField {
        index: usize,
        previous_offset: u64,
        offset: u64,
    },
    OverlappingField {
        index: usize,
        previous_end: u64,
        offset: u64,
    },
    InvalidLayout(ValidationError),
}

impl fmt::Display for GeneratedArgumentLayoutError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReorderedField {
                index,
                previous_offset,
                offset,
            } => write!(
                formatter,
                "argument {index} offset {offset} precedes previous offset {previous_offset}"
            ),
            Self::OverlappingField {
                index,
                previous_end,
                offset,
            } => write!(
                formatter,
                "argument {index} offset {offset} overlaps previous end {previous_end}"
            ),
            Self::InvalidLayout(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for GeneratedArgumentLayoutError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidLayout(error) => Some(error),
            Self::ReorderedField { .. } | Self::OverlappingField { .. } => None,
        }
    }
}

/// Why generated packing metadata did not match the authenticated manifest.
#[derive(Clone, Debug, Eq, PartialEq)]
#[doc(hidden)]
#[non_exhaustive]
pub enum GeneratedArgumentPackingError {
    KernargSize {
        generated: u64,
        manifest: u64,
    },
    KernargAlignment {
        generated: u32,
        manifest: u32,
    },
    PointerWidth {
        generated: PointerWidth,
        manifest: PointerWidth,
    },
    ArgumentCount {
        generated: usize,
        manifest: usize,
    },
    FieldMismatch {
        index: usize,
        property: GeneratedArgumentFieldProperty,
    },
}

impl fmt::Display for GeneratedArgumentPackingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::KernargSize {
                generated,
                manifest,
            } => write!(
                formatter,
                "generated kernarg size {generated} does not match manifest size {manifest}"
            ),
            Self::KernargAlignment {
                generated,
                manifest,
            } => write!(
                formatter,
                "generated kernarg alignment {generated} does not match manifest alignment {manifest}"
            ),
            Self::PointerWidth {
                generated,
                manifest,
            } => write!(
                formatter,
                "generated pointer width {generated:?} does not match manifest width {manifest:?}"
            ),
            Self::ArgumentCount {
                generated,
                manifest,
            } => write!(
                formatter,
                "generated argument count {generated} does not match manifest count {manifest}"
            ),
            Self::FieldMismatch { index, property } => write!(
                formatter,
                "generated argument {index} has a manifest mismatch in {property}"
            ),
        }
    }
}

impl std::error::Error for GeneratedArgumentPackingError {}

/// Exact field property that differs between generated and manifest layouts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[doc(hidden)]
#[non_exhaustive]
pub enum GeneratedArgumentFieldProperty {
    Name,
    Offset,
    Size,
    Alignment,
    Kind,
    TypeIdentity,
    Mutability,
    Access,
    AddressSpace,
    Ownership,
    AliasClass,
}

impl fmt::Display for GeneratedArgumentFieldProperty {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Name => "name",
            Self::Offset => "offset",
            Self::Size => "size",
            Self::Alignment => "alignment",
            Self::Kind => "kind",
            Self::TypeIdentity => "Rust type/layout identity",
            Self::Mutability => "mutability",
            Self::Access => "access",
            Self::AddressSpace => "address space",
            Self::Ownership => "ownership",
            Self::AliasClass => "alias class",
        })
    }
}

fn validate_field_order(fields: &[AbiField]) -> Result<(), GeneratedArgumentLayoutError> {
    let mut previous_offset = 0_u64;
    let mut previous_end = 0_u64;
    for (index, field) in fields.iter().enumerate() {
        if index != 0 && field.offset() < previous_offset {
            return Err(GeneratedArgumentLayoutError::ReorderedField {
                index,
                previous_offset,
                offset: field.offset(),
            });
        }
        if field.offset() < previous_end {
            return Err(GeneratedArgumentLayoutError::OverlappingField {
                index,
                previous_end,
                offset: field.offset(),
            });
        }
        previous_offset = field.offset();
        previous_end = field
            .offset()
            .checked_add(field.size())
            .expect("AbiField construction validates its end");
    }
    Ok(())
}

fn validate_argument_packing(
    kernel_id: KernelId,
    manifest: &AbiLayout,
    generated: &CompilerGeneratedArgumentLayoutV1,
) -> Result<GeneratedArgumentPackingPlanV1, GeneratedArgumentPackingError> {
    let expected = &generated.layout;
    if expected.size() != manifest.size() {
        return Err(GeneratedArgumentPackingError::KernargSize {
            generated: expected.size(),
            manifest: manifest.size(),
        });
    }
    if expected.alignment() != manifest.alignment() {
        return Err(GeneratedArgumentPackingError::KernargAlignment {
            generated: expected.alignment(),
            manifest: manifest.alignment(),
        });
    }
    if expected.pointer_width() != manifest.pointer_width() {
        return Err(GeneratedArgumentPackingError::PointerWidth {
            generated: expected.pointer_width(),
            manifest: manifest.pointer_width(),
        });
    }
    if expected.fields().len() != manifest.fields().len() {
        return Err(GeneratedArgumentPackingError::ArgumentCount {
            generated: expected.fields().len(),
            manifest: manifest.fields().len(),
        });
    }

    for (index, (expected, actual)) in expected.fields().iter().zip(manifest.fields()).enumerate() {
        if let Some(property) = first_field_mismatch(expected, actual) {
            return Err(GeneratedArgumentPackingError::FieldMismatch { index, property });
        }
    }

    Ok(GeneratedArgumentPackingPlanV1 {
        kernel_id,
        kernarg_size: manifest.size(),
        kernarg_alignment: manifest.alignment(),
        pointer_width: manifest.pointer_width(),
        fields: manifest.fields().to_vec().into_boxed_slice(),
        components: packing_components(manifest).into_boxed_slice(),
    })
}

fn first_field_mismatch(
    generated: &AbiField,
    manifest: &AbiField,
) -> Option<GeneratedArgumentFieldProperty> {
    [
        (
            generated.name() == manifest.name(),
            GeneratedArgumentFieldProperty::Name,
        ),
        (
            generated.offset() == manifest.offset(),
            GeneratedArgumentFieldProperty::Offset,
        ),
        (
            generated.size() == manifest.size(),
            GeneratedArgumentFieldProperty::Size,
        ),
        (
            generated.alignment() == manifest.alignment(),
            GeneratedArgumentFieldProperty::Alignment,
        ),
        (
            generated.kind() == manifest.kind(),
            GeneratedArgumentFieldProperty::Kind,
        ),
        (
            generated.type_identity() == manifest.type_identity(),
            GeneratedArgumentFieldProperty::TypeIdentity,
        ),
        (
            generated.mutability() == manifest.mutability(),
            GeneratedArgumentFieldProperty::Mutability,
        ),
        (
            generated.access() == manifest.access(),
            GeneratedArgumentFieldProperty::Access,
        ),
        (
            generated.address_space() == manifest.address_space(),
            GeneratedArgumentFieldProperty::AddressSpace,
        ),
        (
            generated.ownership() == manifest.ownership(),
            GeneratedArgumentFieldProperty::Ownership,
        ),
        (
            generated.alias_class() == manifest.alias_class(),
            GeneratedArgumentFieldProperty::AliasClass,
        ),
    ]
    .into_iter()
    .find_map(|(matches, property)| (!matches).then_some(property))
}

fn packing_components(layout: &AbiLayout) -> Vec<GeneratedPackingComponentV1> {
    let pointer_size = match layout.pointer_width() {
        PointerWidth::Bits32 => 4,
        PointerWidth::Bits64 => 8,
    };
    let mut components = Vec::with_capacity(
        layout
            .fields()
            .iter()
            .map(|field| usize::from(matches!(field.kind(), AbiKind::Slice { .. })) + 1)
            .sum(),
    );
    for (argument_index, field) in layout.fields().iter().enumerate() {
        match field.kind() {
            AbiKind::Scalar(_) => components.push(GeneratedPackingComponentV1 {
                argument_index,
                kind: GeneratedPackingComponentKindV1::Scalar,
                offset: field.offset(),
                size: field.size(),
                alignment: field.alignment(),
            }),
            AbiKind::Pointer { .. } => components.push(GeneratedPackingComponentV1 {
                argument_index,
                kind: GeneratedPackingComponentKindV1::Pointer,
                offset: field.offset(),
                size: field.size(),
                alignment: field.alignment(),
            }),
            AbiKind::Slice { .. } => {
                components.push(GeneratedPackingComponentV1 {
                    argument_index,
                    kind: GeneratedPackingComponentKindV1::SlicePointer,
                    offset: field.offset(),
                    size: pointer_size,
                    alignment: field.alignment(),
                });
                components.push(GeneratedPackingComponentV1 {
                    argument_index,
                    kind: GeneratedPackingComponentKindV1::SliceLength,
                    offset: field.offset() + pointer_size,
                    size: pointer_size,
                    alignment: field.alignment(),
                });
            }
        }
    }
    components
}

#[cfg(test)]
mod tests {
    use super::{
        CompilerGeneratedArgumentLayoutV1, GeneratedArgumentFieldProperty,
        GeneratedArgumentLayoutError, GeneratedArgumentPackingError,
        GeneratedPackingComponentKindV1, validate_argument_packing,
    };
    use crate::KernelId;
    use fe2o3_artifacts::{
        AbiField, AbiKind, AbiLayout, Access, AddressSpace, AliasClass, ArgumentOwnership,
        DeclaredRustLayoutIdentity, DeclaredRustTypeIdentity, DigestBytes, Mutability, Name,
        PointerWidth, ScalarType, TypeIdentity,
    };

    const KERNEL_ID: KernelId = KernelId::from_bytes([9; 32]);

    fn identity(seed: u8) -> TypeIdentity {
        TypeIdentity::new(
            DeclaredRustTypeIdentity::from_untrusted_bytes(DigestBytes::from_bytes([seed; 32])),
            DeclaredRustLayoutIdentity::from_untrusted_bytes(DigestBytes::from_bytes(
                [seed.wrapping_add(1); 32],
            )),
        )
    }

    fn scalar(name: &str, offset: u64, alignment: u32, seed: u8) -> AbiField {
        AbiField::new(
            Name::new(name).unwrap(),
            offset,
            4,
            alignment,
            AbiKind::Scalar(ScalarType::U32),
            Mutability::Immutable,
            Access::ByValue,
            AddressSpace::Value,
            identity(seed),
            ArgumentOwnership::ByValue,
            AliasClass::Value,
        )
        .unwrap()
    }

    fn reference(
        name: &str,
        offset: u64,
        kind: AbiKind,
        access: Access,
        address_space: AddressSpace,
        seed: u8,
    ) -> AbiField {
        let mutable = access != Access::ReadOnly;
        let size = if matches!(kind, AbiKind::Slice { .. }) {
            16
        } else {
            8
        };
        AbiField::new(
            Name::new(name).unwrap(),
            offset,
            size,
            8,
            kind,
            if mutable {
                Mutability::Mutable
            } else {
                Mutability::Immutable
            },
            access,
            address_space,
            identity(seed),
            if mutable {
                ArgumentOwnership::UniqueBorrow
            } else {
                ArgumentOwnership::SharedBorrow
            },
            if mutable {
                AliasClass::Exclusive
            } else {
                AliasClass::SharedReadOnly
            },
        )
        .unwrap()
    }

    fn pointer(name: &str, offset: u64, access: Access, space: AddressSpace, seed: u8) -> AbiField {
        reference(
            name,
            offset,
            AbiKind::Pointer {
                pointee_size: 4,
                pointee_alignment: 4,
            },
            access,
            space,
            seed,
        )
    }

    fn slice(name: &str, offset: u64, seed: u8) -> AbiField {
        reference(
            name,
            offset,
            AbiKind::Slice {
                element_size: 4,
                element_alignment: 4,
            },
            Access::ReadOnly,
            AddressSpace::Global,
            seed,
        )
    }

    fn layout(fields: Vec<AbiField>, size: u64, alignment: u32) -> AbiLayout {
        AbiLayout::new(size, alignment, PointerWidth::Bits64, fields).unwrap()
    }

    fn generated(
        fields: Vec<AbiField>,
        size: u64,
        alignment: u32,
    ) -> CompilerGeneratedArgumentLayoutV1 {
        CompilerGeneratedArgumentLayoutV1::new(size, alignment, PointerWidth::Bits64, fields)
            .unwrap()
    }

    fn validate(
        manifest: &AbiLayout,
        generated: &CompilerGeneratedArgumentLayoutV1,
    ) -> Result<super::GeneratedArgumentPackingPlanV1, GeneratedArgumentPackingError> {
        validate_argument_packing(KERNEL_ID, manifest, generated)
    }

    #[test]
    fn generic_manifest_layout_derives_bound_physical_packing_order() {
        let fields = vec![scalar("count", 0, 4, 1), slice("values", 8, 2)];
        let manifest = layout(fields.clone(), 24, 8);
        let plan = validate(&manifest, &generated(fields, 24, 8)).unwrap();

        assert_eq!(plan.kernel_id(), KERNEL_ID);
        assert_eq!(plan.kernarg_size(), 24);
        assert_eq!(plan.argument_count(), 2);
        assert_eq!(plan.component_count(), 3);
        assert_eq!(
            plan.component(0).unwrap().kind(),
            GeneratedPackingComponentKindV1::Scalar
        );
        assert_eq!(plan.component(0).unwrap().offset(), 0);
        assert_eq!(
            plan.component(1).unwrap().kind(),
            GeneratedPackingComponentKindV1::SlicePointer
        );
        assert_eq!(plan.component(1).unwrap().offset(), 8);
        assert_eq!(
            plan.component(2).unwrap().kind(),
            GeneratedPackingComponentKindV1::SliceLength
        );
        assert_eq!(plan.component(2).unwrap().offset(), 16);
    }

    #[test]
    fn generated_fields_reject_reorder_and_overlap() {
        let first = pointer("first", 0, Access::ReadOnly, AddressSpace::Global, 1);
        let second = pointer("second", 8, Access::ReadOnly, AddressSpace::Global, 2);
        assert!(matches!(
            CompilerGeneratedArgumentLayoutV1::new(
                16,
                8,
                PointerWidth::Bits64,
                vec![second, first.clone()]
            ),
            Err(GeneratedArgumentLayoutError::ReorderedField { index: 1, .. })
        ));

        let overlapping = scalar("overlapping", 4, 4, 2);
        assert!(matches!(
            CompilerGeneratedArgumentLayoutV1::new(
                16,
                8,
                PointerWidth::Bits64,
                vec![first, overlapping]
            ),
            Err(GeneratedArgumentLayoutError::OverlappingField { index: 1, .. })
        ));
    }

    #[test]
    fn type_size_and_alignment_must_match() {
        let manifest = layout(vec![scalar("value", 0, 4, 1)], 8, 8);
        assert_eq!(
            validate(&manifest, &generated(vec![scalar("value", 0, 4, 2)], 8, 8)),
            Err(GeneratedArgumentPackingError::FieldMismatch {
                index: 0,
                property: GeneratedArgumentFieldProperty::TypeIdentity,
            })
        );

        let manifest = layout(vec![slice("value", 0, 1)], 16, 8);
        let wrong_size = pointer("value", 0, Access::ReadOnly, AddressSpace::Global, 1);
        assert_eq!(
            validate(&manifest, &generated(vec![wrong_size], 16, 8)),
            Err(GeneratedArgumentPackingError::FieldMismatch {
                index: 0,
                property: GeneratedArgumentFieldProperty::Size,
            })
        );

        let manifest = layout(vec![scalar("value", 0, 4, 1)], 8, 8);
        assert_eq!(
            validate(&manifest, &generated(vec![scalar("value", 0, 2, 1)], 8, 8)),
            Err(GeneratedArgumentPackingError::FieldMismatch {
                index: 0,
                property: GeneratedArgumentFieldProperty::Alignment,
            })
        );
    }

    #[test]
    fn access_mutability_and_address_space_must_match() {
        let manifest = layout(
            vec![pointer(
                "value",
                0,
                Access::ReadOnly,
                AddressSpace::Global,
                1,
            )],
            8,
            8,
        );
        let wrong_mutability = pointer("value", 0, Access::WriteOnly, AddressSpace::Global, 1);
        assert_eq!(
            validate(&manifest, &generated(vec![wrong_mutability], 8, 8)),
            Err(GeneratedArgumentPackingError::FieldMismatch {
                index: 0,
                property: GeneratedArgumentFieldProperty::Mutability,
            })
        );

        let mutable_manifest = layout(
            vec![pointer(
                "value",
                0,
                Access::WriteOnly,
                AddressSpace::Global,
                1,
            )],
            8,
            8,
        );
        let wrong_access = pointer("value", 0, Access::ReadWrite, AddressSpace::Global, 1);
        assert_eq!(
            validate(&mutable_manifest, &generated(vec![wrong_access], 8, 8)),
            Err(GeneratedArgumentPackingError::FieldMismatch {
                index: 0,
                property: GeneratedArgumentFieldProperty::Access,
            })
        );

        let wrong_space = pointer("value", 0, Access::ReadOnly, AddressSpace::Generic, 1);
        assert_eq!(
            validate(&manifest, &generated(vec![wrong_space], 8, 8)),
            Err(GeneratedArgumentPackingError::FieldMismatch {
                index: 0,
                property: GeneratedArgumentFieldProperty::AddressSpace,
            })
        );
    }

    #[test]
    fn total_size_and_bounds_are_checked() {
        let field = pointer("value", 0, Access::ReadOnly, AddressSpace::Global, 1);
        let manifest = layout(vec![field.clone()], 8, 8);
        assert_eq!(
            validate(&manifest, &generated(vec![field.clone()], 16, 8)),
            Err(GeneratedArgumentPackingError::KernargSize {
                generated: 16,
                manifest: 8,
            })
        );

        assert!(matches!(
            CompilerGeneratedArgumentLayoutV1::new(4, 4, PointerWidth::Bits64, vec![field]),
            Err(GeneratedArgumentLayoutError::InvalidLayout(
                fe2o3_artifacts::ValidationError::InvalidLayout(_)
            ))
        ));
    }
}
