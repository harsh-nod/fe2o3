fn observation<'a>(
    place_path: &'a [SemanticProjectionV1],
    requested_path: &'a [SemanticProjectionKindV1],
) -> Observation<'a> {
    Observation {
        body: [0xab; 32],
        target: "Global",
        site: CapabilityDefinitionSiteV1 {
            block: 5,
            statement: Some(2),
            local: 21,
        },
        consumer: CapabilityDefinitionSiteV1 {
            block: 25,
            statement: None,
            local: 75,
        },
        value: SsaValueV1::Definition(fe2o3_mir_model::SsaDefinitionIdV1::new(19)),
        root: Some(17),
        view: Some([0xcd; 32]),
        query: "checked",
        same_plan: Some(true),
        promoted: Some(false),
        source_block: Some((2, 11, 0)),
        source_statement: Some(SemanticExpandedStatementOriginV1::Source { statement: 4 }),
        source_local: Some((2, 11, 3)),
        owner_local: 20,
        owner_type: Some(29),
        owner_role: Some(SemanticLocalRoleV1::Temporary),
        place_type: 29,
        reference_type: 41,
        events: Some(26),
        place_path,
        requested_path,
    }
}

fn render(observation: &Observation<'_>) -> String {
    let mut bytes = Vec::new();
    write_observation(&mut bytes, observation).unwrap();
    String::from_utf8(bytes).unwrap()
}

#[test]
fn bf16_borrow_observer_requires_exact_opt_in() {
    assert!(enabled(Some(OsStr::new("1"))));
    for value in [
        None,
        Some(""),
        Some("0"),
        Some("true"),
        Some("01"),
        Some("1\n"),
    ] {
        assert!(!enabled(value.map(OsStr::new)));
    }
}

#[test]
fn bf16_borrow_observer_parses_distinct_original_and_expanded_coordinates() {
    let output = render(&observation(&[], &[SemanticProjectionKindV1::Dereference]));
    let lines = output.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), 5);
    assert_eq!(
        lines
            .iter()
            .map(|line| line.split_once(' ').unwrap().0)
            .collect::<Vec<_>>(),
        [
            "BF16_BORROW_BOUNDARY",
            "BF16_BORROW_OWNER",
            "BF16_BORROW_SOURCE",
            "BF16_BORROW_PLACE",
            "BF16_BORROW_REQUEST"
        ]
    );
    assert!(lines[0].starts_with(&format!(
        "BF16_BORROW_BOUNDARY body={} target=Global",
        "ab".repeat(32)
    )));
    assert!(lines[0].contains("block: 5, statement: Some(2), local: 21"));
    assert!(lines[0].ends_with("diagnostic_only=true"));
    assert!(lines[1].contains(&format!(
        "view={} query=checked same_plan=Some(true)",
        "cd".repeat(32)
    )));
    assert!(lines[1].ends_with("promoted=Some(false) events=Some(26)"));
    assert!(lines[2].contains(
        "block=Some((2, 11, 0)) statement=Some(Source { statement: 4 }) local=Some((2, 11, 3))"
    ));
    assert!(lines[2].ends_with("owner_local=20 owner_type=Some(29) owner_role=Some(Temporary) place_type=29 reference_type=41"));
    assert_eq!(
        lines[3],
        "BF16_BORROW_PLACE count=0 truncated=false prefix=[]"
    );
    assert_eq!(
        lines[4],
        "BF16_BORROW_REQUEST count=1 truncated=false prefix=[Dereference]"
    );
}

#[test]
fn bf16_borrow_observer_caps_both_projection_paths_before_formatting() {
    let projection = SemanticProjectionV1::new(
        SemanticProjectionKindV1::ConstantIndex {
            offset: u64::MAX - 1,
            minimum_length: u64::MAX,
            from_end: true,
        },
        SemanticTypeIdV1::from_index(u32::MAX),
    )
    .unwrap();
    let path = [projection; 256];
    let mut requested = [SemanticProjectionKindV1::Field(u32::MAX); 256];
    requested[PREFIX] = SemanticProjectionKindV1::Downcast(123456789);
    let mut item = observation(&path, &requested);
    item.owner_local = u32::MAX;
    item.source_block = Some((u32::MAX, u32::MAX, u32::MAX));
    let output = render(&item);
    assert_eq!(output.lines().count(), 5);
    assert_eq!(output.matches("ConstantIndex").count(), PREFIX);
    assert_eq!(output.matches("Field(4294967295)").count(), PREFIX);
    assert!(!output.contains("123456789"));
    assert_eq!(output.matches("count=256 truncated=true").count(), 2);
    assert!(
        output.len() < 4096,
        "bounded scalar metadata and two eight-item prefixes"
    );
}

#[test]
fn bf16_borrow_observer_does_not_invent_missing_owner_or_promotion() {
    let mut item = observation(&[], &[]);
    item.root = None;
    item.view = None;
    item.query = "unavailable";
    item.same_plan = None;
    item.promoted = None;
    item.source_block = None;
    item.source_statement = None;
    item.source_local = None;
    let output = render(&item);
    assert!(
        output
            .lines()
            .nth(1)
            .unwrap()
            .contains("root=None view=unavailable query=unavailable same_plan=None promoted=None")
    );
    assert!(
        output
            .lines()
            .nth(2)
            .unwrap()
            .starts_with("BF16_BORROW_SOURCE block=None statement=None local=None")
    );
    assert!(!output.contains("query=checked"));
}

#[test]
fn bf16_borrow_observer_writer_failure_produces_no_value_or_loan() {
    let mut full = &mut [0u8; 0][..];
    assert_eq!(
        write_observation(&mut full, &observation(&[], &[]))
            .unwrap_err()
            .kind(),
        io::ErrorKind::WriteZero
    );
}
