# Live Prepared Persistent Cancellation

Status: R125 is locally accepted at the CPU/test boundary above published R124
(`c171d915047daa5153c1f77253e15260e55da8c5`), following two independent archive
reviews. This accepts the live prepared cancellation ownership path, detached
generation repair and bounded inline layout, not native or formal qualification.
A1/A2 and issue #182 remain incomplete.

## Ownership Boundary

Both single- and three-binding public cancellation routes use the private
`queue_live/persistent_cancel.rs` sequencer. Receipt validation remains in the
public wrappers. Once an exact attachment is admitted, cleanup failure is
terminal and does not return retryable authority.

The outer root retains the bounded attachment and its allocation/use ledgers.
A native-only carrier retains the lower control-cleanup root, returned data,
mapped lease slots, initialization flags, separate restoration/cancellation
prefixes, and output storage. The original dispatch remains recoverable when
the model callback never opens. The model callback returns unit; no returned
data or attachment travels through the model driver's generic result.

Normal lower errors still extract available data without replacing the original
error with an extraction error. Retake errors override normal lower outcomes;
a captured lower panic takes precedence over subsequent retake or poison panics.
Custody is retained before final poisoning or resuming an unwind.

Three-binding cancellation preserves its all-entry restore preflight, restore
prefix, all-entry cancellation preflight, and cancellation prefix. Single-binding
cancellation does not gain those additional preflights. Before restoration
starts, Prepared entries retain the prior quarantine policy. Once the first
restore is attempted, failed and untouched Prepared leases remain exact; no
blanket quarantine or later-entry cancellation overwrites a partial prefix.

Returned generation, cardinality, storage identity, and initialization policy
are checked while all data remains rooted. Single cancellation uses the returned
initialization flag; three-binding cancellation also checks attachment flags.
Both reject authenticated-but-uninitialized output before any restoration.
Ledger access precedes infallible output construction; all four detached-ledger
updates follow validated output and completed owner transitions.

Terminal re-entry preserves existing native custody, including custody retained
by earlier operations. Mixed prefixes have no falsely uniform stage observation.
Foreign, wrong-generation, and wrong-shape receipts remain recoverable.

## Detached Generations

Constructed tests exposed a replay bug hidden by the older returned-data hook:
fresh persistent preparation after detached generation 7 started its counter
at 8 but reported cancellation generation 0. They also exposed rejection of
`Some(0)` when rebinding after initial prepublication cancellation.

Persistent preparation now records inherited detached generation separately
from actual recycle history. Its cancellation query prefers actual later
recycle history, then the inherited predecessor, then zero. `None`, `Some(0)`,
and positive predecessors retain their distinct construction provenance without
minting a completion on the new owner. Ordinary generation constructors and
returning-destroy/after-recycle queries are unchanged. No predecessor is inferred
from an advanced counter after cancelled reservations.

## Bounded Owner Layout

The first complete GNU attempt for this cancellation implementation exposed a
runtime-library stack overflow. Three output owners in the new cancellation
carrier enlarged the inline custody representation; each owner previously
embedded its entire 64-slot use ledger, and the bounded attachment also reserves
16 owner slots. An unchanged runtime fixture reproduced the failure on the
ordinary test stack.

Persistent allocation owners now allocate a boxed, fixed-size ledger once at
construction. This adds one host allocation per owner, without changing ledger
capacity, token identity, transition ordering or native ownership. Cancellation,
failure retention and terminal re-entry do not allocate this ledger. The existing
infallible constructor allocation policy is unchanged; recoverable allocation
failure is not newly established.

Measured GNU production type sizes, in bytes:

| Type | Before | Repaired |
| --- | ---: | ---: |
| Persistent allocation owner | 3,320 | 256 |
| Cancellation native carrier | 14,120 | 4,928 |
| Bounded persistent attachment | 73,072 | 21,048 |
| Compute queue session | 83,256 | 31,232 |
| KFD runtime backend | 105,328 | 50,304 |

These are inline type sizes, not maximum call-stack or total retained-memory
measurements. The fixed ledger still occupies host heap storage. Layout budgets
and local/peer owner tests cover moves, recoverable extraction failure, typed
transitions, retirement and exact native identity. The original failing fixture
is unchanged; no larger test stack was configured.

## Validation Scope

The new constructed fixtures use registered memory, directional persistent
promotion, exact Prepared leases, actual persistent preparation, the production
model-loan driver, and lower control cleanup. They do not use the existing
`persistent_compute_test_release` shortcut. The single fixture uses the checked-in
write-only active-checkpoint kernel and its inspected complete kernarg extent;
the kernel is never submitted to hardware.

Coverage includes initial/replay generations, initialized/cold/digest-free single
inputs, initialized three-binding inputs, malformed returned data, model and
cleanup errors/panics, all-three preflights, every restore/cancel prefix, exact
failed Prepared identities, primary panic precedence, and public terminal receipt
handling. Native cleanup operations here are CPU fixture calls, not KFD hardware
execution. Development checks do not constitute formal production refinement.

The earlier GNU-v2 attempt passed all 1,241 KFD tests but failed in the runtime
library with the stack overflow above. Its records remain failed evidence, not
a passing full regression. After the layout repair, the boxed-ledger source cohort
`6486eaa2d52d9a4035db5d137fa216dcef9f853f8df6518c0aa47905534be72e`
passed all 744 runtime library tests and all 45 selected KFD allocation,
cancellation and persistent lower-cleanup tests. Workspace formatting and strict
all-feature/all-target Clippy for KFD and runtime passed.
The renewed full GNU-v3 regression subsequently passed 2,900 tests, with five
ignored, across the exact 48 libtest targets plus one harnessless CSV target.
The exact roster/record checker passed with all 5,709 source identities
unchanged and the owned process group absent. Fresh checker calibrations pass
55 record/parser cases, 49 gate cases and 101 mutation-parser cases. These are
CPU-only development results; the calibration cases are not compiled runtime
mutations. The full musl-v3 attempt subsequently hit its predeclared 60-minute
deadline: 508 passing rows and no assertion failures were observed before the
runner terminated the incomplete suite. Source was unchanged and the owned
process group was closed. This attempt is rejected evidence, not musl
qualification.

The subsequent timeout investigation found that a test-only setup assertion
iterated byte by byte over each 186,019,840-byte context-save mapping. It now
uses exact 4 KiB slice comparisons, including the final partial page, as the
existing lossless snapshot encoding already does. Real mappings, production
zeroing, fault transitions and every lifecycle snapshot remain unchanged. A new
regression covers empty, aligned, unaligned and partial-page slices and injects
corruption at every included byte position.

This changes two fixture files relative to the boxed-ledger cohort. The new
source identity is
`29b127dd3e2f1eaa9cbc3e8bd7bfd7fb26bb8879e58bdf0ab801c291f188dd80`.
The byte-corruption test passes on GNU and musl. One paired run of the unchanged
30-case retained-control error/panic fixture passed with these test durations:

| Target | Previous | Exact-Byte Scan |
| --- | ---: | ---: |
| GNU | 136.85 s | 47.67 s |
| musl | 190.11 s | 85.36 s |

Test durations exclude compilation; retained wrapper durations include Cargo
startup and compilation. The audit checks all six diagnostic records, exact
commands and test bodies, source endpoints, the two-file source delta, clock
ordering and current-boot process closure. These are single paired CPU fixture
observations, not a full-suite speedup, a proven timeout root cause, or GPU
performance evidence. The previous full GNU pass does not qualify this changed
source. Fresh GNU/musl full regressions expect 2,901 passes and five ignored,
with a prospectively declared 90-minute deadline per full run. GNU-v4 completed
in 1,391.690165518 monotonic seconds (23.2 minutes), passing all 2,901 tests with
five ignored across 48 libtest targets and one separately checked CSV target.
The exact executable/test/summary checker passed with all 5,709 source identities
unchanged and the owned process group absent. Fresh calibrations pass 57 record
and transcript cases, 49 gate cases and 101 mutation-parser cases; none of these
207 cases is a compiled runtime mutation. Full musl-v4 subsequently completed
in 1,921.67384712 monotonic seconds (32.0 minutes), passing the same exact
2,901-test roster with five ignored across 48 libtest targets and one separately
checked CSV target. The combined GNU/musl checker passed; all 5,709 source
identities remain unchanged and both owned process groups are absent. The old
musl-v3 timeout remains rejected historical evidence, not a qualified run.
All 25 source/auxiliary leaf gates and their aggregate checker now pass.
Each of 18 compiled production mutations reaches its predeclared behavioral
assertion with Cargo exit 101, followed by exact restoration of the source map.
These are cancellation/generation negatives, not mutation coverage of the
boxed-ledger layout, owner-move tests or exact-byte scan. The restored 47-test
KFD and 744-test runtime suites also pass. The collector validates and replays
all 92 prerequisite runs and retains its own closed record separately.

The [accepted evidence archive](evidence/local-r125-live-persistent-cancel-2026-09-15/README.md)
contains 576 raw artifacts totaling 216,503,126 bytes. Two independent archive
reviews verify all hashes, rosters, 126 closed process groups, 122 predecessor
links and the exact collector replay. The 33 historical runs remain explicitly
nonqualifying. Static implementation review found no actionable defect; additional
cancellation-specific coverage for a suppressed panic payload with a panicking
destructor remains a defense-in-depth test opportunity.

Native execution, aggregate retained
memory, formal implementation correspondence, and HIP/HSA performance acceptance
remain open; A1/A2 and issue #182 are not complete.

Both current three-binding admission layers require initialized A/B/C storage.
Cold C output is a separate admission/readiness gap, not a passing cancellation
profile. Cold returned-data corruption is tested only as a negative probe.

## Next Integration

The next packet is retained ordinary primary-queue Release custody. It must
retain the parent and each native/model cleanup prefix through errors and
panics, including exception payload, doorbell, resource authorities, dispatch
and signals. Preserve all four GPU unmaps before any of their releases, then
complete shadows; process-gate reuse waits for confirmed complete teardown.
Generated DATA-ADOPT, actual ISSUE, completion/readback/typed replies and
Stop/drain/graph integration follow. Other teardown profiles remain required
for the broader parity objective; they are not accepted by this checkpoint.
