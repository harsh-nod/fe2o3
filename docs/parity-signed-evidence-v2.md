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

Before classification, the protected verifier independently checks the event
identity: the protected base must equal the freshly fetched default-branch tip,
the protected and candidate checkouts must equal their event SHAs, and the
candidate head must descend from that exact tip. A stale feature branch or a
substituted checkout therefore cannot retain an older evidence policy.

The protected workflow also handles GitHub `merge_group/checks_requested` and
reruns the same base-tip, ancestry, trust-monotonicity, and promotion checks on
the synthetic merge commit. This is necessary for merge queues, but workflow
code alone cannot stop GitHub from merging a stale or unchecked commit. The
repository must enforce an active ruleset with no bypass actors, strict
source-pinned checks, required protected workflows, and a merge queue. See
[GitHub's merge queue documentation](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/configuring-pull-request-merges/managing-a-merge-queue)
and [repository rules API](https://docs.github.com/en/rest/repos/rules).

Repository-rule activation remains an external production blocker. This
repository can render and validate a proposed ruleset document, but neither the
workflow nor its tests establish that a ruleset is installed on GitHub. No
ruleset was installed as part of this work. An administrator must install it
out of band and run authenticated remote verification before production
activation.

This repository intentionally contains no active production trust policy and
no production public or private key. Production promotion therefore fails
closed until an operator installs protected public-key configuration and
provisions runner-held private keys. PEM files under scripts/tests/fixtures
are explicitly test-only. Their trust domain is test, which production gating
rejects.

## Operator Provisioning

1. Generate separate Ed25519 attestor, publisher, and reviewer keys outside the
   repository.
2. Keep private keys runner-owned, mode 0600, and outside every checkout.
3. Export only the public keys and run the fail-closed bootstrap into a new,
   otherwise absent directory:

       scripts/parity-row-evidence.sh bootstrap-production-trust \
         --output-root /tmp/parity-production-trust \
         --attestor-public-key /operator-public/attestor.pem \
         --attestor-key-id operator-runner-v1 \
         --publisher-public-key /operator-public/publisher.pem \
         --publisher-key-id operator-publisher-v1 \
         --reviewer-public-key /operator-public/reviewer.pem \
         --reviewer-key-id operator-reviewer-v1

4. Review and copy the generated `docs/parity-evidence` tree into the candidate.
   The bootstrap canonicalizes public PEM, rejects private input, requires
   distinct keys and IDs, writes exact SHA-256 bindings, and never overwrites an
   existing output. It writes through stable directory descriptors, fsyncs
   every public-key and policy file plus each containing directory, and
   publishes with `renameat2(RENAME_NOREPLACE)` through a held parent
   descriptor. A successful return therefore means the new trust tree and its
   parent-directory entry reached the filesystem durability boundary.
5. Merge the configuration without changing parity status. It becomes trusted
   only after it is part of a protected base commit.
6. Protect the workflow and trust paths with repository rules and code-owner
   review.
7. Require pull-request branches to be up to date with the protected default
   branch before merge, or require a merge queue. The workflow checks default-tip
   freshness when it runs, but cannot configure repository rules or prevent the
   default branch from advancing between a completed check and merge.

8. Render, review, and install the repository ruleset. Obtain the numeric
   repository ID and GitHub Actions integration ID from authenticated GitHub
   API responses, then run:

       scripts/parity-repository-rules.sh render \
         --repository-id REPOSITORY_ID \
         --actions-integration-id ACTIONS_INTEGRATION_ID \
         --default-branch main > /tmp/parity-rules.json

       scripts/parity-repository-rules.sh bootstrap \
         --repo OWNER/REPO \
         --repository-id REPOSITORY_ID \
         --actions-integration-id ACTIONS_INTEGRATION_ID \
         --default-branch main

       scripts/parity-repository-rules.sh verify \
         --repo OWNER/REPO \
         --repository-id REPOSITORY_ID \
         --actions-integration-id ACTIONS_INTEGRATION_ID \
         --default-branch main

   `bootstrap` requires repository Administration write permission and refuses
   to update or replace an existing ruleset. `verify` requires sufficient
   access for GitHub to return `bypass_actors`; an omitted field fails closed.
   The generated policy allows no bypass, pins both workflows to protected
   `main`, requires strict GitHub-Actions-sourced status checks, stale-review
   dismissal, code-owner and last-push review, and an `ALLGREEN` squash merge
   queue with one PR per merge group.

Validate a staged or installed tree independently:

    scripts/parity-row-evidence.sh validate-production-trust \
      --trusted-root /path/to/export \
      --trust-policy /path/to/export/docs/parity-evidence/trust-policy-v2.tsv

Validation requires the canonical policy and key locations, exactly one key for
each attestor, publisher, and reviewer role, three distinct canonical Ed25519
public keys, the production domain, and the fixed metadata allowlist. Merely
hand-writing a parseable policy is insufficient.

The active trust policy is canonical TSV:

    parity_trust_policy_schema_version  2
    trust_domain                        production
    metadata_path_count                 2
    metadata_path  0000  exact   docs/cuda-oxide-parity-status.tsv
    metadata_path  0001  prefix  docs/parity-evidence/archive/
    key_count                           3
    key  0000  attestor  runner-v1    PUBLIC_KEY_PATH  SHA256  ed25519
    key  0001  publisher publisher-v1 PUBLIC_KEY_PATH  SHA256  ed25519
    key  0002  reviewer  reviewer-v1  PUBLIC_KEY_PATH  SHA256  ed25519

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

## Immutable Archive Ingestion

MI300X output is not copied directly into a promotion branch. The operator
first pins the manifest digest, baseline, source commit and tree, target, and
lane over an out-of-band authenticated control path. It then finalizes the
source archive with the Linux immutable flag on the root, every directory, and
every file before invoking ingestion:

    scripts/parity-row-evidence.sh ingest-archive \
      --repo /detached/source \
      --source-root /operator-output/run-001 \
      --destination-root /candidate/docs/parity-evidence/archive \
      --trusted-root /protected/base-export \
      --trust-policy /protected/base-export/docs/parity-evidence/trust-policy-v2.tsv \
      --manifest manifests/promotion-v2.tsv \
      --expected-manifest-sha256 SHA256 \
      --expected-baseline BASE_COMMIT \
      --expected-source SOURCE_COMMIT \
      --expected-tree SOURCE_TREE \
      --expected-target gfx942 \
      --expected-lane mi300x-gfx942-release

The current in-process publisher is **not production-authoritative**. Production
ingestion fails closed with `production archive publication requires an
externally protected publisher contract`, even after validating production
trust and an immutable source. Activation requires a separately provisioned
publisher identity or service whose destination namespace and content cannot be
modified by the evidence-producing UID, plus a verifier for that contract.
That external contract is not present in this repository or on the reviewed
host.

The inert/test publisher rejects mutable production sources, symlinks,
hardlinks,
non-regular entries, missing referenced content, undeclared files or
directories, test-domain results, identity mismatches, and pre-existing output.
It opens the source root component-by-component and traverses it with stable
directory descriptors and Linux `openat2` using
`RESOLVE_BENEATH|RESOLVE_NO_SYMLINKS|RESOLVE_NO_XDEV`. Every file remains open
from authentication through parsing and copy; immutable state, type, device,
inode, link count, size, and digest are checked on that exact descriptor.
Individual files are limited to 256 MiB and the complete source archive to 2
GiB, with both limits enforced from `fstat` before hashing or copying.

The destination parent is independently opened by an absolute component walk
that rejects symlinks. A permanent parent-wide lock serializes enumeration,
stale recovery, and creation across every manifest. Each destination/manifest
identity uses deterministic lease and staging names. Lease initialization first
creates a recognizable provisional name containing owner UID, PID, Linux
process start time, and challenge; it fsyncs the complete lease and atomically
renames it to the canonical create-new name before creating staging. Empty or
partial provisional files left at any crash boundary can therefore be recovered
only after the encoded PID/start-time owner is provably dead. Canonical stale
lease, staging, and provisional dirents remain open and identity-checked while
they are removed under the global lock, so two recoverers cannot delete a new
winner. A live publisher fails busy. The parent admits at most 128 recognized
lease, provisional, and staging entries, and checks the exact post-creation
count under the same lock. Private staging directories, copied files, and the
index are then
created relative to retained parent/staging descriptors. Every copied-file and
index descriptor remains open through publication and is re-read, re-hashed,
and re-`fstat`ed immediately before and after rename. Publication
uses `renameat2(parent_fd, staging_name, parent_fd, destination_name,
RENAME_NOREPLACE)`. The requested parent path must still resolve to the held
parent inode before rename; the published child is reopened relative to both
the held and freshly resolved parent and must match the staging inode. A parent
path replacement therefore fails rather than publishing into a detached
directory. Before rename, copied files, the generated index, every created
directory bottom-up, and the staging root are fsynced. After rename, the
destination root and parent are fsynced. A post-rename fsync failure is reported
as an indeterminate durable-publication result and never deletes the complete
published archive. The Git commit supplies the subsequent immutable object
identity consumed by hosted CI.

Mode `0444`/`0555` is packaging metadata, not a same-UID security boundary: the
owner can chmod and mutate those objects. Retained descriptors detect mutation
around rename, but continuous namespace and content integrity after return is
possible only under the external protected-publisher contract. Consequently,
test-mode publication cannot grant production promotion authority.

## Protected Publisher Receipt

Production uses a detached `publisher-receipt-v2.tsv`. It is delivered to a
runner-owned directory by the external protected publisher service, separate
from the candidate checkout and containing no other files. The protected gate
and production archive validator require the canonical payload to be signed in
the production trust domain by the distinct `publisher` key role. Detachment
avoids a commit self-reference: the receipt can bind the final candidate commit
without its own bytes changing that commit. The receipt binds:

- protected publisher/key identity and
  `external-protected-portable-archive-v2` destination contract;
- the canonical logical repository destination, independent of checkout-local
  device and inode numbers;
- SHA-256 identity of every relative file path, size, digest, and directory in
  the transported archive, including the canonical archive index;
- manifest path/digest, source commit/tree, target, and hardware lane;
- baseline and candidate status-file digests, current default tip, and exact
  candidate head; and
- a service-issued 256-bit expected challenge, issue time, expiry, and a
  maximum 24-hour lifetime.

The promotion gate receives the receipt directory and expected challenge over
the protected runner/service channel. It verifies current freshness, exact
event default tip and candidate HEAD, status transition digests, logical
destination, and the complete descriptor-scanned archive tree. Copying the same
bytes to a fresh checkout therefore remains valid even though filesystem
device/inode identities change. Any file-content, relative-path, empty-directory,
transition, candidate, default-tip, or challenge substitution fails. Reusing a
receipt for another transition or event fails because those values are signed
and independently expected. The external service must issue unique challenges
and must not reissue a consumed challenge for a different request.

The detached receipt is not part of the candidate archive index. Placing a
receipt directly in Git adds an undeclared archive file and changes the signed
tree identity; it cannot substitute for the protected-service receipt. The
operator bootstrap therefore requires a third public key dedicated to the
`publisher` role. No production publisher private key, receipt, or expected
challenge is present in this repository.

Hosted workflows intentionally fail closed unless the protected publisher
service populates `${RUNNER_TEMP}/fe2o3-protected-publisher-receipt` and
`${RUNNER_TEMP}/fe2o3-protected-publisher-challenge` before the gate runs. The
current repository does not install or authenticate such a service. Its
publisher identity, key custody, unique-challenge state, and runner integration
remain external activation prerequisites.

`--allow-test-fixtures` requires a test-domain trust policy. A test-domain
publisher receipt can exercise schema/signature rejection paths, but its
signature context is not production and the test archive index does not admit
it as production authority.

The generic parity suite verifies that production ingestion fails closed for a
mutable source, but it cannot manufacture a privileged immutable filesystem.
The privileged ext4/XFS harness creates ephemeral production keys and evidence
at runtime and does not use or install repository test keys. It is currently a
prerequisite/inert test: after immutable-source validation it must stop at the
missing external publisher contract rather than publish production evidence.
Once that contract has an independently verifiable implementation, these are
the intended operator invocations:

    sudo -E env FE2O3_RUN_PRIVILEGED_IMMUTABLE_TEST=1 \
      FE2O3_IMMUTABLE_TEST_FILESYSTEM=ext4 \
      scripts/ci-local.sh parity-production-immutable

    sudo -E env FE2O3_RUN_PRIVILEGED_IMMUTABLE_TEST=1 \
      FE2O3_IMMUTABLE_TEST_FILESYSTEM=xfs \
      scripts/ci-local.sh parity-production-immutable

The command exits 77 unless explicitly opted in, and fails rather than skipping
when root privileges, loop/mount support, `chattr`, or the requested filesystem
tooling is unavailable. It cannot create a production attestation or promote a
parity row under the current same-UID publisher.

The protected promotion gate independently recomputes the index and requires
the archive to contain exactly the manifest's transitive result, queue,
toolchain, log, artifact, and reviewer-authorization closure. The
`--allow-test-fixtures` bypass exists only for generated shell tests and cannot
authorize production promotion.

Archive ingestion authenticates transport and completeness. It does not turn
an `execution_closure=inert` queue result into promotable hardware evidence;
the separately provisioned hermetic OCI executor must emit a reviewed
promotable schema.

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

Hosted CI executes the verifier extracted from the protected base:

    python3 /protected/base/scripts/parity-signed-evidence.py gate \
      --repo . \
      --archive-root docs/parity-evidence/archive \
      --trusted-root /protected/base \
      --trust-policy /protected/base/docs/parity-evidence/trust-policy-v2.tsv \
      --manifest manifests/promotion-v2.tsv \
      --trusted-policy /protected/base/docs/parity-row-evidence-policy-v2.tsv \
      --candidate-policy docs/parity-row-evidence-policy-v2.tsv \
      --baseline-status /tmp/status-before.tsv \
      --candidate-status docs/cuda-oxide-parity-status.tsv

The gate accepts only Missing-to-Partial, Missing-to-Complete, or
Partial-to-Complete, requires exact policy classes, rejects test-domain
evidence, verifies source trees, and permits only protected-policy metadata
changes after attestation.

## Tests

    scripts/ci-local.sh parity-evidence

Shell suites generate test-domain signatures at runtime. They cover key and
policy substitution, signature mutation, replay, row/class/target relabeling,
stale source, duplicate identity, insufficient evidence, queue bypass,
Complete downgrade, missing review, metadata-only deltas, lock link attacks,
and concurrent queues. The test hardware queue does not claim a real GPU run.
