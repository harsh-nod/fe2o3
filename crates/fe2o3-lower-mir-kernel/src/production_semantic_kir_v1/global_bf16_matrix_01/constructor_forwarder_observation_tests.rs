fn context() -> observation::Context {
    observation::Context {
        root: 17,
        view: [11; 32],
        expanded_body: [12; 32],
        wrapper: (3, 4),
        checked_instance: 5,
        construction: (6, Some(2), 9),
    }
}

fn render(body: &SemanticFunctionDeclV1, call_block: Option<SemanticBlockIdV1>) -> String {
    let checked = SemanticFunctionIdV1::from_index(7);
    let callables = [SemanticCallableDeclV1::defined(checked)];
    let before = exact_forwarder_at(body, &callables, checked, call_block);
    let mut bytes = Vec::new();
    observation::write_if_enabled(
        Some(std::ffi::OsStr::new("1")),
        &mut bytes,
        body,
        &callables,
        checked,
        call_block,
        &context(),
    )
    .unwrap();
    assert_eq!(
        exact_forwarder_at(body, &callables, checked, call_block),
        before
    );
    let output = String::from_utf8(bytes).unwrap();
    assert_eq!(output.lines().count(), 5);
    assert!(
        output.len() < 4096,
        "bounded primitive record, no body or constant Debug dump"
    );
    output
}

#[test]
fn forwarding_observer_requires_exact_opt_in() {
    let body = fixture(Mutation::None);
    for flag in [
        None,
        Some(""),
        Some("0"),
        Some("true"),
        Some("01"),
        Some("1 "),
    ] {
        let mut bytes = Vec::new();
        observation::write_if_enabled(
            flag.map(std::ffi::OsStr::new),
            &mut bytes,
            &body,
            &[],
            SemanticFunctionIdV1::from_index(7),
            None,
            &context(),
        )
        .unwrap();
        assert!(bytes.is_empty());
    }
    assert!(render(&body, Some(body.entry())).contains("diagnostic_only=true"));
}

#[test]
fn forwarding_observer_records_canonical_frame_and_all_predicate_axes() {
    let body = reordered(Mutation::None);
    let output = render(&body, Some(body.entry()));
    let tags: Vec<_> = output
        .lines()
        .map(|line| line.split_whitespace().next().unwrap())
        .collect();
    assert_eq!(
        tags,
        [
            "BF16_FORWARDER_FRAME",
            "BF16_FORWARDER_SHAPE",
            "BF16_FORWARDER_LOCALS",
            "BF16_FORWARDER_CALL",
            "BF16_FORWARDER_ARGUMENTS"
        ]
    );
    assert!(output.contains("root=17 wrapper=(3, 4) checked_instance=5 checked_function=7 construction=(6, Some(2), 9) call_block=Some(1) entry=1"));
    assert!(output.contains("inputs=6 locals=7 blocks=2 statements_prefix=[Some(0), Some(0)]"));
    assert!(output.contains("ownership_count=6 ownership_prefix=[SharedBorrow, SharedBorrow, ByValue, ByValue, ByValue, ByValue]"));
    assert!(
        output.contains("BF16_FORWARDER_LOCALS prefix=[Some((3, Return)), Some((0, Argument(0)))")
    );
    assert!(output.contains("callee_kind=Defined callee_function=Some(7) arguments=5 variadic_abis=0 unwind_unreachable=true destination=Some((0, 3, 0, 0, CallReturn, true))"));
    assert!(output.contains("Some((\"Copy\", Some(2), 1, Some(0)))"));
    assert!(output.contains("Some((\"Copy\", Some(6), 2, Some(0)))"));
}

#[test]
fn forwarding_observer_does_not_change_any_existing_rejection() {
    for mutation in [
        Mutation::ForwardReceiver,
        Mutation::ReorderGeometry,
        Mutation::MoveGlobal,
        Mutation::MutableReceiver,
        Mutation::MutableGlobal,
        Mutation::ReturnOtherLocal,
        Mutation::InsertStatement,
        Mutation::ChangedReturn,
        Mutation::ExtraBlock,
        Mutation::WrongLocalRole,
    ] {
        let body = fixture(mutation);
        assert!(
            exact_forwarder_at(
                &body,
                &[SemanticCallableDeclV1::defined(
                    SemanticFunctionIdV1::from_index(7)
                )],
                SemanticFunctionIdV1::from_index(7),
                Some(body.entry())
            )
            .is_none()
        );
        render(&body, Some(body.entry()));
    }
    let moved = fixture(Mutation::MoveGlobal);
    assert!(render(&moved, Some(moved.entry())).contains("Some((\"Move\", Some(2), 1, Some(0)))"));
    let mutable = fixture(Mutation::MutableGlobal);
    assert!(
        render(&mutable, Some(mutable.entry()))
            .contains("ownership_prefix=[SharedBorrow, UniqueBorrow, ByValue")
    );
    let role = fixture(Mutation::WrongLocalRole);
    assert!(render(&role, Some(role.entry())).contains("Some((0, Temporary))"));
}

#[test]
fn forwarding_observer_missing_call_and_frame_mismatch_stay_five_lines() {
    let original = fixture(Mutation::None);
    let body = with_blocks(
        &original,
        SemanticBlockIdV1::from_index(1),
        original.blocks().to_vec(),
    );
    let output = render(&body, None);
    assert!(output.contains("call_block=None entry=1"));
    assert!(output.contains("BF16_FORWARDER_CALL entry_call=false\nBF16_FORWARDER_ARGUMENTS count=0 available=false prefix=[]"));
    let output = render(&original, Some(SemanticBlockIdV1::from_index(1)));
    assert!(output.contains("call_block=Some(1) entry=0"));
}
