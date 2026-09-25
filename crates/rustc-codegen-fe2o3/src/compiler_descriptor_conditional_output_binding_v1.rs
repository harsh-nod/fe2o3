//! Descriptive source-output/typed-descriptor agreement, not admission or custody.
#![allow(dead_code, reason = "pending transaction-owned descriptor integration")]

use super::{
    CompilerDescriptorError, DescriptorArgumentKindV1, TypedDescriptorRootV1,
    production_descriptor_argument_matches_kernel_type_v1,
    require_production_descriptor_argument_semantic_type_v1,
    validate_production_v1_semantic_root_ownership_evidence,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, Type,
};
use fe2o3_lower_mir_kernel::{ProductionConditionalOutputBindingV1, ProductionPreRankedKirOwnerV1};
use fe2o3_mir_model::semantic_mir_v1::{
    AdmittedInertSemanticMirV1, SemanticAbiArgumentRoleV1, SemanticAbiPassModeV1,
    SemanticFunctionAbiV1, SemanticFunctionIdV1, SemanticRustTypeKindV1, SemanticTypeDeclV1,
    SemanticTypeShapeV1,
};
use std::fmt;

#[path = "compiler_descriptor_conditional_output_binding_mapping_v1.rs"]
mod mapping;
pub(crate) use mapping::{GeneratedFieldCoordinatesV1, checked_generated_field_v1};

/// Failure of descriptive agreement or the caller's cumulative resource budget.
#[derive(Debug)]
pub(crate) enum CompilerConditionalOutputDescriptorErrorV1 {
    /// The sealed conditional binding belongs to another canonical owner.
    ForeignOwner,
    /// The complete ordered root roster or selected root does not agree.
    Root,
    /// Generated packing does not support this source/adjusted ABI profile.
    UnsupportedAbi,
    /// Logical context elision lacks the original compiler admission.
    ContextAdmission,
    /// The certified whole output does not select an exact writable slice field.
    OutputArgument,
    /// An existing descriptor/semantic identity check failed unchanged.
    Descriptor(CompilerDescriptorError),
    /// The caller did not retain the owner floor or exhausted its work budget.
    Resource(Resource),
}

impl fmt::Display for CompilerConditionalOutputDescriptorErrorV1 {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Descriptor(error) => error.fmt(out),
            Self::Resource(error) => error.fmt(out),
            other => write!(out, "conditional output descriptor disagreement: {other:?}"),
        }
    }
}

impl std::error::Error for CompilerConditionalOutputDescriptorErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Descriptor(error) => Some(error),
            Self::Resource(error) => Some(error),
            _ => None,
        }
    }
}

impl From<CompilerDescriptorError> for CompilerConditionalOutputDescriptorErrorV1 {
    fn from(error: CompilerDescriptorError) -> Self {
        Self::Descriptor(error)
    }
}

impl From<Resource> for CompilerConditionalOutputDescriptorErrorV1 {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}

type Error = CompilerConditionalOutputDescriptorErrorV1;

/// A borrowed whole-source-output to generated-field agreement.
///
/// The original source ordinal, adjusted FnAbi ordinal and physical KIR slot
/// remain in `conditional`; `argument_index` is a generated ABI field ordinal.
/// This does not authenticate caller-supplied typed roots as belonging to the
/// same transaction. Only the transaction's owned binding path can supply that
/// custody. No allocation extent, runtime predicate, proof, descriptor packing
/// validity, artifact, or launch authority is established here.
#[derive(Debug)]
pub(crate) struct CompilerConditionalOutputDescriptorBindingV1<'a, 'owner> {
    conditional: &'a ProductionConditionalOutputBindingV1<'owner>,
    typed_root: &'a TypedDescriptorRootV1,
    argument_index: usize,
}

impl<'a, 'owner> CompilerConditionalOutputDescriptorBindingV1<'a, 'owner> {
    /// Retains the sealed owner/root/body/source/physical correspondence.
    pub(crate) const fn conditional(&self) -> &'a ProductionConditionalOutputBindingV1<'owner> {
        self.conditional
    }

    /// The exact borrowed typed root, not a reconstructed or authenticated copy.
    pub(crate) const fn typed_root(&self) -> &'a TypedDescriptorRootV1 {
        self.typed_root
    }

    /// Generated ABI field ordinal, never inferred from a physical KIR slot.
    pub(crate) const fn argument_index(&self) -> usize {
        self.argument_index
    }
}

/// Joins an existing sealed output binding to the complete typed-root roster.
///
/// Production must pass the transaction's own `expected_owner` and typed roots;
/// pointer equality below does not authenticate the supplied root slice. The
/// caller must already reserve `expected_owner.retained_analysis_storage_v1()`.
/// New scans and temporary logical maps are prepaid on the incoming ledger;
/// their scope restores the caller's storage floor. Existing source/SSA/typed-root
/// retention exclusions remain unchanged.
///
/// The sealed binding already checked source/KIR argument correspondence. This
/// query independently replays the admitted ABI map and transparent-body selection.
/// Tuple expansion, ignored/hidden inputs and context forwarding are unsupported;
/// no argument is compacted or skipped to manufacture a generated field index.
pub(crate) fn bind_conditional_output_descriptor_v1<'a, 'owner>(
    expected_owner: &'owner ProductionPreRankedKirOwnerV1,
    typed_roots: &'a [TypedDescriptorRootV1],
    binding: &'a ProductionConditionalOutputBindingV1<'owner>,
    budget: &mut Budget<'_>,
) -> Result<CompilerConditionalOutputDescriptorBindingV1<'a, 'owner>, Error> {
    budget.charge_work(8)?;
    if !std::ptr::eq(expected_owner, binding.owner()) {
        return Err(Error::ForeignOwner);
    }
    if budget.storage() < expected_owner.retained_analysis_storage_v1() {
        return Err(Resource::Accounting.into());
    }
    let semantic = expected_owner.semantic_ssa().source_semantic();
    let association = binding.association();
    let typed_root = select_typed_root_v1(
        typed_roots,
        semantic,
        association.correspondence_owner(),
        budget,
    )?;
    budget.charge_work(sum(&[
        16,
        typed_root.entry_symbol().len(),
        binding.coverage().kernel().id.as_str().len(),
    ])?)?;
    if typed_root.entry_symbol() != binding.coverage().kernel().id.as_str() {
        return Err(Error::Root);
    }
    // This descriptive API has no compiler context custody. A logical context
    // therefore remains inadmissible, even if its semantic type looks correct.
    let index = checked_generated_field_v1(
        semantic,
        (
            association.correspondence_owner(),
            association.semantic_function(),
        ),
        None,
        GeneratedFieldCoordinatesV1 {
            source: binding.source_argument(),
            adjusted: binding.adjusted_argument(),
            local: binding.semantic_local(),
            ty: binding.semantic_type(),
        },
        typed_root.arguments.len(),
        budget,
    )?;
    let semantic_type = semantic
        .types()
        .get(binding.semantic_type().index() as usize)
        .ok_or(Error::OutputArgument)?;
    let physical_index = usize::try_from(binding.coverage().output_parameter_index())
        .map_err(|_| Error::OutputArgument)?;
    let physical_type = binding
        .coverage()
        .function()
        .signature
        .parameters
        .get(physical_index)
        .ok_or(Error::OutputArgument)?;
    let argument_index =
        checked_output_field_v1(typed_root, index, semantic_type, physical_type, budget)?;
    Ok(CompilerConditionalOutputDescriptorBindingV1 {
        conditional: binding,
        typed_root,
        argument_index,
    })
}

pub(super) fn select_typed_root_v1<'a>(
    typed_roots: &'a [TypedDescriptorRootV1],
    semantic: &AdmittedInertSemanticMirV1,
    selected: SemanticFunctionIdV1,
    budget: &mut Budget<'_>,
) -> Result<&'a TypedDescriptorRootV1, Error> {
    budget.charge_work(8)?;
    if typed_roots.is_empty() || typed_roots.len() != semantic.roots().len() {
        return Err(Error::Root);
    }
    let mut found = None;
    // Admitted semantic roots already have unique kernel bindings. Exact
    // pairwise validation therefore also excludes duplicate/reordered typed
    // roots without another identity map or an allocation-backed roster copy.
    for (typed, root_id) in typed_roots.iter().zip(semantic.roots()) {
        budget.charge_work(8)?;
        let function = semantic
            .functions()
            .get(root_id.index() as usize)
            .ok_or(Error::Root)?;
        let entry = function.kernel_entry().ok_or(Error::Root)?;
        // Cover UTF-8/identity comparisons and both adjusted-argument scans in
        // the unchanged validator, plus its per-argument fixed-size checks.
        budget.charge_work(sum(&[
            64,
            typed.export_name.len(),
            product(entry.export_symbol().as_bytes().len(), 2)?,
            product(function.abi().arguments().len(), 8)?,
            product(typed.arguments.len(), 64)?,
        ])?)?;
        validate_production_v1_semantic_root_ownership_evidence(typed, semantic, function)?;
        require_flat_abi_v1(function.abi(), semantic.types(), budget)?;
        for argument in typed.arguments.as_slice() {
            budget.charge_work(1)?;
            if argument.kind.is_compiler_laid_out() {
                return Err(Error::UnsupportedAbi);
            }
        }
        if *root_id == selected && found.replace(typed).is_some() {
            return Err(Error::Root);
        }
    }
    found.ok_or(Error::Root)
}

pub(super) fn require_flat_abi_v1(
    abi: &SemanticFunctionAbiV1,
    types: &[SemanticTypeDeclV1],
    budget: &mut Budget<'_>,
) -> Result<(), Error> {
    budget.charge_work(8)?;
    if abi.c_variadic() || abi.arguments().len() != abi.source_input_types().len() {
        return Err(Error::UnsupportedAbi);
    }
    for (argument, source_type) in abi.arguments().iter().zip(abi.source_input_types()) {
        budget.charge_work(12)?;
        let ty = types
            .get(source_type.index() as usize)
            .ok_or(Error::UnsupportedAbi)?;
        if argument.role() != SemanticAbiArgumentRoleV1::Source
            || argument.ty() != *source_type
            || !matches!(
                argument.mode(),
                SemanticAbiPassModeV1::Direct(_) | SemanticAbiPassModeV1::Pair { .. }
            )
            || matches!(ty.shape(), SemanticTypeShapeV1::Tuple(_))
            || matches!(ty.rust_type_kind(), SemanticRustTypeKindV1::Execution(_))
        {
            return Err(Error::UnsupportedAbi);
        }
    }
    Ok(())
}

fn checked_output_field_v1(
    root: &TypedDescriptorRootV1,
    index: usize,
    semantic_type: &SemanticTypeDeclV1,
    physical_type: &Type,
    budget: &mut Budget<'_>,
) -> Result<usize, Error> {
    budget.charge_work(64)?;
    // Descriptor source_index is the contiguous generated field position. Keep
    // its wire bound, but do not construct a descriptor or infer an offset here.
    u16::try_from(index).map_err(|_| Error::OutputArgument)?;
    let argument = root
        .arguments
        .as_slice()
        .get(index)
        .ok_or(Error::OutputArgument)?;
    require_production_descriptor_argument_semantic_type_v1(argument, semantic_type.identity())?;
    if !matches!(argument.kind, DescriptorArgumentKindV1::DisjointSlice(_))
        || !production_descriptor_argument_matches_kernel_type_v1(
            argument.kind,
            argument.access,
            physical_type,
        )
    {
        return Err(Error::OutputArgument);
    }
    Ok(index)
}

fn sum(values: &[usize]) -> Result<usize, Resource> {
    values
        .iter()
        .try_fold(0usize, |total, value| total.checked_add(*value))
        .ok_or(Resource::Arithmetic)
}

fn product(left: usize, right: usize) -> Result<usize, Resource> {
    left.checked_mul(right).ok_or(Resource::Arithmetic)
}

#[cfg(test)]
#[path = "compiler_descriptor_conditional_output_binding_v1_tests.rs"]
mod tests;
