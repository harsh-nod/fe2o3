use crate::{AuthenticatedKernelArtifactV1, CompilerGeneratedKernelContractV1, KernelId};
use fe2o3_artifacts::{
    AbiField, AbiKind, AbiLayout, Access, AddressSpace, MAX_ABI_BYTES, PointerWidth, ScalarType,
    ValidationError,
};
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

#[derive(Clone, Debug, Eq, PartialEq)]
enum GeneratedArgumentValueV1 {
    Scalar {
        scalar_type: ScalarType,
        bytes: [u8; 8],
        byte_length: u8,
    },
    Slice {
        address: u64,
        length: u64,
        pointer_width: PointerWidth,
        address_space: AddressSpace,
        access: Access,
    },
}

/// One value bound by generated code to an exact manifest argument.
///
/// The fields are private so downstream code cannot relabel a value as a
/// different source argument or physical component. Construction is available
/// only through a manifest-validated [`GeneratedArgumentPackingPlanV1`].
#[derive(Clone, Debug, Eq, PartialEq)]
#[doc(hidden)]
pub struct GeneratedArgumentInputV1 {
    kernel_id: KernelId,
    source_field: AbiField,
    argument_index: usize,
    value: GeneratedArgumentValueV1,
}

/// Inert, deterministically initialized kernel-argument bytes.
///
/// This value contains no module, context, alias admission, retained resource,
/// or launch authority. In particular, it deliberately has no raw-pointer
/// accessor. Generated launch code must pair it with the existing lifetime and
/// admission capabilities before an unsafe runtime boundary may consume it.
#[must_use = "packed arguments are inert until paired with launch authority"]
#[doc(hidden)]
pub struct GeneratedPackedArgumentsV1 {
    kernel_id: KernelId,
    alignment: u32,
    bytes: Box<[u8]>,
}

impl GeneratedPackedArgumentsV1 {
    pub const fn kernel_id(&self) -> KernelId {
        self.kernel_id
    }

    pub const fn alignment(&self) -> u32 {
        self.alignment
    }

    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Returns the inert packed representation for generated runtime plumbing.
    ///
    /// Addresses encoded in these bytes remain subject to the unsafe promises
    /// made when their slice inputs were constructed. Access to these bytes
    /// alone does not authorize loading or launching a kernel.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

impl fmt::Debug for GeneratedPackedArgumentsV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedPackedArgumentsV1")
            .field("kernel_id", &self.kernel_id)
            .field("alignment", &self.alignment)
            .field("byte_length", &self.bytes.len())
            .finish_non_exhaustive()
    }
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

    pub fn scalar_i8(
        &self,
        argument_index: usize,
        value: i8,
    ) -> Result<GeneratedArgumentInputV1, GeneratedArgumentPackError> {
        self.bind_scalar(argument_index, ScalarType::I8, &[value as u8])
    }

    pub fn scalar_u8(
        &self,
        argument_index: usize,
        value: u8,
    ) -> Result<GeneratedArgumentInputV1, GeneratedArgumentPackError> {
        self.bind_scalar(argument_index, ScalarType::U8, &[value])
    }

    pub fn scalar_i16(
        &self,
        argument_index: usize,
        value: i16,
    ) -> Result<GeneratedArgumentInputV1, GeneratedArgumentPackError> {
        self.bind_scalar(argument_index, ScalarType::I16, &value.to_le_bytes())
    }

    pub fn scalar_u16(
        &self,
        argument_index: usize,
        value: u16,
    ) -> Result<GeneratedArgumentInputV1, GeneratedArgumentPackError> {
        self.bind_scalar(argument_index, ScalarType::U16, &value.to_le_bytes())
    }

    /// Binds the exact IEEE-754 binary16 bit representation generated for an
    /// `f16` argument. Rust has no stable `f16` primitive on the supported MSRV.
    pub fn scalar_f16_bits(
        &self,
        argument_index: usize,
        bits: u16,
    ) -> Result<GeneratedArgumentInputV1, GeneratedArgumentPackError> {
        self.bind_scalar(argument_index, ScalarType::F16, &bits.to_le_bytes())
    }

    pub fn scalar_i32(
        &self,
        argument_index: usize,
        value: i32,
    ) -> Result<GeneratedArgumentInputV1, GeneratedArgumentPackError> {
        self.bind_scalar(argument_index, ScalarType::I32, &value.to_le_bytes())
    }

    pub fn scalar_u32(
        &self,
        argument_index: usize,
        value: u32,
    ) -> Result<GeneratedArgumentInputV1, GeneratedArgumentPackError> {
        self.bind_scalar(argument_index, ScalarType::U32, &value.to_le_bytes())
    }

    pub fn scalar_f32(
        &self,
        argument_index: usize,
        value: f32,
    ) -> Result<GeneratedArgumentInputV1, GeneratedArgumentPackError> {
        self.bind_scalar(
            argument_index,
            ScalarType::F32,
            &value.to_bits().to_le_bytes(),
        )
    }

    pub fn scalar_i64(
        &self,
        argument_index: usize,
        value: i64,
    ) -> Result<GeneratedArgumentInputV1, GeneratedArgumentPackError> {
        self.bind_scalar(argument_index, ScalarType::I64, &value.to_le_bytes())
    }

    pub fn scalar_u64(
        &self,
        argument_index: usize,
        value: u64,
    ) -> Result<GeneratedArgumentInputV1, GeneratedArgumentPackError> {
        self.bind_scalar(argument_index, ScalarType::U64, &value.to_le_bytes())
    }

    pub fn scalar_f64(
        &self,
        argument_index: usize,
        value: f64,
    ) -> Result<GeneratedArgumentInputV1, GeneratedArgumentPackError> {
        self.bind_scalar(
            argument_index,
            ScalarType::F64,
            &value.to_bits().to_le_bytes(),
        )
    }

    /// Binds one generated device slice to its exact logical argument.
    ///
    /// `pointer_width`, `address_space`, and `access` are checked against the
    /// authenticated manifest before an input is returned. Pointer and length
    /// values that do not fit the selected physical width are rejected.
    ///
    /// # Safety
    ///
    /// `device_pointer` must be the device address supplied by the retained
    /// resource capability for this exact generated argument. It must address
    /// at least `length` initialized elements of the compiler-established type
    /// in the stated address space, with the stated access permitted, and must
    /// remain live until GPU completion. The caller must preserve the required
    /// aliasing and context association separately. This function converts the
    /// opaque device pointer to bytes but does not grant launch authority.
    pub unsafe fn slice(
        &self,
        argument_index: usize,
        device_pointer: *const (),
        length: u64,
        pointer_width: PointerWidth,
        address_space: AddressSpace,
        access: Access,
    ) -> Result<GeneratedArgumentInputV1, GeneratedArgumentPackError> {
        let address = u64::try_from(device_pointer.addr()).map_err(|_| {
            GeneratedArgumentPackError::IntegerWidthOverflow {
                argument_index,
                component: GeneratedPackingComponentKindV1::SlicePointer,
                value: u64::MAX,
                pointer_width,
            }
        })?;
        self.bind_input(
            argument_index,
            GeneratedArgumentValueV1::Slice {
                address,
                length,
                pointer_width,
                address_space,
                access,
            },
        )
    }

    /// Packs a complete set of generated inputs into exact manifest offsets.
    ///
    /// Input order is irrelevant. Every logical manifest argument must appear
    /// exactly once and remain bound to its original field. Padding starts as
    /// zero and is never overwritten. Standalone raw-pointer arguments are not
    /// supported by this executor because no retained capability is represented
    /// here.
    pub fn pack(
        &self,
        inputs: impl IntoIterator<Item = GeneratedArgumentInputV1>,
    ) -> Result<GeneratedPackedArgumentsV1, GeneratedArgumentPackError> {
        pack_arguments(self, inputs)
    }

    fn bind_scalar(
        &self,
        argument_index: usize,
        scalar_type: ScalarType,
        bytes: &[u8],
    ) -> Result<GeneratedArgumentInputV1, GeneratedArgumentPackError> {
        let mut storage = [0_u8; 8];
        storage[..bytes.len()].copy_from_slice(bytes);
        self.bind_input(
            argument_index,
            GeneratedArgumentValueV1::Scalar {
                scalar_type,
                bytes: storage,
                byte_length: bytes.len() as u8,
            },
        )
    }

    fn bind_input(
        &self,
        argument_index: usize,
        value: GeneratedArgumentValueV1,
    ) -> Result<GeneratedArgumentInputV1, GeneratedArgumentPackError> {
        let source_field = self.fields.get(argument_index).ok_or(
            GeneratedArgumentPackError::ArgumentIndexOutOfBounds {
                argument_index,
                argument_count: self.fields.len(),
            },
        )?;
        let input = GeneratedArgumentInputV1 {
            kernel_id: self.kernel_id,
            source_field: source_field.clone(),
            argument_index,
            value,
        };
        validate_input(self, &input)?;
        Ok(input)
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

/// Why manifest-driven argument-value binding or packing failed.
#[derive(Clone, Debug, Eq, PartialEq)]
#[doc(hidden)]
#[non_exhaustive]
pub enum GeneratedArgumentPackError {
    KernargTooLarge {
        size: u64,
        maximum: u64,
    },
    UnsupportedPointerArgument {
        argument_index: usize,
    },
    ArgumentIndexOutOfBounds {
        argument_index: usize,
        argument_count: usize,
    },
    SourceKernelMismatch {
        argument_index: usize,
    },
    SourceFieldMismatch {
        argument_index: usize,
    },
    DuplicateArgument {
        argument_index: usize,
    },
    MissingArgument {
        argument_index: usize,
    },
    KindMismatch {
        argument_index: usize,
        expected: &'static str,
        provided: &'static str,
    },
    ScalarTypeMismatch {
        argument_index: usize,
        expected: ScalarType,
        provided: ScalarType,
    },
    ComponentWidthMismatch {
        argument_index: usize,
        component: GeneratedPackingComponentKindV1,
        expected: u64,
        provided: u64,
    },
    PointerWidthMismatch {
        argument_index: usize,
        expected: PointerWidth,
        provided: PointerWidth,
    },
    AddressSpaceMismatch {
        argument_index: usize,
        expected: AddressSpace,
        provided: AddressSpace,
    },
    AccessMismatch {
        argument_index: usize,
        expected: Access,
        provided: Access,
    },
    IntegerWidthOverflow {
        argument_index: usize,
        component: GeneratedPackingComponentKindV1,
        value: u64,
        pointer_width: PointerWidth,
    },
    NullSlicePointer {
        argument_index: usize,
        length: u64,
    },
    PhysicalComponentMismatch {
        argument_index: usize,
        component: GeneratedPackingComponentKindV1,
    },
    ComponentOutOfBounds {
        argument_index: usize,
        component: GeneratedPackingComponentKindV1,
        offset: u64,
        size: u64,
        kernarg_size: u64,
    },
}

impl fmt::Display for GeneratedArgumentPackError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::KernargTooLarge { size, maximum } => {
                write!(
                    formatter,
                    "kernarg size {size} exceeds packing limit {maximum}"
                )
            }
            Self::UnsupportedPointerArgument { argument_index } => write!(
                formatter,
                "argument {argument_index} is a standalone pointer, which this executor does not support"
            ),
            Self::ArgumentIndexOutOfBounds {
                argument_index,
                argument_count,
            } => write!(
                formatter,
                "argument index {argument_index} is outside the {argument_count}-argument plan"
            ),
            Self::SourceKernelMismatch { argument_index } => write!(
                formatter,
                "argument {argument_index} was bound by a different kernel plan"
            ),
            Self::SourceFieldMismatch { argument_index } => write!(
                formatter,
                "argument {argument_index} was bound to a different manifest field"
            ),
            Self::DuplicateArgument { argument_index } => {
                write!(
                    formatter,
                    "argument {argument_index} was provided more than once"
                )
            }
            Self::MissingArgument { argument_index } => {
                write!(formatter, "argument {argument_index} was not provided")
            }
            Self::KindMismatch {
                argument_index,
                expected,
                provided,
            } => write!(
                formatter,
                "argument {argument_index} requires {expected}, but {provided} was provided"
            ),
            Self::ScalarTypeMismatch {
                argument_index,
                expected,
                provided,
            } => write!(
                formatter,
                "argument {argument_index} requires scalar {expected:?}, but {provided:?} was provided"
            ),
            Self::ComponentWidthMismatch {
                argument_index,
                component,
                expected,
                provided,
            } => write!(
                formatter,
                "argument {argument_index} component {component:?} requires {expected} bytes, but {provided} were provided"
            ),
            Self::PointerWidthMismatch {
                argument_index,
                expected,
                provided,
            } => write!(
                formatter,
                "argument {argument_index} requires pointer width {expected:?}, but {provided:?} was provided"
            ),
            Self::AddressSpaceMismatch {
                argument_index,
                expected,
                provided,
            } => write!(
                formatter,
                "argument {argument_index} requires address space {expected:?}, but {provided:?} was provided"
            ),
            Self::AccessMismatch {
                argument_index,
                expected,
                provided,
            } => write!(
                formatter,
                "argument {argument_index} requires access {expected:?}, but {provided:?} was provided"
            ),
            Self::IntegerWidthOverflow {
                argument_index,
                component,
                value,
                pointer_width,
            } => write!(
                formatter,
                "argument {argument_index} component {component:?} value {value} does not fit {pointer_width:?}"
            ),
            Self::NullSlicePointer {
                argument_index,
                length,
            } => write!(
                formatter,
                "argument {argument_index} has null slice pointer with nonzero length {length}"
            ),
            Self::PhysicalComponentMismatch {
                argument_index,
                component,
            } => write!(
                formatter,
                "argument {argument_index} has no exact {component:?} physical component"
            ),
            Self::ComponentOutOfBounds {
                argument_index,
                component,
                offset,
                size,
                kernarg_size,
            } => write!(
                formatter,
                "argument {argument_index} component {component:?} at {offset}+{size} exceeds kernarg size {kernarg_size}"
            ),
        }
    }
}

impl std::error::Error for GeneratedArgumentPackError {}

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

fn pack_arguments(
    plan: &GeneratedArgumentPackingPlanV1,
    inputs: impl IntoIterator<Item = GeneratedArgumentInputV1>,
) -> Result<GeneratedPackedArgumentsV1, GeneratedArgumentPackError> {
    if plan.kernarg_size > MAX_ABI_BYTES {
        return Err(GeneratedArgumentPackError::KernargTooLarge {
            size: plan.kernarg_size,
            maximum: MAX_ABI_BYTES,
        });
    }
    for (argument_index, field) in plan.fields.iter().enumerate() {
        if matches!(field.kind(), AbiKind::Pointer { .. }) {
            return Err(GeneratedArgumentPackError::UnsupportedPointerArgument { argument_index });
        }
    }

    let mut by_argument = vec![None; plan.fields.len()];
    for input in inputs {
        validate_input(plan, &input)?;
        let argument_index = input.argument_index;
        let slot = by_argument.get_mut(argument_index).ok_or(
            GeneratedArgumentPackError::ArgumentIndexOutOfBounds {
                argument_index,
                argument_count: plan.fields.len(),
            },
        )?;
        if slot.is_some() {
            return Err(GeneratedArgumentPackError::DuplicateArgument { argument_index });
        }
        *slot = Some(input);
    }

    let mut bytes = vec![
        0_u8;
        usize::try_from(plan.kernarg_size).map_err(|_| {
            GeneratedArgumentPackError::KernargTooLarge {
                size: plan.kernarg_size,
                maximum: MAX_ABI_BYTES,
            }
        })?
    ];
    for (argument_index, input) in by_argument.into_iter().enumerate() {
        let input = input.ok_or(GeneratedArgumentPackError::MissingArgument { argument_index })?;
        match input.value {
            GeneratedArgumentValueV1::Scalar {
                bytes: value,
                byte_length,
                ..
            } => {
                let component = exact_component(
                    plan,
                    argument_index,
                    GeneratedPackingComponentKindV1::Scalar,
                )?;
                write_component(
                    &mut bytes,
                    plan.kernarg_size,
                    component,
                    &value[..usize::from(byte_length)],
                )?;
            }
            GeneratedArgumentValueV1::Slice {
                address,
                length,
                pointer_width,
                ..
            } => {
                let pointer = encode_width(address, pointer_width);
                let pointer_component = exact_component(
                    plan,
                    argument_index,
                    GeneratedPackingComponentKindV1::SlicePointer,
                )?;
                write_component(
                    &mut bytes,
                    plan.kernarg_size,
                    pointer_component,
                    &pointer[..usize::try_from(pointer_width.bytes()).expect("width is bounded")],
                )?;

                let encoded_length = encode_width(length, pointer_width);
                let length_component = exact_component(
                    plan,
                    argument_index,
                    GeneratedPackingComponentKindV1::SliceLength,
                )?;
                write_component(
                    &mut bytes,
                    plan.kernarg_size,
                    length_component,
                    &encoded_length
                        [..usize::try_from(pointer_width.bytes()).expect("width is bounded")],
                )?;
            }
        }
    }

    Ok(GeneratedPackedArgumentsV1 {
        kernel_id: plan.kernel_id,
        alignment: plan.kernarg_alignment,
        bytes: bytes.into_boxed_slice(),
    })
}

fn validate_input(
    plan: &GeneratedArgumentPackingPlanV1,
    input: &GeneratedArgumentInputV1,
) -> Result<(), GeneratedArgumentPackError> {
    let argument_index = input.argument_index;
    let field = plan.fields.get(argument_index).ok_or(
        GeneratedArgumentPackError::ArgumentIndexOutOfBounds {
            argument_index,
            argument_count: plan.fields.len(),
        },
    )?;
    if input.kernel_id != plan.kernel_id {
        return Err(GeneratedArgumentPackError::SourceKernelMismatch { argument_index });
    }
    if input.source_field != *field {
        return Err(GeneratedArgumentPackError::SourceFieldMismatch { argument_index });
    }

    match (&input.value, field.kind()) {
        (
            GeneratedArgumentValueV1::Scalar {
                scalar_type,
                byte_length,
                ..
            },
            AbiKind::Scalar(expected),
        ) => {
            if *scalar_type != expected {
                return Err(GeneratedArgumentPackError::ScalarTypeMismatch {
                    argument_index,
                    expected,
                    provided: *scalar_type,
                });
            }
            let provided = u64::from(*byte_length);
            if provided != scalar_width(*scalar_type) || provided != field.size() {
                return Err(GeneratedArgumentPackError::ComponentWidthMismatch {
                    argument_index,
                    component: GeneratedPackingComponentKindV1::Scalar,
                    expected: field.size(),
                    provided,
                });
            }
            validate_exact_component(
                plan,
                argument_index,
                GeneratedPackingComponentKindV1::Scalar,
                field.offset(),
                field.size(),
            )
        }
        (GeneratedArgumentValueV1::Scalar { .. }, AbiKind::Slice { .. }) => {
            Err(GeneratedArgumentPackError::KindMismatch {
                argument_index,
                expected: "slice",
                provided: "scalar",
            })
        }
        (GeneratedArgumentValueV1::Scalar { .. }, AbiKind::Pointer { .. }) => {
            Err(GeneratedArgumentPackError::KindMismatch {
                argument_index,
                expected: "pointer",
                provided: "scalar",
            })
        }
        (
            GeneratedArgumentValueV1::Slice {
                address,
                length,
                pointer_width,
                address_space,
                access,
            },
            AbiKind::Slice { .. },
        ) => {
            if *pointer_width != plan.pointer_width {
                return Err(GeneratedArgumentPackError::PointerWidthMismatch {
                    argument_index,
                    expected: plan.pointer_width,
                    provided: *pointer_width,
                });
            }
            if *address_space != field.address_space() {
                return Err(GeneratedArgumentPackError::AddressSpaceMismatch {
                    argument_index,
                    expected: field.address_space(),
                    provided: *address_space,
                });
            }
            if *access != field.access() {
                return Err(GeneratedArgumentPackError::AccessMismatch {
                    argument_index,
                    expected: field.access(),
                    provided: *access,
                });
            }
            if *address == 0 && *length != 0 {
                return Err(GeneratedArgumentPackError::NullSlicePointer {
                    argument_index,
                    length: *length,
                });
            }
            validate_width_value(
                argument_index,
                GeneratedPackingComponentKindV1::SlicePointer,
                *address,
                *pointer_width,
            )?;
            validate_width_value(
                argument_index,
                GeneratedPackingComponentKindV1::SliceLength,
                *length,
                *pointer_width,
            )?;
            let width = pointer_width.bytes();
            validate_exact_component(
                plan,
                argument_index,
                GeneratedPackingComponentKindV1::SlicePointer,
                field.offset(),
                width,
            )?;
            validate_exact_component(
                plan,
                argument_index,
                GeneratedPackingComponentKindV1::SliceLength,
                field.offset() + width,
                width,
            )
        }
        (GeneratedArgumentValueV1::Slice { .. }, AbiKind::Scalar(_)) => {
            Err(GeneratedArgumentPackError::KindMismatch {
                argument_index,
                expected: "scalar",
                provided: "slice",
            })
        }
        (GeneratedArgumentValueV1::Slice { .. }, AbiKind::Pointer { .. }) => {
            Err(GeneratedArgumentPackError::KindMismatch {
                argument_index,
                expected: "pointer",
                provided: "slice",
            })
        }
    }
}

fn validate_exact_component(
    plan: &GeneratedArgumentPackingPlanV1,
    argument_index: usize,
    kind: GeneratedPackingComponentKindV1,
    offset: u64,
    size: u64,
) -> Result<(), GeneratedArgumentPackError> {
    let component = exact_component(plan, argument_index, kind)?;
    if component.offset != offset
        || component.size != size
        || component.offset % u64::from(component.alignment) != 0
        || component.alignment > plan.kernarg_alignment
    {
        return Err(GeneratedArgumentPackError::PhysicalComponentMismatch {
            argument_index,
            component: kind,
        });
    }
    Ok(())
}

fn exact_component(
    plan: &GeneratedArgumentPackingPlanV1,
    argument_index: usize,
    kind: GeneratedPackingComponentKindV1,
) -> Result<GeneratedPackingComponentV1, GeneratedArgumentPackError> {
    let mut matches =
        plan.components.iter().copied().filter(|component| {
            component.argument_index == argument_index && component.kind == kind
        });
    let component =
        matches
            .next()
            .ok_or(GeneratedArgumentPackError::PhysicalComponentMismatch {
                argument_index,
                component: kind,
            })?;
    if matches.next().is_some() {
        return Err(GeneratedArgumentPackError::PhysicalComponentMismatch {
            argument_index,
            component: kind,
        });
    }
    Ok(component)
}

fn write_component(
    destination: &mut [u8],
    kernarg_size: u64,
    component: GeneratedPackingComponentV1,
    value: &[u8],
) -> Result<(), GeneratedArgumentPackError> {
    if component.size != value.len() as u64 {
        return Err(GeneratedArgumentPackError::ComponentWidthMismatch {
            argument_index: component.argument_index,
            component: component.kind,
            expected: component.size,
            provided: value.len() as u64,
        });
    }
    let end = component.offset.checked_add(component.size).ok_or(
        GeneratedArgumentPackError::ComponentOutOfBounds {
            argument_index: component.argument_index,
            component: component.kind,
            offset: component.offset,
            size: component.size,
            kernarg_size,
        },
    )?;
    if end > kernarg_size {
        return Err(GeneratedArgumentPackError::ComponentOutOfBounds {
            argument_index: component.argument_index,
            component: component.kind,
            offset: component.offset,
            size: component.size,
            kernarg_size,
        });
    }
    let start = usize::try_from(component.offset).map_err(|_| {
        GeneratedArgumentPackError::ComponentOutOfBounds {
            argument_index: component.argument_index,
            component: component.kind,
            offset: component.offset,
            size: component.size,
            kernarg_size,
        }
    })?;
    let end =
        usize::try_from(end).map_err(|_| GeneratedArgumentPackError::ComponentOutOfBounds {
            argument_index: component.argument_index,
            component: component.kind,
            offset: component.offset,
            size: component.size,
            kernarg_size,
        })?;
    let slot = destination.get_mut(start..end).ok_or(
        GeneratedArgumentPackError::ComponentOutOfBounds {
            argument_index: component.argument_index,
            component: component.kind,
            offset: component.offset,
            size: component.size,
            kernarg_size,
        },
    )?;
    slot.copy_from_slice(value);
    Ok(())
}

fn validate_width_value(
    argument_index: usize,
    component: GeneratedPackingComponentKindV1,
    value: u64,
    pointer_width: PointerWidth,
) -> Result<(), GeneratedArgumentPackError> {
    if pointer_width == PointerWidth::Bits32 && value > u64::from(u32::MAX) {
        return Err(GeneratedArgumentPackError::IntegerWidthOverflow {
            argument_index,
            component,
            value,
            pointer_width,
        });
    }
    Ok(())
}

fn encode_width(value: u64, pointer_width: PointerWidth) -> [u8; 8] {
    match pointer_width {
        PointerWidth::Bits32 => {
            let mut encoded = [0_u8; 8];
            encoded[..4].copy_from_slice(&(value as u32).to_le_bytes());
            encoded
        }
        PointerWidth::Bits64 => value.to_le_bytes(),
    }
}

const fn scalar_width(scalar_type: ScalarType) -> u64 {
    match scalar_type {
        ScalarType::I8 | ScalarType::U8 => 1,
        ScalarType::I16 | ScalarType::U16 | ScalarType::F16 => 2,
        ScalarType::I32 | ScalarType::U32 | ScalarType::F32 => 4,
        ScalarType::I64 | ScalarType::U64 | ScalarType::F64 => 8,
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

pub(crate) fn validate_argument_packing(
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
        GeneratedArgumentLayoutError, GeneratedArgumentPackError, GeneratedArgumentPackingError,
        GeneratedArgumentValueV1, GeneratedPackingComponentKindV1, validate_argument_packing,
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
        typed_scalar(name, offset, alignment, ScalarType::U32, seed)
    }

    fn typed_scalar(
        name: &str,
        offset: u64,
        alignment: u32,
        scalar_type: ScalarType,
        seed: u8,
    ) -> AbiField {
        AbiField::new(
            Name::new(name).unwrap(),
            offset,
            super::scalar_width(scalar_type),
            alignment,
            AbiKind::Scalar(scalar_type),
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
        layout_with_width(fields, size, alignment, PointerWidth::Bits64)
    }

    fn layout_with_width(
        fields: Vec<AbiField>,
        size: u64,
        alignment: u32,
        pointer_width: PointerWidth,
    ) -> AbiLayout {
        AbiLayout::new(size, alignment, pointer_width, fields).unwrap()
    }

    fn generated(
        fields: Vec<AbiField>,
        size: u64,
        alignment: u32,
    ) -> CompilerGeneratedArgumentLayoutV1 {
        generated_with_width(fields, size, alignment, PointerWidth::Bits64)
    }

    fn generated_with_width(
        fields: Vec<AbiField>,
        size: u64,
        alignment: u32,
        pointer_width: PointerWidth,
    ) -> CompilerGeneratedArgumentLayoutV1 {
        CompilerGeneratedArgumentLayoutV1::new(size, alignment, pointer_width, fields).unwrap()
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

    #[test]
    fn all_scalar_widths_pack_little_endian_and_zero_padding() {
        let fields = vec![
            typed_scalar("i8", 0, 1, ScalarType::I8, 1),
            typed_scalar("u8", 1, 1, ScalarType::U8, 2),
            typed_scalar("i16", 2, 2, ScalarType::I16, 3),
            typed_scalar("u16", 4, 2, ScalarType::U16, 4),
            typed_scalar("f16", 6, 2, ScalarType::F16, 5),
            typed_scalar("i32", 8, 4, ScalarType::I32, 6),
            typed_scalar("u32", 12, 4, ScalarType::U32, 7),
            typed_scalar("f32", 16, 4, ScalarType::F32, 8),
            typed_scalar("i64", 24, 8, ScalarType::I64, 9),
            typed_scalar("u64", 32, 8, ScalarType::U64, 10),
            typed_scalar("f64", 40, 8, ScalarType::F64, 11),
        ];
        let manifest = layout(fields.clone(), 48, 8);
        let plan = validate(&manifest, &generated(fields, 48, 8)).unwrap();
        let inputs = vec![
            plan.scalar_f64(10, -13.5).unwrap(),
            plan.scalar_u64(9, 0x8877_6655_4433_2211).unwrap(),
            plan.scalar_i64(8, -0x0102_0304_0506_0708).unwrap(),
            plan.scalar_f32(7, -2.5).unwrap(),
            plan.scalar_u32(6, 0x8877_6655).unwrap(),
            plan.scalar_i32(5, -0x0102_0304).unwrap(),
            plan.scalar_f16_bits(4, 0x3e00).unwrap(),
            plan.scalar_u16(3, 0x8877).unwrap(),
            plan.scalar_i16(2, -0x0102).unwrap(),
            plan.scalar_u8(1, 0x88).unwrap(),
            plan.scalar_i8(0, -2).unwrap(),
        ];
        let packed = plan.pack(inputs).unwrap();

        let mut expected = [0_u8; 48];
        expected[0] = (-2_i8) as u8;
        expected[1] = 0x88;
        expected[2..4].copy_from_slice(&(-0x0102_i16).to_le_bytes());
        expected[4..6].copy_from_slice(&0x8877_u16.to_le_bytes());
        expected[6..8].copy_from_slice(&0x3e00_u16.to_le_bytes());
        expected[8..12].copy_from_slice(&(-0x0102_0304_i32).to_le_bytes());
        expected[12..16].copy_from_slice(&0x8877_6655_u32.to_le_bytes());
        expected[16..20].copy_from_slice(&(-2.5_f32).to_bits().to_le_bytes());
        expected[24..32].copy_from_slice(&(-0x0102_0304_0506_0708_i64).to_le_bytes());
        expected[32..40].copy_from_slice(&0x8877_6655_4433_2211_u64.to_le_bytes());
        expected[40..48].copy_from_slice(&(-13.5_f64).to_bits().to_le_bytes());

        assert_eq!(packed.kernel_id(), KERNEL_ID);
        assert_eq!(packed.alignment(), 8);
        assert_eq!(packed.len(), 48);
        assert!(!packed.is_empty());
        assert_eq!(packed.bytes(), expected);
        assert_eq!(&packed.bytes()[20..24], &[0; 4]);
    }

    #[test]
    fn slices_pack_pointer_and_length_for_both_widths() {
        let fields = vec![slice("values", 0, 1)];
        let manifest = layout(fields.clone(), 16, 8);
        let plan = validate(&manifest, &generated(fields, 16, 8)).unwrap();
        let pointer = 0x1122_3344_5566_7788_usize as *const ();
        let input = unsafe {
            plan.slice(
                0,
                pointer,
                0x8877_6655_4433_2211,
                PointerWidth::Bits64,
                AddressSpace::Global,
                Access::ReadOnly,
            )
        }
        .unwrap();
        let packed = plan.pack([input]).unwrap();
        assert_eq!(
            packed.bytes(),
            &[
                0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66,
                0x77, 0x88,
            ]
        );

        let field = reference(
            "values",
            0,
            AbiKind::Slice {
                element_size: 4,
                element_alignment: 4,
            },
            Access::ReadOnly,
            AddressSpace::Global,
            1,
        );
        let field = AbiField::new(
            field.name().clone(),
            0,
            8,
            4,
            field.kind(),
            field.mutability(),
            field.access(),
            field.address_space(),
            field.type_identity(),
            field.ownership(),
            field.alias_class(),
        )
        .unwrap();
        let fields = vec![field];
        let manifest = layout_with_width(fields.clone(), 8, 4, PointerWidth::Bits32);
        let generated = generated_with_width(fields, 8, 4, PointerWidth::Bits32);
        let plan = validate(&manifest, &generated).unwrap();
        let input = unsafe {
            plan.slice(
                0,
                0x1122_3344_usize as *const (),
                0x8877_6655,
                PointerWidth::Bits32,
                AddressSpace::Global,
                Access::ReadOnly,
            )
        }
        .unwrap();
        assert_eq!(
            plan.pack([input]).unwrap().bytes(),
            &[0x44, 0x33, 0x22, 0x11, 0x55, 0x66, 0x77, 0x88]
        );
    }

    #[test]
    fn reordered_inputs_are_deterministic_and_padding_never_leaks() {
        let fields = vec![scalar("first", 0, 4, 1), scalar("second", 8, 4, 2)];
        let manifest = layout(fields.clone(), 16, 8);
        let plan = validate(&manifest, &generated(fields, 16, 8)).unwrap();
        let first = plan.scalar_u32(0, 0x1122_3344).unwrap();
        let second = plan.scalar_u32(1, 0x5566_7788).unwrap();

        let forward = plan.pack([first.clone(), second.clone()]).unwrap();
        let reverse = plan.pack([second, first]).unwrap();
        assert_eq!(forward.bytes(), reverse.bytes());
        assert_eq!(&forward.bytes()[4..8], &[0; 4]);
        assert_eq!(&forward.bytes()[12..16], &[0; 4]);
        assert!(!format!("{forward:?}").contains("11223344"));
    }

    #[test]
    fn missing_duplicate_and_out_of_range_inputs_are_rejected() {
        let fields = vec![scalar("first", 0, 4, 1), scalar("second", 4, 4, 2)];
        let manifest = layout(fields.clone(), 8, 4);
        let plan = validate(&manifest, &generated(fields, 8, 4)).unwrap();
        let first = plan.scalar_u32(0, 1).unwrap();
        assert_eq!(
            plan.pack([first.clone()]).unwrap_err(),
            GeneratedArgumentPackError::MissingArgument { argument_index: 1 }
        );
        assert_eq!(
            plan.pack([first.clone(), first]).unwrap_err(),
            GeneratedArgumentPackError::DuplicateArgument { argument_index: 0 }
        );
        assert_eq!(
            plan.scalar_u32(2, 1).unwrap_err(),
            GeneratedArgumentPackError::ArgumentIndexOutOfBounds {
                argument_index: 2,
                argument_count: 2,
            }
        );
    }

    #[test]
    fn kind_scalar_width_and_slice_metadata_must_match() {
        let slice_fields = vec![slice("values", 0, 1)];
        let slice_manifest = layout(slice_fields.clone(), 16, 8);
        let slice_plan =
            validate(&slice_manifest, &generated(slice_fields.clone(), 16, 8)).unwrap();
        assert!(matches!(
            slice_plan.scalar_u32(0, 1),
            Err(GeneratedArgumentPackError::KindMismatch {
                argument_index: 0,
                expected: "slice",
                provided: "scalar",
            })
        ));

        let scalar_fields = vec![scalar("value", 0, 4, 2)];
        let scalar_manifest = layout(scalar_fields.clone(), 4, 4);
        let scalar_plan = validate(&scalar_manifest, &generated(scalar_fields, 4, 4)).unwrap();
        assert!(matches!(
            scalar_plan.scalar_u64(0, 1),
            Err(GeneratedArgumentPackError::ScalarTypeMismatch {
                argument_index: 0,
                expected: ScalarType::U32,
                provided: ScalarType::U64,
            })
        ));

        let pointer = core::ptr::dangling::<()>();
        assert!(matches!(
            unsafe {
                slice_plan.slice(
                    0,
                    pointer,
                    1,
                    PointerWidth::Bits32,
                    AddressSpace::Global,
                    Access::ReadOnly,
                )
            },
            Err(GeneratedArgumentPackError::PointerWidthMismatch { .. })
        ));
        assert!(matches!(
            unsafe {
                slice_plan.slice(
                    0,
                    pointer,
                    1,
                    PointerWidth::Bits64,
                    AddressSpace::Generic,
                    Access::ReadOnly,
                )
            },
            Err(GeneratedArgumentPackError::AddressSpaceMismatch { .. })
        ));
        assert!(matches!(
            unsafe {
                slice_plan.slice(
                    0,
                    pointer,
                    1,
                    PointerWidth::Bits64,
                    AddressSpace::Global,
                    Access::ReadWrite,
                )
            },
            Err(GeneratedArgumentPackError::AccessMismatch { .. })
        ));
    }

    #[test]
    fn field_binding_prevents_relabeling_or_cross_kernel_reuse() {
        let fields = vec![scalar("first", 0, 4, 1), scalar("second", 4, 4, 2)];
        let manifest = layout(fields.clone(), 8, 4);
        let plan = validate(&manifest, &generated(fields.clone(), 8, 4)).unwrap();
        let mut relabeled = plan.scalar_u32(0, 1).unwrap();
        relabeled.argument_index = 1;
        assert_eq!(
            plan.pack([plan.scalar_u32(0, 2).unwrap(), relabeled])
                .unwrap_err(),
            GeneratedArgumentPackError::SourceFieldMismatch { argument_index: 1 }
        );

        let other = super::validate_argument_packing(
            KernelId::from_bytes([10; 32]),
            &manifest,
            &generated(fields, 8, 4),
        )
        .unwrap();
        let foreign = plan.scalar_u32(0, 1).unwrap();
        assert_eq!(
            other.pack([foreign]).unwrap_err(),
            GeneratedArgumentPackError::SourceKernelMismatch { argument_index: 0 }
        );
    }

    #[test]
    fn scalar_component_width_is_defensively_rechecked() {
        let fields = vec![scalar("value", 0, 4, 1)];
        let manifest = layout(fields.clone(), 4, 4);
        let plan = validate(&manifest, &generated(fields, 4, 4)).unwrap();
        let mut input = plan.scalar_u32(0, 1).unwrap();
        let GeneratedArgumentValueV1::Scalar { byte_length, .. } = &mut input.value else {
            unreachable!();
        };
        *byte_length = 2;
        assert!(matches!(
            plan.pack([input]),
            Err(GeneratedArgumentPackError::ComponentWidthMismatch {
                argument_index: 0,
                component: GeneratedPackingComponentKindV1::Scalar,
                expected: 4,
                provided: 2,
            })
        ));
    }

    #[test]
    fn narrow_slice_values_and_nonempty_null_are_rejected() {
        let field = AbiField::new(
            Name::new("values").unwrap(),
            0,
            8,
            4,
            AbiKind::Slice {
                element_size: 4,
                element_alignment: 4,
            },
            Mutability::Immutable,
            Access::ReadOnly,
            AddressSpace::Global,
            identity(1),
            ArgumentOwnership::SharedBorrow,
            AliasClass::SharedReadOnly,
        )
        .unwrap();
        let fields = vec![field];
        let manifest = layout_with_width(fields.clone(), 8, 4, PointerWidth::Bits32);
        let generated = generated_with_width(fields, 8, 4, PointerWidth::Bits32);
        let plan = validate(&manifest, &generated).unwrap();

        assert!(matches!(
            unsafe {
                plan.slice(
                    0,
                    (u64::from(u32::MAX) + 1) as usize as *const (),
                    1,
                    PointerWidth::Bits32,
                    AddressSpace::Global,
                    Access::ReadOnly,
                )
            },
            Err(GeneratedArgumentPackError::IntegerWidthOverflow {
                component: GeneratedPackingComponentKindV1::SlicePointer,
                ..
            })
        ));
        assert!(matches!(
            unsafe {
                plan.slice(
                    0,
                    core::ptr::dangling::<()>(),
                    u64::from(u32::MAX) + 1,
                    PointerWidth::Bits32,
                    AddressSpace::Global,
                    Access::ReadOnly,
                )
            },
            Err(GeneratedArgumentPackError::IntegerWidthOverflow {
                component: GeneratedPackingComponentKindV1::SliceLength,
                ..
            })
        ));
        assert_eq!(
            unsafe {
                plan.slice(
                    0,
                    core::ptr::null(),
                    1,
                    PointerWidth::Bits32,
                    AddressSpace::Global,
                    Access::ReadOnly,
                )
            }
            .unwrap_err(),
            GeneratedArgumentPackError::NullSlicePointer {
                argument_index: 0,
                length: 1,
            }
        );
        let empty = unsafe {
            plan.slice(
                0,
                core::ptr::null(),
                0,
                PointerWidth::Bits32,
                AddressSpace::Global,
                Access::ReadOnly,
            )
        }
        .unwrap();
        assert_eq!(plan.pack([empty]).unwrap().bytes(), &[0; 8]);
    }

    #[test]
    fn standalone_pointers_and_oversized_forged_plans_are_rejected() {
        let field = pointer("value", 0, Access::ReadOnly, AddressSpace::Global, 1);
        let manifest = layout(vec![field.clone()], 8, 8);
        let plan = validate(&manifest, &generated(vec![field], 8, 8)).unwrap();
        assert_eq!(
            plan.pack([]).unwrap_err(),
            GeneratedArgumentPackError::UnsupportedPointerArgument { argument_index: 0 }
        );

        let oversized = super::GeneratedArgumentPackingPlanV1 {
            kernel_id: KERNEL_ID,
            kernarg_size: fe2o3_artifacts::MAX_ABI_BYTES + 1,
            kernarg_alignment: 1,
            pointer_width: PointerWidth::Bits64,
            fields: Box::new([]),
            components: Box::new([]),
        };
        assert_eq!(
            oversized.pack([]).unwrap_err(),
            GeneratedArgumentPackError::KernargTooLarge {
                size: fe2o3_artifacts::MAX_ABI_BYTES + 1,
                maximum: fe2o3_artifacts::MAX_ABI_BYTES,
            }
        );
    }
}
