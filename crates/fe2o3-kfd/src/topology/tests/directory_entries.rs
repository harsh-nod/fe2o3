use super::*;
use std::ffi::OsString;
use std::os::unix::{ffi::OsStringExt, fs::symlink};

// Frozen tuple-based reader from 87bfbc0f for observation/error differentials.
fn legacy_read_directory(
    path: &Path,
    maximum: usize,
) -> Result<Vec<(String, PathBuf)>, TopologyError> {
    ensure_directory(path)?;
    host_diagnostics::io("read directory", path, maximum);
    let entries = fs::read_dir(path).map_err(|source| io_error("list", path, source))?;
    let mut result = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|source| io_error("list entry in", path, source))?;
        if result.len() == maximum {
            return Err(TopologyError::TooManyEntries {
                path: path.to_path_buf(),
                maximum,
            });
        }
        let entry_path = entry.path();
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| TopologyError::InvalidEntryName(entry_path.clone()))?;
        result.push((name, entry_path));
    }
    result.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(result)
}

fn normalized(
    result: Result<Vec<DirectoryEntry>, TopologyError>,
) -> Result<Vec<(String, PathBuf)>, String> {
    result
        .map(|entries| {
            entries
                .into_iter()
                .map(|entry| (entry.name().to_owned(), entry.path))
                .collect()
        })
        .map_err(|error| format!("{error:?}"))
}

fn assert_reader_equivalent(path: &Path, maximum: usize) {
    let (legacy, legacy_trace) = host_diagnostics::capture(|| legacy_read_directory(path, maximum));
    let (current, current_trace) = host_diagnostics::capture(|| read_directory(path, maximum));
    assert_eq!(normalized(current), legacy.map_err(|e| format!("{e:?}")));
    assert_eq!(current_trace, legacy_trace);
}

#[test]
fn directory_reader_preserves_names_paths_bounds_and_lexicographic_order() {
    let fixture = Fixture::valid(1);
    let directory = fixture.root.join("entries");
    fs::create_dir(&directory).unwrap();
    assert_reader_equivalent(&directory, 0);
    let names = ["0", "1", "10", "2", "a", "\u{e9}", "\u{4e00}", "\u{1f642}"];
    for (index, name) in names.iter().enumerate() {
        fs::write(directory.join(name), "").unwrap();
        for maximum in 0..=index + 2 {
            assert_reader_equivalent(&directory, maximum);
        }
    }
    let entries = read_directory(&directory, names.len()).unwrap();
    assert_eq!(
        entries.iter().map(DirectoryEntry::name).collect::<Vec<_>>(),
        names
    );
    for entry in entries {
        let path_bytes = entry.path.as_os_str().as_encoded_bytes();
        let name = entry.name().as_bytes();
        assert_eq!(
            name.as_ptr(),
            path_bytes[path_bytes.len() - name.len()..].as_ptr()
        );
    }
    assert_eq!(size_of::<DirectoryEntry>(), size_of::<PathBuf>());

    for index in 0..64 {
        fs::write(
            directory.join(format!("{index:02}-{}", "\u{e9}".repeat(80))),
            "",
        )
        .unwrap();
    }
    assert_reader_equivalent(&directory, names.len() + 64);
}

#[test]
fn directory_reader_preserves_invalid_basename_and_non_utf8_parent_behavior() {
    let fixture = Fixture::valid(1);
    let directory = fixture.root.join(OsString::from_vec(vec![b'p', 0xff]));
    fs::create_dir(&directory).unwrap();
    fs::write(directory.join("valid"), "").unwrap();
    assert_reader_equivalent(&directory, 1);
    assert_eq!(read_directory(&directory, 1).unwrap()[0].name(), "valid");
    fs::remove_file(directory.join("valid")).unwrap();
    let invalid = directory.join(OsString::from_vec(vec![0xff]));
    fs::write(&invalid, "").unwrap();
    for maximum in 0..=2 {
        assert_reader_equivalent(&directory, maximum);
    }
    assert!(matches!(
        read_directory(&directory, 0),
        Err(TopologyError::TooManyEntries { maximum: 0, .. })
    ));
    assert!(
        matches!(read_directory(&directory, 1), Err(TopologyError::InvalidEntryName(path)) if path == invalid)
    );
}

#[test]
fn directory_reader_preserves_initial_file_type_and_absence_errors() {
    let fixture = Fixture::valid(1);
    let link = fixture.root.join("symlink");
    symlink(fixture.root.join("nodes"), &link).unwrap();
    for path in [
        link,
        fixture.root.join("generation_id"),
        fixture.root.join("absent"),
    ] {
        assert_reader_equivalent(&path, 0);
        assert_reader_equivalent(&path, 16);
    }
}

#[test]
fn membership_checks_preserve_unknown_entry_type_precedence_and_trace() {
    let fixture = Fixture::valid(1);
    let unexpected = fixture.root.join("a-unknown");
    symlink(fixture.root.join("generation_id"), &unexpected).unwrap();
    let (result, trace) = host_diagnostics::capture(|| validate_root(&fixture.root));
    assert!(
        matches!(result, Err(TopologyError::UnexpectedEntry { path, name }) if path == fixture.root && name == "a-unknown")
    );
    assert_eq!(
        trace,
        vec![
            ("inspect", fixture.root.clone(), 0),
            ("read directory", fixture.root.clone(), MAX_ROOT_ENTRIES),
        ]
    );
    fs::remove_file(&unexpected).unwrap();
    let node = fixture.node(1);
    let unexpected = node.join("a-unknown");
    symlink(node.join("gpu_id"), &unexpected).unwrap();
    let (result, trace) = host_diagnostics::capture(|| validate_node_entries(&node));
    assert!(matches!(result, Err(TopologyError::Symlink(path)) if path == unexpected));
    assert_eq!(
        trace,
        vec![
            ("inspect", node.clone(), 0),
            ("read directory", node.clone(), MAX_NODE_ENTRIES),
            ("inspect", unexpected.clone(), 0),
        ]
    );
    fs::remove_file(&unexpected).unwrap();
    fs::write(&unexpected, "").unwrap();
    assert!(
        matches!(validate_node_entries(&node), Err(TopologyError::UnexpectedEntry { path, name }) if path == node && name == "a-unknown")
    );
}

#[test]
fn membership_checks_keep_missing_and_expected_type_errors() {
    let fixture = Fixture::valid(1);
    fs::remove_file(fixture.root.join("generation_id")).unwrap();
    assert!(
        matches!(fixture.discover(), Err(TopologyError::MissingEntry(path)) if path == fixture.root.join("generation_id"))
    );
    fs::create_dir(fixture.root.join("generation_id")).unwrap();
    assert!(
        matches!(fixture.discover(), Err(TopologyError::UnexpectedFileType { path, expected: "regular file" }) if path == fixture.root.join("generation_id"))
    );
    fs::remove_dir(fixture.root.join("generation_id")).unwrap();
    fs::write(fixture.root.join("generation_id"), "7\n").unwrap();
    let node = fixture.node(1);
    fs::remove_file(node.join("gpu_id")).unwrap();
    assert!(
        matches!(fixture.discover(), Err(TopologyError::MissingEntry(path)) if path == node.join("gpu_id"))
    );
    fs::create_dir(node.join("gpu_id")).unwrap();
    assert!(
        matches!(fixture.discover(), Err(TopologyError::UnexpectedFileType { path, expected: "regular file" }) if path == node.join("gpu_id"))
    );
    fs::remove_dir(node.join("gpu_id")).unwrap();
    fs::write(node.join("gpu_id"), "1001\n").unwrap();
    fs::remove_dir(node.join("caches")).unwrap();
    fs::write(node.join("caches"), "").unwrap();
    assert!(
        matches!(fixture.discover(), Err(TopologyError::UnexpectedFileType { path, expected: "directory" }) if path == node.join("caches"))
    );
}

#[test]
fn membership_checks_preserve_first_missing_field_order() {
    let fixture = Fixture::valid(1);
    fs::remove_file(fixture.root.join("generation_id")).unwrap();
    fs::remove_file(fixture.root.join("system_properties")).unwrap();
    assert!(
        matches!(validate_root(&fixture.root), Err(TopologyError::MissingEntry(path)) if path == fixture.root.join("generation_id"))
    );
    let node = fixture.node(1);
    for name in ["gpu_id", "name", "properties"] {
        fs::remove_file(node.join(name)).unwrap();
    }
    for (name, next) in [("gpu_id", "name"), ("name", "properties")] {
        assert!(
            matches!(validate_node_entries(&node), Err(TopologyError::MissingEntry(path)) if path == node.join(name))
        );
        fs::write(node.join(name), "").unwrap();
        assert!(
            matches!(validate_node_entries(&node), Err(TopologyError::MissingEntry(path)) if path == node.join(next))
        );
    }
}

#[test]
fn links_keep_count_index_and_content_error_precedence() {
    let fixture = Fixture::valid(1);
    let node = fixture.node(1);
    let links = node.join("io_links");
    fs::rename(links.join("0"), links.join("10")).unwrap();
    assert!(matches!(
        parse_topology_links(&node, 1, KfdTopologyLinkSetV1::Io, 2),
        Err(TopologyError::LinkCountMismatch {
            declared: 2,
            observed: 1,
            ..
        })
    ));
    assert!(
        matches!(fixture.discover(), Err(TopologyError::NonCanonicalLinkIndex { path, index: 10 }) if path == links.join("10"))
    );
    fs::rename(links.join("10"), links.join("0")).unwrap();
    fs::write(links.join("0/extra"), "").unwrap();
    fs::remove_file(links.join("0/properties")).unwrap();
    assert!(
        matches!(fixture.discover(), Err(TopologyError::UnexpectedLinkEntry(path)) if path == links.join("0"))
    );
    fs::write(links.join("0/second"), "").unwrap();
    fs::write(links.join("0/third"), "").unwrap();
    assert!(
        matches!(fixture.discover(), Err(TopologyError::TooManyEntries { path, maximum: 2 }) if path == links.join("0"))
    );

    let fixture = Fixture::valid(11);
    // Numeric sorting would accept this list; the existing lexicographic rule does not.
    assert!(matches!(
        fixture.discover(),
        Err(TopologyError::NonCanonicalLinkIndex { index: 10, .. })
    ));
}

#[test]
fn fresh_discovery_still_observes_membership_and_link_changes() {
    let fixture = Fixture::valid(3);
    let first = fixture.discover().unwrap();
    assert_eq!(first, fixture.discover().unwrap());
    let unexpected = fixture.node(2).join("unexpected");
    fs::write(&unexpected, "").unwrap();
    assert!(
        matches!(fixture.discover(), Err(TopologyError::UnexpectedEntry { name, .. }) if name == "unexpected")
    );
    fs::remove_file(unexpected).unwrap();
    assert_eq!(first, fixture.discover().unwrap());
    let path = fixture.node(3).join("p2p_links/0/properties");
    let original = fs::read_to_string(&path).unwrap();
    fs::write(&path, original.replace("weight 15\n", "weight 16\n")).unwrap();
    assert_ne!(first, fixture.discover().unwrap());
}

pub(super) mod allocation_counter;

#[test]
fn directory_reader_removes_per_entry_filename_allocations() {
    let fixture = Fixture::valid(1);
    for count in [0, 1, 3, 8, 17, 64] {
        let directory = fixture.root.join(format!("entries-{count}"));
        fs::create_dir(&directory).unwrap();
        for index in 0..count {
            fs::write(
                directory.join(format!("{index:02}-{}", "\u{e9}".repeat(80))),
                "",
            )
            .unwrap();
        }
        let (legacy, before) =
            allocation_counter::counted(|| legacy_read_directory(&directory, count));
        let (current, after) = allocation_counter::counted(|| read_directory(&directory, count));
        assert_eq!(normalized(current), legacy.map_err(|e| format!("{e:?}")));
        assert!(
            after + count <= before,
            "entries={count} legacy={before} path_only={after}"
        );
    }
}
