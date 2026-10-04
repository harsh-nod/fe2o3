# Exact debugger command-line refusal — 2026-09-26

The diagnostic controller added after the earlier generic setup refusal was
qualified on mi350, then used for exactly one fresh root-owned attempt.
That attempt **failed**; there is no physical capture or successful stop sequence.
Public bindings remain disabled and broad accepted exits remain **6/18**.

## Prerequisites and attempt

The private controller passed 58 library and 46 transport tests, strict Clippy
and build. The bound process-family implementation passed 32 Rust and 191 Node
controls, actual build/deployment and a read-only prerequisite replay.
A missing test-only fixture import in the first family CPU attempt was corrected
in a new immutable overlay; that failed attempt remains retained.

Root rehashed the selected inputs, obtained quiet-writer coordination, checked
compiler quiescence, and retained the original exclusive native lock throughout
one attempt. Resource limits, exact executable/argument bindings and independent
cleanup checks were unchanged. There was no automatic retry.

The actual failure diagnostic reports:

- Stage: **Cmdline**.
- Child PID/stamp observed and debugger custody acquired.
- Most recent pre-read child wait observation: Running.
- Command-line read: **zero bytes**, exact expected-argument equality: false.
- Reader threads started: **zero**; debugger commands sent: **zero**.

The controller already performs an EOF-based bounded read, not a read limited
by the proc file's reported size. An early child exit between checks is a
hypothesis, not an established cause. The earlier Running observation does not
prove that the child remained live at command-line EOF. Existing teardown loses
the detailed exit status and unread stdout/stderr when failure precedes readers;
bounded failure-only diagnostics are the next step. No argv or identity check
is relaxed.

## Independent cleanup and evidence

The terminal audit joined family and owner pidfd-exit/ECHILD/stream-EOF receipts,
the terminal acknowledgement and an empty/removed scope. Root independently
rehashed all **186 selected inputs and 25 retained products**, verified the
owner, client, controller, debugger and launcher PIDs absent, and checked the
exact cgroup absent. No process-family kill was needed.

- Failed runner receipt: 173,707 bytes,
  `ff9e84439d76617d7dea592e3edebe2561b0861fd53ef7ed0c07aa93f38783a8`.
- Terminal audit: 17,025 bytes,
  `b9122b389f6d830aac1c55132b4e7b41fcfe33823c9ffac3df2eff1546fd82de`.
- Controller diagnostic: 577 bytes,
  `7adcf8d95d58cf86756b1bf0d72c2b85f6b8ee2b7fba079fb4710058f7a19c2c`.
- Independent cleanup audit: 1,281 bytes,
  `3ebf36b1a673ca614d21e52ed4e65316dd9f5ca8ea85db013016076af0c2427d`.

The controller's local pre-reader cleanup still reports incomplete local stream
evidence; that is distinct from the independently joined outer family cleanup.
Cleanup does not establish physical capture, rollback, GPU dispatch, debugger
acceptance or source/runtime authority. The failed output and consumed
coordination remain immutable.
