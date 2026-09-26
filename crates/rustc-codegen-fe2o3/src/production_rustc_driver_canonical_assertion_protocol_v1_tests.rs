// Strict wire data is diagnostic, never a source or graph proof capability.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct AssertionRequest {
    schema: u16,
    run_id: String,
    case: AssertionCase,
    target: Target,
    captured: [u8; 32],
    executed: [u8; 32],
    cwd: PathBuf,
    source: [FileStamp; 4],
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Invocation {
    captured: Vec<String>,
    executed: Vec<String>,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Subject {
    digest: [u8; 32],
    bytes: usize,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Root {
    name: String,
    function: [u8; 32],
    body: [u8; 32],
    entry: String,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Origin {
    file: [u8; 32],
    bytes: [u64; 2],
    start: [u32; 2],
    end: [u32; 2],
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct EdgeRow {
    coordinate: [u32; 3],
    target: [u32; 2],
    target_id: u32,
    payload: [usize; 2],
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
enum ProofRow {
    ExactRange,
    CheckedArithmetic,
    LiteralShift,
    Other,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
enum MessageRow {
    Add,
    Multiply,
    Shift,
    Bounds,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct AssertionRow {
    span: usize,
    association: usize,
    source: [u32; 3],
    helper: bool,
    origin: Origin,
    expansion: Origin,
    expected: bool,
    message: MessageRow,
    signed_literal: bool,
    semantic_success: u32,
    condition: Option<[u32; 3]>,
    definition: Option<[u32; 5]>,
    success: [u32; 3],
    failure: Option<[u32; 3]>,
    proof: ProofRow,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
enum CallKind {
    Root,
    EmptyOnly,
    DeterministicEmpty,
    PrivateFrame,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
enum DecisionRow {
    Rejected,
    EmptyOnly,
    DeterministicEmpty,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct CallRow {
    association: usize,
    source: [u32; 2],
    function: u32,
    helper: bool,
    kind: CallKind,
    decision: DecisionRow,
    frame: [usize; 4],
    physical_calls: usize,
    abi: [usize; 3],
    source_unit: bool,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct PairRow {
    call: [u32; 3],
    declaration: u32,
    incoming: [usize; 2],
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct IncomingRow {
    condition: [u32; 3],
    definition: [u32; 5],
    success: [u32; 3],
    failure: [u32; 3],
    expected: bool,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct DefinitionRow {
    ordinal: usize,
    function: u32,
    history_function: usize,
    stages: [u8; 9],
    paired: usize,
    clean: bool,
    floor: [usize; 3],
    invocation: [usize; 3],
    denied: bool,
    panicked: bool,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Rows {
    original: Vec<AssertionRow>,
    assertions: Vec<AssertionRow>,
    edges: Vec<EdgeRow>,
    payloads: Vec<u32>,
    calls: Vec<CallRow>,
    pairs: Vec<PairRow>,
    incoming: Vec<IncomingRow>,
    definitions: Vec<DefinitionRow>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Observation {
    actual_target: Target,
    original: Subject,
    after: Subject,
    policy_subject: Subject,
    roots: Vec<Root>,
    fixture: Origin,
    original_edges: Vec<EdgeRow>,
    original_payloads: Vec<u32>,
    physical_definitions: Vec<u32>,
    rows: Rows,
    source_assertions: usize,
    physical_assertions: usize,
    associations: usize,
    call_transport: [usize; 4],
    spans: usize,
    zero_spans: usize,
    semantic_switches: usize,
    semantic_masks: usize,
    semantic_shifts: usize,
    graph_switches: usize,
    graph_masks: usize,
    graph_shifts: usize,
    graph_memory: [usize; 3],
    memory: [usize; 2],
    policy_resources: [usize; 3],
    source_resources: [usize; 3],
    resource_denial: bool,
    private_policy: bool,
    pending: usize,
    complete: bool,
    authority: bool,
    source_floor: usize,
    diagnostics_floor: usize,
    restored_floor: usize,
    // Restored source/structured domain only; serialized protocol backing stays live.
    final_storage: usize,
    protocol_storage: usize,
    work: usize,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Report {
    request: AssertionRequest,
    callbacks: usize,
    result: Result<Observation, String>,
}

fn require(ok: bool, reason: &str) -> Result<(), String> {
    if ok { Ok(()) } else { Err(reason.into()) }
}
fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .canonicalize()
        .unwrap()
}
fn stamps() -> Result<[FileStamp; 4], String> {
    let base = workspace();
    [
        "Cargo.lock".into(),
        format!("{BASE}/Cargo.toml"),
        format!("{BASE}/src/lib.rs"),
        format!("{BASE}/src/canonical_assertion_source.rs"),
    ]
    .into_iter()
    .map(|path: String| {
        let path = base.join(path).canonicalize().map_err(|e| e.to_string())?;
        let metadata = std::fs::metadata(&path).map_err(|e| e.to_string())?;
        require(
            metadata.is_file() && metadata.len() <= INPUT_CAP as u64,
            "bounded regular source",
        )?;
        let sha256 = digest(&std::fs::read(&path).map_err(|e| e.to_string())?);
        Ok(FileStamp { path, sha256 })
    })
    .collect::<Result<Vec<_>, String>>()?
    .try_into()
    .map_err(|_| "source roster".into())
}
fn args_hash(args: &[String]) -> [u8; 32] {
    digest(&serde_json::to_vec(args).unwrap())
}
fn executed(captured: &[String], case: AssertionCase) -> Result<Vec<String>, String> {
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
    for cfg in CASE_CFGS {
        result.push(format!("--check-cfg=cfg({cfg})"));
    }
    result.push(format!("--cfg={}", CASE_CFGS[case.ordinal()]));
    result.push("-Zmir-opt-level=0".into());
    result.push("-Zinline-mir=no".into());
    Ok(result)
}
fn options<'a>(args: &'a [String], prefix: &str) -> Vec<&'a str> {
    args.iter()
        .enumerate()
        .filter_map(|(n, arg)| {
            if arg == prefix {
                args.get(n + 1).map(String::as_str)
            } else {
                arg.strip_prefix(&format!("{prefix}="))
            }
        })
        .collect()
}
fn check_request(request: &AssertionRequest, invocation: &Invocation) -> Result<(), String> {
    require(
        request.schema == 1
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
            && invocation.executed == executed(&invocation.captured, request.case)?,
        "exact source/cwd/argv request binding",
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
        "captured target/crate/source entry",
    )
}
fn read_json<T: serde::de::DeserializeOwned>(path: &Path, cap: usize) -> Result<T, String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(cap as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    require(bytes.len() <= cap, "bounded JSON exceeded")?;
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}
fn write_json(path: &Path, value: &impl Serialize, cap: usize) -> Result<(), String> {
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    serde_json::to_writer(
        CappedFile {
            file,
            remaining: cap,
        },
        value,
    )
    .map_err(|e| e.to_string())
}
struct CappedFile {
    file: std::fs::File,
    remaining: usize,
}
impl Write for CappedFile {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > self.remaining {
            return Err(std::io::Error::other("bounded JSON exceeded"));
        }
        let written = self.file.write(bytes)?;
        self.remaining -= written;
        Ok(written)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.file.flush()
    }
}

fn origin_valid(origin: &Origin, fixture: &Origin) -> bool {
    origin.file == fixture.file
        && origin.bytes[0] < origin.bytes[1]
        && origin.bytes[1] <= fixture.bytes[1]
        && origin.start[0] > 0
        && origin.start <= origin.end
        && origin.start >= fixture.start
        && origin.end <= fixture.end
}
fn edge<'a>(row: &'a Observation, coordinate: [u32; 3]) -> Result<&'a EdgeRow, String> {
    let mut found = row
        .rows
        .edges
        .iter()
        .filter(|edge| edge.coordinate == coordinate);
    let edge = found.next().ok_or("missing original edge occurrence")?;
    require(found.next().is_none(), "duplicate edge occurrence")?;
    Ok(edge)
}
fn validate(request: &AssertionRequest, row: &Observation) -> Result<(), String> {
    let mut names = row
        .roots
        .iter()
        .map(|r| r.name.as_str())
        .collect::<Vec<_>>();
    names.sort_unstable();
    let mut expected = request.case.roots().to_vec();
    expected.sort_unstable();
    require(
        row.actual_target == request.target
            && names == expected
            && row
                .roots
                .iter()
                .all(|r| r.function != [0; 32] && r.body != [0; 32] && !r.entry.is_empty())
            && row.original == row.after
            && row.original == row.policy_subject
            && row.original.digest != [0; 32]
            && row.original.bytes > 0,
        "actual original source/graph/profile roster",
    )?;
    require(
        row.fixture.file != [0; 32]
            && row.fixture.bytes[0] == 0
            && row.fixture.bytes[1]
                == std::fs::metadata(&request.source[3].path)
                    .map_err(|e| e.to_string())?
                    .len()
            && row.original_edges == row.rows.edges
            && row.original_payloads == row.rows.payloads,
        "actual source file and ordered graph payloads",
    )?;
    let mut end = 0usize;
    for edge in &row.rows.edges {
        require(
            edge.payload[0] == end
                && edge.payload[1] <= row.rows.payloads.len()
                && edge.payload[0] <= edge.payload[1],
            "complete ordered edge payload extent",
        )?;
        end = edge.payload[1];
    }
    require(end == row.rows.payloads.len(), "unclaimed edge payload")?;
    require(
        row.source_assertions == row.rows.assertions.len()
            && row.source_assertions > 0
            && row.rows.original.len() == row.source_assertions
            && row.associations == row.rows.calls.len()
            && row.spans > 0
            && row.zero_spans > 0
            && row.pending == 19
            && !row.complete
            && !row.authority
            && row.source_floor > 0
            && row.diagnostics_floor > 0
            && row.source_floor.checked_add(row.diagnostics_floor) == Some(row.restored_floor)
            && row.final_storage == 0
            && row.protocol_storage > 0
            && !row.resource_denial
            && row.source_resources[0] <= row.work
            && row.source_resources[1] > row.restored_floor
            && row.source_resources[2] >= row.source_resources[1]
            && row.work > 0,
        "complete source aliases and original paid floor",
    )?;
    let mut aliases = std::collections::BTreeSet::new();
    let mut physical = std::collections::BTreeSet::new();
    for (original, proved) in row.rows.original.iter().zip(&row.rows.assertions) {
        let mut expected = *original;
        expected.proof = proved.proof;
        require(
            expected == *proved
                && proved.proof != ProofRow::Other
                && aliases.insert(proved.source)
                && proved.association < row.associations
                && origin_valid(&proved.origin, &row.fixture)
                && origin_valid(&proved.expansion, &row.fixture),
            "exact source alias/fresh proof join",
        )?;
        physical.insert(proved.success);
        edge(row, proved.success)?;
        if let Some(failure) = proved.failure {
            edge(row, failure)?;
            require(
                proved.success[..2] == failure[..2]
                    && proved.success[2] == u32::from(!proved.expected)
                    && failure[2] == u32::from(proved.expected)
                    && proved.condition == Some([proved.success[0], proved.success[1], 0])
                    && proved.definition.is_some(),
                "retained original polarity and condition",
            )?;
            require(
                row.rows
                    .incoming
                    .iter()
                    .filter(|incoming| {
                        incoming.condition == proved.condition.unwrap()
                            && Some(incoming.definition) == proved.definition
                            && incoming.success == proved.success
                            && incoming.failure == failure
                            && incoming.expected == proved.expected
                    })
                    .count()
                    == 1,
                "complete retained alias to physical incoming join",
            )?;
        } else {
            require(
                proved.success[2] == 0
                    && proved.condition.is_none()
                    && proved.definition.is_none()
                    && !row
                        .rows
                        .incoming
                        .iter()
                        .any(|incoming| incoming.success == proved.success),
                "genuine source elision has no invented condition or trap",
            )?;
        }
    }
    require(
        physical.len() == row.physical_assertions,
        "complete physical assertion roster",
    )?;
    let mut incoming_end = 0;
    for pair in &row.rows.pairs {
        require(
            pair.incoming[0] == incoming_end
                && pair.incoming[0] < pair.incoming[1]
                && pair.incoming[1] <= row.rows.incoming.len(),
            "complete ordered trap pair incoming extent",
        )?;
        for incoming in &row.rows.incoming[pair.incoming[0]..pair.incoming[1]] {
            require(
                edge(row, incoming.failure)?.target == [pair.call[0], pair.call[1]]
                    && row.rows.assertions.iter().any(|a| {
                        a.failure == Some(incoming.failure)
                            && a.success == incoming.success
                            && a.expected == incoming.expected
                            && a.condition == Some(incoming.condition)
                            && a.definition == Some(incoming.definition)
                    }),
                "bidirectional physical trap coverage",
            )?;
        }
        incoming_end = pair.incoming[1];
    }
    require(
        incoming_end == row.rows.incoming.len(),
        "unclaimed trap incoming edge",
    )?;
    require(
        row.rows.definitions.len() == row.physical_definitions.len()
            && !row.rows.definitions.is_empty(),
        "all real definitions checked",
    )?;
    let mut floor = [0usize; 3];
    for (ordinal, definition) in row.rows.definitions.iter().enumerate() {
        require(
            definition.ordinal == ordinal
                && definition.function == row.physical_definitions[ordinal]
                && definition.history_function == definition.function as usize
                && definition.stages == [0, 1, 2, 3, 4, 5, 6, 7, 8]
                && definition.paired == 9
                && definition.clean
                && !definition.denied
                && !definition.panicked
                && definition.floor == floor
                && definition.invocation[0] > 0
                && definition.invocation[2] >= definition.invocation[1],
            "actual ordered fixed-nine definition history",
        )?;
        floor = [
            floor[0]
                .checked_add(definition.invocation[0])
                .ok_or("history work overflow")?,
            floor[1]
                .checked_add(definition.invocation[1])
                .ok_or("history storage overflow")?,
            floor[2].max(
                floor[1]
                    .checked_add(definition.invocation[2])
                    .ok_or("history peak overflow")?,
            ),
        ];
    }
    require(
        floor == row.policy_resources,
        "complete aggregate real C history",
    )?;
    for (ordinal, call) in row.rows.calls.iter().enumerate() {
        require(
            call.association == ordinal
                && row.physical_definitions.contains(&call.function)
                && (!call.helper && call.kind == CallKind::Root
                    || call.helper && call.kind != CallKind::Root),
            "complete source callable association",
        )?;
        match call.kind {
            CallKind::PrivateFrame => require(
                row.private_policy
                    && call.decision == DecisionRow::Rejected
                    && call.frame[0] > 0
                    && call.frame[1] > 0,
                "private frame is not empty/deterministic",
            )?,
            CallKind::DeterministicEmpty => require(
                call.decision == DecisionRow::DeterministicEmpty && call.frame == [0; 4],
                "actual source deterministic empty summary",
            )?,
            CallKind::EmptyOnly => require(
                call.decision == DecisionRow::EmptyOnly && call.frame == [0; 4],
                "empty-only is not deterministic",
            )?,
            CallKind::Root => {}
        }
    }
    let assertions = &row.rows.assertions;
    match request.case {
        AssertionCase::RetainedArithmeticOpt0 => {
            require(
                assertions.len() == 2
                    && assertions.iter().all(|a| {
                        !a.helper
                            && !a.expected
                            && a.failure.is_some()
                            && a.proof == ProofRow::CheckedArithmetic
                    })
                    && assertions.iter().any(|a| a.message == MessageRow::Add)
                    && assertions.iter().any(|a| a.message == MessageRow::Multiply)
                    && row.rows.pairs.len() == 1
                    && row.rows.incoming.len() == 2,
                "two genuine arithmetic assertions share the original sink",
            )?;
        }
        AssertionCase::SignedLiteralShiftOpt0 => require(
            assertions.len() == 1
                && !assertions[0].helper
                && assertions[0].expected
                && assertions[0].signed_literal
                && assertions[0].message == MessageRow::Shift
                && assertions[0].failure.is_some()
                && assertions[0].proof == ProofRow::LiteralShift
                && row.semantic_shifts > 0
                && row.graph_shifts > 0,
            "genuine signed-literal rule and retained shift",
        )?,
        AssertionCase::MaskedShiftElidedOpt0 => require(
            assertions.len() == 1
                && assertions[0].expected
                && assertions[0].message == MessageRow::Shift
                && assertions[0].failure.is_none()
                && assertions[0].proof == ProofRow::ExactRange
                && row.rows.pairs.is_empty()
                && row.rows.incoming.is_empty()
                && row.semantic_switches > 0
                && row.semantic_masks > 0
                && row.semantic_shifts > 0
                && row.graph_switches > 0
                && row.graph_masks > 0
                && row.graph_shifts > 0,
            "real masked source elision retains integer switch and downstream shift",
        )?,
        AssertionCase::SharedScalarAliasesOpt0 => {
            require(
                assertions.len() == 2
                    && assertions.iter().all(|a| {
                        a.helper
                            && a.proof == ProofRow::LiteralShift
                            && a.failure.is_some()
                            && a.expected
                            && a.signed_literal
                            && a.message == MessageRow::Shift
                    })
                    && assertions[0].source[0] != assertions[1].source[0]
                    && assertions[0].source[1..] == assertions[1].source[1..]
                    && assertions[0].success == assertions[1].success
                    && row.physical_assertions == 1
                    && row.rows.incoming.len() == 1,
                "two source aliases of one actual scalar helper assertion",
            )?;
            require(
                row.rows
                    .calls
                    .iter()
                    .filter(|c| {
                        c.helper
                            && c.source[1] == assertions[0].source[1]
                            && c.kind == CallKind::DeterministicEmpty
                            && c.physical_calls == 2
                            && c.abi == [1, 1, 1]
                            && !c.source_unit
                    })
                    .count()
                    == 2,
                "two independently classified deterministic helper associations",
            )?;
        }
        AssertionCase::UnitLocalPrivateAliasesOpt0 => {
            let helpers = assertions.iter().filter(|a| a.helper).collect::<Vec<_>>();
            require(
                !helpers.is_empty()
                    && helpers.len() % 2 == 0
                    && helpers.iter().all(|a| {
                        a.message == MessageRow::Bounds && a.expected && a.failure.is_some()
                    })
                    && assertions
                        .iter()
                        .filter(|a| {
                            !a.helper
                                && a.proof == ProofRow::CheckedArithmetic
                                && a.message == MessageRow::Add
                                && a.failure.is_some()
                        })
                        .count()
                        == 2
                    && row.private_policy
                    && row.memory[0] > 0
                    && row.memory[1] > 0
                    && row.graph_memory.iter().all(|count| *count > 0),
                "genuine private memory and retained bounds assertion aliases",
            )?;
            for a in &helpers {
                require(
                    helpers
                        .iter()
                        .filter(|b| a.source[1..] == b.source[1..] && a.success == b.success)
                        .count()
                        == 2,
                    "complete private helper assertion aliases",
                )?;
            }
            require(
                row.rows
                    .calls
                    .iter()
                    .filter(|c| {
                        c.kind == CallKind::PrivateFrame
                            && c.physical_calls == 2
                            && c.abi == [0, 0, 0]
                            && c.source_unit
                    })
                    .count()
                    == 2,
                "two real private helper frames",
            )?;
        }
    }
    let call_transport_total = row.call_transport[1]
        .checked_add(row.call_transport[2])
        .ok_or("call/return transport count overflow")?;
    require(
        row.call_transport[0] == call_transport_total && row.call_transport[2] > 0,
        "complete actual source call/return transport",
    )?;
    if matches!(
        request.case,
        AssertionCase::SharedScalarAliasesOpt0 | AssertionCase::UnitLocalPrivateAliasesOpt0
    ) {
        require(row.call_transport[1] >= 2, "genuine original helper calls")?;
    }
    if request.case != AssertionCase::UnitLocalPrivateAliasesOpt0 {
        require(
            !row.private_policy && row.memory == [0; 2] && row.graph_memory == [0; 3],
            "scalar case cannot acquire private effects",
        )?;
    }
    Ok(())
}
fn decode(
    status: Option<i32>,
    bytes: Option<&[u8]>,
    expected: &AssertionRequest,
) -> Result<Observation, String> {
    require(status == Some(0), "exact zero child exit required")?;
    let bytes = bytes.ok_or("missing fresh report")?;
    require(bytes.len() <= REPORT_CAP, "oversized report")?;
    let report: Report = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    require(
        report.request == *expected && report.callbacks == 1,
        "stale/foreign request or callback count",
    )?;
    let row = report.result?;
    validate(expected, &row)?;
    Ok(row)
}

fn audit_successful_protocol(request: &AssertionRequest, invocation: &Invocation, report: &Report) {
    check_request(request, invocation).unwrap();
    assert!(report.result.is_ok());
    let baseline = serde_json::to_vec(report).unwrap();
    decode(Some(0), Some(&baseline), request).unwrap();
    let accepted_again = || {
        check_request(request, invocation).unwrap();
        decode(Some(0), Some(&baseline), request).unwrap();
    };
    for change in 0..9 {
        let mut changed = request.clone();
        match change {
            0 => changed.source[3].sha256[0] ^= 1,
            1 => changed.schema += 1,
            2 => changed.run_id.clear(),
            3 => changed.cwd.push("foreign-cwd"),
            4 => changed.captured[0] ^= 1,
            5 => changed.executed[0] ^= 1,
            6 => changed.source[2] = changed.source[3].clone(),
            7 => {
                changed.target = match request.target {
                    Target::Gfx942 => Target::Gfx950,
                    Target::Gfx950 => Target::Gfx942,
                }
            }
            _ => {
                changed.case = if request.case == AssertionCase::MaskedShiftElidedOpt0 {
                    AssertionCase::SignedLiteralShiftOpt0
                } else {
                    AssertionCase::MaskedShiftElidedOpt0
                }
            }
        }
        assert!(
            check_request(&changed, invocation).is_err(),
            "request mutation {change}"
        );
        accepted_again();
    }
    let other = match request.target {
        Target::Gfx942 => Target::Gfx950,
        Target::Gfx950 => Target::Gfx942,
    };
    for (option, value) in [
        ("--crate-name", "foreign_fixture"),
        ("-Ctarget-cpu", other.cpu()),
    ] {
        let mut captured = invocation.captured.clone();
        let n = captured
            .iter()
            .position(|arg| arg == option || arg.starts_with(&format!("{option}=")))
            .unwrap();
        if captured[n] == option {
            captured[n + 1] = value.into();
        } else {
            captured[n] = format!("{option}={value}");
        }
        let changed = Invocation {
            executed: executed(&captured, request.case).unwrap(),
            captured,
        };
        let mut changed_request = request.clone();
        changed_request.captured = args_hash(&changed.captured);
        changed_request.executed = args_hash(&changed.executed);
        assert!(check_request(&changed_request, &changed).is_err());
        accepted_again();
    }
    let mut changed = invocation.clone();
    let n = changed
        .captured
        .iter()
        .position(|arg| {
            request.cwd.join(arg).canonicalize().ok() == Some(request.source[2].path.clone())
        })
        .unwrap();
    changed.captured[n] = request.source[3].path.to_str().unwrap().into();
    changed.executed = executed(&changed.captured, request.case).unwrap();
    let mut changed_request = request.clone();
    changed_request.captured = args_hash(&changed.captured);
    changed_request.executed = args_hash(&changed.executed);
    assert!(check_request(&changed_request, &changed).is_err());
    accepted_again();
    for extra in [
        "-Coverflow-checks=off",
        "-Coverflow-checks=on",
        "-C",
        "-Zmir-opt-level=1",
        "-Zinline-mir=yes",
        "--cfg=fe2o3_canonical_assertion_private",
        "--cfg=fe2o3_canonical_scalar_noop",
    ] {
        let mut captured = invocation.captured.clone();
        captured.push(extra.into());
        assert!(
            executed(&captured, request.case).is_err(),
            "preexisting override {extra}"
        );
        accepted_again();
    }
    let mut changed = invocation.clone();
    changed.executed.push("-Cdebuginfo=0".into());
    let mut changed_request = request.clone();
    changed_request.executed = args_hash(&changed.executed);
    assert!(check_request(&changed_request, &changed).is_err());
    accepted_again();
    for change in 0..29 {
        let mut changed = serde_json::to_value(report).unwrap();
        let row = &mut changed["result"]["Ok"];
        match change {
            0 => {
                changed["request"]["run_id"] =
                    serde_json::json!(format!("{}-stale", request.run_id))
            }
            1 => changed["callbacks"] = serde_json::json!(0),
            2 => changed["callbacks"] = serde_json::json!(2),
            3 => row["actual_target"] = serde_json::to_value(other).unwrap(),
            4 => changed["target_bound_stage"] = serde_json::json!(true),
            5 => changed["result"] = serde_json::json!({"Err": "actual source refusal"}),
            6 => {
                row["roots"].as_array_mut().unwrap().pop();
            }
            7 => {
                let root = row["roots"][0].clone();
                row["roots"].as_array_mut().unwrap().push(root);
            }
            8 => {
                row["after"]["digest"][0] =
                    serde_json::json!(row["after"]["digest"][0].as_u64().unwrap() ^ 1)
            }
            9 => row["policy_subject"]["bytes"] = serde_json::json!(0),
            10 => row["pending"] = serde_json::json!(0),
            11 => row["authority"] = serde_json::json!(true),
            12 => row["complete"] = serde_json::json!(true),
            13 => row["final_storage"] = serde_json::json!(1),
            14 => row["restored_floor"] = serde_json::json!(0),
            15 => {
                row["rows"]["assertions"].as_array_mut().unwrap().pop();
            }
            16 => {
                let alias = row["rows"]["assertions"][0].clone();
                row["rows"]["assertions"]
                    .as_array_mut()
                    .unwrap()
                    .push(alias);
            }
            17 => {
                row["rows"]["assertions"][0]["expected"] =
                    serde_json::json!(!row["rows"]["assertions"][0]["expected"].as_bool().unwrap())
            }
            18 => {
                row["rows"]["assertions"][0]["success"][2] = serde_json::json!(
                    row["rows"]["assertions"][0]["success"][2].as_u64().unwrap() + 1
                )
            }
            19 => row["rows"]["assertions"][0]["proof"] = serde_json::json!("Other"),
            20 => {
                row["rows"]["assertions"][0]["failure"] =
                    if row["rows"]["assertions"][0]["failure"].is_null() {
                        serde_json::json!([0, 0, 1])
                    } else {
                        serde_json::Value::Null
                    }
            }
            21 => {
                row["rows"]["definitions"].as_array_mut().unwrap().pop();
            }
            22 => row["rows"]["definitions"][0]["stages"][0] = serde_json::json!(8),
            23 => row["rows"]["definitions"][0]["history_function"] = serde_json::json!(u32::MAX),
            24 => row["rows"]["definitions"][0]["denied"] = serde_json::json!(true),
            25 => {
                row["rows"]["edges"][0]["target"][1] =
                    serde_json::json!(row["rows"]["edges"][0]["target"][1].as_u64().unwrap() + 1)
            }
            26 => {
                row["rows"]["calls"].as_array_mut().unwrap().pop();
            }
            27 => {
                row["rows"]["assertions"][0]["origin"]["file"][0] = serde_json::json!(
                    row["rows"]["assertions"][0]["origin"]["file"][0]
                        .as_u64()
                        .unwrap()
                        ^ 1
                )
            }
            _ => row["unexpected_component"] = serde_json::json!(true),
        }
        assert!(
            decode(
                Some(0),
                Some(&serde_json::to_vec(&changed).unwrap()),
                request
            )
            .is_err(),
            "actual accepted report mutation {change}"
        );
        accepted_again();
    }
    let actual = report.result.as_ref().unwrap();
    {
        assert!(actual.call_transport[2] > 0);
        let mut changed = serde_json::to_value(report).unwrap();
        changed["result"]["Ok"]["call_transport"][1] = serde_json::json!(usize::MAX);
        let overflow = serde_json::to_vec(&changed).unwrap();
        assert!(serde_json::from_slice::<Report>(&overflow).is_ok());
        assert_eq!(
            decode(Some(0), Some(&overflow), request).unwrap_err(),
            "call/return transport count overflow",
        );
    }
    accepted_again();
    if !actual.rows.payloads.is_empty() {
        let mut changed = serde_json::to_value(report).unwrap();
        changed["result"]["Ok"]["rows"]["payloads"][0] =
            serde_json::json!(actual.rows.payloads[0] ^ 1);
        assert!(
            decode(
                Some(0),
                Some(&serde_json::to_vec(&changed).unwrap()),
                request
            )
            .is_err()
        );
        accepted_again();
    }
    if let Some(ordinal) = actual.rows.calls.iter().position(|call| call.helper) {
        let mut changed = serde_json::to_value(report).unwrap();
        changed["result"]["Ok"]["rows"]["calls"][ordinal]["kind"] =
            serde_json::json!(if actual.private_policy {
                "DeterministicEmpty"
            } else {
                "PrivateFrame"
            });
        assert!(
            decode(
                Some(0),
                Some(&serde_json::to_vec(&changed).unwrap()),
                request
            )
            .is_err()
        );
        accepted_again();
    }
    if !actual.rows.incoming.is_empty() {
        let mut changed = serde_json::to_value(report).unwrap();
        changed["result"]["Ok"]["rows"]["incoming"][0]["failure"][2] =
            serde_json::json!(actual.rows.incoming[0].failure[2] ^ 1);
        assert!(
            decode(
                Some(0),
                Some(&serde_json::to_vec(&changed).unwrap()),
                request
            )
            .is_err()
        );
        accepted_again();
    }
    for status in [None, Some(1), Some(101)] {
        assert!(decode(status, Some(&baseline), request).is_err());
        accepted_again();
    }
    assert!(decode(Some(0), None, request).is_err());
    assert!(decode(Some(0), Some(b"{"), request).is_err());
    let mut oversized = baseline.clone();
    oversized.resize(REPORT_CAP + 1, b' ');
    assert!(serde_json::from_slice::<Report>(&oversized).is_ok());
    assert!(decode(Some(0), Some(&oversized), request).is_err());
    accepted_again();
}
