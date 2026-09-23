//! Fixed actual Rust edits and exact 25-attempt public-adapter census.
use super::*;

pub(super) const SOURCES: [&str; 12] = [
    "positive",
    "renamed",
    "new-item",
    "changed-operator",
    "local-alias",
    "ambiguous",
    "wrong-launch",
    "wrong-target",
    "stale-source",
    "probe-source-change",
    "probe-reentry",
    "probe-fatal",
];
pub(super) const RECIPE_NAMES: [&str; 7] = [
    "source-exact",
    "reverse-exact",
    "source-rebind",
    "reverse-rebind",
    "new-source",
    "new-reverse",
    "advisory-source",
];

#[derive(Clone, Copy, Debug)]
pub(super) enum Mode {
    Create {
        order: Order,
        relation: Relation,
        strength: Strength,
        binding: BindingMode,
        saved: Option<&'static str>,
    },
    Replay {
        recipe: &'static str,
    },
}
#[derive(Clone, Copy, Debug)]
pub(super) enum Expected {
    Success,
    ExactSourceRevision,
    ItemBinding,
    SourceCase,
    CurrentSourceRevision,
    ExactConstraint,
    PostObserverSource,
    Reentry,
    Fatal,
}
#[derive(Clone, Copy, Debug)]
pub(super) struct Step {
    pub(super) id: &'static str,
    pub(super) source: &'static str,
    pub(super) mode: Mode,
    pub(super) expected: Expected,
}

const fn create(
    id: &'static str,
    source: &'static str,
    order: Order,
    binding: BindingMode,
    saved: &'static str,
) -> Step {
    Step {
        id,
        source,
        mode: Mode::Create {
            order,
            relation: match order {
                Order::SourceOrder => Relation::XorBeforeOr,
                Order::ReverseReady => Relation::OrBeforeXor,
            },
            strength: Strength::Exact,
            binding,
            saved: Some(saved),
        },
        expected: Expected::Success,
    }
}
const fn replay(
    id: &'static str,
    source: &'static str,
    recipe: &'static str,
    expected: Expected,
) -> Step {
    Step {
        id,
        source,
        mode: Mode::Replay { recipe },
        expected,
    }
}
pub(super) const STEPS: [Step; 25] = [
    create(
        "create-source-exact",
        "positive",
        Order::SourceOrder,
        BindingMode::ExactRevision,
        "source-exact",
    ),
    create(
        "create-reverse-exact",
        "positive",
        Order::ReverseReady,
        BindingMode::ExactRevision,
        "reverse-exact",
    ),
    create(
        "create-source-rebind",
        "positive",
        Order::SourceOrder,
        BindingMode::RebindCurrent,
        "source-rebind",
    ),
    create(
        "create-reverse-rebind",
        "positive",
        Order::ReverseReady,
        BindingMode::RebindCurrent,
        "reverse-rebind",
    ),
    replay(
        "replay-source",
        "positive",
        "source-exact",
        Expected::Success,
    ),
    replay(
        "repeat-source",
        "positive",
        "source-exact",
        Expected::Success,
    ),
    replay(
        "replay-reverse",
        "positive",
        "reverse-exact",
        Expected::Success,
    ),
    replay(
        "repeat-reverse",
        "positive",
        "reverse-exact",
        Expected::Success,
    ),
    replay(
        "edited-source-exact",
        "renamed",
        "source-exact",
        Expected::ExactSourceRevision,
    ),
    replay(
        "edited-reverse-exact",
        "renamed",
        "reverse-exact",
        Expected::ExactSourceRevision,
    ),
    replay(
        "edited-source-rebind",
        "renamed",
        "source-rebind",
        Expected::Success,
    ),
    replay(
        "edited-reverse-rebind",
        "renamed",
        "reverse-rebind",
        Expected::Success,
    ),
    replay(
        "changed-item-refused",
        "new-item",
        "source-rebind",
        Expected::ItemBinding,
    ),
    create(
        "regenerate-source",
        "new-item",
        Order::SourceOrder,
        BindingMode::ExactRevision,
        "new-source",
    ),
    create(
        "regenerate-reverse",
        "new-item",
        Order::ReverseReady,
        BindingMode::ExactRevision,
        "new-reverse",
    ),
    replay(
        "new-source-replay",
        "new-item",
        "new-source",
        Expected::Success,
    ),
    replay(
        "new-reverse-replay",
        "new-item",
        "new-reverse",
        Expected::Success,
    ),
    replay(
        "operator-refused",
        "changed-operator",
        "source-rebind",
        Expected::SourceCase,
    ),
    replay(
        "alias-refused",
        "local-alias",
        "source-rebind",
        Expected::SourceCase,
    ),
    replay(
        "ambiguous-refused",
        "ambiguous",
        "source-rebind",
        Expected::SourceCase,
    ),
    replay(
        "launch-refused",
        "wrong-launch",
        "source-rebind",
        Expected::SourceCase,
    ),
    replay(
        "target-refused",
        "wrong-target",
        "source-rebind",
        Expected::SourceCase,
    ),
    replay(
        "stale-current-source",
        "stale-source",
        "source-rebind",
        Expected::CurrentSourceRevision,
    ),
    Step {
        id: "exact-mismatch",
        source: "positive",
        mode: Mode::Create {
            order: Order::SourceOrder,
            relation: Relation::OrBeforeXor,
            strength: Strength::Exact,
            binding: BindingMode::ExactRevision,
            saved: None,
        },
        expected: Expected::ExactConstraint,
    },
    Step {
        id: "advisory-mismatch",
        source: "positive",
        mode: Mode::Create {
            order: Order::SourceOrder,
            relation: Relation::OrBeforeXor,
            strength: Strength::Advisory,
            binding: BindingMode::ExactRevision,
            saved: Some("advisory-source"),
        },
        expected: Expected::Success,
    },
];

pub(super) const FAULTS: [Step; 3] = [
    fault("probe-source-change", Expected::PostObserverSource),
    fault("probe-reentry", Expected::Reentry),
    fault("probe-fatal", Expected::Fatal),
];
const fn fault(id: &'static str, expected: Expected) -> Step {
    Step {
        id,
        source: id,
        mode: Mode::Create {
            order: Order::SourceOrder,
            relation: Relation::XorBeforeOr,
            strength: Strength::Exact,
            binding: BindingMode::ExactRevision,
            saved: None,
        },
        expected,
    }
}
pub(super) fn is_fault(step: Step) -> bool {
    FAULTS.iter().any(|row| row.id == step.id)
}
pub(super) fn step(id: &str) -> Step {
    *STEPS
        .iter()
        .chain(FAULTS.iter())
        .find(|step| step.id == id)
        .expect("closed release recipe step")
}
pub(super) fn source(case: &str) -> String {
    assert!(SOURCES.contains(&case));
    if case == "new-item" {
        fixture_cases::source("positive").replace("choose_order", "choose_order_new")
    } else if case.starts_with("probe-") {
        fixture_cases::source("positive")
    } else {
        fixture_cases::source(case)
    }
}
pub(super) fn relative(directory: &Path, case: &str) -> PathBuf {
    assert!(SOURCES.contains(&case));
    paths::relative_root(directory).join(case).join("source.rs")
}
pub(super) fn absolute(directory: &Path, case: &str) -> PathBuf {
    let path = repository().join(relative(directory, case));
    paths::checked_source_file(&path);
    assert!(fs::metadata(&path).unwrap().len() <= 64 * 1024);
    path
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Invocation {
    pub(super) step: String,
    pub(super) source_case: String,
    pub(super) source_relative: PathBuf,
    pub(super) source_sha256: String,
    pub(super) recipe_name: Option<String>,
    pub(super) recipe_sha256: Option<String>,
    pub(super) args: Vec<String>,
    pub(super) crate_binding: String,
    pub(super) cargo_observation: String,
    fixture_sha256: [String; 4],
    artifacts_sha256: String,
    metadata_sha256: String,
}
pub(super) fn derive(directory: &Path, step: Step) -> Invocation {
    let source = absolute(directory, step.source);
    let (args, crate_binding, cargo_observation) = invocation_for_fixture_source(
        directory,
        &fixture(),
        PACKAGE,
        CRATE_NAME,
        Some(FEATURES[0]),
        &source,
        if step.source == "wrong-target" {
            "gfx950"
        } else {
            "gfx942"
        },
    );
    let recipe = match step.mode {
        Mode::Replay { recipe } => Some(recipe),
        _ => None,
    };
    Invocation {
        step: step.id.into(),
        source_case: step.source.into(),
        source_relative: relative(directory, step.source),
        source_sha256: hash(&source),
        recipe_name: recipe.map(str::to_owned),
        recipe_sha256: recipe.map(|name| hash(&io::recipe_path(directory, name))),
        args,
        crate_binding,
        cargo_observation,
        fixture_sha256: FIXTURE_FILES.map(|(path, _)| hash(&fixture().join(path))),
        artifacts_sha256: hash(&directory.join("dependencies.stdout")),
        metadata_sha256: hash(&directory.join("metadata.stdout")),
    }
}

#[test]
fn release_recipe_ladder_has_exact_closed_attempt_and_source_census() {
    let mut ids = std::collections::BTreeSet::new();
    for step in STEPS.into_iter().chain(FAULTS) {
        assert!(ids.insert(step.id));
        assert!(SOURCES.contains(&step.source));
        match step.mode {
            Mode::Create {
                saved: Some(name), ..
            } => assert!(RECIPE_NAMES.contains(&name)),
            Mode::Replay { recipe } => assert!(RECIPE_NAMES.contains(&recipe)),
            _ => {}
        }
    }
    assert_eq!(STEPS.len(), 25);
    assert_eq!(FAULTS.len(), 3);
    assert_eq!(ids.len(), 28);
    assert!(
        FAULTS
            .iter()
            .all(|step| !matches!(step.expected, Expected::Success))
    );
    assert_eq!(
        STEPS
            .iter()
            .filter(|step| matches!(step.expected, Expected::Success))
            .count(),
        15
    );
    assert_eq!(
        STEPS
            .iter()
            .filter(|step| !matches!(step.expected, Expected::Success))
            .count(),
        10
    );
    assert!(source("renamed").contains("renamed_a"));
    assert!(source("renamed").contains("// λ:"));
    assert!(source("new-item").contains("choose_order_new"));
    assert_ne!(source("positive"), source("renamed"));
    assert_ne!(source("positive"), source("new-item"));
}
