//! Private progression from retained expanded inputs to scoped support text.
//! This is not initialization/step/trace proof or a publication route.
use super::*;
use fe2o3_verifier::{ExpandedSupportModelV280, ExpandedSupportRuntimeV280};

fn observe_model<R: 'static, F>(
    source: &Source<'_>,
    original: &Original<'_>,
    tile: &Expanded<'_, '_>,
    pair: &ExpandedPairInputV279<'_, '_, '_, '_>,
    budget: &mut Budget<'_>,
    consume: F,
) -> Result<(R, usize), Error>
where
    F: for<'model, 'source, 'work> FnOnce(
        &ExpandedSupportModelV280<'model, 'source>,
        &mut Budget<'work>,
    ) -> Result<(R, usize), Error>,
{
    let mut pending = formal_context_v19::PendingConsumerV19::new(consume);
    pair.check(source, original, tile, budget)?;
    let floor = budget.storage();
    let mut consumer_error = None;
    let prepare = |budget: &mut Budget<'_>| {
        let rows = pair.roots(budget)?;
        let bytes = rows
            .len()
            .checked_mul(size_of::<ExpandedSupportRuntimeV280>())
            .ok_or(Resource::Arithmetic)?;
        budget.reserve_storage(bytes)?;
        let mut runtime = Vec::new();
        runtime
            .try_reserve_exact(rows.len())
            .map_err(|_| Resource::Allocation)?;
        budget.reserve_storage(
            runtime
                .capacity()
                .checked_mul(size_of::<ExpandedSupportRuntimeV280>())
                .and_then(|actual| actual.checked_sub(bytes))
                .ok_or(Resource::Accounting)?,
        )?;
        for row in rows {
            budget.charge_work(1)?;
            runtime.push(ExpandedSupportRuntimeV280 {
                source_root: row.source_root,
                source_launch: row.source_launch,
                source_layout: row.source_layout,
                physical: row.launch,
            });
        }
        let (width, endian) = pair.runtime(budget)?;
        let result = fe2o3_verifier::with_expanded_support_model_v280(
            source,
            original,
            tile,
            &runtime,
            width,
            endian,
            budget,
            |model, budget| {
                pair.check(source, original, tile, budget)?;
                let outcome = catch_unwind(AssertUnwindSafe(|| pending.take()(model, budget)));
                let checked = pair.check(source, original, tile, budget);
                match outcome {
                    Err(payload) => resume_unwind(payload),
                    Ok(Err(error)) => Err(error),
                    Ok(Ok(value)) => match checked {
                        Ok(()) => Ok(value),
                        Err(error) => {
                            formal_context_v19::discard(value);
                            Err(error)
                        }
                    },
                }
            },
        )
        .map_err(Error::ExpandedModel)?;
        match result {
            Ok(value) => Ok(formal_context_v19::PendingConsumerV19::new(value)),
            Err(error) => {
                consumer_error = Some(error);
                Err(Error::Unsupported("expanded model consumer refused"))
            }
        }
    };
    let headers = [
        2 * std::mem::size_of_val(&prepare),
        std::mem::align_of_val(&prepare),
        size_of::<Vec<ExpandedSupportRuntimeV280>>(),
        2 * size_of::<ExpandedSupportRuntimeV280>(),
        size_of::<formal_context_v19::PendingConsumerV19<F>>(),
        2 * size_of::<formal_context_v19::PendingConsumerV19<(R, usize)>>(),
        2 * size_of::<Result<(R, usize), Error>>(),
        size_of::<Option<Error>>(),
        size_of::<std::thread::Result<Result<(R, usize), Error>>>(),
        size_of::<fe2o3_kernel_ir::FormalIndexWidth>(),
        size_of::<fe2o3_kernel_ir::EndiannessV2>(),
        8 * size_of::<usize>(),
    ]
    .into_iter()
    .try_fold(0usize, |sum, bytes| {
        sum.checked_add(bytes).ok_or(Resource::Arithmetic)
    })?;
    let result = budget.with_prepaid_scope(floor, 1, 1, headers, prepare);
    if let Some(error) = consumer_error {
        return Err(error);
    }
    let mut payload = result?;
    let value = payload.take();
    // The verifier transfers a complete payload; V259's outer entry frame has
    // already prepaid R's inline carrier. Restore only its surviving backing.
    let Some(dynamic) = value.1.checked_sub(size_of::<R>()) else {
        formal_context_v19::discard(value);
        return Err(source
            .retain_query_resource_error_v18(Resource::Accounting)
            .into());
    };
    if let Err(error) = budget.reserve_storage(dynamic) {
        formal_context_v19::discard(value);
        return Err(source.retain_query_resource_error_v18(error).into());
    }
    Ok((value.0, dynamic))
}

impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    /// Obligatorily composes V259 with scoped support generation on the original
    /// account. Unlike V259 input preparation, this stage refuses runtime inputs
    /// outside the existing model domain (including oversized 32-bit envelopes).
    /// The borrowed output is not a paired proof, receipt or default route.
    pub(crate) fn with_original_source_expanded_model_v280<R: 'static, F>(
        self, budget: &mut Budget<'_>, consume: F,
    ) -> Result<SourceOwnedCompilationContinuationV29<R>, Error>
    where
        F: for<'view, 'source, 'original, 'scope, 'tile, 'abi, 'pair, 'model, 'model_source, 'work> FnOnce(
            &'view Source<'source>, &'original Original<'scope>,
            &'tile Expanded<'original, 'scope>, &[AbiRoot<'abi>], TargetProfile,
            &'pair ExpandedPairInputV279<'pair, 'source, 'original, 'scope>,
            &ExpandedSupportModelV280<'model, 'model_source>, &mut Budget<'work>,
        ) -> Result<(R, usize), Error>,
    {
        let mut pending = formal_context_v19::PendingConsumerV19::new(consume);
        self.with_original_source_expanded_v259(
            budget,
            move |source, original, tile, roots, target, pair, budget| {
                observe_model(source, original, tile, pair, budget, |model, budget| {
                    pending.take()(source, original, tile, roots, target, pair, model, budget)
                })
            },
        )
    }
}
