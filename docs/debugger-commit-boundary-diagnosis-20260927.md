# Native debugger commit-boundary diagnosis — 2026-09-27

A fresh bounded native attempt reached a loaded-runtime commit guard and refused.
It did not produce an accepted stopped-wave capture. The attempt is consumed;
neither its lease nor its failed receipt is reusable as capture authority.

## Observed sequence and cause

The retained complete diagnostic records fourteen commands. Selection completed,
continue began, and the owned host advanced through s1/p1, s2/p2 and s3/p2.
A successful runtime observation followed, then refused(15), commit-site=7.
There were no later capture commands or accepted physical stop records.
Three printed copies retain the same first refusal; they are not three distinct
localized failures.

Site 7 is the no-path generic commit's unloaded-entry maintenance predicate.
The accepted loaded-runtime ACK retires that entry epoch. GDB subsequently calls
commit_resumed without a new resume, so that unloaded-entry-only exception no
longer applies. Exact retained callers and adapter sources join the selected
build ancestry.

This is a real effect boundary: the AMD commit calls its beneath target and then
requires forward progress. Skipping the guard or rearming the unloaded epoch
would lose the ownership distinction. A separately authenticated loaded-host
maintenance transition is being implemented. The proposed initial scope ends
at the first runtime callback; it is not yet an end-to-end capture fix.

## Cleanup and postflight

The inner and outer scope owners joined their cleanup acknowledgements, reaped
the selected family and adopted inferior, and observed the owned scope empty.
Root independently confirmed that all four selected process generations and the
owned cgroup were absent. This is selected-family cleanup evidence, not a claim
of OS-global process or GPU quiescence.

The failed outer runner intentionally has no automatic after-snapshots.
Root separately verified the unchanged compiler source census and all 1,009
selected input and 17 tool identity/content bindings, including the request and
captured streams. This manual postflight does not reclassify the failed command
as passed. The original protocol result was Incomplete; a later publication
Deadline and empty-controller-output framing error are not the primary cause.

Failed native receipt:
`a3c5de301d7c2e052205f38d04af7bd56ad2dda572ca90f98da626c7d9f187b3`.

Root attempt audit:
`5d47243826bb4d39145ee0e530448b1d27fcd41c85e6c01d42a1b614bf8ea10c`.

Manual postflight:
`0f50aab020ccddfe98a6942824ba6fef87f3e2aa5f05179330aae0f65982dfc7`.

Complete decoded diagnostic:
`4b43b2b137b808286cb85b9176e017403b32a8438c2b5670a9b69e354d654bc1`.

A successor still requires reviewed source, actual-body controls, a full debugger
build and layout check, updated deployment/replay bindings, fresh coordination,
a new bounded attempt and accepted cleanup/postflight. No capture gate, global
compiler pin or broad milestone is advanced here. Accepted broad exits remain
**M1/V1/V2/U1/U2/U3 (6/18)**.
