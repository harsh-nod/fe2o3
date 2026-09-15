//! Closed global-backed BF16 lane reads. This contract preserves nominal source
//! types and memory provenance; it is not matrix arithmetic or numerical proof.

use super::*;

impl InertSemanticMirRequestV1 {
    pub fn admit_exact_v22(
        self,
        limits: SemanticMirLimitsV1,
    ) -> Result<AdmittedInertSemanticMirV1, SemanticMirErrorV1> {
        self.admit_for_wire_version(SemanticMirWireVersionV1::V22, limits)
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticGlobalBf16MatrixTypesV1 {
    pub matrix: SemanticTypeIdV1,
    pub global: SemanticTypeIdV1,
    pub lane: SemanticTypeIdV1,
    pub fragment: SemanticTypeIdV1,
    pub element: SemanticTypeIdV1,
    pub index: SemanticTypeIdV1,
}

impl SemanticGlobalBf16MatrixTypesV1 {
    pub const fn all(self) -> [SemanticTypeIdV1; 6] {
        [
            self.matrix,
            self.global,
            self.lane,
            self.fragment,
            self.element,
            self.index,
        ]
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticGlobalBf16MatrixLoadV1 {
    types: SemanticGlobalBf16MatrixTypesV1,
    operand: SemanticMfmaOperandContractV1,
    matrix_brand: SemanticTypeIdentityV1,
    global_brand: SemanticTypeIdentityV1,
    provenance: SemanticKernelCapabilityProvenanceV1,
    source_identity: SemanticFunctionIdentityV1,
}

impl SemanticGlobalBf16MatrixLoadV1 {
    pub fn new(
        types: SemanticGlobalBf16MatrixTypesV1,
        operand: SemanticMfmaOperandContractV1,
        matrix_brand: SemanticTypeIdentityV1,
        global_brand: SemanticTypeIdentityV1,
        provenance: SemanticKernelCapabilityProvenanceV1,
        source_identity: SemanticFunctionIdentityV1,
    ) -> Result<Self, SemanticMirErrorV1> {
        if operand.profile != SemanticMfmaProfileV1::Bf16F32M16N16K16
            || operand.register_distribution != SemanticMfmaRegisterDistributionV1::Tile16x16
            || operand.wave_width != 64
            || matrix_brand.as_bytes() == &[0; 32]
            || global_brand.as_bytes() == &[0; 32]
            || source_identity.as_bytes() == &[0; 32]
            || types
                .all()
                .iter()
                .enumerate()
                .any(|(i, ty)| types.all()[..i].contains(ty))
        {
            return Err(SemanticMirErrorV1::InvalidFunctionAbi);
        }
        Ok(Self {
            types,
            operand,
            matrix_brand,
            global_brand,
            provenance,
            source_identity,
        })
    }

    pub const fn types(self) -> SemanticGlobalBf16MatrixTypesV1 {
        self.types
    }
    pub const fn operand(self) -> SemanticMfmaOperandContractV1 {
        self.operand
    }
    pub const fn storage_layout(self) -> SemanticMfmaStorageLayoutV1 {
        SemanticMfmaStorageLayoutV1::RowMajor
    }
    pub const fn matrix_brand(self) -> SemanticTypeIdentityV1 {
        self.matrix_brand
    }
    pub const fn global_brand(self) -> SemanticTypeIdentityV1 {
        self.global_brand
    }
    pub const fn provenance(self) -> SemanticKernelCapabilityProvenanceV1 {
        self.provenance
    }
    pub const fn source_identity(self) -> SemanticFunctionIdentityV1 {
        self.source_identity
    }
    pub const fn memory(self) -> SemanticCapabilityMemoryContractV1 {
        SemanticCapabilityMemoryContractV1::global_read_only()
    }
}

pub(super) fn record_claim(
    claims: &mut IntrinsicCapabilityClaimsV1,
    contract: SemanticGlobalBf16MatrixLoadV1,
) -> bool {
    // A read cannot create its own memory binding. Admission requires the
    // matching actual global bind among this source's compiler intrinsics.
    claims.record_memory_access(
        contract.types.global,
        BoundCapabilityMemoryViewV1 {
            element: contract.types.element,
            contract: contract.memory(),
            provenance: contract.provenance,
        },
    )
}

pub(super) fn abi_matches(
    request: &InertSemanticMirRequestV1,
    abi: &SemanticFunctionAbiV1,
    contract: SemanticGlobalBf16MatrixLoadV1,
) -> bool {
    let t = contract.types;
    let inputs = abi.source_input_types();
    if inputs.len() != 4
        || abi.source_output_type() != t.fragment
        || !capability_memory_source_ownership_matches(
            abi,
            &[
                SemanticSourceArgumentOwnershipV1::SharedBorrow,
                SemanticSourceArgumentOwnershipV1::SharedBorrow,
                SemanticSourceArgumentOwnershipV1::ByValue,
                SemanticSourceArgumentOwnershipV1::ByValue,
            ],
        )
        || !shared_reference_to(request, inputs[0], t.matrix)
        || !shared_reference_to(request, inputs[1], t.lane)
        || inputs[2..] != [t.index, t.index]
        || !is_unsigned_integer_with_bits(request, t.index, 64)
        || !is_unsigned_integer_with_bits(request, t.element, 16)
        || !kernel_capability_provenance_matches(request, contract.provenance)
        || !semantic_global_bf16_matrix_layout_matches_v1(&request.types, t)
    {
        return false;
    }
    true
}

/// Runtime payload is one shared Global borrow and four usize coordinates.
/// Source-order marker fields remain present and must be inhabited ZSTs.
pub fn semantic_global_bf16_matrix_layout_matches_v1(
    types: &[SemanticTypeDeclV1],
    t: SemanticGlobalBf16MatrixTypesV1,
) -> bool {
    if !t
        .all()
        .iter()
        .all(|ty| types.get(ty.index() as usize).is_some())
        || !matches!(
            types[t.element.index() as usize].shape(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 16
            })
        )
        || !matches!(
            types[t.index.index() as usize].shape(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 64
            })
        )
    {
        return false;
    }
    let Some(matrix) = types.get(t.matrix.index() as usize) else {
        return false;
    };
    let SemanticTypeShapeV1::Aggregate(fields) = matrix.shape() else {
        return false;
    };
    let SemanticTypeLayoutDetailsV1::Aggregate(layout) = matrix.layout().details() else {
        return false;
    };
    let [storage, offset, rows, columns, stride, markers @ ..] = fields.fields() else {
        return false;
    };
    let Some(storage) = types.get(storage.index() as usize) else {
        return false;
    };
    let SemanticTypeShapeV1::Pointer(pointer) = storage.shape() else {
        return false;
    };
    pointer.kind() == SemanticPointerKindV1::Reference
        && pointer.mutability() == SemanticMutabilityV1::Immutable
        && pointer.pointee() == t.global
        && pointer.metadata() == SemanticPointerMetadataV1::None
        && pointer.address_space() == 0
        && pointer.pointer_width_bits() == 64
        && [*offset, *rows, *columns, *stride] == [t.index; 4]
        && markers.len() == 3
        && markers.iter().all(|marker| {
            types.get(marker.index() as usize).is_some_and(|ty| {
                ty.layout().size_bytes() == Some(0) && !ty.layout().is_uninhabited()
            })
        })
        && fields.fields().len() == layout.field_offsets().len()
        && !matrix.layout().is_uninhabited()
}

pub(super) fn encode(
    writer: &mut CanonicalWriterV1,
    contract: SemanticGlobalBf16MatrixLoadV1,
) -> Result<(), SemanticMirErrorV1> {
    for ty in contract.types.all() {
        writer.u32(ty.index())?;
    }
    encode_mfma_operand_contract(writer, contract.operand)?;
    encode_mfma_storage_layout(writer, contract.storage_layout())?;
    writer.identity(contract.matrix_brand.0)?;
    writer.identity(contract.global_brand.0)?;
    encode_capability_memory_contract(writer, contract.memory())?;
    encode_kernel_capability_provenance(writer, contract.provenance)?;
    writer.identity(contract.source_identity.0)
}
