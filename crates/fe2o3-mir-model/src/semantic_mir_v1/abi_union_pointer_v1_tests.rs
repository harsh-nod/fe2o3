use super::*;

fn pointer(initialized: bool) -> SemanticBackendScalarV1 {
    let primitive = SemanticBackendPrimitiveV1::pointer(0, 8, 8);
    if initialized {
        SemanticBackendScalarV1::initialized(
            primitive,
            SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
        )
    } else {
        SemanticBackendScalarV1::union(primitive)
    }
}

fn check(
    attributes: SemanticAbiValueAttributesV1,
    pointee: Option<SemanticAbiPointeeInfoV1>,
    safe_non_null_override: bool,
) -> Result<(), SemanticMirErrorV1> {
    validate_scalar_abi_attributes(
        attributes,
        pointer(false),
        pointee,
        safe_non_null_override,
        false,
        true,
    )
}

#[test]
fn abi_union_pointer_accepts_only_plain_fact_free_transport() {
    for foreign in [false, true] {
        for is_return in [false, true] {
            assert_eq!(
                validate_scalar_abi_attributes(
                    SemanticAbiValueAttributesV1::plain(),
                    pointer(false),
                    None,
                    false,
                    foreign,
                    is_return,
                ),
                Ok(())
            );
        }
    }
}

#[test]
fn abi_union_pointer_rejects_each_forged_regular_fact() {
    for regular in [
        SemanticAbiRegularAttributesV1::new(true, None, false, false, false, false),
        SemanticAbiRegularAttributesV1::new(
            false,
            Some(SemanticAbiPointerCaptureV1::CapturesNone),
            false,
            false,
            false,
            false,
        ),
        SemanticAbiRegularAttributesV1::new(
            false,
            Some(SemanticAbiPointerCaptureV1::CapturesAddress),
            false,
            false,
            false,
            false,
        ),
        SemanticAbiRegularAttributesV1::new(
            false,
            Some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
            false,
            false,
            false,
            false,
        ),
        SemanticAbiRegularAttributesV1::new(false, None, true, false, false, false),
        SemanticAbiRegularAttributesV1::new(false, None, false, true, false, false),
        SemanticAbiRegularAttributesV1::new(false, None, false, false, true, false),
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
    ] {
        let attributes =
            SemanticAbiValueAttributesV1::new(regular, SemanticAbiExtensionV1::None, 0, None)
                .unwrap();
        assert_eq!(
            check(attributes, None, false),
            Err(SemanticMirErrorV1::InvalidFunctionAbi),
            "{regular:?}"
        );
    }
}

#[test]
fn abi_union_pointer_rejects_extensions_alignment_and_pointee_size() {
    let plain = SemanticAbiRegularAttributesV1::new(false, None, false, false, false, false);
    for (extension, size, alignment) in [
        (SemanticAbiExtensionV1::ZeroExtend, 0, None),
        (SemanticAbiExtensionV1::SignExtend, 0, None),
        (SemanticAbiExtensionV1::None, 8, None),
        (SemanticAbiExtensionV1::None, 0, Some(8)),
    ] {
        let attributes =
            SemanticAbiValueAttributesV1::new(plain, extension, size, alignment).unwrap();
        assert_eq!(
            check(attributes, None, false),
            Err(SemanticMirErrorV1::InvalidFunctionAbi)
        );
    }
}

#[test]
fn abi_union_pointer_cannot_borrow_pointee_or_nonnull_authority() {
    let raw = SemanticAbiPointeeInfoV1::new(SemanticAbiPointeeKindV1::Raw, 0, 1).unwrap();
    assert_eq!(
        check(SemanticAbiValueAttributesV1::plain(), Some(raw), false),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    );
    assert_eq!(
        check(SemanticAbiValueAttributesV1::plain(), None, true),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    );
}

#[test]
fn abi_initialized_pointer_still_requires_exact_pointee_metadata() {
    let initialized = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap();
    assert_eq!(
        validate_scalar_abi_attributes(initialized, pointer(true), None, false, false, true),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    );
    let raw = SemanticAbiPointeeInfoV1::new(SemanticAbiPointeeKindV1::Raw, 0, 1).unwrap();
    assert_eq!(
        validate_scalar_abi_attributes(initialized, pointer(true), Some(raw), false, false, true),
        Ok(())
    );
}
