use super::*;
use crate::topology::link_properties::{self as fixed, LinkProperties};

// The unchanged generic parser and this independent schema are the pre-optimization oracle.
const SCHEMA: [(&str, u64, u64); 13] = [
    ("type", 0, u32::MAX as u64),
    ("version_major", 0, u32::MAX as u64),
    ("version_minor", 0, u32::MAX as u64),
    ("node_from", 0, u16::MAX as u64),
    ("node_to", 0, u16::MAX as u64),
    ("weight", 0, u32::MAX as u64),
    ("min_latency", 0, u64::MAX),
    ("max_latency", 0, u64::MAX),
    ("min_bandwidth", 0, u64::MAX),
    ("max_bandwidth", 0, u64::MAX),
    ("recommended_transfer_size", 0, u64::MAX),
    ("recommended_sdma_engine_id_mask", 0, u64::MAX),
    ("flags", 0, u32::MAX as u64),
];

#[derive(Debug, Eq, PartialEq)]
enum Failure {
    Malformed(PathBuf, usize),
    TooMany(PathBuf, usize),
    Unknown(PathBuf, String),
    Duplicate(PathBuf, String),
    Missing(PathBuf, &'static str),
    Range(PathBuf, String, u64, u64, u64),
}

impl From<TopologyError> for Failure {
    fn from(error: TopologyError) -> Self {
        match error {
            TopologyError::MalformedPropertyLine { path, line } => Self::Malformed(path, line),
            TopologyError::TooManyProperties { path, maximum } => Self::TooMany(path, maximum),
            TopologyError::UnknownProperty { path, key } => Self::Unknown(path, key),
            TopologyError::DuplicateProperty { path, key } => Self::Duplicate(path, key),
            TopologyError::MissingProperty { path, key } => Self::Missing(path, key),
            TopologyError::PropertyOutOfRange {
                path,
                key,
                value,
                minimum,
                maximum,
            } => Self::Range(path, key, value, minimum, maximum),
            error => panic!("unexpected parser error: {error:?}"),
        }
    }
}

fn fields(value: LinkProperties) -> [u64; 13] {
    [
        u64::from(value.link_type),
        u64::from(value.version_major),
        u64::from(value.version_minor),
        u64::from(value.node_from),
        u64::from(value.node_to),
        u64::from(value.weight),
        value.min_latency,
        value.max_latency,
        value.min_bandwidth,
        value.max_bandwidth,
        value.recommended_transfer_size,
        value.recommended_sdma_engine_id_mask,
        u64::from(value.flags),
    ]
}

struct Cases {
    _fixture: Fixture,
    path: PathBuf,
}

impl Cases {
    fn new() -> Self {
        let fixture = Fixture::valid(1);
        let path = fixture.node(1).join("io_links/0/properties");
        Self {
            _fixture: fixture,
            path,
        }
    }

    fn check(&self, text: &str) -> Result<[u64; 13], Failure> {
        fs::write(&self.path, text).unwrap();
        let reference =
            parse_named_properties_prechecked(inspect_regular(&self.path).unwrap(), &SCHEMA)
                .map(|properties| SCHEMA.map(|(key, _, _)| properties[key]))
                .map_err(Failure::from);
        let actual = fixed::read(inspect_regular(&self.path).unwrap(), &mut Vec::new())
            .map(fields)
            .map_err(Failure::from);
        assert_eq!(actual, reference, "input: {text:?}");
        actual
    }
}

fn lines(values: [u64; 13]) -> Vec<String> {
    SCHEMA
        .iter()
        .zip(values)
        .map(|((key, _, _), value)| format!("{key} {value}\n"))
        .collect()
}

#[test]
fn distinct_fields_bounds_and_property_order_match_generic_parser() {
    let cases = Cases::new();
    let values = std::array::from_fn(|index| 17 + index as u64);
    let original = lines(values);
    for rotation in 0..SCHEMA.len() {
        let mut permutation = original.clone();
        permutation.rotate_left(rotation);
        assert_eq!(cases.check(&permutation.concat()).unwrap(), values);
        permutation.reverse();
        assert_eq!(cases.check(&permutation.concat()).unwrap(), values);
    }
    let mut seed = 0x4fa2_035d_u64;
    for _ in 0..128 {
        let mut permutation = original.clone();
        for index in (1..SCHEMA.len()).rev() {
            seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
            permutation.swap(index, (seed % (index as u64 + 1)) as usize);
        }
        assert_eq!(cases.check(&permutation.concat()).unwrap(), values);
    }
    for (slot, (_, minimum, maximum)) in SCHEMA.iter().enumerate() {
        for value in [*minimum, *maximum] {
            let mut values = values;
            values[slot] = value;
            assert_eq!(cases.check(&lines(values).concat()).unwrap(), values);
        }
    }
    let all_maximum = SCHEMA.map(|(_, _, maximum)| maximum);
    assert_eq!(
        cases.check(&lines(all_maximum).concat()).unwrap(),
        all_maximum
    );
}

#[test]
fn every_nonempty_schema_subset_preserves_first_missing_key() {
    let cases = Cases::new();
    let original = lines([0; 13]);
    for mask in 1_u16..(1 << SCHEMA.len()) {
        let text: String = original
            .iter()
            .enumerate()
            .filter(|(slot, _)| mask & (1 << slot) != 0)
            .map(|(_, line)| line.as_str())
            .collect();
        let actual = cases.check(&text);
        match (0..SCHEMA.len()).find(|slot| mask & (1 << slot) == 0) {
            Some(slot) => assert_eq!(
                actual,
                Err(Failure::Missing(cases.path.clone(), SCHEMA[slot].0))
            ),
            None => assert_eq!(actual, Ok([0; 13])),
        }
    }
}

#[test]
fn duplicate_numeric_range_and_unknown_precedence_match_generic_parser() {
    let cases = Cases::new();
    for (key, _, maximum) in SCHEMA {
        assert_eq!(
            cases.check(&format!("{key} 0\n{key} 1\n")),
            Err(Failure::Duplicate(cases.path.clone(), key.to_owned()))
        );
        for raw in [
            "",
            "00",
            "01",
            "+1",
            "-1",
            "1 ",
            " 1",
            "1\t",
            "1\r",
            "18446744073709551616",
        ] {
            assert_eq!(
                cases.check(&format!("{key} 0\n{key} {raw}\n")),
                Err(Failure::Malformed(cases.path.clone(), 2))
            );
        }
        if let Some(out_of_range) = maximum.checked_add(1) {
            assert_eq!(
                cases.check(&format!("{key} 0\n{key} {out_of_range}\n")),
                Err(Failure::Range(
                    cases.path.clone(),
                    key.to_owned(),
                    out_of_range,
                    0,
                    maximum
                ))
            );
        }
    }
    let original = lines([0; 13]);
    for index in 0..SCHEMA.len() {
        for raw in ["0", "18446744073709551616"] {
            let mut changed = original.clone();
            changed[index] = format!("unknown {raw}\n");
            assert_eq!(
                cases.check(&changed.concat()),
                Err(Failure::Unknown(cases.path.clone(), "unknown".to_owned()))
            );
        }
        for raw in ["00", "+1", "-1", "", "1 2"] {
            let mut changed = original.clone();
            changed[index] = format!("unknown {raw}\n");
            assert_eq!(
                cases.check(&changed.concat()),
                Err(Failure::Malformed(cases.path.clone(), index + 1))
            );
        }
    }
}

#[test]
fn malformed_lines_terminators_and_overfull_input_preserve_error_order() {
    let cases = Cases::new();
    let original = lines([0; 13]);
    for index in 0..SCHEMA.len() {
        for line in [
            "\n",
            "type\n",
            " 1\n",
            "type  1\n",
            "type\t1\n",
            "type 1\r\n",
            "type 1\t\n",
            "type 1\0\n",
        ] {
            let mut changed = original.clone();
            changed[index] = line.to_owned();
            assert_eq!(
                cases.check(&changed.concat()),
                Err(Failure::Malformed(cases.path.clone(), index + 1))
            );
        }
    }
    for text in [
        "",
        "\n",
        "type 0",
        "unknown 0\ntype 0",
        "type 0\ntype 1\nflags 0",
    ] {
        assert_eq!(
            cases.check(text),
            Err(Failure::Malformed(cases.path.clone(), 1))
        );
    }
    for extra in [
        "\n",
        "malformed\n",
        "unknown 0\n",
        "type 0\n",
        "type 4294967296\n",
    ] {
        assert_eq!(
            cases.check(&(original.concat() + extra)),
            Err(Failure::TooMany(cases.path.clone(), SCHEMA.len()))
        );
    }
    assert_eq!(
        cases.check(&(original.concat() + "malformed")),
        Err(Failure::Malformed(cases.path.clone(), 1))
    );
}

fn reference_link(path: &Path, set: KfdTopologyLinkSetV1, index: u32) -> KfdTopologyLinkV1 {
    let p = parse_named_properties_prechecked(inspect_regular(path).unwrap(), &SCHEMA).unwrap();
    KfdTopologyLinkV1 {
        set,
        index,
        link_type: p["type"] as u32,
        version_major: p["version_major"] as u32,
        version_minor: p["version_minor"] as u32,
        node_from: p["node_from"] as u32,
        node_to: p["node_to"] as u32,
        weight: p["weight"] as u32,
        min_latency: p["min_latency"],
        max_latency: p["max_latency"],
        min_bandwidth: p["min_bandwidth"],
        max_bandwidth: p["max_bandwidth"],
        recommended_transfer_size: p["recommended_transfer_size"],
        recommended_sdma_engine_id_mask: p["recommended_sdma_engine_id_mask"],
        flags: p["flags"] as u32,
    }
}

#[test]
fn full_discovery_matches_reference_links_and_is_invariant_to_property_order() {
    for gpu_count in [1, 2, 3] {
        let fixture = Fixture::valid(gpu_count);
        let first = fixture.discover().unwrap();
        for node in &first.gpu_nodes {
            for (name, set, links) in [
                ("io_links", KfdTopologyLinkSetV1::Io, &node.io_links),
                ("p2p_links", KfdTopologyLinkSetV1::P2p, &node.p2p_links),
            ] {
                for link in links {
                    let path = fixture
                        .node(node.node_id)
                        .join(name)
                        .join(link.index.to_string())
                        .join("properties");
                    assert_eq!(*link, reference_link(&path, set, link.index));
                    let text = fs::read_to_string(&path).unwrap();
                    let reversed: String = text.split_inclusive('\n').rev().collect();
                    fs::write(path, reversed).unwrap();
                }
            }
        }
        assert_eq!(fixture.discover().unwrap(), first);
    }
}

#[test]
fn endpoint_and_range_validation_order_and_zero_maximum_are_unchanged() {
    let fixture = Fixture::valid(1);
    let node = fixture.node(1);
    let path = node.join("io_links/0/properties");
    let original = fs::read_to_string(&path).unwrap();
    for (minimum, maximum) in [
        ("min_latency", "max_latency"),
        ("min_bandwidth", "max_bandwidth"),
    ] {
        let invalid_range = original
            .replace(&format!("{minimum} 0\n"), &format!("{minimum} 9\n"))
            .replace(
                if maximum == "max_latency" {
                    "max_latency 0\n"
                } else {
                    "max_bandwidth 64000\n"
                },
                &format!("{maximum} 1\n"),
            );
        fs::write(
            &path,
            invalid_range.replace("node_from 1\n", "node_from 0\n"),
        )
        .unwrap();
        assert!(
            matches!(parse_topology_links(&node, 1, KfdTopologyLinkSetV1::Io, 1),
            Err(TopologyError::InvalidLinkEndpoint { path: p, expected_from: 1, observed_from: 0, observed_to: 0 }) if p == path)
        );
        fs::write(&path, &invalid_range).unwrap();
        assert!(
            matches!(parse_topology_links(&node, 1, KfdTopologyLinkSetV1::Io, 1),
            Err(TopologyError::InvalidLinkRange(p)) if p == path)
        );
        fs::write(
            &path,
            invalid_range.replace(&format!("{maximum} 1\n"), &format!("{maximum} 0\n")),
        )
        .unwrap();
        assert!(parse_topology_links(&node, 1, KfdTopologyLinkSetV1::Io, 1).is_ok());
    }
}
