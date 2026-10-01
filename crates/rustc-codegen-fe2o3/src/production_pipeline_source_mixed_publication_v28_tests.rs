use super::*;

#[test]
fn mixed_publication_candidate_headers_preserve_the_original_borrowed_chain() {
    type Fields<'a> = (&'a (), &'a (), &'a (), &'a (), &'a (), &'a (), usize);
    assert_eq!(
        size_of::<PreparedMixedPublicationV28<'_, '_, '_>>(),
        size_of::<Fields<'_>>()
    );
    assert_eq!(
        size_of::<ProtectedMixedPublicationV28<'_, '_, '_>>(),
        size_of::<Fields<'_>>()
    );
    let expected = 2 * (size_of::<Fields<'_>>() + align_of::<Fields<'_>>())
        + size_of::<Result<ProtectedMixedPublicationV28<'_, '_, '_>, Error>>()
        + size_of::<Result<(&AdmittedProtectedRustcInvocationV1, BuildAttempt), Error>>()
        + size_of::<Result<(), Error>>()
        + size_of::<Result<(), ProtectedRustcInvocationErrorV1>>()
        + size_of::<[usize; 4]>();
    assert_eq!(headers().unwrap(), expected);
    type Consumer = for<'a, 'v, 's, 'w> fn(
        PreparedMixedPublicationV28<'a, 'v, 's>,
        &mut Budget<'w>,
    ) -> Result<(), Error>;
    assert_eq!(
        <PublicationInput as FinalConsumer<(), Consumer>>::headers().unwrap(),
        expected
            + size_of::<Pending<Consumer>>()
            + align_of::<Pending<Consumer>>()
            + original_mir_v30::headers::<(), Consumer>().unwrap()
    );
}

#[test]
fn mixed_publication_custody_cannot_be_constructed_from_extraction() {
    let custody = ProductionCompilerCustody::extraction_only();
    let result = retained_invocation(&custody);
    assert!(matches!(
        result,
        Err(Error::MixedPublication(
            MixedPublicationErrorV28::ExtractionOnly
        ))
    ));
}

#[test]
fn original_mir_worker_frames_cover_actual_capture_and_result_envelopes() {
    use fe2o3_kernel_ir::{EndiannessV2, ExplicitLaunchExtent, FormalIndexWidth};

    type RuntimeFields<'a> = (&'a [ExplicitLaunchExtent], FormalIndexWidth, EndiannessV2);
    type Consumer = for<'a, 'v, 's, 'w> fn(
        PreparedMixedPublicationV28<'a, 'v, 's>,
        &mut Budget<'w>,
    ) -> Result<(), Error>;
    type Fields<'a> = (
        (&'a (), &'a (), &'a (), &'a (), &'a ()),
        &'a mut (),
        &'a std::cell::Cell<usize>,
        Pending<Consumer>,
    );
    let expected = size_of::<Fields<'_>>()
        + align_of::<Fields<'_>>()
        + size_of::<std::panic::AssertUnwindSafe<Fields<'_>>>()
        + size_of::<std::cell::Cell<usize>>()
        + size_of::<OriginalMir<'_, '_>>()
        + size_of::<Result<OriginalMir<'_, '_>, fe2o3_verifier::MixedOptimizerRefinementErrorV26>>(
        )
        + size_of::<
            Result<
                fe2o3_verifier::OriginalSemanticMirRefinementSubjectV36,
                fe2o3_verifier::MixedOptimizerRefinementErrorV26,
            >,
        >()
        + size_of::<fe2o3_verifier::OriginalSemanticMirRefinementSubjectV36>()
        + size_of::<Subject>()
        + size_of::<Result<Subject, fe2o3_verifier::MixedOptimizerRelocationErrorV28>>()
        + 4 * size_of::<Result<(), Error>>()
        + size_of::<std::thread::Result<Result<(), Error>>>()
        + 2 * size_of::<&()>()
        + 3 * size_of::<usize>()
        + size_of::<RuntimeFields<'_>>()
        + size_of::<Result<RuntimeFields<'_>, Error>>();
    assert_eq!(
        original_mir_v30::headers::<(), Consumer>().unwrap(),
        expected
    );
}
