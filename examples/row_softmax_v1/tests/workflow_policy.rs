const GENERIC_WORKFLOW: &str = include_str!("../../../.github/workflows/row-softmax-v1.yml");
const REVIEWED_WORKFLOW: &str =
    include_str!("../../../.github/workflows/row-softmax-v1-reviewed-host.yml");

const TRUSTED_JOB_GUARD: &str = r#"    if: >-
      github.repository == 'harsh-nod/fe2o3' &&
      github.ref == 'refs/heads/main' &&
      (github.event_name == 'push' || github.event_name == 'workflow_dispatch')"#;

fn has_trusted_reviewed_host_policy(workflow: &str) -> bool {
    workflow.contains("  push:\n    branches: [main]\n")
        && workflow.contains("  workflow_dispatch:\n")
        && !workflow.contains("pull_request")
        && !workflow.contains("merge_group")
        && workflow.contains(TRUSTED_JOB_GUARD)
        && workflow.matches("runs-on: [self-hosted").count() == 1
        && workflow.contains("uses: actions/checkout@11bd71901bbe5b1630ceea73d27597364c9af683")
}

#[test]
fn untrusted_events_have_only_github_hosted_jobs() {
    assert!(GENERIC_WORKFLOW.contains("pull_request:"));
    assert!(GENERIC_WORKFLOW.contains("merge_group:"));
    assert_eq!(GENERIC_WORKFLOW.matches("runs-on: ubuntu-24.04").count(), 2);
    assert!(!GENERIC_WORKFLOW.contains("self-hosted"));
    assert!(!GENERIC_WORKFLOW.contains("workflow_dispatch"));
    assert!(!GENERIC_WORKFLOW.contains("pull_request_target"));
    assert!(GENERIC_WORKFLOW.contains(
        "raw.githubusercontent.com/verus-lang/verus/\
b677dd5a766f25f56e9aa1e32621aa4e53304b47/source/rust_verify/src/attributes.rs"
    ));
    assert!(GENERIC_WORKFLOW.contains("--audit-parser-source"));
}

#[test]
fn reviewed_host_requires_exact_trusted_repository_ref_and_events() {
    assert!(has_trusted_reviewed_host_policy(REVIEWED_WORKFLOW));
}

#[test]
fn reviewed_host_policy_rejects_each_weakened_guard() {
    let mutations = [
        REVIEWED_WORKFLOW.replace("harsh-nod/fe2o3", "attacker/fe2o3"),
        REVIEWED_WORKFLOW.replace("refs/heads/main", "refs/heads/feature"),
        REVIEWED_WORKFLOW.replace("github.event_name == 'push'", "true"),
        REVIEWED_WORKFLOW.replace("github.event_name == 'workflow_dispatch'", "true"),
        REVIEWED_WORKFLOW.replace("branches: [main]", "branches: ['**']"),
        REVIEWED_WORKFLOW.replace("  workflow_dispatch:\n", "  pull_request:\n"),
    ];

    for mutation in mutations {
        assert!(!has_trusted_reviewed_host_policy(&mutation));
    }
}
