use super::*;
use sha2::{Digest, Sha256};

const BYTES: &[u8] = include_bytes!("goldens.bin");
const COUNT: usize = 396;
const RECORD: usize = 120;
const TRAILER: usize = 64 + COUNT * RECORD;
const SHA256: [u8; 32] = [
    0xd4, 0xc7, 0x84, 0x9b, 0x71, 0x6f, 0xa3, 0x1f, 0x21, 0x84, 0xd1, 0x35, 0x6a, 0xab, 0xc9, 0x84,
    0xbf, 0x9f, 0x40, 0x06, 0x24, 0x7d, 0x93, 0x99, 0x50, 0x93, 0xd4, 0x0c, 0xa3, 0x7e, 0x75, 0x7d,
];

fn number(offset: usize) -> usize {
    usize::try_from(u64::from_le_bytes(
        BYTES[offset..offset + 8].try_into().unwrap(),
    ))
    .unwrap()
}
fn profile() {
    static CHECKED: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    CHECKED.get_or_init(|| {
        assert_eq!(&BYTES[..16], b"FE2O3SSAGOLD31V1");
        assert_eq!(BYTES.len(), TRAILER + 80);
        assert_eq!(<Sha256 as Digest>::digest(BYTES).as_slice(), SHA256);
        assert_eq!(number(16), COUNT);
        assert_eq!(
            number(32),
            std::mem::size_of::<usize>(),
            "golden host profile"
        );
        assert_eq!(number(40), std::mem::size_of::<Option<usize>>());
        assert_eq!(number(48), std::mem::size_of::<Vec<SsaVariableIdV1>>());
        assert_eq!(number(56), 1);
    });
}
fn record(case: usize) -> usize {
    profile();
    assert!(case < COUNT, "unknown golden case {case}");
    64 + case * RECORD
}
pub(super) fn old_plan_size() -> usize {
    profile();
    number(24)
}
pub(super) fn old_storage(case: usize) -> usize {
    number(record(case) + 96)
}
pub(super) fn expected_work(case: usize) -> usize {
    number(record(case) + 112)
}
pub(super) fn expected_storage(case: usize, input: &SsaConstructionInputV1) -> usize {
    let rows = number(record(case) + 104);
    let v = input.variable_count() as usize;
    let words = |bytes: usize| bytes.div_ceil(std::mem::size_of::<usize>());
    let saved = words(v * std::mem::size_of::<Option<usize>>())
        - words(v * std::mem::size_of::<Option<std::num::NonZeroU32>>())
        + words(input.blocks().len() * std::mem::size_of::<Vec<SsaVariableIdV1>>())
        + rows;
    // Frozen row layouts: case 389 saves 5 - 3; cases 394/395 save 20 - (12 + 5).
    let window_saved = match case {
        389 => 2,
        394 | 395 => 3,
        _ => 0,
    };
    old_storage(case)
        .checked_sub(
            saved + window_saved + journal_saving(case, input) + frontier_saving(input)
                + incoming_edge_saving(input),
        )
        .unwrap()
}
pub(super) fn incoming_edge_saving(input: &SsaConstructionInputV1) -> usize {
    let blocks = input.blocks().len();
    let words = |bytes: usize| bytes.div_ceil(size_of::<usize>());
    words(blocks * size_of::<Option<(usize, usize)>>())
        - words(blocks * size_of::<Option<std::num::NonZeroU64>>())
}
pub(super) fn frontier_saving(input: &SsaConstructionInputV1) -> usize {
    super::super::planner::frontier_indices_v1_tests::oracle::saving(input)
}
pub(super) fn journal_saving(case: usize, input: &SsaConstructionInputV1) -> usize {
    // Independent input reachability plus the immutable merge roster total.
    // Do not query the candidate plan to predict its journal allocation.
    let mut changes = number(record(case) + 104);
    let mut seen = vec![false; input.blocks().len()];
    let mut pending = vec![input.entry().get() as usize];
    while let Some(block) = pending.pop() {
        if std::mem::replace(&mut seen[block], true) {
            continue;
        }
        let block = &input.blocks()[block];
        changes += block.events().iter().filter(|event| {
            matches!(event, SsaEventV1::Define(_) | SsaEventV1::Kill(_))
                && input.promotable()[event.variable().get() as usize]
        }).count();
        for edge in block.edges() {
            changes += edge.definitions().iter()
                .filter(|v| input.promotable()[v.get() as usize]).count();
            pending.push(edge.target().get() as usize);
        }
    }
    let old = changes * size_of::<(usize, Option<SsaValueV1>)>();
    let new = changes * size_of::<(u32, Option<SsaValueV1>)>();
    old.div_ceil(8) - new.div_ceil(8)
}
pub(super) fn assert_extra(offset: usize, error: &SsaPlannerErrorV1) {
    profile();
    assert!(matches!(offset, 0 | 32));
    assert_eq!(
        debug_digest(error),
        BYTES[TRAILER + offset..TRAILER + offset + 32]
    );
}
pub(super) fn old_storage_failure() -> (usize, usize) {
    profile();
    (number(TRAILER + 64), number(TRAILER + 72))
}
pub(super) fn compare(
    case: usize,
    input: &SsaConstructionInputV1,
) -> Option<SsaConstructionPlanV1> {
    let offset = record(case);
    assert_eq!(
        debug_digest(input),
        BYTES[offset..offset + 32],
        "case {case} input"
    );
    let result = plan_ssa_v1(input);
    assert_eq!(
        public_digest(input, &result),
        BYTES[offset + 64..offset + 96],
        "case {case} views/error"
    );
    match result {
        Err(_) => {
            assert!(BYTES[offset + 32..offset + 64].iter().all(|b| *b == 0));
            assert_eq!((old_storage(case), expected_work(case)), (0, 0));
            None
        }
        Ok(plan) => {
            assert_eq!(plan.identity().as_bytes(), &BYTES[offset + 32..offset + 64]);
            assert_eq!(
                plan.resources().storage_words(),
                expected_storage(case, input)
            );
            assert_eq!(plan.resources().work_units(), expected_work(case));
            let mut rows = 0;
            for block in 0..=input.blocks().len() {
                let id = SsaBlockIdV1::new(block as u32);
                if let Some(merge) = plan.merge_variables(id) {
                    let transport = plan.transport_variables(id).unwrap();
                    assert_eq!(merge, transport);
                    assert_eq!(
                        merge.as_ptr(),
                        transport.as_ptr(),
                        "one immutable row allocation"
                    );
                    rows += merge.len();
                }
            }
            assert_eq!(rows, number(offset + 104));
            plan.verify_replay(input, SsaPlannerLimitsV1::default())
                .unwrap();
            Some(plan)
        }
    }
}

use std::fmt::{Debug, Write as _};

// Frozen test encoding: the same complete Debug values as the original
// differential assertions, streamed fieldwise, never the private Plan layout.
struct Stream(Sha256);
impl std::fmt::Write for Stream {
    fn write_str(&mut self, text: &str) -> std::fmt::Result {
        self.0.update(text.as_bytes());
        Ok(())
    }
}
pub(super) fn debug_digest(value: &impl Debug) -> [u8; 32] {
    let mut out = Stream(Sha256::new());
    out.0.update(b"fe2o3.ssa.test-debug-golden.v1\0");
    write!(out, "{value:?}").unwrap();
    out.0.finalize().into()
}
pub(super) fn public_digest(
    input: &SsaConstructionInputV1,
    result: &Result<SsaConstructionPlanV1, SsaPlannerErrorV1>,
) -> [u8; 32] {
    let mut out = Stream(Sha256::new());
    out.0.update(b"fe2o3.ssa.public-getter-golden.v1\0");
    macro_rules! field {
        ($name:literal, $value:expr) => {
            writeln!(out, "{}={:?}", $name, $value).unwrap()
        };
    }
    field!("error", result.as_ref().err());
    if let Ok(plan) = result {
        field!("identity", plan.identity().as_bytes());
        field!("definitions", plan.definition_count());
        field!("rpo", plan.reverse_postorder());
        field!("promoted", plan.promoted_variables());
        field!("entry-definitions", plan.entry_definitions());
        field!("entry-arguments", plan.entry_arguments());
        for block in 0..=input.blocks().len() {
            let id = SsaBlockIdV1::new(block as u32);
            field!("block", block);
            field!("reachable", plan.is_reachable(id));
            field!("live-in", plan.live_in(id));
            field!("merge", plan.merge_variables(id));
            field!("transport", plan.transport_variables(id));
            field!("events", plan.resolved_events(id));
            let source = input.blocks().get(block);
            for event in 0..=source.map_or(0, |b| b.events().len()) {
                field!("event", (event, plan.resolved_event(id, event as u32)));
            }
            for ordinal in 0..=source.map_or(0, |b| b.edges().len()) {
                let edge = SsaEdgeIdV1::new(id, ordinal as u32);
                field!("edge", ordinal);
                field!("edge-arguments", plan.edge_arguments(edge));
                field!("edge-definitions", plan.edge_definitions(edge));
            }
        }
        let r = plan.resources();
        field!(
            "resources-except-storage",
            (
                r.input_blocks(),
                r.reachable_blocks(),
                r.pruned_blocks(),
                r.input_edges(),
                r.input_events(),
                r.input_edge_definitions(),
                r.generated_definitions(),
                r.output_items(),
                r.work_units()
            )
        );
    }
    out.0.finalize().into()
}
