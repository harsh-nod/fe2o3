//! Read-only exact-context review of the staged apply_patch files.
use std::{
    collections::BTreeMap,
    env, fs,
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
};

fn main() {
    let mut args = env::args().skip(1);
    let root = PathBuf::from(args.next().expect("repository"));
    let rustfmt = args.next().expect("pinned rustfmt");
    let mut files = BTreeMap::<String, String>::new();
    let mut failures = Vec::new();
    for patch in args {
        let input = fs::read_to_string(&patch).expect("read draft");
        let lines: Vec<_> = input.lines().collect();
        assert_eq!(lines.first(), Some(&"*** Begin Patch"));
        let mut line = 1;
        while line < lines.len() && lines[line] != "*** End Patch" {
            let Some(path) = lines[line].strip_prefix("*** Update File: ") else {
                panic!("{patch}:{} expected update header", line + 1);
            };
            line += 1;
            let source = files
                .entry(path.to_owned())
                .or_insert_with(|| fs::read_to_string(root.join(path)).expect("read target"));
            let mut cursor = 0;
            while line < lines.len() && lines[line].starts_with("@@") {
                let hunk_line = line + 1;
                assert_eq!(
                    lines[line], "@@",
                    "review supports exact-context hunks only"
                );
                line += 1;
                let mut before = String::new();
                let mut after = String::new();
                while line < lines.len()
                    && !lines[line].starts_with("@@")
                    && !lines[line].starts_with("*** ")
                {
                    let (tag, text) = lines[line].split_at(1);
                    match tag {
                        " " => {
                            before.push_str(text);
                            before.push('\n');
                            after.push_str(text);
                            after.push('\n');
                        }
                        "-" => {
                            before.push_str(text);
                            before.push('\n');
                        }
                        "+" => {
                            after.push_str(text);
                            after.push('\n');
                        }
                        _ => panic!("{patch}:{} invalid hunk prefix", line + 1),
                    }
                    line += 1;
                }
                if before.is_empty() {
                    failures.push(format!("{patch}:{hunk_line} empty anchor"));
                    continue;
                }
                let matches: Vec<_> = source[cursor..]
                    .match_indices(&before)
                    .map(|(i, _)| cursor + i)
                    .collect();
                if let [at] = matches.as_slice() {
                    source.replace_range(*at..*at + before.len(), &after);
                    cursor = *at + after.len();
                } else {
                    failures.push(format!(
                        "{patch}:{hunk_line} {path}: {} forward matches for {:?}",
                        matches.len(),
                        before
                    ));
                }
            }
        }
    }
    if !failures.is_empty() {
        for failure in failures {
            eprintln!("{failure}");
        }
        std::process::exit(1);
    }
    for (path, source) in files {
        let mut child = Command::new(&rustfmt)
            .args([
                "--edition",
                "2024",
                "--emit",
                "stdout",
                "--config",
                "skip_children=true",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("start rustfmt");
        child
            .stdin
            .take()
            .unwrap()
            .write_all(source.as_bytes())
            .expect("feed in-memory candidate");
        let output = child.wait_with_output().expect("finish syntax check");
        if !output.status.success() {
            eprintln!(
                "candidate syntax {path}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            std::process::exit(1);
        }
        println!("exact-context and syntax checked, NOT mounted: {path}");
    }
}
