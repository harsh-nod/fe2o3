//! Mandatory original-source producer inside the existing Worker consumer.
//! The nominal V36 request covers scalar bodies and concrete invocation CFGs. Preparation is
//! concrete source refinement input, not executed proof authority.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalFormalLaunchInputV19 as Launch, EndiannessV2, ExplicitLaunchExtent, FormalIndexWidth,
};
use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

fn runtime_headers() -> Result<usize, Resource> {
    [
        formal_context_v19::launch_context_headers_v19()?,
        size_of::<crate::semantic_layout_bridge::SemanticLayoutTargetV1>(),
        size_of::<sha2::Sha256>(),
        size_of::<Box<[u8]>>(),
        8 * size_of::<&()>(),
        10 * size_of::<usize>(),
        size_of::<(Vec<Launch>, FormalIndexWidth)>(),
        size_of::<Result<(Vec<Launch>, FormalIndexWidth), Error>>(),
        size_of::<(&[ExplicitLaunchExtent], FormalIndexWidth, EndiannessV2)>(),
        3 * size_of::<Result<(), Error>>(),
    ]
    .into_iter()
    .try_fold(0usize, |sum, n| {
        sum.checked_add(n).ok_or(Resource::Arithmetic)
    })
}

fn runtime_mismatch() -> Error {
    MixedPublicationErrorV28::Binding(
        "original MIR runtime differs from the retained source/target context",
    )
    .into()
}

fn match_launches(
    retained: &[ExplicitLaunchExtent],
    width: FormalIndexWidth,
    checked: &[Launch],
    checked_width: FormalIndexWidth,
    budget: &mut Budget<'_>,
) -> Result<(), Error> {
    budget.charge_work(3)?;
    if width != checked_width
        || matches!(width, FormalIndexWidth::Unknown)
        || retained.len() != checked.len()
    {
        return Err(runtime_mismatch());
    }
    for (retained, checked) in retained.iter().zip(checked) {
        budget.charge_work(5)?;
        if *checked != Launch::PhysicalEnvelope(*retained) {
            return Err(runtime_mismatch());
        }
    }
    Ok(())
}

fn target_byte_order(
    source: fe2o3_mir_model::semantic_mir_v1::SemanticLayoutIdentityV1,
    target: &crate::semantic_layout_bridge::SemanticLayoutTargetV1,
    budget: &mut Budget<'_>,
) -> Result<EndiannessV2, Error> {
    // Shared V1 codec: six length-prefixed fields, including the fixed domain
    // and two pointer-width bytes. Pay Vec-to-Box coexistence before the codec.
    let bytes = [
        b"fe2o3/semantic-mir/rustc-target-layout/v1".len(),
        target.llvm_target().len(),
        target.data_layout().len(),
        2,
        target.active_cpu().unwrap_or_default().len(),
        target.active_features().unwrap_or_default().len(),
    ]
    .into_iter()
    .try_fold(6usize * 8, |sum, n| {
        sum.checked_add(n).ok_or(Resource::Arithmetic)
    })?;
    budget.charge_work(
        bytes
            .checked_mul(3)
            .and_then(|n| n.checked_add(32))
            .ok_or(Resource::Arithmetic)?,
    )?;
    if target.data_layout() != crate::production_target_v1::PRODUCTION_RUSTC_DATA_LAYOUT_V1 {
        return Err(runtime_mismatch());
    }
    let payload = bytes.checked_mul(2).ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(payload)?;
    let observed = crate::rustc_semantic_adapter_v1::canonical_target_layout_v1(target).identity;
    budget.release_storage(payload)?;
    if observed != source {
        return Err(runtime_mismatch());
    }
    // The entire LLVM data-layout string was authenticated above. Its first
    // grammar field is the declared byte order, not the pointer or host width.
    match target.data_layout().split('-').next() {
        Some("e") => Ok(EndiannessV2::Little),
        Some("E") => Ok(EndiannessV2::Big),
        _ => Err(runtime_mismatch()),
    }
}

fn runtime<'a>(
    inputs: &'a FinalInputs<'_, '_, '_, '_>,
    budget: &mut Budget<'_>,
) -> Result<(&'a [ExplicitLaunchExtent], FormalIndexWidth, EndiannessV2), Error> {
    inputs
        .native
        .check_original_source(inputs.source.source_ssa(budget)?, budget)?;
    let (retained, width) = inputs.native.launch_context(budget)?;
    let floor = budget.storage();
    let endianness = budget.with_prepaid_scope(floor, 1, 1, runtime_headers()?, |budget| {
        let (checked, checked_width) = inputs.context.launches(inputs.source, budget)?;
        match_launches(retained, width, &checked, checked_width, budget)?;
        let source_target = inputs
            .source
            .source_semantic(budget)?
            .target_layout_identity();
        let endian = target_byte_order(
            source_target,
            inputs.context.bindings.rustc_target.rustc_layout(),
            budget,
        )?;
        drop(checked);
        Ok::<_, Error>(endian)
    })?;
    Ok((retained, width, endianness))
}

pub(super) fn replay_runtime(
    inputs: &FinalInputs<'_, '_, '_, '_>,
    original: &OriginalMir<'_, '_>,
    budget: &mut Budget<'_>,
) -> Result<(), Error> {
    let (launches, width, endianness) = runtime(inputs, budget)?;
    original
        .check_runtime_v36(launches, width, endianness, budget)
        .map_err(Error::OriginalMir)
}

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
                fe2o3_verifier::OriginalSemanticMirRefinementSubjectV36,
                fe2o3_verifier::MixedOptimizerRefinementErrorV26,
            >,
        >(),
        size_of::<fe2o3_verifier::OriginalSemanticMirRefinementSubjectV36>(),
        size_of::<Subject>(),
        size_of::<Result<Subject, fe2o3_verifier::MixedOptimizerRelocationErrorV28>>(),
        2 * size_of::<Result<R, Error>>(),
        size_of::<std::thread::Result<Result<R, Error>>>(),
        2 * size_of::<Result<(), Error>>(),
        size_of::<(&Source<'_>, &Native<'_, '_, '_, '_>)>(),
        3 * size_of::<usize>(),
        size_of::<(&[ExplicitLaunchExtent], FormalIndexWidth, EndiannessV2)>(),
        size_of::<Result<(&[ExplicitLaunchExtent], FormalIndexWidth, EndiannessV2), Error>>(),
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
        let (launches, width, endianness) = runtime(&inputs, budget)?;
        let original = fe2o3_verifier::prepare_original_semantic_mir_refinement_v36(
            inputs.source,
            launches,
            width,
            endianness,
            budget,
        )
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

#[cfg(test)]
mod runtime_tests {
    use super::*;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

    #[test]
    fn original_mir_final_context_matches_exact_native_launch_width_and_interpretation() {
        let retained = [ExplicitLaunchExtent::Exact {
            rank: 1,
            extents: [64, 1, 1],
        }; 2];
        let checked = retained.map(Launch::PhysicalEnvelope);
        let run = |retained: &[ExplicitLaunchExtent], width, checked: &[Launch], limit| {
            let mut work = Work::new(limit);
            let mut budget = Budget::new(&mut work, 0);
            let result = match_launches(
                retained,
                width,
                checked,
                FormalIndexWidth::Bits64,
                &mut budget,
            );
            (result, budget.work())
        };
        let exact = run(&retained, FormalIndexWidth::Bits64, &checked, 3 + 5 * 2);
        exact.0.unwrap();
        assert_eq!(exact.1, 13);
        assert!(
            run(&retained, FormalIndexWidth::Bits64, &checked, 12)
                .0
                .is_err()
        );
        assert!(
            run(&retained, FormalIndexWidth::Bits32, &checked, 13)
                .0
                .is_err()
        );
        assert!(
            run(&retained[..1], FormalIndexWidth::Bits64, &checked, 13)
                .0
                .is_err()
        );
        assert!(
            run(
                &retained,
                FormalIndexWidth::Bits64,
                &retained.map(Launch::Exact),
                13
            )
            .0
            .is_err()
        );
        let mut changed = retained;
        changed[1] = ExplicitLaunchExtent::Exact {
            rank: 1,
            extents: [65, 1, 1],
        };
        assert!(
            run(&changed, FormalIndexWidth::Bits64, &checked, 13)
                .0
                .is_err()
        );
    }

    #[test]
    fn original_mir_byte_order_requires_exact_source_target_layout_identity() {
        use crate::semantic_layout_bridge::SemanticLayoutTargetV1;
        use fe2o3_amd_target::ProductionAmdTargetProfileV1 as Profile;
        let target = |profile: Profile, layout: &str| {
            SemanticLayoutTargetV1::new_with_codegen_profile(
                profile.rustc_target(),
                layout,
                64,
                profile.cpu(),
                "",
                profile.rustc_features(),
            )
            .unwrap()
        };
        let layout = crate::production_target_v1::PRODUCTION_RUSTC_DATA_LAYOUT_V1;
        let exact = target(Profile::Gfx942, layout);
        let identity =
            crate::rustc_semantic_adapter_v1::canonical_target_layout_v1(&exact).identity;
        let run = |candidate: &SemanticLayoutTargetV1| {
            let mut work = Work::new(1_000_000);
            let mut budget = Budget::new(&mut work, 1_000_000);
            budget.with_prepaid_scope(0, 1, 1, runtime_headers().unwrap(), |budget| {
                target_byte_order(identity, candidate, budget)
            })
        };
        assert_eq!(run(&exact).unwrap(), EndiannessV2::Little);
        assert!(run(&target(Profile::Gfx950, layout)).is_err());
        let changed = format!("E{}", &layout[1..]);
        assert!(run(&target(Profile::Gfx942, &changed)).is_err());
        assert!(run(&target(Profile::Gfx942, "e-p:64:64")).is_err());
    }
}
