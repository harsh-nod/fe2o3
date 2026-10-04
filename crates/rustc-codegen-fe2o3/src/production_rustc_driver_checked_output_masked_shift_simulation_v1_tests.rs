//! Independent host Rust oracle; this test module grants no admission authority.
use super::*;
pub(in super::super) use constant_shift::Integer;

#[path = "production_rustc_driver_checked_output_masked_shift_graph_v1_tests.rs"]
mod graph;
pub(in super::super) use graph::check_graph;

pub(super) const NUMERICAL_POLICY: &str =
    "rust-fixed-integer-full-u32-masked-shifts-exact-bytes-v1";
pub(in super::super) const ROOTS: [&str; 2] = ["masked_shift_left", "masked_shift_right"];

pub(super) fn expected_grid(active: usize) -> [u64; 3] {
    [(active.max(1) as u64).div_ceil(64) * 64, 1, 1]
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(in super::super) enum Spelling {
    ExplicitMask,
    WrappingMethod,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(in super::super) struct Batch {
    pub(in super::super) integer: Integer,
    pub(in super::super) spelling: Spelling,
    pub(in super::super) retained: bool,
}

macro_rules! integers {
    ($(($variant:ident, $ty:ident)),* $(,)?) => {
        impl Batch {
            pub(in super::super) fn name(self) -> &'static str {
                match (self.integer, self.spelling, self.retained) {
                    $((Integer::$variant, Spelling::ExplicitMask, false) => concat!("masked-", stringify!($ty)),
                      (Integer::$variant, Spelling::ExplicitMask, true) => concat!("masked-", stringify!($ty), "-retained"),
                      (Integer::$variant, Spelling::WrappingMethod, false) => concat!("wrapping-", stringify!($ty)),
                      (Integer::$variant, Spelling::WrappingMethod, true) => concat!("wrapping-", stringify!($ty), "-retained")),*
                }
            }
        }
        fn host_rust(integer: Integer, value: u128, right: bool, count: u32) -> u128 {
            let bits = match integer {
                $(Integer::$variant => {
                    let value = value as $ty;
                    if right { value.wrapping_shr(count) as u128 }
                    else { value.wrapping_shl(count) as u128 }
                }),*
            };
            bits & mask(integer)
        }
        fn host_explicit(integer: Integer, value: u128, right: bool, count: u32) -> u128 {
            let count = count & (integer.width() - 1);
            let bits = match integer {
                $(Integer::$variant => {
                    let value = value as $ty;
                    if right { (value >> count) as u128 } else { (value << count) as u128 }
                }),*
            };
            bits & mask(integer)
        }
    }
}
integers! { (I8, i8), (U8, u8), (I16, i16), (U16, u16), (I32, i32), (U32, u32), (I64, i64), (U64, u64) }

impl Batch {
    pub(in super::super) fn all() -> Vec<Self> {
        Integer::ALL
            .into_iter()
            .flat_map(|integer| {
                [Spelling::ExplicitMask, Spelling::WrappingMethod]
                    .into_iter()
                    .flat_map(move |spelling| {
                        [false, true].into_iter().map(move |retained| Self {
                            integer,
                            spelling,
                            retained,
                        })
                    })
            })
            .collect()
    }

    pub(in super::super) fn parse(name: &str) -> Option<Self> {
        Self::all().into_iter().find(|batch| batch.name() == name)
    }
}

pub(in super::super) fn root_ordinal(root: &str) -> Result<usize, SourceFailure> {
    ROOTS
        .iter()
        .position(|expected| *expected == root)
        .ok_or_else(|| failure("unknown masked shift root identity"))
}

pub(in super::super) fn unique_root_ordinals<'a>(
    roots: impl IntoIterator<Item = &'a str>,
) -> Result<Vec<usize>, SourceFailure> {
    let mut seen = [false; ROOTS.len()];
    let mut ordinals = Vec::new();
    for root in roots {
        let ordinal = root_ordinal(root)?;
        if std::mem::replace(&mut seen[ordinal], true) {
            return Err(failure("duplicate masked shift root identity"));
        }
        ordinals.push(ordinal);
    }
    if seen.iter().any(|seen| !seen) {
        return Err(failure("missing masked shift root identity"));
    }
    Ok(ordinals)
}

fn mask(integer: Integer) -> u128 {
    (1_u128 << integer.width()) - 1
}

fn counts(integer: Integer) -> Vec<u32> {
    let width = integer.width();
    [
        0,
        1,
        width - 1,
        width,
        width + 1,
        2 * width - 1,
        2 * width,
        255,
        256,
        257,
        65_535,
        65_536,
        0x8000_0000,
        0x8000_0001,
        0x8000_0000 + width - 1,
        u32::MAX - 1,
        u32::MAX,
    ]
    .into_iter()
    .collect::<std::collections::BTreeSet<_>>()
    .into_iter()
    .collect()
}

fn vectors(integer: Integer) -> Vec<u128> {
    let high = 1_u128 << (integer.width() - 1);
    vec![
        0,
        1,
        high - 1,
        high,
        high + 1,
        mask(integer),
        0xaaaa_aaaa_aaaa_aaaa & mask(integer),
    ]
}

fn root_scenarios(batch: Batch, ordinal: usize) -> Result<Vec<Scenario>, SourceFailure> {
    assert!(ordinal < ROOTS.len());
    let integer = batch.integer;
    let values = vectors(integer);
    let counts = counts(integer);
    // Every full U32 count crosses both sign-bit and all-one inputs. Extra
    // values cover zero, low bits and alternating patterns without a huge grid.
    let pairs = std::iter::once((0, 0))
        .chain(counts.iter().flat_map(|&count| {
            [1_u128 << (integer.width() - 1), mask(integer)]
                .into_iter()
                .map(move |value| (value, count))
        }))
        .chain(values.into_iter().map(|value| (value, integer.width() + 1)));
    pairs
        .enumerate()
        .map(|(vector, (input, count))| {
            let len = if vector == 0 {
                0
            } else {
                [1, 63, 64, 65][(vector - 1) % 4]
            };
            let (output, initial) = constant_shift::backing(integer.scalar(), 0x37, len)?;
            let expected = constant_shift::backing(
                integer.scalar(),
                host_rust(integer, input, ordinal == 1, count),
                len,
            )?
            .1;
            Ok(Scenario {
                label: format!(
                    "{}-bits-{input:016x}-count-{count:08x}-{vector}-len-{len}",
                    ROOTS[ordinal]
                ),
                active: len,
                arguments: vec![
                    output,
                    SimulationArgumentV1::Scalar(
                        ScalarBitsV1::new(integer.scalar(), input, TARGET).map_err(failure)?,
                    ),
                    SimulationArgumentV1::Scalar(
                        ScalarBitsV1::new(ScalarType::U32, u128::from(count), TARGET)
                            .map_err(failure)?,
                    ),
                ],
                backings: vec![initial],
                expected: vec![expected],
                output_elements: len,
                written_elements: len,
            })
        })
        .collect()
}

pub(super) fn scenarios(batch: Batch) -> Result<Vec<Scenario>, SourceFailure> {
    let mut result = Vec::new();
    for ordinal in 0..ROOTS.len() {
        result.extend(root_scenarios(batch, ordinal)?);
    }
    Ok(result)
}

pub(super) fn runs(
    module: &AdmittedSimulationModuleV1,
    batch: Batch,
) -> Result<Vec<(&Kernel, Scenario)>, SourceFailure> {
    check_graph(module.module(), batch)?;
    let mut result = Vec::new();
    // Canonical module order derives from source identities. The independent
    // scenario order is stable by exact root ID, not by canonical ordinal.
    for (ordinal, root) in ROOTS.iter().enumerate() {
        let kernel = module
            .module()
            .kernels
            .iter()
            .find(|kernel| kernel.id.as_str() == *root)
            .ok_or_else(|| failure("masked shift oracle root missing"))?;
        for scenario in root_scenarios(batch, ordinal)? {
            result.push((kernel, scenario));
        }
    }
    Ok(result)
}

#[test]
fn full_u32_count_oracle_matches_both_safe_rust_spellings() {
    for integer in Integer::ALL {
        let high = 1_u128 << (integer.width() - 1);
        assert!(counts(integer).contains(&u32::MAX));
        assert!(counts(integer).contains(&0x8000_0001));
        for value in vectors(integer) {
            for count in counts(integer) {
                for right in [false, true] {
                    assert_eq!(
                        host_rust(integer, value, right, count),
                        host_explicit(integer, value, right, count)
                    );
                }
            }
        }
        assert_eq!(host_rust(integer, high, false, 0x8000_0000), high);
        assert_eq!(host_rust(integer, high, false, 0x8000_0001), 0);
        assert_eq!(
            host_rust(integer, high, true, u32::MAX),
            if integer.signed() { mask(integer) } else { 1 }
        );
    }
    let batches = Batch::all();
    assert_eq!(batches.len(), 32);
    for batch in batches {
        assert_eq!(Batch::parse(batch.name()), Some(batch));
        let rows = scenarios(batch).unwrap();
        assert_eq!(
            rows.len(),
            2 * (1 + 2 * counts(batch.integer).len() + vectors(batch.integer).len())
        );
        assert_eq!(rows.iter().filter(|row| row.active == 0).count(), 2);
        assert!(rows.iter().any(|row| row.label.contains("count-ffffffff")));
        assert!(rows.iter().any(|row| row.label.contains("count-80000001")));
    }
}

#[test]
fn dynamic_shift_oracle_detects_count_transport_and_signedness_errors() {
    for integer in Integer::ALL {
        let high = 1_u128 << (integer.width() - 1);
        assert_ne!(
            host_rust(integer, high, false, u32::MAX),
            host_rust(integer, high, false, 0)
        );
        if integer.signed() {
            assert_ne!(
                host_rust(integer, high, true, u32::MAX),
                high >> (integer.width() - 1)
            );
        }
    }
    let scenario = root_scenarios(
        Batch {
            integer: Integer::U8,
            spelling: Spelling::ExplicitMask,
            retained: false,
        },
        0,
    )
    .unwrap()
    .remove(1);
    let mut actual = scenario.expected.clone();
    let buffer = &actual[0].buffer;
    let mut bytes = buffer.bytes().to_vec();
    bytes[GUARD_ELEMENTS] ^= 1;
    actual[0].buffer = BufferArgumentV1::new(
        buffer.element(),
        buffer.access(),
        buffer.alignment(),
        bytes,
        buffer.initialized().to_vec(),
        TARGET,
    )
    .unwrap();
    assert!(check_backings(&actual, &scenario.expected).is_err());
}

#[test]
fn masked_shift_roster_joins_exact_ids_in_either_canonical_order() {
    assert_eq!(unique_root_ordinals(ROOTS).unwrap(), [0, 1]);
    assert_eq!(unique_root_ordinals([ROOTS[1], ROOTS[0]]).unwrap(), [1, 0]);
    assert!(unique_root_ordinals([ROOTS[0], ROOTS[0]]).is_err());
    assert!(unique_root_ordinals([ROOTS[0]]).is_err());
    assert!(unique_root_ordinals([ROOTS[0], "foreign"]).is_err());
    assert!(unique_root_ordinals([ROOTS[0], ROOTS[1], ROOTS[0]]).is_err());
}

// Inert framing rows only, shared by protocol tests. This does not invoke a
// simulator, construct a compiler owner or qualify any source invocation.
pub(in super::super) fn inert_report_framing(
    batch: Batch,
    output_digest: [u8; 32],
) -> SimulationObservation {
    SimulationObservation {
        case: Case::MaskedShift(batch),
        native_output_digest: output_digest,
        simulator_digest: output_digest,
        simulator_wire_version: 12,
        canonical_bytes: 1,
        numerical_policy: NUMERICAL_POLICY.into(),
        scenarios: scenarios(batch)
            .unwrap()
            .into_iter()
            .map(|scenario| {
                let grid = expected_grid(scenario.active);
                ScenarioObservation {
                    label: scenario.label,
                    grid,
                    workgroup: [64, 1, 1],
                    output_elements: scenario.output_elements,
                    written_elements: scenario.written_elements,
                    checked_backing_bytes: scenario
                        .expected
                        .iter()
                        .map(|row| row.buffer.bytes().len())
                        .sum(),
                    steps: 1,
                    invocations: grid[0],
                    deterministic_replays: 2,
                    conflicts: Assessment::NoObserved,
                    races: Assessment::NoObserved,
                }
            })
            .collect(),
    }
}

#[test]
fn masked_shift_report_requires_exact_grid_and_workgroup_for_every_batch() {
    for (active, extent) in [(0, 64), (1, 64), (63, 64), (64, 64), (65, 128)] {
        assert_eq!(expected_grid(active), [extent, 1, 1]);
    }
    for batch in Batch::all() {
        let case = Case::MaskedShift(batch);
        let report = inert_report_framing(batch, [1; 32]);
        check_report(&report, [1; 32], case).unwrap();
        let json = serde_json::to_value(&report).unwrap();
        for mutation in 0..6 {
            let mut changed = json.clone();
            let row = &mut changed["scenarios"][0];
            match mutation {
                0 => row["workgroup"] = serde_json::json!([32, 1, 1]),
                1 => row["workgroup"] = serde_json::json!([64, 2, 1]),
                2 | 3 => {
                    let extent = if mutation == 2 { 1 } else { 128 };
                    row["grid"] = serde_json::json!([extent, 1, 1]);
                    row["invocations"] = serde_json::json!(extent);
                }
                4 => row["grid"] = serde_json::json!([64, 2, 1]),
                5 => row["workgroup"] = serde_json::json!([128, 1, 1]),
                _ => unreachable!(),
            }
            let changed: SimulationObservation = serde_json::from_value(changed).unwrap();
            assert!(
                check_report(&changed, [1; 32], case).is_err(),
                "{} mutation {mutation}",
                batch.name()
            );
        }
    }
}
