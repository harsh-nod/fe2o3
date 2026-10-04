use std::{fmt::Write as _, fs, path::Path};

use fe2o3_mir_model::{
    SsaArgumentV1, SsaBlockIdV1, SsaBlockInputV1, SsaConstructionInputV1, SsaEdgeIdV1,
    SsaEdgeInputV1, SsaEdgeRoleV1, SsaEventV1, SsaPlannerLimitsV1, SsaResolvedEventV1, SsaValueV1,
    SsaVariableIdV1, plan_ssa_with_limits_v1,
};

const RUN_DIRECTIVE: &str = "// RUN: fe2o3-mir-ssa-lit %s";
const MAX_FIXTURES: usize = 128;
const MAX_FIXTURE_BYTES: u64 = 64 * 1024;

#[test]
fn textual_generic_ssa_lit_suite() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ssa-lit");
    let mut fixtures = fs::read_dir(&root)
        .expect("SSA lit fixture directory")
        .map(|entry| entry.expect("SSA lit directory entry").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "mir"))
        .collect::<Vec<_>>();
    fixtures.sort();
    assert!(
        fixtures.len() >= 18,
        "generic SSA lit suite unexpectedly shrank"
    );
    assert!(
        fixtures.len() <= MAX_FIXTURES,
        "generic SSA lit suite exceeds its fixture-count limit"
    );
    for fixture in fixtures {
        run_fixture(&fixture);
    }
}

fn run_fixture(path: &Path) {
    let metadata = fs::metadata(path).expect("SSA fixture metadata");
    assert!(metadata.is_file(), "{} is not a file", path.display());
    assert!(
        metadata.len() <= MAX_FIXTURE_BYTES,
        "{} exceeds the fixture byte limit",
        path.display()
    );
    let source = fs::read_to_string(path).expect("UTF-8 SSA fixture");
    assert_eq!(
        source.lines().filter(|line| *line == RUN_DIRECTIVE).count(),
        1,
        "{} must name the generic SSA planner exactly once",
        path.display()
    );
    let expects = source
        .lines()
        .filter_map(|line| line.strip_prefix("// EXPECT: "))
        .collect::<Vec<_>>();
    assert_eq!(expects.len(), 1, "{} has ambiguous EXPECT", path.display());
    assert!(
        matches!(expects[0], "PASS" | "REJECT"),
        "{} has an invalid EXPECT",
        path.display()
    );
    let checks = source
        .lines()
        .filter_map(|line| line.strip_prefix("// CHECK: "))
        .collect::<Vec<_>>();
    let check_nots = source
        .lines()
        .filter_map(|line| line.strip_prefix("// CHECK-NOT: "))
        .collect::<Vec<_>>();
    let check_errors = source
        .lines()
        .filter_map(|line| line.strip_prefix("// CHECK-ERROR: "))
        .collect::<Vec<_>>();
    assert!(
        !checks.is_empty() || !check_errors.is_empty(),
        "{} has no positive check",
        path.display()
    );
    if expects[0] == "PASS" {
        assert!(
            check_errors.is_empty(),
            "{} expects PASS but contains CHECK-ERROR",
            path.display()
        );
    } else {
        assert_eq!(
            check_errors.len(),
            1,
            "{} rejection must have one CHECK-ERROR",
            path.display()
        );
    }

    let parsed = parse_fixture(&source)
        .unwrap_or_else(|error| panic!("{} failed to parse: {error}", path.display()));
    let output = match plan_ssa_with_limits_v1(&parsed.input, parsed.limits) {
        Ok(plan) => {
            plan.verify_replay(&parsed.input, parsed.limits)
                .unwrap_or_else(|error| panic!("{} replay failed: {error}", path.display()));
            let repeated = plan_ssa_with_limits_v1(&parsed.input, parsed.limits)
                .expect("the same bounded input must plan twice");
            assert_eq!(plan, repeated, "{} is nondeterministic", path.display());
            render_plan(&parsed.input, &plan)
        }
        Err(error) => format!("REJECT\nerror {error}\n"),
    };
    assert_eq!(
        output.starts_with("REJECT\n"),
        expects[0] == "REJECT",
        "{}: {output}",
        path.display()
    );
    let mut cursor = 0;
    for check in checks {
        let offset = output[cursor..].find(check).unwrap_or_else(|| {
            panic!(
                "{} missing ordered CHECK `{check}` in:\n{output}",
                path.display()
            )
        });
        cursor += offset + check.len();
    }
    for check in check_nots {
        assert!(
            !output.contains(check),
            "{} matched forbidden CHECK-NOT `{check}` in:\n{output}",
            path.display()
        );
    }
    for check in check_errors {
        assert_eq!(
            output.lines().nth(1),
            Some(check),
            "{} did not produce the exact CHECK-ERROR in:\n{output}",
            path.display(),
        );
    }
}

struct ParsedFixture {
    input: SsaConstructionInputV1,
    limits: SsaPlannerLimitsV1,
}

#[derive(Default)]
struct BlockBuilder {
    events: Vec<SsaEventV1>,
    edges: Vec<SsaEdgeInputV1>,
}

fn parse_fixture(source: &str) -> Result<ParsedFixture, String> {
    let mut entry = None;
    let mut promotable = None;
    let mut entry_definitions = Vec::new();
    let mut blocks = Vec::<BlockBuilder>::new();
    let mut current_block = None;
    let mut limits = SsaPlannerLimitsV1::default();

    for (line_index, original) in source.lines().enumerate() {
        let line_number = line_index + 1;
        let line = original.trim();
        if line.is_empty() || line.starts_with("//") {
            continue;
        }
        let fields = line.split_ascii_whitespace().collect::<Vec<_>>();
        let fail = |reason: &str| format!("line {line_number}: {reason}: `{line}`");
        match fields.as_slice() {
            ["entry", block] if entry.is_none() => entry = Some(parse_block(block, line_number)?),
            ["variables", variables @ ..] if promotable.is_none() => {
                let mut bitmap = Vec::with_capacity(variables.len());
                for (expected, variable) in variables.iter().enumerate() {
                    let (is_promotable, variable) = match variable.strip_prefix('!') {
                        Some(variable) => (false, variable),
                        None => (true, *variable),
                    };
                    if parse_variable(variable, line_number)?.get() as usize != expected {
                        return Err(fail("variables must be the dense sequence v0..vN"));
                    }
                    bitmap.push(is_promotable);
                }
                promotable = Some(bitmap);
            }
            ["entry-def", variables @ ..] => {
                entry_definitions = parse_variables(variables, line_number)?;
            }
            ["limits", assignments @ ..] => {
                limits = parse_limits(assignments, line_number)?;
            }
            ["block", block] => {
                let block = parse_block(block, line_number)?;
                if block.get() as usize != blocks.len() {
                    return Err(fail("blocks must be declared in dense b0..bN order"));
                }
                blocks.push(BlockBuilder::default());
                current_block = Some(block.get() as usize);
            }
            ["use", variable] => {
                let block = current_block.ok_or_else(|| fail("use appears before a block"))?;
                blocks[block]
                    .events
                    .push(SsaEventV1::Use(parse_variable(variable, line_number)?));
            }
            ["def", variable] => {
                let block = current_block.ok_or_else(|| fail("def appears before a block"))?;
                blocks[block]
                    .events
                    .push(SsaEventV1::Define(parse_variable(variable, line_number)?));
            }
            ["kill", variable] => {
                let block = current_block.ok_or_else(|| fail("kill appears before a block"))?;
                blocks[block]
                    .events
                    .push(SsaEventV1::Kill(parse_variable(variable, line_number)?));
            }
            ["edge", role, target] => {
                let block = current_block.ok_or_else(|| fail("edge appears before a block"))?;
                blocks[block].edges.push(SsaEdgeInputV1::new(
                    parse_role(role, line_number)?,
                    parse_block(target, line_number)?,
                    vec![],
                ));
            }
            ["edge", role, target, "defines", definitions @ ..] if !definitions.is_empty() => {
                let block = current_block.ok_or_else(|| fail("edge appears before a block"))?;
                blocks[block].edges.push(SsaEdgeInputV1::new(
                    parse_role(role, line_number)?,
                    parse_block(target, line_number)?,
                    parse_variables(definitions, line_number)?,
                ));
            }
            _ => return Err(fail("unknown or malformed directive")),
        }
    }

    let promotable = promotable.ok_or_else(|| "missing variables directive".to_owned())?;
    let variable_count = u32::try_from(promotable.len())
        .map_err(|_| "variable count does not fit u32".to_owned())?;
    let entry = entry.ok_or_else(|| "missing entry directive".to_owned())?;
    let blocks = blocks
        .into_iter()
        .map(|block| SsaBlockInputV1::new(block.events, block.edges))
        .collect();
    Ok(ParsedFixture {
        input: SsaConstructionInputV1::new(
            entry,
            variable_count,
            promotable,
            entry_definitions,
            blocks,
        ),
        limits,
    })
}

fn parse_limits(assignments: &[&str], line: usize) -> Result<SsaPlannerLimitsV1, String> {
    let defaults = SsaPlannerLimitsV1::default();
    let mut values = [
        defaults.max_variables(),
        defaults.max_blocks(),
        defaults.max_edges(),
        defaults.max_events(),
        defaults.max_edge_definitions(),
        defaults.max_output_items(),
        defaults.max_storage_words(),
        defaults.max_work_units(),
    ];
    for assignment in assignments {
        let (name, value) = assignment
            .split_once('=')
            .ok_or_else(|| format!("line {line}: malformed limit `{assignment}`"))?;
        let value = value
            .parse::<usize>()
            .map_err(|_| format!("line {line}: invalid limit value `{value}`"))?;
        let slot = match name {
            "variables" => 0,
            "blocks" => 1,
            "edges" => 2,
            "events" => 3,
            "edge-definitions" => 4,
            "output" => 5,
            "storage" => 6,
            "work" => 7,
            _ => return Err(format!("line {line}: unknown limit `{name}`")),
        };
        values[slot] = value;
    }
    SsaPlannerLimitsV1::try_new(
        values[0], values[1], values[2], values[3], values[4], values[5], values[6], values[7],
    )
    .map_err(|error| format!("line {line}: {error}"))
}

fn parse_block(value: &str, line: usize) -> Result<SsaBlockIdV1, String> {
    parse_dense(value, 'b', line).map(SsaBlockIdV1::new)
}

fn parse_variable(value: &str, line: usize) -> Result<SsaVariableIdV1, String> {
    parse_dense(value, 'v', line).map(SsaVariableIdV1::new)
}

fn parse_variables(values: &[&str], line: usize) -> Result<Vec<SsaVariableIdV1>, String> {
    values
        .iter()
        .map(|value| parse_variable(value, line))
        .collect()
}

fn parse_role(value: &str, line: usize) -> Result<SsaEdgeRoleV1, String> {
    parse_dense(value, 'r', line)
        .and_then(|role| {
            u16::try_from(role).map_err(|_| format!("line {line}: edge role exceeds u16"))
        })
        .map(SsaEdgeRoleV1::new)
}

fn parse_dense(value: &str, prefix: char, line: usize) -> Result<u32, String> {
    value
        .strip_prefix(prefix)
        .ok_or_else(|| format!("line {line}: expected `{prefix}<number>`, found `{value}`"))?
        .parse::<u32>()
        .map_err(|_| format!("line {line}: invalid dense identity `{value}`"))
}

fn render_plan(
    input: &SsaConstructionInputV1,
    plan: &fe2o3_mir_model::SsaConstructionPlanV1,
) -> String {
    let mut output = String::from("PASS\n");
    writeln!(output, "identity {}", plan.identity()).unwrap();
    let resources = plan.resources();
    writeln!(
        output,
        "resources blocks={} reachable={} pruned={} edges={} events={} edge-definitions={} definitions={} output={} storage={} work={}",
        resources.input_blocks(),
        resources.reachable_blocks(),
        resources.pruned_blocks(),
        resources.input_edges(),
        resources.input_events(),
        resources.input_edge_definitions(),
        resources.generated_definitions(),
        resources.output_items(),
        resources.storage_words(),
        resources.work_units(),
    )
    .unwrap();
    writeln!(output, "rpo {}", render_blocks(plan.reverse_postorder())).unwrap();
    writeln!(
        output,
        "promoted {}",
        render_variables(plan.promoted_variables())
    )
    .unwrap();
    writeln!(
        output,
        "entry-definitions {}",
        render_arguments(plan.entry_definitions())
    )
    .unwrap();
    writeln!(
        output,
        "entry-arguments {}",
        render_arguments(plan.entry_arguments())
    )
    .unwrap();
    for (block_index, block) in input.blocks().iter().enumerate() {
        let block_id = SsaBlockIdV1::new(block_index as u32);
        if !plan.is_reachable(block_id) {
            writeln!(output, "block b{block_index} unreachable").unwrap();
            continue;
        }
        writeln!(
            output,
            "block b{block_index} live={} merge={} transport={}",
            render_variables(plan.live_in(block_id).unwrap()),
            render_variables(plan.merge_variables(block_id).unwrap()),
            render_variables(plan.transport_variables(block_id).unwrap()),
        )
        .unwrap();
        for (event_index, event) in block.events().iter().enumerate() {
            let resolved = match plan.resolved_event(block_id, event_index as u32) {
                Some(SsaResolvedEventV1::Use { variable, value }) => {
                    format!("use v{}={}", variable.get(), render_value(*value))
                }
                Some(SsaResolvedEventV1::Define { variable, value }) => {
                    format!("def v{}={}", variable.get(), render_value(*value))
                }
                Some(SsaResolvedEventV1::Kill { variable, previous }) => format!(
                    "kill v{}={}",
                    variable.get(),
                    previous.map_or_else(|| "undefined".to_owned(), render_value),
                ),
                None => format!("retained v{}", event.variable().get()),
            };
            writeln!(output, "  event {event_index} {resolved}").unwrap();
        }
        for (edge_index, edge) in block.edges().iter().enumerate() {
            let edge_id = SsaEdgeIdV1::new(block_id, edge_index as u32);
            writeln!(
                output,
                "  edge e{block_index}:{edge_index} role=r{} target=b{} defines={} args={} ssa-definitions={}",
                edge.role().get(),
                edge.target().get(),
                render_variables(edge.definitions()),
                render_arguments(plan.edge_arguments(edge_id).unwrap()),
                render_arguments(plan.edge_definitions(edge_id).unwrap()),
            )
            .unwrap();
        }
    }
    output
}

fn render_blocks(blocks: &[SsaBlockIdV1]) -> String {
    format!(
        "[{}]",
        blocks
            .iter()
            .map(|block| format!("b{}", block.get()))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn render_variables(variables: &[SsaVariableIdV1]) -> String {
    format!(
        "[{}]",
        variables
            .iter()
            .map(|variable| format!("v{}", variable.get()))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn render_arguments(arguments: &[SsaArgumentV1]) -> String {
    format!(
        "[{}]",
        arguments
            .iter()
            .map(|argument| format!(
                "v{}={}",
                argument.variable().get(),
                render_value(argument.value())
            ))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn render_value(value: SsaValueV1) -> String {
    match value {
        SsaValueV1::Definition(definition) => format!("d{}", definition.get()),
        SsaValueV1::BlockArgument { block, variable } => {
            format!("b{}.v{}", block.get(), variable.get())
        }
    }
}
