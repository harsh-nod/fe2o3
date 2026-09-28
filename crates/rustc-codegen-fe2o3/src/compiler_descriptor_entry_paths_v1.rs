//! Borrowed structural paths joined to a completed packing schedule, not a codec.
use super::*;
use fe2o3_lower_mir_kernel::{
    ProductionArgumentCoverageV1 as SourceCoverage, ProductionArgumentNodeV1 as SourceNode,
    ProductionArgumentProjectionV1 as Projection,
};
use fe2o3_mir_model::semantic_mir_v1::SemanticTypeIdV1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Coverage {
    Zero,
    Slots { first: usize, end: usize },
    WithinSlot(usize),
}

// No detached constructor, Clone, stored callback or budget access. Even the
// source path borrow expires at the end of this one structural-node callback.
pub(super) struct Path<'node> {
    root: usize,
    argument: Argument,
    source_type: SemanticTypeIdentityV1,
    source_layout: SemanticLayoutIdentityV1,
    path: &'node [Projection],
    coverage: Coverage,
    components: &'node [Component],
}
impl Path<'_> {
    pub(super) const fn root(&self) -> usize {
        self.root
    }
    pub(super) const fn argument(&self) -> Argument {
        self.argument
    }
    pub(super) const fn source_type(&self) -> SemanticTypeIdentityV1 {
        self.source_type
    }
    pub(super) const fn source_layout(&self) -> SemanticLayoutIdentityV1 {
        self.source_layout
    }
    pub(super) fn source_path(&self) -> &[Projection] {
        self.path
    }
    pub(super) const fn coverage(&self) -> Coverage {
        self.coverage
    }
    pub(super) fn components(&self) -> &[Component] {
        self.components
    }
    pub(super) const fn grants_authority(&self) -> bool {
        false
    }
}

fn join<'a>(
    root: usize,
    row: Root,
    arguments: &'a [Argument],
    components: &'a [Component],
    semantic: &'a AdmittedInertSemanticMirV1,
    node: &'a SourceNode<'_>,
) -> R<Path<'a>> {
    let argument_index = row
        .first_argument
        .checked_add(node.source_argument() as usize)
        .ok_or(Resource::Arithmetic)?;
    let argument = *arguments
        .get(argument_index)
        .filter(|_| argument_index < row.end_argument)
        .ok_or(E::Mismatch("packing path logical argument"))?;
    let Some((local, local_path)) = node.local_binding() else {
        return Err(E::Mismatch("packing path original local"));
    };
    if argument.root != root
        || argument.ordinal != node.source_argument() as usize
        || argument.local != local
        || local_path != node.source_path()
    {
        return Err(E::Mismatch("packing path exact source projection"));
    }
    let ty = semantic
        .types()
        .get(node.semantic_type().index() as usize)
        .ok_or(E::Mismatch("packing path source type"))?;
    if node.source_path().is_empty()
        && (argument.source_type != ty.identity() || argument.source_layout != ty.layout_identity())
    {
        return Err(E::Mismatch("packing path whole argument identity"));
    }
    let all = components
        .get(argument.first_component..argument.end_component)
        .ok_or(E::Mismatch("packing path component range"))?;
    let (coverage, first, end, parameter) = match node.coverage() {
        SourceCoverage::Zero => (
            Coverage::Zero,
            argument.first_slot,
            argument.first_slot,
            None,
        ),
        SourceCoverage::Components { first, end } => {
            (Coverage::Slots { first, end }, first, end, None)
        }
        SourceCoverage::Parameter(entry) => {
            let first = entry.slot();
            let end = first.checked_add(1).ok_or(Resource::Arithmetic)?;
            (
                Coverage::Slots { first, end },
                first,
                end,
                Some((entry, false)),
            )
        }
        SourceCoverage::WithinAtomicParameter(entry) => {
            let first = entry.slot();
            let end = first.checked_add(1).ok_or(Resource::Arithmetic)?;
            (Coverage::WithinSlot(first), first, end, Some((entry, true)))
        }
    };
    let selected = if coverage == Coverage::Zero {
        &all[..0]
    } else {
        if first < argument.first_slot || end > argument.end_slot || first >= end {
            return Err(E::Mismatch("packing path exact physical slots"));
        }
        let begin = all.partition_point(|row| row.slot < first);
        let finish = all.partition_point(|row| row.slot < end);
        let selected = &all[begin..finish];
        if selected.first().is_none_or(|row| row.slot != first)
            || selected
                .last()
                .is_none_or(|row| row.slot.checked_add(1) != Some(end))
        {
            return Err(E::Mismatch("packing path dense physical coverage"));
        }
        selected
    };
    if let Some((entry, contained)) = parameter {
        // One physical slot has at most pointer+length in the completed schedule.
        if selected.len() > 2
            || selected.iter().any(|row| {
                row.value != entry.value() || (!contained && row.source_type != ty.identity())
            })
        {
            return Err(E::Mismatch("packing path exact physical leaf"));
        }
    }
    Ok(Path {
        root,
        argument,
        source_type: ty.identity(),
        source_layout: ty.layout_identity(),
        path: node.source_path(),
        coverage,
        components: selected,
    })
}

fn node_work(components: usize) -> R<usize> {
    let bits = usize::BITS as usize - components.leading_zeros() as usize;
    // Fixed identities/schema joins include up to four 32-byte comparisons.
    // Two binary partitions each have at most bit_width(n)+1 comparisons.
    256usize
        .checked_add(
            bits.checked_add(1)
                .ok_or(Resource::Arithmetic)?
                .checked_mul(8)
                .ok_or(Resource::Arithmetic)?,
        )
        .ok_or(Resource::Arithmetic.into())
}

type Run<'a, 'owner, 'work, F> = (
    OwnerRef<'owner>,
    &'owner [TypedDescriptorRootV1],
    ProductionAmdTargetProfileV1,
    &'a [Root],
    &'a [Argument],
    &'a [Component],
    &'a Cell<Option<Resource>>,
    &'a mut Budget<'work>,
    F,
);
type RootCapture<'a, F> = (
    &'a usize,
    &'a Root,
    &'a &'a [Argument],
    &'a &'a [Component],
    &'a &'a AdmittedInertSemanticMirV1,
    &'a &'a Module,
    &'a &'a Cell<Option<Resource>>,
    &'a mut F,
);
type QueryCapture<'a, 'owner, 'work, F> = (
    &'a OwnerRef<'owner>,
    &'a &'owner [TypedDescriptorRootV1],
    &'a &'a [Root],
    &'a &'a [Argument],
    &'a &'a [Component],
    &'a &'a Cell<Option<Resource>>,
    &'a mut Budget<'work>,
    &'a mut F,
);
type NodeCapture<'a, F> = (
    usize,
    Root,
    &'a [Argument],
    &'a [Component],
    &'a AdmittedInertSemanticMirV1,
    &'a Cell<Option<Resource>>,
    &'a mut F,
    &'a mut Option<E>,
);
fn headers<F>() -> R<usize> {
    sum_frames(&[
        frame::<F>(),
        align_of::<F>(),
        frame::<Run<'_, '_, '_, F>>(),
        align_of::<Run<'_, '_, '_, F>>(),
        frame::<AssertUnwindSafe<Run<'_, '_, '_, F>>>(),
        frame::<RootCapture<'_, F>>(),
        frame::<QueryCapture<'_, '_, '_, F>>(),
        align_of::<QueryCapture<'_, '_, '_, F>>(),
        frame::<NodeCapture<'_, F>>(),
        frame::<(&AdmittedInertSemanticMirV1, &Module)>(),
        frame::<&Module>(),
        frame::<&fe2o3_lower_mir_kernel::ProductionSemanticKirOwnerV1>(),
        frame::<&fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1>(),
        frame::<&fe2o3_pliron::ProductionSemanticMirOwnerV1>(),
        frame::<&fe2o3_pliron::ProductionSemanticSsaOwnerV1>(),
        frame::<&fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12>(),
        frame::<&AdmittedInertSemanticMirV1>(),
        frame::<&[u8]>(),
        frame::<&[SemanticFunctionIdV1]>(),
        frame::<SemanticFunctionIdV1>(),
        frame::<fe2o3_mir_model::semantic_mir_v1::SemanticKernelBodySelectionV1>(),
        frame::<Option<fe2o3_mir_model::semantic_mir_v1::SemanticKernelBodySelectionV1>>(),
        frame::<std::iter::Enumerate<std::slice::Iter<'_, Root>>>(),
        frame::<(usize, &Root)>(),
        frame::<Option<(usize, &Root)>>(),
        frame::<(
            usize,
            &TypedDescriptorRootV1,
            &AdmittedInertSemanticMirV1,
            &mut Plan<'_, '_, '_>,
        )>(),
        frame::<(
            usize,
            Root,
            &[Argument],
            &[Component],
            &AdmittedInertSemanticMirV1,
            &SourceNode<'_>,
        )>(),
        frame::<Path<'_>>(),
        frame::<Coverage>(),
        frame::<Root>(),
        frame::<Argument>(),
        frame::<&Root>(),
        frame::<&Argument>(),
        frame::<&Component>(),
        frame::<&SourceNode<'_>>(),
        frame::<SourceNode<'_>>(),
        frame::<SourceCoverage<'_>>(),
        frame::<&[Root]>(),
        frame::<&[Argument]>(),
        3 * frame::<&[Component]>(),
        frame::<Option<&Root>>(),
        frame::<Option<&Argument>>(),
        frame::<(&usize, &Root, &&Argument)>(),
        2 * frame::<Option<&Component>>(),
        frame::<Option<&[Component]>>(),
        frame::<Option<&SemanticTypeDeclV1>>(),
        frame::<&SemanticTypeDeclV1>(),
        frame::<&[SemanticTypeDeclV1]>(),
        frame::<SemanticTypeIdV1>(),
        frame::<SemanticTypeIdentityV1>(),
        frame::<SemanticLayoutIdentityV1>(),
        2 * frame::<&[Projection]>(),
        frame::<(SemanticLocalIdV1, &[Projection])>(),
        frame::<Option<(SemanticLocalIdV1, &[Projection])>>(),
        frame::<fe2o3_lower_mir_kernel::ProductionPhysicalArgumentV1<'_>>(),
        frame::<
            Option<(
                fe2o3_lower_mir_kernel::ProductionPhysicalArgumentV1<'_>,
                bool,
            )>,
        >(),
        frame::<(
            Coverage,
            usize,
            usize,
            Option<(
                fe2o3_lower_mir_kernel::ProductionPhysicalArgumentV1<'_>,
                bool,
            )>,
        )>(),
        frame::<std::slice::Iter<'_, Component>>(),
        frame::<(&usize, &Component)>(),
        frame::<(
            &fe2o3_lower_mir_kernel::ProductionPhysicalArgumentV1<'_>,
            &bool,
            &&SemanticTypeDeclV1,
        )>(),
        frame::<(&Root, &Option<E>)>(),
        frame::<Option<E>>(),
        frame::<&mut Option<E>>(),
        frame::<&E>(),
        frame::<&Resource>(),
        frame::<Option<Resource>>(),
        frame::<&Cell<Option<Resource>>>(),
        frame::<&mut F>(),
        frame::<&mut Plan<'_, '_, '_>>(),
        frame::<(&mut Budget<'_>,)>(),
        frame::<Option<usize>>(),
        frame::<std::ops::Range<usize>>(),
        frame::<Result<(), SourceError>>(),
        frame::<&R<()>>(),
        frame::<&Result<R<()>, Box<dyn std::any::Any + Send>>>(),
        frame::<Result<R<()>, Box<dyn std::any::Any + Send>>>(),
        16 * frame::<usize>(),
        4 * frame::<bool>(),
    ])
}

impl Schedule<'_, '_, '_> {
    /// Reborrows checked source structure from an already complete immutable
    /// schedule. No field/index path, node or component borrow can escape.
    pub(super) fn visit_paths<F>(&mut self, visit: F) -> R<()>
    where
        F: for<'node> FnMut(Path<'node>) -> R<()>,
    {
        self.check()?;
        let floor = self.budget.storage();
        let bytes = match headers::<F>() {
            Ok(bytes) => bytes,
            Err(error) => return self.retain(Err(error)),
        };
        let reserved = self.budget.reserve_storage(bytes).map_err(E::Resource);
        self.retain(reserved)?;
        let required = self.budget.storage();
        let state = (
            self.owner,
            self.captured,
            self.profile,
            self.roots,
            self.arguments,
            self.components,
            &self.first,
            &mut *self.budget,
            visit,
        );
        let result = catch_unwind(AssertUnwindSafe(move || {
            let (owner, captured, _profile, roots, arguments, components, first, budget, mut visit) =
                state;
            let result = (|| {
                // The private schedule retains the exact immutable owner/capture/
                // target binding checked for every root before any consumer ran.
                // Only source argument scratch is rebuilt here, not P4/formal work.
                budget.charge_work(16)?;
                let (semantic, module) = match owner {
                    OwnerRef::Direct(value) => {
                        let source = value.source_semantic_kir();
                        (source.semantic().semantic(), source.module())
                    }
                    OwnerRef::Erased(value) => {
                        let source = value.original_source();
                        (
                            source.semantic_ssa().source_semantic(),
                            source.executable().module(),
                        )
                    }
                };
                if roots.len() != captured.len() || roots.len() != semantic.roots().len() {
                    return Err(E::Mismatch("packing path complete root roster"));
                }
                for (index, &row) in roots.iter().enumerate() {
                    budget.charge_work(semantic.canonical_encoding().len())?;
                    let root = semantic.roots()[index];
                    let selected_body = semantic
                        .select_kernel_body_for_root_v1(root)
                        .ok_or(E::Mismatch("packing path exact source body"))?;
                    let result = with_plan(owner, root, selected_body.body(), budget, |plan| {
                        plan.check_subject(semantic, module)?;
                        let mut selected = None;
                        let result = plan.visit_nodes_with_work(
                            node_work(row.end_component - row.first_component)?,
                            2,
                            |node| match join(index, row, arguments, components, semantic, &node)
                                .and_then(&mut visit)
                            {
                                Ok(()) => Ok(()),
                                Err(error) => {
                                    if let Some(error) = resource(&error) {
                                        first.set(first.get().or(Some(error)));
                                    }
                                    selected = Some(error);
                                    Err(SourceError::CorrespondenceMismatch)
                                }
                            },
                        );
                        match (result, selected) {
                            (Err(error @ SourceError::ArgumentCorrespondenceResource(_)), _) => {
                                Err(E::Source(error))
                            }
                            (Err(_), Some(error)) => Err(error),
                            (Err(error), None) => Err(E::Source(error)),
                            (Ok(()), None) => Ok(()),
                            (Ok(()), Some(_)) => {
                                Err(E::Mismatch("packing path callback completion"))
                            }
                        }
                    });
                    if let Err(error) = &result
                        && let Some(error) = resource(error)
                    {
                        first.set(first.get().or(Some(error)));
                    }
                    result?;
                }
                Ok(())
            })();
            if let Err(error) = &result
                && let Some(error) = resource(error)
            {
                first.set(first.get().or(Some(error)));
            }
            drop(visit);
            result
        }));
        let valid = self.slot == self.budget as *const Budget<'_> as usize
            && self.ledger == self.budget.work_ledger_identity_v1()
            && self.budget.storage() >= required;
        if let Ok(Err(error)) = &result
            && let Some(error) = resource(error)
        {
            self.first.set(self.first.get().or(Some(error)));
        }
        if valid {
            let released = self
                .budget
                .release_storage(self.budget.storage() - floor)
                .map_err(E::Resource);
            if let Err(error) = released
                && let Some(error) = resource(&error)
            {
                self.first.set(self.first.get().or(Some(error)));
            }
        } else {
            self.first
                .set(self.first.get().or(Some(Resource::Accounting)));
        }
        match result {
            Err(payload) => resume_unwind(payload),
            Ok(result) => self.retain(result),
        }
    }
}

#[cfg(test)]
#[path = "compiler_descriptor_entry_paths_v1_tests.rs"]
pub(super) mod tests;
