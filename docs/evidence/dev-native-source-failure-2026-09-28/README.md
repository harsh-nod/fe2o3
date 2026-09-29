# Native Source Failure Settlement

Development evidence only. A1/A2 and issue #182 remain incomplete; accepted
lane checkpoints are unchanged. This packet adds no GPU execution, HIP/HSA
parity claim, or performance measurement. No remote hardware was used.

## Qualified Source

- Shared production bodies, proof, CPU tests and checker:
  `859f0354b6f616bb7d63f147c629b690b9243810`.
- Isolated omission control and final qualified source:
  `cbe547efadc1bae500424599d60263c319641cb7`.

The actual inline post-binding source failure join, native recipe cancellation
and resource-to-generation forwarding now share executable macros with Verus.
The existing epoch cancellation root and its shared body are unchanged. The
inline join retains the caller's error/drop scope. Error normalization spells
out the same DispatchBinding constructor used by the existing From conversion.
No production allocation, scan or validation pass is added. This is source
analysis, not a measured latency improvement.

## Contract and Boundary

For the native recipe, the normal-return contract is:

1. Retryable input invokes cancellation exactly once on the supplied identity.
   Success vacates only the selected Reserved epoch and returns the original
   Retryable error payload. Generations, counters and neighboring epochs remain.
2. Cancellation refusal leaves the owner unchanged and returns Terminal with
   StaleDispatchGeneration, including when the underlying refusal was Poisoned.
3. Rejected or Terminal input skips cancellation and returns Terminal with the
   original error payload, preserving owner state.

The exact premise is **Retryable input implies a present dispatch owner**.
Absent-owner Rejected and Terminal inputs remain valid, unchanged cases. An
absent-owner Retryable input still panics at the existing expect; it is not
silently reclassified or covered by the normal-return proof.

No Ready, cancellability, valid-capacity, roster-provenance or healthy-session
premise is added. Witnesses cover arbitrary neighbors, MAX counters, poisoned
and stale identities, absent non-retry owners and non-Copy error/resource/session
payloads. Ghost traces observe the actual production cancellation-call macro.

This is a **native-recipe specialization**, not a contract for arbitrary
DependencySourceRecipeV1 implementations. The proof frames the retained table
and opaque remaining resource/session values; it does not establish how binding
or native execution supplies that retained owner. Native no-effect authority,
binding/issuance provenance, the outer terminalization wrapper, Drop, panic and
unwind are not proved. The earlier completion rollback and this dispatch join
are not yet one end-to-end native source-publication theorem. Standard
Verus/vstd/Z3/compiler trust remains; no custom external-body contract is added.

## Qualification

Both final campaigns use **--no-cheating** at the same signed source:

| Campaign | Stages | Whole-crate obligations | Executable negatives |
| --- | ---: | ---: | ---: |
| Failure join and native forwarding | 21/21 | 21/0 errors | 13 |
| Resource-to-generation forwarding | 11/11 | 21/0 errors | 3 |

These are **32 passing stages and 16 executable negatives**. Each campaign
passes original, exact relocated and closing whole-crate positives, signed blob
binding, **6218 unchanged source hashes**, and the pinned **190-file /
129019839-byte** Verus release closure. The 21 obligations overlap between runs
and include the existing 16 epoch-root obligations and derived checks; they are
not distinct runtime operations. The historical 22 epoch mutants are **not**
rerun in these campaigns.

Six synthetic calibration groups reject changed source graphs, additional trust,
wrong production wiring, ambiguous mutation sites, invalid positives and mixed
or foreign diagnostics. The exact five-file proof closure is audited. The
inherited strict diagnostic classifier remains unchanged.

- Focused source suite: **13 passed**, including three new test groups.
- Final KFD regression: **1421 passed, zero failures**, with **320 construction
  tests explicitly excluded**. This is not a full KFD-suite rerun.
- Full runtime library: **1828 passed, zero failures, 30 hardware ignores**.
- **124 doctests**: 42 KFD, 53 runtime (9 plus 44), 29 runtime-model.
- Strict all-feature/all-target Clippy, no-default-feature production checks,
  workspace/included-body formatting and diff checks pass.
- All **48 recorded process groups**, including the stopped campaign, are reaped
  and independently confirmed absent.

The new native CPU group uses actual preparation and retained dispatch resources,
with three neighboring N=3 bindings on each lane across all three failure classes
(six cases). It checks the complete generation-table snapshot, selected retained
resource identities, memory observations, both completion ledgers and dependency
state. The original String pointer, capacity and contents survive appropriate
returns. Cleanup uses the actual ordinary release path and asserts all prepared
memory was released. These snapshots are not every dispatch-resource field.

Additional groups cover the three absent-owner classes and two distinct callback
refusals. The callback spy explicitly counts exactly one refusal cancellation;
successful native CPU cancellation is checked by its state transition, while
the formal call trace establishes exactly-once execution for the proved join.
The spy does not instantiate the native resource proof or establish GPU authority.

## Retained Refusal

The first campaign rejected an omitted-call mutant despite its genuine
postcondition failure because it also emitted two unused-macro warnings. The
corrected mutant keeps a syntactically referenced but unreachable call, so the
execution is still omitted without unrelated diagnostics. No checker rule was
relaxed. Both the failed campaign and an earlier proof-development frontend
failure remain in the archive; neither is counted as qualification success.

## Remaining Work

Next is allocation-free post-publication event binding and its composition with
completion publication. The current scratch Vec adds an allocation-failure point
after native publication. Full validation followed by in-place commit can remove
it while retaining O(N) work. A binding refusal must preserve the already-Published
completion prefix, not claim publication rollback. The later lane-event collection
is a separate allocation and remains outside that proposed improvement.

Native publication authenticity, owner/issuance provenance, outer dispatch
publication and terminalization, Context/Worker composition, target scheduling,
physical overlap, high-depth reuse, multi-device/distributed faults and matched
HIP/HSA measurement remain open. A1/A2 exit criteria are not promoted.

## Archive

`raw.tar.gz` is **3929510 bytes**, containing **343 manifested files plus the
inner SHA256SUMS manifest**. It retains signed relocated/mutated inputs,
development and stopped-run logs, final proof and CPU/static process records,
reproduction commands and the independent final audit.

An independent restore passed every hash and recursive comparison; its owned
temporary directory was removed and absence checked. No shared-machine files
were touched. The outer SHA256SUMS seals this report and archive. See
`COMMANDS.md`, `qualify.py`, `audit-final.py` and `final-audit.json` in the archive.
