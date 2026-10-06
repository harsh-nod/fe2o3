//! Exact borrowed source/reference binding, not functional verification.
//!
//! The CPU replay validates its independent retained body/effects. It does not
//! discharge its bounds or compare any GPU execution. Nonempty sets therefore
//! remain unconditionally refused by `require_discharged`.
use super::*;
use crate::reference_effect_v1::{
    AuthenticatedReferenceEffectBindingV1 as Reference, ReferenceBindingErrorV1,
    ReferenceOutputCoordinateV1,
};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticFunctionDeclV1, SemanticFunctionIdV1, SemanticLocalRoleV1,
};
use fe2o3_pliron::ProductionSemanticSsaOwnerV1;
use fe2o3_verifier::portable_reference_v1::ReplayedCpuEffectsV1;
use std::mem::size_of_val;

// Four length-prefixed fields: local domain, function, complete MIR body and
// raw local number; then compare the resulting 32-byte identity.
const RETURN_IDENTITY_WORK: usize =
    4 * 8 + b"fe2o3/semantic-mir/rustc-local/v1".len() + 32 + 32 + 4 + 32;

#[derive(Debug)]
pub(crate) enum ReferenceObligationErrorV69 {
    Binding(&'static str),
    Replay(ReferenceBindingErrorV1),
}

impl std::fmt::Display for ReferenceObligationErrorV69 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Binding(reason) => formatter.write_str(reason),
            Self::Replay(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ReferenceObligationErrorV69 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Binding(_) => None,
            Self::Replay(error) => Some(error),
        }
    }
}

fn binding(reason: &'static str) -> Error {
    Error::ReferenceObligations(ReferenceObligationErrorV69::Binding(reason))
}

struct Root<'a> {
    ordinal: usize,
    semantic_root: SemanticFunctionIdV1,
    body: &'a SemanticFunctionDeclV1,
    descriptor: &'a crate::compiler_descriptor::TypedDescriptorRootV1,
}

struct Obligation<'a> {
    root: usize,
    reference: &'a Reference,
}

/// Borrowing both owners prevents an equal-byte donor from replacing either
/// side. No verified receipt or executable graph can be obtained from this type.
struct Unproved<'a> {
    source: &'a ProductionSemanticSsaOwnerV1,
    bindings: &'a AuthenticatedProductionBindings,
    roots: Vec<Root<'a>>,
    obligations: Vec<Obligation<'a>>,
    slot: usize,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    required: usize,
}

fn headers<R>() -> Result<usize, Resource> {
    [
        size_of::<Unproved<'_>>(),
        align_of::<Unproved<'_>>(),
        size_of::<Vec<Root<'_>>>(),
        size_of::<Vec<Obligation<'_>>>(),
        size_of::<Vec<bool>>(),
        size_of::<Result<R, Error>>(),
        align_of::<Result<R, Error>>(),
        size_of::<(
            &ProductionSemanticSsaOwnerV1,
            &AuthenticatedProductionBindings,
        )>(),
        size_of::<crate::rustc_semantic_adapter_v1::SemanticIdentityDigestV1>(),
        align_of::<crate::rustc_semantic_adapter_v1::SemanticIdentityDigestV1>(),
        size_of::<[&[u8]; 3]>(),
        size_of::<[u8; 4]>(),
        3 * size_of::<[u8; 32]>(),
    ]
    .into_iter()
    .try_fold(0usize, |n, bytes| {
        n.checked_add(bytes).ok_or(Resource::Arithmetic)
    })
}

impl Unproved<'_> {
    fn check(
        &self,
        source: &ProductionSemanticSsaOwnerV1,
        bindings: &AuthenticatedProductionBindings,
        budget: &mut Budget<'_>,
    ) -> Result<(), Error> {
        if self.slot != std::ptr::from_ref(budget) as usize
            || self.ledger != budget.work_ledger_identity_v1()
        {
            return Err(Resource::Accounting.into());
        }
        budget.check_prior_denials_v1()?;
        budget.charge_work(5)?;
        if budget.storage() < self.required {
            return Err(Resource::Accounting.into());
        }
        if !std::ptr::eq(self.source, source) || !std::ptr::eq(self.bindings, bindings) {
            return Err(binding("source reference obligation owner changed"));
        }
        Ok(())
    }

    fn with_replayed<R>(
        &self,
        index: usize,
        budget: &mut Budget<'_>,
        consume: impl for<'cpu> FnOnce(&'cpu ReplayedCpuEffectsV1, &mut Budget<'_>) -> Result<R, Error>,
    ) -> Result<R, Error> {
        self.check(self.source, self.bindings, budget)?;
        let floor = budget.storage();
        let scratch = replay_headers::<R>()?
            .checked_add(size_of_val(&consume))
            .ok_or(Resource::Arithmetic)?;
        budget.with_prepaid_scope(floor, 1, scratch, scratch, |budget| {
            let row = self
                .obligations
                .get(index)
                .ok_or_else(|| binding("source reference obligation index absent"))?;
            let root = self
                .roots
                .get(row.root)
                .ok_or_else(|| binding("source reference obligation root absent"))?;
            budget.charge_work(2)?;
            if self.source.source_semantic().roots().get(root.ordinal) != Some(&root.semantic_root)
            {
                return Err(binding("source reference original root ordinal changed"));
            }
            check_function(row.reference, root.body, budget)?;
            let result = row
                .reference
                .with_replayed_output_writes_v1(budget, |effects, budget| {
                    budget.charge_work(effects.writes.len())?;
                    if effects.writes.is_empty()
                        || effects.writes.iter().any(|write| {
                            !matches!(
                                write.coordinate,
                                ReferenceOutputCoordinateV1::LogicalPoint(_)
                            )
                        })
                    {
                        return Err(binding(
                            "source reference requires scalar logical-point writes",
                        ));
                    }
                    consume(effects, budget)
                });
            // The portable replay preserves the original sticky account but its
            // diagnostic error is textual. Return the actual typed denial first.
            self.check(self.source, self.bindings, budget)?;
            result.map_err(|error| {
                Error::ReferenceObligations(ReferenceObligationErrorV69::Replay(error))
            })?
        })
    }
}

fn replay_headers<R>() -> Result<usize, Resource> {
    [
        size_of::<(&Unproved<'_>, usize)>(),
        size_of::<Result<R, Error>>(),
        align_of::<Result<R, Error>>(),
        size_of::<Result<Result<R, Error>, ReferenceBindingErrorV1>>(),
        align_of::<Result<Result<R, Error>, ReferenceBindingErrorV1>>(),
    ]
    .into_iter()
    .try_fold(0usize, |sum, bytes| {
        sum.checked_add(bytes).ok_or(Resource::Arithmetic)
    })
}

fn check_function(
    reference: &Reference,
    body: &SemanticFunctionDeclV1,
    budget: &mut Budget<'_>,
) -> Result<(), Error> {
    budget.check_prior_denials_v1()?;
    budget.charge_work(5 * 32 + 4)?;
    let identity = &reference.kernel;
    if body.identity().as_bytes() != &identity.function_sha256
        || body.item_definition_identity().as_bytes() != &identity.item_definition_sha256
        || body.monomorphization_identity().as_bytes() != &identity.monomorphization_sha256
        || body.generic_type_arguments_identity().as_bytes()
            != &identity.generic_type_arguments_sha256
        || body.const_generic_arguments_identity().as_bytes()
            != &identity.const_generic_arguments_sha256
        || body.abi().source_signature().inputs().len()
            != reference.signature_preimage.kernel_inputs().len()
    {
        return Err(binding(
            "source reference kernel identity or signature arity changed",
        ));
    }
    // Canonical locals are sorted by identity and may include normalization
    // temporaries. The importer preserves the unique rustc Return local (raw
    // local zero), whose identity commits to the complete original MIR body.
    // This binds that commitment to an already authenticated source owner; it
    // is not validation of a caller-constructed semantic function.
    budget.charge_work(
        body.locals()
            .len()
            .checked_add(RETURN_IDENTITY_WORK)
            .ok_or(Resource::Arithmetic)?,
    )?;
    let mut returns = body
        .locals()
        .iter()
        .filter(|local| local.role() == SemanticLocalRoleV1::Return);
    let return_local = returns
        .next()
        .ok_or_else(|| binding("source reference original return local absent"))?;
    if returns.next().is_some()
        || return_local.identity()
            != crate::rustc_semantic_adapter_v1::rustc_local_identity_v1(
                body.identity(),
                identity.rustc_mir_body_sha256,
                0,
            )
    {
        return Err(binding(
            "source reference original return/body identity changed",
        ));
    }
    Ok(())
}

fn with_unproved<R>(
    source: &ProductionSemanticSsaOwnerV1,
    bindings: &AuthenticatedProductionBindings,
    budget: &mut Budget<'_>,
    consume: impl for<'borrow> FnOnce(&Unproved<'borrow>, &mut Budget<'_>) -> Result<R, Error>,
) -> Result<R, Error> {
    budget.check_prior_denials_v1()?;
    let floor = budget.storage();
    let scratch = headers::<R>()?
        .checked_add(size_of_val(&consume))
        .ok_or(Resource::Arithmetic)?;
    budget.with_prepaid_scope(floor, 1, scratch, scratch, |budget| {
        let semantic = source.source_semantic();
        let count = semantic.roots().len();
        budget.charge_work(4)?;
        if count == 0
            || bindings.typed_descriptor_roots.len() != count
            || bindings
                .rustc_preflight_plan
                .rustc_identity_inventory_sha256()
                != bindings.rustc_identity_inventory.sha256()
        {
            return Err(binding(
                "source reference original root or importer roster changed",
            ));
        }
        let mut roots = paid_vec::<Root<'_>>(count, budget)?;
        let mut names = 0usize;
        for (ordinal, (root, descriptor)) in semantic
            .roots()
            .iter()
            .zip(&bindings.typed_descriptor_roots)
            .enumerate()
        {
            // The existing structured selector scans only admitted source; this
            // conservative debit covers its bounded wrapper/body inspection.
            budget.charge_work(semantic.canonical_encoding().len())?;
            let selected = semantic
                .select_kernel_body_for_root_v1(*root)
                .ok_or_else(|| binding("source reference kernel body selection refused"))?;
            let function = semantic
                .functions()
                .get(root.index() as usize)
                .ok_or_else(|| binding("source reference original root absent"))?;
            let entry = function
                .kernel_entry()
                .ok_or_else(|| binding("source reference original entry absent"))?;
            budget.charge_work(
                descriptor
                    .entry_symbol()
                    .len()
                    .checked_add(32)
                    .ok_or(Resource::Arithmetic)?,
            )?;
            if entry.kernel_binding_identity().as_bytes() != &descriptor.kernel_binding_bytes()
                || entry.export_symbol().as_bytes() != descriptor.entry_symbol().as_bytes()
            {
                return Err(binding("source reference typed ABI root changed"));
            }
            names = names
                .checked_add(
                    descriptor
                        .logical_name()
                        .len()
                        .checked_add(32)
                        .ok_or(Resource::Arithmetic)?,
                )
                .ok_or(Resource::Arithmetic)?;
            roots.push(Root {
                ordinal,
                semantic_root: *root,
                body: &semantic.functions()[selected.body().index() as usize],
                descriptor,
            });
        }
        let depth = usize::BITS as usize - count.leading_zeros() as usize;
        budget.charge_work(
            names
                .checked_mul(depth.checked_add(1).ok_or(Resource::Arithmetic)?)
                .and_then(|n| n.checked_mul(128))
                .ok_or(Resource::Arithmetic)?,
        )?;
        roots.sort_unstable_by_key(|row| (row.body.identity(), row.descriptor.logical_name()));
        if roots.windows(2).any(|pair| {
            pair[0].body.identity() == pair[1].body.identity()
                && pair[0].descriptor.logical_name() == pair[1].descriptor.logical_name()
        }) {
            return Err(binding("source reference root assignment is ambiguous"));
        }
        let references = bindings.reference_effect_bindings.as_slice();
        let mut obligations = paid_vec::<Obligation<'_>>(references.len(), budget)?;
        let mut seen = paid_vec::<bool>(count, budget)?;
        seen.resize(count, false);
        for reference in references {
            let key = (
                fe2o3_mir_model::semantic_mir_v1::SemanticFunctionIdentityV1::from_sha256(
                    reference.kernel.function_sha256,
                ),
                reference.logical_kernel_name.as_str(),
            );
            budget.charge_work(
                reference
                    .logical_kernel_name
                    .len()
                    .checked_add(32)
                    .and_then(|n| n.checked_mul(depth + 2))
                    .ok_or(Resource::Arithmetic)?,
            )?;
            let index = roots
                .binary_search_by_key(&key, |row| {
                    (row.body.identity(), row.descriptor.logical_name())
                })
                .map_err(|_| {
                    binding("source reference binding is outside the original root roster")
                })?;
            if std::mem::replace(&mut seen[index], true) {
                return Err(binding(
                    "source reference binding duplicates an original root",
                ));
            }
            check_function(reference, roots[index].body, budget)?;
            obligations.push(Obligation {
                root: index,
                reference,
            });
        }
        let pending = Unproved {
            source,
            bindings,
            roots,
            obligations,
            slot: std::ptr::from_ref(budget) as usize,
            ledger: budget.work_ledger_identity_v1(),
            required: budget.storage(),
        };
        let result = consume(&pending, budget);
        pending.check(source, bindings, budget)?;
        drop(pending);
        drop(seen);
        result
    })
}

#[cfg(test)]
#[path = "production_pipeline_source_reference_obligations_v69_tests.rs"]
pub(super) mod tests;

pub(super) fn require_discharged(
    source: &ProductionSemanticSsaOwnerV1,
    bindings: &AuthenticatedProductionBindings,
    budget: &mut Budget<'_>,
) -> Result<(), Error> {
    if bindings.reference_effect_bindings.as_slice().is_empty() {
        return Ok(());
    }
    with_unproved(source, bindings, budget, |pending, budget| {
        for index in 0..pending.obligations.len() {
            pending.with_replayed(index, budget, |_, _| Ok(()))?;
        }
        Err(Error::Unsupported(
            "source-owned scalar reference obligations",
        ))
    })
}

/// All rows are borrowed from the original authenticated registration owner.
/// The callback must retain these inert obligations through actual proof
/// execution; preparing or replaying this list does not discharge any row.
pub(super) fn with_inputs<R>(
    source: &ProductionSemanticSsaOwnerV1,
    bindings: &AuthenticatedProductionBindings,
    budget: &mut Budget<'_>,
    consume: impl for<'rows> FnOnce(
        &'rows [fe2o3_verifier::SourceScalarReferenceInputV69<'rows>],
        &mut Budget<'_>,
    ) -> Result<R, Error>,
) -> Result<R, Error> {
    use fe2o3_verifier::{
        SourceScalarReferenceInputV69 as Input,
        portable_reference_v1::{ReferenceArgumentRelationV1 as Relation, ReferenceReplayInputV1},
    };
    if bindings.reference_effect_bindings.as_slice().is_empty() {
        let floor = budget.storage();
        let headers = size_of_val(&consume)
            .checked_add(size_of::<Result<R, Error>>())
            .and_then(|n| n.checked_add(align_of::<Result<R, Error>>()))
            .ok_or(Resource::Arithmetic)?;
        return budget
            .with_prepaid_scope(floor, 1, headers, headers, |budget| consume(&[], budget));
    }
    with_unproved(source, bindings, budget, |pending, budget| {
        budget.reserve_storage(input_headers::<R>()?)?;
        let mut rows = paid_vec(pending.obligations.len(), budget)?;
        for obligation in &pending.obligations {
            pending.check(source, bindings, budget)?;
            let root = pending
                .roots
                .get(obligation.root)
                .ok_or_else(|| binding("reference original root absent during input capture"))?;
            let reference = obligation.reference;
            check_function(reference, root.body, budget)?;
            for relation in &reference.effect_ir.relations {
                budget.charge_work(1)?;
                if let Relation::DisjointOutputCoordinate { argument, .. } = relation {
                    crate::compiler_descriptor::source_owned_v29::require_reference_index1d_v69(
                        root.descriptor,
                        *argument,
                        budget,
                    )?;
                }
            }
            // Construct one complete inert row, then move it to retained
            // backing. This includes identities and borrowed slice headers.
            budget.charge_work(2 * size_of::<Input<'_>>())?;
            rows.push(Input {
                original_root: root.ordinal,
                kernel: reference.kernel.clone(),
                reference: reference.reference.clone(),
                replay: ReferenceReplayInputV1 {
                    signature_preimage: &reference.signature_preimage,
                    effect_ir: &reference.effect_ir,
                    effect_ir_sha256: reference.effect_ir_sha256,
                    observable_output_writes: &reference.observable_output_writes,
                },
            });
        }
        let result = consume(&rows, budget);
        pending.check(source, bindings, budget)?;
        drop(rows);
        result
    })
}

fn input_headers<R>() -> Result<usize, Resource> {
    use fe2o3_verifier::SourceScalarReferenceInputV69 as Input;
    [
        size_of::<Vec<Input<'_>>>(),
        2 * size_of::<Input<'_>>(),
        align_of::<Input<'_>>(),
        size_of::<Result<R, Error>>(),
        align_of::<Result<R, Error>>(),
        6 * size_of::<&()>(),
    ]
    .into_iter()
    .try_fold(0usize, |sum, bytes| {
        sum.checked_add(bytes).ok_or(Resource::Arithmetic)
    })
}
