//! Nominal witness moves and shared-reference transfers on original SSA locals.
//! Runtime metadata is transported from a current source value, never rebuilt
//! from its scalar representation or a physical target endpoint.
use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticDisjointIndexSpaceV1 as IndexSpace, SemanticMutabilityV1 as Mutability,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Transfer {
    destination: usize,
    input: usize,
    source_type: u32,
    reference: bool,
    moved: bool,
}

impl Transfer {
    pub(super) fn derive(
        context: &Context<'_, '_, '_>,
        assignment: &fe2o3_mir_model::semantic_mir_v1::SemanticAssignmentV1,
        out: &mut Writer<'_, '_>,
    ) -> Result<Option<Self>> {
        let Rvalue::Use(operand) = assignment.value().kind() else {
            return Ok(None);
        };
        out.budget.charge_work(12)?;
        let ty = operand.ty();
        let shape = context
            .types
            .get(ty.index() as usize)
            .ok_or_else(mismatch)?
            .shape();
        let (source_type, reference) = match shape {
            Shape::Pointer(pointer) => {
                let Some((_, _, _, space)) = context.slots.witness_class(pointer.pointee(), out)?
                else {
                    return Ok(None);
                };
                if pointer.kind() != PointerKind::Reference
                    || pointer.mutability() != Mutability::Immutable
                    || pointer.metadata() != PointerMetadata::None
                    || space != IndexSpace::Index1d
                {
                    return Err(unsupported());
                }
                (pointer.pointee(), true)
            }
            _ => {
                let Some((_, _, _, space)) = context.slots.witness_class(ty, out)? else {
                    return Ok(None);
                };
                if space != IndexSpace::Index1d {
                    return Err(unsupported());
                }
                (ty, false)
            }
        };
        let (input, moved) = match operand {
            Operand::Move(place) => (place, true),
            Operand::Copy(place) if reference => (place, false),
            // Compiler-issued witnesses are move-only. Constants cannot issue
            // either witness authority or a reference to that authority.
            Operand::Copy(_) | Operand::Constant(_) => return Err(unsupported()),
        };
        let destination = assignment.destination();
        if assignment.value().result_type() != ty
            || destination.ty() != ty
            || !destination.projections().is_empty()
            || !input.projections().is_empty()
            || context
                .function
                .locals()
                .get(input.local().index() as usize)
                .map(|local| local.ty())
                != Some(ty)
            || context
                .function
                .locals()
                .get(destination.local().index() as usize)
                .map(|local| local.ty())
                != Some(ty)
        {
            return Err(mismatch());
        }
        if context.descriptor(input.local().index(), out)?.is_some()
            || context
                .descriptor(destination.local().index(), out)?
                .is_some()
        {
            return Err(unsupported());
        }
        Ok(Some(Self {
            destination: context.local(destination.local().index())?,
            input: context.local(input.local().index())?,
            source_type: source_type.index(),
            reference,
            moved,
        }))
    }

    pub(super) fn emit(self, out: &mut Writer<'_, '_>) -> Result<()> {
        out.budget.charge_work(1)?;
        write!(out, "InvocationSourceByteEventV36::WitnessTransfer {{ destination: {}, input: {}, source_type: {}, reference: {}, moved: {} }}",
            self.destination, self.input, self.source_type, self.reference, self.moved)
            .map_err(|_| out.error())
    }
}

pub(super) fn headers() -> usize {
    size_of::<Transfer>()
        + size_of::<Result<Option<Transfer>>>()
        + size_of::<(TypeId, bool, bool, IndexSpace)>()
        + size_of::<(
            Option<(TypeId, u16, bool, IndexSpace)>,
            [usize; 4],
            [&(); 8],
        )>()
        + size_of::<(Result<()>, std::fmt::Result)>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mixed_optimizer_refinement_v26::SOURCE_LIMIT;
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrWorkBudgetV1 as Work,
    };

    fn emission(plan: Transfer, work: usize, storage: usize) -> (Result<String>, usize, usize) {
        let mut work = Work::new(work);
        let mut budget = Budget::new(&mut work, storage);
        let result = (|| {
            budget.reserve_storage(SOURCE_LIMIT + headers())?;
            let mut out = Writer::new(&mut budget)?;
            plan.emit(&mut out)?;
            out.finish()
        })();
        (result, budget.work(), budget.peak_storage())
    }

    #[test]
    fn original_mir_nominal_transfer_emission_has_independent_exact_resource_oracle() {
        for (reference, moved) in [(false, true), (true, true), (true, false)] {
            let plan = Transfer {
                destination: 13,
                input: 7,
                source_type: 5,
                reference,
                moved,
            };
            let expected = format!(
                "InvocationSourceByteEventV36::WitnessTransfer {{ destination: 13, input: 7, source_type: 5, reference: {reference}, moved: {moved} }}"
            );
            let work = 1 + expected.len();
            let storage = SOURCE_LIMIT + headers();
            let (result, actual_work, actual_storage) = emission(plan, work, storage);
            assert_eq!(result.unwrap(), expected);
            assert_eq!((actual_work, actual_storage), (work, storage));
            assert!(matches!(emission(plan, work - 1, storage).0,
                Err(Error::Resource(Resource::Work(error))) if error.limit() == work - 1 && error.actual() == work));
            assert!(matches!(emission(plan, work, storage - 1).0,
                Err(Error::Resource(Resource::Storage(error))) if error.limit() == storage - 1 && error.actual() == storage));
        }
    }

    #[test]
    fn original_mir_nominal_transfer_headers_account_for_complete_fixed_plan() {
        type Fields = (usize, usize, u32, bool, bool);
        assert_eq!(size_of::<Transfer>(), size_of::<Fields>());
        let expected = size_of::<Fields>()
            + size_of::<Result<Option<Fields>>>()
            + size_of::<(TypeId, bool, bool, IndexSpace)>()
            + size_of::<(
                Option<(TypeId, u16, bool, IndexSpace)>,
                [usize; 4],
                [&(); 8],
            )>()
            + size_of::<(Result<()>, std::fmt::Result)>();
        assert_eq!(headers(), expected);
    }
}
