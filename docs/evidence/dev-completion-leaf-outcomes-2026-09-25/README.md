# Completion Leaf Outcome Development

This successor to the sealed shared-planner safety packet strengthens only the
standalone finite-graph projection. The production planner body is unchanged.
No native, HIP/HSA performance, protected execution or A1/A2 exit gate closes.

The shared general planner now has a conditional exact eligible-leaf outcome
theorem, supported by executable singleton constructors for both profiles.
Absent/Success input returns Success; Unknown input returns Quiescence. Both
take exactly two iterations when settlement succeeds. Every nonzero u8 injected
settlement failure returns its exact error after one iteration, preserves
Pending/custody, and quarantines. Exact effects frame unrelated nodes and the
directed state. Validators now specify exact success and rejection results.

These are constructor witnesses of the finite projection, not real Context
constructors. In particular, real empty-dependency leaf queries return None;
the model's empty-dependency Success/Unknown inputs are not shown reachable from
real construction. Fixed observation inputs and synthetic failure markers do
not establish evolving-journal or concrete failure-prefix refinement.

`run.py` requires clean signed source and the pinned Verus closure, runs full
root positives and an exact relocated replay, checks four isolated logical
mutants, and rechecks source/tool continuity. Solver limits remain unchanged.
Only identified verification failures qualify as negative results; parse/type
failures, warnings, timeouts and solver resource limits are rejected.
Measured bytes are bound directly to signed Git blob identities, independently
of index flags and content filters. Diagnostic locations must resolve to files
in the exact mutant snapshot.

The mutation cases skip successful settlement, promote Unknown to Success,
omit failure quarantine, and reject valid custody. Source-correspondence tests
remain separate from these intentionally mutated proof snapshots. The runner
does not run CPU runtime tests or hardware tests; the preceding safety packet's
CPU results remain evidence of that earlier signed source only.

## Results

All fourteen phases of `retained/signed-campaign-v3` pass from signed
`7185cc6fba1ac7b154469881b1e2df17aec25b6f`:

- Opening, relocated and closing whole-root positives each verify 53 obligations
  with no diagnostics and unchanged default solver limits.
- All four focused mutants produce identified logical verification failures.
- Twelve negative-classifier calibration groups, eight source-correspondence
  calibration groups, the source-body check and workspace formatting pass.
- Opening and closing pinned Verus closure checks match 190 files / 129019839
  bytes; both source brackets match 5966 inputs, bound to signed blobs.

All 372 artifacts are retained byte-exact, including eleven numbered development
attempts and both incomplete earlier campaigns. Attempts 01/02 failed validator
postconditions; 03 passed focused validation; 04/05/07 failed parsing, typing or
attribute support; 06/08/09 hit solver limits; 10/11 passed the full 53-obligation
root. Attempts 01-09 used `prove-original.py`; later attempts retain their runner.
The first signed campaign was deliberately interrupted during its first mutant
to strengthen signed-source binding. The second stopped because the classifier
rejected a function-specific Verus selection note; the final runner adds a
tested exact allowlist. Neither incomplete campaign is an accepted campaign
pass, and interrupted/resource-limited attempts are not logical negatives.

The collector requires the exact 41-record attempt/phase roster, terminal
process groups and complete development source snapshots before retention.
It removed only the fixed owned scratch tree, accounting for 10309632 allocated
bytes, and rechecked path absence and retained hashes. No remote resources were
created. `retention.json`, `cleanup-before.json` and `cleanup-after.json` record
the observations; `SHA256SUMS` seals this packet.

This is scoped development evidence. Real Context construction, evolving journal
observations, production adapter composition, broader mutation/admission closure,
native out-of-order/overlap qualification and matched performance remain open.
