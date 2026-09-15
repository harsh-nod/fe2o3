#[test]
fn math_ssa_transport_keeps_one_exact_existing_value_for_each_role() {
    let (types, contract, context) = fixture(MATH_FUNCTIONS[0]);
    for bound in [false, true] {
        let transport = MathTransportV1 { contract, bound, capture: None };
        let ty = transport
            .kernel_type(&types, transport.semantic_type(), Some(&context))
            .unwrap();
        let value = SemanticValueBindingV1::Value {
            id: ValueId(73),
            ty: ty.clone(),
        };
        assert_eq!(
            transport.values(&value, &[ty.clone()]).unwrap(),
            [(ValueId(73), ty.clone())]
        );
        let restored = transport
            .from_values(
                &types,
                transport.semantic_type(),
                &[ValueDef::new(ValueId(73), ty.clone())],
                &[ty.clone()],
            )
            .unwrap();
        assert_eq!(restored.values().unwrap(), [(ValueId(73), ty)]);
    }
}

#[test]
fn math_ssa_transport_rejects_missing_fabricated_and_other_role_values() {
    let (types, contract, context) = fixture(MATH_FUNCTIONS[0]);
    let transport = MathTransportV1 {
        contract,
        bound: true,
        capture: None,
    };
    let ty = transport
        .kernel_type(&types, transport.semantic_type(), Some(&context))
        .unwrap();
    for forged in [
        SemanticValueBindingV1::Unit,
        SemanticValueBindingV1::Unmaterialized,
        SemanticValueBindingV1::Aggregate(vec![]),
    ] {
        assert!(transport.values(&forged, &[ty.clone()]).is_err());
    }
    assert!(
        transport
            .from_values(&types, transport.semantic_type(), &[], &[ty.clone()])
            .is_err()
    );
    assert!(
        transport
            .kernel_type(&types, transport.semantic_type(), None)
            .is_err()
    );
    assert!(
        transport
            .kernel_type(&types, contract.types().math, Some(&context))
            .is_err()
    );
    let source = MathTransportV1 {
        contract,
        bound: false,
        capture: None,
    };
    let source_ty = source
        .kernel_type(&types, source.semantic_type(), Some(&context))
        .unwrap();
    assert!(
        transport
            .values(
                &SemanticValueBindingV1::Value {
                    id: ValueId(73),
                    ty: source_ty.clone()
                },
                &[ty.clone()]
            )
            .is_err()
    );
    assert!(
        transport
            .from_values(
                &types,
                transport.semantic_type(),
                &[ValueDef::new(ValueId(73), source_ty)],
                &[ty]
            )
            .is_err()
    );
}

#[test]
fn math_ssa_transport_rejects_root_policy_and_nominal_substitutions() {
    let (types, contract, context) = fixture(MATH_FUNCTIONS[0]);
    let transport = MathTransportV1 {
        contract,
        bound: true,
        capture: None,
    };
    let expected = transport
        .kernel_type(&types, transport.semantic_type(), Some(&context))
        .unwrap();
    let Type::ExecutionCapability(original) = &expected else {
        panic!("expected a bound capability");
    };
    for mutation in 0..3 {
        let mut changed = original.clone();
        match mutation {
            0 => changed.provenance.root = "other_root".into(),
            1 => {
                let ExecutionCapabilityRoleV1::NumericalPolicyMathBound(ref mut binding) =
                    changed.role
                else {
                    unreachable!()
                };
                binding.policy = ExecutionTypeIdentityV1::new([99; 32]);
            }
            _ => changed.source_type = ExecutionTypeIdentityV1::new([98; 32]),
        }
        let ty = Type::ExecutionCapability(changed);
        assert!(
            transport
                .values(
                    &SemanticValueBindingV1::Value {
                        id: ValueId(73),
                        ty: ty.clone()
                    },
                    &[expected.clone()]
                )
                .is_err()
        );
        assert!(
            transport
                .from_values(
                    &types,
                    transport.semantic_type(),
                    &[ValueDef::new(ValueId(73), ty)],
                    &[expected.clone()]
                )
                .is_err()
        );
    }
}
