//! Fixed real source fixtures; recipe files are separate inert authoring inputs.
use super::*;

pub(super) const CASES: [&str; 8] = [
    "positive",
    "renamed",
    "changed-operator",
    "ambiguous",
    "new-item",
    "stale-source",
    "wrong-target",
    "stale-recipe",
];

pub(super) fn source(case: &str) -> String {
    assert!(CASES.contains(&case));
    match case {
        "new-item" => fixture_cases::source("positive").replace("choose_order", "choose_order_new"),
        "stale-recipe" => fixture_cases::source("positive"),
        _ => fixture_cases::source(case),
    }
}

pub(super) fn refusal(case: &str, run: &str) -> Option<&'static str> {
    match (case, run) {
        ("new-item", "old") => Some(codec::BINDING_CHANGED),
        ("new-item", _) => None,
        ("stale-recipe", _) => Some("local-order recipe retained file changed"),
        _ => fixture_cases::refusal(case),
    }
}

pub(super) fn relative(directory: &Path, case: &str) -> PathBuf {
    assert!(CASES.contains(&case));
    paths::relative_root(directory).join(case).join("source.rs")
}

pub(super) fn absolute(directory: &Path, case: &str) -> PathBuf {
    let path = repository().join(relative(directory, case));
    paths::checked_source_file(&path);
    path
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Invocation {
    pub(super) case: String,
    pub(super) run: String,
    pub(super) recipe_name: Option<String>,
    pub(super) source_relative: PathBuf,
    pub(super) source_sha256: String,
    pub(super) recipe_sha256: Option<String>,
    pub(super) args: Vec<String>,
    pub(super) crate_binding: String,
    pub(super) cargo_observation: String,
    fixture_sha256: [String; 4],
    artifacts_sha256: String,
    metadata_sha256: String,
}
pub(super) fn derive(directory: &Path, case: &str, run: &str, recipe: Option<&str>) -> Invocation {
    assert!(CASES.contains(&case));
    assert!(matches!(
        run,
        "generate" | "source" | "reverse" | "repeat" | "old" | "regenerate"
    ));
    let source = absolute(directory, case);
    let (args, crate_binding, cargo_observation) = invocation_for_fixture_source(
        directory,
        &fixture(),
        PACKAGE,
        CRATE_NAME,
        Some(FEATURES[0]),
        &source,
        if case == "wrong-target" {
            "gfx950"
        } else {
            "gfx942"
        },
    );
    Invocation {
        case: case.into(),
        run: run.into(),
        recipe_name: recipe.map(str::to_owned),
        source_relative: relative(directory, case),
        source_sha256: hash(&source),
        recipe_sha256: recipe.map(|name| hash(&io::recipe_path(directory, name))),
        args,
        crate_binding,
        cargo_observation,
        fixture_sha256: FIXTURE_FILES.map(|(path, _)| hash(&fixture().join(path))),
        artifacts_sha256: hash(&directory.join("dependencies.stdout")),
        metadata_sha256: hash(&directory.join("metadata.stdout")),
    }
}
