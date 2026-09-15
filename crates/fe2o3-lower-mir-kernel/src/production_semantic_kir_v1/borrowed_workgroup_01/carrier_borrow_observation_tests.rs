// Formatter-only data. These tests do not create a query, plan, or source owner.
fn observation<'a>(
    path: &'a [SemanticProjectionV1],
    events: std::result::Result<&'a [(u32, SsaResolvedEventV1)], QueryError>,
) -> Observation<'a> {
    Observation {
        body: [1; 32],
        view: [2; 32],
        root: 3,
        site: Site {
            block: 5,
            statement: Some(7),
            local: 9,
        },
        source_local: 11,
        source_type: Some(13),
        place_type: 17,
        reference_type: 19,
        same_body: true,
        same_plan: true,
        promoted: false,
        source_block: Some((23, 29, 31)),
        source_statement: Some(SemanticExpandedStatementOriginV1::Source { statement: 37 }),
        original_local: Some((23, 29, 41)),
        place_path: path,
        requested_path: path,
        events,
        query_remaining: 97,
    }
}

fn rendered(o: &Observation<'_>) -> String {
    let mut bytes = Vec::new();
    write_observation(&mut bytes, o).unwrap();
    String::from_utf8(bytes).unwrap()
}

#[test]
fn carrier_observation_flag_is_exact_and_default_off_without_environment_mutation() {
    assert!(!enabled(None));
    for value in ["", "0", "true", " 1", "1\n"] {
        assert!(!enabled(Some(OsStr::new(value))));
    }
    assert!(enabled(Some(OsStr::new("1"))));
}

#[test]
fn original_frame_and_selected_event_window_remain_distinct() {
    let events = [(
        4,
        SsaResolvedEventV1::Kill {
            variable: SsaVariableIdV1::new(11),
            previous: None,
        },
    )];
    let text = rendered(&observation(&[], Ok(&events)));
    assert!(text.contains("block: 5, statement: Some(7), local: 9"));
    assert!(text.contains("block=Some((23, 29, 31))"));
    assert!(text.contains("statement=Some(Source { statement: 37 })"));
    assert!(text.contains("local=Some((23, 29, 41)) source_local=11"));
    assert!(text.contains("same_body=true same_plan=true promoted=false"));
    assert!(text.contains("WINDOW count=1 truncated=false query_remaining=97"));
    assert!(text.contains("EVENT event=4 resolved=Kill"));
    assert!(text.contains("error=NoPromotedUse diagnostic_only=true"));
}

#[test]
fn paths_and_events_are_prefix_bounded_with_explicit_truncation() {
    let projection = SemanticProjectionV1::new(
        SemanticProjectionKindV1::Field(u32::MAX),
        SemanticTypeIdV1::from_index(u32::MAX),
    )
    .unwrap();
    let path = vec![projection; PREFIX + 1];
    let events: Vec<_> = (0..=PREFIX)
        .map(|index| {
            (
                index as u32,
                SsaResolvedEventV1::Kill {
                    variable: SsaVariableIdV1::new(u32::MAX),
                    previous: None,
                },
            )
        })
        .collect();
    let text = rendered(&observation(&path, Ok(&events)));
    assert_eq!(text.matches("count=17 truncated=true").count(), 3);
    assert_eq!(text.matches("WORKGROUP_BORROW_EVENT ").count(), PREFIX);
    assert!(!text.contains("EVENT event=16 "));
    assert!(text.len() < 16_384);
}

#[test]
fn unavailable_window_and_output_failure_do_not_produce_a_replacement_use() {
    for error in [
        QueryError::NoPromotedUse,
        QueryError::WrongOwner,
        QueryError::WorkLimit,
    ] {
        let text = rendered(&observation(&[], Err(error)));
        assert!(text.contains(&format!("WORKGROUP_BORROW_WINDOW error={error:?}")));
        assert!(!text.contains("WORKGROUP_BORROW_EVENT "));
    }
    struct Broken;
    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::ErrorKind::WriteZero.into())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    assert!(write_observation(&mut Broken, &observation(&[], Ok(&[]))).is_err());
}
