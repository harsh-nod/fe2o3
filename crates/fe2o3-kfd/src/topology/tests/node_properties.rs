use super::*;
use crate::topology::node_properties as fixed;

mod reference;

const KEYS: [&str; 39] = [
    "array_count",
    "caches_count",
    "capability",
    "capability2",
    "cpu_core_id_base",
    "cpu_cores_count",
    "cu_per_simd_array",
    "debug_prop",
    "device_id",
    "domain",
    "drm_render_minor",
    "fw_version",
    "gds_size_in_kb",
    "gfx_target_version",
    "hive_id",
    "io_links_count",
    "lds_size_in_kb",
    "local_mem_size",
    "location_id",
    "max_engine_clk_ccompute",
    "max_engine_clk_fcompute",
    "max_slots_scratch_cu",
    "max_waves_per_simd",
    "mem_banks_count",
    "num_cp_queues",
    "num_gws",
    "num_sdma_engines",
    "num_sdma_queues_per_engine",
    "num_sdma_xgmi_engines",
    "num_xcc",
    "p2p_links_count",
    "sdma_fw_version",
    "simd_arrays_per_engine",
    "simd_count",
    "simd_id_base",
    "simd_per_cu",
    "unique_id",
    "vendor_id",
    "wave_front_size",
];

#[derive(Debug, Eq, PartialEq)]
enum Failure {
    Malformed(PathBuf, usize),
    TooMany(PathBuf, usize),
    Unknown(PathBuf, String),
    Duplicate(PathBuf, String),
    Range(PathBuf, String, u64, u64, u64),
}

impl From<TopologyError> for Failure {
    fn from(error: TopologyError) -> Self {
        match error {
            TopologyError::MalformedPropertyLine { path, line } => Self::Malformed(path, line),
            TopologyError::TooManyProperties { path, maximum } => Self::TooMany(path, maximum),
            TopologyError::UnknownProperty { path, key } => Self::Unknown(path, key),
            TopologyError::DuplicateProperty { path, key } => Self::Duplicate(path, key),
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

fn check(text: &str) -> Result<[Option<u64>; 39], Failure> {
    let path = Path::new("nodes/1/properties");
    let expected = reference::parse(path, text)
        .map(|properties| KEYS.map(|key| properties.get(key).copied()))
        .map_err(Failure::from);
    let actual = fixed::parse(path, text)
        .map(|properties| {
            for unknown in ["", "type", "platform_id", "unknown", "Array_count"] {
                assert_eq!(properties.get(unknown), None);
            }
            KEYS.map(|key| properties.get(key).copied())
        })
        .map_err(Failure::from);
    assert_eq!(actual, expected, "input: {text:?}");
    actual
}

fn lines(values: [u64; 39]) -> Vec<String> {
    KEYS.iter()
        .zip(values)
        .map(|(key, value)| format!("{key} {value}\n"))
        .collect()
}

#[test]
fn every_slot_has_independent_presence_value_and_exact_bounds() {
    assert_eq!(
        std::mem::size_of::<fixed::NodeProperties>(),
        40 * std::mem::size_of::<u64>()
    );
    for (slot, key) in KEYS.iter().enumerate() {
        let (minimum, maximum) = reference::property_range(key).unwrap();
        for value in [minimum, 1, maximum] {
            let mut expected = [None; 39];
            expected[slot] = Some(value);
            assert_eq!(check(&format!("{key} {value}\n")).unwrap(), expected);
        }
        if let Some(value) = maximum.checked_add(1) {
            assert!(
                matches!(check(&format!("{key} {value}\n")), Err(Failure::Range(_, _, observed, min, max))
                if observed == value && min == minimum && max == maximum)
            );
        }
    }
    let maxima = KEYS.map(|key| reference::property_range(key).unwrap().1);
    assert_eq!(check(&lines(maxima).concat()).unwrap(), maxima.map(Some));
}

#[test]
fn property_order_and_sparse_subsets_match_the_frozen_parser() {
    let values = std::array::from_fn(|slot| slot as u64 + 1);
    let original = lines(values);
    for rotation in 0..KEYS.len() {
        let mut reordered = original.clone();
        reordered.rotate_left(rotation);
        assert_eq!(check(&reordered.concat()).unwrap(), values.map(Some));
        reordered.reverse();
        assert_eq!(check(&reordered.concat()).unwrap(), values.map(Some));
    }
    let mut seed = 0x49e2_067d_u64;
    for _ in 0..256 {
        let mut permutation = original.clone();
        for index in (1..permutation.len()).rev() {
            seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
            permutation.swap(index, (seed % (index as u64 + 1)) as usize);
        }
        assert_eq!(check(&permutation.concat()).unwrap(), values.map(Some));
        let length = 1 + (seed % KEYS.len() as u64) as usize;
        assert!(check(&permutation[..length].concat()).is_ok());
    }
    for first in 0..KEYS.len() {
        for second in first + 1..KEYS.len() {
            let mut expected = [None; 39];
            expected[first] = Some(values[first]);
            expected[second] = Some(values[second]);
            assert_eq!(
                check(&format!("{}{}", original[second], original[first])).unwrap(),
                expected
            );
        }
    }
}

#[test]
fn malformed_unknown_overflow_and_duplicate_precedence_are_unchanged() {
    for key in KEYS {
        assert!(
            matches!(check(&format!("{key} 0\n{key} 1\n")), Err(Failure::Duplicate(_, k)) if k == key)
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
            "1 2",
            "18446744073709551616",
        ] {
            assert!(matches!(
                check(&format!("{key} 0\n{key} {raw}\n")),
                Err(Failure::Malformed(_, 2))
            ));
        }
        if let Some(value) = reference::property_range(key).unwrap().1.checked_add(1) {
            assert!(
                matches!(check(&format!("{key} 0\n{key} {value}\n")), Err(Failure::Range(_, k, _, _, _)) if k == key)
            );
        }
    }
    for raw in ["0", "18446744073709551616"] {
        assert!(matches!(
            check(&format!("unknown {raw}\n")),
            Err(Failure::Unknown(_, _))
        ));
    }
    for key in ["", "Unknown", "unknown-", "unknown\t", "\u{e9}"] {
        assert!(matches!(
            check(&format!("{key} 1\n")),
            Err(Failure::Malformed(_, 1))
        ));
    }
    for raw in ["", "00", "+1", "1 2", "\u{0661}"] {
        assert!(matches!(
            check(&format!("unknown {raw}\n")),
            Err(Failure::Malformed(_, 1))
        ));
    }
}

#[test]
fn line_grammar_terminators_and_overfull_inputs_match() {
    for text in [
        "unknown 0\narray_count 1",
        "array_count 0\narray_count 1\ncaches_count 1",
    ] {
        assert!(matches!(check(text), Err(Failure::Malformed(_, 1))));
    }
    for text in [
        "",
        "\n",
        "array_count",
        "array_count 1",
        "array_count 1\n\n",
        "array_count\t1\n",
        "array_count 1\r\n",
        "array_count 1\0\n",
    ] {
        assert!(check(text).is_err());
    }
    let valid = lines([1; 39]);
    for position in 0..valid.len() {
        for malformed in ["\n", "broken\n", " 1\n", "array_count  1\n"] {
            let mut changed = valid.clone();
            changed[position] = malformed.to_owned();
            assert!(
                matches!(check(&changed.concat()), Err(Failure::Malformed(_, line)) if line == position + 1)
            );
        }
    }
    // The closed schema must fail before the generic line cap: a 40th accepted key repeats.
    for count in [40, 64, 65, 128] {
        let text = valid
            .iter()
            .cycle()
            .take(count)
            .cloned()
            .collect::<String>();
        assert!(matches!(check(&text), Err(Failure::Duplicate(_, _))));
    }
    for count in [1, 39, 64, 65] {
        assert!(matches!(
            check(&"unknown 0\n".repeat(count)),
            Err(Failure::Unknown(_, _))
        ));
    }
}

#[test]
fn reader_observations_and_errors_match_the_frozen_parser() {
    let fixture = Fixture::valid(1);
    let path = fixture.node(1).join("properties");
    let original = fs::read(&path).unwrap();
    for bytes in [
        original,
        b"unknown 0\n".to_vec(),
        vec![0xff],
        vec![b'0'; MAX_PROPERTY_BYTES + 1],
    ] {
        fs::write(&path, bytes).unwrap();
        let expected = host_diagnostics::capture(|| {
            read_text(&path, MAX_PROPERTY_BYTES)
                .and_then(|text| reference::parse(&path, &text))
                .map(|properties| KEYS.map(|key| properties.get(key).copied()))
                .map_err(|error| format!("{error:?}"))
        });
        let actual = host_diagnostics::capture(|| {
            parse_properties(&path)
                .map(|properties| KEYS.map(|key| properties.get(key).copied()))
                .map_err(|error| format!("{error:?}"))
        });
        assert_eq!(actual, expected);
        let operations: Vec<_> = actual
            .1
            .iter()
            .map(|(operation, _, _)| *operation)
            .collect();
        let mut expected = vec!["inspect", "open", "opened metadata", "bounded read"];
        if fs::metadata(&path).unwrap().len() <= MAX_PROPERTY_BYTES as u64 {
            expected.push("closing metadata");
        }
        assert_eq!(operations, expected);
    }
}

#[test]
fn cpu_gpu_discovery_is_invariant_under_property_order_and_unused_keys() {
    let fixture = Fixture::valid(3);
    let expected = fixture.discover().unwrap();
    for node in 0..=3 {
        let path = fixture.node(node).join("properties");
        let text = fs::read_to_string(&path).unwrap();
        assert!(check(&text).is_ok());
        let mut rows: Vec<_> = text.split_inclusive('\n').collect();
        rows.reverse();
        fs::write(
            &path,
            format!(
                "{}capability 0\ndebug_prop 18446744073709551615\n",
                rows.concat()
            ),
        )
        .unwrap();
    }
    assert_eq!(fixture.discover().unwrap(), expected);
    // CPU properties remain strictly parsed even though they do not construct a GPU record.
    fs::write(
        fixture.node(0).join("properties"),
        "cpu_cores_count 48\ncpu_cores_count 48\n",
    )
    .unwrap();
    assert!(
        matches!(fixture.discover(), Err(TopologyError::DuplicateProperty { key, .. }) if key == "cpu_cores_count")
    );
}

#[test]
fn gpu_missing_keys_preserve_admission_order_and_optional_sdma_fields() {
    const REQUIRED: [&str; 23] = [
        "gfx_target_version",
        "vendor_id",
        "device_id",
        "drm_render_minor",
        "unique_id",
        "hive_id",
        "location_id",
        "domain",
        "fw_version",
        "sdma_fw_version",
        "simd_count",
        "simd_per_cu",
        "array_count",
        "simd_arrays_per_engine",
        "lds_size_in_kb",
        "max_waves_per_simd",
        "num_cp_queues",
        "mem_banks_count",
        "caches_count",
        "io_links_count",
        "p2p_links_count",
        "wave_front_size",
        "num_xcc",
    ];
    const OPTIONAL: [&str; 3] = [
        "num_sdma_engines",
        "num_sdma_xgmi_engines",
        "num_sdma_queues_per_engine",
    ];
    let fixture = Fixture::valid(1);
    let path = fixture.node(1).join("properties");
    let original = fs::read_to_string(&path).unwrap();
    for (index, &first) in REQUIRED.iter().enumerate() {
        for &second in &REQUIRED[index..] {
            let text: String = original
                .split_inclusive('\n')
                .filter(|line| {
                    let key = line.split_once(' ').unwrap().0;
                    key != first && key != second
                })
                .collect();
            assert!(check(&text).is_ok());
            let properties = fixed::parse(&path, &text).unwrap();
            assert!(
                matches!(parse_gpu_node(1, 1001, "ip discovery".to_owned(), &path, &properties),
                    Err(TopologyError::MissingProperty { path: p, key }) if p == path && key == first)
            );
        }
    }
    for mask in 0..8 {
        let text: String = original
            .split_inclusive('\n')
            .filter(|line| {
                let key = line.split_once(' ').unwrap().0;
                OPTIONAL
                    .iter()
                    .position(|optional| *optional == key)
                    .is_none_or(|slot| mask & (1 << slot) != 0)
            })
            .collect();
        let properties = fixed::parse(&path, &text).unwrap();
        let gpu = parse_gpu_node(1, 1001, "ip discovery".to_owned(), &path, &properties).unwrap();
        let capability = gpu.sdma_topology_capability;
        assert_eq!(
            [
                capability.engine_count,
                capability.xgmi_engine_count,
                capability.queues_per_engine
            ],
            std::array::from_fn(|slot| (mask & (1 << slot) != 0).then_some([2, 14, 8][slot]))
        );
    }
    let zero_optional: String = original
        .split_inclusive('\n')
        .map(|line| {
            let key = line.split_once(' ').unwrap().0;
            if OPTIONAL.contains(&key) {
                format!("{key} 0\n")
            } else {
                line.to_owned()
            }
        })
        .collect();
    let properties = fixed::parse(&path, &zero_optional).unwrap();
    let gpu = parse_gpu_node(1, 1001, "ip discovery".to_owned(), &path, &properties).unwrap();
    let capability = gpu.sdma_topology_capability;
    assert_eq!(
        [
            capability.engine_count,
            capability.xgmi_engine_count,
            capability.queues_per_engine
        ],
        [Some(0); 3]
    );
}
