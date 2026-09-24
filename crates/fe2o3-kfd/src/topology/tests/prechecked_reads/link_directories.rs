use super::*;
use crate::topology::tests::host_diagnostics;

#[test]
fn every_discovery_checks_each_link_directory_once_and_reads_all_properties() {
    let fixture = Fixture::valid(3);
    let mut previous = None;
    for pass in 0..2 {
        let (result, events) = host_diagnostics::capture(|| fixture.discover());
        let snapshot = result.unwrap();
        if let Some(previous) = previous {
            assert_ne!(snapshot, previous);
        }
        for node in 1..=3 {
            for (set, count) in [("io_links", 3), ("p2p_links", 2)] {
                for index in 0..count {
                    let path = fixture.node(node).join(set).join(index.to_string());
                    let directory_events: Vec<_> = events
                        .iter()
                        .filter(|(_, observed, _)| *observed == path)
                        .map(|(operation, _, bound)| (*operation, *bound))
                        .collect();
                    // These hooks count Rust I/O boundaries, not kernel syscalls.
                    assert_eq!(directory_events, [("inspect", 0), ("read directory", 2)]);
                    let properties = path.join("properties");
                    let file_events: Vec<_> = events
                        .iter()
                        .filter(|(_, observed, _)| *observed == properties)
                        .map(|(operation, _, bound)| (*operation, *bound))
                        .collect();
                    assert_eq!(
                        file_events,
                        [
                            ("inspect", 0),
                            ("open", 0),
                            ("opened metadata", 0),
                            ("bounded read", MAX_PROPERTY_BYTES + 1),
                            ("closing metadata", 0),
                        ]
                    );
                }
            }
        }
        previous = Some(snapshot);
        if pass == 0 {
            let path = fixture.node(3).join("p2p_links/1/properties");
            let before = fs::read_to_string(&path).unwrap();
            assert!(before.contains("weight 15\n"));
            fs::write(path, before.replace("weight 15\n", "weight 16\n")).unwrap();
        }
    }
}

#[test]
fn single_directory_check_preserves_stable_contents_and_type_refusals() {
    for set in ["io_links", "p2p_links"] {
        for case in 0..6 {
            let fixture = Fixture::valid(2);
            let path = fixture.node(1).join(set).join("0");
            if case != 0 {
                fs::remove_dir_all(&path).unwrap();
                match case {
                    1 => fs::write(&path, "not a directory").unwrap(),
                    2 => mkfifoat(CWD, &path, Mode::RUSR | Mode::WUSR).unwrap(),
                    3 => symlink(fixture.node(2).join(set).join("0"), &path).unwrap(),
                    4 => symlink(fixture.root.join("missing"), &path).unwrap(),
                    5 => symlink(&path, &path).unwrap(),
                    _ => unreachable!(),
                }
            }
            let old = (|| {
                ensure_directory(&path)?;
                read_directory(&path, 2)
            })();
            let (new, events) = host_diagnostics::capture(|| read_directory(&path, 2));
            let normalize = |result: Result<Vec<DirectoryEntry>, TopologyError>| {
                result
                    .map(|entries| {
                        entries
                            .into_iter()
                            .map(|entry| entry.path)
                            .collect::<Vec<_>>()
                    })
                    .map_err(|error| format!("{error:?}"))
            };
            assert_eq!(normalize(old), normalize(new));
            assert_eq!(events[0], ("inspect", path.clone(), 0));
            assert_eq!(events.len(), if case == 0 { 2 } else { 1 });
            if case != 0 {
                let error = fixture.discover().unwrap_err();
                if case <= 2 {
                    assert!(matches!(error, TopologyError::UnexpectedFileType { .. }));
                } else {
                    assert!(matches!(error, TopologyError::Symlink(_)));
                }
            }
        }
    }
}

#[test]
fn listing_and_property_validation_remain_fail_closed() {
    for set in ["io_links", "p2p_links"] {
        for case in 0..4 {
            let fixture = Fixture::valid(2);
            let path = fixture.node(1).join(set).join("0");
            let properties = path.join("properties");
            match case {
                0 => fs::remove_file(&properties).unwrap(),
                1 => fs::write(path.join("extra"), "").unwrap(),
                2 => fs::write(&properties, "type 11\n").unwrap(),
                3 => fs::remove_dir_all(&path).unwrap(),
                _ => unreachable!(),
            }
            let error = fixture.discover().unwrap_err();
            match case {
                0 | 1 => assert!(matches!(error, TopologyError::UnexpectedLinkEntry(_))),
                2 => assert!(matches!(error, TopologyError::MissingProperty { .. })),
                3 => assert!(matches!(error, TopologyError::LinkCountMismatch { .. })),
                _ => unreachable!(),
            }
        }
    }
}
