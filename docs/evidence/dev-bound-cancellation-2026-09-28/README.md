# Complete Bound Cancellation Refinement

Production, CPU and proof source: `b87580f30dc76c3bcda5703fb73ffd23ea3c718a`.
Final signed campaign source: `cb45869762adcd2421da46f2ba6de5c73ea26fa6`.
The latter changes only this campaign's checker and synthetic calibration.

This packet refines the complete host-ledger bound-cancellation operation. It
does not close A1/A2, issue #182, source rollback, or HIP/HSA parity.

## Shared Production Body

Rust and Verus now share packet-count validation, bound/retention validation,
the first-pinned-slot check and the complete retaining cancellation body. The
three existing private retention callers pass their expected phase directly;
indexed loops preserve the former traversal order. Validation retains the real
128-word bitmap, checks slot bounds before indexing it, and precedes all pin
checks and owner mutation. Work is O(N + 128), with 1024 bytes of bitmap scratch
and no new allocation. No measured performance improvement is claimed.

The theorem derives validity from the executed checks, without assuming an
authenticated roster or Ready owner. It covers zero/oversized packet counts,
packet-presence precedence, exact owner/slot/batch identities, the existing
dispatch checks, distinct slots and the first pin error's exact counters.
Failure returns the original non-Copy retention and unchanged modeled owner.
Success changes only selected slot phases to Available. Generations, identity
counters, neighboring slots, owner phase and an opaque non-Copy ledger remain
unchanged. It neither clears poison nor rewinds burned identities.

The proof executes the actual bitmap operations. A bitvector membership lemma
and prefix invariant establish exact duplicate detection across all words.
Constructed witnesses exercise slots 63/64, duplicate refusal, late pin errors,
success in arbitrary owner phases, malformed neighbors and count boundaries.

These are the raw method's actual checks: dispatch generation need only be
nonzero, and code/kernarg allocation identities are checked only through their
VMs. This is not a proof of full dispatch authentication or native currentness.

## Qualification

- Final signed campaign: **31/31 stages pass**. Opening, relocated and closing
  positives each report **25 verified obligations, zero errors**. The count
  includes ten derived Clone checks, not 25 independent runtime operations.
- **23 executable negative controls** fail logically under strict diagnostic
  classification: omitted validation, bitmap/identity/count errors, pin/error
  omissions, neighbor/generation mutation and substituted returned retention.
- Signed source binding, exact two-file relocation, complete source continuity
  and before/after pinned tool closure pass: 6192 source hashes and 190 tool
  files (129019839 bytes). All 31 recorded process groups are reaped and absent.
- Completion tests: **52 passed**. Three new groups cover 15 refusal cases,
  genuine cleanup/retry, bitmap boundaries and the raw owner-phase contract.
  Refusals check all token fields, both Box addresses and a full owner snapshot.
- Broader KFD regressions: **1412 passed, zero failures**, explicitly excluding
  **320 construction-primary tests**. This is not a full KFD-suite rerun.
- Full runtime suite: **1828 passed, zero failures, 30 hardware ignores**.
- **124 doctests**, strict all-feature/all-target Clippy, no-default-feature
  production checks, workspace/included-source formatting and diff checks pass.

The seven-file CPU qualification hash bracket begins after the focused build;
its closing check passes. The final checker-only commit leaves production,
tests and proof unchanged; final source hashes are separately retained.

The first signed campaign is retained as failed evidence. Its opening proof
reported 25 verified obligations and zero errors, but the classifier rejected
an unknown informational bitvector-enumeration note emitted by the pinned
verifier with `--multiple-errors 0`. The final checker recognizes only that
exact note. It does not admit new error diagnostics or change the authenticated
historical controller/classifier files. Calibration rejects altered/error-level,
foreign-source, nested-error and resource-bearing notes; the note alone cannot
qualify a negative control. Four synthetic calibration groups pass outside the
31 owned stages. Development failures are retained, not counted as controls.

## Boundaries And Next Work

Trusted boundaries include Rust/compiler behavior, pinned Verus/vstd/Z3,
standard Box/array/arithmetic contracts and the stated identity/storage
projection. The proof does not establish allocator internals, physical Box
address preservation, machine-code correspondence, global ledger/pin cardinality,
native publication/currentness or performance. Selected CPU tests inspect actual
Box-address preservation; that is not a universal allocator proof.

Next is complete batch event-release refinement, preserving its two fallible
allocation boundaries and legacy error order, then the release/cancel/dispatch
rollback composition. The actual source wrapper short-circuits and discards
returned tokens on cleanup failure, then terminalizes. Release success followed
by cancellation failure is a terminal partial cleanup, not whole-chain atomic
rollback or recovered Rust-token custody. Recording freshness/pin increments
and native callback ledger frames also remain to be composed.

Native target scheduling, protected Worker/compiler execution, physical versus
semantic settlement, lane-local storage, high-depth async ownership, multi-GPU
and distributed qualification, and matched HIP/HSA performance remain open.
Accepted lane checkpoints and A1/A2 status are unchanged. No MI300X work or new
GPU/performance result was produced in this packet.

## Replay

`raw.tar.gz` contains logs, source/tool/test-ELF hashes, development and failed
campaign evidence, the complete final campaign, command notes and an open #182
snapshot. ELFs are not archived; no hermetic-build claim is made. `SHA256SUMS`
seals this README and archive; the archive has its own file manifest.
A fresh restore matches the raw tree exactly and passes all 266 file hashes.

From a clean signed source checkout with the pinned tool closure installed:

```sh
python3 -I -B crates/fe2o3-runtime-model/verus/check-completion-bound-cancel.py \
  --output /absolute/new/owned/output \
  --verus /absolute/pinned/verus-x86-linux/verus
```
