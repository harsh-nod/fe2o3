# Exact Completion Event Release

Source commit: `f70bd5af95e37cc7850975bd8c30c83a11d5ff69`.

This packet qualifies the actual private single-event host-ledger release body
and its Ready, active-event and live-occurrence validators. It does not close
A1/A2, issue #182, or HIP/HSA parity.

## Implementation And Theorem

The production methods and Verus root include the same executable macro bodies
from `crates/fe2o3-kfd/src/queue_completion/event_release_body.rs`. The extraction
preserves check ordering, error precedence and accepted states. The former local
preflight closure is a private method; a guarded or-pattern becomes an equivalent
match because the pinned verifier does not support the former syntax.

Without assuming a healthy ledger, the proof establishes:

- Success exactly when the owner is Ready, the complete event identity matches
  its ledger entry, queue/mapping and live slot generation/batch match, and the
  selected event-pin count is positive.
- Refusal returns the original non-Copy event and unchanged logical owner state.
  Non-Ready owners return Poisoned before other validation.
- Success removes only that ledger entry and decrements only its event pin.
  Reader storage/pins, other events/slots, generations, phases and identity
  counters remain unchanged.

Bound, Published and Completed slots are accepted. Release does not independently
require a packet ID, nonzero logical IDs, completion observation or zero reader
pins. The constructed witness covers all three phases and every positive u32 pin
count, with surviving reader state and malformed neighboring records.

The proof uses the actual `Box<[Slot; 8192]>` and standard `HashMap<u64, Exact>`
operations, not an assumed authenticated lookup or selected-entry adapter.
Numeric identity newtypes project to u64 while preserving every nested field.
Reader storage is an arbitrary non-Copy value. The debug equality check is proved
as an assertion, including configurations where Rust would erase it.

## Results

- Signed Verus campaign: **23/23 stages pass**. Opening, relocated and closing
  positives each report **16 verified functions**, comprising five executable
  methods, one constructed witness and ten derived Clone implementations.
- **15 executable mutations** produce strictly classified logical failures:
  missing/weak identity and phase checks, missing Ready/active preflight,
  unchanged pins, wrong event removal, wrong slot update, reader-pin destruction,
  identity rewind and substituted returned custody. Each selects the changed
  method, not a caller that could assume an unchanged helper contract.
- Pinned Verus closure matches before and after: 190 files, 129019839 bytes.
  Signed source binding, exact two-file relocation and closing continuity pass.
- Full KFD library suite: **1726 passed, 0 failed, 0 ignored**.
- Full runtime library suite: **1828 passed, 0 failed, 30 hardware ignores**.
- **95 doctests**, strict all-feature/all-target Clippy, production-only checks,
  formatting, included-test formatting and diff checks pass.
- Four new CPU groups cover 18 identity substitutions, 12 refusal cases, six
  malformed-but-accepted controls, and all three live phases with genuine
  same-slot events, an independent reader and a neighboring event. Refusal
  snapshots include all logical owner fields and slot/ledger Box addresses.

The four-group synthetic classifier/source-wiring test is separately recorded
after signing. The campaign also invokes it before the inherited source bracket;
it is not one of the 23 owned stages. Historical controllers/classifiers are
SHA-authenticated and unchanged. Development frontend failures and preliminary
runs remain in the archive and are not counted as final qualification.

## Trust And Remaining Work

Trusted boundaries include Rust/compiler semantics, the pinned Verus/vstd/Z3
closure, standard-library contracts for HashMap, Box, arrays and arithmetic, and
the stated structural representation projection. HashMap internals, allocator
behavior/capacity/hasher state, physical allocation addresses and machine-code
correspondence are not proved by the logical state view.

This is not a proof of global ledger-to-pin cardinality, batch rollback, the
public session/lane wrapper, runtime source-event handoff, unwind/currentness,
native target scheduling, GPU execution, physical overlap or performance.
Canonical historical custody manifests and accepted lane checkpoints are
unchanged. No MI300X jobs or remote artifacts were created.

The next composition is batch recording, the actual native no-effect callback's
ledger frame, batch event release and bound cancellation. Source rollback creates
one event per distinct slot; a generic aliasing-roster theorem additionally needs
aggregate pin validation or a proved cardinality invariant.

## Replay And Artifacts

`raw.tar.gz` contains CPU logs, source/test-ELF hashes, development diagnostics,
the complete signed campaign (including relocated/mutated inputs), toolchain
metadata and the fresh open #182 snapshot. `SHA256SUMS` seals this README and the
archive; the archive contains its own file manifest. Test ELFs are not archived;
no hermetic build or benchmark claim is made.

From a clean signed source checkout with the pinned Verus closure installed:

```sh
python3 -I -B crates/fe2o3-runtime-model/verus/check-completion-event-release.py \
  --output /absolute/new/owned/output \
  --verus /absolute/pinned/verus-x86-linux/verus
```

Use a new output directory outside the checkout. Exact CPU commands and scope
notes are in the archived `NOTES.md`; archive restoration and all file hashes
were checked before publication.
