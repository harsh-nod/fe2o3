// Inert diagnostics only; the real source and final-F checks happen in process.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct HistoryRequest {
    schema: u16,
    route: String,
    run_id: String,
    case: HistoryCase,
    target: Target,
    captured: [u8; 32],
    executed: [u8; 32],
    cwd: PathBuf,
    source: [FileStamp; 4],
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct HistoryRound {
    ordinal: u16,
    input: Subject,
    integer: Subject,
    output: Subject,
    changed: bool,
    integer_passes: [u8; 2],
    scalar_passes: [u8; 8],
    maps: [[u8; 32]; 2],
    executions: [[u8; 32]; 2],
    occurrences: [[usize; 9]; 2],
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
enum Placement {
    Retained([u32; 3]),
    Internal([u32; 3]),
    Omitted,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct HistoryAssertion {
    original: AssertionRow,
    disposition: u8,
    condition: Option<[u32; 3]>,
    definition: Option<[u32; 5]>,
    failure: Option<[u32; 3]>,
    success: Placement,
    selection: Option<[u16; 2]>,
    removal: Option<[u16; 2]>,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct SpanRow {
    association: usize,
    block: [u32; 2],
    kind: u8,
    source: [u32; 4],
    operations: [usize; 2],
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct OperationRow {
    output: [u32; 3],
    original: Option<[u32; 3]>,
    synthesis: Option<(u16, bool, [u32; 5])>,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct DescendantRow {
    original: [u32; 5],
    output: [u32; 5],
    retained: bool,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct SegmentRow {
    output: [u32; 2],
    original: [u32; 2],
    connector: Option<[u32; 3]>,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct BlockRow {
    original: [u32; 2],
    placement: Option<[u32; 3]>,
    reachable: bool,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ControlRow {
    original: [u32; 3],
    placement: Placement,
    executable: bool,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Sizes {
    assertions: usize,
    spans: usize,
    associations: usize,
    functions: usize,
    operations: usize,
    descendants: usize,
    segments: usize,
    blocks: usize,
    controls: usize,
    uses: usize,
    edges: usize,
    arguments: usize,
    pairs: usize,
    incoming: usize,
    definitions: usize,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct HistoryRows {
    original: Vec<AssertionRow>,
    assertions: Vec<HistoryAssertion>,
    spans: Vec<SpanRow>,
    functions: Vec<[u32; 2]>,
    operations: Vec<OperationRow>,
    descendants: Vec<DescendantRow>,
    segments: Vec<SegmentRow>,
    blocks: Vec<BlockRow>,
    controls: Vec<ControlRow>,
    uses: Vec<[[u32; 5]; 3]>,
    edges: Vec<[[u32; 3]; 2]>,
    arguments: Vec<[[u32; 4]; 2]>,
    pairs: Vec<PairRow>,
    incoming: Vec<IncomingRow>,
    definitions: Vec<DefinitionRow>,
    final_edges: Vec<EdgeRow>,
    final_payloads: Vec<u32>,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct GraphRow {
    subject: Subject,
    edges: Vec<EdgeRow>,
    payloads: Vec<u32>,
    definitions: Vec<u32>,
    symbols: Vec<String>,
    kernels: Vec<String>,
    // Actual switches, masks, shifts, checked adds, blocks and body definitions.
    counts: [usize; 6],
    memory: [usize; 3],
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct HistoryObservation {
    actual_target: Target,
    fixture: Origin,
    roots: Vec<Root>,
    original: GraphRow,
    after: Subject,
    final_graph: GraphRow,
    policy_subject: Subject,
    rounds: Vec<HistoryRound>,
    rows: HistoryRows,
    sizes: Sizes,
    semantic: [usize; 3],
    source_assertions: usize,
    owner_floors: [usize; 3],
    diagnostics_floor: usize,
    restored_floor: usize,
    source_resources: [usize; 3],
    policy_resources: [usize; 3],
    no_denial: bool,
    pending: usize,
    complete: bool,
    authority: bool,
    final_source_storage: usize,
    protocol_storage: usize,
    work: usize,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct HistoryReport {
    request: HistoryRequest,
    callbacks: usize,
    result: Result<HistoryObservation, String>,
}
fn history_executed(captured: &[String], case: HistoryCase) -> Result<Vec<String>, String> {
    require_canonical_overflow_checks_v1(captured).map_err(|e| e.to_string())?;
    require(
        !captured.iter().any(|arg| {
            arg.contains("fe2o3_scalar_fixed_point_")
                || arg.contains("fe2o3_canonical_scalar_")
                || arg.contains("fe2o3_canonical_assertion_")
                || arg.contains("mir-opt-level")
                || arg.contains("inline-mir")
        }),
        "preexisting case cfg or MIR override",
    )?;
    let mut result = captured.to_vec();
    for cfg in CASE_CFGS.into_iter().chain([H_RETAINED_CFG]) {
        result.push(format!("--check-cfg=cfg({cfg})"));
    }
    result.push(format!("--cfg={}", case.cfg()));
    result.push("-Zmir-opt-level=0".into());
    result.push("-Zinline-mir=no".into());
    Ok(result)
}
fn history_check_request(request: &HistoryRequest, invocation: &Invocation) -> Result<(), String> {
    require(
        request.schema == 1
            && request.route == H_ROUTE
            && !request.run_id.is_empty()
            && request.run_id.len() <= 256
            && request.source == stamps()?
            && request.cwd == env::current_dir().map_err(|e| e.to_string())?
            && request
                .source
                .iter()
                .map(|s| &s.path)
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                == 4
            && request.captured == args_hash(&invocation.captured)
            && request.executed == args_hash(&invocation.executed)
            && invocation.executed == history_executed(&invocation.captured, request.case)?,
        "exact history source/cwd/argv binding",
    )?;
    require(
        options(&invocation.captured, "-Ctarget-cpu") == [request.target.cpu()]
            && options(&invocation.captured, "--crate-name")
                == ["fe2o3_production_extraction_fixture"]
            && invocation
                .captured
                .iter()
                .filter(|arg| {
                    request.cwd.join(arg).canonicalize().ok()
                        == Some(request.source[2].path.clone())
                })
                .count()
                == 1,
        "captured history target/crate/source entry",
    )
}
fn history_graph(graph: &GraphRow) -> Result<(), String> {
    require(
        graph.subject.digest != [0; 32]
            && graph.subject.bytes > 0
            && graph.counts[5] == graph.definitions.len()
            && !graph.definitions.is_empty()
            && !graph.symbols.is_empty()
            && graph.memory == [0; 3],
        "real scalar graph census",
    )?;
    let mut end = 0;
    let mut seen = std::collections::BTreeSet::new();
    for edge in &graph.edges {
        require(
            seen.insert(edge.coordinate)
                && edge.payload[0] == end
                && edge.payload[0] <= edge.payload[1]
                && edge.payload[1] <= graph.payloads.len(),
            "ordered successor occurrence extent",
        )?;
        end = edge.payload[1];
    }
    require(
        end == graph.payloads.len(),
        "complete successor payload census",
    )
}
fn history_edge(graph: &GraphRow, coordinate: [u32; 3]) -> Result<&EdgeRow, String> {
    let mut rows = graph.edges.iter().filter(|r| r.coordinate == coordinate);
    let row = rows.next().ok_or("missing successor occurrence")?;
    require(rows.next().is_none(), "ambiguous successor occurrence")?;
    Ok(row)
}
fn history_validate(request: &HistoryRequest, row: &HistoryObservation) -> Result<(), String> {
    require(
        row.actual_target == request.target
            && row.roots.len() == 1
            && row.roots[0].name == request.case.root()
            && row.roots[0].function != [0; 32]
            && row.roots[0].body != [0; 32]
            && !row.roots[0].entry.is_empty(),
        "actual history source roots/profile",
    )?;
    history_graph(&row.original)?;
    history_graph(&row.final_graph)?;
    require(
        row.original.subject == row.after
            && row.final_graph.subject == row.policy_subject
            && row.original.symbols == row.final_graph.symbols
            && row.original.kernels == row.final_graph.kernels
            && row.original.kernels == [row.roots[0].entry.clone()]
            && row.fixture.file != [0; 32]
            && row.fixture.bytes[0] == 0
            && row.fixture.bytes[1]
                == std::fs::metadata(&request.source[3].path)
                    .map_err(|e| e.to_string())?
                    .len(),
        "separate N/F custody and preserved rosters",
    )?;
    require(
        !row.rounds.is_empty()
            && row.rounds.len() <= fe2o3_kernel_opt::SCALAR_FIXED_POINT_MAX_ROUNDS_V1,
        "complete actual history extent",
    )?;
    let mut input = row.original.subject;
    for (ordinal, round) in row.rounds.iter().enumerate() {
        require(
            usize::from(round.ordinal) == ordinal
                && round.input == input
                && round.integer.bytes > 0
                && round.integer.digest != [0; 32]
                && round.output.bytes > 0
                && round.output.digest != [0; 32]
                && round.integer_passes == [0, 1]
                && round.scalar_passes == [2, 3, 4, 1, 5, 6, 1, 3]
                && round.maps.iter().all(|d| *d != [0; 32])
                && round.executions.iter().all(|d| *d != [0; 32])
                && round.occurrences.iter().all(|c| c[0] > 0)
                && round.changed == (round.input != round.output)
                && round.changed
                    == (ordinal.checked_add(1).ok_or("round overflow")? != row.rounds.len()),
            "actual ordered substages and terminal round",
        )?;
        input = round.output;
    }
    require(
        input == row.final_graph.subject,
        "actual final scalar owner",
    )?;
    let s = row.sizes;
    let r = &row.rows;
    require(
        s.assertions == r.original.len()
            && s.assertions == r.assertions.len()
            && s.assertions == row.source_assertions
            && s.assertions > 0
            && s.spans == r.spans.len()
            && s.associations == 1
            && s.functions == r.functions.len()
            && s.operations == r.operations.len()
            && s.descendants == r.descendants.len()
            && s.segments == r.segments.len()
            && s.blocks == r.blocks.len()
            && s.controls == r.controls.len()
            && s.uses == r.uses.len()
            && s.edges == r.edges.len()
            && s.arguments == r.arguments.len()
            && s.pairs == r.pairs.len()
            && s.incoming == r.incoming.len()
            && s.definitions == r.definitions.len()
            && r.final_edges == row.final_graph.edges
            && r.final_payloads == row.final_graph.payloads,
        "complete fresh source/lineage/final rosters",
    )?;
    require(
        r.spans.iter().any(|s| s.operations[0] == s.operations[1])
            && r.spans.iter().all(|s| {
                s.association < row.sizes.associations
                    && s.kind <= 2
                    && s.operations[0] <= s.operations[1]
            })
            && r.functions.len() == row.final_graph.symbols.len()
            && r.functions
                .iter()
                .enumerate()
                .all(|(n, f)| f[0] as usize == n && f[1] as usize == n),
        "all source spans and exact function lineage",
    )?;
    let mut aliases = std::collections::BTreeSet::new();
    for (original, final_row) in r.original.iter().zip(&r.assertions) {
        let mut proved = *original;
        proved.proof = final_row.original.proof;
        require(
            proved == final_row.original
                && proved.proof != ProofRow::Other
                && aliases.insert(original.source)
                && !original.helper
                && original.association < s.associations
                && original.span < r.spans.len()
                && origin_valid(&original.origin, &row.fixture)
                && origin_valid(&original.expansion, &row.fixture),
            "complete exact original source assertion join",
        )?;
        let source_span = &r.spans[original.span];
        require(
            source_span.kind == 1
                && source_span.association == original.association
                && source_span.source[..3] == original.source,
            "root-qualified source span association",
        )?;
        history_edge(&row.original, original.success)?;
        if let Some(failure) = original.failure {
            let failed = history_edge(&row.original, failure)?;
            require(
                original.success[..2] == failure[..2]
                    && original.success[2] == u32::from(!original.expected)
                    && failure[2] == u32::from(original.expected)
                    && original.condition == Some([failure[0], failure[1], 0])
                    && original.definition.is_some()
                    && failed.payload[0] == failed.payload[1],
                "original condition/polarity occurrences",
            )?;
        } else {
            require(
                original.condition.is_none()
                    && original.definition.is_none()
                    && original.success[2] == 0,
                "source-elided original binding",
            )?;
        }
        let controls = r
            .controls
            .iter()
            .filter(|c| c.original == original.success)
            .collect::<Vec<_>>();
        require(
            controls.len() == 1 && controls[0].placement == final_row.success,
            "actual original-to-final success occurrence",
        )?;
        require(
            final_row.removal.is_none(),
            "reachable fixtures cannot claim removal",
        )?;
        match final_row.disposition {
            0 => {
                require(
                    final_row.selection.is_none()
                        && final_row.condition.is_some()
                        && final_row.definition.is_some()
                        && final_row.failure.is_some(),
                    "retained is not selected or omitted",
                )?;
                let Placement::Retained(success) = final_row.success else {
                    return Err("retained assertion needs final edge".into());
                };
                require(
                    r.incoming
                        .iter()
                        .filter(|i| {
                            Some(i.condition) == final_row.condition
                                && Some(i.definition) == final_row.definition
                                && Some(i.failure) == final_row.failure
                                && i.success == success
                                && i.expected == original.expected
                        })
                        .count()
                        == 1,
                    "exact retained final incoming join",
                )?;
            }
            1 | 2 => {
                require(
                    final_row.condition.is_none()
                        && final_row.definition.is_none()
                        && final_row.failure.is_none()
                        && final_row.success != Placement::Omitted,
                    "elided success keeps actual route",
                )?;
                if final_row.disposition == 1 {
                    require(
                        original.failure.is_none() && final_row.selection.is_none(),
                        "source elision cannot invent history selection",
                    )?;
                } else {
                    require(
                        original.failure.is_some() && final_row.selection == Some([0, 0]),
                        "first scalar-substage asserted-success selection",
                    )?;
                }
            }
            _ => return Err("unsupported assertion disposition".into()),
        }
        match final_row.success {
            Placement::Retained(edge) => {
                history_edge(&row.final_graph, edge)?;
            }
            Placement::Internal(place) => require(
                r.segments.iter().any(|segment| {
                    segment.output == place[..2] && segment.connector == Some(original.success)
                }),
                "actual internal success connector",
            )?,
            Placement::Omitted => return Err("live success omitted".into()),
        }
    }
    let mut incoming_end = 0;
    for pair in &r.pairs {
        require(
            pair.incoming[0] == incoming_end
                && pair.incoming[0] < pair.incoming[1]
                && pair.incoming[1] <= r.incoming.len(),
            "ordered final trap incoming roster",
        )?;
        for incoming in &r.incoming[pair.incoming[0]..pair.incoming[1]] {
            let failure = history_edge(&row.final_graph, incoming.failure)?;
            history_edge(&row.final_graph, incoming.success)?;
            require(
                failure.target == pair.call[..2]
                    && failure.payload[0] == failure.payload[1]
                    && r.assertions
                        .iter()
                        .filter(|a| {
                            a.condition == Some(incoming.condition)
                                && a.definition == Some(incoming.definition)
                                && a.failure == Some(incoming.failure)
                                && a.success == Placement::Retained(incoming.success)
                                && a.original.expected == incoming.expected
                        })
                        .count()
                        == 1,
                "bidirectional final trap coverage",
            )?;
        }
        incoming_end = pair.incoming[1];
    }
    require(incoming_end == r.incoming.len(), "unclaimed final incoming")?;
    require(
        r.controls.len() == row.original.edges.len()
            && r.controls
                .iter()
                .zip(&row.original.edges)
                .all(|(c, e)| c.original == e.coordinate)
            && r.edges.len() == row.final_graph.edges.len()
            && r.edges.iter().zip(&row.final_graph.edges).all(|(e, f)| {
                e[0] == f.coordinate && row.original.edges.iter().any(|n| n.coordinate == e[1])
            }),
        "complete successor lineage rosters",
    )?;
    for pair in &r.arguments {
        let output = history_edge(&row.final_graph, [pair[0][0], pair[0][1], pair[0][2]])?;
        let original = history_edge(&row.original, [pair[1][0], pair[1][1], pair[1][2]])?;
        require(
            (pair[0][3] as usize) < output.payload[1] - output.payload[0]
                && (pair[1][3] as usize) < original.payload[1] - original.payload[0],
            "ordered payload occurrence lineage",
        )?;
    }
    let mut total = [0usize; 3];
    require(
        r.definitions.len() == row.final_graph.definitions.len(),
        "all actual final definitions",
    )?;
    for (ordinal, definition) in r.definitions.iter().enumerate() {
        require(
            definition.ordinal == ordinal
                && definition.function == row.final_graph.definitions[ordinal]
                && definition.history_function == definition.function as usize
                && definition.stages == [0, 1, 2, 3, 4, 5, 6, 7, 8]
                && definition.paired == 9
                && definition.clean
                && !definition.denied
                && !definition.panicked
                && definition.floor == total
                && definition.invocation[0] > 0
                && definition.invocation[2] >= definition.invocation[1],
            "fresh complete final-F fixed-nine invocation",
        )?;
        total = [
            total[0]
                .checked_add(definition.invocation[0])
                .ok_or("native work overflow")?,
            total[1]
                .checked_add(definition.invocation[1])
                .ok_or("native storage overflow")?,
            total[2].max(
                total[1]
                    .checked_add(definition.invocation[2])
                    .ok_or("native peak overflow")?,
            ),
        ];
    }
    require(total == row.policy_resources, "final native aggregate")?;
    let owner_floor = row.owner_floors[0]
        .checked_add(row.owner_floors[1])
        .ok_or("history owner floor overflow")?;
    let restored = owner_floor
        .checked_add(row.diagnostics_floor)
        .ok_or("history diagnostic floor overflow")?;
    require(
        row.owner_floors[0] > 0
            && row.owner_floors[1] > 0
            && owner_floor == row.owner_floors[2]
            && restored == row.restored_floor
            && row.diagnostics_floor > 0
            && row.protocol_storage > 0
            && row.final_source_storage == 0
            && row.work > 0
            && row.no_denial
            && row.source_resources[0] <= row.work
            && row.source_resources[1] > restored
            && row.source_resources[2] >= row.source_resources[1]
            && row.pending == 19
            && !row.complete
            && !row.authority,
        "paid source/history/diagnostics and unchanged authority",
    )?;
    let a = &r.assertions;
    match request.case {
        HistoryCase::ArithmeticHistoryElided => require(
            a.len() == 2
                && a.iter().all(|a| {
                    a.disposition == 2
                        && !a.original.expected
                        && a.original.proof == ProofRow::CheckedArithmetic
                })
                && a.iter().any(|a| a.original.message == MessageRow::Add)
                && a.iter().any(|a| a.original.message == MessageRow::Multiply)
                && row.original.subject != row.final_graph.subject
                && row.rounds.len() >= 2
                && history_edge(&row.original, a[0].original.failure.ok_or("first N trap")?)?
                    .target
                    == history_edge(&row.original, a[1].original.failure.ok_or("second N trap")?)?
                        .target
                && r.pairs.is_empty(),
            "real arithmetic identity history elision",
        )?,
        HistoryCase::LiteralHistoryElided => require(
            a.len() == 1
                && a[0].disposition == 2
                && a[0].original.expected
                && a[0].original.signed_literal
                && a[0].original.proof == ProofRow::LiteralShift
                && a[0].original.message == MessageRow::Shift
                && row.semantic[2] > 0
                && row.original.counts[2] > 0
                && row.original.subject != row.final_graph.subject
                && row.rounds.len() >= 2
                && r.pairs.is_empty(),
            "real signed literal history selection",
        )?,
        HistoryCase::MaskedSourceElided => require(
            a.len() == 1
                && a[0].disposition == 1
                && a[0].original.expected
                && a[0].original.proof == ProofRow::ExactRange
                && a[0].original.message == MessageRow::Shift
                && row.semantic.iter().all(|n| *n > 0)
                && row.original.counts[..3].iter().all(|n| *n > 0)
                && r.pairs.is_empty(),
            "genuine masked source elision",
        )?,
        HistoryCase::MaskedAddRetained => require(
            a.len() == 1
                && a[0].disposition == 0
                && !a[0].original.expected
                && a[0].original.proof == ProofRow::CheckedArithmetic
                && a[0].original.message == MessageRow::Add
                && row.semantic[1] > 0
                && row.original.counts[1] > 0
                && row.original.counts[3] > 0
                && row.final_graph.counts[1] > 0
                && row.final_graph.counts[3] > 0
                && r.pairs.len() == 1
                && r.incoming.len() == 1,
            "genuine masked-add final retained selector",
        )?,
    }
    Ok(())
}
fn history_decode(
    status: Option<i32>,
    bytes: Option<&[u8]>,
    expected: &HistoryRequest,
) -> Result<HistoryObservation, String> {
    require(status == Some(0), "exact zero history child exit required")?;
    let bytes = bytes.ok_or("missing fresh history report")?;
    require(bytes.len() <= REPORT_CAP, "oversized history report")?;
    let report: HistoryReport = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    require(
        report.request == *expected && report.callbacks == 1,
        "stale/foreign history request or callback count",
    )?;
    let row = report.result?;
    history_validate(expected, &row)?;
    Ok(row)
}
fn history_audit_protocol(
    request: &HistoryRequest,
    invocation: &Invocation,
    report: &HistoryReport,
) {
    history_check_request(request, invocation).unwrap();
    let baseline = serde_json::to_vec(report).unwrap();
    history_decode(Some(0), Some(&baseline), request).unwrap();
    let accepted = || {
        history_check_request(request, invocation).unwrap();
        history_decode(Some(0), Some(&baseline), request).unwrap();
    };
    for mutation in 0..10 {
        let mut changed = request.clone();
        match mutation {
            0 => changed.source[3].sha256[0] ^= 1,
            1 => changed.schema += 1,
            2 => changed.run_id.clear(),
            3 => changed.cwd.push("foreign-cwd"),
            4 => changed.captured[0] ^= 1,
            5 => changed.executed[0] ^= 1,
            6 => changed.source[2] = changed.source[3].clone(),
            7 => {
                changed.target = if request.target == Target::Gfx942 {
                    Target::Gfx950
                } else {
                    Target::Gfx942
                }
            }
            8 => {
                changed.case = if request.case == HistoryCase::MaskedAddRetained {
                    HistoryCase::LiteralHistoryElided
                } else {
                    HistoryCase::MaskedAddRetained
                }
            }
            _ => changed.route = "original-N-B".into(),
        }
        assert!(
            history_check_request(&changed, invocation).is_err(),
            "request {mutation}"
        );
        accepted();
    }
    let other = if request.target == Target::Gfx942 {
        Target::Gfx950
    } else {
        Target::Gfx942
    };
    for (option, value) in [
        ("--crate-name", "foreign_fixture"),
        ("-Ctarget-cpu", other.cpu()),
    ] {
        let mut args = invocation.captured.clone();
        let n = args
            .iter()
            .position(|a| a == option || a.starts_with(&format!("{option}=")))
            .unwrap();
        if args[n] == option {
            args[n + 1] = value.into();
        } else {
            args[n] = format!("{option}={value}");
        }
        let changed = Invocation {
            executed: history_executed(&args, request.case).unwrap(),
            captured: args,
        };
        let mut r = request.clone();
        r.captured = args_hash(&changed.captured);
        r.executed = args_hash(&changed.executed);
        assert!(history_check_request(&r, &changed).is_err());
        accepted();
    }
    let mut changed = invocation.clone();
    let n = changed
        .captured
        .iter()
        .position(|a| {
            request.cwd.join(a).canonicalize().ok() == Some(request.source[2].path.clone())
        })
        .unwrap();
    changed.captured[n] = request.source[3].path.to_str().unwrap().into();
    changed.executed = history_executed(&changed.captured, request.case).unwrap();
    let mut r = request.clone();
    r.captured = args_hash(&changed.captured);
    r.executed = args_hash(&changed.executed);
    assert!(history_check_request(&r, &changed).is_err());
    accepted();
    for extra in [
        "-Coverflow-checks=off",
        "-Coverflow-checks=on",
        "-C",
        "-Zmir-opt-level=1",
        "-Zinline-mir=yes",
        "--cfg=fe2o3_canonical_assertion_private",
        "--cfg=fe2o3_canonical_scalar_noop",
    ] {
        let mut args = invocation.captured.clone();
        args.push(extra.into());
        assert!(history_executed(&args, request.case).is_err());
        accepted();
    }
    let mut changed = invocation.clone();
    changed.executed.push("-Cdebuginfo=0".into());
    let mut r = request.clone();
    r.executed = args_hash(&changed.executed);
    assert!(history_check_request(&r, &changed).is_err());
    accepted();
    for mutation in 0..45 {
        let mut v = serde_json::to_value(report).unwrap();
        let row = &mut v["result"]["Ok"];
        match mutation {
            0 => v["request"]["run_id"] = serde_json::json!(format!("{}-stale", request.run_id)),
            1 => v["callbacks"] = serde_json::json!(0),
            2 => v["callbacks"] = serde_json::json!(2),
            3 => v["request"]["route"] = serde_json::json!("original-N-B"),
            4 => v["request"]["case"] = serde_json::json!("UnitLocalPrivateAliasesOpt0"),
            5 => v["unexpected"] = serde_json::json!(true),
            6 => v["result"] = serde_json::json!({"Err":"actual source refused"}),
            7 => row["actual_target"] = serde_json::to_value(other).unwrap(),
            8 => {
                row["roots"].as_array_mut().unwrap().pop();
            }
            9 => row["after"]["bytes"] = serde_json::json!(0),
            10 => row["policy_subject"]["bytes"] = serde_json::json!(0),
            11 => {
                row["rounds"].as_array_mut().unwrap().pop();
            }
            12 => {
                let r = row["rounds"][0].clone();
                row["rounds"].as_array_mut().unwrap().push(r);
            }
            13 => row["rounds"][0]["ordinal"] = serde_json::json!(65535),
            14 => row["rounds"][0]["input"]["bytes"] = serde_json::json!(0),
            15 => row["rounds"][0]["integer_passes"][0] = serde_json::json!(6),
            16 => row["rounds"][0]["scalar_passes"][0] = serde_json::json!(1),
            17 => {
                let r = row["rounds"].as_array_mut().unwrap().last_mut().unwrap();
                r["changed"] = serde_json::json!(true);
            }
            18 => {
                row["rows"]["assertions"].as_array_mut().unwrap().pop();
            }
            19 => {
                let a = row["rows"]["assertions"][0].clone();
                row["rows"]["assertions"].as_array_mut().unwrap().push(a);
            }
            20 => {
                row["rows"]["assertions"][0]["original"]["expected"] = serde_json::json!(
                    !row["rows"]["assertions"][0]["original"]["expected"]
                        .as_bool()
                        .unwrap()
                )
            }
            21 => row["rows"]["assertions"][0]["original"]["proof"] = serde_json::json!("Other"),
            22 => {
                row["rows"]["assertions"][0]["original"]["success"][2] = serde_json::json!(u32::MAX)
            }
            23 => row["rows"]["assertions"][0]["success"] = serde_json::json!("Omitted"),
            24 => row["rows"]["assertions"][0]["removal"] = serde_json::json!([0, 0]),
            25 => {
                row["rows"]["definitions"].as_array_mut().unwrap().pop();
            }
            26 => row["rows"]["definitions"][0]["stages"][0] = serde_json::json!(8),
            27 => row["rows"]["definitions"][0]["history_function"] = serde_json::json!(u32::MAX),
            28 => row["rows"]["definitions"][0]["paired"] = serde_json::json!(0),
            29 => row["rows"]["definitions"][0]["panicked"] = serde_json::json!(true),
            30 => row["rows"]["definitions"][0]["floor"][0] = serde_json::json!(1),
            31 => row["restored_floor"] = serde_json::json!(0),
            32 => row["final_source_storage"] = serde_json::json!(1),
            33 => row["pending"] = serde_json::json!(0),
            34 => row["authority"] = serde_json::json!(true),
            35 => row["complete"] = serde_json::json!(true),
            36 => {
                row["rows"]["assertions"][0]["original"]["origin"]["file"][0] = serde_json::json!(
                    row["rows"]["assertions"][0]["original"]["origin"]["file"][0]
                        .as_u64()
                        .unwrap()
                        ^ 1
                )
            }
            37 => row["sizes"]["edges"] = serde_json::json!(usize::MAX),
            38 => {
                row["rows"]["spans"].as_array_mut().unwrap().pop();
            }
            39 => row["original"]["subject"]["bytes"] = serde_json::json!(0),
            40 => row["final_graph"]["subject"]["bytes"] = serde_json::json!(0),
            41 => row["rows"]["definitions"][0]["clean"] = serde_json::json!(false),
            42 => row["rows"]["definitions"][0]["denied"] = serde_json::json!(true),
            43 => row["rows"]["definitions"][0]["invocation"][0] = serde_json::json!(0),
            _ => row["policy_resources"][0] = serde_json::json!(0),
        }
        assert!(
            history_decode(Some(0), Some(&serde_json::to_vec(&v).unwrap()), request).is_err(),
            "accepted history report mutation {mutation}"
        );
        accepted();
    }
    let actual = report.result.as_ref().unwrap();
    for mutation in 0..4 {
        let mut v = serde_json::to_value(report).unwrap();
        let a = &mut v["result"]["Ok"]["rows"]["assertions"][0];
        match mutation {
            0 => {
                a["condition"] = if actual.rows.assertions[0].condition.is_some() {
                    serde_json::json!([u32::MAX, 0, 0])
                } else {
                    serde_json::json!([0, 0, 0])
                }
            }
            1 => {
                a["definition"] = if actual.rows.assertions[0].definition.is_some() {
                    serde_json::json!([0, u32::MAX, 0, 0, 0])
                } else {
                    serde_json::json!([0, 0, 0, 0, 0])
                }
            }
            2 => {
                a["failure"] = if actual.rows.assertions[0].failure.is_some() {
                    serde_json::json!([u32::MAX, 0, 0])
                } else {
                    serde_json::json!([0, 0, 0])
                }
            }
            _ => a["selection"] = serde_json::json!([u16::MAX, 1]),
        }
        assert!(history_decode(Some(0), Some(&serde_json::to_vec(&v).unwrap()), request).is_err());
        accepted();
    }
    if !actual.rows.final_payloads.is_empty() {
        let mut v = serde_json::to_value(report).unwrap();
        v["result"]["Ok"]["rows"]["final_payloads"][0] =
            serde_json::json!(actual.rows.final_payloads[0] ^ 1);
        assert!(history_decode(Some(0), Some(&serde_json::to_vec(&v).unwrap()), request).is_err());
        accepted();
    }
    let mut conditional = [0usize; 3];
    if !actual.rows.edges.is_empty() {
        for field in ["origin", "target"] {
            let mut v = serde_json::to_value(report).unwrap();
            let row = &mut v["result"]["Ok"];
            if field == "origin" {
                row["rows"]["edges"][0][1] = serde_json::json!([u32::MAX, 0, 0]);
            } else {
                row["rows"]["final_edges"][0]["target"] = serde_json::json!([u32::MAX, 0]);
            }
            assert!(
                history_decode(Some(0), Some(&serde_json::to_vec(&v).unwrap()), request).is_err()
            );
            accepted();
            conditional[0] += 1;
        }
    }
    if !actual.rows.arguments.is_empty() {
        let mut v = serde_json::to_value(report).unwrap();
        v["result"]["Ok"]["rows"]["arguments"][0][0][3] = serde_json::json!(u32::MAX);
        assert!(history_decode(Some(0), Some(&serde_json::to_vec(&v).unwrap()), request).is_err());
        accepted();
        conditional[1] += 1;
    }
    if !actual.rows.incoming.is_empty() {
        let mut v = serde_json::to_value(report).unwrap();
        v["result"]["Ok"]["rows"]["incoming"][0]["expected"] =
            serde_json::json!(!actual.rows.incoming[0].expected);
        assert!(history_decode(Some(0), Some(&serde_json::to_vec(&v).unwrap()), request).is_err());
        accepted();
        conditional[2] += 1;
    }
    for selected_substage in [false, true] {
        let mut v = serde_json::to_value(report).unwrap();
        let row = &mut v["result"]["Ok"]["rows"];
        if selected_substage {
            row["assertions"][0]["selection"] = serde_json::json!([0, 1]);
        } else {
            row["original"][0]["source"][2] = serde_json::json!(u32::MAX);
            row["assertions"][0]["original"]["source"][2] = serde_json::json!(u32::MAX);
        }
        assert!(history_decode(Some(0), Some(&serde_json::to_vec(&v).unwrap()), request).is_err());
        accepted();
    }
    println!(
        "CANONICAL_ASSERTION_HISTORY_PROTOCOL {:?} edge={} payload_ordinal={} retained_selector={} payload_value={}",
        request.case,
        conditional[0],
        conditional[1],
        conditional[2],
        usize::from(!actual.rows.final_payloads.is_empty()),
    );
    {
        let mut v = serde_json::to_value(report).unwrap();
        v["result"]["Ok"]["owner_floors"][1] = serde_json::json!(usize::MAX);
        let wire = serde_json::to_vec(&v).unwrap();
        assert!(serde_json::from_slice::<HistoryReport>(&wire).is_ok());
        assert_eq!(
            history_decode(Some(0), Some(&wire), request).unwrap_err(),
            "history owner floor overflow"
        );
    }
    accepted();
    for status in [None, Some(1), Some(101)] {
        assert!(history_decode(status, Some(&baseline), request).is_err());
        accepted();
    }
    assert!(history_decode(Some(0), None, request).is_err());
    accepted();
    assert!(history_decode(Some(0), Some(b"{"), request).is_err());
    accepted();
    let mut oversized = baseline.clone();
    oversized.resize(REPORT_CAP + 1, b' ');
    assert!(serde_json::from_slice::<HistoryReport>(&oversized).is_ok());
    assert!(history_decode(Some(0), Some(&oversized), request).is_err());
    accepted();
}
