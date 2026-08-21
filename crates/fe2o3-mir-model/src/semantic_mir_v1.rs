//! Bounded, rustc-independent semantic MIR records.
//!
//! This module is an inert data-model foundation. It retains caller-asserted
//! identities and normalized semantic records, but it does not authenticate a
//! compiler session or grant proof, compiler, artifact, publication, load, or
//! launch authority. In particular, its SHA-256 value identifies only the
//! versioned bytes produced here.
//!
//! This request-wide schema is the intended successor to the isolated type
//! qualification records in `semantic_type` and `semantic_type_v2`. Those
//! modules remain compatibility inputs; they are not independent production
//! authorities alongside this module.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

use sha2::{Digest, Sha256};

const MAGIC: &[u8] = b"fe2o3.inert-semantic-mir";
pub const INERT_SEMANTIC_MIR_VERSION_V1: u16 = 1;

pub const HARD_MAX_TYPES_V1: u64 = 16_384;
pub const HARD_MAX_FUNCTIONS_V1: u64 = 4_096;
pub const HARD_MAX_CALLABLES_V1: u64 = 8_192;
pub const HARD_MAX_ALLOCATIONS_V1: u64 = 4_096;
pub const HARD_MAX_STATICS_V1: u64 = 4_096;
pub const HARD_MAX_VTABLES_V1: u64 = 4_096;
pub const HARD_MAX_ROOTS_V1: u64 = 4_096;
pub const HARD_MAX_LOCALS_V1: u64 = 262_144;
pub const HARD_MAX_BLOCKS_V1: u64 = 262_144;
pub const HARD_MAX_STATEMENTS_V1: u64 = 1_048_576;
pub const HARD_MAX_PROJECTIONS_V1: u64 = 1_048_576;
pub const HARD_MAX_OPERANDS_V1: u64 = 1_048_576;
pub const HARD_MAX_CALL_ARGUMENTS_V1: u64 = 65_536;
pub const HARD_MAX_SWITCH_TARGETS_V1: u64 = 65_536;
pub const HARD_MAX_RELOCATIONS_V1: u64 = 262_144;
pub const HARD_MAX_CONSTANT_BYTES_V1: u64 = 64 * 1024 * 1024;
pub const HARD_MAX_LINK_SYMBOL_BYTES_V1: u64 = 1024 * 1024;
pub const HARD_MAX_CANONICAL_BYTES_V1: u64 = 128 * 1024 * 1024;
pub const HARD_MAX_VALIDATION_WORK_V1: u64 = 16_000_000;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticMirResourceV1 {
    Types,
    Functions,
    Callables,
    Allocations,
    Statics,
    VTables,
    Roots,
    Locals,
    Blocks,
    Statements,
    Projections,
    Operands,
    CallArguments,
    SwitchTargets,
    Relocations,
    ConstantBytes,
    LinkSymbolBytes,
    CanonicalBytes,
    ValidationWork,
}

impl SemanticMirResourceV1 {
    const fn hard_max(self) -> u64 {
        match self {
            Self::Types => HARD_MAX_TYPES_V1,
            Self::Functions => HARD_MAX_FUNCTIONS_V1,
            Self::Callables => HARD_MAX_CALLABLES_V1,
            Self::Allocations => HARD_MAX_ALLOCATIONS_V1,
            Self::Statics => HARD_MAX_STATICS_V1,
            Self::VTables => HARD_MAX_VTABLES_V1,
            Self::Roots => HARD_MAX_ROOTS_V1,
            Self::Locals => HARD_MAX_LOCALS_V1,
            Self::Blocks => HARD_MAX_BLOCKS_V1,
            Self::Statements => HARD_MAX_STATEMENTS_V1,
            Self::Projections => HARD_MAX_PROJECTIONS_V1,
            Self::Operands => HARD_MAX_OPERANDS_V1,
            Self::CallArguments => HARD_MAX_CALL_ARGUMENTS_V1,
            Self::SwitchTargets => HARD_MAX_SWITCH_TARGETS_V1,
            Self::Relocations => HARD_MAX_RELOCATIONS_V1,
            Self::ConstantBytes => HARD_MAX_CONSTANT_BYTES_V1,
            Self::LinkSymbolBytes => HARD_MAX_LINK_SYMBOL_BYTES_V1,
            Self::CanonicalBytes => HARD_MAX_CANONICAL_BYTES_V1,
            Self::ValidationWork => HARD_MAX_VALIDATION_WORK_V1,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticMirLimitsV1 {
    types: u64,
    functions: u64,
    callables: u64,
    allocations: u64,
    statics: u64,
    vtables: u64,
    roots: u64,
    locals: u64,
    blocks: u64,
    statements: u64,
    projections: u64,
    operands: u64,
    call_arguments: u64,
    switch_targets: u64,
    relocations: u64,
    constant_bytes: u64,
    link_symbol_bytes: u64,
    canonical_bytes: u64,
    validation_work: u64,
}

impl Default for SemanticMirLimitsV1 {
    fn default() -> Self {
        Self {
            types: HARD_MAX_TYPES_V1,
            functions: HARD_MAX_FUNCTIONS_V1,
            callables: HARD_MAX_CALLABLES_V1,
            allocations: HARD_MAX_ALLOCATIONS_V1,
            statics: HARD_MAX_STATICS_V1,
            vtables: HARD_MAX_VTABLES_V1,
            roots: HARD_MAX_ROOTS_V1,
            locals: HARD_MAX_LOCALS_V1,
            blocks: HARD_MAX_BLOCKS_V1,
            statements: HARD_MAX_STATEMENTS_V1,
            projections: HARD_MAX_PROJECTIONS_V1,
            operands: HARD_MAX_OPERANDS_V1,
            call_arguments: HARD_MAX_CALL_ARGUMENTS_V1,
            switch_targets: HARD_MAX_SWITCH_TARGETS_V1,
            relocations: HARD_MAX_RELOCATIONS_V1,
            constant_bytes: HARD_MAX_CONSTANT_BYTES_V1,
            link_symbol_bytes: HARD_MAX_LINK_SYMBOL_BYTES_V1,
            canonical_bytes: HARD_MAX_CANONICAL_BYTES_V1,
            validation_work: HARD_MAX_VALIDATION_WORK_V1,
        }
    }
}

impl SemanticMirLimitsV1 {
    pub fn with_limit(
        mut self,
        resource: SemanticMirResourceV1,
        requested: u64,
    ) -> Result<Self, SemanticMirErrorV1> {
        let hard_max = resource.hard_max();
        if requested > hard_max {
            return Err(SemanticMirErrorV1::LimitExceeded {
                resource,
                actual: requested,
                max: hard_max,
            });
        }
        *self.limit_mut(resource) = requested;
        Ok(self)
    }

    pub const fn limit(self, resource: SemanticMirResourceV1) -> u64 {
        match resource {
            SemanticMirResourceV1::Types => self.types,
            SemanticMirResourceV1::Functions => self.functions,
            SemanticMirResourceV1::Callables => self.callables,
            SemanticMirResourceV1::Allocations => self.allocations,
            SemanticMirResourceV1::Statics => self.statics,
            SemanticMirResourceV1::VTables => self.vtables,
            SemanticMirResourceV1::Roots => self.roots,
            SemanticMirResourceV1::Locals => self.locals,
            SemanticMirResourceV1::Blocks => self.blocks,
            SemanticMirResourceV1::Statements => self.statements,
            SemanticMirResourceV1::Projections => self.projections,
            SemanticMirResourceV1::Operands => self.operands,
            SemanticMirResourceV1::CallArguments => self.call_arguments,
            SemanticMirResourceV1::SwitchTargets => self.switch_targets,
            SemanticMirResourceV1::Relocations => self.relocations,
            SemanticMirResourceV1::ConstantBytes => self.constant_bytes,
            SemanticMirResourceV1::LinkSymbolBytes => self.link_symbol_bytes,
            SemanticMirResourceV1::CanonicalBytes => self.canonical_bytes,
            SemanticMirResourceV1::ValidationWork => self.validation_work,
        }
    }

    fn limit_mut(&mut self, resource: SemanticMirResourceV1) -> &mut u64 {
        match resource {
            SemanticMirResourceV1::Types => &mut self.types,
            SemanticMirResourceV1::Functions => &mut self.functions,
            SemanticMirResourceV1::Callables => &mut self.callables,
            SemanticMirResourceV1::Allocations => &mut self.allocations,
            SemanticMirResourceV1::Statics => &mut self.statics,
            SemanticMirResourceV1::VTables => &mut self.vtables,
            SemanticMirResourceV1::Roots => &mut self.roots,
            SemanticMirResourceV1::Locals => &mut self.locals,
            SemanticMirResourceV1::Blocks => &mut self.blocks,
            SemanticMirResourceV1::Statements => &mut self.statements,
            SemanticMirResourceV1::Projections => &mut self.projections,
            SemanticMirResourceV1::Operands => &mut self.operands,
            SemanticMirResourceV1::CallArguments => &mut self.call_arguments,
            SemanticMirResourceV1::SwitchTargets => &mut self.switch_targets,
            SemanticMirResourceV1::Relocations => &mut self.relocations,
            SemanticMirResourceV1::ConstantBytes => &mut self.constant_bytes,
            SemanticMirResourceV1::LinkSymbolBytes => &mut self.link_symbol_bytes,
            SemanticMirResourceV1::CanonicalBytes => &mut self.canonical_bytes,
            SemanticMirResourceV1::ValidationWork => &mut self.validation_work,
        }
    }
}

macro_rules! index_id {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(u32);

        impl $name {
            pub const fn from_index(index: u32) -> Self {
                Self(index)
            }

            pub const fn index(self) -> u32 {
                self.0
            }
        }
    };
}

index_id!(SemanticTypeIdV1);
index_id!(SemanticFunctionIdV1);
index_id!(SemanticCallableIdV1);
index_id!(SemanticAllocationIdV1);
index_id!(SemanticStaticIdV1);
index_id!(SemanticVTableIdV1);
index_id!(SemanticLocalIdV1);
index_id!(SemanticBlockIdV1);

macro_rules! digest_identity {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name([u8; 32]);

        impl $name {
            /// Wraps a caller-asserted digest. This does not authenticate it.
            pub const fn from_sha256(bytes: [u8; 32]) -> Self {
                Self(bytes)
            }

            pub const fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }
        }
    };
}

digest_identity!(SemanticTypeIdentityV1);
digest_identity!(SemanticFunctionIdentityV1);
digest_identity!(SemanticItemDefinitionIdentityV1);
digest_identity!(SemanticMonomorphizationIdentityV1);
digest_identity!(SemanticGenericTypeArgumentsIdentityV1);
digest_identity!(SemanticConstGenericArgumentsIdentityV1);
digest_identity!(SemanticAllocationIdentityV1);
digest_identity!(SemanticLocalIdentityV1);
digest_identity!(SemanticBlockIdentityV1);
digest_identity!(SemanticSourceFileIdentityV1);
digest_identity!(SemanticAbiIdentityV1);
digest_identity!(SemanticLayoutIdentityV1);
digest_identity!(SemanticStaticIdentityV1);
digest_identity!(SemanticVTableIdentityV1);
digest_identity!(SemanticDynPredicateIdentityV1);
digest_identity!(SemanticTraitRefIdentityV1);
digest_identity!(SemanticKernelBindingIdentityV1);
digest_identity!(SemanticDeviceFfiContractIdentityV1);
digest_identity!(SemanticDeviceFfiPhysicalAbiIdentityV1);
digest_identity!(SemanticDeviceFfiSemanticIdentityV1);
digest_identity!(SemanticCompilerIntrinsicIdentityV1);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticTargetArchitectureV1 {
    AmdGpuGfx942,
}

/// Inspectable target facts that constrain rustc layout and calling-convention records.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticTargetDataLayoutV1 {
    identity: SemanticLayoutIdentityV1,
    architecture: SemanticTargetArchitectureV1,
    object_size_bound_bytes: u64,
}

impl SemanticTargetDataLayoutV1 {
    pub const fn gfx942(identity: SemanticLayoutIdentityV1) -> Self {
        Self {
            identity,
            architecture: SemanticTargetArchitectureV1::AmdGpuGfx942,
            object_size_bound_bytes: 1 << 61,
        }
    }

    pub const fn identity(self) -> SemanticLayoutIdentityV1 {
        self.identity
    }

    pub const fn architecture(self) -> SemanticTargetArchitectureV1 {
        self.architecture
    }

    pub const fn object_size_bound_bytes(self) -> u64 {
        self.object_size_bound_bytes
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticSourceOriginV1 {
    file: SemanticSourceFileIdentityV1,
    byte_start: u64,
    byte_end: u64,
    line_start: u32,
    column_start: u32,
    line_end: u32,
    column_end: u32,
}

impl SemanticSourceOriginV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        file: SemanticSourceFileIdentityV1,
        byte_start: u64,
        byte_end: u64,
        line_start: u32,
        column_start: u32,
        line_end: u32,
        column_end: u32,
    ) -> Result<Self, SemanticMirErrorV1> {
        if byte_start > byte_end
            || line_start == 0
            || line_end == 0
            || (line_start, column_start) > (line_end, column_end)
        {
            return Err(SemanticMirErrorV1::InvalidSourceOrigin);
        }
        Ok(Self {
            file,
            byte_start,
            byte_end,
            line_start,
            column_start,
            line_end,
            column_end,
        })
    }

    pub const fn file(&self) -> SemanticSourceFileIdentityV1 {
        self.file
    }

    pub const fn byte_range(&self) -> (u64, u64) {
        (self.byte_start, self.byte_end)
    }

    pub const fn start_coordinate(&self) -> (u32, u32) {
        (self.line_start, self.column_start)
    }

    pub const fn end_coordinate(&self) -> (u32, u32) {
        (self.line_end, self.column_end)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticSourceProvenanceV1 {
    expansion: Option<SemanticSourceOriginV1>,
    call_site: Option<SemanticSourceOriginV1>,
}

impl SemanticSourceProvenanceV1 {
    /// Retains independent expansion and source-callsite coordinates.
    pub const fn new(
        expansion: Option<SemanticSourceOriginV1>,
        call_site: Option<SemanticSourceOriginV1>,
    ) -> Self {
        Self {
            expansion,
            call_site,
        }
    }

    /// Represents a rustc record for which neither span is available.
    pub const fn unavailable() -> Self {
        Self::new(None, None)
    }

    pub const fn expansion(&self) -> Option<SemanticSourceOriginV1> {
        self.expansion
    }

    pub const fn call_site(&self) -> Option<SemanticSourceOriginV1> {
        self.call_site
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticRustcVariantsV1 {
    Empty,
    Single { index: u32 },
    Multiple(Box<SemanticEnumLayoutV1>),
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticTypeLayoutV1 {
    rustc_size_bytes: u64,
    size_bytes: Option<u64>,
    alignment_bytes: u64,
    fields: SemanticFieldsShapeV1,
    variants: SemanticRustcVariantsV1,
    backend_repr: SemanticBackendReprV1,
    largest_niche: Option<SemanticLayoutNicheV1>,
    uninhabited: bool,
    max_repr_alignment_bytes: Option<u64>,
    unadjusted_abi_alignment_bytes: u64,
    randomization_seed: u64,
    details: SemanticTypeLayoutDetailsV1,
}

impl SemanticTypeLayoutV1 {
    pub fn new(size_bytes: Option<u64>, alignment_bytes: u64) -> Result<Self, SemanticMirErrorV1> {
        Self::with_details(
            size_bytes,
            alignment_bytes,
            SemanticBackendReprV1::memory(size_bytes.is_some()),
            false,
            SemanticTypeLayoutDetailsV1::None,
        )
    }

    /// Builds a layout from normalized rustc layout facts.
    pub fn new_with_backend_repr(
        size_bytes: Option<u64>,
        alignment_bytes: u64,
        backend_repr: SemanticBackendReprV1,
        uninhabited: bool,
    ) -> Result<Self, SemanticMirErrorV1> {
        Self::with_details(
            size_bytes,
            alignment_bytes,
            backend_repr,
            uninhabited,
            SemanticTypeLayoutDetailsV1::None,
        )
    }

    pub fn aggregate(
        size_bytes: Option<u64>,
        alignment_bytes: u64,
        aggregate: SemanticAggregateLayoutV1,
    ) -> Result<Self, SemanticMirErrorV1> {
        Self::with_details(
            size_bytes,
            alignment_bytes,
            SemanticBackendReprV1::memory(size_bytes.is_some()),
            false,
            SemanticTypeLayoutDetailsV1::Aggregate(aggregate),
        )
    }

    pub fn aggregate_with_backend_repr(
        size_bytes: Option<u64>,
        alignment_bytes: u64,
        backend_repr: SemanticBackendReprV1,
        uninhabited: bool,
        aggregate: SemanticAggregateLayoutV1,
    ) -> Result<Self, SemanticMirErrorV1> {
        Self::with_details(
            size_bytes,
            alignment_bytes,
            backend_repr,
            uninhabited,
            SemanticTypeLayoutDetailsV1::Aggregate(aggregate),
        )
    }

    pub fn enum_layout(
        size_bytes: u64,
        alignment_bytes: u64,
        enum_layout: SemanticEnumLayoutV1,
    ) -> Result<Self, SemanticMirErrorV1> {
        Self::enum_layout_with_backend_repr(
            size_bytes,
            alignment_bytes,
            SemanticBackendReprV1::memory(true),
            false,
            enum_layout,
        )
    }

    pub fn enum_layout_with_backend_repr(
        size_bytes: u64,
        alignment_bytes: u64,
        backend_repr: SemanticBackendReprV1,
        uninhabited: bool,
        enum_layout: SemanticEnumLayoutV1,
    ) -> Result<Self, SemanticMirErrorV1> {
        let largest_niche = match &enum_layout.encoding {
            SemanticEnumEncodingV1::Direct(direct) => match direct.tag {
                SemanticBackendScalarV1::Initialized {
                    primitive,
                    valid_range,
                } => layout_niche_from_scalar(direct.tag_offset_bytes, primitive, valid_range)?,
                SemanticBackendScalarV1::Union { .. } => None,
            },
            SemanticEnumEncodingV1::Niche(niche) => {
                let SemanticBackendScalarV1::Initialized {
                    primitive,
                    valid_range,
                } = niche.tag
                else {
                    return Err(SemanticMirErrorV1::InvalidTypeLayout);
                };
                layout_niche_from_scalar(
                    niche.source.expected_offset_bytes,
                    primitive,
                    valid_range,
                )?
            }
        };
        Self::with_exact_rustc_layout(
            size_bytes,
            alignment_bytes,
            enum_outer_fields(&enum_layout)?,
            SemanticRustcVariantsV1::Multiple(Box::new(enum_layout)),
            backend_repr,
            largest_niche,
            uninhabited,
            None,
            alignment_bytes,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
    }

    fn with_details(
        size_bytes: Option<u64>,
        alignment_bytes: u64,
        backend_repr: SemanticBackendReprV1,
        uninhabited: bool,
        details: SemanticTypeLayoutDetailsV1,
    ) -> Result<Self, SemanticMirErrorV1> {
        if size_bytes.is_none()
            != matches!(backend_repr, SemanticBackendReprV1::Memory { sized: false })
        {
            return Err(SemanticMirErrorV1::InvalidTypeLayout);
        }
        let (fields, largest_niche) = match &details {
            SemanticTypeLayoutDetailsV1::Aggregate(aggregate) => (
                SemanticFieldsShapeV1::arbitrary(
                    aggregate.field_offsets.to_vec(),
                    memory_order_for_offsets(&aggregate.field_offsets)?,
                )?,
                None,
            ),
            SemanticTypeLayoutDetailsV1::None => backend_default_fields_and_niche(backend_repr)?,
        };
        let variants = SemanticRustcVariantsV1::Single { index: 0 };
        let unadjusted_abi_alignment_bytes = match backend_repr {
            SemanticBackendReprV1::SimdVector { element, .. }
            | SemanticBackendReprV1::SimdScalableVector { element, .. } => {
                element.primitive().alignment_bytes()
            }
            SemanticBackendReprV1::Memory { .. }
            | SemanticBackendReprV1::Scalar(_)
            | SemanticBackendReprV1::ScalarPair { .. } => alignment_bytes,
        };
        Self::with_exact_rustc_layout(
            size_bytes.unwrap_or(0),
            alignment_bytes,
            fields,
            variants,
            backend_repr,
            largest_niche,
            uninhabited,
            None,
            unadjusted_abi_alignment_bytes,
            backend_default_randomization_seed(backend_repr)?,
            details,
        )
    }

    /// Builds a layout from every durable field of pinned rustc `LayoutData`.
    #[allow(clippy::too_many_arguments)]
    pub fn with_exact_rustc_layout(
        rustc_size_bytes: u64,
        alignment_bytes: u64,
        fields: SemanticFieldsShapeV1,
        variants: SemanticRustcVariantsV1,
        backend_repr: SemanticBackendReprV1,
        largest_niche: Option<SemanticLayoutNicheV1>,
        uninhabited: bool,
        max_repr_alignment_bytes: Option<u64>,
        unadjusted_abi_alignment_bytes: u64,
        randomization_seed: u64,
        details: SemanticTypeLayoutDetailsV1,
    ) -> Result<Self, SemanticMirErrorV1> {
        let size_bytes = match backend_repr {
            SemanticBackendReprV1::Memory { sized: false } => None,
            _ => Some(rustc_size_bytes),
        };
        if !valid_rustc_alignment(alignment_bytes)
            || (!rustc_size_bytes.is_multiple_of(alignment_bytes)
                && !backend_repr_is_overaligned_pointer(backend_repr, rustc_size_bytes))
            || max_repr_alignment_bytes.is_some_and(|alignment| !valid_rustc_alignment(alignment))
            || !valid_rustc_alignment(unadjusted_abi_alignment_bytes)
        {
            return Err(SemanticMirErrorV1::InvalidTypeLayout);
        }
        validate_fields_shape(&fields, Some(rustc_size_bytes))?;
        enforce_rustc_variants_shape(&variants, uninhabited)?;
        validate_backend_repr(size_bytes, alignment_bytes, &backend_repr)?;
        if let Some(niche) = largest_niche {
            validate_layout_niche(niche, Some(rustc_size_bytes))?;
        }
        Ok(Self {
            rustc_size_bytes,
            size_bytes,
            alignment_bytes,
            fields,
            variants,
            backend_repr,
            largest_niche,
            uninhabited,
            max_repr_alignment_bytes,
            unadjusted_abi_alignment_bytes,
            randomization_seed,
            details,
        })
    }

    /// Returns rustc's retained lower-bound size, including for unsized layouts.
    pub const fn rustc_size_bytes(&self) -> u64 {
        self.rustc_size_bytes
    }

    pub const fn size_bytes(&self) -> Option<u64> {
        self.size_bytes
    }

    pub const fn alignment_bytes(&self) -> u64 {
        self.alignment_bytes
    }

    pub const fn fields(&self) -> &SemanticFieldsShapeV1 {
        &self.fields
    }

    pub const fn variants(&self) -> &SemanticRustcVariantsV1 {
        &self.variants
    }

    pub const fn backend_repr(&self) -> &SemanticBackendReprV1 {
        &self.backend_repr
    }

    pub const fn largest_niche(&self) -> Option<SemanticLayoutNicheV1> {
        self.largest_niche
    }

    pub const fn is_uninhabited(&self) -> bool {
        self.uninhabited
    }

    pub const fn max_repr_alignment_bytes(&self) -> Option<u64> {
        self.max_repr_alignment_bytes
    }

    pub const fn unadjusted_abi_alignment_bytes(&self) -> u64 {
        self.unadjusted_abi_alignment_bytes
    }

    pub const fn randomization_seed(&self) -> u64 {
        self.randomization_seed
    }

    pub const fn details(&self) -> &SemanticTypeLayoutDetailsV1 {
        &self.details
    }
}

/// A normalized rustc `Primitive` with its target-resolved ABI alignment.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticBackendPrimitiveV1 {
    Integer {
        signed: bool,
        bits: u16,
        alignment_bytes: u64,
    },
    Float {
        bits: u16,
        alignment_bytes: u64,
    },
    Pointer {
        address_space: u32,
        size_bytes: u64,
        alignment_bytes: u64,
    },
}

impl SemanticBackendPrimitiveV1 {
    pub const fn integer(signed: bool, bits: u16, alignment_bytes: u64) -> Self {
        Self::Integer {
            signed,
            bits,
            alignment_bytes,
        }
    }

    pub const fn float(bits: u16, alignment_bytes: u64) -> Self {
        Self::Float {
            bits,
            alignment_bytes,
        }
    }

    pub const fn pointer(address_space: u32, size_bytes: u64, alignment_bytes: u64) -> Self {
        Self::Pointer {
            address_space,
            size_bytes,
            alignment_bytes,
        }
    }

    pub const fn size_bytes(self) -> Option<u64> {
        match self {
            Self::Integer { bits, .. } if matches!(bits, 8 | 16 | 32 | 64 | 128) => {
                Some((bits / 8) as u64)
            }
            Self::Float { bits, .. } if matches!(bits, 16 | 32 | 64 | 128) => {
                Some((bits / 8) as u64)
            }
            Self::Pointer { size_bytes, .. } if size_bytes > 0 => Some(size_bytes),
            Self::Integer { .. } | Self::Float { .. } | Self::Pointer { .. } => None,
        }
    }

    pub const fn alignment_bytes(self) -> u64 {
        match self {
            Self::Integer {
                alignment_bytes, ..
            }
            | Self::Float {
                alignment_bytes, ..
            }
            | Self::Pointer {
                alignment_bytes, ..
            } => alignment_bytes,
        }
    }

    const fn bits(self) -> Option<u16> {
        match self {
            Self::Integer { bits, .. } | Self::Float { bits, .. } => Some(bits),
            Self::Pointer { size_bytes, .. } => {
                let bits = size_bytes.checked_mul(8);
                match bits {
                    Some(bits) if bits <= u16::MAX as u64 => Some(bits as u16),
                    _ => None,
                }
            }
        }
    }
}

/// A normalized rustc `Scalar`, including wrapping validity or union undef.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticBackendScalarV1 {
    Initialized {
        primitive: SemanticBackendPrimitiveV1,
        valid_range: SemanticScalarValidityRangeV1,
    },
    Union {
        primitive: SemanticBackendPrimitiveV1,
    },
}

impl SemanticBackendScalarV1 {
    pub const fn initialized(
        primitive: SemanticBackendPrimitiveV1,
        valid_range: SemanticScalarValidityRangeV1,
    ) -> Self {
        Self::Initialized {
            primitive,
            valid_range,
        }
    }

    pub const fn union(primitive: SemanticBackendPrimitiveV1) -> Self {
        Self::Union { primitive }
    }

    pub const fn primitive(self) -> SemanticBackendPrimitiveV1 {
        match self {
            Self::Initialized { primitive, .. } | Self::Union { primitive } => primitive,
        }
    }

    pub const fn valid_range(self) -> Option<SemanticScalarValidityRangeV1> {
        match self {
            Self::Initialized { valid_range, .. } => Some(valid_range),
            Self::Union { .. } => None,
        }
    }
}

/// Mirrors the pinned rustc `BackendRepr` without importing rustc types.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticBackendReprV1 {
    Memory {
        sized: bool,
    },
    Scalar(SemanticBackendScalarV1),
    ScalarPair {
        first: SemanticBackendScalarV1,
        second: SemanticBackendScalarV1,
    },
    SimdVector {
        element: SemanticBackendScalarV1,
        count: u64,
    },
    SimdScalableVector {
        element: SemanticBackendScalarV1,
        count: u64,
    },
}

impl SemanticBackendReprV1 {
    pub const fn memory(sized: bool) -> Self {
        Self::Memory { sized }
    }

    pub const fn scalar(scalar: SemanticBackendScalarV1) -> Self {
        Self::Scalar(scalar)
    }

    pub const fn scalar_pair(
        first: SemanticBackendScalarV1,
        second: SemanticBackendScalarV1,
    ) -> Self {
        Self::ScalarPair { first, second }
    }

    pub const fn simd_vector(element: SemanticBackendScalarV1, count: u64) -> Self {
        Self::SimdVector { element, count }
    }

    pub const fn simd_scalable_vector(element: SemanticBackendScalarV1, count: u64) -> Self {
        Self::SimdScalableVector { element, count }
    }
}

/// Exact rustc `FieldsShape` facts, including both source and memory order.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticFieldsShapeV1 {
    Primitive,
    Union {
        field_count: u64,
    },
    Array {
        stride_bytes: u64,
        count: u64,
    },
    Arbitrary {
        source_order_offsets_bytes: Box<[u64]>,
        memory_order_source_indices: Box<[u32]>,
    },
}

impl SemanticFieldsShapeV1 {
    pub fn union(field_count: u64) -> Result<Self, SemanticMirErrorV1> {
        if field_count == 0 {
            return Err(SemanticMirErrorV1::InvalidTypeLayout);
        }
        enforce_hard(
            SemanticMirResourceV1::Types,
            usize::try_from(field_count).map_err(|_| SemanticMirErrorV1::InvalidTypeLayout)?,
        )?;
        Ok(Self::Union { field_count })
    }

    pub const fn array(stride_bytes: u64, count: u64) -> Self {
        Self::Array {
            stride_bytes,
            count,
        }
    }

    pub fn arbitrary(
        source_order_offsets_bytes: Vec<u64>,
        memory_order_source_indices: Vec<u32>,
    ) -> Result<Self, SemanticMirErrorV1> {
        enforce_hard(
            SemanticMirResourceV1::Types,
            source_order_offsets_bytes.len(),
        )?;
        enforce_hard(
            SemanticMirResourceV1::Types,
            memory_order_source_indices.len(),
        )?;
        let fields = Self::Arbitrary {
            source_order_offsets_bytes: source_order_offsets_bytes.into_boxed_slice(),
            memory_order_source_indices: memory_order_source_indices.into_boxed_slice(),
        };
        validate_fields_shape(&fields, None)?;
        Ok(fields)
    }

    pub const fn field_count(&self) -> u64 {
        match self {
            Self::Primitive => 0,
            Self::Union { field_count } => *field_count,
            Self::Array { count, .. } => *count,
            Self::Arbitrary {
                source_order_offsets_bytes,
                ..
            } => source_order_offsets_bytes.len() as u64,
        }
    }

    pub fn source_order_offsets_bytes(&self) -> Option<&[u64]> {
        match self {
            Self::Arbitrary {
                source_order_offsets_bytes,
                ..
            } => Some(source_order_offsets_bytes),
            Self::Primitive | Self::Union { .. } | Self::Array { .. } => None,
        }
    }

    pub fn memory_order_source_indices(&self) -> Option<&[u32]> {
        match self {
            Self::Arbitrary {
                memory_order_source_indices,
                ..
            } => Some(memory_order_source_indices),
            Self::Primitive | Self::Union { .. } | Self::Array { .. } => None,
        }
    }
}

/// Exact rustc `Niche` fact retained independently from enum tag encoding.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticLayoutNicheV1 {
    offset_bytes: u64,
    primitive: SemanticBackendPrimitiveV1,
    valid_range: SemanticScalarValidityRangeV1,
}

impl SemanticLayoutNicheV1 {
    pub fn new(
        offset_bytes: u64,
        primitive: SemanticBackendPrimitiveV1,
        valid_range: SemanticScalarValidityRangeV1,
    ) -> Result<Self, SemanticMirErrorV1> {
        let niche = Self {
            offset_bytes,
            primitive,
            valid_range,
        };
        validate_layout_niche(niche, None)?;
        Ok(niche)
    }

    pub const fn offset_bytes(self) -> u64 {
        self.offset_bytes
    }

    pub const fn primitive(self) -> SemanticBackendPrimitiveV1 {
        self.primitive
    }

    pub const fn valid_range(self) -> SemanticScalarValidityRangeV1 {
        self.valid_range
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticPaddingV1 {
    offset_bytes: u64,
    size_bytes: u64,
}

impl SemanticPaddingV1 {
    pub fn new(offset_bytes: u64, size_bytes: u64) -> Result<Self, SemanticMirErrorV1> {
        if size_bytes == 0 || offset_bytes.checked_add(size_bytes).is_none() {
            return Err(SemanticMirErrorV1::InvalidTypeLayout);
        }
        Ok(Self {
            offset_bytes,
            size_bytes,
        })
    }

    pub const fn offset_bytes(self) -> u64 {
        self.offset_bytes
    }

    pub const fn size_bytes(self) -> u64 {
        self.size_bytes
    }
}

/// Semantic side evidence for field offsets and bytes proven safe to discard.
///
/// rustc's `LayoutData` does not classify every apparent field gap as padding,
/// so admission checks only the explicit padding ranges supplied by the importer.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticAggregateLayoutV1 {
    field_offsets: Box<[u64]>,
    padding: Box<[SemanticPaddingV1]>,
}

impl SemanticAggregateLayoutV1 {
    pub fn new(
        field_offsets: Vec<u64>,
        padding: Vec<SemanticPaddingV1>,
    ) -> Result<Self, SemanticMirErrorV1> {
        enforce_hard(SemanticMirResourceV1::Types, field_offsets.len())?;
        enforce_hard(SemanticMirResourceV1::Types, padding.len())?;
        Ok(Self {
            field_offsets: field_offsets.into_boxed_slice(),
            padding: padding.into_boxed_slice(),
        })
    }

    pub fn field_offsets(&self) -> &[u64] {
        &self.field_offsets
    }

    pub fn padding(&self) -> &[SemanticPaddingV1] {
        &self.padding
    }
}

/// Exact pinned-rustc `LayoutData` retained for one multi-variant layout.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticEnumVariantLayoutV1 {
    variant_index: u32,
    rustc_size_bytes: u64,
    alignment_bytes: u64,
    fields: SemanticFieldsShapeV1,
    backend_repr: SemanticBackendReprV1,
    largest_niche: Option<SemanticLayoutNicheV1>,
    uninhabited: bool,
    max_repr_alignment_bytes: Option<u64>,
    unadjusted_abi_alignment_bytes: u64,
    randomization_seed: u64,
    aggregate: SemanticAggregateLayoutV1,
}

impl SemanticEnumVariantLayoutV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn from_rustc(
        variant_index: u32,
        rustc_size_bytes: u64,
        alignment_bytes: u64,
        fields: SemanticFieldsShapeV1,
        backend_repr: SemanticBackendReprV1,
        largest_niche: Option<SemanticLayoutNicheV1>,
        uninhabited: bool,
        max_repr_alignment_bytes: Option<u64>,
        unadjusted_abi_alignment_bytes: u64,
        randomization_seed: u64,
        aggregate: SemanticAggregateLayoutV1,
    ) -> Result<Self, SemanticMirErrorV1> {
        if !valid_rustc_alignment(alignment_bytes)
            || !rustc_size_bytes.is_multiple_of(alignment_bytes)
            || matches!(backend_repr, SemanticBackendReprV1::Memory { sized: false })
            || max_repr_alignment_bytes.is_some_and(|alignment| !valid_rustc_alignment(alignment))
            || !valid_rustc_alignment(unadjusted_abi_alignment_bytes)
        {
            return Err(SemanticMirErrorV1::InvalidTypeLayout);
        }
        validate_fields_shape(&fields, Some(rustc_size_bytes))?;
        validate_backend_repr(Some(rustc_size_bytes), alignment_bytes, &backend_repr)?;
        if let Some(niche) = largest_niche {
            validate_layout_niche(niche, Some(rustc_size_bytes))?;
        }
        let SemanticFieldsShapeV1::Arbitrary {
            source_order_offsets_bytes,
            ..
        } = &fields
        else {
            return Err(SemanticMirErrorV1::InvalidTypeLayout);
        };
        if source_order_offsets_bytes.as_ref() != aggregate.field_offsets.as_ref() {
            return Err(SemanticMirErrorV1::InvalidTypeLayout);
        }
        Ok(Self {
            variant_index,
            rustc_size_bytes,
            alignment_bytes,
            fields,
            backend_repr,
            largest_niche,
            uninhabited,
            max_repr_alignment_bytes,
            unadjusted_abi_alignment_bytes,
            randomization_seed,
            aggregate,
        })
    }

    pub const fn variant_index(&self) -> u32 {
        self.variant_index
    }

    pub const fn rustc_size_bytes(&self) -> u64 {
        self.rustc_size_bytes
    }

    pub const fn alignment_bytes(&self) -> u64 {
        self.alignment_bytes
    }

    pub const fn fields(&self) -> &SemanticFieldsShapeV1 {
        &self.fields
    }

    pub const fn backend_repr(&self) -> &SemanticBackendReprV1 {
        &self.backend_repr
    }

    pub const fn largest_niche(&self) -> Option<SemanticLayoutNicheV1> {
        self.largest_niche
    }

    pub const fn is_uninhabited(&self) -> bool {
        self.uninhabited
    }

    pub const fn max_repr_alignment_bytes(&self) -> Option<u64> {
        self.max_repr_alignment_bytes
    }

    pub const fn unadjusted_abi_alignment_bytes(&self) -> u64 {
        self.unadjusted_abi_alignment_bytes
    }

    pub const fn randomization_seed(&self) -> u64 {
        self.randomization_seed
    }

    pub const fn aggregate(&self) -> &SemanticAggregateLayoutV1 {
        &self.aggregate
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticScalarValidityRangeV1 {
    start: u128,
    end: u128,
}

impl SemanticScalarValidityRangeV1 {
    pub const fn new(start: u128, end: u128) -> Self {
        Self { start, end }
    }

    pub const fn start(self) -> u128 {
        self.start
    }

    pub const fn end(self) -> u128 {
        self.end
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticValidityScalarTypeV1 {
    scalar: SemanticScalarTypeV1,
    valid_ranges: Box<[SemanticScalarValidityRangeV1]>,
}

impl SemanticValidityScalarTypeV1 {
    pub fn new(
        scalar: SemanticScalarTypeV1,
        valid_ranges: Vec<SemanticScalarValidityRangeV1>,
    ) -> Result<Self, SemanticMirErrorV1> {
        enforce_hard(SemanticMirResourceV1::Types, valid_ranges.len())?;
        validate_validity_ranges(scalar, &valid_ranges)?;
        Ok(Self {
            scalar,
            valid_ranges: valid_ranges.into_boxed_slice(),
        })
    }

    pub const fn scalar(&self) -> SemanticScalarTypeV1 {
        self.scalar
    }

    pub fn valid_ranges(&self) -> &[SemanticScalarValidityRangeV1] {
        &self.valid_ranges
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticNichePathComponentV1 {
    Field(u32),
    ArrayElement(u64),
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticNicheSourceV1 {
    path: Box<[SemanticNichePathComponentV1]>,
    expected_offset_bytes: u64,
}

impl SemanticNicheSourceV1 {
    pub fn new(
        path: Vec<SemanticNichePathComponentV1>,
        expected_offset_bytes: u64,
    ) -> Result<Self, SemanticMirErrorV1> {
        enforce_hard(SemanticMirResourceV1::Types, path.len())?;
        if path.is_empty() {
            return Err(SemanticMirErrorV1::InvalidTypeLayout);
        }
        Ok(Self {
            path: path.into_boxed_slice(),
            expected_offset_bytes,
        })
    }

    pub fn path(&self) -> &[SemanticNichePathComponentV1] {
        &self.path
    }

    pub const fn expected_offset_bytes(&self) -> u64 {
        self.expected_offset_bytes
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticDirectEnumEncodingV1 {
    tag_field: u32,
    tag_offset_bytes: u64,
    tag: SemanticBackendScalarV1,
}

impl SemanticDirectEnumEncodingV1 {
    pub const fn new(tag_field: u32, tag_offset_bytes: u64, tag: SemanticBackendScalarV1) -> Self {
        Self {
            tag_field,
            tag_offset_bytes,
            tag,
        }
    }

    pub const fn tag_field(self) -> u32 {
        self.tag_field
    }

    pub const fn tag_offset_bytes(self) -> u64 {
        self.tag_offset_bytes
    }

    pub const fn tag(self) -> SemanticBackendScalarV1 {
        self.tag
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticNicheEnumEncodingV1 {
    tag_field: u32,
    source: SemanticNicheSourceV1,
    source_niche: SemanticLayoutNicheV1,
    tag: SemanticBackendScalarV1,
    untagged_variant: u32,
    niche_variants_start: u32,
    niche_variants_end: u32,
    niche_start: u128,
}

impl SemanticNicheEnumEncodingV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        tag_field: u32,
        source: SemanticNicheSourceV1,
        source_niche: SemanticLayoutNicheV1,
        tag: SemanticBackendScalarV1,
        untagged_variant: u32,
        niche_variants_start: u32,
        niche_variants_end: u32,
        niche_start: u128,
    ) -> Result<Self, SemanticMirErrorV1> {
        validate_layout_niche(source_niche, None)?;
        validate_backend_scalar(tag)?;
        if !matches!(tag, SemanticBackendScalarV1::Initialized { .. }) {
            return Err(SemanticMirErrorV1::InvalidTypeLayout);
        }
        Ok(Self {
            tag_field,
            source,
            source_niche,
            tag,
            untagged_variant,
            niche_variants_start,
            niche_variants_end,
            niche_start,
        })
    }

    pub const fn tag_field(&self) -> u32 {
        self.tag_field
    }

    pub const fn source(&self) -> &SemanticNicheSourceV1 {
        &self.source
    }

    pub const fn source_niche(&self) -> SemanticLayoutNicheV1 {
        self.source_niche
    }

    pub const fn tag(&self) -> SemanticBackendScalarV1 {
        self.tag
    }

    pub const fn untagged_variant(&self) -> u32 {
        self.untagged_variant
    }

    pub const fn niche_variant_range(&self) -> (u32, u32) {
        (self.niche_variants_start, self.niche_variants_end)
    }

    pub const fn niche_start(&self) -> u128 {
        self.niche_start
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticEnumEncodingV1 {
    Direct(SemanticDirectEnumEncodingV1),
    Niche(SemanticNicheEnumEncodingV1),
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticEnumLayoutV1 {
    variants: Box<[SemanticEnumVariantLayoutV1]>,
    encoding: SemanticEnumEncodingV1,
}

impl SemanticEnumLayoutV1 {
    pub fn new(
        variants: Vec<SemanticEnumVariantLayoutV1>,
        encoding: SemanticEnumEncodingV1,
    ) -> Result<Self, SemanticMirErrorV1> {
        enforce_hard(SemanticMirResourceV1::Types, variants.len())?;
        Ok(Self {
            variants: variants.into_boxed_slice(),
            encoding,
        })
    }

    pub fn variants(&self) -> &[SemanticEnumVariantLayoutV1] {
        &self.variants
    }

    pub const fn encoding(&self) -> &SemanticEnumEncodingV1 {
        &self.encoding
    }
}

fn enum_outer_fields(
    layout: &SemanticEnumLayoutV1,
) -> Result<SemanticFieldsShapeV1, SemanticMirErrorV1> {
    let offset = match &layout.encoding {
        SemanticEnumEncodingV1::Direct(direct) if direct.tag_field == 0 => direct.tag_offset_bytes,
        SemanticEnumEncodingV1::Niche(niche) if niche.tag_field == 0 => {
            niche.source.expected_offset_bytes
        }
        SemanticEnumEncodingV1::Direct(_) | SemanticEnumEncodingV1::Niche(_) => {
            return Err(SemanticMirErrorV1::InvalidTypeLayout);
        }
    };
    SemanticFieldsShapeV1::arbitrary(vec![offset], vec![0])
}

fn enforce_rustc_variants_shape(
    variants: &SemanticRustcVariantsV1,
    uninhabited: bool,
) -> Result<(), SemanticMirErrorV1> {
    match variants {
        SemanticRustcVariantsV1::Empty if uninhabited => Ok(()),
        SemanticRustcVariantsV1::Single { .. } => Ok(()),
        SemanticRustcVariantsV1::Multiple(layout)
            if !layout.variants.is_empty()
                && matches!(
                    &layout.encoding,
                    SemanticEnumEncodingV1::Direct(_) | SemanticEnumEncodingV1::Niche(_)
                ) =>
        {
            Ok(())
        }
        SemanticRustcVariantsV1::Empty | SemanticRustcVariantsV1::Multiple(_) => {
            Err(SemanticMirErrorV1::InvalidTypeLayout)
        }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticTypeLayoutDetailsV1 {
    None,
    Aggregate(SemanticAggregateLayoutV1),
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticScalarTypeV1 {
    Bool,
    Char,
    Integer { signed: bool, bits: u16 },
    Float { bits: u16 },
}

impl SemanticScalarTypeV1 {
    const fn byte_width(self) -> Option<u64> {
        match self {
            Self::Bool => Some(1),
            Self::Char => Some(4),
            Self::Integer { bits, .. } if matches!(bits, 8 | 16 | 32 | 64 | 128) => {
                Some((bits / 8) as u64)
            }
            Self::Float { bits } if matches!(bits, 16 | 32 | 64 | 128) => Some((bits / 8) as u64),
            Self::Integer { .. } | Self::Float { .. } => None,
        }
    }

    const fn bits(self) -> Option<u16> {
        match self {
            Self::Bool => Some(1),
            Self::Char => Some(32),
            Self::Integer { bits, .. } | Self::Float { bits } => {
                if self.byte_width().is_some() {
                    Some(bits)
                } else {
                    None
                }
            }
        }
    }

    const fn is_integer(self) -> bool {
        matches!(self, Self::Integer { .. })
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticMutabilityV1 {
    Immutable,
    Mutable,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticPointerKindV1 {
    Raw,
    Reference,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticPointerMetadataV1 {
    None,
    SliceLength,
    VTable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticPointerTypeV1 {
    pointee: SemanticTypeIdV1,
    kind: SemanticPointerKindV1,
    mutability: SemanticMutabilityV1,
    address_space: u32,
    pointer_width_bits: u16,
    metadata: SemanticPointerMetadataV1,
}

impl SemanticPointerTypeV1 {
    pub fn new(
        pointee: SemanticTypeIdV1,
        mutability: SemanticMutabilityV1,
        address_space: u32,
        pointer_width_bits: u16,
        metadata: SemanticPointerMetadataV1,
    ) -> Result<Self, SemanticMirErrorV1> {
        Self::new_with_kind(
            pointee,
            SemanticPointerKindV1::Raw,
            mutability,
            address_space,
            pointer_width_bits,
            metadata,
        )
    }

    pub fn new_with_kind(
        pointee: SemanticTypeIdV1,
        kind: SemanticPointerKindV1,
        mutability: SemanticMutabilityV1,
        address_space: u32,
        pointer_width_bits: u16,
        metadata: SemanticPointerMetadataV1,
    ) -> Result<Self, SemanticMirErrorV1> {
        if pointer_width_bits == 0 || !pointer_width_bits.is_multiple_of(8) {
            return Err(SemanticMirErrorV1::InvalidPointerWidth);
        }
        Ok(Self {
            pointee,
            kind,
            mutability,
            address_space,
            pointer_width_bits,
            metadata,
        })
    }

    pub const fn pointee(&self) -> SemanticTypeIdV1 {
        self.pointee
    }

    pub const fn kind(&self) -> SemanticPointerKindV1 {
        self.kind
    }

    pub const fn mutability(&self) -> SemanticMutabilityV1 {
        self.mutability
    }

    pub const fn address_space(&self) -> u32 {
        self.address_space
    }

    pub const fn pointer_width_bits(&self) -> u16 {
        self.pointer_width_bits
    }

    pub const fn metadata(&self) -> SemanticPointerMetadataV1 {
        self.metadata
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticAggregateTypeV1 {
    fields: Box<[SemanticTypeIdV1]>,
}

impl SemanticAggregateTypeV1 {
    pub fn new(fields: Vec<SemanticTypeIdV1>) -> Result<Self, SemanticMirErrorV1> {
        enforce_hard(SemanticMirResourceV1::Types, fields.len())?;
        Ok(Self {
            fields: fields.into_boxed_slice(),
        })
    }

    pub fn fields(&self) -> &[SemanticTypeIdV1] {
        &self.fields
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticEnumVariantV1 {
    discriminant: u128,
    fields: SemanticAggregateTypeV1,
    uninhabited: bool,
}

impl SemanticEnumVariantV1 {
    pub const fn new(discriminant: u128, fields: SemanticAggregateTypeV1) -> Self {
        Self::new_with_inhabitedness(discriminant, fields, false)
    }

    pub const fn new_with_inhabitedness(
        discriminant: u128,
        fields: SemanticAggregateTypeV1,
        uninhabited: bool,
    ) -> Self {
        Self {
            discriminant,
            fields,
            uninhabited,
        }
    }

    pub const fn discriminant(&self) -> u128 {
        self.discriminant
    }

    pub const fn fields(&self) -> &SemanticAggregateTypeV1 {
        &self.fields
    }

    pub const fn is_uninhabited(&self) -> bool {
        self.uninhabited
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticFunctionSafetyV1 {
    Safe,
    Unsafe,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SemanticTypeShapeV1 {
    Unit,
    Never,
    Scalar(SemanticScalarTypeV1),
    ValidityScalar(SemanticValidityScalarTypeV1),
    Pointer(SemanticPointerTypeV1),
    Array {
        element: SemanticTypeIdV1,
        length: u64,
    },
    Slice {
        element: SemanticTypeIdV1,
    },
    Tuple(SemanticAggregateTypeV1),
    Aggregate(SemanticAggregateTypeV1),
    Union(SemanticAggregateTypeV1),
    Enum {
        discriminant: SemanticTypeIdV1,
        variants: Box<[SemanticEnumVariantV1]>,
    },
    FunctionPointer {
        safety: SemanticFunctionSafetyV1,
        extern_abi: SemanticExternAbiV1,
        c_variadic: bool,
        arguments: SemanticAggregateTypeV1,
        return_type: SemanticTypeIdV1,
    },
    Opaque,
}

/// rustc's safe-pointer classification after target/session policy has been
/// reduced to the facts that affect `ArgAttributes`.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticAbiPointeeKindV1 {
    Raw,
    SharedReference { frozen: bool },
    MutableReference { unpin: bool },
    Box { unpin: bool, global: bool },
}

/// Normalized result of rustc's `pointee_info_at` query for one ABI scalar.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticAbiPointeeInfoV1 {
    kind: SemanticAbiPointeeKindV1,
    guaranteed_size_bytes: u64,
    reliable_alignment_bytes: u64,
}

impl SemanticAbiPointeeInfoV1 {
    pub fn new(
        kind: SemanticAbiPointeeKindV1,
        guaranteed_size_bytes: u64,
        reliable_alignment_bytes: u64,
    ) -> Result<Self, SemanticMirErrorV1> {
        if !valid_rustc_alignment(reliable_alignment_bytes)
            || (matches!(kind, SemanticAbiPointeeKindV1::Raw)
                && (guaranteed_size_bytes != 0 || reliable_alignment_bytes != 1))
            || (matches!(
                kind,
                SemanticAbiPointeeKindV1::SharedReference { frozen: false }
                    | SemanticAbiPointeeKindV1::MutableReference { unpin: false }
                    | SemanticAbiPointeeKindV1::Box { .. }
            ) && guaranteed_size_bytes != 0)
        {
            return Err(SemanticMirErrorV1::InvalidFunctionAbi);
        }
        Ok(Self {
            kind,
            guaranteed_size_bytes,
            reliable_alignment_bytes,
        })
    }

    pub const fn kind(self) -> SemanticAbiPointeeKindV1 {
        self.kind
    }

    pub const fn guaranteed_size_bytes(self) -> u64 {
        self.guaranteed_size_bytes
    }

    pub const fn reliable_alignment_bytes(self) -> u64 {
        self.reliable_alignment_bytes
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticTypeAbiPropertiesV1 {
    pass_indirectly_in_non_rustic_abis: bool,
    has_unsized_foreign_tail: bool,
    rustc_layout_is_noundef: bool,
    first_pointee: Option<SemanticAbiPointeeInfoV1>,
    second_pointee: Option<SemanticAbiPointeeInfoV1>,
}

impl SemanticTypeAbiPropertiesV1 {
    pub const fn new(
        pass_indirectly_in_non_rustic_abis: bool,
        has_unsized_foreign_tail: bool,
    ) -> Self {
        Self {
            pass_indirectly_in_non_rustic_abis,
            has_unsized_foreign_tail,
            rustc_layout_is_noundef: false,
            first_pointee: None,
            second_pointee: None,
        }
    }

    pub const fn with_rustc_layout_is_noundef(mut self, rustc_layout_is_noundef: bool) -> Self {
        self.rustc_layout_is_noundef = rustc_layout_is_noundef;
        self
    }

    pub const fn with_scalar_pointee_info(
        mut self,
        first: Option<SemanticAbiPointeeInfoV1>,
        second: Option<SemanticAbiPointeeInfoV1>,
    ) -> Self {
        self.first_pointee = first;
        self.second_pointee = second;
        self
    }

    pub const fn pass_indirectly_in_non_rustic_abis(self) -> bool {
        self.pass_indirectly_in_non_rustic_abis
    }

    pub const fn has_unsized_foreign_tail(self) -> bool {
        self.has_unsized_foreign_tail
    }

    pub const fn rustc_layout_is_noundef(self) -> bool {
        self.rustc_layout_is_noundef
    }

    pub const fn first_pointee(self) -> Option<SemanticAbiPointeeInfoV1> {
        self.first_pointee
    }

    pub const fn second_pointee(self) -> Option<SemanticAbiPointeeInfoV1> {
        self.second_pointee
    }
}

impl SemanticTypeShapeV1 {
    pub fn enum_type(
        discriminant: SemanticTypeIdV1,
        variants: Vec<SemanticEnumVariantV1>,
    ) -> Result<Self, SemanticMirErrorV1> {
        enforce_hard(SemanticMirResourceV1::Types, variants.len())?;
        Ok(Self::Enum {
            discriminant,
            variants: variants.into_boxed_slice(),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticTypeDeclV1 {
    identity: SemanticTypeIdentityV1,
    layout_identity: SemanticLayoutIdentityV1,
    layout: SemanticTypeLayoutV1,
    shape: SemanticTypeShapeV1,
    abi_properties: SemanticTypeAbiPropertiesV1,
}

impl SemanticTypeDeclV1 {
    pub const fn new(
        identity: SemanticTypeIdentityV1,
        layout_identity: SemanticLayoutIdentityV1,
        layout: SemanticTypeLayoutV1,
        shape: SemanticTypeShapeV1,
    ) -> Self {
        Self {
            identity,
            layout_identity,
            layout,
            shape,
            abi_properties: SemanticTypeAbiPropertiesV1::new(false, false),
        }
    }

    pub const fn with_rustc_abi_properties(
        mut self,
        abi_properties: SemanticTypeAbiPropertiesV1,
    ) -> Self {
        self.abi_properties = abi_properties;
        self
    }

    pub const fn identity(&self) -> SemanticTypeIdentityV1 {
        self.identity
    }

    pub const fn layout_identity(&self) -> SemanticLayoutIdentityV1 {
        self.layout_identity
    }

    pub const fn layout(&self) -> &SemanticTypeLayoutV1 {
        &self.layout
    }

    pub const fn shape(&self) -> &SemanticTypeShapeV1 {
        &self.shape
    }

    pub const fn abi_properties(&self) -> SemanticTypeAbiPropertiesV1 {
        self.abi_properties
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticArmCallV1 {
    Aapcs,
    CCmseNonSecureCall,
    CCmseNonSecureEntry,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticInterruptKindV1 {
    Avr,
    AvrNonBlocking,
    Msp430,
    RiscvMachine,
    RiscvSupervisor,
    X86,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticX86CallV1 {
    Fastcall,
    Stdcall,
    SysV64,
    Thiscall,
    Vectorcall,
    Win64,
}

/// The exact canonical calling-convention grammar of the pinned rustc.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticCanonAbiV1 {
    C,
    Rust,
    RustCold,
    RustPreserveNone,
    Custom,
    Arm(SemanticArmCallV1),
    GpuKernel,
    Interrupt(SemanticInterruptKindV1),
    X86(SemanticX86CallV1),
}

/// The source `ExternAbi` distinction retained before target canonicalization.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticExternAbiV1 {
    C { unwind: bool },
    Cdecl { unwind: bool },
    System { unwind: bool },
    Rust,
    RustCall,
    RustCold,
    RustPreserveNone,
    Unadjusted,
    Custom,
    GpuKernel,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticAbiExtensionV1 {
    None,
    ZeroExtend,
    SignExtend,
}

/// One of rustc's three explicit LLVM pointer-capture restrictions.
///
/// Absence of a capture restriction is represented by `None` around this enum.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticAbiPointerCaptureV1 {
    CapturesNone,
    CapturesAddress,
    CapturesReadOnly,
}

impl SemanticAbiPointerCaptureV1 {
    const fn rustc_bits(self) -> u8 {
        match self {
            Self::CapturesNone => 0b111,
            Self::CapturesAddress => 0b110,
            Self::CapturesReadOnly => 0b100,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticAbiRegularAttributesV1 {
    rustc_bits: u8,
}

impl SemanticAbiRegularAttributesV1 {
    const CAPTURE_MASK: u8 = 0b111;
    const NO_ALIAS: u8 = 1 << 3;
    const NON_NULL: u8 = 1 << 4;
    const READ_ONLY: u8 = 1 << 5;
    const IN_REGISTER: u8 = 1 << 6;
    const NO_UNDEF: u8 = 1 << 7;

    /// Constructs the exact rustc `ArgAttribute` facts retained by this model.
    pub const fn new(
        no_alias: bool,
        pointer_capture: Option<SemanticAbiPointerCaptureV1>,
        non_null: bool,
        read_only: bool,
        in_register: bool,
        no_undef: bool,
    ) -> Self {
        let rustc_bits = (if no_alias { Self::NO_ALIAS } else { 0 })
            | match pointer_capture {
                Some(capture) => capture.rustc_bits(),
                None => 0,
            }
            | if non_null { Self::NON_NULL } else { 0 }
            | if read_only { Self::READ_ONLY } else { 0 }
            | if in_register { Self::IN_REGISTER } else { 0 }
            | if no_undef { Self::NO_UNDEF } else { 0 };
        Self { rustc_bits }
    }

    /// Validates and decodes rustc's pinned `ArgAttribute::bits()` value.
    pub fn from_rustc_bits(bits: u8) -> Result<Self, SemanticMirErrorV1> {
        let pointer_capture = match bits & Self::CAPTURE_MASK {
            0 => None,
            0b111 => Some(SemanticAbiPointerCaptureV1::CapturesNone),
            0b110 => Some(SemanticAbiPointerCaptureV1::CapturesAddress),
            0b100 => Some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
            _ => return Err(SemanticMirErrorV1::InvalidFunctionAbi),
        };
        Ok(Self::new(
            bits & Self::NO_ALIAS != 0,
            pointer_capture,
            bits & Self::NON_NULL != 0,
            bits & Self::READ_ONLY != 0,
            bits & Self::IN_REGISTER != 0,
            bits & Self::NO_UNDEF != 0,
        ))
    }

    /// Returns the collision-free bit pattern used by the pinned rustc.
    pub const fn rustc_bits(self) -> u8 {
        self.rustc_bits
    }

    pub const fn no_alias(self) -> bool {
        self.rustc_bits & Self::NO_ALIAS != 0
    }

    pub const fn pointer_capture(self) -> Option<SemanticAbiPointerCaptureV1> {
        match self.rustc_bits & Self::CAPTURE_MASK {
            0b111 => Some(SemanticAbiPointerCaptureV1::CapturesNone),
            0b110 => Some(SemanticAbiPointerCaptureV1::CapturesAddress),
            0b100 => Some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
            0 => None,
            _ => unreachable!(),
        }
    }

    pub const fn non_null(self) -> bool {
        self.rustc_bits & Self::NON_NULL != 0
    }

    pub const fn read_only(self) -> bool {
        self.rustc_bits & Self::READ_ONLY != 0
    }

    pub const fn in_register(self) -> bool {
        self.rustc_bits & Self::IN_REGISTER != 0
    }

    pub const fn no_undef(self) -> bool {
        self.rustc_bits & Self::NO_UNDEF != 0
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticAbiValueAttributesV1 {
    regular: SemanticAbiRegularAttributesV1,
    extension: SemanticAbiExtensionV1,
    pointee_size_bytes: u64,
    pointee_alignment_bytes: Option<u64>,
}

impl SemanticAbiValueAttributesV1 {
    pub fn new(
        regular: SemanticAbiRegularAttributesV1,
        extension: SemanticAbiExtensionV1,
        pointee_size_bytes: u64,
        pointee_alignment_bytes: Option<u64>,
    ) -> Result<Self, SemanticMirErrorV1> {
        if pointee_alignment_bytes.is_some_and(|alignment| !valid_rustc_alignment(alignment)) {
            return Err(SemanticMirErrorV1::InvalidFunctionAbi);
        }
        Ok(Self {
            regular,
            extension,
            pointee_size_bytes,
            pointee_alignment_bytes,
        })
    }

    pub const fn plain() -> Self {
        Self {
            regular: SemanticAbiRegularAttributesV1::new(false, None, false, false, false, false),
            extension: SemanticAbiExtensionV1::None,
            pointee_size_bytes: 0,
            pointee_alignment_bytes: None,
        }
    }

    pub const fn regular(self) -> SemanticAbiRegularAttributesV1 {
        self.regular
    }

    pub const fn extension(self) -> SemanticAbiExtensionV1 {
        self.extension
    }

    pub const fn pointee_size_bytes(self) -> u64 {
        self.pointee_size_bytes
    }

    pub const fn pointee_alignment_bytes(self) -> Option<u64> {
        self.pointee_alignment_bytes
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticAbiRegisterKindV1 {
    Integer,
    Float,
    Vector,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticAbiRegisterV1 {
    kind: SemanticAbiRegisterKindV1,
    size_bytes: u64,
}

impl SemanticAbiRegisterV1 {
    pub fn new(
        kind: SemanticAbiRegisterKindV1,
        size_bytes: u64,
    ) -> Result<Self, SemanticMirErrorV1> {
        if size_bytes == 0 {
            return Err(SemanticMirErrorV1::InvalidFunctionAbi);
        }
        Ok(Self { kind, size_bytes })
    }

    pub const fn kind(self) -> SemanticAbiRegisterKindV1 {
        self.kind
    }

    pub const fn size_bytes(self) -> u64 {
        self.size_bytes
    }
}

/// rustc's `Uniform` rest description within a `CastTarget`.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticAbiUniformV1 {
    unit: SemanticAbiRegisterV1,
    total_bytes: u64,
    consecutive: bool,
}

impl SemanticAbiUniformV1 {
    pub fn new(unit: SemanticAbiRegisterV1, total_bytes: u64) -> Result<Self, SemanticMirErrorV1> {
        Self::from_rustc(unit, total_bytes, false)
    }

    pub fn consecutive(
        unit: SemanticAbiRegisterV1,
        total_bytes: u64,
    ) -> Result<Self, SemanticMirErrorV1> {
        Self::from_rustc(unit, total_bytes, true)
    }

    pub fn from_rustc(
        unit: SemanticAbiRegisterV1,
        total_bytes: u64,
        consecutive: bool,
    ) -> Result<Self, SemanticMirErrorV1> {
        if total_bytes != 0
            && !total_bytes.is_multiple_of(unit.size_bytes)
            && unit.kind != SemanticAbiRegisterKindV1::Integer
        {
            return Err(SemanticMirErrorV1::InvalidFunctionAbi);
        }
        Ok(Self {
            unit,
            total_bytes,
            consecutive,
        })
    }

    pub const fn unit(self) -> SemanticAbiRegisterV1 {
        self.unit
    }

    pub const fn total_bytes(self) -> u64 {
        self.total_bytes
    }

    pub const fn is_consecutive(self) -> bool {
        self.consecutive
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticAbiCastV1 {
    prefix: [Option<SemanticAbiRegisterV1>; 8],
    rest_offset_bytes: Option<u64>,
    rest: SemanticAbiUniformV1,
    attributes: SemanticAbiValueAttributesV1,
}

impl SemanticAbiCastV1 {
    pub const fn new(
        prefix: [Option<SemanticAbiRegisterV1>; 8],
        rest_offset_bytes: Option<u64>,
        rest: SemanticAbiUniformV1,
        attributes: SemanticAbiValueAttributesV1,
    ) -> Self {
        Self {
            prefix,
            rest_offset_bytes,
            rest,
            attributes,
        }
    }

    pub const fn prefix(&self) -> &[Option<SemanticAbiRegisterV1>; 8] {
        &self.prefix
    }

    pub const fn rest_offset_bytes(&self) -> Option<u64> {
        self.rest_offset_bytes
    }

    pub const fn rest(&self) -> SemanticAbiUniformV1 {
        self.rest
    }

    pub const fn rest_total_bytes(&self) -> u64 {
        self.rest.total_bytes
    }

    pub const fn rest_consecutive(&self) -> bool {
        self.rest.consecutive
    }

    pub const fn attributes(&self) -> SemanticAbiValueAttributesV1 {
        self.attributes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SemanticAbiPassModeV1 {
    Ignore,
    Direct(SemanticAbiValueAttributesV1),
    Pair {
        first: SemanticAbiValueAttributesV1,
        second: SemanticAbiValueAttributesV1,
    },
    Cast {
        pad_i32: bool,
        cast: SemanticAbiCastV1,
    },
    Indirect {
        attributes: SemanticAbiValueAttributesV1,
        metadata_attributes: Option<SemanticAbiValueAttributesV1>,
        on_stack: bool,
    },
}

impl SemanticAbiPassModeV1 {
    pub const fn cast(pad_i32: bool, cast: SemanticAbiCastV1) -> Self {
        Self::Cast { pad_i32, cast }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticAbiAdjustedTypeV1 {
    ty: SemanticTypeIdV1,
    layout_identity: SemanticLayoutIdentityV1,
    layout: SemanticTypeLayoutV1,
}

impl SemanticAbiAdjustedTypeV1 {
    pub const fn new(
        ty: SemanticTypeIdV1,
        layout_identity: SemanticLayoutIdentityV1,
        layout: SemanticTypeLayoutV1,
    ) -> Self {
        Self {
            ty,
            layout_identity,
            layout,
        }
    }

    pub const fn ty(&self) -> SemanticTypeIdV1 {
        self.ty
    }

    pub const fn layout_identity(&self) -> SemanticLayoutIdentityV1 {
        self.layout_identity
    }

    pub const fn layout(&self) -> &SemanticTypeLayoutV1 {
        &self.layout
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticAbiValueV1 {
    source_ty: SemanticTypeIdV1,
    adjusted: Option<Box<SemanticAbiAdjustedTypeV1>>,
    pointee_override: Option<SemanticAbiPointeeInfoV1>,
    mode: SemanticAbiPassModeV1,
}

impl SemanticAbiValueV1 {
    pub const fn new(source_ty: SemanticTypeIdV1, mode: SemanticAbiPassModeV1) -> Self {
        Self {
            source_ty,
            adjusted: None,
            pointee_override: None,
            mode,
        }
    }

    pub fn new_with_adjusted_type(
        source_ty: SemanticTypeIdV1,
        adjusted: SemanticAbiAdjustedTypeV1,
        mode: SemanticAbiPassModeV1,
    ) -> Self {
        Self {
            source_ty,
            adjusted: Some(Box::new(adjusted)),
            pointee_override: None,
            mode,
        }
    }

    pub const fn with_pointee_override(
        mut self,
        pointee_override: SemanticAbiPointeeInfoV1,
    ) -> Self {
        self.pointee_override = Some(pointee_override);
        self
    }

    pub const fn ty(&self) -> SemanticTypeIdV1 {
        self.source_ty
    }

    pub const fn source_ty(&self) -> SemanticTypeIdV1 {
        self.source_ty
    }

    pub fn adjusted(&self) -> Option<&SemanticAbiAdjustedTypeV1> {
        self.adjusted.as_deref()
    }

    pub fn adjusted_ty(&self) -> SemanticTypeIdV1 {
        self.adjusted
            .as_deref()
            .map_or(self.source_ty, SemanticAbiAdjustedTypeV1::ty)
    }

    pub const fn pointee_override(&self) -> Option<SemanticAbiPointeeInfoV1> {
        self.pointee_override
    }

    pub const fn mode(&self) -> &SemanticAbiPassModeV1 {
        &self.mode
    }
}

/// An ABI argument synthesized by rustc rather than supplied by the MIR caller.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticAbiHiddenArgumentRoleV1 {
    CallerLocation,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticAbiArgumentRoleV1 {
    Source,
    RustCallTupleField(u32),
    Hidden(SemanticAbiHiddenArgumentRoleV1),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticAbiArgumentV1 {
    role: SemanticAbiArgumentRoleV1,
    value: SemanticAbiValueV1,
}

impl SemanticAbiArgumentV1 {
    pub const fn source(value: SemanticAbiValueV1) -> Self {
        Self {
            role: SemanticAbiArgumentRoleV1::Source,
            value,
        }
    }

    pub const fn hidden(role: SemanticAbiHiddenArgumentRoleV1, value: SemanticAbiValueV1) -> Self {
        Self {
            role: SemanticAbiArgumentRoleV1::Hidden(role),
            value,
        }
    }

    pub const fn rust_call_tuple_field(field: u32, value: SemanticAbiValueV1) -> Self {
        Self {
            role: SemanticAbiArgumentRoleV1::RustCallTupleField(field),
            value,
        }
    }

    pub const fn role(&self) -> SemanticAbiArgumentRoleV1 {
        self.role
    }

    pub const fn value(&self) -> &SemanticAbiValueV1 {
        &self.value
    }

    pub const fn ty(&self) -> SemanticTypeIdV1 {
        self.value.source_ty
    }

    pub const fn mode(&self) -> &SemanticAbiPassModeV1 {
        &self.value.mode
    }

    pub const fn is_source(&self) -> bool {
        matches!(
            self.role,
            SemanticAbiArgumentRoleV1::Source | SemanticAbiArgumentRoleV1::RustCallTupleField(_)
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticSourceFnSignatureV1 {
    extern_abi: SemanticExternAbiV1,
    c_variadic: bool,
    inputs: Box<[SemanticTypeIdV1]>,
    output: SemanticTypeIdV1,
}

impl SemanticSourceFnSignatureV1 {
    pub const fn extern_abi(&self) -> SemanticExternAbiV1 {
        self.extern_abi
    }

    pub const fn c_variadic(&self) -> bool {
        self.c_variadic
    }

    pub fn inputs(&self) -> &[SemanticTypeIdV1] {
        &self.inputs
    }

    pub const fn output(&self) -> SemanticTypeIdV1 {
        self.output
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticFunctionAbiV1 {
    identity: SemanticAbiIdentityV1,
    layout_identity: SemanticLayoutIdentityV1,
    canon_abi: SemanticCanonAbiV1,
    source_signature: SemanticSourceFnSignatureV1,
    can_unwind: bool,
    fixed_count: u32,
    arguments: Box<[SemanticAbiArgumentV1]>,
    return_value: SemanticAbiValueV1,
}

impl SemanticFunctionAbiV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        identity: SemanticAbiIdentityV1,
        layout_identity: SemanticLayoutIdentityV1,
        canon_abi: SemanticCanonAbiV1,
        can_unwind: bool,
        c_variadic: bool,
        arguments: Vec<SemanticAbiValueV1>,
        return_value: SemanticAbiValueV1,
    ) -> Result<Self, SemanticMirErrorV1> {
        enforce_hard(SemanticMirResourceV1::CallArguments, arguments.len())?;
        let fixed_count =
            u32::try_from(arguments.len()).map_err(|_| SemanticMirErrorV1::InvalidFunctionAbi)?;
        let extern_abi = default_extern_abi(canon_abi, can_unwind)?;
        let source_input_types = arguments
            .iter()
            .map(SemanticAbiValueV1::source_ty)
            .collect();
        Self::from_rustc_with_source_signature(
            identity,
            layout_identity,
            canon_abi,
            extern_abi,
            can_unwind,
            c_variadic,
            fixed_count,
            source_input_types,
            return_value.source_ty,
            arguments
                .into_iter()
                .map(SemanticAbiArgumentV1::source)
                .collect(),
            return_value,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn from_rustc(
        identity: SemanticAbiIdentityV1,
        layout_identity: SemanticLayoutIdentityV1,
        canon_abi: SemanticCanonAbiV1,
        extern_abi: SemanticExternAbiV1,
        can_unwind: bool,
        c_variadic: bool,
        fixed_count: u32,
        arguments: Vec<SemanticAbiArgumentV1>,
        return_value: SemanticAbiValueV1,
    ) -> Result<Self, SemanticMirErrorV1> {
        if extern_abi == SemanticExternAbiV1::RustCall {
            return Err(SemanticMirErrorV1::InvalidFunctionAbi);
        }
        let source_input_types = arguments
            .iter()
            .take_while(|argument| matches!(argument.role, SemanticAbiArgumentRoleV1::Source))
            .map(|argument| argument.value.source_ty)
            .collect();
        Self::from_rustc_with_source_signature(
            identity,
            layout_identity,
            canon_abi,
            extern_abi,
            can_unwind,
            c_variadic,
            fixed_count,
            source_input_types,
            return_value.source_ty,
            arguments,
            return_value,
        )
    }

    /// Builds the adjusted rustc `FnAbi` together with its pre-adjustment Rust signature.
    #[allow(clippy::too_many_arguments)]
    pub fn from_rustc_with_source_signature(
        identity: SemanticAbiIdentityV1,
        layout_identity: SemanticLayoutIdentityV1,
        canon_abi: SemanticCanonAbiV1,
        extern_abi: SemanticExternAbiV1,
        can_unwind: bool,
        c_variadic: bool,
        fixed_count: u32,
        source_input_types: Vec<SemanticTypeIdV1>,
        source_output_type: SemanticTypeIdV1,
        arguments: Vec<SemanticAbiArgumentV1>,
        return_value: SemanticAbiValueV1,
    ) -> Result<Self, SemanticMirErrorV1> {
        enforce_hard(SemanticMirResourceV1::CallArguments, arguments.len())?;
        enforce_hard(
            SemanticMirResourceV1::CallArguments,
            source_input_types.len(),
        )?;
        validate_abi_argument_contract(
            extern_abi,
            c_variadic,
            fixed_count,
            &source_input_types,
            source_output_type,
            &arguments,
            return_value.source_ty,
        )?;
        if canonicalize_extern_abi(extern_abi) != Some(canon_abi) {
            return Err(SemanticMirErrorV1::InvalidFunctionAbi);
        }
        Ok(Self {
            identity,
            layout_identity,
            canon_abi,
            source_signature: SemanticSourceFnSignatureV1 {
                extern_abi,
                c_variadic,
                inputs: source_input_types.into_boxed_slice(),
                output: source_output_type,
            },
            can_unwind,
            fixed_count,
            arguments: arguments.into_boxed_slice(),
            return_value,
        })
    }

    pub const fn identity(&self) -> SemanticAbiIdentityV1 {
        self.identity
    }

    pub const fn layout_identity(&self) -> SemanticLayoutIdentityV1 {
        self.layout_identity
    }

    pub const fn canon_abi(&self) -> SemanticCanonAbiV1 {
        self.canon_abi
    }

    /// Retains the pre-canonicalization `ExternAbi::Unadjusted` distinction.
    pub const fn spec_abi_unadjusted(&self) -> bool {
        matches!(
            self.source_signature.extern_abi,
            SemanticExternAbiV1::Unadjusted
        )
    }

    pub const fn extern_abi(&self) -> SemanticExternAbiV1 {
        self.source_signature.extern_abi
    }

    pub const fn source_signature(&self) -> &SemanticSourceFnSignatureV1 {
        &self.source_signature
    }

    pub const fn can_unwind(&self) -> bool {
        self.can_unwind
    }

    pub const fn c_variadic(&self) -> bool {
        self.source_signature.c_variadic
    }

    pub const fn fixed_count(&self) -> u32 {
        self.fixed_count
    }

    pub fn arguments(&self) -> &[SemanticAbiArgumentV1] {
        &self.arguments
    }

    pub fn source_input_types(&self) -> &[SemanticTypeIdV1] {
        &self.source_signature.inputs
    }

    pub const fn source_output_type(&self) -> SemanticTypeIdV1 {
        self.source_signature.output
    }

    /// Returns rustc's adjusted non-hidden arguments, including RustCall tuple fields.
    pub fn adjusted_arguments(&self) -> &[SemanticAbiArgumentV1] {
        let source_count = self
            .arguments
            .iter()
            .take_while(|argument| argument.is_source())
            .count();
        &self.arguments[..source_count]
    }

    pub fn fixed_arguments(&self) -> &[SemanticAbiArgumentV1] {
        &self.arguments[..self.fixed_count as usize]
    }

    pub fn hidden_arguments(&self) -> &[SemanticAbiArgumentV1] {
        &self.arguments[self.adjusted_arguments().len()..]
    }

    pub const fn return_type(&self) -> SemanticTypeIdV1 {
        self.return_value.source_ty
    }

    pub const fn return_value(&self) -> &SemanticAbiValueV1 {
        &self.return_value
    }
}

fn validate_abi_argument_contract(
    extern_abi: SemanticExternAbiV1,
    c_variadic: bool,
    fixed_count: u32,
    source_input_types: &[SemanticTypeIdV1],
    source_output_type: SemanticTypeIdV1,
    arguments: &[SemanticAbiArgumentV1],
    adjusted_output_type: SemanticTypeIdV1,
) -> Result<(), SemanticMirErrorV1> {
    if c_variadic
        && !matches!(
            extern_abi,
            SemanticExternAbiV1::C { .. }
                | SemanticExternAbiV1::Cdecl { .. }
                | SemanticExternAbiV1::System { .. }
        )
    {
        return Err(SemanticMirErrorV1::InvalidFunctionAbi);
    }
    let fixed_count =
        usize::try_from(fixed_count).map_err(|_| SemanticMirErrorV1::InvalidFunctionAbi)?;
    let mut fixed_source_count = 0_usize;
    let mut tuple_field_count = 0_usize;
    let mut saw_tuple_field = false;
    let mut saw_hidden = false;
    for argument in arguments {
        match argument.role {
            SemanticAbiArgumentRoleV1::Source => {
                if saw_tuple_field || saw_hidden {
                    return Err(SemanticMirErrorV1::InvalidFunctionAbi);
                }
                fixed_source_count += 1;
            }
            SemanticAbiArgumentRoleV1::RustCallTupleField(field) => {
                if extern_abi != SemanticExternAbiV1::RustCall
                    || saw_hidden
                    || field as usize != tuple_field_count
                {
                    return Err(SemanticMirErrorV1::InvalidFunctionAbi);
                }
                saw_tuple_field = true;
                tuple_field_count += 1;
            }
            SemanticAbiArgumentRoleV1::Hidden(role) => {
                if extern_abi != SemanticExternAbiV1::Rust
                    || role != SemanticAbiHiddenArgumentRoleV1::CallerLocation
                    || saw_hidden
                {
                    return Err(SemanticMirErrorV1::InvalidFunctionAbi);
                }
                saw_hidden = true;
            }
        }
    }
    let expected_fixed_count = if extern_abi == SemanticExternAbiV1::RustCall {
        source_input_types
            .len()
            .checked_sub(1)
            .ok_or(SemanticMirErrorV1::InvalidFunctionAbi)?
    } else {
        if saw_tuple_field {
            return Err(SemanticMirErrorV1::InvalidFunctionAbi);
        }
        source_input_types.len()
    };
    if fixed_count != expected_fixed_count
        || fixed_source_count != expected_fixed_count
        || adjusted_output_type != source_output_type
        || arguments
            .iter()
            .take(fixed_source_count)
            .zip(source_input_types)
            .any(|(adjusted, source)| adjusted.value.source_ty != *source)
    {
        return Err(SemanticMirErrorV1::InvalidFunctionAbi);
    }
    Ok(())
}

fn default_extern_abi(
    canon_abi: SemanticCanonAbiV1,
    can_unwind: bool,
) -> Result<SemanticExternAbiV1, SemanticMirErrorV1> {
    match canon_abi {
        SemanticCanonAbiV1::C => Ok(SemanticExternAbiV1::C { unwind: can_unwind }),
        SemanticCanonAbiV1::Rust => Ok(SemanticExternAbiV1::Rust),
        SemanticCanonAbiV1::RustCold => Ok(SemanticExternAbiV1::RustCold),
        SemanticCanonAbiV1::RustPreserveNone => Ok(SemanticExternAbiV1::RustPreserveNone),
        SemanticCanonAbiV1::Custom => Ok(SemanticExternAbiV1::Custom),
        SemanticCanonAbiV1::GpuKernel => Ok(SemanticExternAbiV1::GpuKernel),
        SemanticCanonAbiV1::Arm(_)
        | SemanticCanonAbiV1::Interrupt(_)
        | SemanticCanonAbiV1::X86(_) => Err(SemanticMirErrorV1::InvalidFunctionAbi),
    }
}

const fn canonicalize_extern_abi(extern_abi: SemanticExternAbiV1) -> Option<SemanticCanonAbiV1> {
    match extern_abi {
        SemanticExternAbiV1::C { .. }
        | SemanticExternAbiV1::Cdecl { .. }
        | SemanticExternAbiV1::System { .. }
        | SemanticExternAbiV1::Unadjusted => Some(SemanticCanonAbiV1::C),
        SemanticExternAbiV1::Rust | SemanticExternAbiV1::RustCall => Some(SemanticCanonAbiV1::Rust),
        SemanticExternAbiV1::RustCold => Some(SemanticCanonAbiV1::RustCold),
        SemanticExternAbiV1::RustPreserveNone => Some(SemanticCanonAbiV1::RustPreserveNone),
        SemanticExternAbiV1::Custom => Some(SemanticCanonAbiV1::Custom),
        SemanticExternAbiV1::GpuKernel => Some(SemanticCanonAbiV1::GpuKernel),
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticLocalRoleV1 {
    Return,
    Argument(u32),
    Temporary,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticLocalDeclV1 {
    identity: SemanticLocalIdentityV1,
    ty: SemanticTypeIdV1,
    role: SemanticLocalRoleV1,
    source: SemanticSourceProvenanceV1,
}

impl SemanticLocalDeclV1 {
    pub const fn new(
        identity: SemanticLocalIdentityV1,
        ty: SemanticTypeIdV1,
        role: SemanticLocalRoleV1,
        source: SemanticSourceProvenanceV1,
    ) -> Self {
        Self {
            identity,
            ty,
            role,
            source,
        }
    }

    pub const fn identity(&self) -> SemanticLocalIdentityV1 {
        self.identity
    }

    pub const fn ty(&self) -> SemanticTypeIdV1 {
        self.ty
    }

    pub const fn role(&self) -> SemanticLocalRoleV1 {
        self.role
    }

    pub const fn source(&self) -> SemanticSourceProvenanceV1 {
        self.source
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticProjectionKindV1 {
    Dereference,
    Field(u32),
    Index(SemanticLocalIdV1),
    ConstantIndex {
        offset: u64,
        minimum_length: u64,
        from_end: bool,
    },
    Subslice {
        from: u64,
        to: u64,
        from_end: bool,
    },
    Downcast(u32),
    OpaqueCast,
    Subtype,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticProjectionV1 {
    kind: SemanticProjectionKindV1,
    result_type: SemanticTypeIdV1,
}

impl SemanticProjectionV1 {
    pub fn new(
        kind: SemanticProjectionKindV1,
        result_type: SemanticTypeIdV1,
    ) -> Result<Self, SemanticMirErrorV1> {
        match kind {
            SemanticProjectionKindV1::ConstantIndex {
                offset,
                minimum_length,
                from_end: _,
            } if offset >= minimum_length => {
                return Err(SemanticMirErrorV1::InvalidProjectionShape);
            }
            SemanticProjectionKindV1::Subslice {
                from,
                to,
                from_end: false,
            } if from > to => {
                return Err(SemanticMirErrorV1::InvalidProjectionShape);
            }
            _ => {}
        }
        Ok(Self { kind, result_type })
    }

    pub const fn kind(self) -> SemanticProjectionKindV1 {
        self.kind
    }

    pub const fn result_type(self) -> SemanticTypeIdV1 {
        self.result_type
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticPlaceV1 {
    local: SemanticLocalIdV1,
    projections: Box<[SemanticProjectionV1]>,
    ty: SemanticTypeIdV1,
}

impl SemanticPlaceV1 {
    pub fn new(
        local: SemanticLocalIdV1,
        projections: Vec<SemanticProjectionV1>,
        ty: SemanticTypeIdV1,
    ) -> Result<Self, SemanticMirErrorV1> {
        enforce_hard(SemanticMirResourceV1::Projections, projections.len())?;
        if projections
            .last()
            .is_some_and(|projection| projection.result_type != ty)
        {
            return Err(SemanticMirErrorV1::InvalidProjectionShape);
        }
        Ok(Self {
            local,
            projections: projections.into_boxed_slice(),
            ty,
        })
    }

    pub const fn local(&self) -> SemanticLocalIdV1 {
        self.local
    }

    pub fn projections(&self) -> &[SemanticProjectionV1] {
        &self.projections
    }

    pub const fn ty(&self) -> SemanticTypeIdV1 {
        self.ty
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticScalarValueV1 {
    bits: u128,
    size_bytes: u8,
}

impl SemanticScalarValueV1 {
    pub fn new(bits: u128, size_bytes: u8) -> Result<Self, SemanticMirErrorV1> {
        if size_bytes == 0 || size_bytes > 16 {
            return Err(SemanticMirErrorV1::InvalidScalarValue);
        }
        let bit_width = u32::from(size_bytes) * 8;
        if bit_width < 128 && bits >= (1_u128 << bit_width) {
            return Err(SemanticMirErrorV1::InvalidScalarValue);
        }
        Ok(Self { bits, size_bytes })
    }

    pub const fn bits(self) -> u128 {
        self.bits
    }

    pub const fn size_bytes(self) -> u8 {
        self.size_bytes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticConstantBytesV1(Box<[u8]>);

impl SemanticConstantBytesV1 {
    pub fn new(bytes: Vec<u8>) -> Result<Self, SemanticMirErrorV1> {
        enforce_hard(SemanticMirResourceV1::ConstantBytes, bytes.len())?;
        Ok(Self(bytes.into_boxed_slice()))
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticPointerProvenanceV1 {
    Allocation(SemanticAllocationIdV1),
    Callable(SemanticCallableIdV1),
    Static(SemanticStaticIdV1),
    ExposedAddress,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticPointerValueMetadataV1 {
    None,
    SliceLength(u64),
    VTable(SemanticVTableIdV1),
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticPointerValueV1 {
    byte_offset: u64,
    provenance: SemanticPointerProvenanceV1,
    metadata: SemanticPointerValueMetadataV1,
}

impl SemanticPointerValueV1 {
    pub const fn new(byte_offset: u64, provenance: SemanticPointerProvenanceV1) -> Self {
        Self::new_with_metadata(
            byte_offset,
            provenance,
            SemanticPointerValueMetadataV1::None,
        )
    }

    pub const fn new_with_metadata(
        byte_offset: u64,
        provenance: SemanticPointerProvenanceV1,
        metadata: SemanticPointerValueMetadataV1,
    ) -> Self {
        Self {
            byte_offset,
            provenance,
            metadata,
        }
    }

    pub const fn byte_offset(self) -> u64 {
        self.byte_offset
    }

    pub const fn provenance(self) -> SemanticPointerProvenanceV1 {
        self.provenance
    }

    pub const fn metadata(self) -> SemanticPointerValueMetadataV1 {
        self.metadata
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SemanticConstantValueV1 {
    ZeroSized,
    Scalar(SemanticScalarValueV1),
    Bytes(SemanticConstantBytesV1),
    Pointer(SemanticPointerValueV1),
    Callable(SemanticCallableIdV1),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticConstantV1 {
    ty: SemanticTypeIdV1,
    value: SemanticConstantValueV1,
}

impl SemanticConstantV1 {
    pub const fn new(ty: SemanticTypeIdV1, value: SemanticConstantValueV1) -> Self {
        Self { ty, value }
    }

    pub const fn ty(&self) -> SemanticTypeIdV1 {
        self.ty
    }

    pub const fn value(&self) -> &SemanticConstantValueV1 {
        &self.value
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SemanticOperandV1 {
    Copy(SemanticPlaceV1),
    Move(SemanticPlaceV1),
    Constant(SemanticConstantV1),
}

impl SemanticOperandV1 {
    pub const fn ty(&self) -> SemanticTypeIdV1 {
        match self {
            Self::Copy(place) | Self::Move(place) => place.ty,
            Self::Constant(constant) => constant.ty,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticRelocationTargetV1 {
    Allocation(SemanticAllocationIdV1),
    Callable(SemanticCallableIdV1),
    Static(SemanticStaticIdV1),
    VTable(SemanticVTableIdV1),
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticRelocationV1 {
    byte_offset: u64,
    width_bytes: u8,
    address_space: u32,
    addend: i64,
    target: SemanticRelocationTargetV1,
}

impl SemanticRelocationV1 {
    pub fn new(
        byte_offset: u64,
        width_bytes: u8,
        addend: i64,
        target: SemanticRelocationTargetV1,
    ) -> Result<Self, SemanticMirErrorV1> {
        Self::new_in_address_space(byte_offset, width_bytes, 0, addend, target)
    }

    pub fn new_in_address_space(
        byte_offset: u64,
        width_bytes: u8,
        address_space: u32,
        addend: i64,
        target: SemanticRelocationTargetV1,
    ) -> Result<Self, SemanticMirErrorV1> {
        if width_bytes == 0 || width_bytes > 16 {
            return Err(SemanticMirErrorV1::InvalidRelocation);
        }
        Ok(Self {
            byte_offset,
            width_bytes,
            address_space,
            addend,
            target,
        })
    }

    pub const fn byte_offset(self) -> u64 {
        self.byte_offset
    }

    pub const fn width_bytes(self) -> u8 {
        self.width_bytes
    }

    pub const fn address_space(self) -> u32 {
        self.address_space
    }

    pub const fn addend(self) -> i64 {
        self.addend
    }

    pub const fn target(self) -> SemanticRelocationTargetV1 {
        self.target
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticAllocationDeclV1 {
    identity: SemanticAllocationIdentityV1,
    address_space: u32,
    bytes: Box<[u8]>,
    initialized_mask: Box<[u8]>,
    alignment_bytes: u64,
    mutable: bool,
    relocations: Box<[SemanticRelocationV1]>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticLinkSymbolV1(Box<[u8]>);

impl SemanticLinkSymbolV1 {
    pub fn new(bytes: Vec<u8>) -> Result<Self, SemanticMirErrorV1> {
        enforce_hard(SemanticMirResourceV1::LinkSymbolBytes, bytes.len())?;
        if bytes.is_empty() || bytes.contains(&0) {
            return Err(SemanticMirErrorV1::InvalidStatic);
        }
        Ok(Self(bytes.into_boxed_slice()))
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

pub const MAX_SEMANTIC_WORKGROUP_THREADS_V1: u32 = 1_024;
pub const MAX_SEMANTIC_RESIDENT_WORKGROUPS_V1: u16 = 64;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticWorkgroupDimensionsV1 {
    dimensions: [u32; 3],
}

impl SemanticWorkgroupDimensionsV1 {
    pub fn new(dimensions: [u32; 3]) -> Result<Self, SemanticMirErrorV1> {
        if dimensions.contains(&0) {
            return Err(SemanticMirErrorV1::InvalidKernelEntry);
        }
        let volume = u64::from(dimensions[0])
            .checked_mul(u64::from(dimensions[1]))
            .and_then(|volume| volume.checked_mul(u64::from(dimensions[2])))
            .ok_or(SemanticMirErrorV1::ArithmeticOverflow {
                resource: SemanticMirResourceV1::ValidationWork,
            })?;
        if volume > u64::from(MAX_SEMANTIC_WORKGROUP_THREADS_V1) {
            return Err(SemanticMirErrorV1::InvalidKernelEntry);
        }
        Ok(Self { dimensions })
    }

    pub const fn as_array(self) -> [u32; 3] {
        self.dimensions
    }

    pub const fn volume(self) -> u32 {
        self.dimensions[0] * self.dimensions[1] * self.dimensions[2]
    }

    const fn contains(self, required: Self) -> bool {
        required.dimensions[0] <= self.dimensions[0]
            && required.dimensions[1] <= self.dimensions[1]
            && required.dimensions[2] <= self.dimensions[2]
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticKernelLaunchBoundsV1 {
    required: Option<SemanticWorkgroupDimensionsV1>,
    maximum: Option<SemanticWorkgroupDimensionsV1>,
    min_workgroups_per_compute_unit: Option<u16>,
}

impl SemanticKernelLaunchBoundsV1 {
    pub fn new(
        required: Option<SemanticWorkgroupDimensionsV1>,
        maximum: Option<SemanticWorkgroupDimensionsV1>,
        min_workgroups_per_compute_unit: Option<u16>,
    ) -> Result<Self, SemanticMirErrorV1> {
        if required.is_none() && maximum.is_none() {
            return Err(SemanticMirErrorV1::InvalidKernelEntry);
        }
        if required
            .zip(maximum)
            .is_some_and(|(required, maximum)| !maximum.contains(required))
            || min_workgroups_per_compute_unit.is_some() && maximum.is_none()
            || min_workgroups_per_compute_unit
                .is_some_and(|count| count == 0 || count > MAX_SEMANTIC_RESIDENT_WORKGROUPS_V1)
        {
            return Err(SemanticMirErrorV1::InvalidKernelEntry);
        }
        Ok(Self {
            required,
            maximum,
            min_workgroups_per_compute_unit,
        })
    }

    pub const fn required(self) -> Option<SemanticWorkgroupDimensionsV1> {
        self.required
    }

    pub const fn maximum(self) -> Option<SemanticWorkgroupDimensionsV1> {
        self.maximum
    }

    pub const fn min_workgroups_per_compute_unit(self) -> Option<u16> {
        self.min_workgroups_per_compute_unit
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticUnsafeAssemblyTargetV1 {
    AmdGpuGfx942,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticUnsafeAssemblyDeclarationV1 {
    target: SemanticUnsafeAssemblyTargetV1,
    operand_bits: u16,
    option_bits: u16,
    effect_bits: u16,
}

impl SemanticUnsafeAssemblyDeclarationV1 {
    pub const OPERAND_MASK: u16 = 0x000f;
    pub const OPTION_MASK: u16 = 0x001f;
    pub const EFFECT_MASK: u16 = 0x007f;
    pub const OPTION_NOMEM: u16 = 0x0001;
    pub const OPTION_READONLY: u16 = 0x0002;
    pub const OPTION_PURE: u16 = 0x0004;
    pub const EFFECT_READ_GLOBAL: u16 = 0x0001;
    pub const EFFECT_WRITE_GLOBAL: u16 = 0x0002;
    pub const EFFECT_READ_WORKGROUP: u16 = 0x0004;
    pub const EFFECT_WRITE_WORKGROUP: u16 = 0x0008;
    pub const EFFECT_ATOMIC: u16 = 0x0010;
    pub const EFFECT_BARRIER: u16 = 0x0020;
    pub const EFFECT_CONTROL_FLOW: u16 = 0x0040;

    pub fn new(
        target: SemanticUnsafeAssemblyTargetV1,
        operand_bits: u16,
        option_bits: u16,
        effect_bits: u16,
    ) -> Result<Self, SemanticMirErrorV1> {
        let writes = Self::EFFECT_WRITE_GLOBAL | Self::EFFECT_WRITE_WORKGROUP | Self::EFFECT_ATOMIC;
        let memory = 0x003f;
        if operand_bits == 0
            || operand_bits & !Self::OPERAND_MASK != 0
            || option_bits & !Self::OPTION_MASK != 0
            || effect_bits & !Self::EFFECT_MASK != 0
            || option_bits & Self::OPTION_NOMEM != 0 && option_bits & Self::OPTION_READONLY != 0
            || option_bits & Self::OPTION_PURE != 0
                && option_bits & (Self::OPTION_NOMEM | Self::OPTION_READONLY) == 0
            || option_bits & Self::OPTION_NOMEM != 0 && effect_bits & memory != 0
            || option_bits & Self::OPTION_READONLY != 0 && effect_bits & writes != 0
            || option_bits & Self::OPTION_PURE != 0 && effect_bits & Self::EFFECT_CONTROL_FLOW != 0
            || effect_bits == 0 && option_bits & Self::OPTION_NOMEM == 0
        {
            return Err(SemanticMirErrorV1::InvalidKernelEntry);
        }
        Ok(Self {
            target,
            operand_bits,
            option_bits,
            effect_bits,
        })
    }

    pub const fn target(self) -> SemanticUnsafeAssemblyTargetV1 {
        self.target
    }

    pub const fn operand_bits(self) -> u16 {
        self.operand_bits
    }

    pub const fn option_bits(self) -> u16 {
        self.option_bits
    }

    pub const fn effect_bits(self) -> u16 {
        self.effect_bits
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticReachableAssemblyV1 {
    blocks: u32,
    operand_bits: u16,
    option_bits: u16,
    effect_bits: u16,
}

impl SemanticReachableAssemblyV1 {
    pub fn new(
        blocks: u32,
        operand_bits: u16,
        option_bits: u16,
        effect_bits: u16,
    ) -> Result<Self, SemanticMirErrorV1> {
        if blocks == 0
            || operand_bits == 0
            || operand_bits & !SemanticUnsafeAssemblyDeclarationV1::OPERAND_MASK != 0
            || option_bits & !SemanticUnsafeAssemblyDeclarationV1::OPTION_MASK != 0
            || effect_bits & !SemanticUnsafeAssemblyDeclarationV1::EFFECT_MASK != 0
        {
            return Err(SemanticMirErrorV1::InvalidKernelEntry);
        }
        Ok(Self {
            blocks,
            operand_bits,
            option_bits,
            effect_bits,
        })
    }

    pub const fn blocks(self) -> u32 {
        self.blocks
    }

    pub const fn operand_bits(self) -> u16 {
        self.operand_bits
    }

    pub const fn option_bits(self) -> u16 {
        self.option_bits
    }

    pub const fn effect_bits(self) -> u16 {
        self.effect_bits
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticKernelSourceContractV1 {
    launch: Option<SemanticKernelLaunchBoundsV1>,
    unsafe_assembly: Option<SemanticUnsafeAssemblyDeclarationV1>,
    reachable_assembly: Option<SemanticReachableAssemblyV1>,
}

impl SemanticKernelSourceContractV1 {
    pub fn new(
        launch: Option<SemanticKernelLaunchBoundsV1>,
        unsafe_assembly: Option<SemanticUnsafeAssemblyDeclarationV1>,
        reachable_assembly: Option<SemanticReachableAssemblyV1>,
    ) -> Result<Self, SemanticMirErrorV1> {
        if unsafe_assembly.is_some() != reachable_assembly.is_some()
            || unsafe_assembly
                .zip(reachable_assembly)
                .is_some_and(|(declaration, reachable)| {
                    declaration.operand_bits != reachable.operand_bits
                        || declaration.option_bits != reachable.option_bits
                        || declaration.effect_bits != reachable.effect_bits
                })
        {
            return Err(SemanticMirErrorV1::InvalidKernelEntry);
        }
        Ok(Self {
            launch,
            unsafe_assembly,
            reachable_assembly,
        })
    }

    pub const fn launch(self) -> Option<SemanticKernelLaunchBoundsV1> {
        self.launch
    }

    pub const fn unsafe_assembly(self) -> Option<SemanticUnsafeAssemblyDeclarationV1> {
        self.unsafe_assembly
    }

    pub const fn reachable_assembly(self) -> Option<SemanticReachableAssemblyV1> {
        self.reachable_assembly
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticKernelEntryV1 {
    export_symbol: SemanticLinkSymbolV1,
    kernel_binding_identity: SemanticKernelBindingIdentityV1,
    source_contract: SemanticKernelSourceContractV1,
}

impl SemanticKernelEntryV1 {
    pub const fn new(
        export_symbol: SemanticLinkSymbolV1,
        kernel_binding_identity: SemanticKernelBindingIdentityV1,
        source_contract: SemanticKernelSourceContractV1,
    ) -> Self {
        Self {
            export_symbol,
            kernel_binding_identity,
            source_contract,
        }
    }

    pub const fn export_symbol(&self) -> &SemanticLinkSymbolV1 {
        &self.export_symbol
    }

    pub const fn kernel_binding_identity(&self) -> SemanticKernelBindingIdentityV1 {
        self.kernel_binding_identity
    }

    pub const fn source_contract(&self) -> SemanticKernelSourceContractV1 {
        self.source_contract
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SemanticFunctionExportV1 {
    Kernel(SemanticKernelEntryV1),
    DeviceFfi { export_symbol: SemanticLinkSymbolV1 },
}

impl SemanticFunctionExportV1 {
    pub const fn export_symbol(&self) -> &SemanticLinkSymbolV1 {
        match self {
            Self::Kernel(entry) => entry.export_symbol(),
            Self::DeviceFfi { export_symbol } => export_symbol,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SemanticStaticDefinitionV1 {
    Defined { initializer: SemanticAllocationIdV1 },
    ExternalRequired { symbol: SemanticLinkSymbolV1 },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticStaticDeclV1 {
    identity: SemanticStaticIdentityV1,
    source: SemanticSourceProvenanceV1,
    ty: SemanticTypeIdV1,
    mutable: bool,
    address_space: u32,
    definition: SemanticStaticDefinitionV1,
    export_symbol: Option<SemanticLinkSymbolV1>,
}

impl SemanticStaticDeclV1 {
    pub const fn new(
        identity: SemanticStaticIdentityV1,
        source: SemanticSourceProvenanceV1,
        ty: SemanticTypeIdV1,
        mutable: bool,
        address_space: u32,
        definition: SemanticStaticDefinitionV1,
    ) -> Self {
        Self {
            identity,
            source,
            ty,
            mutable,
            address_space,
            definition,
            export_symbol: None,
        }
    }

    pub fn with_export_symbol(mut self, export_symbol: SemanticLinkSymbolV1) -> Self {
        self.export_symbol = Some(export_symbol);
        self
    }

    pub const fn identity(&self) -> SemanticStaticIdentityV1 {
        self.identity
    }

    pub const fn source(&self) -> SemanticSourceProvenanceV1 {
        self.source
    }

    pub const fn ty(&self) -> SemanticTypeIdV1 {
        self.ty
    }

    pub const fn is_mutable(&self) -> bool {
        self.mutable
    }

    pub const fn address_space(&self) -> u32 {
        self.address_space
    }

    pub const fn definition(&self) -> &SemanticStaticDefinitionV1 {
        &self.definition
    }

    pub const fn export_symbol(&self) -> Option<&SemanticLinkSymbolV1> {
        self.export_symbol.as_ref()
    }

    fn link_symbol(&self) -> Option<&SemanticLinkSymbolV1> {
        self.export_symbol.as_ref().or(match &self.definition {
            SemanticStaticDefinitionV1::ExternalRequired { symbol } => Some(symbol),
            SemanticStaticDefinitionV1::Defined { .. } => None,
        })
    }
}

/// A materialized rustc vtable terminal retained independently from statics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticVTableHeaderV1 {
    drop_glue: Option<SemanticFunctionIdV1>,
    size_bytes: u64,
    alignment_bytes: u64,
}

impl SemanticVTableHeaderV1 {
    pub fn new(
        drop_glue: Option<SemanticFunctionIdV1>,
        size_bytes: u64,
        alignment_bytes: u64,
    ) -> Result<Self, SemanticMirErrorV1> {
        if !valid_rustc_alignment(alignment_bytes) {
            return Err(SemanticMirErrorV1::InvalidAllocation);
        }
        Ok(Self {
            drop_glue,
            size_bytes,
            alignment_bytes,
        })
    }

    pub const fn drop_glue(&self) -> Option<SemanticFunctionIdV1> {
        self.drop_glue
    }

    pub const fn size_bytes(&self) -> u64 {
        self.size_bytes
    }

    pub const fn alignment_bytes(&self) -> u64 {
        self.alignment_bytes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SemanticVTableSlotV1 {
    Vacant,
    Method(SemanticFunctionIdV1),
    TraitVPtr {
        trait_ref: SemanticTraitRefIdentityV1,
        target: SemanticVTableIdV1,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticVTableTraitIdentityV1 {
    primary_trait_ref: SemanticTraitRefIdentityV1,
    dyn_predicates: Box<[SemanticDynPredicateIdentityV1]>,
}

impl SemanticVTableTraitIdentityV1 {
    pub fn new(
        primary_trait_ref: SemanticTraitRefIdentityV1,
        dyn_predicates: Vec<SemanticDynPredicateIdentityV1>,
    ) -> Result<Self, SemanticMirErrorV1> {
        enforce_hard(SemanticMirResourceV1::Types, dyn_predicates.len())?;
        if dyn_predicates.is_empty() || dyn_predicates.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(SemanticMirErrorV1::InvalidAllocation);
        }
        Ok(Self {
            primary_trait_ref,
            dyn_predicates: dyn_predicates.into_boxed_slice(),
        })
    }

    pub const fn primary_trait_ref(&self) -> SemanticTraitRefIdentityV1 {
        self.primary_trait_ref
    }

    pub fn dyn_predicates(&self) -> &[SemanticDynPredicateIdentityV1] {
        &self.dyn_predicates
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticVTableDeclV1 {
    identity: SemanticVTableIdentityV1,
    concrete_type: SemanticTypeIdV1,
    dyn_type: SemanticTypeIdV1,
    trait_identity: SemanticVTableTraitIdentityV1,
    header: SemanticVTableHeaderV1,
    slots: Box<[SemanticVTableSlotV1]>,
    allocation: SemanticAllocationIdV1,
}

impl SemanticVTableDeclV1 {
    pub fn new(
        identity: SemanticVTableIdentityV1,
        concrete_type: SemanticTypeIdV1,
        dyn_type: SemanticTypeIdV1,
        dyn_predicates: Vec<SemanticDynPredicateIdentityV1>,
        header: SemanticVTableHeaderV1,
        method_slots: Vec<SemanticFunctionIdV1>,
        allocation: SemanticAllocationIdV1,
    ) -> Result<Self, SemanticMirErrorV1> {
        Self::new_with_slots(
            identity,
            concrete_type,
            dyn_type,
            dyn_predicates,
            header,
            method_slots
                .into_iter()
                .map(SemanticVTableSlotV1::Method)
                .collect(),
            allocation,
        )
    }

    pub fn new_with_slots(
        identity: SemanticVTableIdentityV1,
        concrete_type: SemanticTypeIdV1,
        dyn_type: SemanticTypeIdV1,
        dyn_predicates: Vec<SemanticDynPredicateIdentityV1>,
        header: SemanticVTableHeaderV1,
        slots: Vec<SemanticVTableSlotV1>,
        allocation: SemanticAllocationIdV1,
    ) -> Result<Self, SemanticMirErrorV1> {
        let primary_trait_ref = dyn_predicates
            .first()
            .map(|identity| SemanticTraitRefIdentityV1::from_sha256(*identity.as_bytes()))
            .ok_or(SemanticMirErrorV1::InvalidAllocation)?;
        Self::new_with_trait_identity_and_slots(
            identity,
            concrete_type,
            dyn_type,
            SemanticVTableTraitIdentityV1::new(primary_trait_ref, dyn_predicates)?,
            header,
            slots,
            allocation,
        )
    }

    pub fn new_with_trait_identity_and_slots(
        identity: SemanticVTableIdentityV1,
        concrete_type: SemanticTypeIdV1,
        dyn_type: SemanticTypeIdV1,
        trait_identity: SemanticVTableTraitIdentityV1,
        header: SemanticVTableHeaderV1,
        slots: Vec<SemanticVTableSlotV1>,
        allocation: SemanticAllocationIdV1,
    ) -> Result<Self, SemanticMirErrorV1> {
        enforce_hard(SemanticMirResourceV1::Functions, slots.len())?;
        Ok(Self {
            identity,
            concrete_type,
            dyn_type,
            trait_identity,
            header,
            slots: slots.into_boxed_slice(),
            allocation,
        })
    }

    pub const fn identity(&self) -> SemanticVTableIdentityV1 {
        self.identity
    }

    pub const fn concrete_type(&self) -> SemanticTypeIdV1 {
        self.concrete_type
    }

    pub const fn dyn_type(&self) -> SemanticTypeIdV1 {
        self.dyn_type
    }

    pub const fn primary_trait_ref(&self) -> SemanticTraitRefIdentityV1 {
        self.trait_identity.primary_trait_ref()
    }

    pub fn dyn_predicates(&self) -> &[SemanticDynPredicateIdentityV1] {
        self.trait_identity.dyn_predicates()
    }

    pub const fn header(&self) -> &SemanticVTableHeaderV1 {
        &self.header
    }

    pub fn slots(&self) -> &[SemanticVTableSlotV1] {
        &self.slots
    }

    pub const fn allocation(&self) -> SemanticAllocationIdV1 {
        self.allocation
    }
}

impl SemanticAllocationDeclV1 {
    pub fn new(
        identity: SemanticAllocationIdentityV1,
        bytes: Vec<u8>,
        initialized_mask: Vec<u8>,
        alignment_bytes: u64,
        mutable: bool,
        relocations: Vec<SemanticRelocationV1>,
    ) -> Result<Self, SemanticMirErrorV1> {
        Self::new_in_address_space(
            identity,
            0,
            bytes,
            initialized_mask,
            alignment_bytes,
            mutable,
            relocations,
        )
    }

    pub fn new_in_address_space(
        identity: SemanticAllocationIdentityV1,
        address_space: u32,
        bytes: Vec<u8>,
        initialized_mask: Vec<u8>,
        alignment_bytes: u64,
        mutable: bool,
        relocations: Vec<SemanticRelocationV1>,
    ) -> Result<Self, SemanticMirErrorV1> {
        enforce_hard(SemanticMirResourceV1::ConstantBytes, bytes.len())?;
        enforce_hard(SemanticMirResourceV1::Relocations, relocations.len())?;
        let mask_len =
            bytes
                .len()
                .checked_add(7)
                .ok_or(SemanticMirErrorV1::ArithmeticOverflow {
                    resource: SemanticMirResourceV1::ConstantBytes,
                })?
                / 8;
        if initialized_mask.len() != mask_len
            || !valid_rustc_alignment(alignment_bytes)
            || initialized_mask
                .last()
                .is_some_and(|last| mask_len * 8 != bytes.len() && *last >> (bytes.len() % 8) != 0)
        {
            return Err(SemanticMirErrorV1::InvalidAllocation);
        }
        let mut previous_end = 0_u64;
        for relocation in &relocations {
            let end = relocation
                .byte_offset
                .checked_add(u64::from(relocation.width_bytes))
                .ok_or(SemanticMirErrorV1::ArithmeticOverflow {
                    resource: SemanticMirResourceV1::Relocations,
                })?;
            if relocation.byte_offset < previous_end || end > bytes.len() as u64 {
                return Err(SemanticMirErrorV1::InvalidRelocation);
            }
            previous_end = end;
        }
        Ok(Self {
            identity,
            address_space,
            bytes: bytes.into_boxed_slice(),
            initialized_mask: initialized_mask.into_boxed_slice(),
            alignment_bytes,
            mutable,
            relocations: relocations.into_boxed_slice(),
        })
    }

    pub const fn address_space(&self) -> u32 {
        self.address_space
    }

    pub const fn identity(&self) -> SemanticAllocationIdentityV1 {
        self.identity
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn initialized_mask(&self) -> &[u8] {
        &self.initialized_mask
    }

    pub const fn alignment_bytes(&self) -> u64 {
        self.alignment_bytes
    }

    pub const fn is_mutable(&self) -> bool {
        self.mutable
    }

    pub fn relocations(&self) -> &[SemanticRelocationV1] {
        &self.relocations
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticUnaryOpV1 {
    Not,
    Negate,
    PointerMetadata,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticBinaryOpV1 {
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
    BitXor,
    BitAnd,
    BitOr,
    ShiftLeft,
    ShiftRight,
    Equal,
    LessThan,
    LessOrEqual,
    NotEqual,
    GreaterOrEqual,
    GreaterThan,
    Offset,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticCastKindV1 {
    Integer,
    Float,
    Pointer,
    PointerExposeProvenance,
    PointerWithExposedProvenance,
    Transmute,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticBorrowKindV1 {
    Shared,
    Mutable,
    Fake,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticVolatilityV1 {
    NonVolatile,
    Volatile,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticAtomicOrderingV1 {
    Relaxed,
    Release,
    Acquire,
    AcquireRelease,
    SequentiallyConsistent,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticAtomicScopeV1 {
    SingleThread,
    Workgroup,
    Agent,
    Device,
    System,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticAtomicAccessV1 {
    ordering: SemanticAtomicOrderingV1,
    scope: SemanticAtomicScopeV1,
}

impl SemanticAtomicAccessV1 {
    pub const fn new(ordering: SemanticAtomicOrderingV1, scope: SemanticAtomicScopeV1) -> Self {
        Self { ordering, scope }
    }

    pub const fn ordering(self) -> SemanticAtomicOrderingV1 {
        self.ordering
    }

    pub const fn scope(self) -> SemanticAtomicScopeV1 {
        self.scope
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticMemoryLoadV1 {
    source: SemanticPlaceV1,
    volatility: SemanticVolatilityV1,
    atomic: Option<SemanticAtomicAccessV1>,
}

impl SemanticMemoryLoadV1 {
    pub const fn new(
        source: SemanticPlaceV1,
        volatility: SemanticVolatilityV1,
        atomic: Option<SemanticAtomicAccessV1>,
    ) -> Self {
        Self {
            source,
            volatility,
            atomic,
        }
    }

    pub const fn source(&self) -> &SemanticPlaceV1 {
        &self.source
    }

    pub const fn volatility(&self) -> SemanticVolatilityV1 {
        self.volatility
    }

    pub const fn atomic(&self) -> Option<SemanticAtomicAccessV1> {
        self.atomic
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SemanticAggregateKindV1 {
    Array,
    Tuple,
    Aggregate,
    EnumVariant(u32),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticAggregateRvalueV1 {
    kind: SemanticAggregateKindV1,
    operands: Box<[SemanticOperandV1]>,
}

impl SemanticAggregateRvalueV1 {
    pub fn new(
        kind: SemanticAggregateKindV1,
        operands: Vec<SemanticOperandV1>,
    ) -> Result<Self, SemanticMirErrorV1> {
        enforce_hard(SemanticMirResourceV1::Operands, operands.len())?;
        Ok(Self {
            kind,
            operands: operands.into_boxed_slice(),
        })
    }

    pub const fn kind(&self) -> &SemanticAggregateKindV1 {
        &self.kind
    }

    pub fn operands(&self) -> &[SemanticOperandV1] {
        &self.operands
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SemanticRvalueKindV1 {
    Use(SemanticOperandV1),
    Unary {
        operation: SemanticUnaryOpV1,
        operand: SemanticOperandV1,
    },
    Binary {
        operation: SemanticBinaryOpV1,
        left: SemanticOperandV1,
        right: SemanticOperandV1,
    },
    Cast {
        kind: SemanticCastKindV1,
        operand: SemanticOperandV1,
    },
    Borrow {
        kind: SemanticBorrowKindV1,
        place: SemanticPlaceV1,
    },
    AddressOf {
        mutability: SemanticMutabilityV1,
        place: SemanticPlaceV1,
    },
    Length(SemanticPlaceV1),
    Discriminant(SemanticPlaceV1),
    Aggregate(SemanticAggregateRvalueV1),
    Load(SemanticMemoryLoadV1),
}

impl SemanticRvalueKindV1 {
    pub fn aggregate(
        kind: SemanticAggregateKindV1,
        operands: Vec<SemanticOperandV1>,
    ) -> Result<Self, SemanticMirErrorV1> {
        Ok(Self::Aggregate(SemanticAggregateRvalueV1::new(
            kind, operands,
        )?))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticRvalueV1 {
    result_type: SemanticTypeIdV1,
    kind: SemanticRvalueKindV1,
}

impl SemanticRvalueV1 {
    pub const fn new(result_type: SemanticTypeIdV1, kind: SemanticRvalueKindV1) -> Self {
        Self { result_type, kind }
    }

    pub const fn result_type(&self) -> SemanticTypeIdV1 {
        self.result_type
    }

    pub const fn kind(&self) -> &SemanticRvalueKindV1 {
        &self.kind
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticAssignmentV1 {
    destination: SemanticPlaceV1,
    value: SemanticRvalueV1,
}

impl SemanticAssignmentV1 {
    pub const fn new(destination: SemanticPlaceV1, value: SemanticRvalueV1) -> Self {
        Self { destination, value }
    }

    pub const fn destination(&self) -> &SemanticPlaceV1 {
        &self.destination
    }

    pub const fn value(&self) -> &SemanticRvalueV1 {
        &self.value
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticMemoryStoreV1 {
    destination: SemanticPlaceV1,
    value: SemanticOperandV1,
    volatility: SemanticVolatilityV1,
    atomic: Option<SemanticAtomicAccessV1>,
}

impl SemanticMemoryStoreV1 {
    pub const fn new(
        destination: SemanticPlaceV1,
        value: SemanticOperandV1,
        volatility: SemanticVolatilityV1,
        atomic: Option<SemanticAtomicAccessV1>,
    ) -> Self {
        Self {
            destination,
            value,
            volatility,
            atomic,
        }
    }

    pub const fn destination(&self) -> &SemanticPlaceV1 {
        &self.destination
    }

    pub const fn value(&self) -> &SemanticOperandV1 {
        &self.value
    }

    pub const fn volatility(&self) -> SemanticVolatilityV1 {
        self.volatility
    }

    pub const fn atomic(&self) -> Option<SemanticAtomicAccessV1> {
        self.atomic
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticAtomicRmwOpV1 {
    Exchange,
    Add,
    Subtract,
    BitAnd,
    BitNand,
    BitOr,
    BitXor,
    SignedMaximum,
    SignedMinimum,
    UnsignedMaximum,
    UnsignedMinimum,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticAtomicRmwV1 {
    destination: SemanticPlaceV1,
    address: SemanticPlaceV1,
    value: SemanticOperandV1,
    operation: SemanticAtomicRmwOpV1,
    access: SemanticAtomicAccessV1,
}

impl SemanticAtomicRmwV1 {
    pub const fn new(
        destination: SemanticPlaceV1,
        address: SemanticPlaceV1,
        value: SemanticOperandV1,
        operation: SemanticAtomicRmwOpV1,
        access: SemanticAtomicAccessV1,
    ) -> Self {
        Self {
            destination,
            address,
            value,
            operation,
            access,
        }
    }

    pub const fn destination(&self) -> &SemanticPlaceV1 {
        &self.destination
    }

    pub const fn address(&self) -> &SemanticPlaceV1 {
        &self.address
    }

    pub const fn value(&self) -> &SemanticOperandV1 {
        &self.value
    }

    pub const fn operation(&self) -> SemanticAtomicRmwOpV1 {
        self.operation
    }

    pub const fn access(&self) -> SemanticAtomicAccessV1 {
        self.access
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticAtomicCompareExchangeV1 {
    destination: SemanticPlaceV1,
    address: SemanticPlaceV1,
    expected: SemanticOperandV1,
    replacement: SemanticOperandV1,
    success: SemanticAtomicAccessV1,
    failure_ordering: SemanticAtomicOrderingV1,
    weak: bool,
}

impl SemanticAtomicCompareExchangeV1 {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        destination: SemanticPlaceV1,
        address: SemanticPlaceV1,
        expected: SemanticOperandV1,
        replacement: SemanticOperandV1,
        success: SemanticAtomicAccessV1,
        failure_ordering: SemanticAtomicOrderingV1,
        weak: bool,
    ) -> Self {
        Self {
            destination,
            address,
            expected,
            replacement,
            success,
            failure_ordering,
            weak,
        }
    }

    pub const fn destination(&self) -> &SemanticPlaceV1 {
        &self.destination
    }

    pub const fn address(&self) -> &SemanticPlaceV1 {
        &self.address
    }

    pub const fn expected(&self) -> &SemanticOperandV1 {
        &self.expected
    }

    pub const fn replacement(&self) -> &SemanticOperandV1 {
        &self.replacement
    }

    pub const fn success(&self) -> SemanticAtomicAccessV1 {
        self.success
    }

    pub const fn failure_ordering(&self) -> SemanticAtomicOrderingV1 {
        self.failure_ordering
    }

    pub const fn is_weak(&self) -> bool {
        self.weak
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SemanticStatementKindV1 {
    Assign(SemanticAssignmentV1),
    Store(SemanticMemoryStoreV1),
    AtomicRmw(SemanticAtomicRmwV1),
    AtomicCompareExchange(SemanticAtomicCompareExchangeV1),
    SetDiscriminant {
        place: SemanticPlaceV1,
        variant_index: u32,
    },
    Deinitialize(SemanticPlaceV1),
    StorageLive(SemanticLocalIdV1),
    StorageDead(SemanticLocalIdV1),
    Nop,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticStatementV1 {
    source: SemanticSourceProvenanceV1,
    kind: SemanticStatementKindV1,
}

impl SemanticStatementV1 {
    pub const fn new(source: SemanticSourceProvenanceV1, kind: SemanticStatementKindV1) -> Self {
        Self { source, kind }
    }

    pub const fn source(&self) -> SemanticSourceProvenanceV1 {
        self.source
    }

    pub const fn kind(&self) -> &SemanticStatementKindV1 {
        &self.kind
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticEdgeRoleV1 {
    Goto,
    SwitchValue,
    SwitchOtherwise,
    CallReturn,
    CallUnwind,
    TailCallUnwind,
    DropReturn,
    DropUnwind,
    AssertSuccess,
    AssertUnwind,
    FalseEdgeReal,
    FalseEdgeImaginary,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticControlFlowEdgeV1 {
    role: SemanticEdgeRoleV1,
    target: SemanticBlockIdV1,
}

impl SemanticControlFlowEdgeV1 {
    pub const fn new(role: SemanticEdgeRoleV1, target: SemanticBlockIdV1) -> Self {
        Self { role, target }
    }

    pub const fn role(self) -> SemanticEdgeRoleV1 {
        self.role
    }

    pub const fn target(self) -> SemanticBlockIdV1 {
        self.target
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticUnwindActionV1 {
    Continue,
    Unreachable,
    Terminate,
    Cleanup(SemanticControlFlowEdgeV1),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticCallDestinationV1 {
    place: SemanticPlaceV1,
    edge: SemanticControlFlowEdgeV1,
}

impl SemanticCallDestinationV1 {
    pub const fn new(place: SemanticPlaceV1, edge: SemanticControlFlowEdgeV1) -> Self {
        Self { place, edge }
    }

    pub const fn place(&self) -> &SemanticPlaceV1 {
        &self.place
    }

    pub const fn edge(&self) -> SemanticControlFlowEdgeV1 {
        self.edge
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticDirectCallV1 {
    callee: SemanticCallableIdV1,
    arguments: Box<[SemanticOperandV1]>,
    variadic_argument_abis: Box<[SemanticAbiValueV1]>,
    destination: Option<SemanticCallDestinationV1>,
    unwind: SemanticUnwindActionV1,
}

impl SemanticDirectCallV1 {
    pub fn new(
        callee: SemanticFunctionIdV1,
        arguments: Vec<SemanticOperandV1>,
        destination: Option<SemanticCallDestinationV1>,
        unwind: SemanticUnwindActionV1,
    ) -> Result<Self, SemanticMirErrorV1> {
        Self::new_callable_with_variadic_argument_abis(
            SemanticCallableIdV1(callee.0),
            arguments,
            vec![],
            destination,
            unwind,
        )
    }

    pub fn new_with_variadic_argument_abis(
        callee: SemanticFunctionIdV1,
        arguments: Vec<SemanticOperandV1>,
        variadic_argument_abis: Vec<SemanticAbiValueV1>,
        destination: Option<SemanticCallDestinationV1>,
        unwind: SemanticUnwindActionV1,
    ) -> Result<Self, SemanticMirErrorV1> {
        Self::new_callable_with_variadic_argument_abis(
            SemanticCallableIdV1(callee.0),
            arguments,
            variadic_argument_abis,
            destination,
            unwind,
        )
    }

    pub fn new_callable(
        callee: SemanticCallableIdV1,
        arguments: Vec<SemanticOperandV1>,
        destination: Option<SemanticCallDestinationV1>,
        unwind: SemanticUnwindActionV1,
    ) -> Result<Self, SemanticMirErrorV1> {
        Self::new_callable_with_variadic_argument_abis(
            callee,
            arguments,
            vec![],
            destination,
            unwind,
        )
    }

    pub fn new_callable_with_variadic_argument_abis(
        callee: SemanticCallableIdV1,
        arguments: Vec<SemanticOperandV1>,
        variadic_argument_abis: Vec<SemanticAbiValueV1>,
        destination: Option<SemanticCallDestinationV1>,
        unwind: SemanticUnwindActionV1,
    ) -> Result<Self, SemanticMirErrorV1> {
        enforce_hard(SemanticMirResourceV1::CallArguments, arguments.len())?;
        enforce_hard(
            SemanticMirResourceV1::CallArguments,
            variadic_argument_abis.len(),
        )?;
        Ok(Self {
            callee,
            arguments: arguments.into_boxed_slice(),
            variadic_argument_abis: variadic_argument_abis.into_boxed_slice(),
            destination,
            unwind,
        })
    }

    pub const fn callee(&self) -> SemanticCallableIdV1 {
        self.callee
    }

    pub fn arguments(&self) -> &[SemanticOperandV1] {
        &self.arguments
    }

    pub fn variadic_argument_abis(&self) -> &[SemanticAbiValueV1] {
        &self.variadic_argument_abis
    }

    pub const fn destination(&self) -> Option<&SemanticCallDestinationV1> {
        self.destination.as_ref()
    }

    pub const fn unwind(&self) -> SemanticUnwindActionV1 {
        self.unwind
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticDirectTailCallV1 {
    callee: SemanticCallableIdV1,
    arguments: Box<[SemanticOperandV1]>,
    unwind: SemanticUnwindActionV1,
}

impl SemanticDirectTailCallV1 {
    pub fn new(
        callee: SemanticFunctionIdV1,
        arguments: Vec<SemanticOperandV1>,
        unwind: SemanticUnwindActionV1,
    ) -> Result<Self, SemanticMirErrorV1> {
        Self::new_callable(SemanticCallableIdV1(callee.0), arguments, unwind)
    }

    pub fn new_callable(
        callee: SemanticCallableIdV1,
        arguments: Vec<SemanticOperandV1>,
        unwind: SemanticUnwindActionV1,
    ) -> Result<Self, SemanticMirErrorV1> {
        enforce_hard(SemanticMirResourceV1::CallArguments, arguments.len())?;
        Ok(Self {
            callee,
            arguments: arguments.into_boxed_slice(),
            unwind,
        })
    }

    pub const fn callee(&self) -> SemanticCallableIdV1 {
        self.callee
    }

    pub fn arguments(&self) -> &[SemanticOperandV1] {
        &self.arguments
    }

    pub const fn unwind(&self) -> SemanticUnwindActionV1 {
        self.unwind
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticSwitchTargetV1 {
    value: u128,
    edge: SemanticControlFlowEdgeV1,
}

impl SemanticSwitchTargetV1 {
    pub const fn new(value: u128, edge: SemanticControlFlowEdgeV1) -> Self {
        Self { value, edge }
    }

    pub const fn value(self) -> u128 {
        self.value
    }

    pub const fn edge(self) -> SemanticControlFlowEdgeV1 {
        self.edge
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticSwitchTargetsV1 {
    values: Box<[SemanticSwitchTargetV1]>,
    otherwise: SemanticControlFlowEdgeV1,
}

impl SemanticSwitchTargetsV1 {
    pub fn new(
        values: Vec<SemanticSwitchTargetV1>,
        otherwise: SemanticControlFlowEdgeV1,
    ) -> Result<Self, SemanticMirErrorV1> {
        enforce_hard(SemanticMirResourceV1::SwitchTargets, values.len())?;
        if values.windows(2).any(|pair| pair[0].value >= pair[1].value) {
            return Err(SemanticMirErrorV1::NonDeterministicOrder {
                entity: SemanticMirEntityV1::SwitchTarget,
            });
        }
        Ok(Self {
            values: values.into_boxed_slice(),
            otherwise,
        })
    }

    pub fn values(&self) -> &[SemanticSwitchTargetV1] {
        &self.values
    }

    pub const fn otherwise(&self) -> SemanticControlFlowEdgeV1 {
        self.otherwise
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SemanticAssertMessageV1 {
    BoundsCheck {
        length: SemanticOperandV1,
        index: SemanticOperandV1,
    },
    Overflow {
        operation: SemanticBinaryOpV1,
        left: SemanticOperandV1,
        right: SemanticOperandV1,
    },
    DivisionByZero(SemanticOperandV1),
    RemainderByZero(SemanticOperandV1),
    MisalignedPointerDereference {
        required_alignment: SemanticOperandV1,
        found_alignment: SemanticOperandV1,
    },
    NullPointerDereference,
    ResumedAfterReturn,
    ResumedAfterPanic,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SemanticTerminatorKindV1 {
    Goto(SemanticControlFlowEdgeV1),
    SwitchInt {
        discriminant: SemanticOperandV1,
        targets: SemanticSwitchTargetsV1,
    },
    Call(SemanticDirectCallV1),
    TailCall(SemanticDirectTailCallV1),
    Drop {
        place: SemanticPlaceV1,
        drop_glue: SemanticFunctionIdV1,
        target: SemanticControlFlowEdgeV1,
        unwind: SemanticUnwindActionV1,
    },
    Assert {
        condition: SemanticOperandV1,
        expected: bool,
        message: SemanticAssertMessageV1,
        target: SemanticControlFlowEdgeV1,
        unwind: SemanticUnwindActionV1,
    },
    FalseEdge {
        real_target: SemanticControlFlowEdgeV1,
        imaginary_target: SemanticControlFlowEdgeV1,
    },
    Return,
    UnwindResume,
    UnwindTerminate,
    Abort,
    Unreachable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticTerminatorV1 {
    source: SemanticSourceProvenanceV1,
    kind: SemanticTerminatorKindV1,
}

impl SemanticTerminatorV1 {
    pub const fn new(source: SemanticSourceProvenanceV1, kind: SemanticTerminatorKindV1) -> Self {
        Self { source, kind }
    }

    pub const fn kind(&self) -> &SemanticTerminatorKindV1 {
        &self.kind
    }

    pub const fn source(&self) -> SemanticSourceProvenanceV1 {
        self.source
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticBasicBlockV1 {
    identity: SemanticBlockIdentityV1,
    source: SemanticSourceProvenanceV1,
    statements: Box<[SemanticStatementV1]>,
    terminator: SemanticTerminatorV1,
}

impl SemanticBasicBlockV1 {
    pub fn new(
        identity: SemanticBlockIdentityV1,
        source: SemanticSourceProvenanceV1,
        statements: Vec<SemanticStatementV1>,
        terminator: SemanticTerminatorV1,
    ) -> Result<Self, SemanticMirErrorV1> {
        enforce_hard(SemanticMirResourceV1::Statements, statements.len())?;
        Ok(Self {
            identity,
            source,
            statements: statements.into_boxed_slice(),
            terminator,
        })
    }

    pub const fn identity(&self) -> SemanticBlockIdentityV1 {
        self.identity
    }

    pub const fn source(&self) -> SemanticSourceProvenanceV1 {
        self.source
    }

    pub fn statements(&self) -> &[SemanticStatementV1] {
        &self.statements
    }

    pub const fn terminator(&self) -> &SemanticTerminatorV1 {
        &self.terminator
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticFunctionRoleV1 {
    KernelRoot,
    InternalHelper,
    DeviceFfiExport,
    DropGlue(SemanticTypeIdV1),
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticAxisV1 {
    X,
    Y,
    Z,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticCompilerIntrinsicOperationV1 {
    ThreadIndex(SemanticAxisV1),
    WorkgroupIndex(SemanticAxisV1),
    WorkgroupDimension(SemanticAxisV1),
    GridDimension(SemanticAxisV1),
    WorkgroupBarrier,
    WaveBarrier,
    FabsF32,
    /// Produces the non-duplicable logical-1D index witness backed by `raw_index`.
    ThreadIndex1d {
        index_witness: SemanticTypeIdV1,
        raw_index: SemanticTypeIdV1,
    },
    /// Extracts the backing integer from an immutable borrow of `index_witness`.
    ThreadIndexGet {
        index_witness: SemanticTypeIdV1,
        raw_index: SemanticTypeIdV1,
    },
    /// Bounds-checks one witness-indexed mutable access to `element`.
    DisjointSliceGetMut {
        disjoint_slice: SemanticTypeIdV1,
        index_witness: SemanticTypeIdV1,
        element: SemanticTypeIdV1,
        raw_index: SemanticTypeIdV1,
    },
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticDeviceFfiTargetV1 {
    AmdGpuGfx942XnackMinus,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticCodeObjectVersionV1 {
    V6,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticDeviceFfiEffectsV1 {
    bits: u16,
}

impl SemanticDeviceFfiEffectsV1 {
    pub const READ_GLOBAL: u16 = 1 << 0;
    pub const WRITE_GLOBAL: u16 = 1 << 1;
    pub const READ_WORKGROUP: u16 = 1 << 2;
    pub const WRITE_WORKGROUP: u16 = 1 << 3;
    pub const ATOMIC: u16 = 1 << 4;
    pub const CONTROL_FLOW: u16 = 1 << 5;
    const MASK: u16 = (1 << 6) - 1;

    pub fn new(bits: u16) -> Result<Self, SemanticMirErrorV1> {
        if bits & !Self::MASK != 0 {
            return Err(SemanticMirErrorV1::InvalidFunctionAbi);
        }
        Ok(Self { bits })
    }

    pub const fn none() -> Self {
        Self { bits: 0 }
    }

    pub const fn bits(self) -> u16 {
        self.bits
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticNonBodyCallableBindingV1 {
    identity: SemanticFunctionIdentityV1,
    item_definition_identity: SemanticItemDefinitionIdentityV1,
    monomorphization_identity: SemanticMonomorphizationIdentityV1,
    generic_type_arguments_identity: SemanticGenericTypeArgumentsIdentityV1,
    const_generic_arguments_identity: SemanticConstGenericArgumentsIdentityV1,
    source: SemanticSourceProvenanceV1,
    abi: SemanticFunctionAbiV1,
}

impl SemanticNonBodyCallableBindingV1 {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        identity: SemanticFunctionIdentityV1,
        item_definition_identity: SemanticItemDefinitionIdentityV1,
        monomorphization_identity: SemanticMonomorphizationIdentityV1,
        generic_type_arguments_identity: SemanticGenericTypeArgumentsIdentityV1,
        const_generic_arguments_identity: SemanticConstGenericArgumentsIdentityV1,
        source: SemanticSourceProvenanceV1,
        abi: SemanticFunctionAbiV1,
    ) -> Self {
        Self {
            identity,
            item_definition_identity,
            monomorphization_identity,
            generic_type_arguments_identity,
            const_generic_arguments_identity,
            source,
            abi,
        }
    }

    pub const fn identity(&self) -> SemanticFunctionIdentityV1 {
        self.identity
    }

    pub const fn abi(&self) -> &SemanticFunctionAbiV1 {
        &self.abi
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticDeviceFfiImportContractV1 {
    contract_identity: SemanticDeviceFfiContractIdentityV1,
    symbol: SemanticLinkSymbolV1,
    target: SemanticDeviceFfiTargetV1,
    code_object_version: SemanticCodeObjectVersionV1,
    physical_abi_identity: SemanticDeviceFfiPhysicalAbiIdentityV1,
    effects: SemanticDeviceFfiEffectsV1,
    semantic_identity: SemanticDeviceFfiSemanticIdentityV1,
}

impl SemanticDeviceFfiImportContractV1 {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        contract_identity: SemanticDeviceFfiContractIdentityV1,
        symbol: SemanticLinkSymbolV1,
        target: SemanticDeviceFfiTargetV1,
        code_object_version: SemanticCodeObjectVersionV1,
        physical_abi_identity: SemanticDeviceFfiPhysicalAbiIdentityV1,
        effects: SemanticDeviceFfiEffectsV1,
        semantic_identity: SemanticDeviceFfiSemanticIdentityV1,
    ) -> Self {
        Self {
            contract_identity,
            symbol,
            target,
            code_object_version,
            physical_abi_identity,
            effects,
            semantic_identity,
        }
    }

    pub const fn symbol(&self) -> &SemanticLinkSymbolV1 {
        &self.symbol
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SemanticCallableDeclV1 {
    Defined {
        function: SemanticFunctionIdV1,
    },
    DeviceFfiImport {
        binding: SemanticNonBodyCallableBindingV1,
        contract: SemanticDeviceFfiImportContractV1,
    },
    CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1,
        operation: SemanticCompilerIntrinsicOperationV1,
        operation_identity: SemanticCompilerIntrinsicIdentityV1,
    },
}

impl SemanticCallableDeclV1 {
    pub const fn defined(function: SemanticFunctionIdV1) -> Self {
        Self::Defined { function }
    }

    pub const fn binding(&self) -> Option<&SemanticNonBodyCallableBindingV1> {
        match self {
            Self::Defined { .. } => None,
            Self::DeviceFfiImport { binding, .. } | Self::CompilerIntrinsic { binding, .. } => {
                Some(binding)
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticFunctionDeclV1 {
    identity: SemanticFunctionIdentityV1,
    role: SemanticFunctionRoleV1,
    export: Option<SemanticFunctionExportV1>,
    item_definition_identity: SemanticItemDefinitionIdentityV1,
    monomorphization_identity: SemanticMonomorphizationIdentityV1,
    generic_type_arguments_identity: SemanticGenericTypeArgumentsIdentityV1,
    const_generic_arguments_identity: SemanticConstGenericArgumentsIdentityV1,
    source: SemanticSourceProvenanceV1,
    abi: SemanticFunctionAbiV1,
    locals: Box<[SemanticLocalDeclV1]>,
    entry: SemanticBlockIdV1,
    blocks: Box<[SemanticBasicBlockV1]>,
}

impl SemanticFunctionDeclV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        identity: SemanticFunctionIdentityV1,
        role: SemanticFunctionRoleV1,
        item_definition_identity: SemanticItemDefinitionIdentityV1,
        monomorphization_identity: SemanticMonomorphizationIdentityV1,
        generic_type_arguments_identity: SemanticGenericTypeArgumentsIdentityV1,
        const_generic_arguments_identity: SemanticConstGenericArgumentsIdentityV1,
        source: SemanticSourceProvenanceV1,
        abi: SemanticFunctionAbiV1,
        locals: Vec<SemanticLocalDeclV1>,
        entry: SemanticBlockIdV1,
        blocks: Vec<SemanticBasicBlockV1>,
    ) -> Result<Self, SemanticMirErrorV1> {
        enforce_hard(SemanticMirResourceV1::Locals, locals.len())?;
        enforce_hard(SemanticMirResourceV1::Blocks, blocks.len())?;
        Ok(Self {
            identity,
            role,
            export: None,
            item_definition_identity,
            monomorphization_identity,
            generic_type_arguments_identity,
            const_generic_arguments_identity,
            source,
            abi,
            locals: locals.into_boxed_slice(),
            entry,
            blocks: blocks.into_boxed_slice(),
        })
    }

    pub const fn identity(&self) -> SemanticFunctionIdentityV1 {
        self.identity
    }

    pub const fn role(&self) -> SemanticFunctionRoleV1 {
        self.role
    }

    pub fn with_role(mut self, role: SemanticFunctionRoleV1) -> Self {
        self.role = role;
        self
    }

    pub fn with_kernel_entry(mut self, kernel_entry: SemanticKernelEntryV1) -> Self {
        self.export = Some(SemanticFunctionExportV1::Kernel(kernel_entry));
        self
    }

    pub const fn kernel_entry(&self) -> Option<&SemanticKernelEntryV1> {
        match &self.export {
            Some(SemanticFunctionExportV1::Kernel(entry)) => Some(entry),
            Some(SemanticFunctionExportV1::DeviceFfi { .. }) | None => None,
        }
    }

    pub fn with_device_ffi_export_symbol(mut self, export_symbol: SemanticLinkSymbolV1) -> Self {
        self.export = Some(SemanticFunctionExportV1::DeviceFfi { export_symbol });
        self
    }

    pub const fn export(&self) -> Option<&SemanticFunctionExportV1> {
        self.export.as_ref()
    }

    pub const fn item_definition_identity(&self) -> SemanticItemDefinitionIdentityV1 {
        self.item_definition_identity
    }

    pub const fn monomorphization_identity(&self) -> SemanticMonomorphizationIdentityV1 {
        self.monomorphization_identity
    }

    pub const fn generic_type_arguments_identity(&self) -> SemanticGenericTypeArgumentsIdentityV1 {
        self.generic_type_arguments_identity
    }

    pub const fn const_generic_arguments_identity(
        &self,
    ) -> SemanticConstGenericArgumentsIdentityV1 {
        self.const_generic_arguments_identity
    }

    pub const fn source(&self) -> SemanticSourceProvenanceV1 {
        self.source
    }

    pub const fn abi(&self) -> &SemanticFunctionAbiV1 {
        &self.abi
    }

    pub fn locals(&self) -> &[SemanticLocalDeclV1] {
        &self.locals
    }

    pub const fn entry(&self) -> SemanticBlockIdV1 {
        self.entry
    }

    pub fn blocks(&self) -> &[SemanticBasicBlockV1] {
        &self.blocks
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InertSemanticMirRequestV1 {
    target: SemanticTargetDataLayoutV1,
    types: Box<[SemanticTypeDeclV1]>,
    allocations: Box<[SemanticAllocationDeclV1]>,
    statics: Box<[SemanticStaticDeclV1]>,
    vtables: Box<[SemanticVTableDeclV1]>,
    functions: Box<[SemanticFunctionDeclV1]>,
    callables: Box<[SemanticCallableDeclV1]>,
    roots: Box<[SemanticFunctionIdV1]>,
}

impl InertSemanticMirRequestV1 {
    pub fn new(
        target: SemanticTargetDataLayoutV1,
        types: Vec<SemanticTypeDeclV1>,
        allocations: Vec<SemanticAllocationDeclV1>,
        statics: Vec<SemanticStaticDeclV1>,
        vtables: Vec<SemanticVTableDeclV1>,
        functions: Vec<SemanticFunctionDeclV1>,
        roots: Vec<SemanticFunctionIdV1>,
    ) -> Result<Self, SemanticMirErrorV1> {
        let callables = (0..functions.len())
            .map(|index| SemanticCallableDeclV1::defined(SemanticFunctionIdV1(index as u32)))
            .collect();
        Self::new_with_callables(
            target,
            types,
            allocations,
            statics,
            vtables,
            functions,
            callables,
            roots,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_with_callables(
        target: SemanticTargetDataLayoutV1,
        types: Vec<SemanticTypeDeclV1>,
        allocations: Vec<SemanticAllocationDeclV1>,
        statics: Vec<SemanticStaticDeclV1>,
        vtables: Vec<SemanticVTableDeclV1>,
        functions: Vec<SemanticFunctionDeclV1>,
        callables: Vec<SemanticCallableDeclV1>,
        roots: Vec<SemanticFunctionIdV1>,
    ) -> Result<Self, SemanticMirErrorV1> {
        enforce_hard(SemanticMirResourceV1::Types, types.len())?;
        enforce_hard(SemanticMirResourceV1::Allocations, allocations.len())?;
        enforce_hard(SemanticMirResourceV1::Statics, statics.len())?;
        enforce_hard(SemanticMirResourceV1::VTables, vtables.len())?;
        enforce_hard(SemanticMirResourceV1::Functions, functions.len())?;
        enforce_hard(SemanticMirResourceV1::Callables, callables.len())?;
        enforce_hard(SemanticMirResourceV1::Roots, roots.len())?;
        if roots.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(SemanticMirErrorV1::NonDeterministicOrder {
                entity: SemanticMirEntityV1::Root,
            });
        }
        Ok(Self {
            target,
            types: types.into_boxed_slice(),
            allocations: allocations.into_boxed_slice(),
            statics: statics.into_boxed_slice(),
            vtables: vtables.into_boxed_slice(),
            functions: functions.into_boxed_slice(),
            callables: callables.into_boxed_slice(),
            roots: roots.into_boxed_slice(),
        })
    }

    pub fn admit(
        self,
        limits: SemanticMirLimitsV1,
    ) -> Result<AdmittedInertSemanticMirV1, SemanticMirErrorV1> {
        validate_request(&self, limits)?;
        let canonical = encode_request(&self, limits)?;
        let semantic_sha256 = InertSemanticMirSha256V1(Sha256::digest(&canonical).into());
        Ok(AdmittedInertSemanticMirV1 {
            request: self,
            canonical: canonical.into_boxed_slice(),
            semantic_sha256,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct InertSemanticMirSha256V1([u8; 32]);

impl InertSemanticMirSha256V1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Move-only owner of schema-validated inert semantic MIR.
///
/// Validation is structural only. This type grants no proof, compiler,
/// artifact, publication, load, or launch authority.
///
/// ```compile_fail
/// use fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1;
/// fn requires_clone<T: Clone>() {}
/// requires_clone::<AdmittedInertSemanticMirV1>();
/// ```
///
/// ```compile_fail
/// use fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1;
/// fn requires_copy<T: Copy>() {}
/// requires_copy::<AdmittedInertSemanticMirV1>();
/// ```
///
/// ```compile_fail
/// use fe2o3_mir_model::semantic_mir_v1::{
///     AdmittedInertSemanticMirV1, InertSemanticMirSha256V1,
/// };
/// let _ = AdmittedInertSemanticMirV1 {
///     request: panic!(),
///     canonical: Box::new([]),
///     semantic_sha256: InertSemanticMirSha256V1([0; 32]),
/// };
/// ```
#[derive(Debug)]
pub struct AdmittedInertSemanticMirV1 {
    request: InertSemanticMirRequestV1,
    canonical: Box<[u8]>,
    semantic_sha256: InertSemanticMirSha256V1,
}

impl AdmittedInertSemanticMirV1 {
    pub const fn target_layout_identity(&self) -> SemanticLayoutIdentityV1 {
        self.request.target.identity
    }

    pub const fn target(&self) -> SemanticTargetDataLayoutV1 {
        self.request.target
    }

    pub fn types(&self) -> &[SemanticTypeDeclV1] {
        &self.request.types
    }

    pub fn allocations(&self) -> &[SemanticAllocationDeclV1] {
        &self.request.allocations
    }

    pub fn statics(&self) -> &[SemanticStaticDeclV1] {
        &self.request.statics
    }

    pub fn vtables(&self) -> &[SemanticVTableDeclV1] {
        &self.request.vtables
    }

    pub fn functions(&self) -> &[SemanticFunctionDeclV1] {
        &self.request.functions
    }

    pub fn callables(&self) -> &[SemanticCallableDeclV1] {
        &self.request.callables
    }

    pub fn roots(&self) -> &[SemanticFunctionIdV1] {
        &self.request.roots
    }

    /// Checks the metadata required before a rustc-owned importer may promote
    /// this inert record into the production transaction. This check grants no
    /// authority and does not authenticate the record's producer.
    pub fn require_complete_external_entries(&self) -> Result<(), SemanticMirErrorV1> {
        if self.request.functions.iter().any(|function| {
            !matches!(
                (function.role, &function.export),
                (
                    SemanticFunctionRoleV1::KernelRoot,
                    Some(SemanticFunctionExportV1::Kernel(_))
                ) | (
                    SemanticFunctionRoleV1::DeviceFfiExport,
                    Some(SemanticFunctionExportV1::DeviceFfi { .. })
                ) | (
                    SemanticFunctionRoleV1::InternalHelper | SemanticFunctionRoleV1::DropGlue(_),
                    None
                )
            ) || matches!(
                &function.export,
                Some(SemanticFunctionExportV1::Kernel(entry))
                    if entry.source_contract.unsafe_assembly.is_some()
            )
        }) {
            Err(SemanticMirErrorV1::InvalidKernelEntry)
        } else {
            Ok(())
        }
    }

    pub fn require_complete_kernel_entries(&self) -> Result<(), SemanticMirErrorV1> {
        self.require_complete_external_entries()?;
        if self
            .request
            .functions
            .iter()
            .any(|function| function.role == SemanticFunctionRoleV1::KernelRoot)
        {
            Ok(())
        } else {
            Err(SemanticMirErrorV1::InvalidKernelEntry)
        }
    }

    /// Returns deterministic V1 bytes. The bytes do not authenticate producers.
    pub fn canonical_encoding(&self) -> &[u8] {
        &self.canonical
    }

    /// Identifies only `canonical_encoding()` under this schema version.
    pub const fn semantic_sha256(&self) -> InertSemanticMirSha256V1 {
        self.semantic_sha256
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticMirEntityV1 {
    Type,
    Function,
    Callable,
    Allocation,
    Static,
    VTable,
    Root,
    Local,
    Block,
    SwitchTarget,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticMirReferenceV1 {
    Type,
    Function,
    Callable,
    Allocation,
    Static,
    VTable,
    Local,
    Block,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticMirLocationV1 {
    Module,
    Type(SemanticTypeIdV1),
    Allocation(SemanticAllocationIdV1),
    Static(SemanticStaticIdV1),
    VTable(SemanticVTableIdV1),
    Root(u32),
    Function(SemanticFunctionIdV1),
    Callable(SemanticCallableIdV1),
    Local {
        function: SemanticFunctionIdV1,
        local: SemanticLocalIdV1,
    },
    Block {
        function: SemanticFunctionIdV1,
        block: SemanticBlockIdV1,
    },
    Statement {
        function: SemanticFunctionIdV1,
        block: SemanticBlockIdV1,
        statement: u32,
    },
    Terminator {
        function: SemanticFunctionIdV1,
        block: SemanticBlockIdV1,
    },
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticAtomicOperationV1 {
    Load,
    Store,
    ReadModifyWrite,
    CompareExchangeFailure,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticTypeOperationV1 {
    Projection,
    Constant,
    Unary,
    Binary,
    Cast,
    Borrow,
    Length,
    Discriminant,
    Aggregate,
    Atomic,
    SetDiscriminant,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SemanticMirErrorV1 {
    LimitExceeded {
        resource: SemanticMirResourceV1,
        actual: u64,
        max: u64,
    },
    ArithmeticOverflow {
        resource: SemanticMirResourceV1,
    },
    InvalidSourceOrigin,
    InvalidTypeLayout,
    InvalidFunctionAbi,
    InvalidPointerWidth,
    InvalidScalarValue,
    InvalidProjectionShape,
    InvalidAllocation,
    InvalidRelocation,
    InvalidStatic,
    InvalidKernelEntry,
    EmptyModel {
        entity: SemanticMirEntityV1,
    },
    NonDeterministicOrder {
        entity: SemanticMirEntityV1,
    },
    DuplicateIdentity {
        entity: SemanticMirEntityV1,
        location: SemanticMirLocationV1,
    },
    InvalidReference {
        reference: SemanticMirReferenceV1,
        index: u32,
        bound: u32,
        location: SemanticMirLocationV1,
    },
    InvalidEdgeRole {
        expected: SemanticEdgeRoleV1,
        actual: SemanticEdgeRoleV1,
        location: SemanticMirLocationV1,
    },
    InvalidLocalRoles {
        function: SemanticFunctionIdV1,
    },
    InvalidFunctionRole {
        function: SemanticFunctionIdV1,
        role: SemanticFunctionRoleV1,
        rooted: bool,
    },
    FunctionOutsideRootClosure {
        function: SemanticFunctionIdV1,
    },
    CallableOutsideRootClosure {
        callable: SemanticCallableIdV1,
    },
    AllocationOutsideRootClosure {
        allocation: SemanticAllocationIdV1,
    },
    StaticOutsideRootClosure {
        static_id: SemanticStaticIdV1,
    },
    VTableOutsideRootClosure {
        vtable: SemanticVTableIdV1,
    },
    TypeOutsideRootClosure {
        ty: SemanticTypeIdV1,
    },
    TypeMismatch {
        expected: SemanticTypeIdV1,
        actual: SemanticTypeIdV1,
        location: SemanticMirLocationV1,
    },
    InvalidCallShape {
        function: SemanticFunctionIdV1,
        tail: bool,
    },
    InvalidAtomicOrdering {
        operation: SemanticAtomicOperationV1,
        ordering: SemanticAtomicOrderingV1,
        location: SemanticMirLocationV1,
    },
    InvalidAtomicCombination {
        location: SemanticMirLocationV1,
    },
    InvalidTypeOperation {
        operation: SemanticTypeOperationV1,
        location: SemanticMirLocationV1,
    },
}

impl fmt::Display for SemanticMirErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LimitExceeded {
                resource,
                actual,
                max,
            } => write!(formatter, "{resource:?} uses {actual}, exceeding {max}"),
            Self::ArithmeticOverflow { resource } => {
                write!(formatter, "checked accounting overflowed for {resource:?}")
            }
            Self::InvalidSourceOrigin => formatter.write_str("source origin is invalid"),
            Self::InvalidTypeLayout => formatter.write_str("type layout is invalid"),
            Self::InvalidFunctionAbi => formatter.write_str("function ABI is invalid"),
            Self::InvalidPointerWidth => formatter.write_str("pointer width is invalid"),
            Self::InvalidScalarValue => formatter.write_str("scalar value is invalid"),
            Self::InvalidProjectionShape => formatter.write_str("projection shape is invalid"),
            Self::InvalidAllocation => formatter.write_str("constant allocation is invalid"),
            Self::InvalidRelocation => formatter.write_str("constant relocation is invalid"),
            Self::InvalidStatic => formatter.write_str("static declaration is invalid"),
            Self::InvalidKernelEntry => formatter.write_str("kernel entry contract is invalid"),
            Self::EmptyModel { entity } => write!(formatter, "model has no {entity:?} records"),
            Self::NonDeterministicOrder { entity } => {
                write!(
                    formatter,
                    "{entity:?} records are not in deterministic order"
                )
            }
            Self::DuplicateIdentity { entity, location } => {
                write!(formatter, "duplicate {entity:?} identity at {location:?}")
            }
            Self::InvalidReference {
                reference,
                index,
                bound,
                location,
            } => write!(
                formatter,
                "{reference:?} reference {index} is outside 0..{bound} at {location:?}"
            ),
            Self::InvalidEdgeRole {
                expected,
                actual,
                location,
            } => write!(
                formatter,
                "edge role {actual:?} does not match {expected:?} at {location:?}"
            ),
            Self::InvalidLocalRoles { function } => {
                write!(formatter, "function {} has invalid local roles", function.0)
            }
            Self::InvalidFunctionRole {
                function,
                role,
                rooted,
            } => write!(
                formatter,
                "function {} has role {role:?} but is {}rooted",
                function.0,
                if *rooted { "" } else { "not " }
            ),
            Self::FunctionOutsideRootClosure { function } => write!(
                formatter,
                "function {} is outside the executable root closure",
                function.0
            ),
            Self::CallableOutsideRootClosure { callable } => write!(
                formatter,
                "callable {} is outside the executable root closure",
                callable.0
            ),
            Self::AllocationOutsideRootClosure { allocation } => write!(
                formatter,
                "allocation {} is outside the executable root closure",
                allocation.0
            ),
            Self::StaticOutsideRootClosure { static_id } => write!(
                formatter,
                "static {} is outside the executable root closure",
                static_id.0
            ),
            Self::VTableOutsideRootClosure { vtable } => write!(
                formatter,
                "vtable {} is outside the executable root closure",
                vtable.0
            ),
            Self::TypeOutsideRootClosure { ty } => {
                write!(
                    formatter,
                    "type {} is outside the semantic root closure",
                    ty.0
                )
            }
            Self::TypeMismatch {
                expected,
                actual,
                location,
            } => write!(
                formatter,
                "type {} does not match expected type {} at {location:?}",
                actual.0, expected.0
            ),
            Self::InvalidCallShape { function, tail } => write!(
                formatter,
                "function {} has an invalid {} call",
                function.0,
                if *tail { "tail" } else { "ordinary" }
            ),
            Self::InvalidAtomicOrdering {
                operation,
                ordering,
                location,
            } => write!(
                formatter,
                "ordering {ordering:?} is invalid for {operation:?} at {location:?}"
            ),
            Self::InvalidAtomicCombination { location } => {
                write!(
                    formatter,
                    "memory access combination is invalid at {location:?}"
                )
            }
            Self::InvalidTypeOperation {
                operation,
                location,
            } => write!(
                formatter,
                "types are invalid for {operation:?} at {location:?}"
            ),
        }
    }
}

impl std::error::Error for SemanticMirErrorV1 {}

fn enforce_hard(resource: SemanticMirResourceV1, actual: usize) -> Result<(), SemanticMirErrorV1> {
    let actual =
        u64::try_from(actual).map_err(|_| SemanticMirErrorV1::ArithmeticOverflow { resource })?;
    let max = resource.hard_max();
    if actual > max {
        return Err(SemanticMirErrorV1::LimitExceeded {
            resource,
            actual,
            max,
        });
    }
    Ok(())
}

#[derive(Default)]
struct ValidationTotalsV1 {
    locals: u64,
    blocks: u64,
    statements: u64,
    projections: u64,
    operands: u64,
    call_arguments: u64,
    switch_targets: u64,
    relocations: u64,
    constant_bytes: u64,
    link_symbol_bytes: u64,
}

impl ValidationTotalsV1 {
    fn charge(
        &mut self,
        resource: SemanticMirResourceV1,
        amount: usize,
        limits: SemanticMirLimitsV1,
    ) -> Result<(), SemanticMirErrorV1> {
        let slot = match resource {
            SemanticMirResourceV1::Locals => &mut self.locals,
            SemanticMirResourceV1::Blocks => &mut self.blocks,
            SemanticMirResourceV1::Statements => &mut self.statements,
            SemanticMirResourceV1::Projections => &mut self.projections,
            SemanticMirResourceV1::Operands => &mut self.operands,
            SemanticMirResourceV1::CallArguments => &mut self.call_arguments,
            SemanticMirResourceV1::SwitchTargets => &mut self.switch_targets,
            SemanticMirResourceV1::Relocations => &mut self.relocations,
            SemanticMirResourceV1::ConstantBytes => &mut self.constant_bytes,
            SemanticMirResourceV1::LinkSymbolBytes => &mut self.link_symbol_bytes,
            _ => {
                return Err(SemanticMirErrorV1::ArithmeticOverflow { resource });
            }
        };
        *slot = slot
            .checked_add(
                u64::try_from(amount)
                    .map_err(|_| SemanticMirErrorV1::ArithmeticOverflow { resource })?,
            )
            .ok_or(SemanticMirErrorV1::ArithmeticOverflow { resource })?;
        let max = limits.limit(resource);
        if *slot > max {
            return Err(SemanticMirErrorV1::LimitExceeded {
                resource,
                actual: *slot,
                max,
            });
        }
        Ok(())
    }
}

struct ValidationContextV1<'a> {
    request: &'a InertSemanticMirRequestV1,
    limits: SemanticMirLimitsV1,
    totals: ValidationTotalsV1,
    work: u64,
}

impl<'a> ValidationContextV1<'a> {
    fn one(&mut self) -> Result<(), SemanticMirErrorV1> {
        self.work = self
            .work
            .checked_add(1)
            .ok_or(SemanticMirErrorV1::ArithmeticOverflow {
                resource: SemanticMirResourceV1::ValidationWork,
            })?;
        let max = self.limits.limit(SemanticMirResourceV1::ValidationWork);
        if self.work > max {
            return Err(SemanticMirErrorV1::LimitExceeded {
                resource: SemanticMirResourceV1::ValidationWork,
                actual: self.work,
                max,
            });
        }
        Ok(())
    }

    fn reference(
        &mut self,
        reference: SemanticMirReferenceV1,
        index: u32,
        bound: usize,
        location: SemanticMirLocationV1,
    ) -> Result<(), SemanticMirErrorV1> {
        self.one()?;
        if index as usize >= bound {
            return Err(SemanticMirErrorV1::InvalidReference {
                reference,
                index,
                bound: u32::try_from(bound).unwrap_or(u32::MAX),
                location,
            });
        }
        Ok(())
    }

    fn type_reference(
        &mut self,
        ty: SemanticTypeIdV1,
        location: SemanticMirLocationV1,
    ) -> Result<(), SemanticMirErrorV1> {
        self.reference(
            SemanticMirReferenceV1::Type,
            ty.0,
            self.request.types.len(),
            location,
        )
    }

    fn function_reference(
        &mut self,
        function: SemanticFunctionIdV1,
        location: SemanticMirLocationV1,
    ) -> Result<(), SemanticMirErrorV1> {
        self.reference(
            SemanticMirReferenceV1::Function,
            function.0,
            self.request.functions.len(),
            location,
        )
    }

    fn callable_reference(
        &mut self,
        callable: SemanticCallableIdV1,
        location: SemanticMirLocationV1,
    ) -> Result<(), SemanticMirErrorV1> {
        self.reference(
            SemanticMirReferenceV1::Callable,
            callable.0,
            self.request.callables.len(),
            location,
        )
    }

    fn allocation_reference(
        &mut self,
        allocation: SemanticAllocationIdV1,
        location: SemanticMirLocationV1,
    ) -> Result<(), SemanticMirErrorV1> {
        self.reference(
            SemanticMirReferenceV1::Allocation,
            allocation.0,
            self.request.allocations.len(),
            location,
        )
    }

    fn static_reference(
        &mut self,
        static_id: SemanticStaticIdV1,
        location: SemanticMirLocationV1,
    ) -> Result<(), SemanticMirErrorV1> {
        self.reference(
            SemanticMirReferenceV1::Static,
            static_id.0,
            self.request.statics.len(),
            location,
        )
    }

    fn vtable_reference(
        &mut self,
        vtable: SemanticVTableIdV1,
        location: SemanticMirLocationV1,
    ) -> Result<(), SemanticMirErrorV1> {
        self.reference(
            SemanticMirReferenceV1::VTable,
            vtable.0,
            self.request.vtables.len(),
            location,
        )
    }
}

fn validate_request(
    request: &InertSemanticMirRequestV1,
    limits: SemanticMirLimitsV1,
) -> Result<(), SemanticMirErrorV1> {
    if request.target.object_size_bound_bytes == 0
        || !request.target.object_size_bound_bytes.is_power_of_two()
    {
        return Err(SemanticMirErrorV1::InvalidTypeLayout);
    }
    enforce_count(SemanticMirResourceV1::Types, request.types.len(), limits)?;
    enforce_count(
        SemanticMirResourceV1::Functions,
        request.functions.len(),
        limits,
    )?;
    enforce_count(
        SemanticMirResourceV1::Callables,
        request.callables.len(),
        limits,
    )?;
    enforce_count(
        SemanticMirResourceV1::Allocations,
        request.allocations.len(),
        limits,
    )?;
    enforce_count(
        SemanticMirResourceV1::Statics,
        request.statics.len(),
        limits,
    )?;
    enforce_count(
        SemanticMirResourceV1::VTables,
        request.vtables.len(),
        limits,
    )?;
    enforce_count(SemanticMirResourceV1::Roots, request.roots.len(), limits)?;
    if request.types.is_empty() {
        return Err(SemanticMirErrorV1::EmptyModel {
            entity: SemanticMirEntityV1::Type,
        });
    }
    if request.functions.is_empty() {
        return Err(SemanticMirErrorV1::EmptyModel {
            entity: SemanticMirEntityV1::Function,
        });
    }
    if request.callables.is_empty() {
        return Err(SemanticMirErrorV1::EmptyModel {
            entity: SemanticMirEntityV1::Callable,
        });
    }
    if request.roots.is_empty() {
        return Err(SemanticMirErrorV1::EmptyModel {
            entity: SemanticMirEntityV1::Root,
        });
    }
    let mut context = ValidationContextV1 {
        request,
        limits,
        totals: ValidationTotalsV1::default(),
        work: 0,
    };
    let identity_order_work = request
        .types
        .len()
        .checked_add(request.allocations.len())
        .and_then(|total| total.checked_add(request.statics.len()))
        .and_then(|total| total.checked_add(request.vtables.len()))
        .and_then(|total| total.checked_add(request.functions.len()))
        .and_then(|total| total.checked_add(request.callables.len()))
        .ok_or(SemanticMirErrorV1::ArithmeticOverflow {
            resource: SemanticMirResourceV1::ValidationWork,
        })?;
    charge_validation_work(&mut context, identity_order_work)?;
    ensure_identity_order(
        request.types.iter().map(|record| record.identity.0),
        SemanticMirEntityV1::Type,
    )?;
    ensure_identity_order(
        request.allocations.iter().map(|record| record.identity.0),
        SemanticMirEntityV1::Allocation,
    )?;
    ensure_identity_order(
        request.statics.iter().map(|record| record.identity.0),
        SemanticMirEntityV1::Static,
    )?;
    ensure_identity_order(
        request.vtables.iter().map(|record| record.identity.0),
        SemanticMirEntityV1::VTable,
    )?;
    ensure_identity_order(
        request.functions.iter().map(|record| record.identity.0),
        SemanticMirEntityV1::Function,
    )?;
    let adjusted_layout_count = request
        .functions
        .iter()
        .try_fold(0_usize, |total, function| {
            total
                .checked_add(function.abi.arguments.len())
                .and_then(|total| total.checked_add(1))
                .ok_or(SemanticMirErrorV1::ArithmeticOverflow {
                    resource: SemanticMirResourceV1::ValidationWork,
                })
        })?
        .checked_add(
            request
                .callables
                .iter()
                .filter_map(SemanticCallableDeclV1::binding)
                .try_fold(0_usize, |total, binding| {
                    total
                        .checked_add(binding.abi.arguments.len())
                        .and_then(|total| total.checked_add(1))
                        .ok_or(SemanticMirErrorV1::ArithmeticOverflow {
                            resource: SemanticMirResourceV1::ValidationWork,
                        })
                })?,
        )
        .ok_or(SemanticMirErrorV1::ArithmeticOverflow {
            resource: SemanticMirResourceV1::ValidationWork,
        })?;
    charge_validation_work(
        &mut context,
        request
            .types
            .len()
            .checked_add(adjusted_layout_count)
            .ok_or(SemanticMirErrorV1::ArithmeticOverflow {
                resource: SemanticMirResourceV1::ValidationWork,
            })?,
    )?;
    let mut layouts_by_identity = BTreeMap::new();
    for ty in &request.types {
        if let Some(previous) = layouts_by_identity.insert(ty.layout_identity, &ty.layout)
            && previous != &ty.layout
        {
            return Err(SemanticMirErrorV1::InvalidTypeLayout);
        }
    }
    for function in &request.functions {
        for value in function
            .abi
            .arguments
            .iter()
            .map(|argument| &argument.value)
            .chain(std::iter::once(&function.abi.return_value))
        {
            if let Some(adjusted) = value.adjusted()
                && let Some(previous) =
                    layouts_by_identity.insert(adjusted.layout_identity, &adjusted.layout)
                && previous != &adjusted.layout
            {
                return Err(SemanticMirErrorV1::InvalidTypeLayout);
            }
        }
    }
    for binding in request
        .callables
        .iter()
        .filter_map(SemanticCallableDeclV1::binding)
    {
        for value in binding
            .abi
            .arguments
            .iter()
            .map(|argument| &argument.value)
            .chain(std::iter::once(&binding.abi.return_value))
        {
            if let Some(adjusted) = value.adjusted()
                && let Some(previous) =
                    layouts_by_identity.insert(adjusted.layout_identity, &adjusted.layout)
                && previous != &adjusted.layout
            {
                return Err(SemanticMirErrorV1::InvalidTypeLayout);
            }
        }
    }

    for (index, ty) in request.types.iter().enumerate() {
        context.one()?;
        validate_type(&mut context, SemanticTypeIdV1(index as u32), ty)?;
    }
    validate_callables(&mut context)?;
    for (index, allocation) in request.allocations.iter().enumerate() {
        context.one()?;
        validate_allocation(
            &mut context,
            SemanticAllocationIdV1(index as u32),
            allocation,
        )?;
    }
    let claimed_allocations = validate_statics(&mut context)?;
    validate_vtables(&mut context, claimed_allocations)?;
    let mut rooted = vec![false; request.functions.len()];
    for (root_index, root) in request.roots.iter().copied().enumerate() {
        context.function_reference(root, SemanticMirLocationV1::Root(root_index as u32))?;
        rooted[root.0 as usize] = true;
        let role = request.functions[root.0 as usize].role;
        if !function_role_is_external_entry(role) {
            return Err(SemanticMirErrorV1::InvalidFunctionRole {
                function: root,
                role,
                rooted: true,
            });
        }
    }
    for (index, function) in request.functions.iter().enumerate() {
        let is_rooted = rooted[index];
        if function_role_is_external_entry(function.role) != is_rooted {
            return Err(SemanticMirErrorV1::InvalidFunctionRole {
                function: SemanticFunctionIdV1(index as u32),
                role: function.role,
                rooted: is_rooted,
            });
        }
    }
    charge_validation_work(&mut context, request.functions.len())?;
    validate_kernel_entries(&mut context)?;
    for (index, function) in request.functions.iter().enumerate() {
        context.one()?;
        validate_function(&mut context, SemanticFunctionIdV1(index as u32), function)?;
    }
    validate_exact_function_closure(&mut context)?;
    Ok(())
}

fn validate_kernel_entries(
    context: &mut ValidationContextV1<'_>,
) -> Result<(), SemanticMirErrorV1> {
    charge_validation_work(context, context.request.statics.len())?;
    charge_validation_work(context, context.request.functions.len())?;
    let mut export_symbols = context
        .request
        .statics
        .iter()
        .filter_map(SemanticStaticDeclV1::link_symbol)
        .map(|symbol| symbol.0.as_ref())
        .collect::<BTreeSet<_>>();
    for callable in &context.request.callables {
        if let SemanticCallableDeclV1::DeviceFfiImport { contract, .. } = callable
            && !export_symbols.insert(contract.symbol.0.as_ref())
        {
            return Err(SemanticMirErrorV1::InvalidKernelEntry);
        }
    }
    let mut kernel_bindings = BTreeSet::new();
    for function in &context.request.functions {
        let Some(export) = &function.export else {
            continue;
        };
        let export_symbol = export.export_symbol();
        context.totals.charge(
            SemanticMirResourceV1::LinkSymbolBytes,
            export_symbol.0.len(),
            context.limits,
        )?;
        let role_matches = match (function.role, export) {
            (SemanticFunctionRoleV1::KernelRoot, SemanticFunctionExportV1::Kernel(entry)) => {
                function.abi.canon_abi == SemanticCanonAbiV1::GpuKernel
                    && context
                        .request
                        .types
                        .get(function.abi.source_output_type().0 as usize)
                        .is_some_and(|ty| matches!(ty.shape, SemanticTypeShapeV1::Unit))
                    && kernel_bindings.insert(entry.kernel_binding_identity)
            }
            (
                SemanticFunctionRoleV1::DeviceFfiExport,
                SemanticFunctionExportV1::DeviceFfi { .. },
            ) => {
                function.abi.canon_abi == SemanticCanonAbiV1::C
                    && function.abi.extern_abi() == SemanticExternAbiV1::C { unwind: false }
                    && !function.abi.c_variadic()
            }
            _ => false,
        };
        let target_matches = match export {
            SemanticFunctionExportV1::Kernel(entry) => entry
                .source_contract
                .unsafe_assembly()
                .is_none_or(|assembly| {
                    matches!(
                        (context.request.target.architecture, assembly.target),
                        (
                            SemanticTargetArchitectureV1::AmdGpuGfx942,
                            SemanticUnsafeAssemblyTargetV1::AmdGpuGfx942
                        )
                    )
                }),
            SemanticFunctionExportV1::DeviceFfi { .. } => true,
        };
        if !role_matches || !target_matches || !export_symbols.insert(export_symbol.0.as_ref()) {
            return Err(SemanticMirErrorV1::InvalidKernelEntry);
        }
    }
    Ok(())
}

fn validate_callables(context: &mut ValidationContextV1<'_>) -> Result<(), SemanticMirErrorV1> {
    if context.request.callables.len() < context.request.functions.len() {
        return Err(SemanticMirErrorV1::InvalidFunctionAbi);
    }
    let mut identities = context
        .request
        .functions
        .iter()
        .map(|function| function.identity)
        .collect::<BTreeSet<_>>();
    if identities.len() != context.request.functions.len() {
        return Err(SemanticMirErrorV1::DuplicateIdentity {
            entity: SemanticMirEntityV1::Function,
            location: SemanticMirLocationV1::Module,
        });
    }
    let mut previous_non_body = None;
    let mut contract_identities = BTreeSet::new();
    let mut intrinsic_identities = BTreeSet::new();
    for (index, callable) in context.request.callables.iter().enumerate() {
        context.one()?;
        let callable_id = SemanticCallableIdV1(index as u32);
        let location = SemanticMirLocationV1::Callable(callable_id);
        if index < context.request.functions.len() {
            if *callable
                != (SemanticCallableDeclV1::Defined {
                    function: SemanticFunctionIdV1(index as u32),
                })
            {
                return Err(SemanticMirErrorV1::InvalidFunctionAbi);
            }
            continue;
        }
        let (tag, binding) = match callable {
            SemanticCallableDeclV1::Defined { .. } => {
                return Err(SemanticMirErrorV1::InvalidFunctionAbi);
            }
            SemanticCallableDeclV1::DeviceFfiImport { binding, contract } => {
                if !contract_identities.insert(contract.contract_identity)
                    || !matches!(
                        contract.target,
                        SemanticDeviceFfiTargetV1::AmdGpuGfx942XnackMinus
                    )
                    || !matches!(
                        contract.code_object_version,
                        SemanticCodeObjectVersionV1::V6
                    )
                {
                    return Err(SemanticMirErrorV1::InvalidFunctionAbi);
                }
                context.totals.charge(
                    SemanticMirResourceV1::LinkSymbolBytes,
                    contract.symbol.0.len(),
                    context.limits,
                )?;
                if binding.abi.canon_abi != SemanticCanonAbiV1::C
                    || binding.abi.extern_abi() != (SemanticExternAbiV1::C { unwind: false })
                    || binding.abi.c_variadic()
                {
                    return Err(SemanticMirErrorV1::InvalidFunctionAbi);
                }
                (1_u8, binding)
            }
            SemanticCallableDeclV1::CompilerIntrinsic {
                binding,
                operation,
                operation_identity,
            } => {
                if !intrinsic_identities.insert(*operation_identity)
                    || !compiler_intrinsic_signature_matches(
                        context.request,
                        *operation,
                        &binding.abi,
                    )
                {
                    return Err(SemanticMirErrorV1::InvalidFunctionAbi);
                }
                (2_u8, binding)
            }
        };
        let key = (tag, binding.identity);
        if previous_non_body.is_some_and(|previous| previous >= key)
            || !identities.insert(binding.identity)
        {
            return Err(SemanticMirErrorV1::NonDeterministicOrder {
                entity: SemanticMirEntityV1::Callable,
            });
        }
        previous_non_body = Some(key);
        validate_non_body_callable_abi(context, location, binding)?;
    }
    Ok(())
}

fn validate_non_body_callable_abi(
    context: &mut ValidationContextV1<'_>,
    location: SemanticMirLocationV1,
    binding: &SemanticNonBodyCallableBindingV1,
) -> Result<(), SemanticMirErrorV1> {
    let abi = &binding.abi;
    validate_function_abi_contract(context.request.target, abi)?;
    if abi.return_value.adjusted().is_some()
        || abi.return_value.pointee_override.is_some()
        || !abi.hidden_arguments().is_empty()
    {
        return Err(SemanticMirErrorV1::InvalidFunctionAbi);
    }
    context.type_reference(abi.return_value.source_ty, location)?;
    validate_abi_value(
        context,
        &abi.return_value,
        abi.canon_abi,
        abi.extern_abi(),
        SemanticAbiValuePositionV1::Return,
    )?;
    validate_abi_argument_contract(
        abi.extern_abi(),
        abi.c_variadic(),
        abi.fixed_count,
        abi.source_input_types(),
        abi.source_output_type(),
        &abi.arguments,
        abi.return_value.source_ty,
    )?;
    context.totals.charge(
        SemanticMirResourceV1::CallArguments,
        abi.source_input_types().len(),
        context.limits,
    )?;
    for source_type in abi.source_input_types() {
        context.type_reference(*source_type, location)?;
    }
    context.type_reference(abi.source_output_type(), location)?;
    validate_rust_call_expansion(context.request, abi)?;
    context.totals.charge(
        SemanticMirResourceV1::CallArguments,
        abi.arguments.len(),
        context.limits,
    )?;
    for argument in &abi.arguments {
        if !matches!(argument.role, SemanticAbiArgumentRoleV1::Source)
            || argument.value.adjusted().is_some()
            || argument.value.pointee_override.is_some()
        {
            return Err(SemanticMirErrorV1::InvalidFunctionAbi);
        }
        context.type_reference(argument.value.source_ty, location)?;
        validate_abi_value(
            context,
            &argument.value,
            abi.canon_abi,
            abi.extern_abi(),
            SemanticAbiValuePositionV1::AdjustedArgument,
        )?;
    }
    Ok(())
}

fn compiler_intrinsic_signature_matches(
    request: &InertSemanticMirRequestV1,
    operation: SemanticCompilerIntrinsicOperationV1,
    abi: &SemanticFunctionAbiV1,
) -> bool {
    if abi.canon_abi != SemanticCanonAbiV1::Rust
        || abi.extern_abi() != SemanticExternAbiV1::Rust
        || abi.c_variadic()
    {
        return false;
    }
    let inputs = abi.source_input_types();
    let output = abi.source_output_type();
    if request.types.get(output.0 as usize).is_none()
        || inputs
            .iter()
            .any(|input| request.types.get(input.0 as usize).is_none())
    {
        return false;
    }
    match operation {
        SemanticCompilerIntrinsicOperationV1::ThreadIndex(_)
        | SemanticCompilerIntrinsicOperationV1::WorkgroupIndex(_)
        | SemanticCompilerIntrinsicOperationV1::WorkgroupDimension(_)
        | SemanticCompilerIntrinsicOperationV1::GridDimension(_) => {
            inputs.is_empty() && is_unsigned_integer_with_bits(request, output, 32)
        }
        SemanticCompilerIntrinsicOperationV1::WorkgroupBarrier
        | SemanticCompilerIntrinsicOperationV1::WaveBarrier => {
            inputs.is_empty()
                && matches!(
                    request.types[output.0 as usize].shape,
                    SemanticTypeShapeV1::Unit
                )
        }
        SemanticCompilerIntrinsicOperationV1::FabsF32 => {
            inputs.len() == 1
                && inputs[0] == output
                && matches!(
                    scalar_type(request, output),
                    Some(SemanticScalarTypeV1::Float { bits: 32 })
                )
        }
        SemanticCompilerIntrinsicOperationV1::ThreadIndex1d {
            index_witness,
            raw_index,
        } => {
            inputs.is_empty()
                && output == index_witness
                && transparent_index_witness_matches(request, index_witness, raw_index)
        }
        SemanticCompilerIntrinsicOperationV1::ThreadIndexGet {
            index_witness,
            raw_index,
        } => {
            inputs.len() == 1
                && output == raw_index
                && transparent_index_witness_matches(request, index_witness, raw_index)
                && shared_reference_to(request, inputs[0], index_witness)
        }
        SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMut {
            disjoint_slice,
            index_witness,
            element,
            raw_index,
        } => {
            inputs.len() == 2
                && inputs[1] == index_witness
                && transparent_index_witness_matches(request, index_witness, raw_index)
                && mutable_reference_to(request, inputs[0], disjoint_slice)
                && disjoint_slice_type_matches(
                    request,
                    disjoint_slice,
                    index_witness,
                    element,
                    raw_index,
                )
                && checked_mutable_access_result_matches(request, output, element)
        }
    }
}

fn transparent_index_witness_matches(
    request: &InertSemanticMirRequestV1,
    witness: SemanticTypeIdV1,
    raw_index: SemanticTypeIdV1,
) -> bool {
    let Some(witness_decl) = request.types.get(witness.0 as usize) else {
        return false;
    };
    let Some(raw_decl) = request.types.get(raw_index.0 as usize) else {
        return false;
    };
    if !is_unsigned_integer_with_bits(request, raw_index, 64)
        || witness_decl.layout.size_bytes != raw_decl.layout.size_bytes
        || witness_decl.layout.alignment_bytes != raw_decl.layout.alignment_bytes
        || witness_decl.layout.backend_repr != raw_decl.layout.backend_repr
    {
        return false;
    }
    let SemanticTypeShapeV1::Aggregate(fields) = &witness_decl.shape else {
        return false;
    };
    let SemanticTypeLayoutDetailsV1::Aggregate(layout) = &witness_decl.layout.details else {
        return false;
    };
    let mut raw_fields = 0_usize;
    let mut marker_fields = 0_usize;
    for (field, offset) in fields.fields.iter().zip(layout.field_offsets.iter()) {
        let Some(field_decl) = request.types.get(field.0 as usize) else {
            return false;
        };
        if *field == raw_index {
            raw_fields += 1;
            if *offset != 0 {
                return false;
            }
        } else if field_decl.layout.size_bytes == Some(0) {
            marker_fields += 1;
        } else {
            return false;
        }
    }
    raw_fields == 1 && marker_fields != 0
}

fn shared_reference_to(
    request: &InertSemanticMirRequestV1,
    reference: SemanticTypeIdV1,
    pointee: SemanticTypeIdV1,
) -> bool {
    reference_to(request, reference, pointee, SemanticMutabilityV1::Immutable)
}

fn mutable_reference_to(
    request: &InertSemanticMirRequestV1,
    reference: SemanticTypeIdV1,
    pointee: SemanticTypeIdV1,
) -> bool {
    reference_to(request, reference, pointee, SemanticMutabilityV1::Mutable)
}

fn reference_to(
    request: &InertSemanticMirRequestV1,
    reference: SemanticTypeIdV1,
    pointee: SemanticTypeIdV1,
    mutability: SemanticMutabilityV1,
) -> bool {
    matches!(
        request.types.get(reference.0 as usize).map(|ty| &ty.shape),
        Some(SemanticTypeShapeV1::Pointer(pointer))
            if pointer.pointee == pointee
                && pointer.kind == SemanticPointerKindV1::Reference
                && pointer.mutability == mutability
                && pointer.address_space == 0
                && pointer.pointer_width_bits == 64
                && pointer.metadata == SemanticPointerMetadataV1::None
    )
}

fn disjoint_slice_type_matches(
    request: &InertSemanticMirRequestV1,
    disjoint_slice: SemanticTypeIdV1,
    index_witness: SemanticTypeIdV1,
    element: SemanticTypeIdV1,
    raw_index: SemanticTypeIdV1,
) -> bool {
    let Some(slice_decl) = request.types.get(disjoint_slice.0 as usize) else {
        return false;
    };
    if request.types.get(element.0 as usize).is_none() {
        return false;
    }
    let SemanticTypeShapeV1::Aggregate(slice_fields) = &slice_decl.shape else {
        return false;
    };
    let SemanticTypeLayoutDetailsV1::Aggregate(slice_layout) = &slice_decl.layout.details else {
        return false;
    };
    let Some(witness_decl) = request.types.get(index_witness.0 as usize) else {
        return false;
    };
    let SemanticTypeShapeV1::Aggregate(witness_fields) = &witness_decl.shape else {
        return false;
    };

    let mut pointer_field = None;
    let mut raw_index_field = None;
    let mut slice_markers = BTreeSet::new();
    for (field_index, field) in slice_fields.fields.iter().copied().enumerate() {
        let Some(field_decl) = request.types.get(field.0 as usize) else {
            return false;
        };
        if matches!(
            &field_decl.shape,
            SemanticTypeShapeV1::Pointer(pointer)
                if pointer.pointee == element
                    && pointer.kind == SemanticPointerKindV1::Raw
                    && pointer.mutability == SemanticMutabilityV1::Mutable
                    && pointer.address_space == 0
                    && pointer.pointer_width_bits == 64
                    && pointer.metadata == SemanticPointerMetadataV1::None
        ) {
            if pointer_field.replace(field_index).is_some() {
                return false;
            }
        } else if field == raw_index {
            if raw_index_field.replace(field_index).is_some() {
                return false;
            }
        } else if field_decl.layout.size_bytes == Some(0) {
            slice_markers.insert(field);
        } else {
            return false;
        }
    }
    let (Some(pointer_field), Some(raw_index_field)) = (pointer_field, raw_index_field) else {
        return false;
    };
    let pointer_type = slice_fields.fields[pointer_field];
    let pointer_decl = &request.types[pointer_type.0 as usize];
    let raw_decl = &request.types[raw_index.0 as usize];
    let pointer_scalar = match pointer_decl.layout.backend_repr {
        SemanticBackendReprV1::Scalar(scalar) => scalar,
        _ => return false,
    };
    let raw_scalar = match raw_decl.layout.backend_repr {
        SemanticBackendReprV1::Scalar(scalar) => scalar,
        _ => return false,
    };
    let expected_raw_offset = match pointer_decl
        .layout
        .rustc_size_bytes
        .checked_add(raw_decl.layout.alignment_bytes.wrapping_sub(1))
    {
        Some(value) => value & !(raw_decl.layout.alignment_bytes - 1),
        None => return false,
    };
    let expected_size = match expected_raw_offset.checked_add(raw_decl.layout.rustc_size_bytes) {
        Some(size) => size,
        None => return false,
    };
    if slice_layout.field_offsets.get(pointer_field) != Some(&0)
        || slice_layout.field_offsets.get(raw_index_field) != Some(&expected_raw_offset)
        || slice_decl.layout.size_bytes != Some(expected_size)
        || slice_decl.layout.alignment_bytes
            != pointer_decl
                .layout
                .alignment_bytes
                .max(raw_decl.layout.alignment_bytes)
        || slice_decl.layout.backend_repr
            != (SemanticBackendReprV1::ScalarPair {
                first: pointer_scalar,
                second: raw_scalar,
            })
    {
        return false;
    }

    witness_fields.fields.iter().any(|field| {
        *field != raw_index
            && slice_markers.contains(field)
            && request
                .types
                .get(field.0 as usize)
                .is_some_and(|ty| ty.layout.size_bytes == Some(0))
    })
}

fn checked_mutable_access_result_matches(
    request: &InertSemanticMirRequestV1,
    result: SemanticTypeIdV1,
    element: SemanticTypeIdV1,
) -> bool {
    let Some(result_decl) = request.types.get(result.0 as usize) else {
        return false;
    };
    let SemanticTypeShapeV1::Enum {
        variants,
        discriminant,
    } = &result_decl.shape
    else {
        return false;
    };
    if variants.len() != 2
        || variants[0].discriminant != 0
        || !variants[0].fields.fields.is_empty()
        || variants[1].discriminant != 1
        || variants[1].fields.fields.len() != 1
        || !is_unsigned_integer_type(request, *discriminant)
        || !mutable_reference_to(request, variants[1].fields.fields[0], element)
    {
        return false;
    }
    matches!(
        &result_decl.layout.variants,
        SemanticRustcVariantsV1::Multiple(layout)
            if matches!(
                &layout.encoding,
                SemanticEnumEncodingV1::Niche(niche)
                    if niche.untagged_variant == 1
                        && niche.niche_variant_range() == (0, 0)
            )
    )
}

const fn function_role_is_external_entry(role: SemanticFunctionRoleV1) -> bool {
    matches!(
        role,
        SemanticFunctionRoleV1::KernelRoot | SemanticFunctionRoleV1::DeviceFfiExport
    )
}

fn enforce_count(
    resource: SemanticMirResourceV1,
    actual: usize,
    limits: SemanticMirLimitsV1,
) -> Result<(), SemanticMirErrorV1> {
    let actual =
        u64::try_from(actual).map_err(|_| SemanticMirErrorV1::ArithmeticOverflow { resource })?;
    let max = limits.limit(resource);
    if actual > max {
        return Err(SemanticMirErrorV1::LimitExceeded {
            resource,
            actual,
            max,
        });
    }
    Ok(())
}

fn ensure_identity_order(
    identities: impl Iterator<Item = [u8; 32]>,
    entity: SemanticMirEntityV1,
) -> Result<(), SemanticMirErrorV1> {
    let mut previous = None;
    for identity in identities {
        if previous.is_some_and(|value| value >= identity) {
            return Err(SemanticMirErrorV1::NonDeterministicOrder { entity });
        }
        previous = Some(identity);
    }
    Ok(())
}

fn validate_type(
    context: &mut ValidationContextV1<'_>,
    id: SemanticTypeIdV1,
    ty: &SemanticTypeDeclV1,
) -> Result<(), SemanticMirErrorV1> {
    let expected_size = match ty.layout.backend_repr {
        SemanticBackendReprV1::Memory { sized: false } => None,
        _ => Some(ty.layout.rustc_size_bytes),
    };
    if ty.layout.rustc_size_bytes >= context.request.target.object_size_bound_bytes
        || !valid_rustc_alignment(ty.layout.alignment_bytes)
        || ty.layout.size_bytes != expected_size
        || (!ty
            .layout
            .rustc_size_bytes
            .is_multiple_of(ty.layout.alignment_bytes)
            && !backend_repr_is_overaligned_pointer(
                ty.layout.backend_repr,
                ty.layout.rustc_size_bytes,
            ))
        || ty
            .layout
            .max_repr_alignment_bytes
            .is_some_and(|alignment| !valid_rustc_alignment(alignment))
        || !valid_rustc_alignment(ty.layout.unadjusted_abi_alignment_bytes)
    {
        return Err(SemanticMirErrorV1::InvalidTypeLayout);
    }
    charge_fields_shape_work(context, &ty.layout.fields)?;
    validate_fields_shape(&ty.layout.fields, Some(ty.layout.rustc_size_bytes))?;
    enforce_rustc_variants_shape(&ty.layout.variants, ty.layout.uninhabited)?;
    if let SemanticRustcVariantsV1::Multiple(layout) = &ty.layout.variants {
        charge_validation_work(context, layout.variants.len())?;
    }
    validate_backend_repr(
        ty.layout.size_bytes,
        ty.layout.alignment_bytes,
        &ty.layout.backend_repr,
    )?;
    validate_target_backend_repr(context.request.target, &ty.layout)?;
    validate_single_backend_layout_facts(&ty.layout)?;
    validate_type_abi_properties(context.request, ty)?;
    if let Some(niche) = ty.layout.largest_niche {
        validate_layout_niche(niche, Some(ty.layout.rustc_size_bytes))?;
    }
    let location = SemanticMirLocationV1::Type(id);
    match &ty.shape {
        SemanticTypeShapeV1::Unit => {
            require_plain_layout(&ty.layout)?;
            require_single_layout(&ty.layout, 0)?;
            if !matches!(
                &ty.layout.fields,
                SemanticFieldsShapeV1::Arbitrary {
                    source_order_offsets_bytes,
                    memory_order_source_indices,
                } if source_order_offsets_bytes.is_empty()
                    && memory_order_source_indices.is_empty()
            ) || ty.layout.size_bytes != Some(0)
                || ty.layout.alignment_bytes != 1
                || ty.layout.uninhabited
                || ty.layout.largest_niche.is_some()
                || ty.layout.max_repr_alignment_bytes.is_some()
                || ty.layout.unadjusted_abi_alignment_bytes != 1
                || ty.layout.randomization_seed != 0
            {
                return Err(SemanticMirErrorV1::InvalidTypeLayout);
            }
        }
        SemanticTypeShapeV1::Never => {
            require_plain_layout(&ty.layout)?;
            if !matches!(ty.layout.variants, SemanticRustcVariantsV1::Empty)
                || !matches!(ty.layout.fields, SemanticFieldsShapeV1::Primitive)
                || ty.layout.size_bytes != Some(0)
                || ty.layout.alignment_bytes != 1
            {
                return Err(SemanticMirErrorV1::InvalidTypeLayout);
            }
        }
        SemanticTypeShapeV1::Opaque => {
            require_plain_layout(&ty.layout)?;
            if matches!(ty.layout.variants, SemanticRustcVariantsV1::Multiple(_)) {
                return Err(SemanticMirErrorV1::InvalidTypeLayout);
            }
        }
        SemanticTypeShapeV1::Scalar(scalar) => {
            validate_scalar_layout(&ty.layout, *scalar)?;
        }
        SemanticTypeShapeV1::ValidityScalar(validity) => {
            validate_validity_scalar_layout(&ty.layout, validity)?;
            charge_validation_work(context, validity.valid_ranges.len())?;
            validate_validity_ranges(validity.scalar, &validity.valid_ranges)?;
        }
        SemanticTypeShapeV1::Pointer(pointer) => {
            require_plain_layout(&ty.layout)?;
            require_single_layout(&ty.layout, 0)?;
            context.type_reference(pointer.pointee, location)?;
            validate_pointer_backend_repr(&ty.layout, pointer)?;
        }
        SemanticTypeShapeV1::Array { element, length } => {
            require_plain_layout(&ty.layout)?;
            require_single_layout(&ty.layout, 0)?;
            context.type_reference(*element, location)?;
            let element_layout = &context.request.types[element.0 as usize].layout;
            let element_size = element_layout
                .size_bytes
                .ok_or(SemanticMirErrorV1::InvalidTypeLayout)?;
            let expected = element_size.checked_mul(*length).ok_or(
                SemanticMirErrorV1::ArithmeticOverflow {
                    resource: SemanticMirResourceV1::Types,
                },
            )?;
            if ty.layout.fields
                != (SemanticFieldsShapeV1::Array {
                    stride_bytes: element_size,
                    count: *length,
                })
                || ty.layout.size_bytes != Some(expected)
                || ty.layout.alignment_bytes != element_layout.alignment_bytes
            {
                return Err(SemanticMirErrorV1::InvalidTypeLayout);
            }
        }
        SemanticTypeShapeV1::Slice { element } => {
            require_plain_layout(&ty.layout)?;
            require_single_layout(&ty.layout, 0)?;
            context.type_reference(*element, location)?;
            let element_layout = &context.request.types[element.0 as usize].layout;
            let element_size = element_layout
                .size_bytes
                .ok_or(SemanticMirErrorV1::InvalidTypeLayout)?;
            if ty.layout.fields
                != (SemanticFieldsShapeV1::Array {
                    stride_bytes: element_size,
                    count: 0,
                })
                || ty.layout.rustc_size_bytes != 0
                || ty.layout.size_bytes.is_some()
                || ty.layout.alignment_bytes != element_layout.alignment_bytes
                || !matches!(
                    ty.layout.backend_repr,
                    SemanticBackendReprV1::Memory { sized: false }
                )
                || ty.layout.largest_niche.is_some()
                || ty.layout.uninhabited
                || ty.layout.max_repr_alignment_bytes.is_some()
                || ty.layout.unadjusted_abi_alignment_bytes != ty.layout.alignment_bytes
                || ty.layout.randomization_seed != 0
            {
                return Err(SemanticMirErrorV1::InvalidTypeLayout);
            }
        }
        SemanticTypeShapeV1::Tuple(fields) | SemanticTypeShapeV1::Aggregate(fields) => {
            require_single_layout(&ty.layout, 0)?;
            validate_type_list(context, fields, location)?;
            let SemanticTypeLayoutDetailsV1::Aggregate(layout) = &ty.layout.details else {
                return Err(SemanticMirErrorV1::InvalidTypeLayout);
            };
            validate_aggregate_layout(
                context,
                ty.layout.size_bytes,
                ty.layout.alignment_bytes,
                &ty.layout.fields,
                fields,
                layout,
                None,
                false,
            )?;
        }
        SemanticTypeShapeV1::Union(fields) => {
            require_single_layout(&ty.layout, 0)?;
            validate_type_list(context, fields, location)?;
            validate_union_layout(context, &ty.layout, fields)?;
        }
        SemanticTypeShapeV1::Enum {
            discriminant,
            variants,
        } => {
            context.type_reference(*discriminant, location)?;
            for variant in variants {
                context.one()?;
                validate_type_list(context, &variant.fields, location)?;
            }
            validate_enum_layout(context, ty, *discriminant, variants)?;
        }
        SemanticTypeShapeV1::FunctionPointer {
            extern_abi,
            c_variadic,
            arguments,
            return_type,
            ..
        } => {
            require_plain_layout(&ty.layout)?;
            require_single_layout(&ty.layout, 0)?;
            if canonicalize_extern_abi(*extern_abi).is_none()
                || (*c_variadic
                    && !matches!(
                        extern_abi,
                        SemanticExternAbiV1::C { .. }
                            | SemanticExternAbiV1::Cdecl { .. }
                            | SemanticExternAbiV1::System { .. }
                    ))
            {
                return Err(SemanticMirErrorV1::InvalidFunctionAbi);
            }
            let expected_pointer = SemanticBackendPrimitiveV1::pointer(0, 8, 8);
            if ty.layout.size_bytes != Some(8)
                || ty.layout.alignment_bytes != 8
                || ty.layout.backend_repr
                    != SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                        expected_pointer,
                        SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
                    ))
            {
                return Err(SemanticMirErrorV1::InvalidTypeLayout);
            }
            validate_type_list(context, arguments, location)?;
            context.type_reference(*return_type, location)?;
        }
    }
    Ok(())
}

fn validate_type_abi_properties(
    request: &InertSemanticMirRequestV1,
    ty: &SemanticTypeDeclV1,
) -> Result<(), SemanticMirErrorV1> {
    let properties = ty.abi_properties;
    let scalar_is_pointer = |scalar: SemanticBackendScalarV1| {
        matches!(
            scalar.primitive(),
            SemanticBackendPrimitiveV1::Pointer { .. }
        )
    };
    let (first_is_pointer, second_is_pointer) = match ty.layout.backend_repr {
        SemanticBackendReprV1::Scalar(scalar) => (scalar_is_pointer(scalar), false),
        SemanticBackendReprV1::ScalarPair { first, second } => {
            (scalar_is_pointer(first), scalar_is_pointer(second))
        }
        _ => (false, false),
    };
    if properties.first_pointee.is_some() && !first_is_pointer
        || properties.second_pointee.is_some() && !second_is_pointer
    {
        return Err(SemanticMirErrorV1::InvalidFunctionAbi);
    }
    match &ty.shape {
        SemanticTypeShapeV1::Pointer(pointer) => {
            let metadata_pointee_matches = match pointer.metadata {
                SemanticPointerMetadataV1::None | SemanticPointerMetadataV1::SliceLength => {
                    properties.second_pointee.is_none()
                }
                SemanticPointerMetadataV1::VTable => {
                    properties.second_pointee.is_some_and(|metadata| {
                        metadata
                            == SemanticAbiPointeeInfoV1 {
                                kind: SemanticAbiPointeeKindV1::Raw,
                                guaranteed_size_bytes: 0,
                                reliable_alignment_bytes: 1,
                            }
                    })
                }
            };
            if !metadata_pointee_matches {
                return Err(SemanticMirErrorV1::InvalidFunctionAbi);
            }
            let Some(info) = properties.first_pointee else {
                return Ok(());
            };
            let kind_matches = matches!(
                (pointer.kind, pointer.mutability, info.kind),
                (SemanticPointerKindV1::Raw, _, SemanticAbiPointeeKindV1::Raw)
                    | (
                        SemanticPointerKindV1::Reference,
                        SemanticMutabilityV1::Immutable,
                        SemanticAbiPointeeKindV1::SharedReference { .. }
                    )
                    | (
                        SemanticPointerKindV1::Reference,
                        SemanticMutabilityV1::Mutable,
                        SemanticAbiPointeeKindV1::MutableReference { .. }
                    )
            );
            let pointee_layout = &request
                .types
                .get(pointer.pointee.0 as usize)
                .ok_or(SemanticMirErrorV1::InvalidFunctionAbi)?
                .layout;
            let expected_guaranteed_size = match pointer.metadata {
                SemanticPointerMetadataV1::None => pointee_layout.rustc_size_bytes,
                SemanticPointerMetadataV1::SliceLength | SemanticPointerMetadataV1::VTable => 0,
            };
            if !kind_matches
                || info.reliable_alignment_bytes > pointee_layout.alignment_bytes
                || (matches!(
                    info.kind,
                    SemanticAbiPointeeKindV1::SharedReference { frozen: true }
                        | SemanticAbiPointeeKindV1::MutableReference { unpin: true }
                ) && info.guaranteed_size_bytes != expected_guaranteed_size)
            {
                return Err(SemanticMirErrorV1::InvalidFunctionAbi);
            }
        }
        SemanticTypeShapeV1::FunctionPointer { .. }
            if properties.pass_indirectly_in_non_rustic_abis
                || properties.has_unsized_foreign_tail
                || properties.first_pointee.is_some()
                || properties.second_pointee.is_some() =>
        {
            return Err(SemanticMirErrorV1::InvalidFunctionAbi);
        }
        SemanticTypeShapeV1::FunctionPointer { .. } => {}
        _ => {}
    }
    Ok(())
}

fn require_plain_layout(layout: &SemanticTypeLayoutV1) -> Result<(), SemanticMirErrorV1> {
    if matches!(layout.details, SemanticTypeLayoutDetailsV1::None) {
        Ok(())
    } else {
        Err(SemanticMirErrorV1::InvalidTypeLayout)
    }
}

fn require_single_layout(
    layout: &SemanticTypeLayoutV1,
    expected_index: u32,
) -> Result<(), SemanticMirErrorV1> {
    if matches!(
        layout.variants,
        SemanticRustcVariantsV1::Single { index } if index == expected_index
    ) {
        Ok(())
    } else {
        Err(SemanticMirErrorV1::InvalidTypeLayout)
    }
}

fn validate_scalar_layout(
    layout: &SemanticTypeLayoutV1,
    scalar: SemanticScalarTypeV1,
) -> Result<(), SemanticMirErrorV1> {
    require_plain_layout(layout)?;
    require_single_layout(layout, 0)?;
    if !matches!(layout.fields, SemanticFieldsShapeV1::Primitive) {
        return Err(SemanticMirErrorV1::InvalidTypeLayout);
    }
    let width = scalar
        .byte_width()
        .ok_or(SemanticMirErrorV1::InvalidTypeLayout)?;
    let SemanticBackendReprV1::Scalar(backend_scalar) = layout.backend_repr else {
        return Err(SemanticMirErrorV1::InvalidTypeLayout);
    };
    if !backend_scalar_matches_semantic_scalar(backend_scalar, scalar)
        || layout.size_bytes != Some(width)
    {
        return Err(SemanticMirErrorV1::InvalidTypeLayout);
    }
    Ok(())
}

fn validate_validity_scalar_layout(
    layout: &SemanticTypeLayoutV1,
    validity: &SemanticValidityScalarTypeV1,
) -> Result<(), SemanticMirErrorV1> {
    require_plain_layout(layout)?;
    require_single_layout(layout, 0)?;
    if !matches!(layout.fields, SemanticFieldsShapeV1::Primitive) {
        return Err(SemanticMirErrorV1::InvalidTypeLayout);
    }
    let width = validity
        .scalar
        .byte_width()
        .ok_or(SemanticMirErrorV1::InvalidTypeLayout)?;
    let SemanticBackendReprV1::Scalar(SemanticBackendScalarV1::Initialized {
        primitive,
        valid_range,
    }) = layout.backend_repr
    else {
        return Err(SemanticMirErrorV1::InvalidTypeLayout);
    };
    if layout.size_bytes != Some(width)
        || !backend_primitive_matches_semantic_scalar(primitive, validity.scalar)
        || validity.valid_ranges.as_ref() != [valid_range]
    {
        return Err(SemanticMirErrorV1::InvalidTypeLayout);
    }
    Ok(())
}

fn validate_backend_repr(
    size_bytes: Option<u64>,
    alignment_bytes: u64,
    backend_repr: &SemanticBackendReprV1,
) -> Result<(), SemanticMirErrorV1> {
    match *backend_repr {
        SemanticBackendReprV1::Memory { sized } => {
            if sized != size_bytes.is_some() {
                return Err(SemanticMirErrorV1::InvalidTypeLayout);
            }
        }
        SemanticBackendReprV1::Scalar(scalar) => {
            let (scalar_size, scalar_alignment) = validate_backend_scalar(scalar)?;
            if size_bytes != Some(scalar_size) || alignment_bytes != scalar_alignment {
                return Err(SemanticMirErrorV1::InvalidTypeLayout);
            }
        }
        SemanticBackendReprV1::ScalarPair { first, second } => {
            let (first_size, first_alignment) = validate_backend_scalar(first)?;
            let (second_size, second_alignment) = validate_backend_scalar(second)?;
            let expected_second_offset = align_up(first_size, second_alignment)?;
            let expected_alignment = first_alignment.max(second_alignment);
            let expected_size = align_up(
                expected_second_offset
                    .checked_add(second_size)
                    .ok_or(SemanticMirErrorV1::InvalidTypeLayout)?,
                expected_alignment,
            )?;
            if alignment_bytes != expected_alignment || size_bytes != Some(expected_size) {
                return Err(SemanticMirErrorV1::InvalidTypeLayout);
            }
        }
        SemanticBackendReprV1::SimdVector { element, count } => {
            let (element_size, element_alignment) = validate_backend_scalar(element)?;
            let unpadded_size = element_size
                .checked_mul(count)
                .ok_or(SemanticMirErrorV1::InvalidTypeLayout)?;
            let expected_size = align_up(unpadded_size, alignment_bytes)?;
            if count == 0
                || count > 32_768
                || alignment_bytes < element_alignment
                || size_bytes != Some(expected_size)
            {
                return Err(SemanticMirErrorV1::InvalidTypeLayout);
            }
        }
        SemanticBackendReprV1::SimdScalableVector { element, count } => {
            let (element_size, element_alignment) = validate_backend_scalar(element)?;
            let unpadded_size = element_size
                .checked_mul(count)
                .ok_or(SemanticMirErrorV1::InvalidTypeLayout)?;
            let expected_size = align_up(unpadded_size, alignment_bytes)?;
            if count == 0
                || count > 32_768
                || alignment_bytes < element_alignment
                || size_bytes != Some(expected_size)
            {
                return Err(SemanticMirErrorV1::InvalidTypeLayout);
            }
        }
    }
    Ok(())
}

fn validate_single_backend_layout_facts(
    layout: &SemanticTypeLayoutV1,
) -> Result<(), SemanticMirErrorV1> {
    if !matches!(layout.variants, SemanticRustcVariantsV1::Single { .. }) {
        return Ok(());
    }
    let (expected_fields, expected_niche) = backend_default_fields_and_niche(layout.backend_repr)?;
    if layout.fields != expected_fields {
        // rustc may forward a scalar, scalar pair, or SIMD backend representation
        // through an aggregate or union while retaining the outer field shape.
        return Ok(());
    }
    let expected_unadjusted_alignment = match layout.backend_repr {
        SemanticBackendReprV1::SimdVector { element, .. }
        | SemanticBackendReprV1::SimdScalableVector { element, .. } => {
            element.primitive().alignment_bytes()
        }
        SemanticBackendReprV1::Memory { .. }
        | SemanticBackendReprV1::Scalar(_)
        | SemanticBackendReprV1::ScalarPair { .. } => layout.alignment_bytes,
    };
    if layout.largest_niche != expected_niche
        || layout.uninhabited
        || layout.max_repr_alignment_bytes.is_some()
        || layout.unadjusted_abi_alignment_bytes != expected_unadjusted_alignment
        || layout.randomization_seed != backend_default_randomization_seed(layout.backend_repr)?
    {
        return Err(SemanticMirErrorV1::InvalidTypeLayout);
    }
    Ok(())
}

fn validate_target_backend_repr(
    target: SemanticTargetDataLayoutV1,
    layout: &SemanticTypeLayoutV1,
) -> Result<(), SemanticMirErrorV1> {
    let validate_scalar =
        |scalar: SemanticBackendScalarV1| validate_target_primitive(target, scalar.primitive());
    match layout.backend_repr {
        SemanticBackendReprV1::Memory { .. } => Ok(()),
        SemanticBackendReprV1::Scalar(scalar) => validate_scalar(scalar),
        SemanticBackendReprV1::ScalarPair { first, second } => {
            validate_scalar(first)?;
            validate_scalar(second)
        }
        SemanticBackendReprV1::SimdVector { element, count }
        | SemanticBackendReprV1::SimdScalableVector { element, count } => {
            validate_scalar(element)?;
            let vector_size = element
                .primitive()
                .size_bytes()
                .and_then(|size| size.checked_mul(count))
                .ok_or(SemanticMirErrorV1::InvalidTypeLayout)?;
            if layout.alignment_bytes != gfx942_vector_alignment(vector_size)? {
                return Err(SemanticMirErrorV1::InvalidTypeLayout);
            }
            Ok(())
        }
    }
}

fn validate_target_primitive(
    target: SemanticTargetDataLayoutV1,
    primitive: SemanticBackendPrimitiveV1,
) -> Result<(), SemanticMirErrorV1> {
    let expected = match target.architecture {
        SemanticTargetArchitectureV1::AmdGpuGfx942 => match primitive {
            SemanticBackendPrimitiveV1::Integer { bits, .. } => match bits {
                8 => Some((1, 1)),
                16 => Some((2, 2)),
                32 => Some((4, 4)),
                64 | 128 => Some(((bits / 8) as u64, 8)),
                _ => None,
            },
            SemanticBackendPrimitiveV1::Float { bits, .. } => match bits {
                16 => Some((2, 2)),
                32 => Some((4, 4)),
                64 => Some((8, 8)),
                128 => Some((16, 16)),
                _ => None,
            },
            SemanticBackendPrimitiveV1::Pointer { address_space, .. } => {
                gfx942_pointer_profile(address_space)
                    .map(|profile| (profile.size_bytes, profile.alignment_bytes))
            }
        },
    };
    if expected
        != primitive
            .size_bytes()
            .map(|size| (size, primitive.alignment_bytes()))
    {
        return Err(SemanticMirErrorV1::InvalidTypeLayout);
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct TargetPointerProfileV1 {
    size_bytes: u64,
    alignment_bytes: u64,
    offset_bits: u16,
}

const fn gfx942_pointer_profile(address_space: u32) -> Option<TargetPointerProfileV1> {
    let (size_bytes, alignment_bytes, offset_bits) = match address_space {
        0 | 1 | 4 => (8, 8, 64),
        2 | 3 | 5 | 6 => (4, 4, 32),
        7 => (20, 32, 32),
        8 => (16, 16, 48),
        9 => (24, 32, 32),
        _ => return None,
    };
    Some(TargetPointerProfileV1 {
        size_bytes,
        alignment_bytes,
        offset_bits,
    })
}

fn target_pointer_profile(
    target: SemanticTargetDataLayoutV1,
    address_space: u32,
) -> Option<TargetPointerProfileV1> {
    match target.architecture {
        SemanticTargetArchitectureV1::AmdGpuGfx942 => gfx942_pointer_profile(address_space),
    }
}

fn target_object_size_bound_in(
    target: SemanticTargetDataLayoutV1,
    address_space: u32,
) -> Option<u64> {
    if matches!(address_space, 7..=9) {
        return None;
    }
    let profile = target_pointer_profile(target, address_space)?;
    match profile.offset_bits {
        16 => Some(1 << 15),
        32 => Some(1 << 31),
        64 => Some(1 << 61),
        // Rust source objects cannot inhabit descriptor address spaces whose
        // pointer-offset width is not one of rustc's object-size domains.
        _ => None,
    }
}

fn gfx942_vector_alignment(vector_size_bytes: u64) -> Result<u64, SemanticMirErrorV1> {
    let alignment = match vector_size_bytes {
        2 => 2,
        3 | 4 => 4,
        6 | 8 => 8,
        12 | 16 => 16,
        24 | 32 => 32,
        64 => 64,
        128 => 128,
        256 => 256,
        size => size
            .checked_next_power_of_two()
            .ok_or(SemanticMirErrorV1::InvalidTypeLayout)?,
    };
    Ok(alignment)
}

fn memory_order_for_offsets(offsets: &[u64]) -> Result<Vec<u32>, SemanticMirErrorV1> {
    let mut order = (0..offsets.len()).collect::<Vec<_>>();
    order.sort_by_key(|index| (offsets[*index], *index));
    order
        .into_iter()
        .map(|index| u32::try_from(index).map_err(|_| SemanticMirErrorV1::InvalidTypeLayout))
        .collect()
}

fn validate_fields_shape(
    fields: &SemanticFieldsShapeV1,
    size_bytes: Option<u64>,
) -> Result<(), SemanticMirErrorV1> {
    match fields {
        SemanticFieldsShapeV1::Primitive => Ok(()),
        SemanticFieldsShapeV1::Union { field_count } => {
            if *field_count == 0 {
                Err(SemanticMirErrorV1::InvalidTypeLayout)
            } else {
                Ok(())
            }
        }
        SemanticFieldsShapeV1::Array {
            stride_bytes,
            count,
        } => {
            let extent = stride_bytes
                .checked_mul(*count)
                .ok_or(SemanticMirErrorV1::InvalidTypeLayout)?;
            if size_bytes.is_some_and(|size| extent > size) {
                return Err(SemanticMirErrorV1::InvalidTypeLayout);
            }
            Ok(())
        }
        SemanticFieldsShapeV1::Arbitrary {
            source_order_offsets_bytes,
            memory_order_source_indices,
        } => {
            if source_order_offsets_bytes.len() != memory_order_source_indices.len()
                || source_order_offsets_bytes
                    .iter()
                    .any(|offset| size_bytes.is_some_and(|size| *offset > size))
            {
                return Err(SemanticMirErrorV1::InvalidTypeLayout);
            }
            let count = source_order_offsets_bytes.len();
            let mut seen = vec![false; count];
            for index in memory_order_source_indices.iter().copied() {
                let index =
                    usize::try_from(index).map_err(|_| SemanticMirErrorV1::InvalidTypeLayout)?;
                let Some(slot) = seen.get_mut(index) else {
                    return Err(SemanticMirErrorV1::InvalidTypeLayout);
                };
                if std::mem::replace(slot, true) {
                    return Err(SemanticMirErrorV1::InvalidTypeLayout);
                }
            }
            if memory_order_source_indices.windows(2).any(|pair| {
                let left = pair[0] as usize;
                let right = pair[1] as usize;
                source_order_offsets_bytes[left] > source_order_offsets_bytes[right]
            }) {
                return Err(SemanticMirErrorV1::InvalidTypeLayout);
            }
            Ok(())
        }
    }
}

fn charge_fields_shape_work(
    context: &mut ValidationContextV1<'_>,
    fields: &SemanticFieldsShapeV1,
) -> Result<(), SemanticMirErrorV1> {
    if let SemanticFieldsShapeV1::Arbitrary {
        source_order_offsets_bytes,
        memory_order_source_indices,
    } = fields
    {
        charge_validation_work(context, source_order_offsets_bytes.len())?;
        charge_validation_work(context, memory_order_source_indices.len())?;
    }
    Ok(())
}

fn validate_layout_niche(
    niche: SemanticLayoutNicheV1,
    size_bytes: Option<u64>,
) -> Result<(), SemanticMirErrorV1> {
    let (primitive_size, _) = validate_backend_scalar(SemanticBackendScalarV1::initialized(
        niche.primitive,
        niche.valid_range,
    ))?;
    if size_bytes.is_some_and(|size| {
        niche
            .offset_bytes
            .checked_add(primitive_size)
            .is_none_or(|end| end > size)
    }) {
        return Err(SemanticMirErrorV1::InvalidTypeLayout);
    }
    let bits = niche
        .primitive
        .bits()
        .ok_or(SemanticMirErrorV1::InvalidTypeLayout)?;
    let mask = unsigned_max(bits);
    let first_invalid = niche.valid_range.end.wrapping_add(1) & mask;
    let available = niche.valid_range.start.wrapping_sub(first_invalid) & mask;
    if available == 0 {
        return Err(SemanticMirErrorV1::InvalidTypeLayout);
    }
    Ok(())
}

fn layout_niche_from_scalar(
    offset_bytes: u64,
    primitive: SemanticBackendPrimitiveV1,
    valid_range: SemanticScalarValidityRangeV1,
) -> Result<Option<SemanticLayoutNicheV1>, SemanticMirErrorV1> {
    let bits = primitive
        .bits()
        .ok_or(SemanticMirErrorV1::InvalidTypeLayout)?;
    let mask = unsigned_max(bits);
    let first_invalid = valid_range.end.wrapping_add(1) & mask;
    let available = valid_range.start.wrapping_sub(first_invalid) & mask;
    if available == 0 {
        Ok(None)
    } else {
        SemanticLayoutNicheV1::new(offset_bytes, primitive, valid_range).map(Some)
    }
}

fn backend_default_fields_and_niche(
    backend_repr: SemanticBackendReprV1,
) -> Result<(SemanticFieldsShapeV1, Option<SemanticLayoutNicheV1>), SemanticMirErrorV1> {
    match backend_repr {
        SemanticBackendReprV1::Memory { .. } => Ok((SemanticFieldsShapeV1::Primitive, None)),
        SemanticBackendReprV1::Scalar(scalar) => Ok((
            SemanticFieldsShapeV1::Primitive,
            scalar_layout_niche(0, scalar)?,
        )),
        SemanticBackendReprV1::ScalarPair { first, second } => {
            let (first_size, _) = validate_backend_scalar(first)?;
            let (_, second_alignment) = validate_backend_scalar(second)?;
            let second_offset = align_up(first_size, second_alignment)?;
            let first_niche = scalar_layout_niche(0, first)?;
            let second_niche = scalar_layout_niche(second_offset, second)?;
            let largest_niche = match (first_niche, second_niche) {
                (Some(first), Some(second)) => {
                    if layout_niche_available(first) >= layout_niche_available(second) {
                        Some(first)
                    } else {
                        Some(second)
                    }
                }
                (Some(niche), None) | (None, Some(niche)) => Some(niche),
                (None, None) => None,
            };
            Ok((
                SemanticFieldsShapeV1::arbitrary(vec![0, second_offset], vec![0, 1])?,
                largest_niche,
            ))
        }
        SemanticBackendReprV1::SimdVector { element, .. }
        | SemanticBackendReprV1::SimdScalableVector { element, .. } => Ok((
            SemanticFieldsShapeV1::arbitrary(vec![0], vec![0])?,
            scalar_layout_niche(0, element)?,
        )),
    }
}

fn backend_default_randomization_seed(
    backend_repr: SemanticBackendReprV1,
) -> Result<u64, SemanticMirErrorV1> {
    match backend_repr {
        SemanticBackendReprV1::Memory { .. } => Ok(0),
        SemanticBackendReprV1::Scalar(scalar) => backend_scalar_randomization_seed(scalar),
        SemanticBackendReprV1::ScalarPair { first, second } => {
            let first_size = first
                .primitive()
                .size_bytes()
                .ok_or(SemanticMirErrorV1::InvalidTypeLayout)?;
            let second_size = second
                .primitive()
                .size_bytes()
                .ok_or(SemanticMirErrorV1::InvalidTypeLayout)?;
            Ok(first_size.wrapping_add(second_size))
        }
        SemanticBackendReprV1::SimdVector { element, count }
        | SemanticBackendReprV1::SimdScalableVector { element, count } => {
            Ok(backend_scalar_randomization_seed(element)?.wrapping_add(count))
        }
    }
}

fn backend_scalar_randomization_seed(
    scalar: SemanticBackendScalarV1,
) -> Result<u64, SemanticMirErrorV1> {
    let primitive = scalar.primitive();
    let size = primitive
        .size_bytes()
        .ok_or(SemanticMirErrorV1::InvalidTypeLayout)?;
    let kind = match primitive {
        SemanticBackendPrimitiveV1::Integer { signed: true, .. } => 1,
        SemanticBackendPrimitiveV1::Integer { signed: false, .. } => 2,
        SemanticBackendPrimitiveV1::Float { .. } => 3,
        SemanticBackendPrimitiveV1::Pointer { .. } => 4,
    };
    let range = scalar.valid_range().unwrap_or_else(|| {
        let end = primitive
            .bits()
            .map(|bits| {
                if bits >= 128 {
                    u128::MAX
                } else {
                    unsigned_max(bits)
                }
            })
            .unwrap_or(u128::MAX);
        SemanticScalarValidityRangeV1::new(0, end)
    });
    Ok(size
        .wrapping_add(kind << 32)
        .wrapping_add((range.start as u64).rotate_right(16))
        .wrapping_add((range.end as u64).rotate_right(16)))
}

fn scalar_layout_niche(
    offset_bytes: u64,
    scalar: SemanticBackendScalarV1,
) -> Result<Option<SemanticLayoutNicheV1>, SemanticMirErrorV1> {
    match scalar {
        SemanticBackendScalarV1::Initialized {
            primitive,
            valid_range,
        } => layout_niche_from_scalar(offset_bytes, primitive, valid_range),
        SemanticBackendScalarV1::Union { .. } => Ok(None),
    }
}

fn layout_niche_available(niche: SemanticLayoutNicheV1) -> u128 {
    let mask = unsigned_max(niche.primitive.bits().unwrap_or(128));
    let first_invalid = niche.valid_range.end.wrapping_add(1) & mask;
    niche.valid_range.start.wrapping_sub(first_invalid) & mask
}

fn validate_backend_scalar(
    scalar: SemanticBackendScalarV1,
) -> Result<(u64, u64), SemanticMirErrorV1> {
    let primitive = scalar.primitive();
    let size = primitive
        .size_bytes()
        .ok_or(SemanticMirErrorV1::InvalidTypeLayout)?;
    let alignment = primitive.alignment_bytes();
    let bits = primitive
        .bits()
        .ok_or(SemanticMirErrorV1::InvalidTypeLayout)?;
    if !valid_rustc_alignment(alignment)
        || (!size.is_multiple_of(alignment)
            && !matches!(primitive, SemanticBackendPrimitiveV1::Pointer { .. }))
    {
        return Err(SemanticMirErrorV1::InvalidTypeLayout);
    }
    if let Some(valid_range) = scalar.valid_range()
        && (bits > 128
            || !value_fits_bits(valid_range.start, bits)
            || !value_fits_bits(valid_range.end, bits))
    {
        return Err(SemanticMirErrorV1::InvalidTypeLayout);
    }
    Ok((size, alignment))
}

fn backend_repr_is_overaligned_pointer(
    backend_repr: SemanticBackendReprV1,
    size_bytes: u64,
) -> bool {
    matches!(
        backend_repr,
        SemanticBackendReprV1::Scalar(scalar)
            if matches!(scalar.primitive(), SemanticBackendPrimitiveV1::Pointer { .. })
                && scalar.primitive().size_bytes() == Some(size_bytes)
    )
}

fn align_up(value: u64, alignment: u64) -> Result<u64, SemanticMirErrorV1> {
    if !valid_rustc_alignment(alignment) {
        return Err(SemanticMirErrorV1::InvalidTypeLayout);
    }
    value
        .checked_add(alignment - 1)
        .map(|value| value & !(alignment - 1))
        .ok_or(SemanticMirErrorV1::InvalidTypeLayout)
}

const fn valid_rustc_alignment(alignment: u64) -> bool {
    alignment != 0 && alignment.is_power_of_two() && alignment <= (1_u64 << 29)
}

fn backend_scalar_matches_semantic_scalar(
    backend: SemanticBackendScalarV1,
    semantic: SemanticScalarTypeV1,
) -> bool {
    let SemanticBackendScalarV1::Initialized {
        primitive,
        valid_range,
    } = backend
    else {
        return false;
    };
    if !backend_primitive_matches_semantic_scalar(primitive, semantic) {
        return false;
    }
    match semantic {
        SemanticScalarTypeV1::Bool => valid_range == SemanticScalarValidityRangeV1::new(0, 1),
        SemanticScalarTypeV1::Char => {
            valid_range == SemanticScalarValidityRangeV1::new(0, 0x10_ffff)
        }
        SemanticScalarTypeV1::Integer { bits, .. } | SemanticScalarTypeV1::Float { bits } => {
            valid_range == SemanticScalarValidityRangeV1::new(0, unsigned_max(bits))
        }
    }
}

fn backend_primitive_matches_semantic_scalar(
    primitive: SemanticBackendPrimitiveV1,
    semantic: SemanticScalarTypeV1,
) -> bool {
    match (semantic, primitive) {
        (
            SemanticScalarTypeV1::Bool,
            SemanticBackendPrimitiveV1::Integer {
                signed: false,
                bits: 8,
                ..
            },
        ) => true,
        (
            SemanticScalarTypeV1::Char,
            SemanticBackendPrimitiveV1::Integer {
                signed: false,
                bits: 32,
                ..
            },
        ) => true,
        (
            SemanticScalarTypeV1::Integer { signed, bits },
            SemanticBackendPrimitiveV1::Integer {
                signed: backend_signed,
                bits: backend_bits,
                ..
            },
        ) => signed == backend_signed && bits == backend_bits,
        (
            SemanticScalarTypeV1::Float { bits },
            SemanticBackendPrimitiveV1::Float {
                bits: backend_bits, ..
            },
        ) => bits == backend_bits,
        _ => false,
    }
}

fn validate_pointer_backend_repr(
    layout: &SemanticTypeLayoutV1,
    pointer: &SemanticPointerTypeV1,
) -> Result<(), SemanticMirErrorV1> {
    let fields_match = match pointer.metadata {
        SemanticPointerMetadataV1::None => {
            matches!(layout.fields, SemanticFieldsShapeV1::Primitive)
        }
        SemanticPointerMetadataV1::SliceLength | SemanticPointerMetadataV1::VTable => {
            backend_default_fields_and_niche(layout.backend_repr)
                .is_ok_and(|(fields, _)| layout.fields == fields)
        }
    };
    if !fields_match {
        return Err(SemanticMirErrorV1::InvalidTypeLayout);
    }
    let matches_data_pointer = |scalar: SemanticBackendScalarV1| {
        let SemanticBackendScalarV1::Initialized {
            primitive,
            valid_range,
        } = scalar
        else {
            return false;
        };
        let Some(bits) = primitive.bits() else {
            return false;
        };
        let expected_range = match pointer.kind {
            SemanticPointerKindV1::Raw => SemanticScalarValidityRangeV1::new(0, unsigned_max(bits)),
            SemanticPointerKindV1::Reference => {
                SemanticScalarValidityRangeV1::new(1, unsigned_max(bits))
            }
        };
        matches!(
            primitive,
            SemanticBackendPrimitiveV1::Pointer {
                address_space,
                size_bytes,
                ..
            } if address_space == pointer.address_space
                && size_bytes.checked_mul(8) == Some(u64::from(pointer.pointer_width_bits))
                && valid_range == expected_range
        )
    };
    match (&pointer.metadata, &layout.backend_repr) {
        (SemanticPointerMetadataV1::None, SemanticBackendReprV1::Scalar(scalar))
            if matches_data_pointer(*scalar) =>
        {
            Ok(())
        }
        (
            SemanticPointerMetadataV1::SliceLength,
            SemanticBackendReprV1::ScalarPair { first, second, .. },
        ) if matches_data_pointer(*first) && scalar_is_gfx942_usize(*second) => Ok(()),
        (
            SemanticPointerMetadataV1::VTable,
            SemanticBackendReprV1::ScalarPair { first, second, .. },
        ) if matches_data_pointer(*first) && scalar_is_gfx942_vtable_pointer(*second) => Ok(()),
        _ => Err(SemanticMirErrorV1::InvalidTypeLayout),
    }
}

fn scalar_is_gfx942_usize(scalar: SemanticBackendScalarV1) -> bool {
    matches!(
        scalar,
        SemanticBackendScalarV1::Initialized {
            primitive: SemanticBackendPrimitiveV1::Integer {
                signed: false,
                bits: 64,
                alignment_bytes: 8,
            },
            valid_range,
        } if valid_range == SemanticScalarValidityRangeV1::new(0, u64::MAX.into())
    )
}

fn scalar_is_gfx942_vtable_pointer(scalar: SemanticBackendScalarV1) -> bool {
    matches!(
        scalar,
        SemanticBackendScalarV1::Initialized {
            primitive: SemanticBackendPrimitiveV1::Pointer {
                address_space: 0,
                size_bytes: 8,
                alignment_bytes: 8,
            },
            valid_range,
        } if valid_range == SemanticScalarValidityRangeV1::new(1, u64::MAX.into())
    )
}

fn validate_validity_ranges(
    scalar: SemanticScalarTypeV1,
    ranges: &[SemanticScalarValidityRangeV1],
) -> Result<(), SemanticMirErrorV1> {
    if !scalar.is_integer() || ranges.is_empty() {
        return Err(SemanticMirErrorV1::InvalidTypeLayout);
    }
    let bits = scalar.bits().ok_or(SemanticMirErrorV1::InvalidTypeLayout)?;
    let mut previous_end = None;
    for range in ranges {
        if range.start > range.end
            || !value_fits_bits(range.end, bits)
            || previous_end.is_some_and(|end| end >= range.start)
        {
            return Err(SemanticMirErrorV1::InvalidTypeLayout);
        }
        previous_end = Some(range.end);
    }
    Ok(())
}

type SemanticByteRangeV1 = (u64, u64);

#[allow(clippy::too_many_arguments)]
fn validate_aggregate_layout(
    context: &mut ValidationContextV1<'_>,
    outer_size_bytes: Option<u64>,
    outer_alignment_bytes: u64,
    outer_fields: &SemanticFieldsShapeV1,
    fields: &SemanticAggregateTypeV1,
    layout: &SemanticAggregateLayoutV1,
    reserved: Option<SemanticByteRangeV1>,
    reserved_may_overlap: bool,
) -> Result<(), SemanticMirErrorV1> {
    let SemanticFieldsShapeV1::Arbitrary {
        source_order_offsets_bytes,
        ..
    } = outer_fields
    else {
        return Err(SemanticMirErrorV1::InvalidTypeLayout);
    };
    if source_order_offsets_bytes.as_ref() != layout.field_offsets.as_ref() {
        return Err(SemanticMirErrorV1::InvalidTypeLayout);
    }
    if fields.fields.len() != layout.field_offsets.len() {
        return Err(SemanticMirErrorV1::InvalidTypeLayout);
    }
    charge_validation_work(context, layout.padding.len())?;
    let mut ranges = Vec::with_capacity(fields.fields.len() + usize::from(reserved.is_some()));
    let mut maximum_alignment = 1_u64;
    let mut unsized_tail = None;
    for (index, (field, offset)) in fields
        .fields
        .iter()
        .zip(layout.field_offsets.iter().copied())
        .enumerate()
    {
        context.one()?;
        let field_layout = &context.request.types[field.0 as usize].layout;
        maximum_alignment = maximum_alignment.max(field_layout.alignment_bytes);
        if !offset.is_multiple_of(field_layout.alignment_bytes) {
            return Err(SemanticMirErrorV1::InvalidTypeLayout);
        }
        match field_layout.size_bytes {
            Some(0) => {
                if outer_size_bytes.is_some_and(|size| offset > size) {
                    return Err(SemanticMirErrorV1::InvalidTypeLayout);
                }
            }
            Some(size) => {
                let end = offset
                    .checked_add(size)
                    .ok_or(SemanticMirErrorV1::InvalidTypeLayout)?;
                ranges.push((offset, end));
            }
            None => {
                if unsized_tail.replace(offset).is_some()
                    || index + 1 != fields.fields.len()
                    || outer_size_bytes.is_some()
                {
                    return Err(SemanticMirErrorV1::InvalidTypeLayout);
                }
            }
        }
    }
    if outer_alignment_bytes < maximum_alignment
        || !outer_alignment_bytes.is_multiple_of(maximum_alignment)
        || outer_size_bytes.is_none() != unsized_tail.is_some()
    {
        return Err(SemanticMirErrorV1::InvalidTypeLayout);
    }
    let bound = outer_size_bytes
        .or(unsized_tail)
        .ok_or(SemanticMirErrorV1::InvalidTypeLayout)?;
    if ranges.iter().any(|(_, end)| *end > bound) {
        return Err(SemanticMirErrorV1::InvalidTypeLayout);
    }
    ranges.sort_unstable();
    if ranges.windows(2).any(|pair| pair[0].1 > pair[1].0) {
        return Err(SemanticMirErrorV1::InvalidTypeLayout);
    }
    if let Some(reserved) = reserved {
        if !reserved_may_overlap
            && ranges
                .iter()
                .any(|range| byte_ranges_overlap(*range, reserved))
        {
            return Err(SemanticMirErrorV1::InvalidTypeLayout);
        }
        ranges.push(reserved);
        ranges.sort_unstable();
    }
    let mut previous_padding = None;
    let mut occupied_index = 0;
    for padding in &layout.padding {
        let range = checked_byte_range(padding.offset_bytes, padding.size_bytes, bound)?;
        while ranges
            .get(occupied_index)
            .is_some_and(|occupied| occupied.1 <= range.0)
        {
            occupied_index += 1;
        }
        if previous_padding.is_some_and(|previous_end| previous_end > range.0)
            || ranges
                .get(occupied_index)
                .is_some_and(|occupied| byte_ranges_overlap(*occupied, range))
        {
            return Err(SemanticMirErrorV1::InvalidTypeLayout);
        }
        previous_padding = Some(range.1);
    }
    Ok(())
}

fn validate_union_layout(
    context: &mut ValidationContextV1<'_>,
    outer: &SemanticTypeLayoutV1,
    fields: &SemanticAggregateTypeV1,
) -> Result<(), SemanticMirErrorV1> {
    require_plain_layout(outer)?;
    let SemanticFieldsShapeV1::Union { field_count } = &outer.fields else {
        return Err(SemanticMirErrorV1::InvalidTypeLayout);
    };
    if *field_count != fields.fields.len() as u64 || outer.size_bytes.is_none() {
        return Err(SemanticMirErrorV1::InvalidTypeLayout);
    }
    let outer_size = outer.size_bytes.unwrap_or(0);
    let mut required_alignment = 1;
    for field in &fields.fields {
        context.one()?;
        let layout = &context.request.types[field.0 as usize].layout;
        let size = layout
            .size_bytes
            .ok_or(SemanticMirErrorV1::InvalidTypeLayout)?;
        required_alignment = required_alignment.max(layout.alignment_bytes);
        if size > outer_size {
            return Err(SemanticMirErrorV1::InvalidTypeLayout);
        }
    }
    if outer.alignment_bytes < required_alignment
        || !outer.alignment_bytes.is_multiple_of(required_alignment)
    {
        return Err(SemanticMirErrorV1::InvalidTypeLayout);
    }
    Ok(())
}

fn validate_enum_layout(
    context: &mut ValidationContextV1<'_>,
    ty: &SemanticTypeDeclV1,
    discriminant: SemanticTypeIdV1,
    variants: &[SemanticEnumVariantV1],
) -> Result<(), SemanticMirErrorV1> {
    let size = ty
        .layout
        .size_bytes
        .ok_or(SemanticMirErrorV1::InvalidTypeLayout)?;
    let logical = scalar_shape(&context.request.types[discriminant.0 as usize].shape)
        .filter(|scalar| scalar.is_integer())
        .ok_or(SemanticMirErrorV1::InvalidTypeLayout)?;
    let logical_bits = logical
        .bits()
        .ok_or(SemanticMirErrorV1::InvalidTypeLayout)?;
    if variants
        .iter()
        .any(|variant| !value_fits_bits(variant.discriminant, logical_bits))
    {
        return Err(SemanticMirErrorV1::InvalidTypeLayout);
    }
    let mut discriminants = BTreeSet::new();
    if variants
        .iter()
        .any(|variant| !discriminants.insert(variant.discriminant))
    {
        return Err(SemanticMirErrorV1::InvalidTypeLayout);
    }

    match &ty.layout.variants {
        SemanticRustcVariantsV1::Empty => {
            if !matches!(ty.layout.details, SemanticTypeLayoutDetailsV1::None)
                || variants.iter().any(|variant| !variant.uninhabited)
            {
                return Err(SemanticMirErrorV1::InvalidTypeLayout);
            }
            Ok(())
        }
        SemanticRustcVariantsV1::Single { index } => {
            let selected = variants
                .get(*index as usize)
                .ok_or(SemanticMirErrorV1::InvalidTypeLayout)?;
            if ty.layout.uninhabited
                || selected.uninhabited
                || variants.iter().enumerate().any(|(candidate, variant)| {
                    candidate != *index as usize && !variant.uninhabited
                })
            {
                return Err(SemanticMirErrorV1::InvalidTypeLayout);
            }
            let SemanticTypeLayoutDetailsV1::Aggregate(layout) = &ty.layout.details else {
                return Err(SemanticMirErrorV1::InvalidTypeLayout);
            };
            validate_aggregate_layout(
                context,
                Some(size),
                ty.layout.alignment_bytes,
                &ty.layout.fields,
                &selected.fields,
                layout,
                None,
                false,
            )
        }
        SemanticRustcVariantsV1::Multiple(layout) => {
            if !matches!(ty.layout.details, SemanticTypeLayoutDetailsV1::None) {
                return Err(SemanticMirErrorV1::InvalidTypeLayout);
            }
            validate_multiple_enum_layout(context, ty, variants, layout, logical, size)
        }
    }
}

fn validate_multiple_enum_layout(
    context: &mut ValidationContextV1<'_>,
    ty: &SemanticTypeDeclV1,
    variants: &[SemanticEnumVariantV1],
    layout: &SemanticEnumLayoutV1,
    logical: SemanticScalarTypeV1,
    size: u64,
) -> Result<(), SemanticMirErrorV1> {
    let expected_outer_offset = match &layout.encoding {
        SemanticEnumEncodingV1::Direct(direct) => direct.tag_offset_bytes,
        SemanticEnumEncodingV1::Niche(niche) => niche.source.expected_offset_bytes,
    };
    if layout.variants.len() != variants.len()
        || ty.layout.uninhabited != variants.iter().all(|variant| variant.uninhabited)
        || !matches!(
            &ty.layout.fields,
            SemanticFieldsShapeV1::Arbitrary {
                source_order_offsets_bytes,
                memory_order_source_indices,
            } if source_order_offsets_bytes.as_ref() == [expected_outer_offset]
                && memory_order_source_indices.as_ref() == [0]
        )
    {
        return Err(SemanticMirErrorV1::InvalidTypeLayout);
    }

    let (reserved, may_overlap) = match &layout.encoding {
        SemanticEnumEncodingV1::Direct(direct) => {
            validate_backend_scalar(direct.tag)?;
            validate_target_primitive(context.request.target, direct.tag.primitive())?;
            let Some(tag) = backend_integer_semantic_scalar(direct.tag) else {
                return Err(SemanticMirErrorV1::InvalidTypeLayout);
            };
            let width = direct
                .tag
                .primitive()
                .size_bytes()
                .ok_or(SemanticMirErrorV1::InvalidTypeLayout)?;
            let expected_largest_niche = match direct.tag {
                SemanticBackendScalarV1::Initialized {
                    primitive,
                    valid_range,
                } => layout_niche_from_scalar(direct.tag_offset_bytes, primitive, valid_range)?,
                SemanticBackendScalarV1::Union { .. } => None,
            };
            if direct.tag_field != 0
                || direct.tag_offset_bytes != 0
                || ty.layout.largest_niche != expected_largest_niche
                || variants.is_empty()
                || variants
                    .iter()
                    .filter(|variant| !variant.uninhabited)
                    .any(|variant| {
                        !discriminant_fits_tag(variant.discriminant, logical, tag)
                            || !backend_scalar_contains_discriminant(
                                direct.tag,
                                variant.discriminant,
                                logical,
                            )
                    })
            {
                return Err(SemanticMirErrorV1::InvalidTypeLayout);
            }
            (
                Some(checked_byte_range(direct.tag_offset_bytes, width, size)?),
                false,
            )
        }
        SemanticEnumEncodingV1::Niche(niche) => {
            validate_backend_scalar(niche.tag)?;
            validate_target_primitive(context.request.target, niche.tag.primitive())?;
            let width = niche
                .tag
                .primitive()
                .size_bytes()
                .ok_or(SemanticMirErrorV1::InvalidTypeLayout)?;
            if niche.tag_field != 0 {
                return Err(SemanticMirErrorV1::InvalidTypeLayout);
            }
            (
                Some(checked_byte_range(
                    niche.source.expected_offset_bytes,
                    width,
                    size,
                )?),
                true,
            )
        }
    };
    for (index, (variant, variant_layout)) in
        variants.iter().zip(layout.variants.iter()).enumerate()
    {
        validate_enum_variant_layout(context, variant_layout, ty.layout.rustc_size_bytes)?;
        if variant_layout.variant_index != index as u32
            || variant_layout.uninhabited != variant.uninhabited
            || variant_layout.rustc_size_bytes > size
            || variant_layout.alignment_bytes > ty.layout.alignment_bytes
            || !enum_variant_backend_is_coherent(&ty.layout.backend_repr, variant_layout)?
        {
            return Err(SemanticMirErrorV1::InvalidTypeLayout);
        }
        validate_aggregate_layout(
            context,
            Some(variant_layout.rustc_size_bytes),
            variant_layout.alignment_bytes,
            &variant_layout.fields,
            &variant.fields,
            &variant_layout.aggregate,
            reserved,
            may_overlap,
        )?;
    }
    if let SemanticEnumEncodingV1::Niche(niche) = &layout.encoding {
        validate_niche_encoding(context, ty, variants, layout, niche)?;
    }
    Ok(())
}

fn enum_variant_backend_is_coherent(
    outer: &SemanticBackendReprV1,
    variant: &SemanticEnumVariantLayoutV1,
) -> Result<bool, SemanticMirErrorV1> {
    if variant.rustc_size_bytes == 0
        || variant.aggregate.field_offsets.is_empty()
        || variant.uninhabited
    {
        return Ok(true);
    }
    match (outer, &variant.backend_repr) {
        (SemanticBackendReprV1::Memory { .. }, _) => Ok(true),
        (SemanticBackendReprV1::Scalar(left), SemanticBackendReprV1::Scalar(right)) => {
            validate_backend_scalar(*left)?;
            validate_backend_scalar(*right)?;
            Ok(left.primitive() == right.primitive())
        }
        (
            SemanticBackendReprV1::ScalarPair {
                first: left_first,
                second: left_second,
            },
            SemanticBackendReprV1::ScalarPair {
                first: right_first,
                second: right_second,
            },
        ) => {
            validate_backend_scalar(*left_first)?;
            validate_backend_scalar(*right_first)?;
            validate_backend_scalar(*left_second)?;
            validate_backend_scalar(*right_second)?;
            Ok(left_first.primitive() == right_first.primitive()
                && left_second.primitive() == right_second.primitive())
        }
        _ => Ok(false),
    }
}

fn validate_enum_variant_layout(
    context: &mut ValidationContextV1<'_>,
    variant: &SemanticEnumVariantLayoutV1,
    outer_size_bytes: u64,
) -> Result<(), SemanticMirErrorV1> {
    if variant.rustc_size_bytes >= context.request.target.object_size_bound_bytes
        || variant.rustc_size_bytes > outer_size_bytes
        || !valid_rustc_alignment(variant.alignment_bytes)
        || (!variant
            .rustc_size_bytes
            .is_multiple_of(variant.alignment_bytes)
            && !backend_repr_is_overaligned_pointer(variant.backend_repr, variant.rustc_size_bytes))
        || matches!(
            variant.backend_repr,
            SemanticBackendReprV1::Memory { sized: false }
        )
        || variant
            .max_repr_alignment_bytes
            .is_some_and(|alignment| !valid_rustc_alignment(alignment))
        || !valid_rustc_alignment(variant.unadjusted_abi_alignment_bytes)
    {
        return Err(SemanticMirErrorV1::InvalidTypeLayout);
    }
    charge_fields_shape_work(context, &variant.fields)?;
    validate_fields_shape(&variant.fields, Some(variant.rustc_size_bytes))?;
    validate_backend_repr(
        Some(variant.rustc_size_bytes),
        variant.alignment_bytes,
        &variant.backend_repr,
    )?;
    validate_target_backend_repr(
        context.request.target,
        &SemanticTypeLayoutV1 {
            rustc_size_bytes: variant.rustc_size_bytes,
            size_bytes: Some(variant.rustc_size_bytes),
            alignment_bytes: variant.alignment_bytes,
            fields: variant.fields.clone(),
            variants: SemanticRustcVariantsV1::Single {
                index: variant.variant_index,
            },
            backend_repr: variant.backend_repr,
            largest_niche: variant.largest_niche,
            uninhabited: variant.uninhabited,
            max_repr_alignment_bytes: variant.max_repr_alignment_bytes,
            unadjusted_abi_alignment_bytes: variant.unadjusted_abi_alignment_bytes,
            randomization_seed: variant.randomization_seed,
            details: SemanticTypeLayoutDetailsV1::None,
        },
    )?;
    if let Some(niche) = variant.largest_niche {
        validate_layout_niche(niche, Some(variant.rustc_size_bytes))?;
    }
    let SemanticFieldsShapeV1::Arbitrary {
        source_order_offsets_bytes,
        ..
    } = &variant.fields
    else {
        return Err(SemanticMirErrorV1::InvalidTypeLayout);
    };
    if source_order_offsets_bytes.as_ref() != variant.aggregate.field_offsets.as_ref() {
        return Err(SemanticMirErrorV1::InvalidTypeLayout);
    }
    Ok(())
}

fn validate_niche_encoding(
    context: &mut ValidationContextV1<'_>,
    ty: &SemanticTypeDeclV1,
    variants: &[SemanticEnumVariantV1],
    layout: &SemanticEnumLayoutV1,
    niche: &SemanticNicheEnumEncodingV1,
) -> Result<(), SemanticMirErrorV1> {
    let (start, end) = niche.niche_variant_range();
    if start > end
        || end as usize >= variants.len()
        || niche.untagged_variant as usize >= variants.len()
        || variants
            .iter()
            .enumerate()
            .any(|(index, variant)| variant.discriminant != index as u128)
        || variants.iter().enumerate().any(|(index, variant)| {
            index != niche.untagged_variant as usize
                && !(start as usize..=end as usize).contains(&index)
                && !variant.uninhabited
        })
    {
        return Err(SemanticMirErrorV1::InvalidTypeLayout);
    }
    let (offset, terminal_scalar) = resolve_niche_source(context, variants, layout, niche)?;
    if layout.variants[niche.untagged_variant as usize].largest_niche != Some(niche.source_niche)
        || layout.variants.iter().enumerate().any(|(index, variant)| {
            index != niche.untagged_variant as usize && variant.largest_niche.is_some()
        })
        || offset != niche.source.expected_offset_bytes
        || offset != niche.source_niche.offset_bytes
        || terminal_scalar
            != SemanticBackendScalarV1::initialized(
                niche.source_niche.primitive,
                niche.source_niche.valid_range,
            )
    {
        return Err(SemanticMirErrorV1::InvalidTypeLayout);
    }
    let count = u128::from(end - start) + 1;
    let (expected_start, expected_tag) = reserve_layout_niche(niche.source_niche, count)
        .ok_or(SemanticMirErrorV1::InvalidTypeLayout)?;
    let expected_outer_niche = match expected_tag {
        SemanticBackendScalarV1::Initialized {
            primitive,
            valid_range,
        } => layout_niche_from_scalar(offset, primitive, valid_range)?,
        SemanticBackendScalarV1::Union { .. } => {
            return Err(SemanticMirErrorV1::InvalidTypeLayout);
        }
    };
    if niche.niche_start != expected_start
        || niche.tag != expected_tag
        || ty.layout.largest_niche != expected_outer_niche
    {
        return Err(SemanticMirErrorV1::InvalidTypeLayout);
    }
    checked_byte_range(
        offset,
        niche.source_niche.primitive.size_bytes().unwrap_or(0),
        ty.layout.size_bytes.unwrap_or(0),
    )?;
    Ok(())
}

fn reserve_layout_niche(
    niche: SemanticLayoutNicheV1,
    count: u128,
) -> Option<(u128, SemanticBackendScalarV1)> {
    if count == 0 {
        return None;
    }
    let bits = niche.primitive.bits()?;
    let mask = unsigned_max(bits);
    let valid = niche.valid_range;
    let first_after_valid = valid.end.wrapping_add(1) & mask;
    let available = valid.start.wrapping_sub(first_after_valid) & mask;
    if count > available {
        return None;
    }
    let move_start = || {
        let start = valid.start.wrapping_sub(count) & mask;
        (
            start,
            SemanticBackendScalarV1::initialized(
                niche.primitive,
                SemanticScalarValidityRangeV1::new(start, valid.end),
            ),
        )
    };
    let move_end = || {
        let start = valid.end.wrapping_add(1) & mask;
        let end = valid.end.wrapping_add(count) & mask;
        (
            start,
            SemanticBackendScalarV1::initialized(
                niche.primitive,
                SemanticScalarValidityRangeV1::new(valid.start, end),
            ),
        )
    };
    let distance_end_zero = mask - valid.end;
    Some(if valid.start > valid.end {
        move_end()
    } else if valid.start <= distance_end_zero {
        if count <= valid.start {
            move_start()
        } else {
            move_end()
        }
    } else {
        let end = valid.end.wrapping_add(count) & mask;
        if (1..=valid.end).contains(&end) {
            move_start()
        } else {
            move_end()
        }
    })
}

fn resolve_niche_source(
    context: &mut ValidationContextV1<'_>,
    variants: &[SemanticEnumVariantV1],
    layout: &SemanticEnumLayoutV1,
    niche: &SemanticNicheEnumEncodingV1,
) -> Result<(u64, SemanticBackendScalarV1), SemanticMirErrorV1> {
    let variant_index = niche.untagged_variant as usize;
    let variant = &variants[variant_index];
    let variant_layout = &layout.variants[variant_index];
    let mut current: Option<SemanticTypeIdV1> = None;
    let mut offset = 0_u64;
    for component in niche.source.path.iter().copied() {
        context.one()?;
        let next = match (component, current) {
            (SemanticNichePathComponentV1::Field(index), None) => {
                let position = index as usize;
                offset = offset
                    .checked_add(
                        *variant_layout
                            .aggregate
                            .field_offsets
                            .get(position)
                            .ok_or(SemanticMirErrorV1::InvalidTypeLayout)?,
                    )
                    .ok_or(SemanticMirErrorV1::InvalidTypeLayout)?;
                *variant
                    .fields
                    .fields
                    .get(position)
                    .ok_or(SemanticMirErrorV1::InvalidTypeLayout)?
            }
            (SemanticNichePathComponentV1::Field(index), Some(id)) => {
                let node = context
                    .request
                    .types
                    .get(id.0 as usize)
                    .ok_or(SemanticMirErrorV1::InvalidTypeLayout)?;
                let fields = match &node.shape {
                    SemanticTypeShapeV1::Tuple(fields) | SemanticTypeShapeV1::Aggregate(fields) => {
                        fields
                    }
                    _ => return Err(SemanticMirErrorV1::InvalidTypeLayout),
                };
                let SemanticTypeLayoutDetailsV1::Aggregate(node_layout) = &node.layout.details
                else {
                    return Err(SemanticMirErrorV1::InvalidTypeLayout);
                };
                let position = index as usize;
                offset = offset
                    .checked_add(
                        *node_layout
                            .field_offsets
                            .get(position)
                            .ok_or(SemanticMirErrorV1::InvalidTypeLayout)?,
                    )
                    .ok_or(SemanticMirErrorV1::InvalidTypeLayout)?;
                *fields
                    .fields
                    .get(position)
                    .ok_or(SemanticMirErrorV1::InvalidTypeLayout)?
            }
            (SemanticNichePathComponentV1::ArrayElement(index), Some(id)) => {
                let node = context
                    .request
                    .types
                    .get(id.0 as usize)
                    .ok_or(SemanticMirErrorV1::InvalidTypeLayout)?;
                let SemanticTypeShapeV1::Array { element, length } = node.shape else {
                    return Err(SemanticMirErrorV1::InvalidTypeLayout);
                };
                if index >= length {
                    return Err(SemanticMirErrorV1::InvalidTypeLayout);
                }
                let stride = context
                    .request
                    .types
                    .get(element.0 as usize)
                    .ok_or(SemanticMirErrorV1::InvalidTypeLayout)?
                    .layout
                    .size_bytes
                    .ok_or(SemanticMirErrorV1::InvalidTypeLayout)?;
                offset = offset
                    .checked_add(
                        stride
                            .checked_mul(index)
                            .ok_or(SemanticMirErrorV1::InvalidTypeLayout)?,
                    )
                    .ok_or(SemanticMirErrorV1::InvalidTypeLayout)?;
                element
            }
            (SemanticNichePathComponentV1::ArrayElement(_), None) => {
                return Err(SemanticMirErrorV1::InvalidTypeLayout);
            }
        };
        current = Some(next);
    }
    let terminal = current.ok_or(SemanticMirErrorV1::InvalidTypeLayout)?;
    let terminal = context
        .request
        .types
        .get(terminal.0 as usize)
        .ok_or(SemanticMirErrorV1::InvalidTypeLayout)?;
    let expected_offset = niche.source.expected_offset_bytes;
    let relative_offset = expected_offset
        .checked_sub(offset)
        .ok_or(SemanticMirErrorV1::InvalidTypeLayout)?;
    let terminal_scalar = match terminal.layout.backend_repr {
        SemanticBackendReprV1::Scalar(scalar) if relative_offset == 0 => scalar,
        SemanticBackendReprV1::ScalarPair { first, second } => {
            let first_size = first
                .primitive()
                .size_bytes()
                .ok_or(SemanticMirErrorV1::InvalidTypeLayout)?;
            let second_offset = align_up(first_size, second.primitive().alignment_bytes())?;
            if relative_offset == 0 {
                first
            } else if relative_offset == second_offset {
                second
            } else {
                return Err(SemanticMirErrorV1::InvalidTypeLayout);
            }
        }
        _ => return Err(SemanticMirErrorV1::InvalidTypeLayout),
    };
    Ok((expected_offset, terminal_scalar))
}

fn charge_validation_work(
    context: &mut ValidationContextV1<'_>,
    count: usize,
) -> Result<(), SemanticMirErrorV1> {
    for _ in 0..count {
        context.one()?;
    }
    Ok(())
}

fn scalar_shape(shape: &SemanticTypeShapeV1) -> Option<SemanticScalarTypeV1> {
    match shape {
        SemanticTypeShapeV1::Scalar(scalar) => Some(*scalar),
        SemanticTypeShapeV1::ValidityScalar(validity) => Some(validity.scalar),
        _ => None,
    }
}

fn backend_integer_semantic_scalar(
    scalar: SemanticBackendScalarV1,
) -> Option<SemanticScalarTypeV1> {
    let SemanticBackendPrimitiveV1::Integer { signed, bits, .. } = scalar.primitive() else {
        return None;
    };
    Some(SemanticScalarTypeV1::Integer { signed, bits })
}

fn backend_scalar_contains_discriminant(
    scalar: SemanticBackendScalarV1,
    raw: u128,
    logical: SemanticScalarTypeV1,
) -> bool {
    let Some(valid_range) = scalar.valid_range() else {
        return false;
    };
    let Some(SemanticScalarTypeV1::Integer { bits, .. }) = backend_integer_semantic_scalar(scalar)
    else {
        return false;
    };
    if !discriminant_fits_tag(
        raw,
        logical,
        SemanticScalarTypeV1::Integer {
            signed: matches!(
                scalar.primitive(),
                SemanticBackendPrimitiveV1::Integer { signed: true, .. }
            ),
            bits,
        },
    ) {
        return false;
    }
    let encoded = raw & unsigned_max(bits);
    if valid_range.start <= valid_range.end {
        (valid_range.start..=valid_range.end).contains(&encoded)
    } else {
        encoded >= valid_range.start || encoded <= valid_range.end
    }
}

fn value_fits_bits(value: u128, bits: u16) -> bool {
    bits == 128 || (bits > 0 && value < (1_u128 << bits))
}

fn discriminant_fits_tag(
    raw: u128,
    logical: SemanticScalarTypeV1,
    physical: SemanticScalarTypeV1,
) -> bool {
    let SemanticScalarTypeV1::Integer {
        signed: logical_signed,
        bits: logical_bits,
    } = logical
    else {
        return false;
    };
    let SemanticScalarTypeV1::Integer {
        signed: physical_signed,
        bits: physical_bits,
    } = physical
    else {
        return false;
    };
    if logical_signed {
        let value = sign_extend_discriminant(raw, logical_bits);
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

fn sign_extend_discriminant(raw: u128, bits: u16) -> i128 {
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

fn checked_byte_range(
    offset: u64,
    width: u64,
    bound: u64,
) -> Result<SemanticByteRangeV1, SemanticMirErrorV1> {
    let end = offset
        .checked_add(width)
        .ok_or(SemanticMirErrorV1::InvalidTypeLayout)?;
    if width == 0 || end > bound {
        return Err(SemanticMirErrorV1::InvalidTypeLayout);
    }
    Ok((offset, end))
}

fn byte_ranges_overlap(left: SemanticByteRangeV1, right: SemanticByteRangeV1) -> bool {
    left.0 < right.1 && right.0 < left.1
}

fn validate_type_list(
    context: &mut ValidationContextV1<'_>,
    fields: &SemanticAggregateTypeV1,
    location: SemanticMirLocationV1,
) -> Result<(), SemanticMirErrorV1> {
    for field in &fields.fields {
        context.type_reference(*field, location)?;
    }
    Ok(())
}

fn validate_allocation(
    context: &mut ValidationContextV1<'_>,
    id: SemanticAllocationIdV1,
    allocation: &SemanticAllocationDeclV1,
) -> Result<(), SemanticMirErrorV1> {
    let location = SemanticMirLocationV1::Allocation(id);
    let object_size_bound =
        target_object_size_bound_in(context.request.target, allocation.address_space)
            .ok_or(SemanticMirErrorV1::InvalidAllocation)?;
    if allocation.bytes.len() as u128 >= u128::from(object_size_bound) {
        return Err(SemanticMirErrorV1::InvalidAllocation);
    }
    if !valid_rustc_alignment(allocation.alignment_bytes) {
        return Err(SemanticMirErrorV1::InvalidAllocation);
    }
    context.totals.charge(
        SemanticMirResourceV1::ConstantBytes,
        allocation.bytes.len(),
        context.limits,
    )?;
    context.totals.charge(
        SemanticMirResourceV1::ConstantBytes,
        allocation.initialized_mask.len(),
        context.limits,
    )?;
    context.totals.charge(
        SemanticMirResourceV1::Relocations,
        allocation.relocations.len(),
        context.limits,
    )?;
    for relocation in &allocation.relocations {
        context.one()?;
        let (target_address_space, target_addend_valid) = match relocation.target {
            SemanticRelocationTargetV1::Allocation(target) => {
                context.allocation_reference(target, location)?;
                let target = &context.request.allocations[target.0 as usize];
                (
                    target.address_space,
                    relocation.addend >= 0
                        && u64::try_from(relocation.addend)
                            .is_ok_and(|addend| addend <= target.bytes.len() as u64),
                )
            }
            SemanticRelocationTargetV1::Callable(target) => {
                context.callable_reference(target, location)?;
                addressable_callable_abi(context.request, target)?;
                (0, relocation.addend == 0)
            }
            SemanticRelocationTargetV1::Static(target) => {
                context.static_reference(target, location)?;
                let target = &context.request.statics[target.0 as usize];
                let target_size = context
                    .request
                    .types
                    .get(target.ty.0 as usize)
                    .and_then(|ty| ty.layout.size_bytes)
                    .ok_or(SemanticMirErrorV1::InvalidRelocation)?;
                (
                    target.address_space,
                    relocation.addend >= 0
                        && u64::try_from(relocation.addend)
                            .is_ok_and(|addend| addend <= target_size),
                )
            }
            SemanticRelocationTargetV1::VTable(target) => {
                context.vtable_reference(target, location)?;
                (0, relocation.addend == 0)
            }
        };
        let pointer_width = target_pointer_profile(context.request.target, target_address_space)
            .ok_or(SemanticMirErrorV1::InvalidRelocation)?
            .size_bytes;
        let start = usize::try_from(relocation.byte_offset)
            .map_err(|_| SemanticMirErrorV1::InvalidRelocation)?;
        let end = start
            .checked_add(usize::from(relocation.width_bytes))
            .ok_or(SemanticMirErrorV1::InvalidRelocation)?;
        if relocation.address_space != target_address_space
            || u64::from(relocation.width_bytes) != pointer_width
            || !target_addend_valid
            || !allocation_range_is_initialized(allocation, start, end)
            || allocation.bytes[start..end].iter().any(|byte| *byte != 0)
        {
            return Err(SemanticMirErrorV1::InvalidRelocation);
        }
    }
    Ok(())
}

fn validate_statics(
    context: &mut ValidationContextV1<'_>,
) -> Result<BTreeSet<SemanticAllocationIdV1>, SemanticMirErrorV1> {
    let mut claimed_initializers = BTreeSet::new();
    let mut external_symbols = BTreeSet::new();
    for (index, static_decl) in context.request.statics.iter().enumerate() {
        context.one()?;
        let static_id = SemanticStaticIdV1(index as u32);
        let location = SemanticMirLocationV1::Static(static_id);
        context.type_reference(static_decl.ty, location)?;
        let layout = &context.request.types[static_decl.ty.0 as usize].layout;
        let Some(size_bytes) = layout.size_bytes else {
            return Err(SemanticMirErrorV1::InvalidStatic);
        };
        let Some(object_size_bound) =
            target_object_size_bound_in(context.request.target, static_decl.address_space)
        else {
            return Err(SemanticMirErrorV1::InvalidStatic);
        };
        if size_bytes >= object_size_bound {
            return Err(SemanticMirErrorV1::InvalidStatic);
        }
        match &static_decl.definition {
            SemanticStaticDefinitionV1::Defined { initializer } => {
                if let Some(symbol) = &static_decl.export_symbol {
                    context.totals.charge(
                        SemanticMirResourceV1::LinkSymbolBytes,
                        symbol.0.len(),
                        context.limits,
                    )?;
                    if !external_symbols.insert(symbol.0.as_ref()) {
                        return Err(SemanticMirErrorV1::InvalidStatic);
                    }
                }
                context.allocation_reference(*initializer, location)?;
                if !claimed_initializers.insert(*initializer) {
                    return Err(SemanticMirErrorV1::InvalidStatic);
                }
                let allocation = &context.request.allocations[initializer.0 as usize];
                if allocation.bytes.len() as u128 != u128::from(size_bytes)
                    || allocation.address_space != static_decl.address_space
                    || allocation.mutable != static_decl.mutable
                    || allocation.alignment_bytes < layout.alignment_bytes
                    || !allocation
                        .alignment_bytes
                        .is_multiple_of(layout.alignment_bytes)
                {
                    return Err(SemanticMirErrorV1::InvalidStatic);
                }
            }
            SemanticStaticDefinitionV1::ExternalRequired { symbol } => {
                if static_decl.export_symbol.is_some() {
                    return Err(SemanticMirErrorV1::InvalidStatic);
                }
                context.totals.charge(
                    SemanticMirResourceV1::LinkSymbolBytes,
                    symbol.0.len(),
                    context.limits,
                )?;
                if symbol.0.is_empty()
                    || symbol.0.contains(&0)
                    || !external_symbols.insert(symbol.0.as_ref())
                {
                    return Err(SemanticMirErrorV1::InvalidStatic);
                }
            }
        }
    }
    Ok(claimed_initializers)
}

fn validate_vtables(
    context: &mut ValidationContextV1<'_>,
    mut claimed_allocations: BTreeSet<SemanticAllocationIdV1>,
) -> Result<(), SemanticMirErrorV1> {
    let mut trait_edges = vec![Vec::new(); context.request.vtables.len()];
    for (index, vtable) in context.request.vtables.iter().enumerate() {
        context.one()?;
        let vtable_id = SemanticVTableIdV1(index as u32);
        let location = SemanticMirLocationV1::VTable(vtable_id);
        context.type_reference(vtable.concrete_type, location)?;
        context.type_reference(vtable.dyn_type, location)?;
        context.allocation_reference(vtable.allocation, location)?;
        let dyn_type = &context.request.types[vtable.dyn_type.0 as usize];
        if let Some(drop_glue) = vtable.header.drop_glue {
            context.function_reference(drop_glue, location)?;
            if context.request.functions[drop_glue.0 as usize].role
                != SemanticFunctionRoleV1::DropGlue(vtable.concrete_type)
            {
                return Err(SemanticMirErrorV1::InvalidAllocation);
            }
        }
        charge_validation_work(context, vtable.trait_identity.dyn_predicates.len())?;
        charge_validation_work(context, vtable.slots.len())?;
        if vtable.concrete_type == vtable.dyn_type
            || !matches!(dyn_type.shape, SemanticTypeShapeV1::Opaque)
            || dyn_type.layout.size_bytes.is_some()
            || !matches!(
                dyn_type.layout.backend_repr,
                SemanticBackendReprV1::Memory { sized: false }
            )
            || vtable.trait_identity.dyn_predicates.is_empty()
            || vtable
                .trait_identity
                .dyn_predicates
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || !claimed_allocations.insert(vtable.allocation)
        {
            return Err(SemanticMirErrorV1::InvalidAllocation);
        }
        let allocation = &context.request.allocations[vtable.allocation.0 as usize];
        let concrete_layout = &context.request.types[vtable.concrete_type.0 as usize].layout;
        let expected_allocation_len = vtable
            .slots
            .len()
            .checked_mul(8)
            .and_then(|slots| slots.checked_add(24))
            .ok_or(SemanticMirErrorV1::ArithmeticOverflow {
                resource: SemanticMirResourceV1::Relocations,
            })?;
        if allocation.address_space != 0
            || allocation.mutable
            || allocation.bytes.len() != expected_allocation_len
            || !allocation.bytes.len().is_multiple_of(8)
            || allocation.alignment_bytes < 8
            || concrete_layout.size_bytes != Some(vtable.header.size_bytes)
            || concrete_layout.alignment_bytes != vtable.header.alignment_bytes
            || !allocation_range_is_initialized(allocation, 0, 24)
            || allocation.bytes[..8] != [0; 8]
            || allocation_u64_le(allocation, 8) != Some(vtable.header.size_bytes)
            || allocation_u64_le(allocation, 16) != Some(vtable.header.alignment_bytes)
        {
            return Err(SemanticMirErrorV1::InvalidAllocation);
        }
        let mut relocation_index = 0;
        if let Some(drop_glue) = vtable.header.drop_glue {
            let Some(relocation) = allocation.relocations.get(relocation_index) else {
                return Err(SemanticMirErrorV1::InvalidAllocation);
            };
            if !vtable_relocation_matches(
                relocation,
                0,
                SemanticRelocationTargetV1::Callable(SemanticCallableIdV1(drop_glue.0)),
            ) {
                return Err(SemanticMirErrorV1::InvalidAllocation);
            }
            relocation_index += 1;
        }
        for (slot_index, slot) in vtable.slots.iter().enumerate() {
            let byte_offset = 24 + slot_index * 8;
            if allocation.bytes[byte_offset..byte_offset + 8] != [0; 8] {
                return Err(SemanticMirErrorV1::InvalidAllocation);
            }
            match slot {
                SemanticVTableSlotV1::Vacant => {
                    if !allocation_range_is_uninitialized(allocation, byte_offset, byte_offset + 8)
                    {
                        return Err(SemanticMirErrorV1::InvalidAllocation);
                    }
                }
                SemanticVTableSlotV1::Method(method) => {
                    context.function_reference(*method, location)?;
                    if context.request.functions[method.0 as usize].role
                        != SemanticFunctionRoleV1::InternalHelper
                        || !allocation_range_is_initialized(
                            allocation,
                            byte_offset,
                            byte_offset + 8,
                        )
                        || !allocation
                            .relocations
                            .get(relocation_index)
                            .is_some_and(|relocation| {
                                vtable_relocation_matches(
                                    relocation,
                                    byte_offset as u64,
                                    SemanticRelocationTargetV1::Callable(SemanticCallableIdV1(
                                        method.0,
                                    )),
                                )
                            })
                    {
                        return Err(SemanticMirErrorV1::InvalidAllocation);
                    }
                    relocation_index += 1;
                }
                SemanticVTableSlotV1::TraitVPtr { trait_ref, target } => {
                    context.vtable_reference(*target, location)?;
                    let target_vtable = &context.request.vtables[target.0 as usize];
                    if *target == vtable_id
                        || target_vtable.concrete_type != vtable.concrete_type
                        || *trait_ref != target_vtable.trait_identity.primary_trait_ref
                        || !allocation_range_is_initialized(
                            allocation,
                            byte_offset,
                            byte_offset + 8,
                        )
                        || !allocation
                            .relocations
                            .get(relocation_index)
                            .is_some_and(|relocation| {
                                vtable_relocation_matches(
                                    relocation,
                                    byte_offset as u64,
                                    SemanticRelocationTargetV1::VTable(*target),
                                )
                            })
                    {
                        return Err(SemanticMirErrorV1::InvalidAllocation);
                    }
                    trait_edges[index].push(target.0 as usize);
                    relocation_index += 1;
                }
            }
        }
        if relocation_index != allocation.relocations.len() {
            return Err(SemanticMirErrorV1::InvalidAllocation);
        }
    }
    validate_vtable_trait_graph(context, &trait_edges)
}

fn vtable_relocation_matches(
    relocation: &SemanticRelocationV1,
    byte_offset: u64,
    target: SemanticRelocationTargetV1,
) -> bool {
    relocation.byte_offset == byte_offset
        && relocation.width_bytes == 8
        && relocation.address_space == 0
        && relocation.addend == 0
        && relocation.target == target
}

fn validate_vtable_trait_graph(
    context: &mut ValidationContextV1<'_>,
    edges: &[Vec<usize>],
) -> Result<(), SemanticMirErrorV1> {
    let mut indegrees = vec![0_usize; edges.len()];
    for targets in edges {
        for target in targets {
            indegrees[*target] = indegrees[*target]
                .checked_add(1)
                .ok_or(SemanticMirErrorV1::InvalidAllocation)?;
        }
    }
    let mut pending = indegrees
        .iter()
        .enumerate()
        .filter_map(|(index, indegree)| (*indegree == 0).then_some(index))
        .collect::<VecDeque<_>>();
    let mut visited = 0;
    while let Some(source) = pending.pop_front() {
        context.one()?;
        visited += 1;
        for target in &edges[source] {
            let indegree = &mut indegrees[*target];
            *indegree -= 1;
            if *indegree == 0 {
                pending.push_back(*target);
            }
        }
    }
    if visited == edges.len() {
        Ok(())
    } else {
        Err(SemanticMirErrorV1::InvalidAllocation)
    }
}

fn allocation_u64_le(allocation: &SemanticAllocationDeclV1, offset: usize) -> Option<u64> {
    let bytes: [u8; 8] = allocation
        .bytes
        .get(offset..offset.checked_add(8)?)?
        .try_into()
        .ok()?;
    Some(u64::from_le_bytes(bytes))
}

fn allocation_range_is_initialized(
    allocation: &SemanticAllocationDeclV1,
    start: usize,
    end: usize,
) -> bool {
    end <= allocation.bytes.len()
        && (start..end)
            .all(|byte| allocation.initialized_mask[byte / 8] & (1_u8 << (byte % 8)) != 0)
}

fn allocation_range_is_uninitialized(
    allocation: &SemanticAllocationDeclV1,
    start: usize,
    end: usize,
) -> bool {
    end <= allocation.bytes.len()
        && (start..end)
            .all(|byte| allocation.initialized_mask[byte / 8] & (1_u8 << (byte % 8)) == 0)
}

fn validate_function(
    context: &mut ValidationContextV1<'_>,
    id: SemanticFunctionIdV1,
    function: &SemanticFunctionDeclV1,
) -> Result<(), SemanticMirErrorV1> {
    let function_location = SemanticMirLocationV1::Function(id);
    context.totals.charge(
        SemanticMirResourceV1::Locals,
        function.locals.len(),
        context.limits,
    )?;
    context.totals.charge(
        SemanticMirResourceV1::Blocks,
        function.blocks.len(),
        context.limits,
    )?;
    let identity_order_work = function
        .locals
        .len()
        .checked_add(function.blocks.len())
        .ok_or(SemanticMirErrorV1::ArithmeticOverflow {
            resource: SemanticMirResourceV1::ValidationWork,
        })?;
    charge_validation_work(context, identity_order_work)?;
    if function.locals.is_empty() {
        return Err(SemanticMirErrorV1::EmptyModel {
            entity: SemanticMirEntityV1::Local,
        });
    }
    if function.blocks.is_empty() {
        return Err(SemanticMirErrorV1::EmptyModel {
            entity: SemanticMirEntityV1::Block,
        });
    }
    validate_function_abi_contract(context.request.target, &function.abi)?;
    if let SemanticFunctionRoleV1::DropGlue(dropped_type) = function.role {
        context.type_reference(dropped_type, function_location)?;
    }
    context.type_reference(function.abi.return_value.source_ty, function_location)?;
    if let Some(adjusted) = function.abi.return_value.adjusted() {
        context.type_reference(adjusted.ty, function_location)?;
    }
    if function.abi.return_value.pointee_override.is_some() {
        return Err(SemanticMirErrorV1::InvalidFunctionAbi);
    }
    validate_abi_value(
        context,
        &function.abi.return_value,
        function.abi.canon_abi,
        function.abi.extern_abi(),
        SemanticAbiValuePositionV1::Return,
    )?;
    validate_abi_argument_contract(
        function.abi.extern_abi(),
        function.abi.c_variadic(),
        function.abi.fixed_count,
        function.abi.source_input_types(),
        function.abi.source_output_type(),
        &function.abi.arguments,
        function.abi.return_value.source_ty,
    )?;
    context.totals.charge(
        SemanticMirResourceV1::CallArguments,
        function.abi.source_input_types().len(),
        context.limits,
    )?;
    for source_type in function.abi.source_input_types() {
        context.type_reference(*source_type, function_location)?;
    }
    context.type_reference(function.abi.source_output_type(), function_location)?;
    validate_rust_call_expansion(context.request, &function.abi)?;
    context.totals.charge(
        SemanticMirResourceV1::CallArguments,
        function.abi.arguments.len(),
        context.limits,
    )?;
    for argument in &function.abi.arguments {
        context.type_reference(argument.value.source_ty, function_location)?;
        if argument.value.adjusted().is_some() {
            // A thin virtual receiver is meaningful only together with an
            // authenticated rustc virtual-instance declaration. V1 does not
            // yet expose that callable kind, so defined bodies fail closed.
            return Err(SemanticMirErrorV1::InvalidFunctionAbi);
        }
        if argument.value.pointee_override.is_some()
            && !matches!(function.role, SemanticFunctionRoleV1::DropGlue(_))
        {
            return Err(SemanticMirErrorV1::InvalidFunctionAbi);
        }
        if let SemanticAbiArgumentRoleV1::Hidden(role) = argument.role {
            validate_hidden_abi_argument(context.request, role, &argument.value)?;
        }
        validate_abi_value(
            context,
            &argument.value,
            function.abi.canon_abi,
            function.abi.extern_abi(),
            SemanticAbiValuePositionV1::AdjustedArgument,
        )?;
    }
    if let SemanticFunctionRoleV1::DropGlue(dropped_type) = function.role {
        validate_drop_glue_abi(context.request, dropped_type, &function.abi)?;
    }
    ensure_identity_order(
        function.locals.iter().map(|record| record.identity.0),
        SemanticMirEntityV1::Local,
    )?;
    ensure_identity_order(
        function.blocks.iter().map(|record| record.identity.0),
        SemanticMirEntityV1::Block,
    )?;
    validate_local_roles(context, id, function)?;
    context.reference(
        SemanticMirReferenceV1::Block,
        function.entry.0,
        function.blocks.len(),
        function_location,
    )?;
    for (block_index, block) in function.blocks.iter().enumerate() {
        context.one()?;
        let block_id = SemanticBlockIdV1(block_index as u32);
        context.totals.charge(
            SemanticMirResourceV1::Statements,
            block.statements.len(),
            context.limits,
        )?;
        for (statement_index, statement) in block.statements.iter().enumerate() {
            let location = SemanticMirLocationV1::Statement {
                function: id,
                block: block_id,
                statement: statement_index as u32,
            };
            context.one()?;
            validate_statement(context, function, location, &statement.kind)?;
        }
        let location = SemanticMirLocationV1::Terminator {
            function: id,
            block: block_id,
        };
        validate_terminator(context, id, function, location, &block.terminator.kind)?;
    }
    Ok(())
}

fn validate_hidden_abi_argument(
    request: &InertSemanticMirRequestV1,
    role: SemanticAbiHiddenArgumentRoleV1,
    value: &SemanticAbiValueV1,
) -> Result<(), SemanticMirErrorV1> {
    match role {
        SemanticAbiHiddenArgumentRoleV1::CallerLocation => {
            let SemanticTypeShapeV1::Pointer(pointer) =
                &request.types[value.adjusted_ty().0 as usize].shape
            else {
                return Err(SemanticMirErrorV1::InvalidFunctionAbi);
            };
            let pointee_layout = &request.types[pointer.pointee.0 as usize].layout;
            let Some(pointee_size) = pointee_layout.size_bytes else {
                return Err(SemanticMirErrorV1::InvalidFunctionAbi);
            };
            let SemanticAbiPassModeV1::Direct(attributes) = value.mode else {
                return Err(SemanticMirErrorV1::InvalidFunctionAbi);
            };
            let regular = attributes.regular;
            if pointer.kind != SemanticPointerKindV1::Reference
                || pointer.mutability != SemanticMutabilityV1::Immutable
                || pointer.address_space != 0
                || pointer.pointer_width_bits != 64
                || pointer.metadata != SemanticPointerMetadataV1::None
                || !regular.no_alias()
                || regular.pointer_capture() != Some(SemanticAbiPointerCaptureV1::CapturesReadOnly)
                || !regular.non_null()
                || !regular.read_only()
                || regular.in_register()
                || !regular.no_undef()
                || attributes.extension != SemanticAbiExtensionV1::None
                || attributes.pointee_size_bytes != pointee_size
                || attributes.pointee_alignment_bytes != Some(pointee_layout.alignment_bytes)
            {
                return Err(SemanticMirErrorV1::InvalidFunctionAbi);
            }
        }
    }
    Ok(())
}

fn validate_drop_glue_abi(
    request: &InertSemanticMirRequestV1,
    dropped_type: SemanticTypeIdV1,
    abi: &SemanticFunctionAbiV1,
) -> Result<(), SemanticMirErrorV1> {
    // V1 authenticates rustc's thin `*mut T` drop-glue ABI only. Unsized drop
    // glue must fail closed until its metadata component is modeled exactly.
    let [argument] = abi.fixed_arguments() else {
        return Err(SemanticMirErrorV1::InvalidFunctionAbi);
    };
    let argument_type = request
        .types
        .get(argument.value.adjusted_ty().0 as usize)
        .ok_or(SemanticMirErrorV1::InvalidFunctionAbi)?;
    let SemanticTypeShapeV1::Pointer(pointer) = &argument_type.shape else {
        return Err(SemanticMirErrorV1::InvalidFunctionAbi);
    };
    let dropped_layout = &request
        .types
        .get(dropped_type.0 as usize)
        .ok_or(SemanticMirErrorV1::InvalidFunctionAbi)?
        .layout;
    let Some(pointee_override) = argument.value.pointee_override else {
        return Err(SemanticMirErrorV1::InvalidFunctionAbi);
    };
    let SemanticAbiPointeeKindV1::MutableReference { unpin } = pointee_override.kind else {
        return Err(SemanticMirErrorV1::InvalidFunctionAbi);
    };
    let expected_size = if unpin {
        dropped_layout.rustc_size_bytes
    } else {
        0
    };
    let return_type = request
        .types
        .get(abi.return_value.source_ty.0 as usize)
        .ok_or(SemanticMirErrorV1::InvalidFunctionAbi)?;
    if abi.extern_abi() != SemanticExternAbiV1::Rust
        || abi.c_variadic()
        || !abi.hidden_arguments().is_empty()
        || pointer.pointee != dropped_type
        || pointer.kind != SemanticPointerKindV1::Raw
        || pointer.mutability != SemanticMutabilityV1::Mutable
        || pointer.address_space != 0
        || pointer.pointer_width_bits != 64
        || pointer.metadata != SemanticPointerMetadataV1::None
        || pointee_override.guaranteed_size_bytes != expected_size
        || pointee_override.reliable_alignment_bytes != dropped_layout.alignment_bytes
        || !matches!(argument.value.mode, SemanticAbiPassModeV1::Direct(_))
        || !matches!(return_type.shape, SemanticTypeShapeV1::Unit)
        || !matches!(abi.return_value.mode, SemanticAbiPassModeV1::Ignore)
    {
        return Err(SemanticMirErrorV1::InvalidFunctionAbi);
    }
    Ok(())
}

fn validate_function_abi_contract(
    target: SemanticTargetDataLayoutV1,
    abi: &SemanticFunctionAbiV1,
) -> Result<(), SemanticMirErrorV1> {
    let target_abi_valid = match target.architecture {
        SemanticTargetArchitectureV1::AmdGpuGfx942 => matches!(
            abi.canon_abi,
            SemanticCanonAbiV1::C
                | SemanticCanonAbiV1::Rust
                | SemanticCanonAbiV1::RustCold
                | SemanticCanonAbiV1::RustPreserveNone
                | SemanticCanonAbiV1::Custom
                | SemanticCanonAbiV1::GpuKernel
        ),
    };
    if !target_abi_valid
        || abi.can_unwind
        || (abi.canon_abi == SemanticCanonAbiV1::GpuKernel
            && !matches!(abi.return_value.mode, SemanticAbiPassModeV1::Ignore))
        || (abi.canon_abi == SemanticCanonAbiV1::Custom
            && (!abi.source_input_types().is_empty()
                || !matches!(abi.return_value.mode, SemanticAbiPassModeV1::Ignore)))
    {
        return Err(SemanticMirErrorV1::InvalidFunctionAbi);
    }
    Ok(())
}

fn validate_rust_call_expansion(
    request: &InertSemanticMirRequestV1,
    abi: &SemanticFunctionAbiV1,
) -> Result<(), SemanticMirErrorV1> {
    if abi.extern_abi() != SemanticExternAbiV1::RustCall {
        return Ok(());
    }
    let tuple_type = *abi
        .source_input_types()
        .last()
        .ok_or(SemanticMirErrorV1::InvalidFunctionAbi)?;
    let SemanticTypeShapeV1::Tuple(tuple_fields) = &request.types[tuple_type.0 as usize].shape
    else {
        return Err(SemanticMirErrorV1::InvalidFunctionAbi);
    };
    let expanded = &abi.arguments[abi.fixed_count as usize..abi.adjusted_arguments().len()];
    if expanded.len() != tuple_fields.fields.len()
        || expanded
            .iter()
            .zip(tuple_fields.fields.iter())
            .enumerate()
            .any(|(index, (argument, field_type))| {
                argument.role != SemanticAbiArgumentRoleV1::RustCallTupleField(index as u32)
                    || argument.value.source_ty != *field_type
            })
    {
        return Err(SemanticMirErrorV1::InvalidFunctionAbi);
    }
    Ok(())
}

fn validate_local_roles(
    context: &mut ValidationContextV1<'_>,
    function_id: SemanticFunctionIdV1,
    function: &SemanticFunctionDeclV1,
) -> Result<(), SemanticMirErrorV1> {
    let mut return_count = 0_usize;
    let source_arguments = function.abi.source_input_types();
    let mut arguments = vec![None; source_arguments.len()];
    for (local_index, local) in function.locals.iter().enumerate() {
        let local_id = SemanticLocalIdV1(local_index as u32);
        let location = SemanticMirLocationV1::Local {
            function: function_id,
            local: local_id,
        };
        context.type_reference(local.ty, location)?;
        match local.role {
            SemanticLocalRoleV1::Return => {
                return_count += 1;
                if local.ty != function.abi.source_output_type() {
                    return Err(SemanticMirErrorV1::TypeMismatch {
                        expected: function.abi.source_output_type(),
                        actual: local.ty,
                        location,
                    });
                }
            }
            SemanticLocalRoleV1::Argument(index) => {
                let Some(slot) = arguments.get_mut(index as usize) else {
                    return Err(SemanticMirErrorV1::InvalidLocalRoles {
                        function: function_id,
                    });
                };
                if slot.replace(local.ty).is_some() || local.ty != source_arguments[index as usize]
                {
                    return Err(SemanticMirErrorV1::InvalidLocalRoles {
                        function: function_id,
                    });
                }
            }
            SemanticLocalRoleV1::Temporary => {}
        }
    }
    if return_count != 1 || arguments.iter().any(Option::is_none) {
        return Err(SemanticMirErrorV1::InvalidLocalRoles {
            function: function_id,
        });
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum SemanticAbiValuePositionV1 {
    Return,
    AdjustedArgument,
    VariadicArgument,
}

fn validate_abi_value(
    context: &mut ValidationContextV1<'_>,
    value: &SemanticAbiValueV1,
    canon_abi: SemanticCanonAbiV1,
    extern_abi: SemanticExternAbiV1,
    position: SemanticAbiValuePositionV1,
) -> Result<(), SemanticMirErrorV1> {
    let (ty, layout) = validate_abi_adjustment(context.request, value, position)?;
    let rustic = matches!(
        canon_abi,
        SemanticCanonAbiV1::Rust
            | SemanticCanonAbiV1::RustCold
            | SemanticCanonAbiV1::RustPreserveNone
    );
    let spec_abi_unadjusted = extern_abi == SemanticExternAbiV1::Unadjusted;
    let foreign_classified = !rustic && !spec_abi_unadjusted;
    let is_return = matches!(position, SemanticAbiValuePositionV1::Return);
    if rustic
        && ((layout.size_bytes == Some(0) && !matches!(&value.mode, SemanticAbiPassModeV1::Ignore))
            || matches!(
                &value.mode,
                SemanticAbiPassModeV1::Indirect { on_stack: true, .. }
            ))
    {
        return Err(SemanticMirErrorV1::InvalidFunctionAbi);
    }
    if foreign_classified
        && ty.abi_properties.pass_indirectly_in_non_rustic_abis
        && !matches!(
            value.mode,
            SemanticAbiPassModeV1::Indirect {
                on_stack: false,
                ..
            }
        )
    {
        return Err(SemanticMirErrorV1::InvalidFunctionAbi);
    }
    if ty.abi_properties.has_unsized_foreign_tail
        && matches!(
            value.mode,
            SemanticAbiPassModeV1::Indirect {
                metadata_attributes: Some(_),
                ..
            }
        )
    {
        return Err(SemanticMirErrorV1::InvalidFunctionAbi);
    }
    if spec_abi_unadjusted
        && matches!(
            value.mode,
            SemanticAbiPassModeV1::Cast { .. } | SemanticAbiPassModeV1::Indirect { .. }
        )
    {
        return Err(SemanticMirErrorV1::InvalidFunctionAbi);
    }
    match &value.mode {
        SemanticAbiPassModeV1::Ignore => {
            if layout.size_bytes != Some(0) {
                return Err(SemanticMirErrorV1::InvalidFunctionAbi);
            }
        }
        SemanticAbiPassModeV1::Direct(attributes) => {
            let pointee_override = if value.adjusted.is_some() {
                Some(SemanticAbiPointeeInfoV1 {
                    kind: SemanticAbiPointeeKindV1::Raw,
                    guaranteed_size_bytes: 0,
                    reliable_alignment_bytes: 1,
                })
            } else {
                value.pointee_override
            };
            validate_direct_abi_attributes(
                *attributes,
                ty,
                layout,
                pointee_override,
                foreign_classified,
                is_return,
            )?;
            if !(matches!(
                layout.backend_repr,
                SemanticBackendReprV1::Scalar(_)
                    | SemanticBackendReprV1::SimdVector { .. }
                    | SemanticBackendReprV1::SimdScalableVector { .. }
            ) || (spec_abi_unadjusted
                && matches!(
                    layout.backend_repr,
                    SemanticBackendReprV1::Memory { sized: true }
                )))
            {
                return Err(SemanticMirErrorV1::InvalidFunctionAbi);
            }
        }
        SemanticAbiPassModeV1::Pair { first, second } => {
            let SemanticBackendReprV1::ScalarPair {
                first: first_scalar,
                second: second_scalar,
            } = layout.backend_repr
            else {
                return Err(SemanticMirErrorV1::InvalidFunctionAbi);
            };
            validate_scalar_abi_attributes(
                *first,
                first_scalar,
                value.pointee_override.or(ty.abi_properties.first_pointee),
                foreign_classified,
                is_return,
            )?;
            validate_scalar_abi_attributes(
                *second,
                second_scalar,
                ty.abi_properties.second_pointee,
                foreign_classified,
                is_return,
            )?;
        }
        SemanticAbiPassModeV1::Cast { pad_i32, cast } => {
            if matches!(
                canon_abi,
                SemanticCanonAbiV1::C | SemanticCanonAbiV1::GpuKernel
            ) || !valid_cast_attributes(
                cast.attributes,
                ty.abi_properties.rustc_layout_is_noundef,
            )? {
                return Err(SemanticMirErrorV1::InvalidFunctionAbi);
            }
            validate_abi_uniform(cast.rest)?;
            charge_validation_work(context, cast.prefix.len())?;
            for register in cast.prefix.iter().flatten() {
                validate_abi_register(*register)?;
            }
            if (cast.rest.total_bytes == 0 && cast.prefix.iter().all(Option::is_none))
                || cast.rest_offset_bytes.is_some()
                    && (cast.prefix[0].is_none()
                        || cast.prefix[1..].iter().any(Option::is_some)
                        || cast.rest.consecutive
                        || cast.rest.total_bytes != cast.rest.unit.size_bytes)
            {
                return Err(SemanticMirErrorV1::InvalidFunctionAbi);
            }
            if layout.size_bytes.is_none()
                || !matches!(
                    layout.backend_repr,
                    SemanticBackendReprV1::Memory { sized: true }
                )
            {
                return Err(SemanticMirErrorV1::InvalidFunctionAbi);
            }
            if rustic {
                let Some(size_bytes) = layout.size_bytes else {
                    return Err(SemanticMirErrorV1::InvalidFunctionAbi);
                };
                if size_bytes > 8
                    || *pad_i32
                    || cast.prefix.iter().any(Option::is_some)
                    || cast.rest_offset_bytes.is_some()
                    || cast.rest.unit.kind != SemanticAbiRegisterKindV1::Integer
                    || cast.rest.unit.size_bytes != size_bytes
                    || cast.rest.total_bytes != size_bytes
                    || cast.rest.consecutive
                {
                    return Err(SemanticMirErrorV1::InvalidFunctionAbi);
                }
            }
        }
        SemanticAbiPassModeV1::Indirect {
            attributes,
            metadata_attributes,
            on_stack,
        } => {
            validate_abi_attributes(*attributes)?;
            if let Some(attributes) = metadata_attributes {
                validate_abi_attributes(*attributes)?;
            }
            let property_forced_indirect =
                foreign_classified && ty.abi_properties.pass_indirectly_in_non_rustic_abis;
            if (!matches!(layout.backend_repr, SemanticBackendReprV1::Memory { .. })
                && !property_forced_indirect)
                || (matches!(
                    canon_abi,
                    SemanticCanonAbiV1::C | SemanticCanonAbiV1::GpuKernel
                ) && *on_stack)
                || metadata_attributes.is_some() != layout.size_bytes.is_none()
                || (metadata_attributes.is_some() && *on_stack)
                || !valid_indirect_attributes(*attributes, layout, *on_stack)
                || metadata_attributes
                    .is_some_and(|attributes| attributes != SemanticAbiValueAttributesV1::plain())
            {
                return Err(SemanticMirErrorV1::InvalidFunctionAbi);
            }
            if rustic
                && matches!(
                    layout.backend_repr,
                    SemanticBackendReprV1::Memory { sized: true }
                )
                && layout.size_bytes.is_some_and(|size| size <= 8)
            {
                return Err(SemanticMirErrorV1::InvalidFunctionAbi);
            }
        }
    }
    Ok(())
}

fn validate_abi_adjustment<'a>(
    request: &'a InertSemanticMirRequestV1,
    value: &'a SemanticAbiValueV1,
    position: SemanticAbiValuePositionV1,
) -> Result<(&'a SemanticTypeDeclV1, &'a SemanticTypeLayoutV1), SemanticMirErrorV1> {
    let Some(adjusted) = value.adjusted() else {
        let ty = request
            .types
            .get(value.source_ty.0 as usize)
            .ok_or(SemanticMirErrorV1::InvalidFunctionAbi)?;
        return Ok((ty, &ty.layout));
    };
    if !matches!(position, SemanticAbiValuePositionV1::AdjustedArgument) {
        return Err(SemanticMirErrorV1::InvalidFunctionAbi);
    }
    // V1 models the direct wide-pointer receiver adjustment. Newtype
    // DispatchFromDyn coercions remain unsupported until their field path is
    // represented and authenticated rather than inferred from layout alone.
    let source_ty = request
        .types
        .get(value.source_ty.0 as usize)
        .ok_or(SemanticMirErrorV1::InvalidFunctionAbi)?;
    let SemanticTypeShapeV1::Pointer(source_pointer) = &source_ty.shape else {
        return Err(SemanticMirErrorV1::InvalidFunctionAbi);
    };
    let ty = request
        .types
        .get(adjusted.ty.0 as usize)
        .ok_or(SemanticMirErrorV1::InvalidFunctionAbi)?;
    let expected_layout = SemanticTypeLayoutV1::new_with_backend_repr(
        Some(8),
        8,
        SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
            SemanticBackendPrimitiveV1::pointer(0, 8, 8),
            SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
        )),
        false,
    )?;
    if source_pointer.metadata != SemanticPointerMetadataV1::VTable
        || adjusted.ty != value.source_ty
        || adjusted.layout != expected_layout
        || adjusted.layout_identity == ty.layout_identity
    {
        return Err(SemanticMirErrorV1::InvalidFunctionAbi);
    }
    Ok((ty, &adjusted.layout))
}

fn validate_direct_abi_attributes(
    attributes: SemanticAbiValueAttributesV1,
    ty: &SemanticTypeDeclV1,
    layout: &SemanticTypeLayoutV1,
    pointee_override: Option<SemanticAbiPointeeInfoV1>,
    foreign_classified: bool,
    is_return: bool,
) -> Result<(), SemanticMirErrorV1> {
    validate_abi_attributes(attributes)?;
    match &layout.backend_repr {
        SemanticBackendReprV1::Scalar(scalar) => {
            let pointee = pointee_override
                .or(ty.abi_properties.first_pointee)
                .or_else(|| {
                    matches!(ty.shape, SemanticTypeShapeV1::FunctionPointer { .. }).then_some(
                        SemanticAbiPointeeInfoV1 {
                            kind: SemanticAbiPointeeKindV1::Raw,
                            guaranteed_size_bytes: 0,
                            reliable_alignment_bytes: 1,
                        },
                    )
                });
            validate_scalar_abi_attributes(
                attributes,
                *scalar,
                pointee,
                foreign_classified,
                is_return,
            )
        }
        SemanticBackendReprV1::SimdVector { .. }
        | SemanticBackendReprV1::SimdScalableVector { .. }
        | SemanticBackendReprV1::Memory { sized: true } => {
            if attributes == SemanticAbiValueAttributesV1::plain() {
                Ok(())
            } else {
                Err(SemanticMirErrorV1::InvalidFunctionAbi)
            }
        }
        SemanticBackendReprV1::Memory { sized: false }
        | SemanticBackendReprV1::ScalarPair { .. } => Err(SemanticMirErrorV1::InvalidFunctionAbi),
    }
}

fn validate_scalar_abi_attributes(
    attributes: SemanticAbiValueAttributesV1,
    scalar: SemanticBackendScalarV1,
    pointee: Option<SemanticAbiPointeeInfoV1>,
    foreign_classified: bool,
    is_return: bool,
) -> Result<(), SemanticMirErrorV1> {
    validate_abi_attributes(attributes)?;
    let regular = attributes.regular;
    let pointer_fact = regular.no_alias()
        || regular.pointer_capture().is_some()
        || regular.non_null()
        || regular.read_only()
        || attributes.pointee_size_bytes != 0
        || attributes.pointee_alignment_bytes.is_some();
    let integer_extension = attributes.extension != SemanticAbiExtensionV1::None;
    let initialized = matches!(scalar, SemanticBackendScalarV1::Initialized { .. });
    if attributes.regular.no_undef() != initialized || attributes.regular.in_register() {
        return Err(SemanticMirErrorV1::InvalidFunctionAbi);
    }
    match scalar.primitive() {
        SemanticBackendPrimitiveV1::Pointer { .. } => {
            let requires_non_null = matches!(
                scalar,
                SemanticBackendScalarV1::Initialized { valid_range, .. }
                    if !validity_range_contains(valid_range, 0)
            );
            validate_pointer_abi_attributes(
                attributes,
                pointee,
                initialized,
                requires_non_null,
                is_return,
            )
        }
        SemanticBackendPrimitiveV1::Integer { signed, bits, .. } => {
            let expected_extension = match scalar {
                SemanticBackendScalarV1::Initialized { valid_range, .. }
                    if bits == 8
                        && !signed
                        && valid_range == SemanticScalarValidityRangeV1::new(0, 1) =>
                {
                    SemanticAbiExtensionV1::ZeroExtend
                }
                SemanticBackendScalarV1::Initialized { .. } if bits < 32 && foreign_classified => {
                    if signed {
                        SemanticAbiExtensionV1::SignExtend
                    } else {
                        SemanticAbiExtensionV1::ZeroExtend
                    }
                }
                SemanticBackendScalarV1::Union { .. } if bits < 32 && foreign_classified => {
                    if signed {
                        SemanticAbiExtensionV1::SignExtend
                    } else {
                        SemanticAbiExtensionV1::ZeroExtend
                    }
                }
                SemanticBackendScalarV1::Initialized { .. }
                | SemanticBackendScalarV1::Union { .. } => SemanticAbiExtensionV1::None,
            };
            if pointer_fact || attributes.extension != expected_extension {
                Err(SemanticMirErrorV1::InvalidFunctionAbi)
            } else {
                Ok(())
            }
        }
        SemanticBackendPrimitiveV1::Float { .. } => {
            if pointer_fact || integer_extension {
                Err(SemanticMirErrorV1::InvalidFunctionAbi)
            } else {
                Ok(())
            }
        }
    }
}

fn validate_pointer_abi_attributes(
    attributes: SemanticAbiValueAttributesV1,
    pointee: Option<SemanticAbiPointeeInfoV1>,
    initialized: bool,
    requires_non_null: bool,
    is_return: bool,
) -> Result<(), SemanticMirErrorV1> {
    if attributes.extension != SemanticAbiExtensionV1::None {
        return Err(SemanticMirErrorV1::InvalidFunctionAbi);
    }
    let regular = attributes.regular;
    let Some(pointee) = pointee else {
        return Err(SemanticMirErrorV1::InvalidFunctionAbi);
    };
    let expected_size = if is_return {
        0
    } else {
        pointee.guaranteed_size_bytes
    };
    let expected_alignment =
        (pointee.reliable_alignment_bytes > 1).then_some(pointee.reliable_alignment_bytes);
    let safe_non_null = !matches!(pointee.kind, SemanticAbiPointeeKindV1::Raw);
    let shared_frozen_argument = !is_return
        && matches!(
            pointee.kind,
            SemanticAbiPointeeKindV1::SharedReference { frozen: true }
        );
    let optional_unique_no_alias = !is_return
        && matches!(
            pointee.kind,
            SemanticAbiPointeeKindV1::MutableReference { unpin: true }
                | SemanticAbiPointeeKindV1::Box {
                    unpin: true,
                    global: true
                }
        );
    let no_alias_valid = regular.no_alias() == shared_frozen_argument
        || (optional_unique_no_alias && regular.no_alias());
    if !no_alias_valid
        || regular.pointer_capture()
            != shared_frozen_argument.then_some(SemanticAbiPointerCaptureV1::CapturesReadOnly)
        || regular.read_only() != shared_frozen_argument
        || regular.non_null() != (requires_non_null || safe_non_null)
        || regular.no_undef() != initialized
        || regular.in_register()
        || attributes.pointee_size_bytes != expected_size
        || attributes.pointee_alignment_bytes != expected_alignment
    {
        return Err(SemanticMirErrorV1::InvalidFunctionAbi);
    }
    Ok(())
}

const fn validity_range_contains(range: SemanticScalarValidityRangeV1, value: u128) -> bool {
    if range.start <= range.end {
        range.start <= value && value <= range.end
    } else {
        range.start <= value || value <= range.end
    }
}

fn valid_cast_attributes(
    attributes: SemanticAbiValueAttributesV1,
    rustc_layout_is_noundef: bool,
) -> Result<bool, SemanticMirErrorV1> {
    validate_abi_attributes(attributes)?;
    Ok(attributes.extension == SemanticAbiExtensionV1::None
        && attributes.pointee_size_bytes == 0
        && attributes.pointee_alignment_bytes.is_none()
        && attributes.regular.rustc_bits() & !SemanticAbiRegularAttributesV1::NO_UNDEF == 0
        && attributes.regular.no_undef() == rustc_layout_is_noundef)
}

fn valid_indirect_attributes(
    attributes: SemanticAbiValueAttributesV1,
    layout: &SemanticTypeLayoutV1,
    on_stack: bool,
) -> bool {
    let regular = attributes.regular;
    regular.no_alias()
        && matches!(
            regular.pointer_capture(),
            Some(
                SemanticAbiPointerCaptureV1::CapturesAddress
                    | SemanticAbiPointerCaptureV1::CapturesNone
            )
        )
        && regular.non_null()
        && regular.no_undef()
        && attributes.extension == SemanticAbiExtensionV1::None
        && attributes.pointee_size_bytes == layout.rustc_size_bytes
        && attributes.pointee_alignment_bytes.is_some()
        && (on_stack || attributes.pointee_alignment_bytes == Some(layout.alignment_bytes))
}

fn validate_abi_attributes(
    attributes: SemanticAbiValueAttributesV1,
) -> Result<(), SemanticMirErrorV1> {
    if SemanticAbiRegularAttributesV1::from_rustc_bits(attributes.regular.rustc_bits())?
        != attributes.regular
    {
        return Err(SemanticMirErrorV1::InvalidFunctionAbi);
    }
    if attributes
        .pointee_alignment_bytes
        .is_some_and(|alignment| !valid_rustc_alignment(alignment))
    {
        return Err(SemanticMirErrorV1::InvalidFunctionAbi);
    }
    Ok(())
}

fn validate_abi_uniform(uniform: SemanticAbiUniformV1) -> Result<(), SemanticMirErrorV1> {
    validate_abi_register(uniform.unit)?;
    if uniform.unit.size_bytes == 0
        || (uniform.total_bytes != 0
            && !uniform.total_bytes.is_multiple_of(uniform.unit.size_bytes)
            && uniform.unit.kind != SemanticAbiRegisterKindV1::Integer)
        || uniform
            .total_bytes
            .checked_add(uniform.unit.size_bytes - 1)
            .is_none()
    {
        return Err(SemanticMirErrorV1::InvalidFunctionAbi);
    }
    Ok(())
}

fn validate_abi_register(register: SemanticAbiRegisterV1) -> Result<(), SemanticMirErrorV1> {
    let valid = match register.kind {
        SemanticAbiRegisterKindV1::Integer => (1..=16).contains(&register.size_bytes),
        SemanticAbiRegisterKindV1::Float => matches!(register.size_bytes, 2 | 4 | 8 | 16),
        SemanticAbiRegisterKindV1::Vector => register.size_bytes != 0,
    };
    if valid {
        Ok(())
    } else {
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    }
}

fn validate_statement(
    context: &mut ValidationContextV1<'_>,
    function: &SemanticFunctionDeclV1,
    location: SemanticMirLocationV1,
    statement: &SemanticStatementKindV1,
) -> Result<(), SemanticMirErrorV1> {
    match statement {
        SemanticStatementKindV1::Assign(assignment) => {
            validate_place(context, function, location, &assignment.destination)?;
            validate_rvalue(context, function, location, &assignment.value)?;
            require_type(
                assignment.destination.ty,
                assignment.value.result_type,
                location,
            )?;
        }
        SemanticStatementKindV1::Store(store) => {
            validate_place(context, function, location, &store.destination)?;
            validate_operand(context, function, location, &store.value)?;
            require_type(store.destination.ty, store.value.ty(), location)?;
            validate_memory_access(
                context.request,
                store.destination.ty,
                store.volatility,
                store.atomic,
                SemanticAtomicOperationV1::Store,
                location,
            )?;
        }
        SemanticStatementKindV1::AtomicRmw(operation) => {
            validate_place(context, function, location, &operation.destination)?;
            validate_place(context, function, location, &operation.address)?;
            validate_operand(context, function, location, &operation.value)?;
            require_type(operation.destination.ty, operation.value.ty(), location)?;
            require_type(operation.address.ty, operation.value.ty(), location)?;
            if !atomic_rmw_type_allowed(context.request, operation.value.ty(), operation.operation)
            {
                return invalid_type_operation(SemanticTypeOperationV1::Atomic, location);
            }
            validate_atomic_ordering(
                SemanticAtomicOperationV1::ReadModifyWrite,
                operation.access.ordering,
                location,
            )?;
        }
        SemanticStatementKindV1::AtomicCompareExchange(operation) => {
            validate_place(context, function, location, &operation.destination)?;
            validate_place(context, function, location, &operation.address)?;
            validate_operand(context, function, location, &operation.expected)?;
            validate_operand(context, function, location, &operation.replacement)?;
            require_type(
                operation.expected.ty(),
                operation.replacement.ty(),
                location,
            )?;
            require_type(operation.address.ty, operation.expected.ty(), location)?;
            require_type(operation.destination.ty, operation.expected.ty(), location)?;
            if !is_atomic_scalar_type(context.request, operation.expected.ty()) {
                return invalid_type_operation(SemanticTypeOperationV1::Atomic, location);
            }
            validate_atomic_ordering(
                SemanticAtomicOperationV1::ReadModifyWrite,
                operation.success.ordering,
                location,
            )?;
            validate_atomic_ordering(
                SemanticAtomicOperationV1::CompareExchangeFailure,
                operation.failure_ordering,
                location,
            )?;
            if !compare_exchange_failure_allowed(
                operation.success.ordering,
                operation.failure_ordering,
            ) {
                return Err(SemanticMirErrorV1::InvalidAtomicOrdering {
                    operation: SemanticAtomicOperationV1::CompareExchangeFailure,
                    ordering: operation.failure_ordering,
                    location,
                });
            }
        }
        SemanticStatementKindV1::SetDiscriminant {
            place,
            variant_index,
        } => {
            validate_place(context, function, location, place)?;
            let SemanticTypeShapeV1::Enum { variants, .. } = type_shape(context, place.ty) else {
                return invalid_type_operation(SemanticTypeOperationV1::SetDiscriminant, location);
            };
            if *variant_index as usize >= variants.len() {
                return invalid_type_operation(SemanticTypeOperationV1::SetDiscriminant, location);
            }
        }
        SemanticStatementKindV1::Deinitialize(place) => {
            validate_place(context, function, location, place)?;
        }
        SemanticStatementKindV1::StorageLive(local)
        | SemanticStatementKindV1::StorageDead(local) => {
            context.reference(
                SemanticMirReferenceV1::Local,
                local.0,
                function.locals.len(),
                location,
            )?;
        }
        SemanticStatementKindV1::Nop => {}
    }
    Ok(())
}

fn validate_rvalue(
    context: &mut ValidationContextV1<'_>,
    function: &SemanticFunctionDeclV1,
    location: SemanticMirLocationV1,
    rvalue: &SemanticRvalueV1,
) -> Result<(), SemanticMirErrorV1> {
    context.type_reference(rvalue.result_type, location)?;
    match &rvalue.kind {
        SemanticRvalueKindV1::Use(operand) => {
            validate_operand(context, function, location, operand)?;
            require_type(rvalue.result_type, operand.ty(), location)?;
        }
        SemanticRvalueKindV1::Unary { operation, operand } => {
            validate_operand(context, function, location, operand)?;
            match operation {
                SemanticUnaryOpV1::Not
                    if (is_integer_type(context.request, operand.ty())
                        || is_bool_type(context.request, operand.ty()))
                        && rvalue.result_type == operand.ty() => {}
                SemanticUnaryOpV1::Negate
                    if is_signed_or_float_type(context.request, operand.ty())
                        && rvalue.result_type == operand.ty() => {}
                SemanticUnaryOpV1::PointerMetadata => {
                    let SemanticTypeShapeV1::Pointer(pointer) = type_shape(context, operand.ty())
                    else {
                        return invalid_type_operation(SemanticTypeOperationV1::Unary, location);
                    };
                    let valid_result = match pointer.metadata {
                        SemanticPointerMetadataV1::SliceLength => {
                            is_unsigned_integer_with_bits(context.request, rvalue.result_type, 64)
                        }
                        SemanticPointerMetadataV1::VTable => {
                            is_gfx942_vtable_metadata_result(context.request, rvalue.result_type)
                        }
                        SemanticPointerMetadataV1::None => false,
                    };
                    if !valid_result {
                        return invalid_type_operation(SemanticTypeOperationV1::Unary, location);
                    }
                }
                _ => return invalid_type_operation(SemanticTypeOperationV1::Unary, location),
            }
        }
        SemanticRvalueKindV1::Binary {
            operation,
            left,
            right,
        } => {
            validate_operand(context, function, location, left)?;
            validate_operand(context, function, location, right)?;
            let valid = match operation {
                SemanticBinaryOpV1::Add
                | SemanticBinaryOpV1::Subtract
                | SemanticBinaryOpV1::Multiply
                | SemanticBinaryOpV1::Divide
                | SemanticBinaryOpV1::Remainder => {
                    left.ty() == right.ty()
                        && left.ty() == rvalue.result_type
                        && is_numeric_type(context.request, left.ty())
                }
                SemanticBinaryOpV1::BitXor
                | SemanticBinaryOpV1::BitAnd
                | SemanticBinaryOpV1::BitOr => {
                    left.ty() == right.ty()
                        && left.ty() == rvalue.result_type
                        && (is_integer_type(context.request, left.ty())
                            || is_bool_type(context.request, left.ty()))
                }
                SemanticBinaryOpV1::ShiftLeft | SemanticBinaryOpV1::ShiftRight => {
                    is_integer_type(context.request, left.ty())
                        && is_integer_type(context.request, right.ty())
                        && left.ty() == rvalue.result_type
                }
                SemanticBinaryOpV1::Equal
                | SemanticBinaryOpV1::LessThan
                | SemanticBinaryOpV1::LessOrEqual
                | SemanticBinaryOpV1::NotEqual
                | SemanticBinaryOpV1::GreaterOrEqual
                | SemanticBinaryOpV1::GreaterThan => {
                    left.ty() == right.ty()
                        && is_comparable_type(context.request, left.ty())
                        && is_bool_type(context.request, rvalue.result_type)
                }
                SemanticBinaryOpV1::Offset => {
                    matches!(
                        type_shape(context, left.ty()),
                        SemanticTypeShapeV1::Pointer(SemanticPointerTypeV1 {
                            kind: SemanticPointerKindV1::Raw,
                            metadata: SemanticPointerMetadataV1::None,
                            address_space: 0..=6,
                            ..
                        })
                    ) && is_integer_type(context.request, right.ty())
                        && left.ty() == rvalue.result_type
                }
            };
            if !valid {
                return invalid_type_operation(SemanticTypeOperationV1::Binary, location);
            }
        }
        SemanticRvalueKindV1::Cast { kind, operand } => {
            validate_operand(context, function, location, operand)?;
            let input = operand.ty();
            let output = rvalue.result_type;
            let valid = match kind {
                SemanticCastKindV1::Integer => {
                    is_scalar_type(context.request, input)
                        && is_integer_type(context.request, output)
                }
                SemanticCastKindV1::Float => {
                    is_numeric_type(context.request, input)
                        && is_float_type(context.request, output)
                }
                SemanticCastKindV1::Pointer => pointer_type(context.request, input)
                    .zip(pointer_type(context.request, output))
                    .is_some_and(|(input, output)| {
                        output.kind == SemanticPointerKindV1::Raw
                            && input.address_space <= 6
                            && input.address_space == output.address_space
                            && input.pointer_width_bits == output.pointer_width_bits
                            && input.metadata == output.metadata
                            && !(input.mutability == SemanticMutabilityV1::Immutable
                                && output.mutability == SemanticMutabilityV1::Mutable)
                            && (input.metadata == SemanticPointerMetadataV1::None
                                || input.pointee == output.pointee)
                    }),
                SemanticCastKindV1::PointerExposeProvenance => pointer_type(context.request, input)
                    .is_some_and(|input| {
                        input.kind == SemanticPointerKindV1::Raw
                            && input.address_space <= 6
                            && input.metadata == SemanticPointerMetadataV1::None
                            && is_unsigned_integer_with_bits(
                                context.request,
                                output,
                                input.pointer_width_bits,
                            )
                    }),
                SemanticCastKindV1::PointerWithExposedProvenance => {
                    pointer_type(context.request, output).is_some_and(|output| {
                        output.kind == SemanticPointerKindV1::Raw
                            && output.address_space <= 6
                            && output.metadata == SemanticPointerMetadataV1::None
                            && is_unsigned_integer_with_bits(
                                context.request,
                                input,
                                output.pointer_width_bits,
                            )
                    })
                }
                SemanticCastKindV1::Transmute => {
                    input == output
                        || type_size(context.request, input).is_some()
                            && type_size(context.request, input)
                                == type_size(context.request, output)
                            && plain_bit_scalar(context.request, input)
                            && plain_bit_scalar(context.request, output)
                }
            };
            if !valid {
                return invalid_type_operation(SemanticTypeOperationV1::Cast, location);
            }
        }
        SemanticRvalueKindV1::Borrow { kind, place } => {
            validate_place(context, function, location, place)?;
            let SemanticTypeShapeV1::Pointer(pointer) = type_shape(context, rvalue.result_type)
            else {
                return invalid_type_operation(SemanticTypeOperationV1::Borrow, location);
            };
            let mutability_valid = match kind {
                SemanticBorrowKindV1::Shared => {
                    pointer.mutability == SemanticMutabilityV1::Immutable
                }
                SemanticBorrowKindV1::Mutable => {
                    pointer.mutability == SemanticMutabilityV1::Mutable
                }
                SemanticBorrowKindV1::Fake => false,
            };
            if pointer.kind != SemanticPointerKindV1::Reference
                || pointer.address_space != 0
                || pointer.pointer_width_bits != 64
                || pointer.metadata != SemanticPointerMetadataV1::None
                || pointer.pointee != place.ty
                || !mutability_valid
            {
                return invalid_type_operation(SemanticTypeOperationV1::Borrow, location);
            }
        }
        SemanticRvalueKindV1::AddressOf { mutability, place } => {
            validate_place(context, function, location, place)?;
            let SemanticTypeShapeV1::Pointer(pointer) = type_shape(context, rvalue.result_type)
            else {
                return invalid_type_operation(SemanticTypeOperationV1::Borrow, location);
            };
            if pointer.kind != SemanticPointerKindV1::Raw
                || pointer.address_space != 0
                || pointer.pointer_width_bits != 64
                || pointer.metadata != SemanticPointerMetadataV1::None
                || pointer.pointee != place.ty
                || pointer.mutability != *mutability
            {
                return invalid_type_operation(SemanticTypeOperationV1::Borrow, location);
            }
        }
        SemanticRvalueKindV1::Length(place) => {
            validate_place(context, function, location, place)?;
            if !matches!(
                type_shape(context, place.ty),
                SemanticTypeShapeV1::Array { .. }
            ) || !is_unsigned_integer_type(context.request, rvalue.result_type)
            {
                return invalid_type_operation(SemanticTypeOperationV1::Length, location);
            }
        }
        SemanticRvalueKindV1::Discriminant(place) => {
            validate_place(context, function, location, place)?;
            let SemanticTypeShapeV1::Enum { discriminant, .. } = type_shape(context, place.ty)
            else {
                return invalid_type_operation(SemanticTypeOperationV1::Discriminant, location);
            };
            require_type(*discriminant, rvalue.result_type, location)?;
        }
        SemanticRvalueKindV1::Aggregate(aggregate) => {
            for operand in &aggregate.operands {
                validate_operand(context, function, location, operand)?;
            }
            validate_aggregate_rvalue(context, location, rvalue.result_type, aggregate)?;
        }
        SemanticRvalueKindV1::Load(load) => {
            validate_place(context, function, location, &load.source)?;
            require_type(load.source.ty, rvalue.result_type, location)?;
            validate_memory_access(
                context.request,
                rvalue.result_type,
                load.volatility,
                load.atomic,
                SemanticAtomicOperationV1::Load,
                location,
            )?;
        }
    }
    Ok(())
}

fn validate_aggregate_rvalue(
    context: &ValidationContextV1<'_>,
    location: SemanticMirLocationV1,
    result_type: SemanticTypeIdV1,
    aggregate: &SemanticAggregateRvalueV1,
) -> Result<(), SemanticMirErrorV1> {
    let operands_match = |expected: &[SemanticTypeIdV1]| {
        aggregate.operands.len() == expected.len()
            && aggregate
                .operands
                .iter()
                .zip(expected)
                .all(|(operand, expected)| operand.ty() == *expected)
    };
    let valid = match (&aggregate.kind, type_shape(context, result_type)) {
        (SemanticAggregateKindV1::Array, SemanticTypeShapeV1::Array { element, length }) => {
            aggregate.operands.len() as u64 == *length
                && aggregate
                    .operands
                    .iter()
                    .all(|operand| operand.ty() == *element)
        }
        (SemanticAggregateKindV1::Tuple, SemanticTypeShapeV1::Tuple(fields))
        | (SemanticAggregateKindV1::Aggregate, SemanticTypeShapeV1::Aggregate(fields)) => {
            operands_match(&fields.fields)
        }
        (
            SemanticAggregateKindV1::EnumVariant(variant),
            SemanticTypeShapeV1::Enum { variants, .. },
        ) => variants
            .get(*variant as usize)
            .is_some_and(|variant| operands_match(&variant.fields.fields)),
        _ => false,
    };
    if !valid {
        return invalid_type_operation(SemanticTypeOperationV1::Aggregate, location);
    }
    Ok(())
}

fn scalar_type(
    request: &InertSemanticMirRequestV1,
    ty: SemanticTypeIdV1,
) -> Option<SemanticScalarTypeV1> {
    match &request.types[ty.0 as usize].shape {
        SemanticTypeShapeV1::Scalar(scalar) => Some(*scalar),
        SemanticTypeShapeV1::ValidityScalar(validity) => Some(validity.scalar),
        _ => None,
    }
}

fn scalar_raw_value_fits(scalar: SemanticScalarTypeV1, value: u128) -> bool {
    match scalar {
        SemanticScalarTypeV1::Bool => value <= 1,
        SemanticScalarTypeV1::Char => value <= 0x10_ffff && !(0xd800..=0xdfff).contains(&value),
        SemanticScalarTypeV1::Integer { bits: 128, .. } => true,
        SemanticScalarTypeV1::Integer { bits, .. } if matches!(bits, 8 | 16 | 32 | 64) => {
            value < (1_u128 << u32::from(bits))
        }
        SemanticScalarTypeV1::Integer { .. } => false,
        SemanticScalarTypeV1::Float { .. } => false,
    }
}

fn is_scalar_type(request: &InertSemanticMirRequestV1, ty: SemanticTypeIdV1) -> bool {
    scalar_type(request, ty).is_some()
}

fn is_integer_type(request: &InertSemanticMirRequestV1, ty: SemanticTypeIdV1) -> bool {
    matches!(
        scalar_type(request, ty),
        Some(SemanticScalarTypeV1::Integer { .. })
    )
}

fn is_unsigned_integer_type(request: &InertSemanticMirRequestV1, ty: SemanticTypeIdV1) -> bool {
    matches!(
        scalar_type(request, ty),
        Some(SemanticScalarTypeV1::Integer { signed: false, .. })
    )
}

fn is_unsigned_integer_with_bits(
    request: &InertSemanticMirRequestV1,
    ty: SemanticTypeIdV1,
    expected_bits: u16,
) -> bool {
    matches!(
        scalar_type(request, ty),
        Some(SemanticScalarTypeV1::Integer {
            signed: false,
            bits,
        }) if bits == expected_bits
    )
}

fn is_gfx942_vtable_metadata_result(
    request: &InertSemanticMirRequestV1,
    ty: SemanticTypeIdV1,
) -> bool {
    matches!(
        &request.types[ty.0 as usize].shape,
        SemanticTypeShapeV1::Pointer(pointer)
            if pointer.address_space == 0
                && pointer.pointer_width_bits == 64
                && pointer.metadata == SemanticPointerMetadataV1::None
    )
}

fn is_float_type(request: &InertSemanticMirRequestV1, ty: SemanticTypeIdV1) -> bool {
    matches!(
        scalar_type(request, ty),
        Some(SemanticScalarTypeV1::Float { .. })
    )
}

fn is_signed_or_float_type(request: &InertSemanticMirRequestV1, ty: SemanticTypeIdV1) -> bool {
    matches!(
        scalar_type(request, ty),
        Some(SemanticScalarTypeV1::Integer { signed: true, .. })
            | Some(SemanticScalarTypeV1::Float { .. })
    )
}

fn is_numeric_type(request: &InertSemanticMirRequestV1, ty: SemanticTypeIdV1) -> bool {
    matches!(
        scalar_type(request, ty),
        Some(SemanticScalarTypeV1::Integer { .. }) | Some(SemanticScalarTypeV1::Float { .. })
    )
}

fn plain_bit_scalar(request: &InertSemanticMirRequestV1, ty: SemanticTypeIdV1) -> bool {
    let ty = &request.types[ty.0 as usize];
    matches!(
        ty.shape,
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer { .. })
            | SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float { .. })
    ) && matches!(
        ty.layout.backend_repr,
        SemanticBackendReprV1::Scalar(SemanticBackendScalarV1::Initialized {
            valid_range: SemanticScalarValidityRangeV1 { start: 0, end },
            ..
        }) if ty
            .layout
            .size_bytes
            .is_some_and(|size| end == unsigned_max((size * 8) as u16))
    )
}

fn is_pointer_type(request: &InertSemanticMirRequestV1, ty: SemanticTypeIdV1) -> bool {
    pointer_type(request, ty).is_some()
}

fn pointer_type(
    request: &InertSemanticMirRequestV1,
    ty: SemanticTypeIdV1,
) -> Option<&SemanticPointerTypeV1> {
    match &request.types[ty.0 as usize].shape {
        SemanticTypeShapeV1::Pointer(pointer) => Some(pointer),
        _ => None,
    }
}

fn is_comparable_type(request: &InertSemanticMirRequestV1, ty: SemanticTypeIdV1) -> bool {
    is_scalar_type(request, ty) || is_pointer_type(request, ty)
}

fn is_atomic_scalar_type(request: &InertSemanticMirRequestV1, ty: SemanticTypeIdV1) -> bool {
    matches!(
        scalar_type(request, ty),
        Some(SemanticScalarTypeV1::Bool) | Some(SemanticScalarTypeV1::Integer { .. })
    ) || is_pointer_type(request, ty)
}

fn atomic_rmw_type_allowed(
    request: &InertSemanticMirRequestV1,
    ty: SemanticTypeIdV1,
    operation: SemanticAtomicRmwOpV1,
) -> bool {
    match operation {
        SemanticAtomicRmwOpV1::Exchange => is_atomic_scalar_type(request, ty),
        SemanticAtomicRmwOpV1::Add
        | SemanticAtomicRmwOpV1::Subtract
        | SemanticAtomicRmwOpV1::BitAnd
        | SemanticAtomicRmwOpV1::BitNand
        | SemanticAtomicRmwOpV1::BitOr
        | SemanticAtomicRmwOpV1::BitXor => is_integer_type(request, ty),
        SemanticAtomicRmwOpV1::SignedMaximum | SemanticAtomicRmwOpV1::SignedMinimum => matches!(
            scalar_type(request, ty),
            Some(SemanticScalarTypeV1::Integer { signed: true, .. })
        ),
        SemanticAtomicRmwOpV1::UnsignedMaximum | SemanticAtomicRmwOpV1::UnsignedMinimum => {
            matches!(
                scalar_type(request, ty),
                Some(SemanticScalarTypeV1::Integer { signed: false, .. })
            )
        }
    }
}

fn type_size(request: &InertSemanticMirRequestV1, ty: SemanticTypeIdV1) -> Option<u64> {
    request.types[ty.0 as usize].layout.size_bytes
}

fn validate_memory_access(
    request: &InertSemanticMirRequestV1,
    ty: SemanticTypeIdV1,
    volatility: SemanticVolatilityV1,
    atomic: Option<SemanticAtomicAccessV1>,
    operation: SemanticAtomicOperationV1,
    location: SemanticMirLocationV1,
) -> Result<(), SemanticMirErrorV1> {
    if volatility == SemanticVolatilityV1::Volatile && atomic.is_some() {
        return Err(SemanticMirErrorV1::InvalidAtomicCombination { location });
    }
    if let Some(access) = atomic {
        if !is_atomic_scalar_type(request, ty) {
            return invalid_type_operation(SemanticTypeOperationV1::Atomic, location);
        }
        validate_atomic_ordering(operation, access.ordering, location)?;
    }
    Ok(())
}

fn validate_atomic_ordering(
    operation: SemanticAtomicOperationV1,
    ordering: SemanticAtomicOrderingV1,
    location: SemanticMirLocationV1,
) -> Result<(), SemanticMirErrorV1> {
    let valid = match operation {
        SemanticAtomicOperationV1::Load => matches!(
            ordering,
            SemanticAtomicOrderingV1::Relaxed
                | SemanticAtomicOrderingV1::Acquire
                | SemanticAtomicOrderingV1::SequentiallyConsistent
        ),
        SemanticAtomicOperationV1::Store => matches!(
            ordering,
            SemanticAtomicOrderingV1::Relaxed
                | SemanticAtomicOrderingV1::Release
                | SemanticAtomicOrderingV1::SequentiallyConsistent
        ),
        SemanticAtomicOperationV1::ReadModifyWrite => true,
        SemanticAtomicOperationV1::CompareExchangeFailure => matches!(
            ordering,
            SemanticAtomicOrderingV1::Relaxed
                | SemanticAtomicOrderingV1::Acquire
                | SemanticAtomicOrderingV1::SequentiallyConsistent
        ),
    };
    if !valid {
        return Err(SemanticMirErrorV1::InvalidAtomicOrdering {
            operation,
            ordering,
            location,
        });
    }
    Ok(())
}

const fn compare_exchange_failure_allowed(
    success: SemanticAtomicOrderingV1,
    failure: SemanticAtomicOrderingV1,
) -> bool {
    match success {
        SemanticAtomicOrderingV1::Relaxed | SemanticAtomicOrderingV1::Release => {
            matches!(failure, SemanticAtomicOrderingV1::Relaxed)
        }
        SemanticAtomicOrderingV1::Acquire | SemanticAtomicOrderingV1::AcquireRelease => matches!(
            failure,
            SemanticAtomicOrderingV1::Relaxed | SemanticAtomicOrderingV1::Acquire
        ),
        SemanticAtomicOrderingV1::SequentiallyConsistent => matches!(
            failure,
            SemanticAtomicOrderingV1::Relaxed
                | SemanticAtomicOrderingV1::Acquire
                | SemanticAtomicOrderingV1::SequentiallyConsistent
        ),
    }
}

fn validate_place(
    context: &mut ValidationContextV1<'_>,
    function: &SemanticFunctionDeclV1,
    location: SemanticMirLocationV1,
    place: &SemanticPlaceV1,
) -> Result<(), SemanticMirErrorV1> {
    context.reference(
        SemanticMirReferenceV1::Local,
        place.local.0,
        function.locals.len(),
        location,
    )?;
    context.type_reference(place.ty, location)?;
    context.totals.charge(
        SemanticMirResourceV1::Projections,
        place.projections.len(),
        context.limits,
    )?;
    let mut current_type = function.locals[place.local.0 as usize].ty;
    let mut downcast_variant = None;
    for projection in &place.projections {
        context.one()?;
        context.type_reference(projection.result_type, location)?;
        if downcast_variant.is_some()
            && !matches!(projection.kind, SemanticProjectionKindV1::Field(_))
        {
            return invalid_type_operation(SemanticTypeOperationV1::Projection, location);
        }
        let expected = match projection.kind {
            SemanticProjectionKindV1::Dereference => {
                let SemanticTypeShapeV1::Pointer(pointer) = type_shape(context, current_type)
                else {
                    return invalid_type_operation(SemanticTypeOperationV1::Projection, location);
                };
                pointer.pointee
            }
            SemanticProjectionKindV1::Field(field) => {
                let field = field as usize;
                match type_shape(context, current_type) {
                    SemanticTypeShapeV1::Tuple(fields) | SemanticTypeShapeV1::Aggregate(fields) => {
                        fields.fields.get(field).copied().ok_or(
                            SemanticMirErrorV1::InvalidTypeOperation {
                                operation: SemanticTypeOperationV1::Projection,
                                location,
                            },
                        )?
                    }
                    SemanticTypeShapeV1::Enum { variants, .. } => {
                        let variant = downcast_variant.take().ok_or(
                            SemanticMirErrorV1::InvalidTypeOperation {
                                operation: SemanticTypeOperationV1::Projection,
                                location,
                            },
                        )? as usize;
                        variants
                            .get(variant)
                            .and_then(|variant| variant.fields.fields.get(field))
                            .copied()
                            .ok_or(SemanticMirErrorV1::InvalidTypeOperation {
                                operation: SemanticTypeOperationV1::Projection,
                                location,
                            })?
                    }
                    _ => {
                        return invalid_type_operation(
                            SemanticTypeOperationV1::Projection,
                            location,
                        );
                    }
                }
            }
            SemanticProjectionKindV1::Index(local) => {
                context.reference(
                    SemanticMirReferenceV1::Local,
                    local.0,
                    function.locals.len(),
                    location,
                )?;
                let index_type = function.locals[local.0 as usize].ty;
                if !is_unsigned_integer_type(context.request, index_type) {
                    return invalid_type_operation(SemanticTypeOperationV1::Projection, location);
                }
                let SemanticTypeShapeV1::Array { element, .. } = type_shape(context, current_type)
                else {
                    return invalid_type_operation(SemanticTypeOperationV1::Projection, location);
                };
                *element
            }
            SemanticProjectionKindV1::ConstantIndex { minimum_length, .. } => {
                let SemanticTypeShapeV1::Array { element, length } =
                    type_shape(context, current_type)
                else {
                    return invalid_type_operation(SemanticTypeOperationV1::Projection, location);
                };
                if minimum_length > *length {
                    return invalid_type_operation(SemanticTypeOperationV1::Projection, location);
                }
                *element
            }
            SemanticProjectionKindV1::Subslice { from, to, from_end } => {
                let SemanticTypeShapeV1::Array { element, length } =
                    type_shape(context, current_type)
                else {
                    return invalid_type_operation(SemanticTypeOperationV1::Projection, location);
                };
                let result_length = if from_end {
                    from.checked_add(to)
                        .filter(|removed| *removed <= *length)
                        .map(|removed| *length - removed)
                } else {
                    (from <= to && to <= *length).then_some(to - from)
                }
                .ok_or(SemanticMirErrorV1::InvalidTypeOperation {
                    operation: SemanticTypeOperationV1::Projection,
                    location,
                })?;
                match type_shape(context, projection.result_type) {
                    SemanticTypeShapeV1::Array {
                        element: result_element,
                        length: actual_length,
                    } if result_element == element && *actual_length == result_length => {}
                    _ => {
                        return invalid_type_operation(
                            SemanticTypeOperationV1::Projection,
                            location,
                        );
                    }
                }
                projection.result_type
            }
            SemanticProjectionKindV1::Downcast(variant) => {
                let SemanticTypeShapeV1::Enum { variants, .. } = type_shape(context, current_type)
                else {
                    return invalid_type_operation(SemanticTypeOperationV1::Projection, location);
                };
                if variant as usize >= variants.len() {
                    return invalid_type_operation(SemanticTypeOperationV1::Projection, location);
                }
                downcast_variant = Some(variant);
                current_type
            }
            SemanticProjectionKindV1::OpaqueCast | SemanticProjectionKindV1::Subtype => {
                if current_type != projection.result_type {
                    return invalid_type_operation(SemanticTypeOperationV1::Projection, location);
                }
                projection.result_type
            }
        };
        require_type(expected, projection.result_type, location)?;
        current_type = projection.result_type;
    }
    if downcast_variant.is_some() {
        return invalid_type_operation(SemanticTypeOperationV1::Projection, location);
    }
    require_type(current_type, place.ty, location)
}

fn invalid_type_operation<T>(
    operation: SemanticTypeOperationV1,
    location: SemanticMirLocationV1,
) -> Result<T, SemanticMirErrorV1> {
    Err(SemanticMirErrorV1::InvalidTypeOperation {
        operation,
        location,
    })
}

fn type_shape<'a>(
    context: &'a ValidationContextV1<'_>,
    ty: SemanticTypeIdV1,
) -> &'a SemanticTypeShapeV1 {
    &context.request.types[ty.0 as usize].shape
}

fn validate_operand(
    context: &mut ValidationContextV1<'_>,
    function: &SemanticFunctionDeclV1,
    location: SemanticMirLocationV1,
    operand: &SemanticOperandV1,
) -> Result<(), SemanticMirErrorV1> {
    context
        .totals
        .charge(SemanticMirResourceV1::Operands, 1, context.limits)?;
    match operand {
        SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => {
            validate_place(context, function, location, place)
        }
        SemanticOperandV1::Constant(constant) => validate_constant(context, location, constant),
    }
}

fn validate_constant(
    context: &mut ValidationContextV1<'_>,
    location: SemanticMirLocationV1,
    constant: &SemanticConstantV1,
) -> Result<(), SemanticMirErrorV1> {
    context.type_reference(constant.ty, location)?;
    let ty = &context.request.types[constant.ty.0 as usize];
    match &constant.value {
        SemanticConstantValueV1::ZeroSized => {
            if ty.layout.size_bytes != Some(0) {
                return invalid_type_operation(SemanticTypeOperationV1::Constant, location);
            }
        }
        SemanticConstantValueV1::Scalar(value) => {
            if ty.layout.size_bytes != Some(u64::from(value.size_bytes))
                || !scalar_value_is_valid(&ty.shape, *value)
            {
                return invalid_type_operation(SemanticTypeOperationV1::Constant, location);
            }
        }
        SemanticConstantValueV1::Bytes(bytes) => {
            context.totals.charge(
                SemanticMirResourceV1::ConstantBytes,
                bytes.0.len(),
                context.limits,
            )?;
            if ty.layout.size_bytes != Some(bytes.0.len() as u64)
                || matches!(
                    ty.shape,
                    SemanticTypeShapeV1::Scalar(_)
                        | SemanticTypeShapeV1::ValidityScalar(_)
                        | SemanticTypeShapeV1::Pointer(_)
                        | SemanticTypeShapeV1::FunctionPointer { .. }
                )
            {
                return invalid_type_operation(SemanticTypeOperationV1::Constant, location);
            }
        }
        SemanticConstantValueV1::Pointer(pointer) => {
            let SemanticTypeShapeV1::Pointer(pointer_ty) = &ty.shape else {
                return invalid_type_operation(SemanticTypeOperationV1::Constant, location);
            };
            match (pointer_ty.metadata, pointer.metadata) {
                (SemanticPointerMetadataV1::None, SemanticPointerValueMetadataV1::None) => {}
                (
                    SemanticPointerMetadataV1::SliceLength,
                    SemanticPointerValueMetadataV1::SliceLength(_),
                ) => {}
                (
                    SemanticPointerMetadataV1::VTable,
                    SemanticPointerValueMetadataV1::VTable(vtable),
                ) => {
                    context.vtable_reference(vtable, location)?;
                    if context.request.vtables[vtable.0 as usize].dyn_type != pointer_ty.pointee {
                        return invalid_type_operation(SemanticTypeOperationV1::Constant, location);
                    }
                }
                _ => {
                    return invalid_type_operation(SemanticTypeOperationV1::Constant, location);
                }
            }
            match pointer.provenance {
                SemanticPointerProvenanceV1::Allocation(allocation) => {
                    context.allocation_reference(allocation, location)?;
                    let allocation = &context.request.allocations[allocation.0 as usize];
                    if pointer_ty.address_space != allocation.address_space
                        || !pointer_target_range_is_valid(
                            context.request,
                            pointer_ty,
                            pointer.metadata,
                            pointer.byte_offset,
                            allocation.bytes.len() as u64,
                            allocation.mutable,
                        )
                    {
                        return Err(SemanticMirErrorV1::InvalidAllocation);
                    }
                }
                SemanticPointerProvenanceV1::Callable(callable) => {
                    context.callable_reference(callable, location)?;
                    let _ = addressable_callable_abi(context.request, callable)?;
                    if pointer.byte_offset != 0
                        || pointer_ty.kind != SemanticPointerKindV1::Raw
                        || pointer_ty.address_space != 0
                        || pointer_ty.metadata != SemanticPointerMetadataV1::None
                    {
                        return invalid_type_operation(SemanticTypeOperationV1::Constant, location);
                    }
                }
                SemanticPointerProvenanceV1::Static(static_id) => {
                    context.static_reference(static_id, location)?;
                    let static_decl = &context.request.statics[static_id.0 as usize];
                    if pointer_ty.address_space != static_decl.address_space {
                        return Err(SemanticMirErrorV1::InvalidStatic);
                    }
                    let static_size = context.request.types[static_decl.ty.0 as usize]
                        .layout
                        .size_bytes
                        .ok_or(SemanticMirErrorV1::InvalidStatic)?;
                    if !pointer_target_range_is_valid(
                        context.request,
                        pointer_ty,
                        pointer.metadata,
                        pointer.byte_offset,
                        static_size,
                        static_decl.mutable,
                    ) {
                        return Err(SemanticMirErrorV1::InvalidStatic);
                    }
                }
                SemanticPointerProvenanceV1::ExposedAddress => {
                    if pointer_ty.kind == SemanticPointerKindV1::Reference
                        || pointer_ty.address_space > 6
                        || pointer_ty.metadata != SemanticPointerMetadataV1::None
                        || u128::from(pointer.byte_offset)
                            > unsigned_max(pointer_ty.pointer_width_bits)
                    {
                        return invalid_type_operation(SemanticTypeOperationV1::Constant, location);
                    }
                }
            }
        }
        SemanticConstantValueV1::Callable(callable) => {
            context.callable_reference(*callable, location)?;
            let SemanticTypeShapeV1::FunctionPointer {
                extern_abi,
                c_variadic,
                arguments,
                return_type,
                ..
            } = &ty.shape
            else {
                return invalid_type_operation(SemanticTypeOperationV1::Constant, location);
            };
            let callee = addressable_callable_abi(context.request, *callable)?;
            if *extern_abi != callee.extern_abi()
                || *c_variadic != callee.c_variadic()
                || arguments.fields.len() != callee.source_input_types().len()
                || arguments
                    .fields
                    .iter()
                    .zip(callee.source_input_types().iter())
                    .any(|(actual, expected)| actual != expected)
                || *return_type != callee.source_output_type()
            {
                return invalid_type_operation(SemanticTypeOperationV1::Constant, location);
            }
        }
    }
    Ok(())
}

fn pointer_target_range_is_valid(
    request: &InertSemanticMirRequestV1,
    pointer: &SemanticPointerTypeV1,
    metadata: SemanticPointerValueMetadataV1,
    offset: u64,
    target_size: u64,
    target_mutable: bool,
) -> bool {
    if target_object_size_bound_in(request.target, pointer.address_space)
        .is_none_or(|bound| target_size >= bound)
        || offset > target_size
    {
        return false;
    }
    if pointer.kind != SemanticPointerKindV1::Reference {
        return true;
    }
    let pointee_layout = &request.types[pointer.pointee.0 as usize].layout;
    let (alignment, extent) = match metadata {
        SemanticPointerValueMetadataV1::None => {
            let Some(size) = pointee_layout.size_bytes else {
                return false;
            };
            (pointee_layout.alignment_bytes, size)
        }
        SemanticPointerValueMetadataV1::SliceLength(length) => {
            let Some(element_size) = pointee_layout.size_bytes else {
                return false;
            };
            let Some(extent) = element_size.checked_mul(length) else {
                return false;
            };
            (pointee_layout.alignment_bytes, extent)
        }
        SemanticPointerValueMetadataV1::VTable(vtable) => {
            let concrete_layout =
                &request.types[request.vtables[vtable.0 as usize].concrete_type.0 as usize].layout;
            let Some(size) = concrete_layout.size_bytes else {
                return false;
            };
            (concrete_layout.alignment_bytes, size)
        }
    };
    if !offset.is_multiple_of(alignment)
        || (pointer.mutability == SemanticMutabilityV1::Mutable && !target_mutable)
    {
        return false;
    }
    offset
        .checked_add(extent)
        .is_some_and(|end| end <= target_size)
}

fn scalar_value_is_valid(shape: &SemanticTypeShapeV1, value: SemanticScalarValueV1) -> bool {
    match shape {
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool) => value.bits <= 1,
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Char) => {
            value.bits <= 0x10_ffff && !(0xd800..=0xdfff).contains(&value.bits)
        }
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer { .. })
        | SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float { .. }) => true,
        SemanticTypeShapeV1::ValidityScalar(validity) => validity
            .valid_ranges
            .iter()
            .any(|range| range.start <= value.bits && value.bits <= range.end),
        _ => false,
    }
}

fn require_type(
    expected: SemanticTypeIdV1,
    actual: SemanticTypeIdV1,
    location: SemanticMirLocationV1,
) -> Result<(), SemanticMirErrorV1> {
    if expected != actual {
        return Err(SemanticMirErrorV1::TypeMismatch {
            expected,
            actual,
            location,
        });
    }
    Ok(())
}

fn validate_terminator(
    context: &mut ValidationContextV1<'_>,
    function_id: SemanticFunctionIdV1,
    function: &SemanticFunctionDeclV1,
    location: SemanticMirLocationV1,
    terminator: &SemanticTerminatorKindV1,
) -> Result<(), SemanticMirErrorV1> {
    match terminator {
        SemanticTerminatorKindV1::Goto(edge) => {
            validate_edge(context, function, location, *edge, SemanticEdgeRoleV1::Goto)?;
        }
        SemanticTerminatorKindV1::SwitchInt {
            discriminant,
            targets,
        } => {
            validate_operand(context, function, location, discriminant)?;
            let Some(scalar) = scalar_type(context.request, discriminant.ty()) else {
                return invalid_type_operation(SemanticTypeOperationV1::Discriminant, location);
            };
            if matches!(scalar, SemanticScalarTypeV1::Float { .. })
                || targets
                    .values
                    .iter()
                    .any(|target| !scalar_raw_value_fits(scalar, target.value))
            {
                return invalid_type_operation(SemanticTypeOperationV1::Discriminant, location);
            }
            context.totals.charge(
                SemanticMirResourceV1::SwitchTargets,
                targets.values.len(),
                context.limits,
            )?;
            for target in &targets.values {
                validate_edge(
                    context,
                    function,
                    location,
                    target.edge,
                    SemanticEdgeRoleV1::SwitchValue,
                )?;
            }
            validate_edge(
                context,
                function,
                location,
                targets.otherwise,
                SemanticEdgeRoleV1::SwitchOtherwise,
            )?;
        }
        SemanticTerminatorKindV1::Call(call) => {
            validate_call(context, function_id, function, location, call)?;
        }
        SemanticTerminatorKindV1::TailCall(call) => {
            validate_tail_call(context, function_id, function, location, call)?;
        }
        SemanticTerminatorKindV1::Drop {
            place,
            drop_glue,
            target,
            unwind,
        } => {
            validate_place(context, function, location, place)?;
            context.function_reference(*drop_glue, location)?;
            match context.request.functions[drop_glue.0 as usize].role {
                SemanticFunctionRoleV1::DropGlue(dropped_type) if dropped_type == place.ty => {}
                SemanticFunctionRoleV1::DropGlue(dropped_type) => {
                    return Err(SemanticMirErrorV1::TypeMismatch {
                        expected: place.ty,
                        actual: dropped_type,
                        location,
                    });
                }
                role => {
                    return Err(SemanticMirErrorV1::InvalidFunctionRole {
                        function: *drop_glue,
                        role,
                        rooted: false,
                    });
                }
            }
            validate_edge(
                context,
                function,
                location,
                *target,
                SemanticEdgeRoleV1::DropReturn,
            )?;
            validate_unwind(
                context,
                function,
                location,
                *unwind,
                SemanticEdgeRoleV1::DropUnwind,
            )?;
        }
        SemanticTerminatorKindV1::Assert {
            condition,
            message,
            target,
            unwind,
            ..
        } => {
            validate_operand(context, function, location, condition)?;
            if !is_bool_type(context.request, condition.ty()) {
                return Err(SemanticMirErrorV1::TypeMismatch {
                    expected: bool_type_id(context.request).unwrap_or(condition.ty()),
                    actual: condition.ty(),
                    location,
                });
            }
            validate_assert_message(context, function, location, message)?;
            validate_edge(
                context,
                function,
                location,
                *target,
                SemanticEdgeRoleV1::AssertSuccess,
            )?;
            validate_unwind(
                context,
                function,
                location,
                *unwind,
                SemanticEdgeRoleV1::AssertUnwind,
            )?;
        }
        SemanticTerminatorKindV1::FalseEdge {
            real_target,
            imaginary_target,
        } => {
            validate_edge(
                context,
                function,
                location,
                *real_target,
                SemanticEdgeRoleV1::FalseEdgeReal,
            )?;
            validate_edge(
                context,
                function,
                location,
                *imaginary_target,
                SemanticEdgeRoleV1::FalseEdgeImaginary,
            )?;
        }
        SemanticTerminatorKindV1::Return
        | SemanticTerminatorKindV1::UnwindResume
        | SemanticTerminatorKindV1::UnwindTerminate
        | SemanticTerminatorKindV1::Abort
        | SemanticTerminatorKindV1::Unreachable => {}
    }
    Ok(())
}

fn validate_call(
    context: &mut ValidationContextV1<'_>,
    function_id: SemanticFunctionIdV1,
    function: &SemanticFunctionDeclV1,
    location: SemanticMirLocationV1,
    call: &SemanticDirectCallV1,
) -> Result<(), SemanticMirErrorV1> {
    context.callable_reference(call.callee, location)?;
    context.totals.charge(
        SemanticMirResourceV1::CallArguments,
        call.arguments.len(),
        context.limits,
    )?;
    context.totals.charge(
        SemanticMirResourceV1::CallArguments,
        call.variadic_argument_abis.len(),
        context.limits,
    )?;
    let callee_abi = callable_abi(context.request, call.callee)?.clone();
    if matches!(
        callee_abi.extern_abi(),
        SemanticExternAbiV1::Custom | SemanticExternAbiV1::GpuKernel
    ) {
        return Err(SemanticMirErrorV1::InvalidCallShape {
            function: function_id,
            tail: false,
        });
    }
    validate_call_arguments(
        context,
        function_id,
        false,
        function,
        location,
        &call.arguments,
        &call.variadic_argument_abis,
        &callee_abi,
    )?;
    match &call.destination {
        Some(destination) => {
            validate_place(context, function, location, &destination.place)?;
            require_type(
                callee_abi.source_output_type(),
                destination.place.ty,
                location,
            )?;
            validate_edge(
                context,
                function,
                location,
                destination.edge,
                SemanticEdgeRoleV1::CallReturn,
            )?;
        }
        None if !is_never_type(context.request, callee_abi.source_output_type()) => {
            return Err(SemanticMirErrorV1::InvalidCallShape {
                function: function_id,
                tail: false,
            });
        }
        None => {}
    }
    validate_unwind(
        context,
        function,
        location,
        call.unwind,
        SemanticEdgeRoleV1::CallUnwind,
    )?;
    Ok(())
}

fn validate_tail_call(
    context: &mut ValidationContextV1<'_>,
    function_id: SemanticFunctionIdV1,
    function: &SemanticFunctionDeclV1,
    location: SemanticMirLocationV1,
    call: &SemanticDirectTailCallV1,
) -> Result<(), SemanticMirErrorV1> {
    context.callable_reference(call.callee, location)?;
    if !matches!(
        context.request.callables[call.callee.0 as usize],
        SemanticCallableDeclV1::Defined { .. }
    ) {
        return Err(SemanticMirErrorV1::InvalidCallShape {
            function: function_id,
            tail: true,
        });
    }
    context.totals.charge(
        SemanticMirResourceV1::CallArguments,
        call.arguments.len(),
        context.limits,
    )?;
    let callee_abi = callable_abi(context.request, call.callee)?.clone();
    validate_call_arguments(
        context,
        function_id,
        true,
        function,
        location,
        &call.arguments,
        &[],
        &callee_abi,
    )?;
    if !supports_guaranteed_tail_call(&function.abi)
        || !supports_guaranteed_tail_call(&callee_abi)
        || !function_abis_are_tail_compatible(&function.abi, &callee_abi)
    {
        return Err(SemanticMirErrorV1::InvalidCallShape {
            function: function_id,
            tail: true,
        });
    }
    validate_unwind(
        context,
        function,
        location,
        call.unwind,
        SemanticEdgeRoleV1::TailCallUnwind,
    )?;
    Ok(())
}

fn callable_abi(
    request: &InertSemanticMirRequestV1,
    callable: SemanticCallableIdV1,
) -> Result<&SemanticFunctionAbiV1, SemanticMirErrorV1> {
    match request
        .callables
        .get(callable.0 as usize)
        .ok_or(SemanticMirErrorV1::InvalidFunctionAbi)?
    {
        SemanticCallableDeclV1::Defined { function } => request
            .functions
            .get(function.0 as usize)
            .map(|function| &function.abi)
            .ok_or(SemanticMirErrorV1::InvalidFunctionAbi),
        SemanticCallableDeclV1::DeviceFfiImport { binding, .. }
        | SemanticCallableDeclV1::CompilerIntrinsic { binding, .. } => Ok(&binding.abi),
    }
}

fn addressable_callable_abi(
    request: &InertSemanticMirRequestV1,
    callable: SemanticCallableIdV1,
) -> Result<&SemanticFunctionAbiV1, SemanticMirErrorV1> {
    match request
        .callables
        .get(callable.0 as usize)
        .ok_or(SemanticMirErrorV1::InvalidFunctionAbi)?
    {
        SemanticCallableDeclV1::Defined { function } => request
            .functions
            .get(function.0 as usize)
            .map(|function| &function.abi)
            .ok_or(SemanticMirErrorV1::InvalidFunctionAbi),
        SemanticCallableDeclV1::DeviceFfiImport { binding, .. } => Ok(&binding.abi),
        SemanticCallableDeclV1::CompilerIntrinsic { .. } => {
            Err(SemanticMirErrorV1::InvalidFunctionAbi)
        }
    }
}

fn supports_guaranteed_tail_call(abi: &SemanticFunctionAbiV1) -> bool {
    !abi.c_variadic()
        && !matches!(
            abi.extern_abi(),
            SemanticExternAbiV1::Custom | SemanticExternAbiV1::GpuKernel
        )
}

fn function_abis_are_tail_compatible(
    caller: &SemanticFunctionAbiV1,
    callee: &SemanticFunctionAbiV1,
) -> bool {
    caller.extern_abi() == callee.extern_abi()
        && !caller.c_variadic()
        && !callee.c_variadic()
        && !caller.hidden_arguments().iter().any(|argument| {
            argument.role
                == SemanticAbiArgumentRoleV1::Hidden(
                    SemanticAbiHiddenArgumentRoleV1::CallerLocation,
                )
        })
        && caller.source_input_types() == callee.source_input_types()
        && caller.source_output_type() == callee.source_output_type()
}

#[allow(clippy::too_many_arguments)]
fn validate_call_arguments(
    context: &mut ValidationContextV1<'_>,
    function_id: SemanticFunctionIdV1,
    tail: bool,
    function: &SemanticFunctionDeclV1,
    location: SemanticMirLocationV1,
    arguments: &[SemanticOperandV1],
    variadic_argument_abis: &[SemanticAbiValueV1],
    callee_abi: &SemanticFunctionAbiV1,
) -> Result<(), SemanticMirErrorV1> {
    if (!callee_abi.c_variadic() && !variadic_argument_abis.is_empty())
        || arguments.len()
            != callee_abi
                .source_input_types()
                .len()
                .checked_add(variadic_argument_abis.len())
                .ok_or(SemanticMirErrorV1::ArithmeticOverflow {
                    resource: SemanticMirResourceV1::CallArguments,
                })?
    {
        return Err(SemanticMirErrorV1::InvalidCallShape {
            function: function_id,
            tail,
        });
    }
    for (index, argument) in arguments.iter().enumerate() {
        validate_operand(context, function, location, argument)?;
        let expected = if index < callee_abi.source_input_types().len() {
            callee_abi.source_input_types()[index]
        } else {
            let variadic_abi =
                &variadic_argument_abis[index - callee_abi.source_input_types().len()];
            context.type_reference(variadic_abi.source_ty, location)?;
            if let Some(adjusted) = variadic_abi.adjusted() {
                context.type_reference(adjusted.ty, location)?;
            }
            validate_abi_value(
                context,
                variadic_abi,
                callee_abi.canon_abi,
                callee_abi.extern_abi(),
                SemanticAbiValuePositionV1::VariadicArgument,
            )?;
            variadic_abi.source_ty
        };
        require_type(expected, argument.ty(), location)?;
    }
    Ok(())
}

fn validate_assert_message(
    context: &mut ValidationContextV1<'_>,
    function: &SemanticFunctionDeclV1,
    location: SemanticMirLocationV1,
    message: &SemanticAssertMessageV1,
) -> Result<(), SemanticMirErrorV1> {
    match message {
        SemanticAssertMessageV1::BoundsCheck { length, index } => {
            validate_operand(context, function, location, length)?;
            validate_operand(context, function, location, index)?;
            if !is_unsigned_integer_type(context.request, length.ty())
                || !is_unsigned_integer_type(context.request, index.ty())
            {
                return invalid_type_operation(SemanticTypeOperationV1::Binary, location);
            }
        }
        SemanticAssertMessageV1::Overflow { left, right, .. } => {
            validate_operand(context, function, location, left)?;
            validate_operand(context, function, location, right)?;
            require_type(left.ty(), right.ty(), location)?;
            if !is_numeric_type(context.request, left.ty()) {
                return invalid_type_operation(SemanticTypeOperationV1::Binary, location);
            }
        }
        SemanticAssertMessageV1::DivisionByZero(operand)
        | SemanticAssertMessageV1::RemainderByZero(operand) => {
            validate_operand(context, function, location, operand)?;
            if !is_numeric_type(context.request, operand.ty()) {
                return invalid_type_operation(SemanticTypeOperationV1::Binary, location);
            }
        }
        SemanticAssertMessageV1::MisalignedPointerDereference {
            required_alignment,
            found_alignment,
        } => {
            validate_operand(context, function, location, required_alignment)?;
            validate_operand(context, function, location, found_alignment)?;
            require_type(required_alignment.ty(), found_alignment.ty(), location)?;
            if !is_unsigned_integer_type(context.request, required_alignment.ty()) {
                return invalid_type_operation(SemanticTypeOperationV1::Binary, location);
            }
        }
        SemanticAssertMessageV1::NullPointerDereference
        | SemanticAssertMessageV1::ResumedAfterReturn
        | SemanticAssertMessageV1::ResumedAfterPanic => {}
    }
    Ok(())
}

fn validate_unwind(
    context: &mut ValidationContextV1<'_>,
    function: &SemanticFunctionDeclV1,
    location: SemanticMirLocationV1,
    unwind: SemanticUnwindActionV1,
    expected_role: SemanticEdgeRoleV1,
) -> Result<(), SemanticMirErrorV1> {
    if let SemanticUnwindActionV1::Cleanup(edge) = unwind {
        validate_edge(context, function, location, edge, expected_role)?;
    }
    Ok(())
}

fn validate_edge(
    context: &mut ValidationContextV1<'_>,
    function: &SemanticFunctionDeclV1,
    location: SemanticMirLocationV1,
    edge: SemanticControlFlowEdgeV1,
    expected_role: SemanticEdgeRoleV1,
) -> Result<(), SemanticMirErrorV1> {
    if edge.role != expected_role {
        return Err(SemanticMirErrorV1::InvalidEdgeRole {
            expected: expected_role,
            actual: edge.role,
            location,
        });
    }
    context.reference(
        SemanticMirReferenceV1::Block,
        edge.target.0,
        function.blocks.len(),
        location,
    )
}

fn is_bool_type(request: &InertSemanticMirRequestV1, ty: SemanticTypeIdV1) -> bool {
    request.types.get(ty.0 as usize).is_some_and(|record| {
        matches!(
            record.shape,
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool)
        )
    })
}

fn bool_type_id(request: &InertSemanticMirRequestV1) -> Option<SemanticTypeIdV1> {
    request
        .types
        .iter()
        .position(|record| {
            matches!(
                record.shape,
                SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool)
            )
        })
        .map(|index| SemanticTypeIdV1(index as u32))
}

fn is_never_type(request: &InertSemanticMirRequestV1, ty: SemanticTypeIdV1) -> bool {
    request
        .types
        .get(ty.0 as usize)
        .is_some_and(|record| matches!(record.shape, SemanticTypeShapeV1::Never))
}

#[derive(Clone, Copy)]
enum SemanticClosureNodeV1 {
    Function(SemanticFunctionIdV1),
    Callable(SemanticCallableIdV1),
    Allocation(SemanticAllocationIdV1),
    Static(SemanticStaticIdV1),
    VTable(SemanticVTableIdV1),
}

fn validate_exact_function_closure(
    context: &mut ValidationContextV1<'_>,
) -> Result<(), SemanticMirErrorV1> {
    let request = context.request;
    let mut reached_functions = vec![false; request.functions.len()];
    let mut reached_callables = vec![false; request.callables.len()];
    let mut reached_allocations = vec![false; request.allocations.len()];
    let mut reached_statics = vec![false; request.statics.len()];
    let mut reached_vtables = vec![false; request.vtables.len()];
    let mut pending = request
        .roots
        .iter()
        .map(|root| SemanticClosureNodeV1::Callable(SemanticCallableIdV1(root.0)))
        .collect::<VecDeque<_>>();
    pending.extend(
        request
            .statics
            .iter()
            .enumerate()
            .filter(|(_, static_decl)| static_decl.export_symbol.is_some())
            .map(|(index, _)| SemanticClosureNodeV1::Static(SemanticStaticIdV1(index as u32))),
    );

    while let Some(node) = pending.pop_front() {
        context.one()?;
        match node {
            SemanticClosureNodeV1::Callable(callable) => {
                let index = callable.0 as usize;
                if reached_callables[index] {
                    continue;
                }
                reached_callables[index] = true;
                if let SemanticCallableDeclV1::Defined { function } = request.callables[index] {
                    pending.push_back(SemanticClosureNodeV1::Function(function));
                }
            }
            SemanticClosureNodeV1::Function(function) => {
                let index = function.0 as usize;
                if reached_functions[index] {
                    continue;
                }
                reached_functions[index] = true;
                reached_callables[index] = true;
                enqueue_retained_function_references(
                    context,
                    &request.functions[index],
                    &mut pending,
                )?;
            }
            SemanticClosureNodeV1::Allocation(allocation) => {
                let index = allocation.0 as usize;
                if reached_allocations[index] {
                    continue;
                }
                reached_allocations[index] = true;
                for relocation in &request.allocations[index].relocations {
                    context.one()?;
                    match relocation.target {
                        SemanticRelocationTargetV1::Allocation(target) => {
                            pending.push_back(SemanticClosureNodeV1::Allocation(target));
                        }
                        SemanticRelocationTargetV1::Callable(target) => {
                            pending.push_back(SemanticClosureNodeV1::Callable(target));
                        }
                        SemanticRelocationTargetV1::Static(target) => {
                            pending.push_back(SemanticClosureNodeV1::Static(target));
                        }
                        SemanticRelocationTargetV1::VTable(target) => {
                            pending.push_back(SemanticClosureNodeV1::VTable(target));
                        }
                    }
                }
            }
            SemanticClosureNodeV1::Static(static_id) => {
                let index = static_id.0 as usize;
                if reached_statics[index] {
                    continue;
                }
                reached_statics[index] = true;
                if let SemanticStaticDefinitionV1::Defined { initializer } =
                    request.statics[index].definition
                {
                    pending.push_back(SemanticClosureNodeV1::Allocation(initializer));
                }
            }
            SemanticClosureNodeV1::VTable(vtable) => {
                let index = vtable.0 as usize;
                if reached_vtables[index] {
                    continue;
                }
                reached_vtables[index] = true;
                pending.push_back(SemanticClosureNodeV1::Allocation(
                    request.vtables[index].allocation,
                ));
            }
        }
    }

    if let Some(index) = reached_functions.iter().position(|reached| !reached) {
        return Err(SemanticMirErrorV1::FunctionOutsideRootClosure {
            function: SemanticFunctionIdV1(index as u32),
        });
    }
    if let Some(index) = reached_callables.iter().position(|reached| !reached) {
        return Err(SemanticMirErrorV1::CallableOutsideRootClosure {
            callable: SemanticCallableIdV1(index as u32),
        });
    }
    if let Some(index) = reached_allocations.iter().position(|reached| !reached) {
        return Err(SemanticMirErrorV1::AllocationOutsideRootClosure {
            allocation: SemanticAllocationIdV1(index as u32),
        });
    }
    if let Some(index) = reached_statics.iter().position(|reached| !reached) {
        return Err(SemanticMirErrorV1::StaticOutsideRootClosure {
            static_id: SemanticStaticIdV1(index as u32),
        });
    }
    if let Some(index) = reached_vtables.iter().position(|reached| !reached) {
        return Err(SemanticMirErrorV1::VTableOutsideRootClosure {
            vtable: SemanticVTableIdV1(index as u32),
        });
    }
    validate_exact_type_closure(context)?;
    Ok(())
}

fn validate_exact_type_closure(
    context: &mut ValidationContextV1<'_>,
) -> Result<(), SemanticMirErrorV1> {
    let request = context.request;
    let mut reached = vec![false; request.types.len()];
    let mut pending = VecDeque::new();
    for function in &request.functions {
        context.one()?;
        if let SemanticFunctionRoleV1::DropGlue(dropped_type) = function.role {
            pending.push_back(dropped_type);
        }
        for argument in &function.abi.arguments {
            pending.push_back(argument.value.source_ty);
            if let Some(adjusted) = argument.value.adjusted() {
                pending.push_back(adjusted.ty);
            }
        }
        pending.push_back(function.abi.return_value.source_ty);
        if let Some(adjusted) = function.abi.return_value.adjusted() {
            pending.push_back(adjusted.ty);
        }
        pending.extend(function.abi.source_input_types().iter().copied());
        pending.push_back(function.abi.source_output_type());
        pending.extend(function.locals.iter().map(|local| local.ty));
        for block in &function.blocks {
            context.one()?;
            for statement in &block.statements {
                context.one()?;
                enqueue_statement_type_references(&statement.kind, &mut pending);
            }
            context.one()?;
            enqueue_terminator_type_references(&block.terminator.kind, &mut pending);
        }
    }
    for callable in &request.callables {
        let Some(binding) = callable.binding() else {
            continue;
        };
        context.one()?;
        for argument in &binding.abi.arguments {
            pending.push_back(argument.value.source_ty);
            if let Some(adjusted) = argument.value.adjusted() {
                pending.push_back(adjusted.ty);
            }
        }
        pending.push_back(binding.abi.return_value.source_ty);
        if let Some(adjusted) = binding.abi.return_value.adjusted() {
            pending.push_back(adjusted.ty);
        }
        pending.extend(binding.abi.source_input_types().iter().copied());
        pending.push_back(binding.abi.source_output_type());
        if let SemanticCallableDeclV1::CompilerIntrinsic { operation, .. } = callable {
            enqueue_compiler_intrinsic_type_references(*operation, &mut pending);
        }
    }
    for static_decl in &request.statics {
        context.one()?;
        pending.push_back(static_decl.ty);
    }
    for vtable in &request.vtables {
        context.one()?;
        pending.push_back(vtable.concrete_type);
        pending.push_back(vtable.dyn_type);
    }

    while let Some(ty) = pending.pop_front() {
        context.one()?;
        let index = ty.0 as usize;
        if reached[index] {
            continue;
        }
        reached[index] = true;
        match &request.types[index].shape {
            SemanticTypeShapeV1::Pointer(pointer) => pending.push_back(pointer.pointee),
            SemanticTypeShapeV1::Array { element, .. } | SemanticTypeShapeV1::Slice { element } => {
                pending.push_back(*element)
            }
            SemanticTypeShapeV1::Tuple(fields)
            | SemanticTypeShapeV1::Aggregate(fields)
            | SemanticTypeShapeV1::Union(fields) => {
                pending.extend(fields.fields.iter().copied());
            }
            SemanticTypeShapeV1::Enum {
                discriminant,
                variants,
            } => {
                pending.push_back(*discriminant);
                for variant in variants {
                    context.one()?;
                    pending.extend(variant.fields.fields.iter().copied());
                }
            }
            SemanticTypeShapeV1::FunctionPointer {
                arguments,
                return_type,
                ..
            } => {
                pending.extend(arguments.fields.iter().copied());
                pending.push_back(*return_type);
            }
            SemanticTypeShapeV1::Unit
            | SemanticTypeShapeV1::Never
            | SemanticTypeShapeV1::Scalar(_)
            | SemanticTypeShapeV1::ValidityScalar(_)
            | SemanticTypeShapeV1::Opaque => {}
        }
    }

    if let Some(index) = reached.iter().position(|reached| !reached) {
        return Err(SemanticMirErrorV1::TypeOutsideRootClosure {
            ty: SemanticTypeIdV1(index as u32),
        });
    }
    Ok(())
}

fn enqueue_compiler_intrinsic_type_references(
    operation: SemanticCompilerIntrinsicOperationV1,
    pending: &mut VecDeque<SemanticTypeIdV1>,
) {
    match operation {
        SemanticCompilerIntrinsicOperationV1::ThreadIndex(_)
        | SemanticCompilerIntrinsicOperationV1::WorkgroupIndex(_)
        | SemanticCompilerIntrinsicOperationV1::WorkgroupDimension(_)
        | SemanticCompilerIntrinsicOperationV1::GridDimension(_)
        | SemanticCompilerIntrinsicOperationV1::WorkgroupBarrier
        | SemanticCompilerIntrinsicOperationV1::WaveBarrier
        | SemanticCompilerIntrinsicOperationV1::FabsF32 => {}
        SemanticCompilerIntrinsicOperationV1::ThreadIndex1d {
            index_witness,
            raw_index,
        }
        | SemanticCompilerIntrinsicOperationV1::ThreadIndexGet {
            index_witness,
            raw_index,
        } => {
            pending.push_back(index_witness);
            pending.push_back(raw_index);
        }
        SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMut {
            disjoint_slice,
            index_witness,
            element,
            raw_index,
        } => {
            pending.push_back(disjoint_slice);
            pending.push_back(index_witness);
            pending.push_back(element);
            pending.push_back(raw_index);
        }
    }
}

fn enqueue_statement_type_references(
    statement: &SemanticStatementKindV1,
    pending: &mut VecDeque<SemanticTypeIdV1>,
) {
    match statement {
        SemanticStatementKindV1::Assign(assignment) => {
            enqueue_place_type_references(&assignment.destination, pending);
            enqueue_rvalue_type_references(&assignment.value, pending);
        }
        SemanticStatementKindV1::Store(store) => {
            enqueue_place_type_references(&store.destination, pending);
            enqueue_operand_type_references(&store.value, pending);
        }
        SemanticStatementKindV1::AtomicRmw(operation) => {
            enqueue_place_type_references(&operation.destination, pending);
            enqueue_place_type_references(&operation.address, pending);
            enqueue_operand_type_references(&operation.value, pending);
        }
        SemanticStatementKindV1::AtomicCompareExchange(operation) => {
            enqueue_place_type_references(&operation.destination, pending);
            enqueue_place_type_references(&operation.address, pending);
            enqueue_operand_type_references(&operation.expected, pending);
            enqueue_operand_type_references(&operation.replacement, pending);
        }
        SemanticStatementKindV1::SetDiscriminant { place, .. }
        | SemanticStatementKindV1::Deinitialize(place) => {
            enqueue_place_type_references(place, pending);
        }
        SemanticStatementKindV1::StorageLive(_)
        | SemanticStatementKindV1::StorageDead(_)
        | SemanticStatementKindV1::Nop => {}
    }
}

fn enqueue_rvalue_type_references(
    rvalue: &SemanticRvalueV1,
    pending: &mut VecDeque<SemanticTypeIdV1>,
) {
    pending.push_back(rvalue.result_type);
    match &rvalue.kind {
        SemanticRvalueKindV1::Use(operand)
        | SemanticRvalueKindV1::Unary { operand, .. }
        | SemanticRvalueKindV1::Cast { operand, .. } => {
            enqueue_operand_type_references(operand, pending);
        }
        SemanticRvalueKindV1::Binary { left, right, .. } => {
            enqueue_operand_type_references(left, pending);
            enqueue_operand_type_references(right, pending);
        }
        SemanticRvalueKindV1::Borrow { place, .. }
        | SemanticRvalueKindV1::AddressOf { place, .. }
        | SemanticRvalueKindV1::Length(place)
        | SemanticRvalueKindV1::Discriminant(place) => {
            enqueue_place_type_references(place, pending);
        }
        SemanticRvalueKindV1::Aggregate(aggregate) => {
            for operand in &aggregate.operands {
                enqueue_operand_type_references(operand, pending);
            }
        }
        SemanticRvalueKindV1::Load(load) => {
            enqueue_place_type_references(&load.source, pending);
        }
    }
}

fn enqueue_terminator_type_references(
    terminator: &SemanticTerminatorKindV1,
    pending: &mut VecDeque<SemanticTypeIdV1>,
) {
    match terminator {
        SemanticTerminatorKindV1::SwitchInt { discriminant, .. } => {
            enqueue_operand_type_references(discriminant, pending);
        }
        SemanticTerminatorKindV1::Call(call) => {
            for argument in &call.arguments {
                enqueue_operand_type_references(argument, pending);
            }
            for argument_abi in &call.variadic_argument_abis {
                pending.push_back(argument_abi.source_ty);
                if let Some(adjusted) = argument_abi.adjusted() {
                    pending.push_back(adjusted.ty);
                }
            }
            if let Some(destination) = &call.destination {
                enqueue_place_type_references(&destination.place, pending);
            }
        }
        SemanticTerminatorKindV1::TailCall(call) => {
            for argument in &call.arguments {
                enqueue_operand_type_references(argument, pending);
            }
        }
        SemanticTerminatorKindV1::Drop { place, .. } => {
            enqueue_place_type_references(place, pending);
        }
        SemanticTerminatorKindV1::Assert {
            condition, message, ..
        } => {
            enqueue_operand_type_references(condition, pending);
            enqueue_assert_type_references(message, pending);
        }
        SemanticTerminatorKindV1::Goto(_)
        | SemanticTerminatorKindV1::FalseEdge { .. }
        | SemanticTerminatorKindV1::Return
        | SemanticTerminatorKindV1::UnwindResume
        | SemanticTerminatorKindV1::UnwindTerminate
        | SemanticTerminatorKindV1::Abort
        | SemanticTerminatorKindV1::Unreachable => {}
    }
}

fn enqueue_assert_type_references(
    message: &SemanticAssertMessageV1,
    pending: &mut VecDeque<SemanticTypeIdV1>,
) {
    match message {
        SemanticAssertMessageV1::BoundsCheck { length, index } => {
            enqueue_operand_type_references(length, pending);
            enqueue_operand_type_references(index, pending);
        }
        SemanticAssertMessageV1::Overflow { left, right, .. } => {
            enqueue_operand_type_references(left, pending);
            enqueue_operand_type_references(right, pending);
        }
        SemanticAssertMessageV1::DivisionByZero(operand)
        | SemanticAssertMessageV1::RemainderByZero(operand) => {
            enqueue_operand_type_references(operand, pending);
        }
        SemanticAssertMessageV1::MisalignedPointerDereference {
            required_alignment,
            found_alignment,
        } => {
            enqueue_operand_type_references(required_alignment, pending);
            enqueue_operand_type_references(found_alignment, pending);
        }
        SemanticAssertMessageV1::NullPointerDereference
        | SemanticAssertMessageV1::ResumedAfterReturn
        | SemanticAssertMessageV1::ResumedAfterPanic => {}
    }
}

fn enqueue_operand_type_references(
    operand: &SemanticOperandV1,
    pending: &mut VecDeque<SemanticTypeIdV1>,
) {
    match operand {
        SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => {
            enqueue_place_type_references(place, pending);
        }
        SemanticOperandV1::Constant(constant) => pending.push_back(constant.ty),
    }
}

fn enqueue_place_type_references(
    place: &SemanticPlaceV1,
    pending: &mut VecDeque<SemanticTypeIdV1>,
) {
    pending.push_back(place.ty);
    pending.extend(
        place
            .projections
            .iter()
            .map(|projection| projection.result_type),
    );
}

fn enqueue_retained_function_references(
    context: &mut ValidationContextV1<'_>,
    function: &SemanticFunctionDeclV1,
    pending: &mut VecDeque<SemanticClosureNodeV1>,
) -> Result<(), SemanticMirErrorV1> {
    for block in &function.blocks {
        context.one()?;
        for statement in &block.statements {
            context.one()?;
            enqueue_statement_closure_references(context, &statement.kind, pending)?;
        }
        context.one()?;
        enqueue_terminator_closure_references(context, &block.terminator.kind, pending)?;
    }
    Ok(())
}

fn enqueue_statement_closure_references(
    context: &mut ValidationContextV1<'_>,
    statement: &SemanticStatementKindV1,
    pending: &mut VecDeque<SemanticClosureNodeV1>,
) -> Result<(), SemanticMirErrorV1> {
    match statement {
        SemanticStatementKindV1::Assign(assignment) => {
            enqueue_rvalue_closure_references(context, &assignment.value.kind, pending)?;
        }
        SemanticStatementKindV1::Store(store) => {
            enqueue_operand_closure_references(context, &store.value, pending)?;
        }
        SemanticStatementKindV1::AtomicRmw(operation) => {
            enqueue_operand_closure_references(context, &operation.value, pending)?;
        }
        SemanticStatementKindV1::AtomicCompareExchange(operation) => {
            enqueue_operand_closure_references(context, &operation.expected, pending)?;
            enqueue_operand_closure_references(context, &operation.replacement, pending)?;
        }
        SemanticStatementKindV1::SetDiscriminant { .. }
        | SemanticStatementKindV1::Deinitialize(_)
        | SemanticStatementKindV1::StorageLive(_)
        | SemanticStatementKindV1::StorageDead(_)
        | SemanticStatementKindV1::Nop => {}
    }
    Ok(())
}

fn enqueue_rvalue_closure_references(
    context: &mut ValidationContextV1<'_>,
    rvalue: &SemanticRvalueKindV1,
    pending: &mut VecDeque<SemanticClosureNodeV1>,
) -> Result<(), SemanticMirErrorV1> {
    match rvalue {
        SemanticRvalueKindV1::Use(operand)
        | SemanticRvalueKindV1::Unary { operand, .. }
        | SemanticRvalueKindV1::Cast { operand, .. } => {
            enqueue_operand_closure_references(context, operand, pending)?;
        }
        SemanticRvalueKindV1::Binary { left, right, .. } => {
            enqueue_operand_closure_references(context, left, pending)?;
            enqueue_operand_closure_references(context, right, pending)?;
        }
        SemanticRvalueKindV1::Aggregate(aggregate) => {
            for operand in &aggregate.operands {
                enqueue_operand_closure_references(context, operand, pending)?;
            }
        }
        SemanticRvalueKindV1::Borrow { .. }
        | SemanticRvalueKindV1::AddressOf { .. }
        | SemanticRvalueKindV1::Length(_)
        | SemanticRvalueKindV1::Discriminant(_)
        | SemanticRvalueKindV1::Load(_) => {}
    }
    Ok(())
}

fn enqueue_terminator_closure_references(
    context: &mut ValidationContextV1<'_>,
    terminator: &SemanticTerminatorKindV1,
    pending: &mut VecDeque<SemanticClosureNodeV1>,
) -> Result<(), SemanticMirErrorV1> {
    match terminator {
        SemanticTerminatorKindV1::SwitchInt { discriminant, .. } => {
            enqueue_operand_closure_references(context, discriminant, pending)?;
        }
        SemanticTerminatorKindV1::Call(call) => {
            for argument in &call.arguments {
                enqueue_operand_closure_references(context, argument, pending)?;
            }
            pending.push_back(SemanticClosureNodeV1::Callable(call.callee));
        }
        SemanticTerminatorKindV1::Drop { drop_glue, .. } => {
            pending.push_back(SemanticClosureNodeV1::Function(*drop_glue));
        }
        SemanticTerminatorKindV1::TailCall(call) => {
            for argument in &call.arguments {
                enqueue_operand_closure_references(context, argument, pending)?;
            }
            pending.push_back(SemanticClosureNodeV1::Callable(call.callee));
        }
        SemanticTerminatorKindV1::Assert {
            condition, message, ..
        } => {
            enqueue_operand_closure_references(context, condition, pending)?;
            enqueue_assert_message_closure_references(context, message, pending)?;
        }
        SemanticTerminatorKindV1::Goto(_)
        | SemanticTerminatorKindV1::FalseEdge { .. }
        | SemanticTerminatorKindV1::Return
        | SemanticTerminatorKindV1::UnwindResume
        | SemanticTerminatorKindV1::UnwindTerminate
        | SemanticTerminatorKindV1::Abort
        | SemanticTerminatorKindV1::Unreachable => {}
    }
    Ok(())
}

fn enqueue_assert_message_closure_references(
    context: &mut ValidationContextV1<'_>,
    message: &SemanticAssertMessageV1,
    pending: &mut VecDeque<SemanticClosureNodeV1>,
) -> Result<(), SemanticMirErrorV1> {
    match message {
        SemanticAssertMessageV1::BoundsCheck { length, index } => {
            enqueue_operand_closure_references(context, length, pending)?;
            enqueue_operand_closure_references(context, index, pending)?;
        }
        SemanticAssertMessageV1::Overflow { left, right, .. } => {
            enqueue_operand_closure_references(context, left, pending)?;
            enqueue_operand_closure_references(context, right, pending)?;
        }
        SemanticAssertMessageV1::DivisionByZero(operand)
        | SemanticAssertMessageV1::RemainderByZero(operand) => {
            enqueue_operand_closure_references(context, operand, pending)?;
        }
        SemanticAssertMessageV1::MisalignedPointerDereference {
            required_alignment,
            found_alignment,
        } => {
            enqueue_operand_closure_references(context, required_alignment, pending)?;
            enqueue_operand_closure_references(context, found_alignment, pending)?;
        }
        SemanticAssertMessageV1::NullPointerDereference
        | SemanticAssertMessageV1::ResumedAfterReturn
        | SemanticAssertMessageV1::ResumedAfterPanic => {}
    }
    Ok(())
}

fn enqueue_operand_closure_references(
    context: &mut ValidationContextV1<'_>,
    operand: &SemanticOperandV1,
    pending: &mut VecDeque<SemanticClosureNodeV1>,
) -> Result<(), SemanticMirErrorV1> {
    context.one()?;
    let SemanticOperandV1::Constant(constant) = operand else {
        return Ok(());
    };
    match constant.value {
        SemanticConstantValueV1::Pointer(pointer) => {
            match pointer.provenance {
                SemanticPointerProvenanceV1::Allocation(allocation) => {
                    pending.push_back(SemanticClosureNodeV1::Allocation(allocation));
                }
                SemanticPointerProvenanceV1::Callable(callable) => {
                    pending.push_back(SemanticClosureNodeV1::Callable(callable));
                }
                SemanticPointerProvenanceV1::Static(static_id) => {
                    pending.push_back(SemanticClosureNodeV1::Static(static_id));
                }
                SemanticPointerProvenanceV1::ExposedAddress => {}
            }
            if let SemanticPointerValueMetadataV1::VTable(vtable) = pointer.metadata {
                pending.push_back(SemanticClosureNodeV1::VTable(vtable));
            }
        }
        SemanticConstantValueV1::Callable(callable) => {
            pending.push_back(SemanticClosureNodeV1::Callable(callable));
        }
        SemanticConstantValueV1::ZeroSized
        | SemanticConstantValueV1::Scalar(_)
        | SemanticConstantValueV1::Bytes(_) => {}
    }
    Ok(())
}

fn encode_request(
    request: &InertSemanticMirRequestV1,
    limits: SemanticMirLimitsV1,
) -> Result<Vec<u8>, SemanticMirErrorV1> {
    let mut writer = CanonicalWriterV1::new(limits.limit(SemanticMirResourceV1::CanonicalBytes));
    writer.raw(MAGIC)?;
    writer.u16(INERT_SEMANTIC_MIR_VERSION_V1)?;
    writer.identity(request.target.identity.0)?;
    writer.u8(match request.target.architecture {
        SemanticTargetArchitectureV1::AmdGpuGfx942 => 0,
    })?;
    writer.u64(request.target.object_size_bound_bytes)?;
    writer.count(request.types.len())?;
    for ty in &request.types {
        encode_type(&mut writer, ty)?;
    }
    writer.count(request.allocations.len())?;
    for allocation in &request.allocations {
        encode_allocation(&mut writer, allocation)?;
    }
    writer.count(request.statics.len())?;
    for static_decl in &request.statics {
        encode_static(&mut writer, static_decl)?;
    }
    writer.count(request.vtables.len())?;
    for vtable in &request.vtables {
        encode_vtable(&mut writer, vtable)?;
    }
    writer.count(request.functions.len())?;
    for function in &request.functions {
        encode_function(&mut writer, function)?;
    }
    writer.count(request.callables.len())?;
    for callable in &request.callables {
        encode_callable(&mut writer, callable)?;
    }
    writer.count(request.roots.len())?;
    for root in &request.roots {
        writer.u32(root.0)?;
    }
    Ok(writer.finish())
}

struct CanonicalWriterV1 {
    bytes: Vec<u8>,
    max: u64,
}

impl CanonicalWriterV1 {
    fn new(max: u64) -> Self {
        Self {
            bytes: Vec::new(),
            max,
        }
    }

    fn finish(self) -> Vec<u8> {
        self.bytes
    }

    fn reserve(&self, additional: usize) -> Result<(), SemanticMirErrorV1> {
        let next = u64::try_from(self.bytes.len())
            .map_err(|_| SemanticMirErrorV1::ArithmeticOverflow {
                resource: SemanticMirResourceV1::CanonicalBytes,
            })?
            .checked_add(u64::try_from(additional).map_err(|_| {
                SemanticMirErrorV1::ArithmeticOverflow {
                    resource: SemanticMirResourceV1::CanonicalBytes,
                }
            })?)
            .ok_or(SemanticMirErrorV1::ArithmeticOverflow {
                resource: SemanticMirResourceV1::CanonicalBytes,
            })?;
        if next > self.max {
            return Err(SemanticMirErrorV1::LimitExceeded {
                resource: SemanticMirResourceV1::CanonicalBytes,
                actual: next,
                max: self.max,
            });
        }
        Ok(())
    }

    fn raw(&mut self, bytes: &[u8]) -> Result<(), SemanticMirErrorV1> {
        self.reserve(bytes.len())?;
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }

    fn blob(&mut self, bytes: &[u8]) -> Result<(), SemanticMirErrorV1> {
        self.u64(u64::try_from(bytes.len()).map_err(|_| {
            SemanticMirErrorV1::ArithmeticOverflow {
                resource: SemanticMirResourceV1::CanonicalBytes,
            }
        })?)?;
        self.raw(bytes)
    }

    fn identity(&mut self, bytes: [u8; 32]) -> Result<(), SemanticMirErrorV1> {
        self.raw(&bytes)
    }

    fn count(&mut self, count: usize) -> Result<(), SemanticMirErrorV1> {
        self.u32(
            u32::try_from(count).map_err(|_| SemanticMirErrorV1::ArithmeticOverflow {
                resource: SemanticMirResourceV1::CanonicalBytes,
            })?,
        )
    }

    fn bool(&mut self, value: bool) -> Result<(), SemanticMirErrorV1> {
        self.u8(u8::from(value))
    }

    fn u8(&mut self, value: u8) -> Result<(), SemanticMirErrorV1> {
        self.raw(&[value])
    }

    fn u16(&mut self, value: u16) -> Result<(), SemanticMirErrorV1> {
        self.raw(&value.to_le_bytes())
    }

    fn u32(&mut self, value: u32) -> Result<(), SemanticMirErrorV1> {
        self.raw(&value.to_le_bytes())
    }

    fn u64(&mut self, value: u64) -> Result<(), SemanticMirErrorV1> {
        self.raw(&value.to_le_bytes())
    }

    fn i64(&mut self, value: i64) -> Result<(), SemanticMirErrorV1> {
        self.raw(&value.to_le_bytes())
    }

    fn u128(&mut self, value: u128) -> Result<(), SemanticMirErrorV1> {
        self.raw(&value.to_le_bytes())
    }
}

fn encode_source(
    writer: &mut CanonicalWriterV1,
    source: SemanticSourceProvenanceV1,
) -> Result<(), SemanticMirErrorV1> {
    encode_source_origin(writer, source.expansion)?;
    encode_source_origin(writer, source.call_site)
}

fn encode_source_origin(
    writer: &mut CanonicalWriterV1,
    source: Option<SemanticSourceOriginV1>,
) -> Result<(), SemanticMirErrorV1> {
    let Some(source) = source else {
        return writer.u8(0);
    };
    writer.u8(1)?;
    writer.identity(source.file.0)?;
    writer.u64(source.byte_start)?;
    writer.u64(source.byte_end)?;
    writer.u32(source.line_start)?;
    writer.u32(source.column_start)?;
    writer.u32(source.line_end)?;
    writer.u32(source.column_end)
}

fn encode_type(
    writer: &mut CanonicalWriterV1,
    ty: &SemanticTypeDeclV1,
) -> Result<(), SemanticMirErrorV1> {
    writer.identity(ty.identity.0)?;
    writer.identity(ty.layout_identity.0)?;
    encode_type_layout(writer, &ty.layout)?;
    writer.bool(ty.abi_properties.pass_indirectly_in_non_rustic_abis)?;
    writer.bool(ty.abi_properties.has_unsized_foreign_tail)?;
    writer.bool(ty.abi_properties.rustc_layout_is_noundef)?;
    encode_optional_pointee_info(writer, ty.abi_properties.first_pointee)?;
    encode_optional_pointee_info(writer, ty.abi_properties.second_pointee)?;
    match &ty.shape {
        SemanticTypeShapeV1::Unit => writer.u8(0),
        SemanticTypeShapeV1::Never => writer.u8(1),
        SemanticTypeShapeV1::Scalar(scalar) => {
            writer.u8(2)?;
            encode_scalar_type(writer, *scalar)
        }
        SemanticTypeShapeV1::ValidityScalar(validity) => {
            writer.u8(10)?;
            encode_scalar_type(writer, validity.scalar)?;
            encode_validity_ranges(writer, &validity.valid_ranges)
        }
        SemanticTypeShapeV1::Pointer(pointer) => {
            writer.u8(3)?;
            writer.u32(pointer.pointee.0)?;
            writer.u8(match pointer.kind {
                SemanticPointerKindV1::Raw => 0,
                SemanticPointerKindV1::Reference => 1,
            })?;
            encode_mutability(writer, pointer.mutability)?;
            writer.u32(pointer.address_space)?;
            writer.u16(pointer.pointer_width_bits)?;
            match pointer.metadata {
                SemanticPointerMetadataV1::None => writer.u8(0),
                SemanticPointerMetadataV1::SliceLength => writer.u8(1),
                SemanticPointerMetadataV1::VTable => writer.u8(2),
            }
        }
        SemanticTypeShapeV1::Array { element, length } => {
            writer.u8(4)?;
            writer.u32(element.0)?;
            writer.u64(*length)
        }
        SemanticTypeShapeV1::Slice { element } => {
            writer.u8(12)?;
            writer.u32(element.0)
        }
        SemanticTypeShapeV1::Tuple(fields) => {
            writer.u8(5)?;
            encode_type_list(writer, fields)
        }
        SemanticTypeShapeV1::Aggregate(fields) => {
            writer.u8(6)?;
            encode_type_list(writer, fields)
        }
        SemanticTypeShapeV1::Union(fields) => {
            writer.u8(11)?;
            encode_type_list(writer, fields)
        }
        SemanticTypeShapeV1::Enum {
            discriminant,
            variants,
        } => {
            writer.u8(7)?;
            writer.u32(discriminant.0)?;
            writer.count(variants.len())?;
            for variant in variants {
                writer.u128(variant.discriminant)?;
                writer.bool(variant.uninhabited)?;
                encode_type_list(writer, &variant.fields)?;
            }
            Ok(())
        }
        SemanticTypeShapeV1::FunctionPointer {
            safety,
            extern_abi,
            c_variadic,
            arguments,
            return_type,
        } => {
            writer.u8(8)?;
            encode_function_safety(writer, *safety)?;
            encode_extern_abi(writer, *extern_abi)?;
            writer.bool(*c_variadic)?;
            encode_type_list(writer, arguments)?;
            writer.u32(return_type.0)
        }
        SemanticTypeShapeV1::Opaque => writer.u8(9),
    }
}

fn encode_function_safety(
    writer: &mut CanonicalWriterV1,
    safety: SemanticFunctionSafetyV1,
) -> Result<(), SemanticMirErrorV1> {
    match safety {
        SemanticFunctionSafetyV1::Safe => writer.u8(0),
        SemanticFunctionSafetyV1::Unsafe => writer.u8(1),
    }
}

fn encode_type_layout(
    writer: &mut CanonicalWriterV1,
    layout: &SemanticTypeLayoutV1,
) -> Result<(), SemanticMirErrorV1> {
    writer.u64(layout.rustc_size_bytes)?;
    match layout.size_bytes {
        Some(size) => {
            writer.u8(1)?;
            writer.u64(size)?;
        }
        None => writer.u8(0)?,
    }
    writer.u64(layout.alignment_bytes)?;
    encode_fields_shape(writer, &layout.fields)?;
    encode_rustc_variants(writer, &layout.variants)?;
    writer.bool(layout.uninhabited)?;
    encode_backend_repr(writer, &layout.backend_repr)?;
    match layout.largest_niche {
        Some(niche) => {
            writer.u8(1)?;
            writer.u64(niche.offset_bytes)?;
            encode_backend_primitive(writer, niche.primitive)?;
            writer.u128(niche.valid_range.start)?;
            writer.u128(niche.valid_range.end)?;
        }
        None => writer.u8(0)?,
    }
    match layout.max_repr_alignment_bytes {
        Some(alignment) => {
            writer.u8(1)?;
            writer.u64(alignment)?;
        }
        None => writer.u8(0)?,
    }
    writer.u64(layout.unadjusted_abi_alignment_bytes)?;
    writer.u64(layout.randomization_seed)?;
    encode_layout_details(writer, &layout.details)
}

fn encode_optional_pointee_info(
    writer: &mut CanonicalWriterV1,
    pointee: Option<SemanticAbiPointeeInfoV1>,
) -> Result<(), SemanticMirErrorV1> {
    let Some(pointee) = pointee else {
        return writer.u8(0);
    };
    writer.u8(1)?;
    match pointee.kind {
        SemanticAbiPointeeKindV1::Raw => writer.u8(0)?,
        SemanticAbiPointeeKindV1::SharedReference { frozen } => {
            writer.u8(1)?;
            writer.bool(frozen)?;
        }
        SemanticAbiPointeeKindV1::MutableReference { unpin } => {
            writer.u8(2)?;
            writer.bool(unpin)?;
        }
        SemanticAbiPointeeKindV1::Box { unpin, global } => {
            writer.u8(3)?;
            writer.bool(unpin)?;
            writer.bool(global)?;
        }
    }
    writer.u64(pointee.guaranteed_size_bytes)?;
    writer.u64(pointee.reliable_alignment_bytes)
}

fn encode_fields_shape(
    writer: &mut CanonicalWriterV1,
    fields: &SemanticFieldsShapeV1,
) -> Result<(), SemanticMirErrorV1> {
    match fields {
        SemanticFieldsShapeV1::Primitive => writer.u8(0),
        SemanticFieldsShapeV1::Union { field_count } => {
            writer.u8(1)?;
            writer.u64(*field_count)
        }
        SemanticFieldsShapeV1::Array {
            stride_bytes,
            count,
        } => {
            writer.u8(2)?;
            writer.u64(*stride_bytes)?;
            writer.u64(*count)
        }
        SemanticFieldsShapeV1::Arbitrary {
            source_order_offsets_bytes,
            memory_order_source_indices,
        } => {
            writer.u8(3)?;
            writer.count(source_order_offsets_bytes.len())?;
            for offset in source_order_offsets_bytes {
                writer.u64(*offset)?;
            }
            writer.count(memory_order_source_indices.len())?;
            for index in memory_order_source_indices {
                writer.u32(*index)?;
            }
            Ok(())
        }
    }
}

fn encode_backend_repr(
    writer: &mut CanonicalWriterV1,
    backend_repr: &SemanticBackendReprV1,
) -> Result<(), SemanticMirErrorV1> {
    match *backend_repr {
        SemanticBackendReprV1::Memory { sized } => {
            writer.u8(0)?;
            writer.bool(sized)
        }
        SemanticBackendReprV1::Scalar(scalar) => {
            writer.u8(1)?;
            encode_backend_scalar(writer, scalar)
        }
        SemanticBackendReprV1::ScalarPair { first, second } => {
            writer.u8(2)?;
            encode_backend_scalar(writer, first)?;
            encode_backend_scalar(writer, second)
        }
        SemanticBackendReprV1::SimdVector { element, count } => {
            writer.u8(3)?;
            encode_backend_scalar(writer, element)?;
            writer.u64(count)
        }
        SemanticBackendReprV1::SimdScalableVector { element, count } => {
            writer.u8(4)?;
            encode_backend_scalar(writer, element)?;
            writer.u64(count)
        }
    }
}

fn encode_backend_scalar(
    writer: &mut CanonicalWriterV1,
    scalar: SemanticBackendScalarV1,
) -> Result<(), SemanticMirErrorV1> {
    match scalar {
        SemanticBackendScalarV1::Initialized {
            primitive,
            valid_range,
        } => {
            writer.u8(0)?;
            encode_backend_primitive(writer, primitive)?;
            writer.u128(valid_range.start)?;
            writer.u128(valid_range.end)
        }
        SemanticBackendScalarV1::Union { primitive } => {
            writer.u8(1)?;
            encode_backend_primitive(writer, primitive)
        }
    }
}

fn encode_backend_primitive(
    writer: &mut CanonicalWriterV1,
    primitive: SemanticBackendPrimitiveV1,
) -> Result<(), SemanticMirErrorV1> {
    match primitive {
        SemanticBackendPrimitiveV1::Integer {
            signed,
            bits,
            alignment_bytes,
        } => {
            writer.u8(0)?;
            writer.bool(signed)?;
            writer.u16(bits)?;
            writer.u64(alignment_bytes)
        }
        SemanticBackendPrimitiveV1::Float {
            bits,
            alignment_bytes,
        } => {
            writer.u8(1)?;
            writer.u16(bits)?;
            writer.u64(alignment_bytes)
        }
        SemanticBackendPrimitiveV1::Pointer {
            address_space,
            size_bytes,
            alignment_bytes,
        } => {
            writer.u8(2)?;
            writer.u32(address_space)?;
            writer.u64(size_bytes)?;
            writer.u64(alignment_bytes)
        }
    }
}

fn encode_layout_details(
    writer: &mut CanonicalWriterV1,
    details: &SemanticTypeLayoutDetailsV1,
) -> Result<(), SemanticMirErrorV1> {
    match details {
        SemanticTypeLayoutDetailsV1::None => writer.u8(0),
        SemanticTypeLayoutDetailsV1::Aggregate(aggregate) => {
            writer.u8(1)?;
            encode_aggregate_layout(writer, aggregate)
        }
    }
}

fn encode_rustc_variants(
    writer: &mut CanonicalWriterV1,
    variants: &SemanticRustcVariantsV1,
) -> Result<(), SemanticMirErrorV1> {
    match variants {
        SemanticRustcVariantsV1::Empty => writer.u8(0),
        SemanticRustcVariantsV1::Single { index } => {
            writer.u8(1)?;
            writer.u32(*index)
        }
        SemanticRustcVariantsV1::Multiple(layout) => {
            writer.u8(2)?;
            writer.count(layout.variants.len())?;
            for variant in &layout.variants {
                encode_enum_variant_layout(writer, variant)?;
            }
            encode_enum_encoding(writer, &layout.encoding)
        }
    }
}

fn encode_enum_variant_layout(
    writer: &mut CanonicalWriterV1,
    variant: &SemanticEnumVariantLayoutV1,
) -> Result<(), SemanticMirErrorV1> {
    writer.u32(variant.variant_index)?;
    writer.u64(variant.rustc_size_bytes)?;
    writer.u64(variant.alignment_bytes)?;
    encode_fields_shape(writer, &variant.fields)?;
    encode_backend_repr(writer, &variant.backend_repr)?;
    match variant.largest_niche {
        Some(niche) => {
            writer.u8(1)?;
            writer.u64(niche.offset_bytes)?;
            encode_backend_primitive(writer, niche.primitive)?;
            writer.u128(niche.valid_range.start)?;
            writer.u128(niche.valid_range.end)?;
        }
        None => writer.u8(0)?,
    }
    writer.bool(variant.uninhabited)?;
    match variant.max_repr_alignment_bytes {
        Some(alignment) => {
            writer.u8(1)?;
            writer.u64(alignment)?;
        }
        None => writer.u8(0)?,
    }
    writer.u64(variant.unadjusted_abi_alignment_bytes)?;
    writer.u64(variant.randomization_seed)?;
    encode_aggregate_layout(writer, &variant.aggregate)
}

fn encode_aggregate_layout(
    writer: &mut CanonicalWriterV1,
    aggregate: &SemanticAggregateLayoutV1,
) -> Result<(), SemanticMirErrorV1> {
    writer.count(aggregate.field_offsets.len())?;
    for offset in &aggregate.field_offsets {
        writer.u64(*offset)?;
    }
    writer.count(aggregate.padding.len())?;
    for padding in &aggregate.padding {
        writer.u64(padding.offset_bytes)?;
        writer.u64(padding.size_bytes)?;
    }
    Ok(())
}

fn encode_enum_encoding(
    writer: &mut CanonicalWriterV1,
    encoding: &SemanticEnumEncodingV1,
) -> Result<(), SemanticMirErrorV1> {
    match encoding {
        SemanticEnumEncodingV1::Direct(direct) => {
            writer.u8(0)?;
            writer.u32(direct.tag_field)?;
            writer.u64(direct.tag_offset_bytes)?;
            encode_backend_scalar(writer, direct.tag)
        }
        SemanticEnumEncodingV1::Niche(niche) => {
            writer.u8(1)?;
            writer.u32(niche.tag_field)?;
            writer.count(niche.source.path.len())?;
            for component in &niche.source.path {
                match component {
                    SemanticNichePathComponentV1::Field(index) => {
                        writer.u8(0)?;
                        writer.u32(*index)?;
                    }
                    SemanticNichePathComponentV1::ArrayElement(index) => {
                        writer.u8(1)?;
                        writer.u64(*index)?;
                    }
                }
            }
            writer.u64(niche.source.expected_offset_bytes)?;
            writer.u64(niche.source_niche.offset_bytes)?;
            encode_backend_primitive(writer, niche.source_niche.primitive)?;
            writer.u128(niche.source_niche.valid_range.start)?;
            writer.u128(niche.source_niche.valid_range.end)?;
            encode_backend_scalar(writer, niche.tag)?;
            writer.u32(niche.untagged_variant)?;
            writer.u32(niche.niche_variants_start)?;
            writer.u32(niche.niche_variants_end)?;
            writer.u128(niche.niche_start)
        }
    }
}

fn encode_validity_ranges(
    writer: &mut CanonicalWriterV1,
    ranges: &[SemanticScalarValidityRangeV1],
) -> Result<(), SemanticMirErrorV1> {
    writer.count(ranges.len())?;
    for range in ranges {
        writer.u128(range.start)?;
        writer.u128(range.end)?;
    }
    Ok(())
}

fn encode_scalar_type(
    writer: &mut CanonicalWriterV1,
    scalar: SemanticScalarTypeV1,
) -> Result<(), SemanticMirErrorV1> {
    match scalar {
        SemanticScalarTypeV1::Bool => writer.u8(0),
        SemanticScalarTypeV1::Char => writer.u8(1),
        SemanticScalarTypeV1::Integer { signed, bits } => {
            writer.u8(2)?;
            writer.bool(signed)?;
            writer.u16(bits)
        }
        SemanticScalarTypeV1::Float { bits } => {
            writer.u8(3)?;
            writer.u16(bits)
        }
    }
}

fn encode_mutability(
    writer: &mut CanonicalWriterV1,
    mutability: SemanticMutabilityV1,
) -> Result<(), SemanticMirErrorV1> {
    writer.u8(match mutability {
        SemanticMutabilityV1::Immutable => 0,
        SemanticMutabilityV1::Mutable => 1,
    })
}

fn encode_type_list(
    writer: &mut CanonicalWriterV1,
    fields: &SemanticAggregateTypeV1,
) -> Result<(), SemanticMirErrorV1> {
    writer.count(fields.fields.len())?;
    for field in &fields.fields {
        writer.u32(field.0)?;
    }
    Ok(())
}

fn encode_allocation(
    writer: &mut CanonicalWriterV1,
    allocation: &SemanticAllocationDeclV1,
) -> Result<(), SemanticMirErrorV1> {
    writer.identity(allocation.identity.0)?;
    writer.u32(allocation.address_space)?;
    writer.blob(&allocation.bytes)?;
    writer.blob(&allocation.initialized_mask)?;
    writer.u64(allocation.alignment_bytes)?;
    writer.bool(allocation.mutable)?;
    writer.count(allocation.relocations.len())?;
    for relocation in &allocation.relocations {
        writer.u64(relocation.byte_offset)?;
        writer.u8(relocation.width_bytes)?;
        writer.u32(relocation.address_space)?;
        writer.i64(relocation.addend)?;
        match relocation.target {
            SemanticRelocationTargetV1::Allocation(id) => {
                writer.u8(0)?;
                writer.u32(id.0)?;
            }
            SemanticRelocationTargetV1::Callable(id) => {
                writer.u8(1)?;
                writer.u32(id.0)?;
            }
            SemanticRelocationTargetV1::Static(id) => {
                writer.u8(2)?;
                writer.u32(id.0)?;
            }
            SemanticRelocationTargetV1::VTable(id) => {
                writer.u8(3)?;
                writer.u32(id.0)?;
            }
        }
    }
    Ok(())
}

fn encode_static(
    writer: &mut CanonicalWriterV1,
    static_decl: &SemanticStaticDeclV1,
) -> Result<(), SemanticMirErrorV1> {
    writer.identity(static_decl.identity.0)?;
    encode_source(writer, static_decl.source)?;
    writer.u32(static_decl.ty.0)?;
    writer.bool(static_decl.mutable)?;
    writer.u32(static_decl.address_space)?;
    match &static_decl.definition {
        SemanticStaticDefinitionV1::Defined { initializer } => {
            writer.u8(0)?;
            writer.u32(initializer.0)?;
        }
        SemanticStaticDefinitionV1::ExternalRequired { symbol } => {
            writer.u8(1)?;
            writer.blob(&symbol.0)?;
        }
    }
    match &static_decl.export_symbol {
        Some(symbol) => {
            writer.u8(1)?;
            writer.blob(&symbol.0)
        }
        None => writer.u8(0),
    }
}

fn encode_vtable(
    writer: &mut CanonicalWriterV1,
    vtable: &SemanticVTableDeclV1,
) -> Result<(), SemanticMirErrorV1> {
    writer.identity(vtable.identity.0)?;
    writer.u32(vtable.concrete_type.0)?;
    writer.u32(vtable.dyn_type.0)?;
    writer.identity(vtable.trait_identity.primary_trait_ref.0)?;
    writer.count(vtable.trait_identity.dyn_predicates.len())?;
    for predicate in &vtable.trait_identity.dyn_predicates {
        writer.identity(predicate.0)?;
    }
    match vtable.header.drop_glue {
        Some(drop_glue) => {
            writer.u8(1)?;
            writer.u32(drop_glue.0)?;
        }
        None => writer.u8(0)?,
    }
    writer.u64(vtable.header.size_bytes)?;
    writer.u64(vtable.header.alignment_bytes)?;
    writer.count(vtable.slots.len())?;
    for slot in &vtable.slots {
        match slot {
            SemanticVTableSlotV1::Vacant => writer.u8(0)?,
            SemanticVTableSlotV1::Method(method) => {
                writer.u8(1)?;
                writer.u32(method.0)?;
            }
            SemanticVTableSlotV1::TraitVPtr { trait_ref, target } => {
                writer.u8(2)?;
                writer.identity(trait_ref.0)?;
                writer.u32(target.0)?;
            }
        }
    }
    writer.u32(vtable.allocation.0)
}

fn encode_function(
    writer: &mut CanonicalWriterV1,
    function: &SemanticFunctionDeclV1,
) -> Result<(), SemanticMirErrorV1> {
    writer.identity(function.identity.0)?;
    match function.role {
        SemanticFunctionRoleV1::KernelRoot => writer.u8(0)?,
        SemanticFunctionRoleV1::InternalHelper => writer.u8(1)?,
        SemanticFunctionRoleV1::DeviceFfiExport => writer.u8(2)?,
        SemanticFunctionRoleV1::DropGlue(dropped_type) => {
            writer.u8(3)?;
            writer.u32(dropped_type.0)?;
        }
    }
    match &function.export {
        Some(SemanticFunctionExportV1::Kernel(entry)) => {
            writer.u8(1)?;
            encode_kernel_entry(writer, entry)?;
        }
        Some(SemanticFunctionExportV1::DeviceFfi { export_symbol }) => {
            writer.u8(2)?;
            writer.blob(&export_symbol.0)?;
        }
        None => writer.u8(0)?,
    }
    writer.identity(function.item_definition_identity.0)?;
    writer.identity(function.monomorphization_identity.0)?;
    writer.identity(function.generic_type_arguments_identity.0)?;
    writer.identity(function.const_generic_arguments_identity.0)?;
    encode_source(writer, function.source)?;
    encode_abi(writer, &function.abi)?;
    writer.count(function.locals.len())?;
    for local in &function.locals {
        writer.identity(local.identity.0)?;
        writer.u32(local.ty.0)?;
        match local.role {
            SemanticLocalRoleV1::Return => writer.u8(0)?,
            SemanticLocalRoleV1::Argument(index) => {
                writer.u8(1)?;
                writer.u32(index)?;
            }
            SemanticLocalRoleV1::Temporary => writer.u8(2)?,
        }
        encode_source(writer, local.source)?;
    }
    writer.u32(function.entry.0)?;
    writer.count(function.blocks.len())?;
    for block in &function.blocks {
        writer.identity(block.identity.0)?;
        encode_source(writer, block.source)?;
        writer.count(block.statements.len())?;
        for statement in &block.statements {
            encode_source(writer, statement.source)?;
            encode_statement(writer, &statement.kind)?;
        }
        encode_source(writer, block.terminator.source)?;
        encode_terminator(writer, &block.terminator.kind)?;
    }
    Ok(())
}

fn encode_callable(
    writer: &mut CanonicalWriterV1,
    callable: &SemanticCallableDeclV1,
) -> Result<(), SemanticMirErrorV1> {
    match callable {
        SemanticCallableDeclV1::Defined { function } => {
            writer.u8(0)?;
            writer.u32(function.0)
        }
        SemanticCallableDeclV1::DeviceFfiImport { binding, contract } => {
            writer.u8(1)?;
            encode_non_body_callable_binding(writer, binding)?;
            writer.identity(contract.contract_identity.0)?;
            writer.blob(&contract.symbol.0)?;
            match contract.target {
                SemanticDeviceFfiTargetV1::AmdGpuGfx942XnackMinus => writer.u8(0)?,
            }
            match contract.code_object_version {
                SemanticCodeObjectVersionV1::V6 => writer.u8(0)?,
            }
            writer.identity(contract.physical_abi_identity.0)?;
            writer.u16(contract.effects.bits)?;
            writer.identity(contract.semantic_identity.0)
        }
        SemanticCallableDeclV1::CompilerIntrinsic {
            binding,
            operation,
            operation_identity,
        } => {
            writer.u8(2)?;
            encode_non_body_callable_binding(writer, binding)?;
            encode_compiler_intrinsic_operation(writer, *operation)?;
            writer.identity(operation_identity.0)
        }
    }
}

fn encode_non_body_callable_binding(
    writer: &mut CanonicalWriterV1,
    binding: &SemanticNonBodyCallableBindingV1,
) -> Result<(), SemanticMirErrorV1> {
    writer.identity(binding.identity.0)?;
    writer.identity(binding.item_definition_identity.0)?;
    writer.identity(binding.monomorphization_identity.0)?;
    writer.identity(binding.generic_type_arguments_identity.0)?;
    writer.identity(binding.const_generic_arguments_identity.0)?;
    encode_source(writer, binding.source)?;
    encode_abi(writer, &binding.abi)
}

fn encode_compiler_intrinsic_operation(
    writer: &mut CanonicalWriterV1,
    operation: SemanticCompilerIntrinsicOperationV1,
) -> Result<(), SemanticMirErrorV1> {
    match operation {
        SemanticCompilerIntrinsicOperationV1::ThreadIndex(axis) => {
            writer.u8(0)?;
            encode_axis(writer, axis)
        }
        SemanticCompilerIntrinsicOperationV1::WorkgroupIndex(axis) => {
            writer.u8(1)?;
            encode_axis(writer, axis)
        }
        SemanticCompilerIntrinsicOperationV1::WorkgroupDimension(axis) => {
            writer.u8(2)?;
            encode_axis(writer, axis)
        }
        SemanticCompilerIntrinsicOperationV1::GridDimension(axis) => {
            writer.u8(3)?;
            encode_axis(writer, axis)
        }
        SemanticCompilerIntrinsicOperationV1::WorkgroupBarrier => writer.u8(4),
        SemanticCompilerIntrinsicOperationV1::WaveBarrier => writer.u8(5),
        SemanticCompilerIntrinsicOperationV1::FabsF32 => writer.u8(6),
        SemanticCompilerIntrinsicOperationV1::ThreadIndex1d {
            index_witness,
            raw_index,
        } => {
            writer.u8(7)?;
            writer.u32(index_witness.0)?;
            writer.u32(raw_index.0)
        }
        SemanticCompilerIntrinsicOperationV1::ThreadIndexGet {
            index_witness,
            raw_index,
        } => {
            writer.u8(8)?;
            writer.u32(index_witness.0)?;
            writer.u32(raw_index.0)
        }
        SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMut {
            disjoint_slice,
            index_witness,
            element,
            raw_index,
        } => {
            writer.u8(9)?;
            writer.u32(disjoint_slice.0)?;
            writer.u32(index_witness.0)?;
            writer.u32(element.0)?;
            writer.u32(raw_index.0)
        }
    }
}

fn encode_axis(
    writer: &mut CanonicalWriterV1,
    axis: SemanticAxisV1,
) -> Result<(), SemanticMirErrorV1> {
    writer.u8(match axis {
        SemanticAxisV1::X => 0,
        SemanticAxisV1::Y => 1,
        SemanticAxisV1::Z => 2,
    })
}

fn encode_kernel_entry(
    writer: &mut CanonicalWriterV1,
    entry: &SemanticKernelEntryV1,
) -> Result<(), SemanticMirErrorV1> {
    writer.blob(&entry.export_symbol.0)?;
    writer.identity(entry.kernel_binding_identity.0)?;
    let contract = entry.source_contract;
    match contract.launch {
        Some(launch) => {
            writer.u8(1)?;
            encode_optional_workgroup_dimensions(writer, launch.required)?;
            encode_optional_workgroup_dimensions(writer, launch.maximum)?;
            match launch.min_workgroups_per_compute_unit {
                Some(count) => {
                    writer.u8(1)?;
                    writer.u16(count)?;
                }
                None => writer.u8(0)?,
            }
        }
        None => writer.u8(0)?,
    }
    match contract.unsafe_assembly {
        Some(assembly) => {
            writer.u8(1)?;
            match assembly.target {
                SemanticUnsafeAssemblyTargetV1::AmdGpuGfx942 => writer.u8(0)?,
            }
            writer.u16(assembly.operand_bits)?;
            writer.u16(assembly.option_bits)?;
            writer.u16(assembly.effect_bits)?;
        }
        None => writer.u8(0)?,
    }
    match contract.reachable_assembly {
        Some(reachable) => {
            writer.u8(1)?;
            writer.u32(reachable.blocks)?;
            writer.u16(reachable.operand_bits)?;
            writer.u16(reachable.option_bits)?;
            writer.u16(reachable.effect_bits)
        }
        None => writer.u8(0),
    }
}

fn encode_optional_workgroup_dimensions(
    writer: &mut CanonicalWriterV1,
    dimensions: Option<SemanticWorkgroupDimensionsV1>,
) -> Result<(), SemanticMirErrorV1> {
    let Some(dimensions) = dimensions else {
        return writer.u8(0);
    };
    writer.u8(1)?;
    for dimension in dimensions.dimensions {
        writer.u32(dimension)?;
    }
    Ok(())
}

fn encode_abi(
    writer: &mut CanonicalWriterV1,
    abi: &SemanticFunctionAbiV1,
) -> Result<(), SemanticMirErrorV1> {
    writer.identity(abi.identity.0)?;
    writer.identity(abi.layout_identity.0)?;
    encode_canon_abi(writer, abi.canon_abi)?;
    encode_extern_abi(writer, abi.extern_abi())?;
    writer.bool(abi.can_unwind)?;
    writer.bool(abi.c_variadic())?;
    writer.u32(abi.fixed_count)?;
    writer.count(abi.source_input_types().len())?;
    for source_type in abi.source_input_types() {
        writer.u32(source_type.0)?;
    }
    writer.u32(abi.source_output_type().0)?;
    writer.count(abi.arguments.len())?;
    for argument in &abi.arguments {
        match argument.role {
            SemanticAbiArgumentRoleV1::Source => writer.u8(0)?,
            SemanticAbiArgumentRoleV1::RustCallTupleField(field) => {
                writer.u8(1)?;
                writer.u32(field)?;
            }
            SemanticAbiArgumentRoleV1::Hidden(SemanticAbiHiddenArgumentRoleV1::CallerLocation) => {
                writer.u8(2)?
            }
        }
        encode_abi_value(writer, &argument.value)?;
    }
    encode_abi_value(writer, &abi.return_value)
}

fn encode_canon_abi(
    writer: &mut CanonicalWriterV1,
    canon_abi: SemanticCanonAbiV1,
) -> Result<(), SemanticMirErrorV1> {
    match canon_abi {
        SemanticCanonAbiV1::Rust => writer.u8(0),
        SemanticCanonAbiV1::C => writer.u8(1),
        SemanticCanonAbiV1::RustCold => writer.u8(2),
        SemanticCanonAbiV1::GpuKernel => writer.u8(3),
        SemanticCanonAbiV1::RustPreserveNone => writer.u8(4),
        SemanticCanonAbiV1::Custom => writer.u8(5),
        SemanticCanonAbiV1::Arm(call) => {
            writer.u8(6)?;
            writer.u8(match call {
                SemanticArmCallV1::Aapcs => 0,
                SemanticArmCallV1::CCmseNonSecureCall => 1,
                SemanticArmCallV1::CCmseNonSecureEntry => 2,
            })
        }
        SemanticCanonAbiV1::Interrupt(kind) => {
            writer.u8(7)?;
            writer.u8(match kind {
                SemanticInterruptKindV1::Avr => 0,
                SemanticInterruptKindV1::AvrNonBlocking => 1,
                SemanticInterruptKindV1::Msp430 => 2,
                SemanticInterruptKindV1::RiscvMachine => 3,
                SemanticInterruptKindV1::RiscvSupervisor => 4,
                SemanticInterruptKindV1::X86 => 5,
            })
        }
        SemanticCanonAbiV1::X86(call) => {
            writer.u8(8)?;
            writer.u8(match call {
                SemanticX86CallV1::Fastcall => 0,
                SemanticX86CallV1::Stdcall => 1,
                SemanticX86CallV1::SysV64 => 2,
                SemanticX86CallV1::Thiscall => 3,
                SemanticX86CallV1::Vectorcall => 4,
                SemanticX86CallV1::Win64 => 5,
            })
        }
    }
}

fn encode_extern_abi(
    writer: &mut CanonicalWriterV1,
    extern_abi: SemanticExternAbiV1,
) -> Result<(), SemanticMirErrorV1> {
    match extern_abi {
        SemanticExternAbiV1::C { unwind } => {
            writer.u8(0)?;
            writer.bool(unwind)
        }
        SemanticExternAbiV1::System { unwind } => {
            writer.u8(1)?;
            writer.bool(unwind)
        }
        SemanticExternAbiV1::Cdecl { unwind } => {
            writer.u8(2)?;
            writer.bool(unwind)
        }
        SemanticExternAbiV1::Rust => writer.u8(3),
        SemanticExternAbiV1::RustCall => writer.u8(4),
        SemanticExternAbiV1::RustCold => writer.u8(5),
        SemanticExternAbiV1::RustPreserveNone => writer.u8(6),
        SemanticExternAbiV1::Unadjusted => writer.u8(7),
        SemanticExternAbiV1::Custom => writer.u8(8),
        SemanticExternAbiV1::GpuKernel => writer.u8(9),
    }
}

fn encode_abi_value(
    writer: &mut CanonicalWriterV1,
    value: &SemanticAbiValueV1,
) -> Result<(), SemanticMirErrorV1> {
    writer.u32(value.source_ty.0)?;
    match value.adjusted() {
        Some(adjusted) => {
            writer.u8(1)?;
            writer.u32(adjusted.ty.0)?;
            writer.identity(adjusted.layout_identity.0)?;
            encode_type_layout(writer, &adjusted.layout)?;
        }
        None => writer.u8(0)?,
    }
    encode_optional_pointee_info(writer, value.pointee_override)?;
    match &value.mode {
        SemanticAbiPassModeV1::Ignore => writer.u8(0),
        SemanticAbiPassModeV1::Direct(attributes) => {
            writer.u8(1)?;
            encode_abi_attributes(writer, *attributes)
        }
        SemanticAbiPassModeV1::Pair { first, second } => {
            writer.u8(2)?;
            encode_abi_attributes(writer, *first)?;
            encode_abi_attributes(writer, *second)
        }
        SemanticAbiPassModeV1::Cast { pad_i32, cast } => {
            writer.u8(3)?;
            writer.bool(*pad_i32)?;
            for register in cast.prefix.iter() {
                encode_optional_register(writer, *register)?;
            }
            encode_optional_u64(writer, cast.rest_offset_bytes)?;
            encode_abi_register(writer, cast.rest.unit)?;
            writer.u64(cast.rest.total_bytes)?;
            writer.bool(cast.rest.consecutive)?;
            encode_abi_attributes(writer, cast.attributes)
        }
        SemanticAbiPassModeV1::Indirect {
            attributes,
            metadata_attributes,
            on_stack,
        } => {
            writer.u8(4)?;
            encode_abi_attributes(writer, *attributes)?;
            match metadata_attributes {
                Some(attributes) => {
                    writer.u8(1)?;
                    encode_abi_attributes(writer, *attributes)?;
                }
                None => writer.u8(0)?,
            }
            writer.bool(*on_stack)
        }
    }
}

fn encode_abi_attributes(
    writer: &mut CanonicalWriterV1,
    attributes: SemanticAbiValueAttributesV1,
) -> Result<(), SemanticMirErrorV1> {
    writer.u8(attributes.regular.rustc_bits())?;
    writer.u8(match attributes.extension {
        SemanticAbiExtensionV1::None => 0,
        SemanticAbiExtensionV1::ZeroExtend => 1,
        SemanticAbiExtensionV1::SignExtend => 2,
    })?;
    writer.u64(attributes.pointee_size_bytes)?;
    encode_optional_u64(writer, attributes.pointee_alignment_bytes)
}

fn encode_optional_register(
    writer: &mut CanonicalWriterV1,
    register: Option<SemanticAbiRegisterV1>,
) -> Result<(), SemanticMirErrorV1> {
    match register {
        Some(register) => {
            writer.u8(1)?;
            encode_abi_register(writer, register)
        }
        None => writer.u8(0),
    }
}

fn encode_abi_register(
    writer: &mut CanonicalWriterV1,
    register: SemanticAbiRegisterV1,
) -> Result<(), SemanticMirErrorV1> {
    writer.u8(match register.kind {
        SemanticAbiRegisterKindV1::Integer => 0,
        SemanticAbiRegisterKindV1::Float => 1,
        SemanticAbiRegisterKindV1::Vector => 2,
    })?;
    writer.u64(register.size_bytes)
}

fn encode_optional_u64(
    writer: &mut CanonicalWriterV1,
    value: Option<u64>,
) -> Result<(), SemanticMirErrorV1> {
    match value {
        Some(value) => {
            writer.u8(1)?;
            writer.u64(value)
        }
        None => writer.u8(0),
    }
}

fn encode_statement(
    writer: &mut CanonicalWriterV1,
    statement: &SemanticStatementKindV1,
) -> Result<(), SemanticMirErrorV1> {
    match statement {
        SemanticStatementKindV1::Assign(assignment) => {
            writer.u8(0)?;
            encode_place(writer, &assignment.destination)?;
            encode_rvalue(writer, &assignment.value)
        }
        SemanticStatementKindV1::Store(store) => {
            writer.u8(1)?;
            encode_place(writer, &store.destination)?;
            encode_operand(writer, &store.value)?;
            encode_volatility(writer, store.volatility)?;
            encode_atomic_access_opt(writer, store.atomic)
        }
        SemanticStatementKindV1::AtomicRmw(operation) => {
            writer.u8(2)?;
            encode_place(writer, &operation.destination)?;
            encode_place(writer, &operation.address)?;
            encode_operand(writer, &operation.value)?;
            writer.u8(match operation.operation {
                SemanticAtomicRmwOpV1::Exchange => 0,
                SemanticAtomicRmwOpV1::Add => 1,
                SemanticAtomicRmwOpV1::Subtract => 2,
                SemanticAtomicRmwOpV1::BitAnd => 3,
                SemanticAtomicRmwOpV1::BitNand => 4,
                SemanticAtomicRmwOpV1::BitOr => 5,
                SemanticAtomicRmwOpV1::BitXor => 6,
                SemanticAtomicRmwOpV1::SignedMaximum => 7,
                SemanticAtomicRmwOpV1::SignedMinimum => 8,
                SemanticAtomicRmwOpV1::UnsignedMaximum => 9,
                SemanticAtomicRmwOpV1::UnsignedMinimum => 10,
            })?;
            encode_atomic_access(writer, operation.access)
        }
        SemanticStatementKindV1::AtomicCompareExchange(operation) => {
            writer.u8(3)?;
            encode_place(writer, &operation.destination)?;
            encode_place(writer, &operation.address)?;
            encode_operand(writer, &operation.expected)?;
            encode_operand(writer, &operation.replacement)?;
            encode_atomic_access(writer, operation.success)?;
            encode_atomic_ordering(writer, operation.failure_ordering)?;
            writer.bool(operation.weak)
        }
        SemanticStatementKindV1::SetDiscriminant {
            place,
            variant_index,
        } => {
            writer.u8(4)?;
            encode_place(writer, place)?;
            writer.u32(*variant_index)
        }
        SemanticStatementKindV1::Deinitialize(place) => {
            writer.u8(5)?;
            encode_place(writer, place)
        }
        SemanticStatementKindV1::StorageLive(local) => {
            writer.u8(6)?;
            writer.u32(local.0)
        }
        SemanticStatementKindV1::StorageDead(local) => {
            writer.u8(7)?;
            writer.u32(local.0)
        }
        SemanticStatementKindV1::Nop => writer.u8(8),
    }
}

fn encode_place(
    writer: &mut CanonicalWriterV1,
    place: &SemanticPlaceV1,
) -> Result<(), SemanticMirErrorV1> {
    writer.u32(place.local.0)?;
    writer.count(place.projections.len())?;
    for projection in &place.projections {
        match projection.kind {
            SemanticProjectionKindV1::Dereference => writer.u8(0)?,
            SemanticProjectionKindV1::Field(index) => {
                writer.u8(1)?;
                writer.u32(index)?;
            }
            SemanticProjectionKindV1::Index(local) => {
                writer.u8(2)?;
                writer.u32(local.0)?;
            }
            SemanticProjectionKindV1::ConstantIndex {
                offset,
                minimum_length,
                from_end,
            } => {
                writer.u8(3)?;
                writer.u64(offset)?;
                writer.u64(minimum_length)?;
                writer.bool(from_end)?;
            }
            SemanticProjectionKindV1::Subslice { from, to, from_end } => {
                writer.u8(4)?;
                writer.u64(from)?;
                writer.u64(to)?;
                writer.bool(from_end)?;
            }
            SemanticProjectionKindV1::Downcast(index) => {
                writer.u8(5)?;
                writer.u32(index)?;
            }
            SemanticProjectionKindV1::OpaqueCast => writer.u8(6)?,
            SemanticProjectionKindV1::Subtype => writer.u8(7)?,
        }
        writer.u32(projection.result_type.0)?;
    }
    writer.u32(place.ty.0)
}

fn encode_operand(
    writer: &mut CanonicalWriterV1,
    operand: &SemanticOperandV1,
) -> Result<(), SemanticMirErrorV1> {
    match operand {
        SemanticOperandV1::Copy(place) => {
            writer.u8(0)?;
            encode_place(writer, place)
        }
        SemanticOperandV1::Move(place) => {
            writer.u8(1)?;
            encode_place(writer, place)
        }
        SemanticOperandV1::Constant(constant) => {
            writer.u8(2)?;
            encode_constant(writer, constant)
        }
    }
}

fn encode_constant(
    writer: &mut CanonicalWriterV1,
    constant: &SemanticConstantV1,
) -> Result<(), SemanticMirErrorV1> {
    writer.u32(constant.ty.0)?;
    match &constant.value {
        SemanticConstantValueV1::ZeroSized => writer.u8(0),
        SemanticConstantValueV1::Scalar(value) => {
            writer.u8(1)?;
            writer.u128(value.bits)?;
            writer.u8(value.size_bytes)
        }
        SemanticConstantValueV1::Bytes(bytes) => {
            writer.u8(2)?;
            writer.blob(&bytes.0)
        }
        SemanticConstantValueV1::Pointer(pointer) => {
            writer.u8(3)?;
            writer.u64(pointer.byte_offset)?;
            encode_pointer_provenance(writer, pointer.provenance)?;
            match pointer.metadata {
                SemanticPointerValueMetadataV1::None => writer.u8(0),
                SemanticPointerValueMetadataV1::SliceLength(length) => {
                    writer.u8(1)?;
                    writer.u64(length)
                }
                SemanticPointerValueMetadataV1::VTable(vtable) => {
                    writer.u8(2)?;
                    writer.u32(vtable.0)
                }
            }
        }
        SemanticConstantValueV1::Callable(function) => {
            writer.u8(4)?;
            writer.u32(function.0)
        }
    }
}

fn encode_pointer_provenance(
    writer: &mut CanonicalWriterV1,
    provenance: SemanticPointerProvenanceV1,
) -> Result<(), SemanticMirErrorV1> {
    match provenance {
        SemanticPointerProvenanceV1::Allocation(allocation) => {
            writer.u8(0)?;
            writer.u32(allocation.0)
        }
        SemanticPointerProvenanceV1::Callable(function) => {
            writer.u8(1)?;
            writer.u32(function.0)
        }
        SemanticPointerProvenanceV1::Static(static_id) => {
            writer.u8(2)?;
            writer.u32(static_id.0)
        }
        SemanticPointerProvenanceV1::ExposedAddress => writer.u8(3),
    }
}

fn encode_rvalue(
    writer: &mut CanonicalWriterV1,
    rvalue: &SemanticRvalueV1,
) -> Result<(), SemanticMirErrorV1> {
    writer.u32(rvalue.result_type.0)?;
    match &rvalue.kind {
        SemanticRvalueKindV1::Use(operand) => {
            writer.u8(0)?;
            encode_operand(writer, operand)
        }
        SemanticRvalueKindV1::Unary { operation, operand } => {
            writer.u8(1)?;
            writer.u8(match operation {
                SemanticUnaryOpV1::Not => 0,
                SemanticUnaryOpV1::Negate => 1,
                SemanticUnaryOpV1::PointerMetadata => 2,
            })?;
            encode_operand(writer, operand)
        }
        SemanticRvalueKindV1::Binary {
            operation,
            left,
            right,
        } => {
            writer.u8(2)?;
            encode_binary_op(writer, *operation)?;
            encode_operand(writer, left)?;
            encode_operand(writer, right)
        }
        SemanticRvalueKindV1::Cast { kind, operand } => {
            writer.u8(3)?;
            writer.u8(match kind {
                SemanticCastKindV1::Integer => 0,
                SemanticCastKindV1::Float => 1,
                SemanticCastKindV1::Pointer => 2,
                SemanticCastKindV1::PointerExposeProvenance => 3,
                SemanticCastKindV1::PointerWithExposedProvenance => 4,
                SemanticCastKindV1::Transmute => 5,
            })?;
            encode_operand(writer, operand)
        }
        SemanticRvalueKindV1::Borrow { kind, place } => {
            writer.u8(4)?;
            writer.u8(match kind {
                SemanticBorrowKindV1::Shared => 0,
                SemanticBorrowKindV1::Mutable => 1,
                SemanticBorrowKindV1::Fake => 2,
            })?;
            encode_place(writer, place)
        }
        SemanticRvalueKindV1::AddressOf { mutability, place } => {
            writer.u8(5)?;
            encode_mutability(writer, *mutability)?;
            encode_place(writer, place)
        }
        SemanticRvalueKindV1::Length(place) => {
            writer.u8(6)?;
            encode_place(writer, place)
        }
        SemanticRvalueKindV1::Discriminant(place) => {
            writer.u8(7)?;
            encode_place(writer, place)
        }
        SemanticRvalueKindV1::Aggregate(aggregate) => {
            writer.u8(8)?;
            match aggregate.kind {
                SemanticAggregateKindV1::Array => writer.u8(0)?,
                SemanticAggregateKindV1::Tuple => writer.u8(1)?,
                SemanticAggregateKindV1::Aggregate => writer.u8(2)?,
                SemanticAggregateKindV1::EnumVariant(variant) => {
                    writer.u8(3)?;
                    writer.u32(variant)?;
                }
            }
            writer.count(aggregate.operands.len())?;
            for operand in &aggregate.operands {
                encode_operand(writer, operand)?;
            }
            Ok(())
        }
        SemanticRvalueKindV1::Load(load) => {
            writer.u8(9)?;
            encode_place(writer, &load.source)?;
            encode_volatility(writer, load.volatility)?;
            encode_atomic_access_opt(writer, load.atomic)
        }
    }
}

fn encode_binary_op(
    writer: &mut CanonicalWriterV1,
    operation: SemanticBinaryOpV1,
) -> Result<(), SemanticMirErrorV1> {
    writer.u8(match operation {
        SemanticBinaryOpV1::Add => 0,
        SemanticBinaryOpV1::Subtract => 1,
        SemanticBinaryOpV1::Multiply => 2,
        SemanticBinaryOpV1::Divide => 3,
        SemanticBinaryOpV1::Remainder => 4,
        SemanticBinaryOpV1::BitXor => 5,
        SemanticBinaryOpV1::BitAnd => 6,
        SemanticBinaryOpV1::BitOr => 7,
        SemanticBinaryOpV1::ShiftLeft => 8,
        SemanticBinaryOpV1::ShiftRight => 9,
        SemanticBinaryOpV1::Equal => 10,
        SemanticBinaryOpV1::LessThan => 11,
        SemanticBinaryOpV1::LessOrEqual => 12,
        SemanticBinaryOpV1::NotEqual => 13,
        SemanticBinaryOpV1::GreaterOrEqual => 14,
        SemanticBinaryOpV1::GreaterThan => 15,
        SemanticBinaryOpV1::Offset => 16,
    })
}

fn encode_volatility(
    writer: &mut CanonicalWriterV1,
    volatility: SemanticVolatilityV1,
) -> Result<(), SemanticMirErrorV1> {
    writer.u8(match volatility {
        SemanticVolatilityV1::NonVolatile => 0,
        SemanticVolatilityV1::Volatile => 1,
    })
}

fn encode_atomic_access_opt(
    writer: &mut CanonicalWriterV1,
    access: Option<SemanticAtomicAccessV1>,
) -> Result<(), SemanticMirErrorV1> {
    let Some(access) = access else {
        return writer.u8(0);
    };
    writer.u8(1)?;
    encode_atomic_access(writer, access)
}

fn encode_atomic_access(
    writer: &mut CanonicalWriterV1,
    access: SemanticAtomicAccessV1,
) -> Result<(), SemanticMirErrorV1> {
    encode_atomic_ordering(writer, access.ordering)?;
    writer.u8(match access.scope {
        SemanticAtomicScopeV1::SingleThread => 0,
        SemanticAtomicScopeV1::Workgroup => 1,
        SemanticAtomicScopeV1::Agent => 2,
        SemanticAtomicScopeV1::Device => 3,
        SemanticAtomicScopeV1::System => 4,
    })
}

fn encode_atomic_ordering(
    writer: &mut CanonicalWriterV1,
    ordering: SemanticAtomicOrderingV1,
) -> Result<(), SemanticMirErrorV1> {
    writer.u8(match ordering {
        SemanticAtomicOrderingV1::Relaxed => 0,
        SemanticAtomicOrderingV1::Release => 1,
        SemanticAtomicOrderingV1::Acquire => 2,
        SemanticAtomicOrderingV1::AcquireRelease => 3,
        SemanticAtomicOrderingV1::SequentiallyConsistent => 4,
    })
}

fn encode_terminator(
    writer: &mut CanonicalWriterV1,
    terminator: &SemanticTerminatorKindV1,
) -> Result<(), SemanticMirErrorV1> {
    match terminator {
        SemanticTerminatorKindV1::Goto(edge) => {
            writer.u8(0)?;
            encode_edge(writer, *edge)
        }
        SemanticTerminatorKindV1::SwitchInt {
            discriminant,
            targets,
        } => {
            writer.u8(1)?;
            encode_operand(writer, discriminant)?;
            writer.count(targets.values.len())?;
            for target in &targets.values {
                writer.u128(target.value)?;
                encode_edge(writer, target.edge)?;
            }
            encode_edge(writer, targets.otherwise)
        }
        SemanticTerminatorKindV1::Call(call) => {
            writer.u8(2)?;
            writer.u32(call.callee.0)?;
            encode_operands(writer, &call.arguments)?;
            writer.count(call.variadic_argument_abis.len())?;
            for argument_abi in &call.variadic_argument_abis {
                encode_abi_value(writer, argument_abi)?;
            }
            match &call.destination {
                Some(destination) => {
                    writer.u8(1)?;
                    encode_place(writer, &destination.place)?;
                    encode_edge(writer, destination.edge)?;
                }
                None => writer.u8(0)?,
            }
            encode_unwind(writer, call.unwind)
        }
        SemanticTerminatorKindV1::TailCall(call) => {
            writer.u8(3)?;
            writer.u32(call.callee.0)?;
            encode_operands(writer, &call.arguments)?;
            encode_unwind(writer, call.unwind)
        }
        SemanticTerminatorKindV1::Drop {
            place,
            drop_glue,
            target,
            unwind,
        } => {
            writer.u8(4)?;
            encode_place(writer, place)?;
            writer.u32(drop_glue.0)?;
            encode_edge(writer, *target)?;
            encode_unwind(writer, *unwind)
        }
        SemanticTerminatorKindV1::Assert {
            condition,
            expected,
            message,
            target,
            unwind,
        } => {
            writer.u8(5)?;
            encode_operand(writer, condition)?;
            writer.bool(*expected)?;
            encode_assert_message(writer, message)?;
            encode_edge(writer, *target)?;
            encode_unwind(writer, *unwind)
        }
        SemanticTerminatorKindV1::FalseEdge {
            real_target,
            imaginary_target,
        } => {
            writer.u8(6)?;
            encode_edge(writer, *real_target)?;
            encode_edge(writer, *imaginary_target)
        }
        SemanticTerminatorKindV1::Return => writer.u8(7),
        SemanticTerminatorKindV1::UnwindResume => writer.u8(8),
        SemanticTerminatorKindV1::UnwindTerminate => writer.u8(9),
        SemanticTerminatorKindV1::Abort => writer.u8(10),
        SemanticTerminatorKindV1::Unreachable => writer.u8(11),
    }
}

fn encode_operands(
    writer: &mut CanonicalWriterV1,
    operands: &[SemanticOperandV1],
) -> Result<(), SemanticMirErrorV1> {
    writer.count(operands.len())?;
    for operand in operands {
        encode_operand(writer, operand)?;
    }
    Ok(())
}

fn encode_edge(
    writer: &mut CanonicalWriterV1,
    edge: SemanticControlFlowEdgeV1,
) -> Result<(), SemanticMirErrorV1> {
    writer.u8(match edge.role {
        SemanticEdgeRoleV1::Goto => 0,
        SemanticEdgeRoleV1::SwitchValue => 1,
        SemanticEdgeRoleV1::SwitchOtherwise => 2,
        SemanticEdgeRoleV1::CallReturn => 3,
        SemanticEdgeRoleV1::CallUnwind => 4,
        SemanticEdgeRoleV1::TailCallUnwind => 5,
        SemanticEdgeRoleV1::DropReturn => 6,
        SemanticEdgeRoleV1::DropUnwind => 7,
        SemanticEdgeRoleV1::AssertSuccess => 8,
        SemanticEdgeRoleV1::AssertUnwind => 9,
        SemanticEdgeRoleV1::FalseEdgeReal => 10,
        SemanticEdgeRoleV1::FalseEdgeImaginary => 11,
    })?;
    writer.u32(edge.target.0)
}

fn encode_unwind(
    writer: &mut CanonicalWriterV1,
    unwind: SemanticUnwindActionV1,
) -> Result<(), SemanticMirErrorV1> {
    match unwind {
        SemanticUnwindActionV1::Continue => writer.u8(0),
        SemanticUnwindActionV1::Unreachable => writer.u8(1),
        SemanticUnwindActionV1::Terminate => writer.u8(2),
        SemanticUnwindActionV1::Cleanup(edge) => {
            writer.u8(3)?;
            encode_edge(writer, edge)
        }
    }
}

fn encode_assert_message(
    writer: &mut CanonicalWriterV1,
    message: &SemanticAssertMessageV1,
) -> Result<(), SemanticMirErrorV1> {
    match message {
        SemanticAssertMessageV1::BoundsCheck { length, index } => {
            writer.u8(0)?;
            encode_operand(writer, length)?;
            encode_operand(writer, index)
        }
        SemanticAssertMessageV1::Overflow {
            operation,
            left,
            right,
        } => {
            writer.u8(1)?;
            encode_binary_op(writer, *operation)?;
            encode_operand(writer, left)?;
            encode_operand(writer, right)
        }
        SemanticAssertMessageV1::DivisionByZero(operand) => {
            writer.u8(2)?;
            encode_operand(writer, operand)
        }
        SemanticAssertMessageV1::RemainderByZero(operand) => {
            writer.u8(3)?;
            encode_operand(writer, operand)
        }
        SemanticAssertMessageV1::MisalignedPointerDereference {
            required_alignment,
            found_alignment,
        } => {
            writer.u8(4)?;
            encode_operand(writer, required_alignment)?;
            encode_operand(writer, found_alignment)
        }
        SemanticAssertMessageV1::NullPointerDereference => writer.u8(5),
        SemanticAssertMessageV1::ResumedAfterReturn => writer.u8(6),
        SemanticAssertMessageV1::ResumedAfterPanic => writer.u8(7),
    }
}

#[cfg(test)]
mod private_tests {
    use super::*;

    #[test]
    fn aggregate_counter_overflow_is_typed() {
        let mut totals = ValidationTotalsV1 {
            operands: u64::MAX,
            ..ValidationTotalsV1::default()
        };
        assert_eq!(
            totals.charge(
                SemanticMirResourceV1::Operands,
                1,
                SemanticMirLimitsV1::default(),
            ),
            Err(SemanticMirErrorV1::ArithmeticOverflow {
                resource: SemanticMirResourceV1::Operands,
            })
        );
    }

    #[test]
    fn compare_exchange_failure_order_is_a_partial_order() {
        assert!(!compare_exchange_failure_allowed(
            SemanticAtomicOrderingV1::Release,
            SemanticAtomicOrderingV1::Acquire,
        ));
        assert!(compare_exchange_failure_allowed(
            SemanticAtomicOrderingV1::AcquireRelease,
            SemanticAtomicOrderingV1::Acquire,
        ));
    }
}
