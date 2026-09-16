# V4-J1 Issuance Integration: Development Receipt

This packet extends `21960410c1998eb462dd0cf1bc6e95a453f66d9b`.
The [scope document](../../runtime-context-version-journal-issuance-v1.md) states
the exact contents-level proof and its remaining Rust/storage/Context boundaries.
Resources R116/V3 and the Native lane's R125 CPU/test checkpoint remain accepted. This is not
full V4, R126, A1/A2, issue #182, native or performance acceptance.

The canonical proof imports the external V15 source with SHA-256
`366c03768a9f662e511c5a5efaa6290cfdaba9b0534e04032af4ac45267825c8`, updates its
historical header and adds whole-journal contents wrappers/refinement lemmas.
The canonical source SHA-256 is
`018a295aa951568aa93f2f32318106af341c79e4568dda649ad803c6f4b95792`.
`source-files.sha256` binds the changed proof, tests, checker/runner, pins and
reproduction script, plus the unchanged Rust journal implementation reviewed
against the proof. Binding and review do not constitute verified correspondence.
The complete nine-file runtime-model `source.patch` has SHA-256
`3ccb2d7a2d506dd376996906d276369b04a308664e41498f30dea6d7810f82ab`.

## Verification Campaign

`verify-issuance.sh CHECKOUT OUTPUT_DIRECTORY VERUS` runs the CPU/proof campaign.
The output directory must already exist and the mutation subdirectory must not.
It requires locked offline Rust dependencies, GNU/musl targets, `jq`, Python,
GNU `prlimit` and the pinned Verus distribution. No GPU is used by this campaign.

The runner retains commands, UTC timestamps, exit codes, raw output and selected
Cargo executable identities. The standalone mutation campaign retains generated
source, source audits, structured solver output, process records, source/tool
hashes and full Verus release-closure checks before/after. Each source is derived
independently from the canonical proof; contracts are not mutated. The ordinary
full proof gate reruns these checks, but its own temporary artifacts are removed
by its existing trap; the separately archived standalone campaign supplies the
raw mutation records. Neither invocation is a full-workspace Rust test campaign.

The helper self-test includes adversarial structured diagnostics and real
owned-process cases: interruption during spawn, during wait and after reaping,
timeout, spawn failure and normal success. Spawn/wait/timeout tests include a
child and grandchild, test unblocked child signals, retained raw output and
process-group absence. These calibrate the runner, not GPU shutdown behavior.

## CPU Results

GNU and musl runtime-model library runs each pass 766 tests, with zero failures
and two existing benchmark-style scale tests ignored (16.01 and 17.81 seconds).
The three new journal tests run normally. Strict model all-feature/all-target
Clippy, workspace formatting, model no-default checking, checker self-tests and
the 686-file negative inventory audit pass. The unsafe-source policy passes five
tests with its explicit maintenance test ignored. These counts do not include
the separate runtime library or the full workspace.

## Standalone Proof Results

The final standalone campaign passes all 23 cases: unchanged positives before
and after each report 69 verified obligations and zero errors; all 21 independent
executable mutations report 68 verified obligations and exactly their intended
postcondition error. Both full release-closure checks match 190 files and
129,019,839 bytes. All 13 recorded input identities match the frozen source and
tools. All 48 serial owned process groups are recorded absent and were also
independently observed absent after completion.

An independent read-only review reconstructed every candidate, checked exact
diagnostic spans and whole-crate counts, and matched the GNU/musl test rosters
and executable hashes. A separate static review confirms the helper's signal
handling and post-reap output preservation, source/helper pins, seven runner
function hashes, 1,511 preseal assignments, runner hash and transcript pin.
These reviews are separate from the full proof gate and final outer source
check recorded below.

## Full Gate Results

All 20 recorded runner commands finish with exit zero. The full runtime-model
proof gate passes 63 positive proof files with 1,443 verified obligations and
zero errors, all 686 historical expected-negative cases, and its own repeated
21-mutation journal campaign bracketed by unchanged 69/0 positives. The final
quality/inventory audit, authenticated Verus release closure, pinned success
transcript and outer source-hash check pass. Its owned temporary directory was
removed by the runner and independently checked absent after completion.

The complete final output is retained under `final/`; it was copied only after
normal runner completion and compared byte-for-byte with the original output.
This qualifies the canonical contents-model proof and its integration into the
model gate. It does not establish Rust implementation correspondence, physical
storage guarantees, production Context integration or the other exclusions above.

## Preliminary History

Preliminary solver and runner observations are retained separately from final
qualification. The initial closure check used the distribution symlink as its
root and rejected it; the resolved distribution subsequently matched all 190
files and 129,019,839 bytes. The initial canonical positive passed 69/0.

The first partial mutation campaign was deliberately interrupted for runner
hardening. It exposed that raising `InterruptedError` from a signal handler can
be swallowed by Python's selector retry logic. The owned active verifier group
was explicitly terminated; the campaign rejected its nonstandard exit. The
replacement uses a distinct exception and protects spawn ownership with a signal
mask, restoring the child mask before exec. A subsequent preliminary CPU runner
was stopped for a final post-reap raw-output preservation fix. These interrupted
runs are not reused as completed acceptance campaigns.

No production runtime behavior, native execution or HIP/HSA measurement is added
by this packet. `SHA256SUMS` covers every other archived member; binaries are not
included.

## Deferred Native Work

A separate read-only MI300X observation found GPUs 1-7 temporarily free of
workload processes, with approximately 298.6 MB each of idle/system VRAM. The
published `21960410c` musl runtime test binary and a prepared launcher were uploaded
to the uniquely owned `/tmp/fe2o3-r126-cold-20260916.TpvYK4` directory.
The immediate pre-execution check then found a new workload on GPU 1 (PID
3530383, total used VRAM 9,073,684,480 bytes), with workloads on all other GPUs.
Neither native probe was started. The uploaded binary hash matched its existing
receipt; the entire owned remote directory was removed and its absence checked.
Only hardware/PID observations are retained; no workload command arguments or
paths were collected. This is deferred scheduling evidence, not native validation.
