use super::*;
use crate::topology::link_properties;
use crate::topology::tests::{directory_entries::allocation_counter, host_diagnostics};

fn properties_path(fixture: &Fixture) -> PathBuf {
    fixture.node(1).join("io_links/0/properties")
}

#[test]
fn long_then_short_links_reuse_storage_without_a_stale_suffix() {
    let fixture = Fixture::valid(1);
    let path = properties_path(&fixture);
    let short = fs::read(&path).unwrap();
    let long = "type 4294967295\nversion_major 4294967295\nversion_minor 4294967295\n\
        node_from 65535\nnode_to 65535\nweight 4294967295\n\
        min_latency 18446744073709551615\nmax_latency 18446744073709551615\n\
        min_bandwidth 18446744073709551615\nmax_bandwidth 18446744073709551615\n\
        recommended_transfer_size 18446744073709551615\n\
        recommended_sdma_engine_id_mask 18446744073709551615\nflags 4294967295\n";
    assert_eq!(long.len(), 367);
    assert!(long.len() > short.len());
    let mut bytes = Vec::new();
    fs::write(&path, long).unwrap();
    let first = link_properties::read(inspect_regular(&path).unwrap(), &mut bytes).unwrap();
    assert_eq!(first.weight, u32::MAX);
    assert_eq!(first.max_bandwidth, u64::MAX);
    let capacity = bytes.capacity();
    let pointer = bytes.as_ptr();
    fs::write(&path, &short).unwrap();
    let second = link_properties::read(inspect_regular(&path).unwrap(), &mut bytes).unwrap();
    assert_eq!(second.weight, 15);
    assert_eq!(bytes, short);
    assert_eq!(bytes.capacity(), capacity);
    assert_eq!(bytes.as_ptr(), pointer);
}

#[test]
fn failed_reads_and_empty_text_cannot_contaminate_the_next_link() {
    let fixture = Fixture::valid(1);
    let path = properties_path(&fixture);
    let good = fs::read(&path).unwrap();
    let mut bytes = Vec::new();
    for (bad, expected) in [
        (vec![b'x'; MAX_PROPERTY_BYTES + 1], "FileTooLarge"),
        (vec![0xff], "InvalidUtf8"),
        (vec![], "MalformedPropertyLine"),
        (b"weight 1\n".to_vec(), "MissingProperty"),
    ] {
        fs::write(&path, &bad).unwrap();
        let error = link_properties::read(inspect_regular(&path).unwrap(), &mut bytes).unwrap_err();
        assert!(format!("{error:?}").contains(expected), "{error:?}");
        assert_eq!(bytes, bad);
        fs::write(&path, &good).unwrap();
        let recovered = link_properties::read(inspect_regular(&path).unwrap(), &mut bytes).unwrap();
        assert_eq!(recovered.weight, 15);
        assert_eq!(bytes, good);
    }
}

#[test]
fn identity_and_open_refusals_clear_scratch_without_allocating_read_storage() {
    let fixture = Fixture::valid(1);
    let path = properties_path(&fixture);
    for missing in [false, true] {
        for populated in [false, true] {
            fs::write(&path, "weight 15\n").unwrap();
            let before = inspect_regular(&path).unwrap();
            if missing {
                fs::remove_file(&path).unwrap();
            } else {
                fs::write(&path, "weight 1500\n").unwrap();
            }
            let mut bytes = if populated { vec![7; 32] } else { Vec::new() };
            let capacity = bytes.capacity();
            let error =
                read_bounded_regular_into(before, MAX_PROPERTY_BYTES, &mut bytes).unwrap_err();
            if missing {
                assert!(matches!(
                    error,
                    TopologyError::Io {
                        operation: "open",
                        ..
                    }
                ));
            } else {
                assert!(matches!(error, TopologyError::ChangedDuringRead(_)));
            }
            assert!(bytes.is_empty());
            assert_eq!(bytes.capacity(), capacity);
        }
    }
}

#[test]
fn reused_link_reads_preserve_bounded_io_observation_order() {
    let fixture = Fixture::valid(1);
    let path = properties_path(&fixture);
    let good = fs::read(&path).unwrap();
    let mut bytes = vec![b'z'; 2048];
    for input in [good, vec![0xff], vec![b'x'; MAX_PROPERTY_BYTES + 1]] {
        fs::write(&path, &input).unwrap();
        let (fresh, original_trace) = host_diagnostics::capture(|| {
            read_text_prechecked(inspect_regular(&path)?, MAX_PROPERTY_BYTES)
        });
        let (reused, reused_trace) = host_diagnostics::capture(|| {
            link_properties::read(inspect_regular(&path)?, &mut bytes)
        });
        assert_eq!(original_trace, reused_trace);
        assert_eq!(
            fresh.as_ref().err().map(|e| format!("{e:?}")),
            reused.as_ref().err().map(|e| format!("{e:?}"))
        );
        let mut expected = vec![
            ("inspect", path.clone(), 0),
            ("open", path.clone(), 0),
            ("opened metadata", path.clone(), 0),
            ("bounded read", path.clone(), MAX_PROPERTY_BYTES + 1),
        ];
        if input.len() <= MAX_PROPERTY_BYTES {
            expected.push(("closing metadata", path.clone(), 0));
        }
        assert_eq!(reused_trace, expected);
    }
}

#[test]
fn reused_link_read_keeps_oversize_then_identity_then_utf8_error_precedence() {
    let fixture = Fixture::valid(1);
    let path = properties_path(&fixture);
    let scope = ObservationScope::new();
    let mut bytes = vec![b'z'; 2048];
    for input in [vec![0xff], vec![0xff; MAX_PROPERTY_BYTES + 1]] {
        fs::write(&path, &input).unwrap();
        scope.after_read(|path| fs::write(path, "changed\n").unwrap());
        let (fresh, original_trace) = host_diagnostics::capture(|| {
            read_text_prechecked(inspect_regular(&path)?, MAX_PROPERTY_BYTES)
        });
        fs::write(&path, &input).unwrap();
        scope.after_read(|path| fs::write(path, "changed\n").unwrap());
        let (reused, reused_trace) = host_diagnostics::capture(|| {
            link_properties::read(inspect_regular(&path)?, &mut bytes)
        });
        assert_eq!(
            format!("{:?}", fresh.unwrap_err()),
            format!("{:?}", reused.as_ref().unwrap_err())
        );
        assert_eq!(original_trace, reused_trace);
        if input.len() > MAX_PROPERTY_BYTES {
            assert!(matches!(reused, Err(TopologyError::FileTooLarge { .. })));
            assert_eq!(reused_trace.last().unwrap().0, "bounded read");
        } else {
            assert!(matches!(reused, Err(TopologyError::ChangedDuringRead(_))));
            assert_eq!(reused_trace.last().unwrap().0, "closing metadata");
        }
    }
}

#[test]
fn healthy_link_set_saves_one_text_allocation_per_subsequent_link() {
    let fixture = Fixture::valid(1);
    let path = properties_path(&fixture);
    // Warm up test-only thread-local hooks outside both measured regions.
    link_properties::read(inspect_regular(&path).unwrap(), &mut Vec::new()).unwrap();
    for count in [1, 2, 8, 64] {
        let (_, fresh) = allocation_counter::counted(|| {
            for _ in 0..count {
                std::hint::black_box(
                    link_properties::read(inspect_regular(&path).unwrap(), &mut Vec::new())
                        .unwrap(),
                );
            }
        });
        let (_, reused) = allocation_counter::counted(|| {
            let mut bytes = Vec::new();
            for _ in 0..count {
                std::hint::black_box(
                    link_properties::read(inspect_regular(&path).unwrap(), &mut bytes).unwrap(),
                );
            }
        });
        assert_eq!(fresh - reused, count - 1, "link count {count}");
    }
}
