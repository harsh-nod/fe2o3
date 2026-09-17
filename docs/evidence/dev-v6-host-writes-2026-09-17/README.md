# V6 Synchronous Host Writes And Unknown Disposal

CPU/model-qualified development above canonical base
`0bd02a821c82e449d8c1933ec5246ab3f8a1169b`. Accepted checkpoints remain Native
R125 CPU/test, Admission R118B C1-C3 and Resources R116/V3. This does not complete
V5/V6, A1/A2, #182, protected native composition or HIP/HSA parity.

## Change

The opt-in Context journal now registers synchronous host writes before backend
entry, using genuine Context IDs and private move-only tickets. Success and
definite pre-effect rejection settle their exact model writer. Quiescent failure
retains Unknown; terminal failure or panic also seals Context and quarantines
that allocation's request credits. Original diagnostics and unwind payloads are
preserved. Default, unconfigured writes retain their prior behavior.

An exact one-member Synchronous Unknown may retire only after confirmed backend
allocation disposal. Failed disposal retains the full owner for retry or terminal
custody. Pending and unsupported Unknown rosters block their own disposal while
cleanup continues unrelated allocations. Retained writer counts prevent false
complete cleanup, even for an empty writer. No lineage or reuse API is exposed.

The model adds a distinct O(k), allocation-free whole-Unknown disposal transition,
not success or NoEffect settlement. Its exact complete roster is an inert premise,
not native authority. Multi-member production disposal remains unsupported.
Ordinary KFD post-upload and XGMI post-unmap invariant failures can no longer be
misclassified as pre-effect Rejected. See the
[integration boundary](../../runtime-context-version-journal-host-write-v1.md).

## Qualification

The frozen source ran locally in
`/home/harsh/.codex-tmp/fe2o3-c4-completion-20260917`. GNU and scoped musl each pass:

| Package | Passed | Ignored |
| --- | ---: | ---: |
| fe2o3-runtime | 970 | 17 |
| fe2o3-host | 271 | 4 |
| fe2o3-runtime-model | 779 | 2 |

Each target has 2,020 passes, no failures or filtered tests. All 21 added tests
pass: six model disposal, eleven Context writer, four KFD classification tests.
The existing chunked SDMA failure test additionally exercises the public write
path. Musl uses `FE2O3_HIP_SYS_DISABLE=1`; unrestricted musl/HIP linkage is not
qualified. Strict all-feature/all-target Clippy, scoped formatting, no-default
checks, 86 doctests and unsafe-source policy (5 passed, 1 ignored) pass.

Fifteen closed raw command records have exit zero and retain exact argv, UTC
start/finish, output and status. The six executed unit binaries are hashed.
The initial single-line roster comparison omitted two successful subprocess
parents; its original subset and record are retained. The authoritative
`*.complete-roster` files include those split-line results, validate per-package
pass/ignored summaries and uniqueness, and match across all 2,043 entries.

The source manifest covers all 17 changed files (14 Rust, three docs), bracketed
by successful pre/post qualification checks. Its SHA-256 is
`df5e3a2e2dbd3d0a72578ac2a3b1cee3ec44148cff3ff6cf0a053a14a048035c`.
The source patch above the base has SHA-256
`ccbb579bcf09cbdec55cffc07cf4619776346309ccd06567fb12a57216160c58`.
Independent read-only reviews checked production ownership, model transitions,
source coverage, claims and the closed GNU record. They are not proofs.
`SHA256SUMS` seals the archive; the recorder refuses sealed or duplicate records.

## Limits

No native GPU, formal solver or matched HIP/HSA benchmark was run. No MI300X files
or processes were created. Scripted copy effects are not GPU execution; XGMI
post-unmap coverage is static wiring, not injected native-session failure.
Secondary settlement/quarantine-panic precedence is source-reviewed, not fault
injected. Model partition preservation assumes valid global prestates. Disposed
allocation-key nonreuse is a caller premise supplied by genuine Context IDs.

SPI classifications are Contracted, not authenticated machine-refinement
evidence. The existing V4-J1 issuance proof is unchanged and does not prove the
new disposal or Rust/native correspondence. Launch/copy/alias mutation coverage,
ordered writers, input leases, multi-member disposal progress, content recovery,
aggregate residency and native qualification remain open. The full runtime
parity/performance goal remains active.
