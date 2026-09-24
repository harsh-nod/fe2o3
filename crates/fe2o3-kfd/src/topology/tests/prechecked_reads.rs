use super::*;
use rustix::fs::{CWD, Mode, OFlags, fcntl_getfl, mkfifoat};
use std::cell::RefCell;
use std::os::unix::fs::symlink;

mod link_scratch;

type ReadHook = Box<dyn FnOnce(&Path)>;

thread_local! {
    static INSPECTIONS: RefCell<Option<BTreeMap<PathBuf, usize>>> = const { RefCell::new(None) };
    static AFTER_READ: RefCell<Option<ReadHook>> = const { RefCell::new(None) };
}

pub(in crate::topology) fn record_inspect(path: &Path) {
    INSPECTIONS.with_borrow_mut(|active| {
        if let Some(counts) = active {
            *counts.entry(path.to_path_buf()).or_default() += 1;
        }
    });
}

pub(in crate::topology) fn after_regular_read(path: &Path) {
    let hook = AFTER_READ.with_borrow_mut(Option::take);
    if let Some(hook) = hook {
        hook(path);
    }
}

struct ObservationScope;

impl ObservationScope {
    fn new() -> Self {
        INSPECTIONS.with_borrow_mut(|active| {
            assert!(active.is_none());
            *active = Some(BTreeMap::new());
        });
        assert!(AFTER_READ.with_borrow(Option::is_none));
        Self
    }

    fn count(&self, path: &Path) -> usize {
        INSPECTIONS.with_borrow(|active| active.as_ref().unwrap().get(path).copied().unwrap_or(0))
    }

    fn after_read(&self, hook: impl FnOnce(&Path) + 'static) {
        AFTER_READ.with_borrow_mut(|active| {
            assert!(active.is_none());
            *active = Some(Box::new(hook));
        });
    }
}

impl Drop for ObservationScope {
    fn drop(&mut self) {
        INSPECTIONS.with_borrow_mut(|active| *active = None);
        AFTER_READ.with_borrow_mut(|active| *active = None);
    }
}

#[test]
fn stable_regular_reads_consume_one_path_inspection() {
    let fixture = Fixture::valid(1);
    let path = fixture.root.join("generation_id");
    let scope = ObservationScope::new();
    assert_eq!(read_text(&path, 64).unwrap(), "7\n");
    assert_eq!(scope.count(&path), 1);
    let before = inspect_regular(&path).unwrap();
    assert_eq!(scope.count(&path), 2);
    assert_eq!(read_text_prechecked(before, 64).unwrap(), "7\n");
    assert_eq!(scope.count(&path), 2);
}

#[test]
fn each_discovery_inspects_each_link_property_once_and_stays_fresh() {
    let fixture = Fixture::valid(3);
    let scope = ObservationScope::new();
    let first = fixture.discover().unwrap();
    let mut properties = Vec::new();
    for node in 1..=3 {
        for (set, count) in [("io_links", 3), ("p2p_links", 2)] {
            for index in 0..count {
                let path = fixture
                    .node(node)
                    .join(set)
                    .join(index.to_string())
                    .join("properties");
                assert_eq!(scope.count(&path), 1, "{}", path.display());
                properties.push(path);
            }
        }
    }
    assert_eq!(properties.len(), 15);
    let path = &properties[0];
    let previous = fs::read_to_string(path).unwrap();
    fs::write(path, previous.replace("weight 15\n", "weight 16\n")).unwrap();
    let peer_path = fixture.node(1).join("p2p_links/0/properties");
    let previous = fs::read_to_string(&peer_path).unwrap();
    fs::write(&peer_path, previous.replace("weight 15\n", "weight 17\n")).unwrap();
    let second = fixture.discover().unwrap();
    assert_ne!(first, second);
    assert_eq!(second.gpu_nodes[0].io_links[0].weight, 16);
    assert_eq!(second.gpu_nodes[0].p2p_links[0].weight, 17);
    for path in properties {
        assert_eq!(scope.count(&path), 2, "{}", path.display());
    }
}

#[test]
fn replaced_regular_file_is_rejected_before_read() {
    let fixture = Fixture::valid(1);
    let path = fixture.root.join("generation_id");
    let before = inspect_regular(&path).unwrap();
    let replacement = fixture.root.join("replacement");
    fs::write(&replacement, "8\n").unwrap();
    fs::rename(replacement, &path).unwrap();
    assert!(
        matches!(read_text_prechecked(before, 64), Err(TopologyError::ChangedDuringRead(p)) if p == path)
    );
}

#[test]
fn final_symlink_to_same_original_inode_is_not_followed() {
    let fixture = Fixture::valid(1);
    let directory = fixture.root.join("observed");
    fs::create_dir(&directory).unwrap();
    let path = directory.join("value");
    fs::write(&path, "7\n").unwrap();
    let before = inspect_regular(&path).unwrap();
    let original = fixture.root.join("original");
    fs::rename(&directory, &original).unwrap();
    fs::create_dir(&directory).unwrap();
    symlink(original.join("value"), &path).unwrap();
    assert_eq!(
        FileIdentity::from_metadata(&fs::metadata(&path).unwrap()),
        before.identity
    );
    assert!(
        matches!(read_text_prechecked(before, 64), Err(TopologyError::Symlink(p)) if p == path)
    );
}

#[test]
fn disappeared_regular_file_preserves_open_error() {
    let fixture = Fixture::valid(1);
    let path = fixture.root.join("generation_id");
    let before = inspect_regular(&path).unwrap();
    fs::remove_file(&path).unwrap();
    assert!(
        matches!(read_text_prechecked(before, 64), Err(TopologyError::Io { operation: "open", path: p, source }) if p == path && source.kind() == io::ErrorKind::NotFound)
    );
    assert!(
        matches!(read_text(&path, 64), Err(TopologyError::Io { operation: "inspect", path: p, source }) if p == path && source.kind() == io::ErrorKind::NotFound)
    );
}

#[test]
fn initial_symlink_directory_and_fifo_are_rejected_before_open() {
    let fixture = Fixture::valid(1);
    let path = fixture.root.join("unexpected");
    symlink(fixture.root.join("generation_id"), &path).unwrap();
    assert!(matches!(inspect_regular(&path), Err(TopologyError::Symlink(p)) if p == path));
    fs::remove_file(&path).unwrap();
    fs::create_dir(&path).unwrap();
    assert!(
        matches!(inspect_regular(&path), Err(TopologyError::UnexpectedFileType { path: p, expected: "regular file" }) if p == path)
    );
    fs::remove_dir(&path).unwrap();
    mkfifoat(CWD, &path, Mode::RUSR | Mode::WUSR).unwrap();
    assert!(
        matches!(inspect_regular(&path), Err(TopologyError::UnexpectedFileType { path: p, expected: "regular file" }) if p == path)
    );
}

#[test]
fn replacement_fifo_cannot_block_the_open() {
    let fixture = Fixture::valid(1);
    let path = fixture.root.join("generation_id");
    let flags = fcntl_getfl(open_observed_regular(&path).unwrap()).unwrap();
    assert!(flags.contains(OFlags::NONBLOCK));
    let before = inspect_regular(&path).unwrap();
    fs::remove_file(&path).unwrap();
    mkfifoat(CWD, &path, Mode::RUSR | Mode::WUSR).unwrap();
    assert!(
        matches!(read_text_prechecked(before, 64), Err(TopologyError::ChangedDuringRead(p)) if p == path)
    );
}

#[test]
fn intermediate_symlink_loop_is_classified_as_symlink() {
    let fixture = Fixture::valid(1);
    let directory = fixture.root.join("observed");
    fs::create_dir(&directory).unwrap();
    let path = directory.join("value");
    fs::write(&path, "7\n").unwrap();
    let before = inspect_regular(&path).unwrap();
    fs::remove_dir_all(&directory).unwrap();
    symlink(&directory, &directory).unwrap();
    assert!(
        matches!(read_text_prechecked(before, 64), Err(TopologyError::Symlink(p)) if p == path)
    );
}

#[test]
fn post_read_identity_mismatch_still_precedes_utf8_validation() {
    let fixture = Fixture::valid(1);
    let path = fixture.root.join("generation_id");
    fs::write(&path, [0xff]).unwrap();
    let scope = ObservationScope::new();
    scope.after_read(|path| fs::write(path, "changed\n").unwrap());
    assert!(matches!(read_text(&path, 64), Err(TopologyError::ChangedDuringRead(p)) if p == path));
}

#[test]
fn oversized_read_still_precedes_post_read_identity_check() {
    let fixture = Fixture::valid(1);
    let path = fixture.root.join("generation_id");
    fs::write(&path, "too long\n").unwrap();
    let scope = ObservationScope::new();
    scope.after_read(|path| fs::write(path, "changed and longer\n").unwrap());
    assert!(
        matches!(read_text(&path, 1), Err(TopologyError::FileTooLarge { path: p, maximum: 1 }) if p == path)
    );
}

#[test]
fn prechecked_text_keeps_byte_limits_and_utf8_errors() {
    let fixture = Fixture::valid(1);
    let path = fixture.root.join("generation_id");
    for bytes in [vec![], vec![b'x'], vec![0xff], vec![b'x'; 65]] {
        fs::write(&path, bytes).unwrap();
        let ordinary = read_text(&path, 64);
        let prechecked = read_text_prechecked(inspect_regular(&path).unwrap(), 64);
        assert_eq!(format!("{ordinary:?}"), format!("{prechecked:?}"));
    }
}

#[test]
fn named_property_parser_results_and_error_order_stay_identical() {
    let fixture = Fixture::valid(1);
    let path = fixture.root.join("system_properties");
    let allowed = [("a", 0, 10), ("b", 0, 10)];
    for (text, expected) in [
        ("a 1\nb 2\n", "Ok("),
        ("a 1\nb 2", "MalformedPropertyLine"),
        ("a 01\nb 2\n", "MalformedPropertyLine"),
        ("a 1\na 2\n", "DuplicateProperty"),
        ("a 1\n", "MissingProperty"),
        ("a 11\nb 2\n", "PropertyOutOfRange"),
        ("unknown 1\nb 2\n", "UnknownProperty"),
        ("a 1\nb 2\nextra 3\n", "TooManyProperties"),
    ] {
        fs::write(&path, text).unwrap();
        let ordinary = parse_named_properties(&path, &allowed);
        let prechecked =
            parse_named_properties_prechecked(inspect_regular(&path).unwrap(), &allowed);
        assert_eq!(format!("{ordinary:?}"), format!("{prechecked:?}"));
        assert!(format!("{ordinary:?}").contains(expected));
    }
}

#[test]
fn same_inode_changes_before_open_precede_read_and_size_errors() {
    let fixture = Fixture::valid(1);
    let path = fixture.root.join("generation_id");
    for replacement in ["", "far too long for the read bound\n"] {
        fs::write(&path, "7\n").unwrap();
        let before = inspect_regular(&path).unwrap();
        fs::write(&path, replacement).unwrap();
        assert_eq!(fs::metadata(&path).unwrap().ino(), before.identity.inode);
        assert!(
            matches!(read_text_prechecked(before, 1), Err(TopologyError::ChangedDuringRead(p)) if p == path)
        );
    }
}

#[test]
fn link_discovery_preserves_type_size_and_utf8_rejection() {
    for set in ["io_links", "p2p_links"] {
        for case in ["symlink", "directory", "fifo", "oversize", "utf8"] {
            let fixture = Fixture::valid(2);
            let path = fixture.node(1).join(set).join("0/properties");
            fs::remove_file(&path).unwrap();
            match case {
                "symlink" => symlink(fixture.root.join("generation_id"), &path).unwrap(),
                "directory" => fs::create_dir(&path).unwrap(),
                "fifo" => mkfifoat(CWD, &path, Mode::RUSR | Mode::WUSR).unwrap(),
                "oversize" => fs::write(&path, vec![b'x'; MAX_PROPERTY_BYTES + 1]).unwrap(),
                "utf8" => fs::write(&path, [0xff]).unwrap(),
                _ => unreachable!(),
            }
            let error = fixture.discover().unwrap_err();
            match case {
                "symlink" => assert!(matches!(error, TopologyError::Symlink(p) if p == path)),
                "directory" | "fifo" => assert!(
                    matches!(error, TopologyError::UnexpectedFileType { path: p, expected: "regular file" } if p == path)
                ),
                "oversize" => assert!(
                    matches!(error, TopologyError::FileTooLarge { path: p, maximum: MAX_PROPERTY_BYTES } if p == path)
                ),
                "utf8" => assert!(matches!(error, TopologyError::InvalidUtf8(p) if p == path)),
                _ => unreachable!(),
            }
        }
    }
}
