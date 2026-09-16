#[test]
fn race_numeric_names_ignore_hostile_debug_aliases_without_hiding_conflicts() {
    use dialect_kernel::{DIALECT_NAME, RankedViewType, ReturnOp, register_dialect};
    use pliron::{builtin::types::FunctionType, dialect::DialectName, op::Op};

    let context = &mut Context::new();
    register_dialect(context, &DialectName::try_new(DIALECT_NAME).unwrap()).unwrap();
    let ty = FunctionType::get(context, vec![], vec![]);
    let function = FuncOp::new(context, "race_numeric_names".try_into().unwrap(), ty);
    let entry = function.get_entry_block(context);
    let invocation = InvocationIndexOp::new(context, 0, 2);
    invocation.get_operation().insert_at_back(entry, context);
    let index = IndexConstantOp::new(context, 0);
    index.get_operation().insert_at_back(entry, context);
    let ty = RankedViewType::new(context, 32, true, vec![1]).unwrap();
    let view = RankedViewOp::new(context, ty, vec![]).unwrap();
    view.get_operation().insert_at_back(entry, context);
    let access = RankedAccessOp::new(
        context,
        AccessKindAttr::Write,
        view.result(context),
        vec![index.result(context)],
    )
    .unwrap();
    access.get_operation().insert_at_back(entry, context);
    ReturnOp::new(context)
        .get_operation()
        .insert_at_back(entry, context);
    let expected = run_pliron_ranked_race_check_v1(context, &function);
    assert!(matches!(
        expected.findings(),
        [RankedRaceFindingV1::ConflictingEffects { .. }]
    ));
    for alias in ["short_alias".to_owned(), "d".repeat(262_144)] {
        for value in [view.result(context), index.result(context)] {
            value.set_name(context, Some(alias.clone().try_into().unwrap()));
        }
        let observed = run_pliron_ranked_race_check_v1(context, &function);
        assert_eq!(observed, expected);
        let [RankedRaceFindingV1::ConflictingEffects { view: name, .. }] = observed.findings()
        else {
            panic!("the real write race must still be rejected");
        };
        assert!(name.starts_with('v'));
        assert!(name[1..].bytes().all(|byte| byte.is_ascii_digit()));
        assert!(name.capacity() <= NUMERIC_RACE_NAME_BYTES_V1);
        assert!(!observed.findings()[0].to_string().contains(&alias));
    }
}

#[test]
fn race_allocation_origin_name_is_bounded_at_maximum_u64() {
    for origin in [0, 1, u64::MAX] {
        let name = allocation_origin_name_v1(origin);
        assert_eq!(name, format!("allocation origin {origin}"));
        assert!(name.len() <= NUMERIC_RACE_NAME_BYTES_V1);
        assert_eq!(name.capacity(), NUMERIC_RACE_NAME_BYTES_V1);
    }
}
