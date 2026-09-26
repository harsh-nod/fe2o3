# Immutable Domain Arena Refinement

Base: signed `e668544e55774fcbee46eb2c51bbb413a5ab0ec9`.
This development extends the production-shared resource-domain proof from
planning over supplied facts to exact immutable arena lookup, selected ancestry
and fact extraction. It is not complete MEM-DOM, A1/A2 or HIP/HSA acceptance.

## Production Boundary

The existing private `Key`, `Node`, root identity and profile constants now have
one declaration source. The actual `State::node` and `State::path` call shared
slice-based helpers over `state.nodes`, and reservation extracts its planner
facts using the same shared implementation proved by Verus. There is no
converted shadow arena, copied roster, new heap allocation, public API change
or runtime uniqueness scan. Global record-total checks, error precedence,
locking, mutable node access and commit remain in the existing adapter.

Lookup checks the selected slot, vacancy and both stored key fields. Path
traversal follows the same leaf-to-root order, immutable three/four-level
profile and terminal root check. Exact field comparisons replace derived key
equality only in these shared bodies; the behavior is checked against the old
independent lookup/traversal implementation. Inactive path entries remain ROOT
(slot zero, generation one), not an all-zero key. Fact extraction copies used,
capacity, all three phase counts and record limit from each selected live node;
inactive facts have zero fields. It does not validate counters before R75 does.

## Formal Properties

The final proof is split into the existing **19-obligation planner** and a
**21-obligation immutable arena** unit. Shared declaration obligations occur in
both counts, so the counts do not describe distinct properties. Both direct
development runs pass with zero errors. No valid-arena
precondition assumes corrupt input away. Path success holds if and only if the
bounded, exact-generation walk terminates at ROOT. On success the proof exposes
the exact selected sequence, leaf identity, parent edges, terminal None parent,
depth bounds, distinct occupied slots and unchanged ROOT padding.

The executable path ensures `exact_path_result`, which binds success/rejection,
the exact walk, depth, terminal root and padding. A separate proved implication
takes that identical predicate as its only premise and derives leaf identity,
linked ancestry and distinct slots. It adds no well-formedness assumptions and
does not invoke quantified graph lemmas inside the executable loop proof.

Uniqueness is proved after termination, not assumed for every traversal prefix:
cyclic malformed inputs can repeat while progressing toward bounded rejection.
If a successful path repeated a node, deterministic successors would force an
earlier nonterminal node to equal the terminal node, contradicting their parent
values. This reasoning is ghost-only and adds no execution scan.

Fact extraction succeeds exactly for a nonempty, at-most-four-entry prefix of
live keys and yields each node's exact planning fields. It independently allows
duplicate/nonroot paths; the preceding production path result supplies the
stronger ancestry properties. The immutable slice and plain shared node fields
frame input state. This is immutable extraction plus the separately proved R75
planner, not a composed proof of the entire admission/commit operation.

The authenticated campaign captures eighteen source/build inputs, including the
accounting library's vector aliases, and the exact eight-file executable proof
closure. Every Rust helper has one pinned hook-free invocation. Thirty-one
semantic mutations cover the prior planner/vector premises and new slot,
generation, root, profile, depth, parent, leaf, padding, fact-node association
and fact-field premises.
Compiler failures, timeout/resource failures or incomplete obligation rosters
cannot substitute for logical rejection. The existing nine controller
calibrations cover source and classifier drift plus process ownership cleanup.

Campaign 4 runs both units for every staged case: the affected unit must have a
complete logical rejection, while the unaffected unit must still pass its full
positive roster. Two positive cases plus 31 mutations require 66 unit executions.
Campaign 4 passes with zero runner/recheck exits. Both opening and closing
positives verify planner 19/0 and arena 21/0. All 31 mutations are logically
rejected by their affected unit and every unaffected unit passes; all nine
controller calibrations pass. The source manifest is
`7d623ac3fac1aaa9addd901fcaac58494da034025f602e84a3f4d08c6ebeefc0`.
`positive_runs: 2` in the result counts bracket cases, not individual unit runs.
An independent post-run audit checks all 66 exit/summary pairs (35 positive
unit runs including unaffected units, and 31 logical rejections), and confirms
all 69 recorded proof/tool process groups are absent.

The outer shell runner is an independently reviewed trust boundary, not a
self-authenticating root. Its before/after digest receipts and the signed source
change bind its bytes. The runner authenticates captured Python before import;
the controller checks the 190-file pinned Verus release before and after proof.
Rust compilation, Verus and hardware remain trusted external boundaries.

Development run 1 retains the initial failed obligations; run 2 is an earlier
31/0 result. Explicit key-field comparisons resolve opaque derived equality in
the executable proof, and a ghost sequence fact establishes empty fact padding.
Campaign 1 correctly stops when the wrong-release-coordinate mutation also
hits the default solver resource limit in the unrelated path proof. The
classifier is not weakened and resource limits are not raised. Slot uniqueness
is factored into a separate lemma; development run 3 exposes a missing explicit
prefix-index correspondence, and run 4 establishes an intermediate 32/0 result. The
same release-coordinate development mutant then fails only its intended logical
obligations (31 verified, one error), with no resource-limit substitute.
Campaign 2 exposes the same coupling under a different planner mutation and
also stops. The independent planner and immutable arena boundaries now have
separate proof roots, preserving all contracts and the full union of captured
inputs. Development run 5 verifies the two units separately; campaign 3 stops
at the key-generation mutant because an unrelated path obligation again hits
the default resource limit. Development run/screen 6 shows that making the walk
opaque and adding loop edge facts alone does not resolve it. Run 7 separates
exact execution from the derived graph properties and verifies 21/0; its screen
then rejects only because the classifier did not admit Rust's ordinary warning
count in the abort footer. The mutant has 20 verified/one intended logical error,
without a resource failure. The footer parser now admits only an optional
numeric warning count after the exact abort summary, with calibrated rejection
of extra text, syntax errors and resource failures. Full error-line matching,
complete obligation rosters and whole-log infrastructure checks remain required.
Development screen 8 passes both positives and all sixteen arena mutants; it
is explicitly unauthenticated and does not replace campaign 4. No earlier
failure is counted as final success. The arena unit's
unused scalar-operation macro warnings are retained; only the shared ZERO body
is used there, while the planner unit verifies the scalar operations.

## Tests And Qualification

Four new Rust test groups cover:

- 432,180 comparisons with the independently frozen old lookup/traversal: every
  four-node parent choice from None, four valid keys, a stale key and an
  out-of-range key, with zero or one vacant slot, six profiles and six leaf
  selections. Empty arenas are checked separately. Cycles, invalid roots and
  three-versus-four-depth behavior are included.
- Exact used/capacity values in all nineteen coordinates, distinct phase counts,
  record limits, poisoned inactive padding, invalid fact depths and stale keys.
- Actual reservations at different ancestor levels before leaf admission,
  preserving exact ancestor deltas and independent final disposal.
- Complete state snapshots for bad ancestry through reserve, retain, unissued
  cancellation, child creation, account Drop, retained disposal and quarantine.
  Only poison and its retention anchor may change on rejection. Fault injection
  is internal CPU corruption, not a claim of public-API reachability.

The first Rust attempt failed to compile a test calling a nonexistent explicit
reservation-cancel method. The test now uses the real unissued Drop path. The
expanded initial accounting run passes all 63 tests. The final suite includes
the later two retained-credit rejection cases within the same test group.

| Final four-package check | Result |
| --- | --- |
| All-feature libraries | Model 1,034 passed/19 ignored; accounting 63 passed; KFD 1,321 passed/one failed/296 construction cases filtered; runtime 1,476 passed/three failed/28 ignored. Exit 101 |
| Selected construction/custody tests, same feature unification | 67 KFD and two runtime passed; exit 0 |
| All-feature doctests | 104 passed: 27 model, three accounting, 28 KFD, 46 runtime; exit 0 |
| Strict all-feature/all-target Clippy | Passed; exit 0 |
| No-default-feature check and workspace formatting | Both passed; exit 0 |

The selections overlap and are not additive coverage. The four failures are the
previously recorded KFD
`target_debug_telemetry_v2::tests::credential_bound_channel_accepts_typed_failure_before_publication`
at `SocketAdmission`, and runtime
`authorized_execution::tests::cooperative_debug_telemetry_emits_only_bounded_logical_records`,
`authorized_execution::tests::failed_session_end_is_explicit_and_terminal`, and
`authorized_execution::tests::pre_native_telemetry_failure_is_returned_and_poisoned`
at `InspectSocket(EPERM)`. This is not full CPU qualification. No native ignore
is promoted to a pass, and production socket validation is unchanged.

The driver is [`validate.sh`](validate.sh), with locked/offline dependencies,
debug information and incremental compilation disabled, and two test threads.
Its exact per-command exits are retained. Every Rust command returned; the
driver exits 1 because the library group failed. The initial source checksums
freeze all four package trees plus workspace manifests before the suite. The
closing recheck detects only six intentional proof/controller/pin changes, all under
the Verus directory; every compiled Rust source/test input is unchanged.
The campaign-3 snapshot and its then-passing recheck are retained under that
name; the closing snapshot records the final reviewed proof/controller revision.
The prior
planner benchmark is not rerun or promoted to this changed source revision;
no new performance result is claimed.

Reproduction of the source-pinned proof needs a fresh output directory:

```sh
sh crates/fe2o3-runtime-model/verus/verify-resource-domain.sh \
  /absolute/new-output /absolute/pinned-verus-release/verus
```

## Cleanup And Remaining Work

The MI300X probe still fails resolving `sharkmi300x-1` before remote entry, so
no shared-machine resource is created and no native result is added.
Owned local target:
`/home/harsh/.codex-tmp/fe2o3-domain-arena-target-20260926-5nObiWoK`.
The target occupied 630,904 KiB before removal. All source and four test-binary
checksums were rechecked, every validation/campaign command returned, then the
owned target was removed. Removal and an independent absence check both return
zero; the receipts are retained. No shared-machine cleanup was needed.

Mutable ledger conservation, free-list/token/record correspondence, node
retirement, poison/anchor/mutex behavior, and whole admission-execution
composition remain unproved here. Native cost/disposal correspondence,
complete bootstrap/terminal/resource closure, protected generated execution,
Worker/device-language/atomic/collective authority and matched HIP/HSA
qualification also remain open. Accepted milestones are unchanged.

The next logical/native composition must use a third request-credit leaf beside
N1/N2, not attach Context to the inclusive session account. A retained-credit
witness must bind each actual allocation to that exact leaf and byte charge,
including direct backend calls and a backend returned by Context shutdown.
Logical and native records must have explicit combined ceilings; pooled native
residency may legitimately outlive logical disposal. Same-root identity alone
does not establish these conditions. The current two-boolean credit-presence
check also needs exact device/credit association before that integration.
The first foundation is a borrowed retained-credit check against the exact
account, live slot/owner generation, Retained phase and complete expected charge
vector, without creating another domain handle. Same-root membership is too
weak. Credits within the same exact leaf and charge are currently fungible;
there is no need to add a redundant allocation-to-slot identity unless a later
native custody contract actually depends on it.
