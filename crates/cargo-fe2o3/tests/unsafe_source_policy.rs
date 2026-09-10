#![forbid(unsafe_code)]

use proc_macro2::{Delimiter, TokenStream, TokenTree};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

type Counts = BTreeMap<String, usize>;
type Inventory = BTreeMap<String, Counts>;

fn is_ident(token: Option<&TokenTree>, expected: &str) -> bool {
    matches!(token, Some(TokenTree::Ident(ident)) if ident == expected)
}

fn unsafe_kind(tokens: &[TokenTree]) -> &'static str {
    match tokens.first() {
        Some(TokenTree::Group(group)) if group.delimiter() == Delimiter::Brace => "block",
        Some(TokenTree::Group(group)) if group.delimiter() == Delimiter::Parenthesis => "attribute",
        token if is_ident(token, "fn") => "function",
        token if is_ident(token, "impl") => "impl",
        token if is_ident(token, "trait") || is_ident(token, "auto") => "trait",
        token if is_ident(token, "extern") => {
            if tokens
                .iter()
                .skip(1)
                .take(2)
                .any(|token| is_ident(Some(token), "fn"))
            {
                "function"
            } else {
                "extern_block"
            }
        }
        _ => "other",
    }
}

fn count_tokens(stream: TokenStream, counts: &mut Counts) {
    let tokens: Vec<_> = stream.into_iter().collect();
    for (index, token) in tokens.iter().enumerate() {
        if is_ident(Some(token), "unsafe") {
            *counts
                .entry(unsafe_kind(&tokens[index + 1..]).into())
                .or_default() += 1;
        }
        if let TokenTree::Group(group) = token {
            count_tokens(group.stream(), counts);
        }
    }
}

fn count_source(source: &str) -> Result<Counts, proc_macro2::LexError> {
    let mut counts = Counts::new();
    count_tokens(source.parse()?, &mut counts);
    Ok(counts)
}

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn inventory(root: &Path) -> Inventory {
    // Include untracked source while developing, but not ignored build output.
    let output = Command::new("git")
        .args([
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
            "--",
            "*.rs",
        ])
        .current_dir(root)
        .output()
        .expect("list repository Rust sources");
    assert!(
        output.status.success(),
        "git ls-files failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let paths: BTreeSet<_> = output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| std::str::from_utf8(path).expect("UTF-8 repository source path"))
        .collect();
    assert!(!paths.is_empty(), "repository contains no Rust sources");
    let mut result = Inventory::new();
    for path in paths {
        let source = fs::read_to_string(root.join(path))
            .unwrap_or_else(|error| panic!("cannot read {path}: {error}"));
        let counts =
            count_source(&source).unwrap_or_else(|error| panic!("cannot tokenize {path}: {error}"));
        if !counts.is_empty() {
            result.insert(path.to_owned(), counts);
        }
    }
    result
}

fn differences(expected: &Inventory, actual: &Inventory) -> Vec<String> {
    let paths: BTreeSet<_> = expected.keys().chain(actual.keys()).collect();
    paths
        .into_iter()
        .filter_map(|path| {
            let before = expected.get(path);
            let after = actual.get(path);
            (before != after).then(|| format!("{path}: reviewed={before:?}, observed={after:?}"))
        })
        .collect()
}

#[test]
fn unsafe_source_matches_reviewed_inventory() {
    let root = workspace();
    let expected: Inventory = serde_json::from_slice(
        &fs::read(root.join("scripts/unsafe-source-baseline.json"))
            .expect("read reviewed unsafe source baseline"),
    )
    .expect("parse reviewed unsafe source baseline");
    let changed = differences(&expected, &inventory(&root));
    assert!(
        changed.is_empty(),
        "unsafe source inventory changed:\n{}\nReview each change and refresh the baseline as documented in docs/unsafe-code-policy.md",
        changed.join("\n")
    );
}

#[test]
#[ignore = "explicit maintenance command; review the resulting baseline diff"]
fn refresh_reviewed_unsafe_inventory() {
    let root = workspace();
    let observed = inventory(&root);
    let mut bytes = serde_json::to_vec_pretty(&observed).expect("encode source inventory");
    bytes.push(b'\n');
    fs::write(root.join("scripts/unsafe-source-baseline.json"), bytes)
        .expect("write reviewed unsafe source baseline");
    eprintln!(
        "{} unsafe sites in {} files",
        observed.values().flat_map(Counts::values).sum::<usize>(),
        observed.len()
    );
}

#[test]
fn comments_literals_and_documentation_do_not_count() {
    let source = r##"
        // unsafe { ignored() }
        /* unsafe trait Ignored {} /* nested comment */ */
        /// unsafe fn ignored() {}
        fn example() {
            let _ = "unsafe { ignored() }";
            let _ = r#"unsafe fn ignored() {}"#;
            let _ = b"unsafe";
            let _ = 'u';
            unsafe { actual() }
        }
    "##;
    assert_eq!(
        count_source(source).unwrap(),
        Counts::from([("block".into(), 1)])
    );
}

#[test]
fn all_constructs_and_macro_templates_are_counted() {
    let source = r#"
        unsafe trait Contract {}
        unsafe impl Contract for u8 {}
        unsafe extern "C" { fn imported(); }
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn exported() { unsafe { imported() } }
        macro_rules! template { () => { unsafe fn generated() {} }; }
        fn generate() { quote! { unsafe { generated() } }; }
        #[cfg(test)] fn test_helper() { unsafe { imported() } }
        #[cfg(any())] unsafe fn disabled() {}
    "#;
    assert_eq!(
        count_source(source).unwrap(),
        Counts::from([
            ("attribute".into(), 1),
            ("block".into(), 3),
            ("extern_block".into(), 1),
            ("function".into(), 3),
            ("impl".into(), 1),
            ("trait".into(), 1),
        ])
    );
}

#[test]
fn malformed_tokens_fail_closed() {
    assert!(count_source("fn incomplete() { unsafe {").is_err());
}

#[test]
fn additions_removals_and_moves_require_baseline_review() {
    let one = Counts::from([("block".into(), 1)]);
    let reviewed = Inventory::from([("first.rs".into(), one.clone())]);
    assert!(differences(&reviewed, &reviewed).is_empty());
    assert_eq!(differences(&Inventory::new(), &reviewed).len(), 1);
    assert_eq!(differences(&reviewed, &Inventory::new()).len(), 1);
    assert_eq!(
        differences(&reviewed, &Inventory::from([("second.rs".into(), one)])).len(),
        2
    );
    let changed_kind =
        Inventory::from([("first.rs".into(), Counts::from([("function".into(), 1)]))]);
    assert_eq!(differences(&reviewed, &changed_kind).len(), 1);
}
