use super::*;

#[test]
fn mixed_publication_candidate_headers_preserve_the_original_borrowed_chain() {
    type Fields<'a> = (&'a (), &'a (), &'a (), &'a (), &'a (), usize);
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
        expected + size_of::<Pending<Consumer>>() + align_of::<Pending<Consumer>>()
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
