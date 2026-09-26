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

The mutation cases skip successful settlement, promote Unknown to Success,
omit failure quarantine, and reject valid custody. Source-correspondence tests
remain separate from these intentionally mutated proof snapshots. The runner
does not run CPU runtime tests or hardware tests; the preceding safety packet's
CPU results remain evidence of that earlier signed source only.

Signed campaign results and exact-owned retention/cleanup will be recorded here
after the campaign finishes. Development failures are not qualification passes.
