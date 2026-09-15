//! V24 defined conversion, tag 8. An inert commitment is not source
//! authentication and this record never issues or initializes an allocation.
use super::*;
const MAX_BODY_BYTES: u64 = 16 * 1024;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticReusableLdsTypesV1 {
    pub input: SemanticTypeIdV1,
    pub output: SemanticTypeIdV1,
    pub element: SemanticTypeIdV1,
    pub storage_marker: SemanticTypeIdV1,
    pub state_marker: SemanticTypeIdV1,
    pub workgroup_marker: SemanticTypeIdV1,
    pub epoch_marker: SemanticTypeIdV1,
    pub thread_marker: SemanticTypeIdV1,
}

impl SemanticReusableLdsTypesV1 {
    pub const fn new(ids: [SemanticTypeIdV1; 8]) -> Self {
        let [
            input,
            output,
            element,
            storage_marker,
            state_marker,
            workgroup_marker,
            epoch_marker,
            thread_marker,
        ] = ids;
        Self {
            input,
            output,
            element,
            storage_marker,
            state_marker,
            workgroup_marker,
            epoch_marker,
            thread_marker,
        }
    }
    pub const fn all(self) -> [SemanticTypeIdV1; 8] {
        [
            self.input,
            self.output,
            self.element,
            self.storage_marker,
            self.state_marker,
            self.workgroup_marker,
            self.epoch_marker,
            self.thread_marker,
        ]
    }
}

/// One original caller's nested producer/consumer edge. Expansion may replay
/// it in multiple caller instances, but may not substitute another source site.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticReusableLdsSourceV1 {
    pub caller: SemanticFunctionIdV1,
    pub caller_identity: SemanticFunctionIdentityV1,
    pub caller_abi: SemanticAbiIdentityV1,
    pub allocation_callable: SemanticCallableIdV1,
    pub allocation_identity: SemanticFunctionIdentityV1,
    pub allocation_abi: SemanticAbiIdentityV1,
    pub allocation_block: SemanticBlockIdV1,
    pub allocation_local: SemanticLocalIdV1,
    pub conversion_block: SemanticBlockIdV1,
    /// Commitment to the live HIR receipt and raw caller body, revalidated by
    /// the private importer custody. This field alone authenticates nothing.
    pub source_binding: [u8; 32],
}

#[must_use]
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticReusableLdsConversionV1 {
    function: SemanticFunctionIdV1,
    source_identity: SemanticFunctionIdentityV1,
    abi_identity: SemanticAbiIdentityV1,
    body_identity: [u8; 32],
    types: SemanticReusableLdsTypesV1,
    source: SemanticReusableLdsSourceV1,
    provenance: SemanticKernelCapabilityProvenanceV1,
    brand: SemanticTypeIdentityV1,
    epoch: SemanticTypeIdentityV1,
    elements: u64,
    element_layout: SemanticLayoutIdentityV1,
    element_size: u64,
    element_align: u64,
}

impl SemanticReusableLdsConversionV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn for_defined_function(
        function: SemanticFunctionIdV1,
        functions: &[SemanticFunctionDeclV1],
        callables: &[SemanticCallableDeclV1],
        declarations: &[SemanticTypeDeclV1],
        types: SemanticReusableLdsTypesV1,
        source: SemanticReusableLdsSourceV1,
        provenance: SemanticKernelCapabilityProvenanceV1,
        brand: SemanticTypeIdentityV1,
        epoch: SemanticTypeIdentityV1,
        elements: u64,
    ) -> Result<Self, SemanticMirErrorV1> {
        let body = functions
            .get(function.index() as usize)
            .ok_or(SemanticMirErrorV1::InvalidFunctionAbi)?;
        require(
            callables.get(function.index() as usize)
                == Some(&SemanticCallableDeclV1::Defined { function }),
        )?;
        require(body_matches(body, types) && types_match(declarations, types))?;
        let (body_identity, _) = body_digest(body, MAX_BODY_BYTES)?;
        let element = &declarations[types.element.index() as usize];
        let record = Self::from_encoded_parts(
            function,
            body.identity(),
            body.abi().identity(),
            body_identity,
            types,
            source,
            provenance,
            brand,
            epoch,
            elements,
            element.layout_identity(),
            element
                .layout()
                .size_bytes()
                .ok_or(SemanticMirErrorV1::InvalidFunctionAbi)?,
            element.layout().alignment_bytes(),
        )?;
        validate_source(functions, callables, record)?;
        Ok(record)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn from_encoded_parts(
        function: SemanticFunctionIdV1,
        source_identity: SemanticFunctionIdentityV1,
        abi_identity: SemanticAbiIdentityV1,
        body_identity: [u8; 32],
        types: SemanticReusableLdsTypesV1,
        source: SemanticReusableLdsSourceV1,
        provenance: SemanticKernelCapabilityProvenanceV1,
        brand: SemanticTypeIdentityV1,
        epoch: SemanticTypeIdentityV1,
        elements: u64,
        element_layout: SemanticLayoutIdentityV1,
        element_size: u64,
        element_align: u64,
    ) -> Result<Self, SemanticMirErrorV1> {
        let ids = types.all();
        require(
            ids.iter()
                .enumerate()
                .all(|(i, id)| u64::from(id.index()) < HARD_MAX_TYPES_V1 && !ids[..i].contains(id))
                && function != source.caller
                && function != provenance.root()
                && u64::from(function.index()) < HARD_MAX_FUNCTIONS_V1
                && u64::from(source.caller.index()) < HARD_MAX_FUNCTIONS_V1
                && u64::from(source.allocation_callable.index()) < HARD_MAX_CALLABLES_V1
                && u64::from(source.allocation_local.index()) < HARD_MAX_LOCALS_V1
                && u64::from(source.allocation_block.index()) < HARD_MAX_BLOCKS_V1
                && u64::from(source.conversion_block.index()) < HARD_MAX_BLOCKS_V1
                && source.allocation_block != source.conversion_block
                && [
                    source_identity.as_bytes(),
                    abi_identity.as_bytes(),
                    &body_identity,
                    source.caller_identity.as_bytes(),
                    source.caller_abi.as_bytes(),
                    source.allocation_identity.as_bytes(),
                    source.allocation_abi.as_bytes(),
                    &source.source_binding,
                    brand.as_bytes(),
                    epoch.as_bytes(),
                    element_layout.as_bytes(),
                ]
                .into_iter()
                .all(|id| id != &[0; 32])
                && source_identity != source.caller_identity
                && source_identity != source.allocation_identity
                && elements != 0
                && element_size != 0
                && element_align.is_power_of_two()
                && element_size.is_multiple_of(element_align)
                && element_size.checked_mul(elements).is_some(),
        )?;
        Ok(Self {
            function,
            source_identity,
            abi_identity,
            body_identity,
            types,
            source,
            provenance,
            brand,
            epoch,
            elements,
            element_layout,
            element_size,
            element_align,
        })
    }
    pub const fn function(self) -> SemanticFunctionIdV1 {
        self.function
    }
    pub const fn source_identity(self) -> SemanticFunctionIdentityV1 {
        self.source_identity
    }
    pub const fn abi_identity(self) -> SemanticAbiIdentityV1 {
        self.abi_identity
    }
    pub const fn body_identity(&self) -> &[u8; 32] {
        &self.body_identity
    }
    pub const fn types(self) -> SemanticReusableLdsTypesV1 {
        self.types
    }
    pub const fn source(self) -> SemanticReusableLdsSourceV1 {
        self.source
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
    pub const fn elements(self) -> u64 {
        self.elements
    }
    pub const fn element_layout(self) -> SemanticLayoutIdentityV1 {
        self.element_layout
    }
    pub const fn element_size(self) -> u64 {
        self.element_size
    }
    pub const fn element_align(self) -> u64 {
        self.element_align
    }
    pub const fn receiver_argument(self) -> u32 {
        0
    }

    pub(super) fn encode_payload(
        self,
        writer: &mut CanonicalWriterV1,
    ) -> Result<(), SemanticMirErrorV1> {
        writer.u32(self.function.index())?;
        writer.identity(*self.source_identity.as_bytes())?;
        writer.identity(*self.abi_identity.as_bytes())?;
        writer.identity(self.body_identity)?;
        for id in self.types.all() {
            writer.u32(id.index())?;
        }
        writer.u32(self.source.caller.index())?;
        writer.identity(*self.source.caller_identity.as_bytes())?;
        writer.identity(*self.source.caller_abi.as_bytes())?;
        writer.u32(self.source.allocation_callable.index())?;
        writer.identity(*self.source.allocation_identity.as_bytes())?;
        writer.identity(*self.source.allocation_abi.as_bytes())?;
        writer.u32(self.source.allocation_block.index())?;
        writer.u32(self.source.allocation_local.index())?;
        writer.u32(self.source.conversion_block.index())?;
        writer.identity(self.source.source_binding)?;
        encode_kernel_capability_provenance(writer, self.provenance)?;
        writer.identity(*self.brand.as_bytes())?;
        writer.identity(*self.epoch.as_bytes())?;
        writer.u64(self.elements)?;
        writer.identity(*self.element_layout.as_bytes())?;
        writer.u64(self.element_size)?;
        writer.u64(self.element_align)
    }
}

fn require(condition: bool) -> Result<(), SemanticMirErrorV1> {
    if condition {
        Ok(())
    } else {
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    }
}

pub(super) fn body_matches(
    body: &SemanticFunctionDeclV1,
    types: SemanticReusableLdsTypesV1,
) -> bool {
    let abi = body.abi();
    body.role() == SemanticFunctionRoleV1::InternalHelper
        && body.export().is_none()
        && body.locals().len() == 2
        && body.blocks().len() == 1
        && body
            .blocks()
            .get(body.entry().index() as usize)
            .is_some_and(|b| {
                b.statements().is_empty()
                    && matches!(b.terminator().kind(), SemanticTerminatorKindV1::Return)
            })
        && body
            .locals()
            .iter()
            .filter(|l| l.role() == SemanticLocalRoleV1::Argument(0) && l.ty() == types.input)
            .count()
            == 1
        && body
            .locals()
            .iter()
            .filter(|l| l.role() == SemanticLocalRoleV1::Return && l.ty() == types.output)
            .count()
            == 1
        && abi.canon_abi() == SemanticCanonAbiV1::Rust
        && abi.extern_abi() == SemanticExternAbiV1::Rust
        && !abi.can_unwind()
        && !abi.c_variadic()
        && abi.fixed_count() == 1
        && abi.hidden_arguments().is_empty()
        && abi.arguments().len() == 1
        && abi.source_input_types() == [types.input]
        && abi.source_output_type() == types.output
        && abi.source_argument_ownership() == [SemanticSourceArgumentOwnershipV1::ByValue]
        && abi.arguments()[0].role() == SemanticAbiArgumentRoleV1::Source
        && abi.arguments()[0].ty() == types.input
        && matches!(
            abi.arguments()[0].value().mode(),
            SemanticAbiPassModeV1::Ignore
        )
        && abi.arguments()[0].value().adjusted().is_none()
        && abi.arguments()[0].value().pointee_override().is_none()
        && abi.return_value().ty() == types.output
        && matches!(abi.return_value().mode(), SemanticAbiPassModeV1::Ignore)
        && abi.return_value().adjusted().is_none()
        && abi.return_value().pointee_override().is_none()
}

fn types_match(declarations: &[SemanticTypeDeclV1], types: SemanticReusableLdsTypesV1) -> bool {
    let aggregate = |id: SemanticTypeIdV1, fields: &[SemanticTypeIdV1]| {
        declarations.get(id.index() as usize).is_some_and(|ty| {
            matches!(ty.shape(), SemanticTypeShapeV1::Aggregate(a) if a.fields() == fields)
                && ty.layout().size_bytes() == Some(0)
                && ty.layout().alignment_bytes() == 1
                && *ty.layout().backend_repr() == SemanticBackendReprV1::Memory { sized: true }
                && !ty.layout().is_uninhabited()
        })
    };
    aggregate(
        types.input,
        &[
            types.storage_marker,
            types.state_marker,
            types.workgroup_marker,
            types.epoch_marker,
            types.thread_marker,
        ],
    ) && aggregate(
        types.output,
        &[
            types.storage_marker,
            types.workgroup_marker,
            types.thread_marker,
        ],
    ) && [
        types.storage_marker,
        types.state_marker,
        types.workgroup_marker,
        types.epoch_marker,
        types.thread_marker,
    ]
    .into_iter()
    .all(|id| aggregate(id, &[]))
        && declarations
            .get(types.element.index() as usize)
            .is_some_and(|ty| {
                ty.layout().size_bytes().is_some_and(|size| size != 0)
                    && !ty.layout().is_uninhabited()
            })
}

fn body_digest(
    body: &SemanticFunctionDeclV1,
    max: u64,
) -> Result<([u8; 32], usize), SemanticMirErrorV1> {
    // The fixed empty body has no post-V19 instructions. Keep the existing
    // fragment format, and strip only this attachment to avoid a hash cycle.
    let original = SemanticFunctionDeclV1::new(
        body.identity(),
        body.role(),
        body.item_definition_identity(),
        body.monomorphization_identity(),
        body.generic_type_arguments_identity(),
        body.const_generic_arguments_identity(),
        body.source(),
        body.abi().clone(),
        body.locals().to_vec(),
        body.entry(),
        body.blocks().to_vec(),
    )?;
    canonical_semantic_function_fragment_sha256_v1(
        &original,
        SemanticMirWireVersionV1::V19,
        max.min(MAX_BODY_BYTES),
    )
}

pub(super) fn validate_attachment(
    body: &SemanticFunctionDeclV1,
    record: SemanticReusableLdsConversionV1,
) -> Result<(), SemanticMirErrorV1> {
    require(
        body_matches(body, record.types)
            && body.identity() == record.source_identity
            && body.abi().identity() == record.abi_identity
            && body_digest(body, MAX_BODY_BYTES)?.0 == record.body_identity,
    )
}

fn source_call(
    body: &SemanticFunctionDeclV1,
    block: SemanticBlockIdV1,
) -> Option<&SemanticDirectCallV1> {
    match body
        .blocks()
        .get(block.index() as usize)?
        .terminator()
        .kind()
    {
        SemanticTerminatorKindV1::Call(call) => Some(call),
        _ => None,
    }
}

/// Checks the exact source producer, not a same-shaped ZST argument. Live HIR
/// authentication of source_binding remains mandatory at production import.
pub(super) fn validate_source(
    functions: &[SemanticFunctionDeclV1],
    callables: &[SemanticCallableDeclV1],
    record: SemanticReusableLdsConversionV1,
) -> Result<(), SemanticMirErrorV1> {
    let source = record.source;
    let caller = functions
        .get(source.caller.index() as usize)
        .ok_or(SemanticMirErrorV1::InvalidFunctionAbi)?;
    require(
        caller.identity() == source.caller_identity && caller.abi().identity() == source.caller_abi,
    )?;
    let allocation = source_call(caller, source.allocation_block)
        .ok_or(SemanticMirErrorV1::InvalidFunctionAbi)?;
    let conversion = source_call(caller, source.conversion_block)
        .ok_or(SemanticMirErrorV1::InvalidFunctionAbi)?;
    let Some(SemanticCallableDeclV1::CompilerIntrinsic {
        binding,
        operation: SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract },
        ..
    }) = callables.get(source.allocation_callable.index() as usize)
    else {
        return Err(SemanticMirErrorV1::InvalidFunctionAbi);
    };
    let SemanticExecutionCapabilityOperationV1::LdsAllocate {
        workgroup,
        lds,
        element,
        elements,
    } = contract.operation()
    else {
        return Err(SemanticMirErrorV1::InvalidFunctionAbi);
    };
    require(
        lds == record.types.input
            && element == record.types.element
            && elements == record.elements
            && contract.provenance() == record.provenance
            && contract.workgroup_brand() == Some(record.brand)
            && contract.epoch_before() == Some(record.epoch)
            && contract.epoch_after().is_none()
            && binding.identity() == source.allocation_identity
            && binding.abi().identity() == source.allocation_abi
            && allocation.callee() == source.allocation_callable
            && allocation.arguments().len() == 1
            && allocation.arguments()[0].ty() == workgroup
            && allocation.unwind() == SemanticUnwindActionV1::Unreachable
            && allocation.destination().is_some_and(|d| {
                d.place().local() == source.allocation_local
                    && d.place().ty() == record.types.input
                    && d.place().projections().is_empty()
                    && d.edge().role() == SemanticEdgeRoleV1::CallReturn
                    && d.edge().target() == source.conversion_block
            })
            && callables.get(conversion.callee().index() as usize)
                == Some(&SemanticCallableDeclV1::Defined {
                    function: record.function,
                })
            && conversion.arguments().len() == 1
            && conversion.arguments()[0].ty() == record.types.input
            && conversion.unwind() == SemanticUnwindActionV1::Unreachable
            && caller.blocks()[source.conversion_block.index() as usize]
                .statements()
                .is_empty()
            && conversion.destination().is_some_and(|d| {
                d.place().ty() == record.types.output
                    && d.place().projections().is_empty()
                    && d.place().local() != source.allocation_local
                    && d.edge().role() == SemanticEdgeRoleV1::CallReturn
                    && d.edge().target() != source.conversion_block
                    && d.edge().target() != source.allocation_block
            }),
    )?;
    require(
        matches!(&conversion.arguments()[0],
        SemanticOperandV1::Move(place) if place.local() == source.allocation_local && place.projections().is_empty())
            || matches!(&conversion.arguments()[0], SemanticOperandV1::Constant(c) if matches!(c.value(), SemanticConstantValueV1::ZeroSized)),
    )?;
    // An inert record cannot grant an alternate entry into the consuming call.
    // Check every edge, including unwind and loop backedges, not block order.
    let mut work = 0u64;
    let mut incoming = 0u32;
    let mut conversions = 0u32;
    for (index, block) in caller.blocks().iter().enumerate() {
        work = work
            .checked_add(1)
            .ok_or(SemanticMirErrorV1::InvalidFunctionAbi)?;
        require(work <= HARD_MAX_VALIDATION_WORK_V1)?;
        if let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() {
            if callables.get(call.callee().index() as usize)
                == Some(&SemanticCallableDeclV1::Defined {
                    function: record.function,
                })
            {
                conversions = conversions
                    .checked_add(1)
                    .ok_or(SemanticMirErrorV1::InvalidFunctionAbi)?;
                require(index == source.conversion_block.index() as usize)?;
            }
        }
        block
            .terminator()
            .kind()
            .try_for_each_edge::<SemanticMirErrorV1>(|edge| {
                work = work
                    .checked_add(1)
                    .ok_or(SemanticMirErrorV1::InvalidFunctionAbi)?;
                require(work <= HARD_MAX_VALIDATION_WORK_V1)?;
                if edge.target() == source.conversion_block {
                    incoming = incoming
                        .checked_add(1)
                        .ok_or(SemanticMirErrorV1::InvalidFunctionAbi)?;
                    require(
                        index == source.allocation_block.index() as usize
                            && edge.role() == SemanticEdgeRoleV1::CallReturn,
                    )?;
                }
                Ok(())
            })?;
    }
    require(incoming == 1 && conversions == 1 && caller.entry() != source.conversion_block)?;
    Ok(())
}

pub(super) fn validate(
    context: &mut ValidationContextV1<'_>,
    function: SemanticFunctionIdV1,
    record: SemanticReusableLdsConversionV1,
) -> Result<(), SemanticMirErrorV1> {
    context.one()?;
    require(
        function == record.function
            && kernel_capability_provenance_matches(context.request, record.provenance),
    )?;
    for id in record.types.all() {
        context.type_reference(id, SemanticMirLocationV1::Function(function))?;
    }
    let body = &context.request.functions[function.index() as usize];
    require(types_match(&context.request.types, record.types))?;
    let element = &context.request.types[record.types.element.index() as usize];
    require(
        element.layout_identity() == record.element_layout
            && element.layout().size_bytes() == Some(record.element_size)
            && element.layout().alignment_bytes() == record.element_align,
    )?;
    let budget = context
        .limits
        .limit(SemanticMirResourceV1::ValidationWork)
        .saturating_sub(context.work)
        / 2;
    require(
        body_matches(body, record.types)
            && body.identity() == record.source_identity
            && body.abi().identity() == record.abi_identity,
    )?;
    let (hash, bytes) = body_digest(body, budget)?;
    charge_validation_work(
        context,
        bytes
            .checked_mul(2)
            .ok_or(SemanticMirErrorV1::InvalidFunctionAbi)?,
    )?;
    require(hash == record.body_identity)?;
    let caller = context
        .request
        .functions
        .get(record.source.caller.index() as usize)
        .ok_or(SemanticMirErrorV1::InvalidFunctionAbi)?;
    let mut work = caller.blocks().len();
    for block in caller.blocks() {
        block
            .terminator()
            .kind()
            .try_for_each_edge::<SemanticMirErrorV1>(|_| {
                work = work
                    .checked_add(1)
                    .ok_or(SemanticMirErrorV1::InvalidFunctionAbi)?;
                Ok(())
            })?;
    }
    charge_validation_work(
        context,
        work.checked_mul(2)
            .ok_or(SemanticMirErrorV1::InvalidFunctionAbi)?,
    )?;
    validate_source(
        &context.request.functions,
        &context.request.callables,
        record,
    )
}

#[cfg(test)]
#[path = "defined_reusable_lds_v1/tests.rs"]
pub(crate) mod tests;
