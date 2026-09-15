use super::*;

fn request() -> InertSemanticMirRequestV1 {
    fixture(SemanticTypeShapeV1::Unit, vec![], false, attributes(false))
}

fn context(request: &InertSemanticMirRequestV1, work: u64) -> ValidationContextV1<'_> {
    ValidationContextV1 {
        request,
        limits: SemanticMirLimitsV1::default()
            .with_limit(SemanticMirResourceV1::ValidationWork, work)
            .unwrap(),
        totals: ValidationTotalsV1::default(),
        work: 0,
    }
}

fn check(request: &InertSemanticMirRequestV1) -> Result<(), SemanticMirErrorV1> {
    validate_source_argument_types(
        &mut context(request, 4),
        &request.functions[0].abi,
        SemanticMirLocationV1::Function(SemanticFunctionIdV1::from_index(0)),
    )
}

#[test]
fn explicit_borrow_and_raw_pointer_claims_require_the_exact_source_pointer_kind() {
    use SemanticSourceArgumentOwnershipV1::{RawPointer, SharedBorrow, UniqueBorrow};
    for metadata in [
        SemanticPointerMetadataV1::None,
        SemanticPointerMetadataV1::SliceLength,
    ] {
        for (kind, mutability, expected) in [
            (
                SemanticPointerKindV1::Raw,
                SemanticMutabilityV1::Immutable,
                RawPointer,
            ),
            (
                SemanticPointerKindV1::Raw,
                SemanticMutabilityV1::Mutable,
                RawPointer,
            ),
            (
                SemanticPointerKindV1::Reference,
                SemanticMutabilityV1::Immutable,
                SharedBorrow,
            ),
            (
                SemanticPointerKindV1::Reference,
                SemanticMutabilityV1::Mutable,
                UniqueBorrow,
            ),
        ] {
            for ownership in [SharedBorrow, UniqueBorrow, RawPointer] {
                let mut request = request();
                request.types[REFERENCE.index() as usize].shape = SemanticTypeShapeV1::Pointer(
                    SemanticPointerTypeV1::new_with_kind(ENV, kind, mutability, 0, 64, metadata)
                        .unwrap(),
                );
                request.functions[0].abi.source_argument_ownership[0] = ownership;
                assert_eq!(
                    check(&request),
                    if ownership == expected {
                        Ok(())
                    } else {
                        Err(SemanticMirErrorV1::InvalidFunctionAbi)
                    },
                    "{kind:?} {mutability:?} {metadata:?} {ownership:?}",
                );
            }
        }
    }
}

#[test]
fn non_pointer_values_cannot_claim_borrow_or_raw_pointer_ownership() {
    use SemanticSourceArgumentOwnershipV1 as Ownership;
    for ownership in [
        Ownership::SharedBorrow,
        Ownership::UniqueBorrow,
        Ownership::RawPointer,
    ] {
        let mut request = request();
        request.functions[0].abi.source_argument_ownership[1] = ownership;
        assert_eq!(check(&request), Err(SemanticMirErrorV1::InvalidFunctionAbi));
        assert!(matches!(
            request.admit_current_production(SemanticMirLimitsV1::default()),
            Err(SemanticMirErrorV1::InvalidFunctionAbi)
        ));
    }
    // Shape consistency is not an aliasing or exclusive-owner proof.
    for ownership in [
        Ownership::Unspecified,
        Ownership::ByValue,
        Ownership::ExclusiveOwner,
    ] {
        let mut request = request();
        request.functions[0].abi.source_argument_ownership[1] = ownership;
        assert_eq!(check(&request), Ok(()));
    }
}

#[test]
fn non_body_abi_validation_uses_the_same_source_ownership_check() {
    let mut request = request();
    let function = &mut request.functions[0];
    function.abi = SemanticFunctionAbiV1::from_rustc(
        function.abi.identity,
        function.abi.layout_identity,
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        2,
        vec![
            function.abi.arguments[0].clone(),
            SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                TAIL,
                SemanticAbiPassModeV1::Ignore,
            )),
        ],
        function.abi.return_value.clone(),
    )
    .unwrap()
    .with_source_argument_ownership(vec![
        SemanticSourceArgumentOwnershipV1::SharedBorrow,
        SemanticSourceArgumentOwnershipV1::ByValue,
    ])
    .unwrap();
    let binding = SemanticNonBodyCallableBindingV1::new(
        function.identity(),
        function.item_definition_identity(),
        function.monomorphization_identity(),
        function.generic_type_arguments_identity(),
        function.const_generic_arguments_identity(),
        function.source(),
        function.abi.clone(),
    );
    let location = SemanticMirLocationV1::Function(SemanticFunctionIdV1::from_index(0));
    let mut valid = context(&request, HARD_MAX_VALIDATION_WORK_V1);
    validate_non_body_callable_abi(&mut valid, location, &binding).unwrap();
    for ownership in [
        SemanticSourceArgumentOwnershipV1::SharedBorrow,
        SemanticSourceArgumentOwnershipV1::UniqueBorrow,
    ] {
        let mut changed = binding.clone();
        changed.abi.source_argument_ownership[1] = ownership;
        let mut invalid = context(&request, HARD_MAX_VALIDATION_WORK_V1);
        assert_eq!(
            validate_non_body_callable_abi(&mut invalid, location, &changed),
            Err(SemanticMirErrorV1::InvalidFunctionAbi),
        );
    }
}

#[test]
fn source_ownership_checks_charge_each_type_and_claim_before_inspection() {
    let request = request();
    let location = SemanticMirLocationV1::Function(SemanticFunctionIdV1::from_index(0));
    for available in 0..=4 {
        let mut context = context(&request, available);
        let result =
            validate_source_argument_types(&mut context, &request.functions[0].abi, location);
        if available == 4 {
            assert_eq!(result, Ok(()));
            assert_eq!(context.work, 4);
        } else {
            assert_eq!(
                result,
                Err(SemanticMirErrorV1::LimitExceeded {
                    resource: SemanticMirResourceV1::ValidationWork,
                    actual: available + 1,
                    max: available,
                })
            );
            assert_eq!(context.work, available + 1);
        }
    }
}
