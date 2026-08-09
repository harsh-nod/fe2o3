# Signed Parity Evidence V2

V2 moves promotion authority out of candidate-owned checksums. Every result
and MI300X queue is an Ed25519-signed canonical payload. Complete also needs a
separate reviewer signature over the exact promotion evidence set.

## Trust Boundary

Hosted CI extracts the verifier, persistent row policy, active trust policy,
and public keys from the protected base commit. The candidate supplies status
changes and an evidence archive. It cannot select a verifier, trusted key, or
weaker row policy. CI compares the candidate row policy byte-for-byte with the
protected policy while processing a promotion.

This repository intentionally contains no active production trust policy and
no production public or private key. Production promotion therefore fails
closed until an operator installs protected public-key configuration and
provisions runner-held private keys. PEM files under scripts/tests/fixtures
are explicitly test-only. Their trust domain is test, which production gating
rejects.

## Operator Provisioning

1. Generate separate Ed25519 attestor and reviewer keys outside the repository.
2. Keep private keys runner-owned, mode 0600, and outside every checkout.
3. Commit only public keys under docs/parity-evidence/trusted-keys.
4. Copy trust-policy-v2.example.tsv to trust-policy-v2.tsv, replace key IDs and
   SHA-256 placeholders, and review the metadata allowlist.
5. Merge the configuration without changing parity status. It becomes trusted
   only after it is part of a protected base commit.
6. Protect the workflow and trust paths with repository rules and code-owner
   review.
7. Require pull-request branches to be up to date with the protected default
   branch before merge, or require a merge queue. The workflow checks default-tip
   freshness when it runs, but cannot configure repository rules or prevent the
   default branch from advancing between a completed check and merge.

The active trust policy is canonical TSV:

    parity_trust_policy_schema_version  2
    trust_domain                        production
    metadata_path_count                 6
    metadata_path  0000  exact   docs/cuda-oxide-parity-matrix.md
    metadata_path  0001  exact   docs/cuda-oxide-parity-status.tsv
    metadata_path  0002  exact   docs/generated/cuda-oxide-parity-dashboard.md
    metadata_path  0003  exact   docs/generated/cuda-oxide-parity-dashboard.tsv
    metadata_path  0004  exact   docs/generated/cuda-oxide-parity-signed-promotions.tsv
    metadata_path  0005  prefix  docs/parity-evidence/archive/
    key_count                           2
    key  0000  attestor  runner-v1    PUBLIC_KEY_PATH  SHA256  ed25519
    key  0001  reviewer  reviewer-v1  PUBLIC_KEY_PATH  SHA256  ed25519

Public-key fingerprints are SHA-256 over canonical Ed25519 SubjectPublicKeyInfo
DER, not PEM file bytes, and must be unique across every role and key ID.
Protected trust updates cannot expand metadata, add or replace signing
authority, remove role separation, change domain, or delete the active policy.
The same reviewed update must preserve the exact row set, target set and
row-to-target identities, and reviewer roles in the persistent row policy.
Partial and Complete class requirements may only grow, and Complete must remain
a strict superset of Partial. No break-glass path is implemented.


Tabs, not spaces, separate fields. Metadata paths are the only files allowed
to differ between the attested source commit and candidate HEAD. No-renames
diff semantics ensure moving or deleting implementation source still fails.
The projection paths do not grant candidate authorship: after the signed gate
succeeds, the protected verifier emits a canonical transaction record and
protected-base generators rebuild the matrix, signed-promotion ledger, and
both dashboards. Hosted CI byte-compares those outputs with the candidate.
The matrix starts from the protected-base template, so prose, feature names,
acceptance targets, classes, and gate assignments cannot change in a promotion.
Existing evidence archive entries are append-only and byte-identical.

## Persistent Row Policy

docs/parity-row-evidence-policy-v2.tsv contains every normative and
supplemental row regardless of current status. Each row independently names
the exact target, exact Partial class set, strictly stronger Complete class
set, and reviewer role. Policies persist after transitions and are never
derived from the current set of Missing rows.

Evidence classes have canonical order:

    unit, ui, ir, compile, verus, hardware, debug

The gfx942 rows that were Missing at the vertical-slice baseline retain the
strong admission bar. GPU-facing Partial policies require hardware evidence;
they also require Verus for pointer, layout, memory, synchronization,
collective, typestate, proof-view, and other safety-bearing contracts.
Complete strictly adds another evidence class and always requires an
independent reviewer signature over the exact transition and evidence set.

Rows 46, 75, 76, and 77 are debugger/output/intrinsic surfaces without a
feature-level proof contract. Their Partial policy therefore requires
hardware plus debug evidence instead of Verus; Complete adds Verus as the
independent broader audit. S09 is a compiler-produced source-metadata surface,
so it requires compile plus debug evidence but not IR or hardware. Runtime
debugger inspection remains covered by hardware-bound row 46.


Required classes are necessary but do not replace semantic review of matrix
acceptance criteria.

## Signed Result

A hardware result is ASCII, tab-separated, newline-terminated, and ordered:

    signed_result_schema_version  3
    result_id                     UNIQUE_64_HEX
    row_id                        04
    from_status                   Missing
    to_status                     Partial
    baseline_commit               COMMIT
    source_commit                 COMMIT
    source_tree                   EXACT_GIT_TREE
    evidence_class                hardware
    target                        gfx942
    hardware_lane                 mi300x-gfx942-release
    execution_mode                production
    execution_closure             inert
    executor_count                2
    executor  0000  bash     /usr/bin/bash     SIZE  SHA256
    executor  0001  timeout  /usr/bin/timeout  SIZE  SHA256
    environment_count             N
    environment  0000  FE2O3_EVIDENCE_ARCHIVE_ROOT  HEX_ENCODED_VALUE
    queue_manifest_path           queues/mi300x.tsv
    queue_manifest_sha256         SHA256
    queue_id                      UNIQUE_64_HEX
    timeout_seconds               3600
    toolchain_count               1
    toolchain  0000  rocm  toolchains/rocm.tsv  SIZE  SHA256
    command_count                 1
    command    0000  HEX_ENCODED_EXACT_COMMAND  0
    log        0000  logs/compile.log  SIZE  SHA256
    artifact_count                1
    artifact   0000  code-object  artifacts/kernel.hsaco  SIZE  SHA256
    signature_schema_version      1
    signature_domain              production
    signature_role                attestor
    signature_algorithm           ed25519
    signing_key_id                runner-v1
    signature_base64              BASE64

The signature covers the object plus every signature-context line through
`signing_key_id`; only `signature_base64` is excluded. The verifier checks the
domain, role, algorithm, key ID, archived toolchain closure, log, and artifact
sizes and digests. A toolchain closure should inventory the exact executable,
version output, dynamic libraries, target data, and other inputs needed to
reproduce the command. Compile and hardware require at least one artifact.

Hardware uses an archive-relative signed queue path and SHA-256 digest of the
complete signed queue file. Its queue job must name the same result identity,
row, transition, source tree, target, lane, and output path.

For hardware, `queue_id` and `timeout_seconds` are nonempty queue values. The
result executor records, complete environment, toolchain records, canonical
command, log path, artifact labels and paths, queue identity, and timeout must
exactly equal the signed queue job. The queue runner executes and records the
same digest-verified absolute invocation:

    /ABS/TIMEOUT --signal=TERM --kill-after=5s TIMEOUT /ABS/BASH scripts/job.sh

Schema 2 remains accepted for non-hardware records. Hardware requires schema 3.

Result identities, paths, and signed-file digests must be unique in a
promotion manifest. One result cannot satisfy two classes or rows.

## Signed MI300X Queue

The MI300X queue is signed by an attestor key:

    signed_queue_schema_version  3
    queue_id                     UNIQUE_64_HEX
    baseline_commit              COMMIT
    source_commit                COMMIT
    source_tree                  TREE
    target                       gfx942
    hardware_lane                mi300x-gfx942-release
    execution_mode               production
    execution_closure            inert
    archive_root                 /absolute/evidence/run-001
    executor_count               2
    executor  0000  bash     /usr/bin/bash     SIZE  SHA256
    executor  0001  timeout  /usr/bin/timeout  SIZE  SHA256
    environment_count            3
    environment  0000  HOME    2f6e6f6e6578697374656e74
    environment  0001  LC_ALL  43
    environment  0002  PATH    2f6e6f6e6578697374656e74
    toolchain_count              1
    toolchain  0000  rocm  toolchains/rocm.tsv  SIZE  SHA256
    job_count                    1
    job  0000  JOB_ID  RESULT_ID  ROW  Partial  Complete  TIMEOUT  scripts/job.sh  SCRIPT_SHA256  results/hardware.tsv  logs/hardware.log  binary=artifacts/output.bin  hardware
    signature_schema_version     1
    signature_domain             production
    signature_role               attestor
    signature_algorithm          ed25519
    signing_key_id               runner-v1
    signature_base64             BASE64

Validation lstat-checks every script path component, rejects symlinks and
escapes, and requires both the checkout file and the blob in `source_tree` to
match `SCRIPT_SHA256`. It also rejects duplicate job IDs, result IDs, records,
logs, and artifact outputs.

Production execution always uses:

    /run/lock/fe2o3/mi300x-gfx942-evidence.lock

The operator provisions a regular, single-link lock owned by the runner with
mode 0600. The queue rejects symlinks, hardlinks, ownership or mode surprises,
and inode replacement. It acquires the lock before trust, manifest, checkout,
or output preflight and holds it through all jobs and result signing.

No production option selects another lock. An alternate root exists only with
test mode and a signed queue whose execution_mode is test. Test-domain queue
results cannot authorize production promotion.

The shell runner cannot prove that an arbitrary script will avoid undeclared
absolute executables. It therefore signs `execution_closure=inert`. The
promotion gate always rejects this hardware class, even after successful queue
execution and validation. A future promotable runner must provide a genuinely
hermetic executable closure and introduce a reviewed schema before it can emit
`verified`; candidate evidence cannot select that state.

Run a production queue:

    scripts/mi300x-evidence-queue.sh run \
      --repo /detached/clean/source-checkout \
      --archive-root /evidence/run-001 \
      --trusted-root /protected/base-export \
      --trust-policy /protected/base-export/docs/parity-evidence/trust-policy-v2.tsv \
      --manifest queues/mi300x.tsv \
      --signing-key /runner-secrets/attestor.pem \
      --key-id runner-v1

The checkout must be detached, clean, and exactly equal to the signed source
commit and tree.

## Promotion Manifest And Shards

Independent agents create one manifest per row shard. Results are sorted by
row and evidence class:

    promotion_manifest_schema_version  2
    baseline_commit                    COMMIT
    source_commit                      COMMIT
    source_tree                        TREE
    target                             gfx942
    hardware_lane                      mi300x-gfx942-release
    result_count                       N
    result  0000  04  Missing  Partial  unit  results/04-unit.tsv  FILE_SHA256  RESULT_ID
    evidence_set_sha256                SHA256_OF_ALL_PRECEDING_BYTES
    authorization_count                0

Validate an independent shard:

    scripts/parity-row-evidence.sh validate-shard \
      --repo . \
      --archive-root /evidence/shard-04 \
      --trusted-root /protected/base-export \
      --trust-policy /protected/base-export/docs/parity-evidence/trust-policy-v2.tsv \
      --manifest manifests/04.tsv \
      --row 04

The manifest row set must equal the repeated row options. An agent cannot
silently include another agent's row.

The final aggregate manifest is append-only and content-addressed. After its
bytes are final, its archive path must be
`manifests/promotion-<full-file-sha256>.tsv`. Every promotion adds exactly one
such file; prior manifests are immutable and cannot be selected again.

## Complete Authorization

Complete adds exactly one reviewer-signed authorization per row:

    review_authorization_schema_version  1
    authorization_id                    UNIQUE_64_HEX
    row_id                              04
    from_status                         Partial
    baseline_commit                     COMMIT
    source_commit                       COMMIT
    source_tree                         TREE
    to_status                           Complete
    target                              gfx942
    hardware_lane                       mi300x-gfx942-release
    evidence_set_sha256                 MANIFEST_EVIDENCE_SET_SHA256
    reviewer_identity                   release-reviewer
    execution_mode                      production
    signature_schema_version            1
    signature_domain                    production
    signature_role                      reviewer
    signature_algorithm                 ed25519
    signing_key_id                      reviewer-v1
    signature_base64                    BASE64

The signature binds the exact evidence set, source, target, row, and
transition.

Only `Missing -> Partial`, `Missing -> Complete`, and `Partial -> Complete`
are supported. Results, queue jobs, manifests, and Complete authorizations all
bind both statuses, preventing cross-transition replay.


## Signing And Promotion

The generic signer appends an Ed25519 trailer:

    scripts/parity-row-evidence.sh sign \
      --repo /detached/source \
      --private-key /runner-secrets/attestor.pem \
      --key-id runner-v1 \
      --domain production \
      --role attestor \
      unsigned.tsv signed.tsv

Production signing rejects repository-contained keys and keys not owned by the
runner with mode 0600. It never overwrites output.

Hosted CI first uses the verifier extracted from the protected base to derive
the sole newly appended manifest from the protected/candidate archive delta:

    manifest="$(python3 /protected/base/scripts/parity-signed-evidence.py \
      derive-promotion-manifest \
      --protected-archive /protected/base/docs/parity-evidence/archive \
      --candidate-archive docs/parity-evidence/archive)"

It then executes the protected gate with that derived path:

    python3 /protected/base/scripts/parity-signed-evidence.py gate \
      --repo . \
      --archive-root docs/parity-evidence/archive \
      --trusted-root /protected/base \
      --trust-policy /protected/base/docs/parity-evidence/trust-policy-v2.tsv \
      --manifest "${manifest}" \
      --trusted-policy /protected/base/docs/parity-row-evidence-policy-v2.tsv \
      --candidate-policy docs/parity-row-evidence-policy-v2.tsv \
      --baseline-status /tmp/status-before.tsv \
      --candidate-status docs/cuda-oxide-parity-status.tsv \
      --projection-output /tmp/promotion-transaction.tsv \
      --archive-closure-output /tmp/promotion-archive-closure.tsv

It then invokes the protected projection transaction:

    bash /protected/base/scripts/parity-promotion-projections.sh \
      /protected/base . /tmp/promotion-transaction.tsv \
      /tmp/promotion-archive-closure.tsv

The gate accepts only Missing-to-Partial, Missing-to-Complete, or
Partial-to-Complete, requires exact policy classes, rejects test-domain
evidence, verifies source trees, and permits only protected-policy metadata
changes after attestation. The protected gate also emits a canonical archive
closure bound to the transaction evidence-set digest. The checker independently
requires its manifest to be newly appended and content-addressed. That closure
contains every transitively referenced signed
result, Complete authorization, hardware queue, log, artifact, and toolchain
closure with its exact byte length and SHA-256 digest.

The projection transaction preserves prior ledger rows and replaces a row only
for a valid subsequent upgrade. The protected checker requires the candidate's
new archive files to equal the gate-produced closure and permits new directories
only when they are required parents of those files. Historical mutation,
unreferenced files, empty namespace reservations, and candidate-authored matrix
or dashboard prose therefore fail closed. Every visible status/count change is
derived from the candidate status using protected generators.

### Protected GitHub Verdict

`.github/workflows/parity-promotion.yml` runs protected default-branch code for
`pull_request_target`. It has no privileged `pull_request_review` or direct
merge-group path. Review events run only
`.github/workflows/parity-review-signal.yml`; direct merge-group execution is an
unprivileged notification. Both notification workflows have read-only
permissions, no secrets, and GitHub-hosted runners. Their completed
`workflow_run` events only wake the default-branch controller.

A notification result is never authorization. `success`, `failure`,
`cancelled`, `skipped`, `neutral`, `timed_out`, `action_required`, `stale`, and
unknown conclusions have identical notification semantics. The controller
independently fetches and validates the exact candidate or merge-group tree
using protected scripts. Generic CI remains a separate required check; the
parity verdict does not infer generic build or test correctness from CI.

Every notification is bound to more than its configured workflow ID and path.
The controller re-queries the exact run ID and requires its event, path, branch,
head SHA, workflow ID, and completed status to match the immutable event. It
then resolves the workflow-file Git blob at that exact source-run SHA and at the
protected `github.workflow_sha`. The object must be a blob and both OIDs must be
identical. A candidate-modified trigger, runner, permission, action, or command
therefore cannot act as a notification source. The candidate or merge-group
tree must also contain byte-identical `.github/workflows/ci.yml` and
`.github/workflows/parity-review-signal.yml` blobs.

Base and head refs are fetched into a runner-owned bare repository. The fetched
refs must still resolve to the declared exact 40-character commit IDs, both
objects must be commits, and a merge-group head must descend from the protected
default base. Detached worktrees are then created from those objects. The
protected classifier verifies those worktrees and derives the authoritative
changed-path count and NUL-delimited paths with an immutable two-tree
`git diff --no-renames`. The live pull-request files API and the event's
`changed_files` count are not authorization inputs.

The workflow publishes `fe2o3/protected-parity-promotion` through the GitHub
Checks API on the exact pull-request or merge-group SHA. Its deterministic
external ID is `fe2o3-parity-v1:<sha>`. Before source lookup or verification,
the controller queries exact-name checks on that SHA and selects only the
configured App ID and slug. It updates the one matching deterministic check,
adopts one unambiguous legacy App check, or creates the first check. Duplicate
dedicated checks, multiple legacy App checks, truncated inventories, and
candidate same-name checks fail closed. Check ID ordering is never an input.

All privileged parity runs share one non-cancelling concurrency group. The
selected check is reset to `in_progress`, verified, and updated in place to
`success` or `failure`. A verifier rejection therefore replaces stale success
on the same check. An interrupted run remains pending and is repaired by the
next event or reconciliation. Immediately before success, a pull-request run
requires the current number, open state, non-draft state, repositories, refs,
and SHAs to remain exact. A merge-group run independently executes the
protected parity classifier/gate on the exact queue SHA, verifies ancestry, and
refetches both refs before success.

Immediately before a successful PR verdict, the controller fetches the current
reviews and reruns the protected classifier. Designated reviewers must be
CODEOWNERS for every changed trust path, their latest review must still be
`APPROVED`, and its `commit_id` must equal the candidate head. The controller
then fetches reviews again and compares canonical review IDs, submission times,
states, commit bindings, and reviewer identities before re-querying the PR
revision. Approval dismissal or revision movement during verification fails.
Review submission, edit, dismissal, and PR synchronization reconcile the same
deterministic exact-SHA check. Native required-review and CODEOWNER rules remain
independent of the App check, so dismissal cannot be authorized by an earlier
parity result.

The pull-request trigger is intentionally not path-filtered: every candidate
SHA receives the required check, and changes outside the parity trust surface
complete through the protected classifier's `no-op` result. Runs are not
concurrency-cancelled. An hourly protected-default `schedule` and manual
`workflow_dispatch` enumerate every open PR, fetch its current reviews, and
enumerate active `gh-readonly-queue` refs. Each exact target passes through the
same protected verifier and check-update path. This repairs interrupted pending
checks and stale results without trusting notification history.

Changes to `.github/workflows/ci.yml` or
`.github/workflows/parity-review-signal.yml` cannot self-authorize. A
`pull_request_target` synchronization first reconciles the target check to
pending, then protected blob comparison and change policy force failure. Such
changes use this administrator migration procedure:

1. Isolate the workflow-byte change from parity status, archive, projection,
   key, policy, and verifier changes. Obtain normal administrator and CODEOWNER
   review of the exact commit.
2. Confirm both workflows remain read-only, GitHub-hosted, and secret-free. Use
   an audited ruleset bypass to merge the exact commit; the parity App check is
   expected to fail and cannot approve it.
3. If a workflow was recreated, update its configured immutable numeric ID.
   Run protected-default `workflow_dispatch` reconciliation and confirm the
   deterministic App check plus the separately required Generic validation
   check before removing the temporary bypass.

Configure the verdict identity as follows:

1. Create a dedicated GitHub App installed only on this repository. Grant its
   installation token `Checks: write` and no other write permission.
2. Create a `parity-verdict` Actions environment. Set
   `PARITY_VERDICT_APP_ID` as an environment variable and
   `PARITY_GENERIC_CI_WORKFLOW_ID` and
   `PARITY_REVIEW_SIGNAL_WORKFLOW_ID` to the immutable numeric workflow IDs.
   Set `PARITY_VERDICT_APP_PRIVATE_KEY` as an environment secret. Restrict
   deployment branches to the protected default branch only. Ordinary branches,
   `refs/pull/*`, and `gh-readonly-queue/*` refs must not receive this
   environment. Candidate-selected `workflow_dispatch`, direct merge-group, and
   notification jobs must not receive App secrets.
3. In the default-branch ruleset, require the exact check name
   `fe2o3/protected-parity-promotion` and select the dedicated App as the
   expected source. Pin its immutable numeric App ID, not merely its display
   name. Do not select the general GitHub Actions App as the source.
4. Also require `Generic validation` from the GitHub Actions source and add an
   organization ruleset `Require workflows to pass before merging` rule pinned
   to this repository's protected `.github/workflows/ci.yml`. Generic CI is
   intentionally independent from the parity App verdict.
5. Enable both required checks for pull requests and merge queue. Keep
   `.github/workflows/parity-promotion.yml`, both protected controller scripts,
   the environment policy, and the ruleset under administrator/CODEOWNER
   control. Require native CODEOWNER approval and dismissal of stale approvals.

The workflow verifies every create/update response against the configured App
ID and App slug, check name, check ID, candidate SHA, status, and conclusion. A
candidate workflow can emit the same visible name, but its GitHub App source is
different and cannot satisfy the source-pinned ruleset. The App private key is
never stored in this repository. Trust-change PR checks still require protected
review approvals bound to the exact PR head; merge-group checks repeat the
immutable-path and monotonic trust-update validation after candidate admission.

Loss or expiration of the dedicated App credential is fail-closed for
availability: the controller cannot reset or complete the required App check,
so admission must stop until the credential is restored and reconciliation is
run. Native CODEOWNER/review protection must remain required independently; it
prevents an old successful parity check from substituting for a dismissed
approval during an App outage.

## Tests

    scripts/ci-local.sh parity-evidence

Shell suites generate test-domain signatures at runtime. They cover key and
policy substitution, signature mutation, replay, row/class/target relabeling,
stale source, duplicate identity, insufficient evidence, queue bypass,
Complete downgrade, missing review, metadata-only deltas, lock link attacks,
and concurrent queues. The test hardware queue does not claim a real GPU run.
