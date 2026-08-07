use fe2o3_contracts::{
    ControlFlowContractErrorV1, IntegerSwitchCaseV1, IntegerSwitchTypeV1, LoopBoundV1,
};

#[test]
fn loop_bounds_are_finite_and_nonzero() {
    assert_eq!(LoopBoundV1::new(1).unwrap().max_iterations(), 1);
    assert_eq!(
        LoopBoundV1::new(0),
        Err(ControlFlowContractErrorV1::ZeroLoopBound)
    );
}

#[test]
fn fixed_integer_switch_types_have_exact_ranges() {
    for (ty, minimum, maximum) in [
        (IntegerSwitchTypeV1::I8, -128, 127),
        (IntegerSwitchTypeV1::I16, -32_768, 32_767),
        (IntegerSwitchTypeV1::U8, 0, 255),
        (IntegerSwitchTypeV1::U16, 0, 65_535),
    ] {
        assert!(ty.contains(minimum));
        assert!(ty.contains(maximum));
        assert!(!ty.contains(minimum - 1));
        assert!(!ty.contains(maximum + 1));
        assert_eq!(
            IntegerSwitchCaseV1::new(ty, maximum).unwrap().value(),
            maximum
        );
    }

    assert!(IntegerSwitchTypeV1::I128.contains(i128::MIN));
    assert!(IntegerSwitchTypeV1::I128.contains(i128::MAX));
    assert!(IntegerSwitchTypeV1::U128.contains(i128::MAX));
    assert!(!IntegerSwitchTypeV1::U128.contains(-1));
}

#[test]
fn unsupported_widths_and_out_of_range_cases_fail_closed() {
    assert_eq!(
        IntegerSwitchTypeV1::new(32, false).unwrap(),
        IntegerSwitchTypeV1::U32
    );
    assert_eq!(
        IntegerSwitchTypeV1::new(24, false),
        Err(ControlFlowContractErrorV1::UnsupportedIntegerWidth(24))
    );
    assert_eq!(
        IntegerSwitchCaseV1::new(IntegerSwitchTypeV1::I8, 128),
        Err(ControlFlowContractErrorV1::IntegerCaseOutOfRange)
    );
}
