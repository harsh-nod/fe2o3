# Context Enrollment Prerequisite Qualification

This is a development proof packet, not complete batch-enrollment or native
runtime acceptance. The [contract](../../runtime-context-version-enrollment-v1.md)
describes the exact admission prefix and conditional final write/truncate loop.

## Result And Limits

The corrected campaign retains two whole-crate positive runs with **168
verified, 0 errors** each: 155 obligations inherited through unchanged J4/J3/J2/J1
and 13 new obligations. Each of seven executable-body mutations produces exactly
**167 verified, 1 error** at its intended postcondition. Compiler/VIR errors,
partial-crate results, timeouts, additional errors and unrelated diagnostic
locations cannot qualify as intended negatives.

The commit theorem derives positive-reader framing from actual selected-slot
vacancy. Its output predicate is only a sufficient subset of middle-phase
properties. It does not establish selected-versus-retained-prefix disjointness,
replay exclusion, canonical admission or allocation-arena reachability.

**Not proved:** the complete batch operation, production sorting or binary
search, all-None restoration after middle-phase collision rejection, the
allocation free partition, or an end-to-end enrollment-based reader witness.
The admission prefix does preserve its caller output, which is a narrower
property than middle-phase restoration. Membership/settlement/retirement/Unknown
composition, caller allocation-key freshness, physical storage and capacity,
unwind behavior and production Rust/native refinement remain separate.

No Cargo, GPU, runtime performance or HIP/HSA parity result is included. No new
axiom, assumed sorting oracle, external-body escape or repeated-single-enrollment
replacement was introduced. Historical J1-J4 pins and proof campaigns were not
changed. This focused checker is not integrated into the shared proof runner.

## Preserved Bytes

- `corrected/`: all 118 original files from
  `/home/harsh/.codex-tmp/fe2o3-enrollment-prereq-20260918-qualified`.
- `preliminary/`: all 29 original files from
  `/home/harsh/.codex-tmp/fe2o3-enrollment-prereq-20260918-first`.
- `source/`: all 24 repository files identified by the corrected input map,
  retaining the exact proof, corrected checker, include/helper closure, pin files
  and release-closure checker. Tool binaries are deliberately not copied.

Both raw trees were mechanically copied and compared by relative file roster
and SHA-256, without changing any original byte or execution path. Each solver
receipt retains the exact source, command, stdout, stderr, normal exit status,
realtime start/end, process-group identity and `group_absent=true`. The corrected
campaign has eleven closed owned-process receipts; the preliminary has three.
Their realtime intervals are retained, not treated as solver-performance
measurements or independent timeout-duration evidence.

The corrected before/after maps contain 27 unchanged identities: the 24 retained
repository files and recorded hashes of `verus`, `rust_verify` and `z3`. The
retained release manifest names Verus `0.2026.08.09.92f466f`. Both corrected
release-closure receipts record 190 files and 129,019,839 bytes; the preliminary
has only its before check. The portable audit does not revalidate any current
tool installation and reports that limitation explicitly.

### Preliminary Attempt

The preliminary directory contains only `positive_before` (168/0),
`foreign_precedence` (167/1), the initial release-closure check, environment and
input map. It has no final report, after-input map or after-closure receipt.
The portable verifier performs **offline reanalysis using the retained corrected
checker**, whose foreign-context mutation and diagnostic expectations accept
those preserved cases. This does not reconstruct or certify completion of the
original controller.

The original checker hash is retained in `inputs-before.json`:
`e4efbd92a6ed127ea3ed7a70ff93c5f3e7a9df0c47eaf770a8bac5002f1848a0`.
Its bytes and the controller terminal output/traceback were not retained as
files. The development-session observation was an exit-1 stop at an ambiguous
mutation inverse anchor before the allocation-error solver case; this narrative
is not a replacement receipt. The corrected checker widens the anchors and
prevalidates every candidate before starting solver jobs. Only its input hash
differs between the two maps. The completed campaign's checker bytes are retained.
Neither controller's combined terminal stream is represented as an archived log;
the individual owned-command logs are complete.

The development-session controller command was the following, with the final
component changed to `fe2o3-enrollment-prereq-20260918-first` for the preliminary
attempt. This is a recorded-in-document command, not a synthetic execution receipt:

```sh
python3 -B crates/fe2o3-runtime-model/verus/check-journal-enrollment.py \
  --verus /home/harsh/.local/opt/verus/verus --timeout 180 \
  --output /home/harsh/.codex-tmp/fe2o3-enrollment-prereq-20260918-qualified
```

## Portable Audit

Run from any location with Python 3.9 or later; Verus, Cargo, GPU hardware and
the original filesystem paths are not required:

```sh
python3 -B /path/to/archive/verify.py
python3 -B /path/to/archive/test_verifier.py
```

`verify.py` first checks exact `SHA256SUMS` file closure and hashes, rejecting
symlinks and extra or missing files. It then binds snapshots to the retained
input maps, authenticates the recursive checker/include pins, reconstructs each
reversible mutation, reruns the source-policy audit in owned temporary storage,
and parses the full solver results and source-located diagnostics. It checks
the recorded commands, serial ordering, normal exits, cleanup receipts,
environment, exact case rosters and narrow final report. It never executes the
recorded Verus commands. SHA-256 integrity is not an external signature or proof
of the recorder's honesty.

`test_verifier.py` exercises 27 adverse archive fixtures, including count and
type drift, partial-crate results, compiler diagnostics, moved diagnostic spans,
timeouts, cleanup failures, altered commands, missing mutations, source/include
drift, overclaims, fabricated preliminary completion and manifest failures.
These parser calibrations mutate owned temporary copies, not this archive and
not production or proof execution. Temporary directories are removed afterward.

Current repository matching is an additional, separate check:

```sh
python3 -B /path/to/archive/verify.py --source-root /path/to/fe2o3
```

It compares only the 24 recorded repository inputs. Later source drift can fail
this check without invalidating the historical archive. It does not compare
unrelated worktree files or authenticate a current Verus installation.

`seal.py` records three packaging commands with complete output/status/timing
and owned-process cleanup: portable semantic audit, all calibrations, and current
source matching. Those additional receipts live in `packaging/`. The verifier's
explicit `--unsealed` mode is only for pre-seal semantic qualification and makes no
manifest-integrity claim. The default portable audit also validates these
packaging receipts. `SHA256SUMS` covers every archive file except itself; the
sealed packet is not modified afterward.
