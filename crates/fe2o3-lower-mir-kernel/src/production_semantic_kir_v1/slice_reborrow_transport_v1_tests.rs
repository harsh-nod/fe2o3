#[test]
fn slice_transport_rejects_changed_access_element_space_or_lost_metadata() {
    // Private carrier transport only, not source admission or a forged owner.
    let expected = Type::slice(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadOnly,
    );
    let transport = SemanticPromotedTransportV1::DirectParameter { parameter_local: 1 };
    let semantic_type = SemanticTypeIdV1::from_index(0);
    for wrong in [
        Type::slice(
            Type::Scalar(ScalarType::U32),
            AddressSpace::Global,
            AccessMode::ReadWrite,
        ),
        Type::slice(
            Type::Scalar(ScalarType::F32),
            AddressSpace::Global,
            AccessMode::ReadOnly,
        ),
        Type::slice(
            Type::Scalar(ScalarType::U32),
            AddressSpace::Private,
            AccessMode::ReadOnly,
        ),
        Type::pointer(
            Type::Scalar(ScalarType::U32),
            AddressSpace::Global,
            AccessMode::ReadOnly,
        ),
        Type::Scalar(ScalarType::U64),
    ] {
        assert_eq!(
            transport.transport_values(
                &SemanticValueBindingV1::Value {
                    id: ValueId(7),
                    ty: wrong.clone(),
                },
                std::slice::from_ref(&expected),
            ),
            Err("promoted direct parameter changed its authenticated ABI carrier"),
        );
        assert!(
            transport
                .binding_from_transport(
                    &[],
                    semantic_type,
                    &[ValueDef::new(ValueId(8), wrong)],
                    std::slice::from_ref(&expected),
                )
                .is_err()
        );
    }
    assert!(
        transport
            .binding_from_transport(
                &[],
                semantic_type,
                &[
                    ValueDef::new(
                        ValueId(9),
                        Type::pointer(
                            Type::Scalar(ScalarType::U32),
                            AddressSpace::Global,
                            AccessMode::ReadOnly,
                        ),
                    ),
                    ValueDef::new(ValueId(10), Type::INDEX),
                ],
                std::slice::from_ref(&expected),
            )
            .is_err()
    );
}
