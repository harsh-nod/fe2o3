//! Bounded task-owned edits. File publication alone is not source authority.
use super::*;

#[path = "source_bitselect_candidate_negatives_v1_tests.rs"]
mod generated_negatives;

const SOURCE_FILES: [&str; 10] = [
    "original.rs",
    "original-loader.rs",
    "candidate.rs",
    "candidate-loader.rs",
    "edited.rs",
    "edited-loader.rs",
    "wrong-output.rs",
    "wrong-output-loader.rs",
    "stale-edited.rs",
    "stale-edited-loader.rs",
];
const BYTE_CAP: usize = 128 * 1024;
const EDIT_CHILD: &str = "production_rustc_driver_v1::source_bitselect_feasibility_v1_tests::roundtrip::machine::fixture_cases::source_candidate_machine_edit_child";
const EDIT_PREFIX: &str = "FE2O3_SOURCE_CANDIDATE_MACHINE_EDIT ";
const FILE_CAP: u64 = SOURCE_FILES.len() as u64 * BYTE_CAP as u64;
const LOW: &str = "    scratch(4); out(5);\n    in(0) = a;\n    in(1) = b;\n    in(2) = mask;";
const HIGH: &str =
    "    scratch(32); out(33);\n    in(34) = a;\n    in(35) = b;\n    in(36) = mask;";
// Anchor the active macro's store, not the same store in an inactive cfg fixture.
const ORIGINAL_STORE: &str = "};\n    let index = thread::index_1d();\n    if let Some(slot) = output.get_mut(index) {\n        *slot = selected;";
const WRONG_STORE: &str = "};\n    let index = thread::index_1d();\n    if let Some(slot) = output.get_mut(index) {\n        *slot = a;";

#[derive(Default, Serialize)]
struct EditMeter {
    work: usize,
    payload: usize,
}
impl EditMeter {
    fn charge(&mut self, work: usize, payload: usize) -> Result<(), String> {
        let work = self
            .work
            .checked_add(work)
            .ok_or("candidate edit work overflow")?;
        let payload = self
            .payload
            .checked_add(payload)
            .ok_or("candidate edit payload overflow")?;
        if work > 32 * 1024 * 1024 || payload > 1024 * 1024 {
            return Err("candidate edit envelope exceeded".into());
        }
        self.work = work;
        self.payload = payload;
        Ok(())
    }
}

fn replace_exact(
    input: &[u8],
    before: &str,
    after: &str,
    meter: &mut EditMeter,
) -> Result<Vec<u8>, String> {
    if input.is_empty()
        || input.len() > BYTE_CAP
        || before.is_empty()
        || before.len() > 128
        || after.len() > 128
    {
        return Err("candidate edit byte/pattern cap".into());
    }
    let capacity = input
        .len()
        .checked_add(after.len())
        .ok_or("candidate edit capacity")?;
    if capacity > BYTE_CAP {
        return Err("candidate edit output cap".into());
    }
    // Prepay a conservative comparison scan plus exact output copy before work.
    let work = input
        .len()
        .checked_mul(before.len() + 2)
        .and_then(|value| value.checked_add(capacity))
        .ok_or("candidate edit work")?;
    meter.charge(work, capacity)?;
    let text = std::str::from_utf8(input).map_err(|_| "candidate edit UTF-8")?;
    let mut matches = text.match_indices(before);
    let (index, _) = matches
        .next()
        .ok_or("candidate edit exact pattern absent")?;
    if matches.next().is_some() {
        return Err("candidate edit pattern ambiguous".into());
    }
    let mut output = Vec::with_capacity(capacity);
    output.extend_from_slice(&input[..index]);
    output.extend_from_slice(after.as_bytes());
    output.extend_from_slice(&input[index + before.len()..]);
    Ok(output)
}

fn publish_variant(
    relative: &Path,
    from: &str,
    to: &str,
    meter: &mut EditMeter,
    edit: impl FnOnce(&[u8], &mut EditMeter) -> Result<Vec<u8>, String>,
) -> Result<Value, String> {
    let mut source = RetainedInput::open(relative.join(from).to_str().unwrap(), true)?;
    let bytes = edit(source.original(), meter)?;
    meter.charge(BYTE_CAP * 3, BYTE_CAP)?;
    source.publish(relative.join(to).to_str().unwrap(), &bytes)?;
    source.recheck()?;
    let actual = read_bounded(&relative.join(to), BYTE_CAP)?;
    if actual != bytes {
        return Err("candidate edit publication readback".into());
    }
    Ok(json!({
        "source":from,"destination":to,"bytes":bytes.len(),
        "sha256":<[u8;32]>::from(Sha256::digest(&bytes)),
        "io_envelope":source.io,
        "semantic_equivalence_claimed":false,"source_admission_reused":false,
    }))
}

#[test]
#[ignore = "bounded publication subprocess; use the actual machine ladder"]
fn source_candidate_machine_edit_child() {
    let directory = PathBuf::from(std::env::var_os(INPUT).expect("prepared output"));
    assert_eq!(std::env::current_dir().unwrap(), repository());
    require_current_source();
    let report = serde_json::to_vec(&prepare_edits(&directory)).unwrap();
    assert!(report.len() <= 64 * 1024);
    require_current_source();
    println!("\n{EDIT_PREFIX}{}", std::str::from_utf8(&report).unwrap());
}

pub(super) fn run_edits(directory: &Path) -> Value {
    // The retained-source API deliberately requires relative paths. Run its
    // publication in a separate repo-root child; never change the test process cwd.
    let mut command = Command::new(std::env::current_exe().unwrap());
    let bytes = checked(
        sanitized(&mut command)
            .current_dir(repository())
            .args(["--exact", EDIT_CHILD, "--ignored", "--nocapture"])
            .env(INPUT, directory),
        directory,
        "edit-publications",
        None,
    );
    let output = std::str::from_utf8(&bytes).unwrap();
    assert_eq!(
        output
            .lines()
            .filter(|line| line.starts_with("test result: ok. 1 passed; 0 failed; 0 ignored;"))
            .count(),
        1
    );
    let mut lines = output
        .lines()
        .filter_map(|line| line.strip_prefix(EDIT_PREFIX));
    let line = lines.next().expect("one publication observation");
    assert!(line.len() <= 64 * 1024 && lines.next().is_none());
    serde_json::from_str(line).unwrap()
}

fn prepare_edits(directory: &Path) -> Value {
    let relative = paths::relative_case(directory, "positive");
    let absolute = paths::absolute_case(directory, "positive");
    let mut meter = EditMeter::default();
    let edited = publish_variant(
        &relative,
        "candidate.rs",
        "edited.rs",
        &mut meter,
        |bytes, meter| replace_exact(bytes, LOW, HIGH, meter),
    )
    .unwrap();
    let wrong = publish_variant(
        &relative,
        "edited.rs",
        "wrong-output.rs",
        &mut meter,
        |bytes, meter| replace_exact(bytes, ORIGINAL_STORE, WRONG_STORE, meter),
    )
    .unwrap();
    let stale = publish_variant(
        &relative,
        "edited.rs",
        "stale-edited.rs",
        &mut meter,
        |bytes, meter| {
            meter.charge(bytes.len(), bytes.len())?;
            Ok(bytes.to_vec())
        },
    )
    .unwrap();
    for name in ["edited", "wrong-output", "stale-edited"] {
        let loader =
            format!("#![no_std]\n#[path = \"{name}.rs\"] mod source_bitselect_feasibility;\n");
        assert!(loader.len() <= 256);
        paths::write_new(
            &absolute.join(format!("{name}-loader.rs")),
            loader.as_bytes(),
        );
    }
    json!({"edits":[edited,wrong,stale],"edit_accounting":meter,
        "edit_work_limit":32*1024*1024,"edit_payload_limit":1024*1024,
        "no_original_or_candidate_overwrite":true})
}

pub(super) fn footprint(directory: &Path) -> (usize, u64) {
    let root = repository().join(paths::relative_root(directory));
    let mut roots = fs::read_dir(&root).unwrap();
    let only = roots.next().unwrap().unwrap();
    assert_eq!(only.file_name().to_str().unwrap(), "positive");
    assert!(roots.next().is_none());
    let case = paths::absolute_case(directory, "positive");
    let mut count = 0usize;
    let mut bytes = 0u64;
    for entry in fs::read_dir(&case).unwrap() {
        count += 1;
        assert!(count <= SOURCE_FILES.len());
        let entry = entry.unwrap();
        assert!(SOURCE_FILES.contains(&entry.file_name().to_str().unwrap()));
        paths::checked_source_file(&entry.path());
        bytes = bytes
            .checked_add(fs::symlink_metadata(entry.path()).unwrap().len())
            .unwrap();
        assert!(bytes <= FILE_CAP);
    }
    assert_eq!(count, SOURCE_FILES.len());
    (count, bytes)
}

#[test]
fn source_candidate_machine_edits_are_exact_and_budgeted() {
    let input = format!("prefix\n{LOW}\nsuffix");
    let mut meter = EditMeter::default();
    let edited = replace_exact(input.as_bytes(), LOW, HIGH, &mut meter).unwrap();
    assert_eq!(
        std::str::from_utf8(&edited).unwrap(),
        format!("prefix\n{HIGH}\nsuffix")
    );
    assert!(replace_exact(b"unrelated", LOW, HIGH, &mut meter).is_err());
    assert!(replace_exact(format!("{LOW}\n{LOW}").as_bytes(), LOW, HIGH, &mut meter).is_err());
    let mut full = EditMeter {
        work: 32 * 1024 * 1024,
        payload: 0,
    };
    assert!(full.charge(1, 1).is_err());
    assert_eq!((full.work, full.payload), (32 * 1024 * 1024, 0));
    let mut full = EditMeter {
        work: 0,
        payload: 1024 * 1024,
    };
    assert!(full.charge(1, 1).is_err());
    assert_eq!((full.work, full.payload), (0, 1024 * 1024));
    let mixed = format!("{ORIGINAL_STORE}\n}}\n// inactive item\n*slot = selected;");
    let wrong = replace_exact(
        mixed.as_bytes(),
        ORIGINAL_STORE,
        WRONG_STORE,
        &mut EditMeter::default(),
    )
    .unwrap();
    let wrong = std::str::from_utf8(&wrong).unwrap();
    assert!(wrong.starts_with(WRONG_STORE));
    assert!(wrong.ends_with("// inactive item\n*slot = selected;"));
}

#[test]
fn source_candidate_machine_negatives_are_distinct_named_boundaries() {
    assert_ne!(
        expected_refusal("wrong-output"),
        expected_refusal("wrong-plan")
    );
    assert!(
        expected_refusal("wrong-output")
            .unwrap()
            .contains("oracle output")
    );
    assert!(
        expected_refusal("stale-edited")
            .unwrap()
            .contains("parsed compiler input")
    );
    for name in ["default", "edited", "repeat"] {
        assert!(expected_refusal(name).is_none());
    }
}
