//! Owner-scoped, checked-source to generated-field projection for inert V4 data.
//! This does not establish theorem, native, finalizer, or launch authority.
#![allow(
    dead_code,
    reason = "scoped projection data is not finalizer authority"
)]
#![allow(
    clippy::result_large_err,
    reason = "preserve child errors without allocation"
)]

use super::{
    AccessMode, CompilerDescriptorError, DescriptorArgumentKindV1 as Kind,
    TypedDescriptorArgumentV1, TypedDescriptorRootV1,
    conditional_output_binding_v1::{
        CompilerConditionalOutputDescriptorErrorV1, require_flat_abi_v1, select_typed_root_v1,
    },
    nominal_v3, production_descriptor_argument_matches_kernel_type_v1,
    require_production_descriptor_argument_semantic_type_v1,
};
use crate::production_pipeline::conditional_generated_fields_v1::ConditionalGeneratedFieldOwnerV1;
use fe2o3_kernel_descriptor::{
    ConditionalArgumentBindingV1, ConditionalArgumentRoleV1 as Role, MAX_CONDITIONAL_ARGUMENTS_V1,
    MAX_CONDITIONAL_READS_V1, MAX_CONDITIONAL_ROOTS_V1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, FunctionOperationLocation, Type,
    ValueId,
};
use fe2o3_lower_mir_kernel::{
    ProductionConditionalSourceArgumentV1, ProductionSemanticKirErrorV1,
    ProductionSourceBoundConditionalAggregateRequestV1 as Request,
};
use fe2o3_mir_model::semantic_mir_v1::{
    AdmittedInertSemanticMirV1, SemanticFunctionIdV1, SemanticTerminatorKindV1,
    SemanticTypeIdentityV1,
};
use fe2o3_pliron::{
    ProductionConditionalAggregateErrorV1, ProductionConditionalSourceCoordinatesV1,
    ProductionSourceArgumentBindingV1, ProductionSourceArgumentErrorV1,
    ProductionSourceArgumentRelationV1,
};
use std::{fmt, mem::size_of};

#[path = "compiler_descriptor_conditional_generated_fields_table_v1.rs"]
mod table;
use table::{Occurrence, Scratch};

#[derive(Debug)]
pub(crate) enum ConditionalGeneratedFieldErrorV1 {
    Resource(Resource),
    Descriptor(CompilerDescriptorError),
    Profile(CompilerConditionalOutputDescriptorErrorV1),
    Source(ProductionSemanticKirErrorV1),
    Argument(ProductionSourceArgumentErrorV1),
    Replay(ProductionConditionalAggregateErrorV1),
    Nominal(nominal_v3::NominalDescriptorErrorV3),
    Mismatch(&'static str),
}
type Error = ConditionalGeneratedFieldErrorV1;
impl From<Resource> for Error {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "conditional generated-field join: {self:?}")
    }
}
impl std::error::Error for Error {}

/// A copied row is descriptive, even when obtained from the scoped checked view.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ConditionalGeneratedArgumentV1 {
    projection: ConditionalArgumentBindingV1,
    canonical_value: ValueId,
}
impl ConditionalGeneratedArgumentV1 {
    pub(crate) const fn projection(self) -> ConditionalArgumentBindingV1 {
        self.projection
    }
    pub(crate) const fn canonical_value(self) -> ValueId {
        self.canonical_value
    }
    pub(crate) fn allocation_origin(self) -> u64 {
        u64::from(self.projection.source_argument) + 1
    }
}

/// Cannot be built from rows, keys, descriptor bytes, or a caller-supplied root
/// slice. Its references are available only during the original owner callback.
/// Copying any getters produces inert data, never retained checked authority.
pub(crate) struct ConditionalGeneratedFieldsV1<'s> {
    request: &'s Request<'s>,
    typed_root: &'s TypedDescriptorRootV1,
    semantic_root: SemanticFunctionIdV1,
    semantic_body: SemanticFunctionIdV1,
    arguments: &'s [ConditionalGeneratedArgumentV1],
    output_argument: u16,
    read_arguments: &'s [u16],
}
impl<'s> ConditionalGeneratedFieldsV1<'s> {
    /// Crosscheck the exact selected root from the enclosing retained replay,
    /// not a caller-assigned field or an inferred canonical/source ordinal.
    pub(crate) fn require_replayed_root_v1(
        &self,
        root: u32,
        budget: &mut Budget<'_>,
    ) -> Result<(), Error> {
        budget.charge_work(1)?;
        require_replayed_root(self.semantic_root, root)
    }
    pub(crate) const fn request(&self) -> &'s Request<'s> {
        self.request
    }
    pub(crate) const fn typed_root(&self) -> &'s TypedDescriptorRootV1 {
        self.typed_root
    }
    pub(crate) const fn semantic_root(&self) -> SemanticFunctionIdV1 {
        self.semantic_root
    }
    pub(crate) const fn semantic_body(&self) -> SemanticFunctionIdV1 {
        self.semantic_body
    }
    /// Unique canonical parameters, strictly sorted, not read occurrence rows.
    pub(crate) const fn arguments(&self) -> &'s [ConditionalGeneratedArgumentV1] {
        self.arguments
    }
    /// Index in `arguments`, not source/adjusted/canonical/generated ordinal.
    pub(crate) const fn output_argument(&self) -> u16 {
        self.output_argument
    }
    /// One argument-table index per request read, preserving exact producer order.
    pub(crate) const fn read_arguments(&self) -> &'s [u16] {
        self.read_arguments
    }
}

/// The private-field owner scope is minted only from an existing compiler stage.
/// Lower's request retains source translation and original-account continuity;
/// this replay must run inside that request's original-ledger callback. No
/// durable work identity, source relation, or generic conditional authority is
/// stored here. All new scratch is fixed-size, prepaid on the incoming ledger.
pub(crate) fn with_generated_fields_v1<'w, T>(
    owner: &ConditionalGeneratedFieldOwnerV1<'_>,
    request: &Request<'_>,
    budget: &mut Budget<'w>,
    consume: impl for<'s> FnOnce(ConditionalGeneratedFieldsV1<'s>, &mut Budget<'w>) -> Result<T, Error>,
) -> Result<T, Error> {
    let scratch_bytes = scratch_bytes()?;
    budget.with_prepaid_scope(
        owner.source().retained_analysis_storage_v1(),
        8,
        8,
        scratch_bytes,
        |budget| project(owner, request, budget, consume),
    )
}

fn scratch_bytes() -> Result<usize, Resource> {
    size_of::<Scratch>()
        .checked_add(size_of::<ConditionalGeneratedFieldsV1<'_>>())
        .and_then(|v| v.checked_add(size_of::<ProductionSourceArgumentRelationV1<'_, '_>>()))
        .and_then(|v| v.checked_add(size_of::<ProductionSourceArgumentBindingV1<'_, '_>>()))
        .and_then(|v| v.checked_add(size_of::<fe2o3_kernel_descriptor::SourceTypeRecordV3>()))
        .and_then(|v| v.checked_add(size_of::<fe2o3_kernel_descriptor::DeviceLayoutRecordV1>()))
        .and_then(|v| v.checked_add(fe2o3_kernel_descriptor::DESCRIPTOR_QUERY_STORAGE_V3))
        .and_then(|v| v.checked_add(2 * size_of::<ConditionalGeneratedArgumentV1>()))
        .and_then(|v| v.checked_add(size_of::<Occurrence>()))
        .ok_or(Resource::Arithmetic)
}

fn project<'w, T>(
    owner: &ConditionalGeneratedFieldOwnerV1<'_>,
    request: &Request<'_>,
    budget: &mut Budget<'w>,
    consume: impl for<'s> FnOnce(ConditionalGeneratedFieldsV1<'s>, &mut Budget<'w>) -> Result<T, Error>,
) -> Result<T, Error> {
    if !std::ptr::eq(owner.source(), request.source()) {
        return Err(Error::Mismatch("original compilation source owner"));
    }
    let input = request.pliron_input();
    check_cardinality(
        input.outputs().len(),
        input.reads().len(),
        request.arguments().len(),
    )?;
    if owner.roots().is_empty() || owner.roots().len() > MAX_CONDITIONAL_ROOTS_V1 {
        return Err(Error::Mismatch("bounded complete typed roots"));
    }
    input
        .require_current_graph_v1(budget)
        .map_err(Error::Replay)?;
    let recipe = request
        .translation()
        .pending()
        .kernel()
        .map_err(|_| Error::Mismatch("retained source recipe"))?;
    if !std::ptr::eq(recipe, input.kernel()) {
        return Err(Error::Mismatch("exact translated aggregate recipe"));
    }
    let semantic = owner.source().semantic_ssa().source_semantic();
    // Lower already checked this exact export against the selected
    // source root. Resolve it in the SAME owner, not by hashing a name
    // or guessing a function/root ordinal from a generated symbol.
    let (root_id, body_id) = select_source(semantic, recipe.function_name(), budget)?;
    let typed_root =
        select_typed_root_v1(owner.roots(), semantic, root_id, budget).map_err(Error::Profile)?;
    let body = semantic
        .functions()
        .get(body_id.index() as usize)
        .ok_or(Error::Mismatch("selected semantic body"))?;
    require_flat_abi_v1(body.abi(), semantic.types(), budget).map_err(Error::Profile)?;
    // Fresh relation, used and dropped while the original Work borrow
    // is live. The aggregate's separate replay above protects its exact
    // canonical/source association; this relation maps generated fields.
    let relation = owner
        .source()
        .checked_source_argument_relation_v1(root_id, body_id, budget)
        .map_err(Error::Source)?;
    let mut scratch = Scratch::new();
    let output = input.outputs()[0];
    if output.canonical_parameter() != output.source().canonical_parameter() {
        return Err(Error::Mismatch("output canonical parameter"));
    }
    let output_row = join_argument(
        typed_root,
        semantic,
        &relation,
        &request.arguments()[0],
        output.source(),
        Role::Output,
        budget,
    )?;
    scratch.insert(output_row, budget)?;
    for (index, read) in input.reads().iter().enumerate() {
        budget.charge_work(6)?;
        if read.canonical().parameter() != read.source().canonical_parameter()
            || read.canonical().slice() != read.source().canonical_value()
            || read.site().canonical != read.canonical().location()
        {
            return Err(Error::Mismatch("read canonical occurrence"));
        }
        let row = join_argument(
            typed_root,
            semantic,
            &relation,
            &request.arguments()[index + 1],
            read.source(),
            Role::Input,
            budget,
        )?;
        scratch.insert(row, budget)?;
        scratch.push_read(
            Occurrence {
                canonical: read.canonical().location(),
                ranked: (read.site().block, read.site().operation),
                parameter: read.source().canonical_parameter(),
            },
            budget,
        )?;
    }
    let output_argument =
        scratch.finish(output.canonical_parameter(), input.reads().len(), budget)?;
    let result = consume(
        ConditionalGeneratedFieldsV1 {
            request,
            typed_root,
            semantic_root: root_id,
            semantic_body: body_id,
            arguments: scratch.arguments(),
            output_argument,
            read_arguments: scratch.read_arguments(),
        },
        budget,
    );
    // Replay even on consumer error, before the common scope cleans up.
    // A foreign ledger cannot be made valid by restoring a storage floor.
    input
        .require_current_graph_v1(budget)
        .map_err(Error::Replay)?;
    result
}

fn check_cardinality(outputs: usize, reads: usize, rows: usize) -> Result<(), Error> {
    if outputs != 1 || reads > MAX_CONDITIONAL_READS_V1 || rows != reads + 1 {
        return Err(Error::Mismatch("complete output/read occurrence roster"));
    }
    Ok(())
}

fn select_source(
    semantic: &AdmittedInertSemanticMirV1,
    symbol: &str,
    budget: &mut Budget<'_>,
) -> Result<(SemanticFunctionIdV1, SemanticFunctionIdV1), Error> {
    let mut selected = None;
    for id in semantic.roots() {
        budget.charge_work(4)?;
        let root = semantic
            .functions()
            .get(id.index() as usize)
            .ok_or(Error::Mismatch("source root function"))?;
        let entry = root
            .kernel_entry()
            .ok_or(Error::Mismatch("source kernel entry"))?;
        budget.charge_work(
            entry
                .export_symbol()
                .as_bytes()
                .len()
                .checked_add(symbol.len())
                .ok_or(Resource::Arithmetic)?,
        )?;
        if entry.export_symbol().as_bytes() == symbol.as_bytes() && selected.replace(*id).is_some()
        {
            return Err(Error::Mismatch("unique selected source export"));
        }
    }
    let id = selected.ok_or(Error::Mismatch("selected source export"))?;
    let root = &semantic.functions()[id.index() as usize];
    // Prepay the existing admitted selector, including exact transparent Result
    // forwarding. Equality scans cannot exceed the root ABI lengths. No wrapper
    // parser or correspondence reconstruction is introduced here.
    budget.charge_work(
        root.abi()
            .source_input_types()
            .len()
            .checked_add(root.abi().source_argument_ownership().len())
            .and_then(|v| v.checked_add(root.blocks().len()))
            .and_then(|v| v.checked_add(semantic.roots().len()))
            .and_then(|v| v.checked_mul(32))
            .and_then(|v| v.checked_add(32))
            .ok_or(Resource::Arithmetic)?,
    )?;
    for block in root.blocks() {
        budget.charge_work(
            block
                .statements()
                .len()
                .checked_mul(4)
                .ok_or(Resource::Arithmetic)?,
        )?;
        if let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() {
            budget.charge_work(
                call.arguments()
                    .len()
                    .checked_mul(8)
                    .ok_or(Resource::Arithmetic)?,
            )?;
        }
    }
    let selection = semantic
        .select_kernel_body_for_root_v1(id)
        .ok_or(Error::Mismatch("exact source root/body selection"))?;
    Ok((id, selection.body()))
}

fn join_argument(
    root: &TypedDescriptorRootV1,
    semantic: &AdmittedInertSemanticMirV1,
    relation: &ProductionSourceArgumentRelationV1<'_, '_>,
    request_row: &ProductionConditionalSourceArgumentV1,
    source: ProductionConditionalSourceCoordinatesV1,
    role: Role,
    budget: &mut Budget<'_>,
) -> Result<ConditionalGeneratedArgumentV1, Error> {
    budget.charge_work(16)?;
    if request_row.canonical_parameter() != source.canonical_parameter()
        || request_row.source_argument() != source.source_argument()
        || request_row.adjusted_argument() != source.adjusted_argument()
        || request_row.semantic_local() != source.semantic_local()
        || request_row.semantic_type() != source.semantic_type()
    {
        return Err(Error::Mismatch("source request/aggregate coordinates"));
    }
    let binding = relation
        .bind_whole_parameter_v1(
            source.canonical_parameter(),
            source.canonical_value(),
            budget,
        )
        .map_err(Error::Argument)?;
    budget.charge_work(8)?;
    if binding.canonical_parameter() != source.canonical_parameter()
        || binding.canonical_value() != source.canonical_value()
        || binding.source_argument() != source.source_argument()
        || binding.adjusted_argument() != source.adjusted_argument()
        || binding.semantic_local() != source.semantic_local()
        || binding.semantic_type() != source.semantic_type()
    {
        return Err(Error::Mismatch("fresh whole source argument replay"));
    }
    // This bounded profile admits only whole flat arguments. Source/adjusted
    // equality is checked, not assumed or used to compact hidden parameters.
    let field = checked_field_index(
        binding.source_argument(),
        binding.adjusted_argument(),
        root.arguments.len(),
    )?;
    let argument = root
        .arguments
        .as_slice()
        .get(field)
        .ok_or(Error::Mismatch("generated logical field"))?;
    let semantic_type = semantic
        .types()
        .get(binding.semantic_type().index() as usize)
        .ok_or(Error::Mismatch("source semantic type"))?;
    let physical_type = relation
        .canonical_function()
        .signature
        .parameters
        .get(binding.canonical_parameter() as usize)
        .ok_or(Error::Mismatch("canonical parameter type"))?;
    check_field(
        argument,
        role,
        semantic_type.identity(),
        physical_type,
        budget,
    )?;
    let (source_type, layout) =
        nominal_v3::records(argument.kind, budget).map_err(Error::Nominal)?;
    Ok(ConditionalGeneratedArgumentV1 {
        projection: ConditionalArgumentBindingV1 {
            canonical_parameter: binding.canonical_parameter(),
            source_argument: binding.source_argument(),
            adjusted_argument: binding.adjusted_argument(),
            semantic_local: binding.semantic_local().index(),
            semantic_type: binding.semantic_type().index(),
            generated_field: u16::try_from(field).map_err(|_| Resource::Arithmetic)?,
            role,
            source_type_identity: *source_type.identity().as_bytes(),
            device_layout_identity: *layout.identity().as_bytes(),
        },
        canonical_value: binding.canonical_value(),
    })
}

fn check_field(
    argument: &TypedDescriptorArgumentV1,
    role: Role,
    semantic_identity: SemanticTypeIdentityV1,
    physical_type: &Type,
    budget: &mut Budget<'_>,
) -> Result<(), Error> {
    budget.charge_work(48)?;
    require_production_descriptor_argument_semantic_type_v1(argument, semantic_identity)
        .map_err(Error::Descriptor)?;
    let kind = match role {
        Role::Input => {
            matches!(argument.kind, Kind::SharedSlice(_)) && argument.access == AccessMode::ReadOnly
        }
        Role::Output => {
            matches!(argument.kind, Kind::DisjointSlice(_))
                && matches!(
                    argument.access,
                    AccessMode::WriteOnly | AccessMode::ReadWrite
                )
        }
    };
    if !kind
        || !production_descriptor_argument_matches_kernel_type_v1(
            argument.kind,
            argument.access,
            physical_type,
        )
    {
        return Err(Error::Mismatch("whole slice kind/access/physical type"));
    }
    Ok(())
}

fn checked_field_index(source: u32, adjusted: u32, count: usize) -> Result<usize, Error> {
    if source != adjusted {
        return Err(Error::Mismatch("flat generated/source/adjusted ABI"));
    }
    let field = usize::try_from(source).map_err(|_| Resource::Arithmetic)?;
    if field >= count || field >= MAX_CONDITIONAL_ARGUMENTS_V1 {
        return Err(Error::Mismatch("bounded generated logical field"));
    }
    Ok(field)
}

fn require_replayed_root(selected: SemanticFunctionIdV1, replayed: u32) -> Result<(), Error> {
    if selected.index() != replayed {
        return Err(Error::Mismatch("retained replay/generated-field root"));
    }
    Ok(())
}

#[cfg(test)]
#[path = "compiler_descriptor_conditional_generated_fields_observation_v1.rs"]
pub(crate) mod observation;

#[cfg(test)]
#[path = "compiler_descriptor_conditional_generated_fields_v1_tests.rs"]
mod tests;
