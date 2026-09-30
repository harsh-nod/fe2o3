//! Explicit final-native and expression-availability production continuation.
//! This analyzes actual LICM output; it does not execute a refinement theorem.
use super::*;
use fe2o3_lower_mir_kernel::{
    ProductionConditionalMixedLicmOutputHandoffV28 as Native,
    ProductionMixedLicmRelocationV28 as Relocation,
};
use fe2o3_verifier::{
    MixedOptimizerRelocationSubjectV28 as Subject, PreparedMixedRelocationExpressionsV28 as Request,
};
#[cfg(test)]
use std::mem::{align_of_val, size_of_val};

type Inspect = for<'p, 'v, 's, 'a, 'w> fn(
    &'v Source<'s>,
    &Relocation<'p, 'v, 's>,
    &[AbiRoot<'a>],
    TargetProfile,
    &mut Budget<'w>,
) -> Result<Subject, Error>;
type NativeCapture<'a, 'n, 'p, 'v, 's, 'abi, 'w> = (
    &'v Source<'s>,
    &'a Native<'n, 'p, 'v, 's>,
    &'a [AbiRoot<'abi>],
    &'a mut Budget<'w>,
);
type RequestCapture<'a, 'h, 'n, 'p, 'v, 's, 'w> =
    (&'a Request<'h, 'n, 'p, 'v, 's>, &'a mut Budget<'w>);
struct Expressions;

fn inspection_headers() -> Result<usize, Resource> {
    [
        size_of::<NativeCapture<'_, '_, '_, '_, '_, '_, '_>>(),
        align_of::<NativeCapture<'_, '_, '_, '_, '_, '_, '_>>(),
        size_of::<AssertUnwindSafe<NativeCapture<'_, '_, '_, '_, '_, '_, '_>>>(),
        size_of::<RequestCapture<'_, '_, '_, '_, '_, '_, '_>>(),
        align_of::<RequestCapture<'_, '_, '_, '_, '_, '_, '_>>(),
        size_of::<AssertUnwindSafe<RequestCapture<'_, '_, '_, '_, '_, '_, '_>>>(),
        2 * size_of::<std::thread::Result<Result<Subject, Error>>>(),
        2 * size_of::<Result<(), Error>>(),
        size_of::<
            Result<
                Native<'_, '_, '_, '_>,
                fe2o3_lower_mir_kernel::ProductionMixedLicmCompletionErrorV28,
            >,
        >(),
        size_of::<
            Result<Request<'_, '_, '_, '_, '_>, fe2o3_verifier::MixedOptimizerRelocationErrorV28>,
        >(),
    ]
    .into_iter()
    .try_fold(0usize, |sum, bytes| {
        sum.checked_add(bytes).ok_or(Resource::Arithmetic)
    })
}

// A fixed private stage adds its real nested frames to the existing source
// callback envelope. Neither caller pass lists nor a new optimizer policy exist.
impl SourceHandoffPolicyV29<Subject, Inspect> for Expressions {
    fn entry_headers() -> Result<usize, Resource> {
        <mixed_licm_v28::MixedLicm as SourceHandoffPolicyV29<Subject, Inspect>>::entry_headers()?
            .checked_add(inspection_headers()?)
            .ok_or(Resource::Arithmetic)
    }
    fn consume<'v, 's, 'a, 'w>(
        source: &'v Source<'s>,
        roots: &[AbiRoot<'a>],
        context: &SourceBindingContextV29<'_>,
        budget: &mut Budget<'w>,
        consume: Inspect,
    ) -> Result<Subject, Error> {
        <mixed_licm_v28::MixedLicm as SourceHandoffPolicyV29<Subject, Inspect>>::consume(
            source, roots, context, budget, consume,
        )
    }
}

fn settle(
    selected: std::thread::Result<Result<Subject, Error>>,
    released: Result<(), Error>,
) -> Result<Subject, Error> {
    match selected {
        Ok(Ok(subject)) => released.map(|()| subject),
        Ok(Err(error)) => Err(error),
        Err(payload) => resume_unwind(payload),
    }
}
fn inspect_request(capture: RequestCapture<'_, '_, '_, '_, '_, '_, '_>) -> Result<Subject, Error> {
    let (request, budget) = capture;
    request
        .replay(budget)
        .map_err(Error::MixedRelocationExpressions)?;
    request
        .subject(budget)
        .map_err(Error::MixedRelocationExpressions)
}
fn inspect_native(capture: NativeCapture<'_, '_, '_, '_, '_, '_, '_>) -> Result<Subject, Error> {
    let (source, native, roots, budget) = capture;
    native
        .check_original_argument_abi_v26(ProductionKernelArgumentAbiInputV18 { roots }, budget)?;
    let request = fe2o3_verifier::prepare_mixed_relocation_expressions_v28(source, native, budget)
        .map_err(Error::MixedRelocationExpressions)?;
    let selected = {
        let capture = (&request, &mut *budget);
        let run = move || inspect_request(capture);
        #[cfg(test)]
        {
            assert_eq!(
                size_of_val(&run),
                size_of::<RequestCapture<'_, '_, '_, '_, '_, '_, '_>>()
            );
            assert_eq!(
                align_of_val(&run),
                align_of::<RequestCapture<'_, '_, '_, '_, '_, '_, '_>>()
            );
        }
        catch_unwind(AssertUnwindSafe(run))
    };
    let released = request
        .discard(budget)
        .map_err(Error::MixedRelocationExpressions);
    settle(selected, released)
}
fn inspect<'p, 'v, 's>(
    source: &'v Source<'s>,
    relocation: &Relocation<'p, 'v, 's>,
    roots: &[AbiRoot<'_>],
    _: TargetProfile,
    budget: &mut Budget<'_>,
) -> Result<Subject, Error> {
    let native = relocation
        .complete_native_v28(budget)
        .map_err(Error::MixedLicmCompletion)?;
    let selected = {
        let capture = (source, &native, roots, &mut *budget);
        let run = move || {
            // Pass the tuple as a whole: both actual closure layout and paid
            // layout use the same closed source/native/budget frame.
            inspect_native(capture)
        };
        #[cfg(test)]
        {
            assert_eq!(
                size_of_val(&run),
                size_of::<NativeCapture<'_, '_, '_, '_, '_, '_, '_>>()
            );
            assert_eq!(
                align_of_val(&run),
                align_of::<NativeCapture<'_, '_, '_, '_, '_, '_, '_>>()
            );
        }
        catch_unwind(AssertUnwindSafe(run))
    };
    let released = native.discard(budget).map_err(Error::from);
    settle(selected, released)
}

impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    /// Actual original Rust -> Policy10 -> checked LICM -> fresh final-native
    /// completion -> source-bound expression/cut replay. The returned summary
    /// is inert and this explicit nondefault stage grants no proof or launch.
    pub(crate) fn analyze_original_source_mixed_relocation_v28(
        self,
    ) -> Result<SourceOwnedCompilationContinuationV29<Subject>, Error> {
        self.with_source_owned_custody_policy_v29::<Expressions, Subject, Inspect>(
            ImportProfile::NominalV35,
            WORK_LIMIT,
            STORAGE_LIMIT,
            inspect as Inspect,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixed_relocation_stage_quotes_both_actual_capture_shapes_independently() {
        type NativeFields<'a> = (&'a (), &'a (), &'a [()], &'a mut ());
        type RequestFields<'a> = (&'a (), &'a mut ());
        let expected = size_of::<NativeFields<'_>>()
            + align_of::<NativeFields<'_>>()
            + size_of::<AssertUnwindSafe<NativeFields<'_>>>()
            + size_of::<RequestFields<'_>>()
            + align_of::<RequestFields<'_>>()
            + size_of::<AssertUnwindSafe<RequestFields<'_>>>()
            + 2 * size_of::<std::thread::Result<Result<Subject, Error>>>()
            + 2 * size_of::<Result<(), Error>>()
            + size_of::<
                Result<
                    Native<'_, '_, '_, '_>,
                    fe2o3_lower_mir_kernel::ProductionMixedLicmCompletionErrorV28,
                >,
            >()
            + size_of::<
                Result<
                    Request<'_, '_, '_, '_, '_>,
                    fe2o3_verifier::MixedOptimizerRelocationErrorV28,
                >,
            >();
        assert_eq!(inspection_headers().unwrap(), expected);
        let base =
            <mixed_licm_v28::MixedLicm as SourceHandoffPolicyV29<Subject, Inspect>>::entry_headers(
            )
            .unwrap();
        assert_eq!(
            <Expressions as SourceHandoffPolicyV29<Subject, Inspect>>::entry_headers().unwrap(),
            base + expected
        );
    }

    #[test]
    fn mixed_relocation_stage_keeps_selected_failure_before_later_settlement() {
        let selected = settle(
            Ok(Err(Error::Unsupported("selected expression replay"))),
            Err(Error::Source(ProductionSourceOwnedViewErrorV18::Resource(
                Resource::Accounting,
            ))),
        );
        assert!(matches!(
            selected,
            Err(Error::Unsupported("selected expression replay"))
        ));
        let caught = catch_unwind(AssertUnwindSafe(|| {
            settle(
                Err(Box::new(281u32)),
                Err(Error::Source(ProductionSourceOwnedViewErrorV18::Resource(
                    Resource::Accounting,
                ))),
            )
        }));
        assert_eq!(*caught.unwrap_err().downcast::<u32>().unwrap(), 281);
    }
}
