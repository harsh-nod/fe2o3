//! Borrowed Workgroup source types and retained defined epoch projections.
//! These records never issue Workgroup SSA authority or authenticate a provider.

use super::*;

const MAX_EPOCH_BODY_BYTES: u64 = 16 * 1024;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticWorkgroupEpochProjectionTypesV1 {
    pub reference: SemanticTypeIdV1,
    pub workgroup: SemanticTypeIdV1,
    pub epoch_reference: SemanticTypeIdV1,
    pub epoch_type: SemanticTypeIdV1,
}

impl SemanticWorkgroupEpochProjectionTypesV1 {
    pub const fn new(ids: [SemanticTypeIdV1; 4]) -> Self {
        let [reference, workgroup, epoch_reference, epoch_type] = ids;
        Self {
            reference,
            workgroup,
            epoch_reference,
            epoch_type,
        }
    }

    pub const fn all(self) -> [SemanticTypeIdV1; 4] {
        [
            self.reference,
            self.workgroup,
            self.epoch_reference,
            self.epoch_type,
        ]
    }
}

/// A recipe bound to one retained defined function, not a substitute terminal.
/// Source authentication and each caller's owned SSA origin remain required.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticWorkgroupEpochProjectionV1 {
    function: SemanticFunctionIdV1,
    source_identity: SemanticFunctionIdentityV1,
    body_identity: [u8; 32],
    types: SemanticWorkgroupEpochProjectionTypesV1,
    provenance: SemanticKernelCapabilityProvenanceV1,
    brand: SemanticTypeIdentityV1,
    epoch: SemanticTypeIdentityV1,
}

impl SemanticWorkgroupEpochProjectionV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn for_defined_function(
        function: SemanticFunctionIdV1,
        body: &SemanticFunctionDeclV1,
        types: SemanticWorkgroupEpochProjectionTypesV1,
        provenance: SemanticKernelCapabilityProvenanceV1,
        brand: SemanticTypeIdentityV1,
        epoch: SemanticTypeIdentityV1,
    ) -> Result<Self, SemanticMirErrorV1> {
        if !body_matches(body, types) {
            return Err(SemanticMirErrorV1::InvalidFunctionAbi);
        }
        let (body_identity, _) = body_digest(body, MAX_EPOCH_BODY_BYTES)?;
        Self::from_encoded_parts(
            function,
            body.identity(),
            body_identity,
            types,
            provenance,
            brand,
            epoch,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn from_encoded_parts(
        function: SemanticFunctionIdV1,
        source_identity: SemanticFunctionIdentityV1,
        body_identity: [u8; 32],
        types: SemanticWorkgroupEpochProjectionTypesV1,
        provenance: SemanticKernelCapabilityProvenanceV1,
        brand: SemanticTypeIdentityV1,
        epoch: SemanticTypeIdentityV1,
    ) -> Result<Self, SemanticMirErrorV1> {
        if u64::from(function.index()) >= HARD_MAX_FUNCTIONS_V1
            || source_identity.as_bytes() == &[0; 32]
            || body_identity == [0; 32]
            || brand.as_bytes() == &[0; 32]
            || epoch.as_bytes() == &[0; 32]
            || brand == epoch
            || !distinct_type_ids(&types.all())
        {
            return Err(SemanticMirErrorV1::InvalidFunctionAbi);
        }
        Ok(Self {
            function,
            source_identity,
            body_identity,
            types,
            provenance,
            brand,
            epoch,
        })
    }

    pub const fn function(self) -> SemanticFunctionIdV1 {
        self.function
    }
    pub const fn source_identity(self) -> SemanticFunctionIdentityV1 {
        self.source_identity
    }
    pub const fn body_identity(&self) -> &[u8; 32] {
        &self.body_identity
    }
    pub const fn types(self) -> SemanticWorkgroupEpochProjectionTypesV1 {
        self.types
    }
    pub const fn provenance(self) -> SemanticKernelCapabilityProvenanceV1 {
        self.provenance
    }
    pub const fn brand(self) -> SemanticTypeIdentityV1 {
        self.brand
    }
    pub const fn epoch(self) -> SemanticTypeIdentityV1 {
        self.epoch
    }
    pub const fn receiver_argument(self) -> u32 {
        0
    }
    pub const fn source_field(self) -> u32 {
        2
    }

    pub fn projection(self) -> [SemanticProjectionV1; 2] {
        [
            SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, self.types.workgroup)
                .expect("fixed dereference projection"),
            SemanticProjectionV1::new(SemanticProjectionKindV1::Field(2), self.types.epoch_type)
                .expect("fixed epoch field projection"),
        ]
    }

    pub(super) fn encode(self, writer: &mut CanonicalWriterV1) -> Result<(), SemanticMirErrorV1> {
        writer.u8(0)?;
        writer.u32(self.function.index())?;
        writer.identity(*self.source_identity.as_bytes())?;
        writer.identity(self.body_identity)?;
        for id in self.types.all() {
            writer.u32(id.index())?;
        }
        encode_kernel_capability_provenance(writer, self.provenance)?;
        writer.identity(*self.brand.as_bytes())?;
        writer.identity(*self.epoch.as_bytes())?;
        writer.u8(self.receiver_argument() as u8)?;
        writer.u32(self.source_field())
    }
}

impl SemanticFunctionDeclV1 {
    pub fn with_workgroup_epoch_projection(
        self,
        record: SemanticWorkgroupEpochProjectionV1,
    ) -> Result<Self, SemanticMirErrorV1> {
        self.with_defined_capability_contract(
            SemanticDefinedCapabilityContractV1::WorkgroupEpochProjection(record),
        )
    }

    pub const fn workgroup_epoch_projection(&self) -> Option<&SemanticWorkgroupEpochProjectionV1> {
        match self.defined_capability_contract() {
            Some(SemanticDefinedCapabilityContractV1::WorkgroupEpochProjection(record)) => {
                Some(record)
            }
            Some(SemanticDefinedCapabilityContractV1::KernelMathDerive(_))
            | Some(SemanticDefinedCapabilityContractV1::PolicyMathBind(_))
            | Some(SemanticDefinedCapabilityContractV1::PolicyMatrixBind(_))
            | Some(SemanticDefinedCapabilityContractV1::KernelMatrixDerive(_))
            | Some(SemanticDefinedCapabilityContractV1::ReusableLdsConversion(_))
            | Some(SemanticDefinedCapabilityContractV1::GuardedGridLeader(_))
            | Some(SemanticDefinedCapabilityContractV1::ReusablePhase(_))
            | Some(SemanticDefinedCapabilityContractV1::PolicyGfx950Narrow(_))
            | None => None,
        }
    }
}

pub(super) fn validate_attachment(
    body: &SemanticFunctionDeclV1,
    record: SemanticWorkgroupEpochProjectionV1,
) -> Result<(), SemanticMirErrorV1> {
    if record.source_identity != body.identity()
        || !body_matches(body, record.types)
        || body_digest(body, MAX_EPOCH_BODY_BYTES)?.0 != record.body_identity
    {
        return Err(SemanticMirErrorV1::InvalidFunctionAbi);
    }
    Ok(())
}

impl InertSemanticMirRequestV1 {
    pub fn admit_exact_v20(
        self,
        limits: SemanticMirLimitsV1,
    ) -> Result<AdmittedInertSemanticMirV1, SemanticMirErrorV1> {
        self.admit_for_wire_version(SemanticMirWireVersionV1::V20, limits)
    }
}

pub(super) fn distinct_type_ids(ids: &[SemanticTypeIdV1]) -> bool {
    ids.iter()
        .enumerate()
        .all(|(index, id)| u64::from(id.index()) < HARD_MAX_TYPES_V1 && !ids[..index].contains(id))
}

fn direct_value(value: &SemanticAbiValueV1) -> bool {
    matches!(value.mode(), SemanticAbiPassModeV1::Direct(_))
        && value.adjusted().is_none()
        && value.pointee_override().is_none()
}

fn reference_abi(
    abi: &SemanticFunctionAbiV1,
    input: SemanticTypeIdV1,
    output: SemanticTypeIdV1,
) -> bool {
    abi.canon_abi() == SemanticCanonAbiV1::Rust
        && abi.extern_abi() == SemanticExternAbiV1::Rust
        && !abi.can_unwind()
        && !abi.c_variadic()
        && abi.fixed_count() == 1
        && abi.source_input_types() == [input]
        && abi.source_output_type() == output
        && abi.source_argument_ownership() == [SemanticSourceArgumentOwnershipV1::SharedBorrow]
        && abi.hidden_arguments().is_empty()
        && matches!(abi.arguments(), [argument] if argument.role() == SemanticAbiArgumentRoleV1::Source
            && argument.ty() == input && direct_value(argument.value()))
        && abi.return_value().ty() == output
        && direct_value(abi.return_value())
}

fn body_matches(
    body: &SemanticFunctionDeclV1,
    types: SemanticWorkgroupEpochProjectionTypesV1,
) -> bool {
    if body.role() != SemanticFunctionRoleV1::InternalHelper
        || body.export().is_some()
        || !reference_abi(body.abi(), types.reference, types.epoch_reference)
        || body.locals().len() != 2
        || body.blocks().len() != 1
        || body.entry().index() != 0
    {
        return false;
    }
    let Some(receiver) = body.locals().iter().position(|local| {
        local.role() == SemanticLocalRoleV1::Argument(0) && local.ty() == types.reference
    }) else {
        return false;
    };
    let Some(output) = body.locals().iter().position(|local| {
        local.role() == SemanticLocalRoleV1::Return && local.ty() == types.epoch_reference
    }) else {
        return false;
    };
    let block = &body.blocks()[0];
    if !matches!(block.terminator().kind(), SemanticTerminatorKindV1::Return) {
        return false;
    }
    let [statement] = block.statements() else {
        return false;
    };
    let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
        return false;
    };
    let destination = assignment.destination();
    let SemanticRvalueKindV1::Borrow {
        kind: SemanticBorrowKindV1::Shared,
        place,
    } = assignment.value().kind()
    else {
        return false;
    };
    destination.local().index() as usize == output
        && destination.projections().is_empty()
        && destination.ty() == types.epoch_reference
        && assignment.value().result_type() == types.epoch_reference
        && place.local().index() as usize == receiver
        && place.ty() == types.epoch_type
        && matches!(place.projections(), [deref, field]
            if deref.kind() == SemanticProjectionKindV1::Dereference && deref.result_type() == types.workgroup
                && field.kind() == SemanticProjectionKindV1::Field(2) && field.result_type() == types.epoch_type)
}

fn body_digest(
    body: &SemanticFunctionDeclV1,
    max_bytes: u64,
) -> Result<([u8; 32], usize), SemanticMirErrorV1> {
    let mut unannotated = body.clone();
    unannotated.defined_capability_contract = None;
    canonical_semantic_function_fragment_sha256_v1(
        &unannotated,
        SemanticMirWireVersionV1::V19,
        max_bytes.min(MAX_EPOCH_BODY_BYTES),
    )
}

fn aggregate_zst(request: &InertSemanticMirRequestV1, id: SemanticTypeIdV1) -> bool {
    request.types.get(id.index() as usize).is_some_and(|ty| {
        matches!(ty.shape(), SemanticTypeShapeV1::Aggregate(_))
            && ty.layout().size_bytes() == Some(0)
            && ty.layout().alignment_bytes() == 1
            && !ty.layout().is_uninhabited()
    })
}

pub(super) fn workgroup_fields(
    request: &InertSemanticMirRequestV1,
    id: SemanticTypeIdV1,
) -> Option<&[SemanticTypeIdV1]> {
    let ty = request.types.get(id.index() as usize)?;
    let SemanticTypeShapeV1::Aggregate(aggregate) = ty.shape() else {
        return None;
    };
    let [size, rank, epoch, marker] = aggregate.fields() else {
        return None;
    };
    (ty.layout().size_bytes() == Some(16)
        && ty.layout().alignment_bytes() == 8
        && !ty.layout().is_uninhabited()
        && [size, rank]
            .into_iter()
            .all(|id| is_unsigned_integer_with_bits(request, *id, 64))
        && aggregate_zst(request, *epoch)
        && aggregate_zst(request, *marker))
    .then_some(aggregate.fields())
}

fn subgroup_layout(request: &InertSemanticMirRequestV1, id: SemanticTypeIdV1) -> bool {
    let fields = |id: SemanticTypeIdV1, offsets: &[u64]| -> Option<&[SemanticTypeIdV1]> {
        let ty = request.types.get(id.index() as usize)?;
        let SemanticTypeShapeV1::Aggregate(fields) = ty.shape() else {
            return None;
        };
        let SemanticTypeLayoutDetailsV1::Aggregate(layout) = ty.layout().details() else {
            return None;
        };
        (ty.layout().size_bytes() == Some(4)
            && ty.layout().alignment_bytes() == 4
            && !ty.layout().is_uninhabited()
            && layout.field_offsets() == offsets)
            .then_some(fields.fields())
    };
    let Some([lane, brand, marker]) = fields(id, &[0, 4, 4]) else {
        return false;
    };
    let Some([rank, width, lane_brand, lane_marker]) = fields(*lane, &[0, 4, 4, 4]) else {
        return false;
    };
    is_unsigned_integer_with_bits(request, *rank, 32)
        && [brand, marker, width, lane_brand, lane_marker]
            .into_iter()
            .all(|id| aggregate_zst(request, *id))
}

pub(super) fn abi_matches(
    request: &InertSemanticMirRequestV1,
    abi: &SemanticFunctionAbiV1,
    operation: SemanticExecutionCapabilityOperationV1,
) -> bool {
    let SemanticExecutionCapabilityOperationV1::SubgroupDeriveBorrowed {
        workgroup_reference,
        workgroup,
        subgroup,
        width,
    } = operation
    else {
        return true;
    };
    width == 64
        && distinct_type_ids(&[workgroup_reference, workgroup, subgroup])
        && reference_abi(abi, workgroup_reference, subgroup)
        && shared_reference_to(request, workgroup_reference, workgroup)
        && workgroup_fields(request, workgroup).is_some()
        && subgroup_layout(request, subgroup)
}

pub(super) fn validate_function_projection(
    context: &mut ValidationContextV1<'_>,
    function: SemanticFunctionIdV1,
    body: &SemanticFunctionDeclV1,
) -> Result<(), SemanticMirErrorV1> {
    if let Some(record) = body.workgroup_epoch_projection() {
        context.one()?;
        let location = SemanticMirLocationV1::Function(record.function);
        for ty in record.types.all() {
            context.type_reference(ty, location)?;
        }
        if record.function != function
            || context
                .request
                .callables
                .get(record.function.index() as usize)
                != Some(&SemanticCallableDeclV1::Defined {
                    function: record.function,
                })
            || body.identity() != record.source_identity
            || !body_matches(body, record.types)
            || !kernel_capability_provenance_matches(context.request, record.provenance)
            || !shared_reference_to(
                context.request,
                record.types.reference,
                record.types.workgroup,
            )
            || !shared_reference_to(
                context.request,
                record.types.epoch_reference,
                record.types.epoch_type,
            )
            || workgroup_fields(context.request, record.types.workgroup)
                .is_none_or(|fields| fields[2] != record.types.epoch_type)
            || record.types.all().iter().any(|id| {
                let identity = context.request.types[id.index() as usize].identity();
                identity == record.brand || identity == record.epoch
            })
        {
            return Err(SemanticMirErrorV1::InvalidFunctionAbi);
        }
        // Reserve an exact bounded fragment budget before encoding; charge both
        // serialization and hashing work rather than hiding a roster-wide scan.
        let remaining = context
            .limits
            .limit(SemanticMirResourceV1::ValidationWork)
            .saturating_sub(context.work);
        let (digest, bytes) = body_digest(body, remaining / 2).map_err(|error| match error {
            SemanticMirErrorV1::LimitExceeded {
                resource: SemanticMirResourceV1::CanonicalBytes,
                ..
            } => SemanticMirErrorV1::LimitExceeded {
                resource: SemanticMirResourceV1::ValidationWork,
                actual: context.work.saturating_add(remaining).saturating_add(1),
                max: context.limits.limit(SemanticMirResourceV1::ValidationWork),
            },
            error => error,
        })?;
        charge_validation_work(context, bytes * 2)?;
        if digest != record.body_identity {
            return Err(SemanticMirErrorV1::InvalidFunctionAbi);
        }
    }
    Ok(())
}

impl IntrinsicCapabilityClaimsV1 {
    pub(super) fn record_borrowed_workgroup(
        &mut self,
        workgroup: SemanticTypeIdV1,
        provenance: SemanticKernelCapabilityProvenanceV1,
        brand: SemanticTypeIdentityV1,
        epoch: SemanticTypeIdentityV1,
    ) -> bool {
        let binding = (provenance, brand, epoch);
        match self.borrowed_workgroups.entry(workgroup) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(binding);
                true
            }
            std::collections::btree_map::Entry::Occupied(entry) => *entry.get() == binding,
        }
    }
}

pub(super) fn record_execution_claim(
    claims: &mut IntrinsicCapabilityClaimsV1,
    contract: SemanticExecutionCapabilityContractV1,
) -> bool {
    let SemanticExecutionCapabilityOperationV1::SubgroupDeriveBorrowed { workgroup, .. } =
        contract.operation()
    else {
        return true;
    };
    let (Some(brand), Some(epoch)) = (contract.workgroup_brand(), contract.epoch_before()) else {
        return false;
    };
    claims.record_borrowed_workgroup(workgroup, contract.provenance(), brand, epoch)
}
