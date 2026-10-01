//! Mandatory original-source producer inside the existing Worker consumer.
//! The current nominal V31 request covers whole scalar CFGs. Preparation is
//! concrete source refinement input, not executed proof authority.
use super::*;
use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

type Capture<'a, 'b, 'v, 's, 'w, F> = (
    FinalInputs<'a, 'b, 'v, 's>,
    &'a mut Budget<'w>,
    &'a Cell<usize>,
    Pending<F>,
);

pub(super) fn headers<R, F>() -> Result<usize, Resource> {
    [
        size_of::<Capture<'_, '_, '_, '_, '_, F>>(),
        align_of::<Capture<'_, '_, '_, '_, '_, F>>(),
        size_of::<AssertUnwindSafe<Capture<'_, '_, '_, '_, '_, F>>>(),
        size_of::<Cell<usize>>(),
        size_of::<OriginalMir<'_, '_>>(),
        size_of::<Result<OriginalMir<'_, '_>, fe2o3_verifier::MixedOptimizerRefinementErrorV26>>(),
        size_of::<
            Result<
                fe2o3_verifier::OriginalSemanticMirRefinementSubjectV31,
                fe2o3_verifier::MixedOptimizerRefinementErrorV26,
            >,
        >(),
        size_of::<fe2o3_verifier::OriginalSemanticMirRefinementSubjectV31>(),
        size_of::<Subject>(),
        size_of::<Result<Subject, fe2o3_verifier::MixedOptimizerRelocationErrorV28>>(),
        2 * size_of::<Result<R, Error>>(),
        size_of::<std::thread::Result<Result<R, Error>>>(),
        2 * size_of::<Result<(), Error>>(),
        size_of::<(&Source<'_>, &Native<'_, '_, '_, '_>)>(),
        3 * size_of::<usize>(),
    ]
    .into_iter()
    .try_fold(0usize, |n, bytes| {
        n.checked_add(bytes).ok_or(Resource::Arithmetic)
    })
}

pub(super) fn consume<R, F>(
    inputs: FinalInputs<'_, '_, '_, '_>,
    budget: &mut Budget<'_>,
    consume: F,
) -> Result<R, Error>
where
    F: for<'a, 'v, 's, 'w> FnOnce(
        PreparedMixedPublicationV28<'a, 'v, 's>,
        &mut Budget<'w>,
    ) -> Result<R, Error>,
{
    let consume = Pending::new(consume);
    inputs.source.check_query_v18(budget)?;
    let source = inputs.source;
    let native = inputs.native;
    let floor = budget.storage();
    native.observe_retained_storage_v28(floor, budget)?;
    let accepted = Cell::new(0usize);
    let capture: Capture<'_, '_, '_, '_, '_, F> = (inputs, &mut *budget, &accepted, consume);
    let invoke = move || {
        let (inputs, budget, accepted, mut consume) = std::convert::identity(capture);
        let original =
            fe2o3_verifier::prepare_original_semantic_mir_refinement_v31(inputs.source, budget)
                .map_err(Error::OriginalMir)?;
        let retained = original.retained_storage();
        budget
            .reserve_storage(retained)
            .map_err(|error| inputs.source.retain_query_resource_error_v18(error))?;
        accepted.set(retained);
        let prepared = PreparedMixedPublicationV28 {
            inputs,
            original_mir: &original,
            required: budget.storage(),
        };
        prepared.replay(budget)?;
        let result = consume.take()(prepared, budget);
        // The borrowed candidate cannot escape F's higher-ranked callback.
        // Drop the actual generated request before settling its accepted credit.
        drop(original);
        result
    };
    #[cfg(test)]
    {
        assert_eq!(
            std::mem::size_of_val(&invoke),
            size_of::<Capture<'_, '_, '_, '_, '_, F>>()
        );
        assert_eq!(
            std::mem::align_of_val(&invoke),
            align_of::<Capture<'_, '_, '_, '_, '_, F>>()
        );
    }
    let selected = catch_unwind(AssertUnwindSafe(invoke));
    let expected = floor
        .checked_add(accepted.get())
        .ok_or(Resource::Arithmetic)?;
    let custody = native.observe_retained_storage_v28(expected, budget);
    let settled = custody.and_then(|()| {
        if budget.storage() != expected {
            return Err(source.retain_query_resource_error_v18(Resource::Accounting));
        }
        budget
            .release_storage(accepted.get())
            .map_err(|error| source.retain_query_resource_error_v18(error))
    });
    match selected {
        Ok(Ok(value)) => {
            settled?;
            Ok(value)
        }
        Ok(Err(error)) => {
            let _ = settled;
            Err(error)
        }
        Err(payload) => resume_unwind(payload),
    }
}
