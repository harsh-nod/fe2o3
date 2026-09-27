# Authentic root argument initialization — 2026-09-27

This checkpoint initializes arguments through the original private root source,
owner, graph and recipe-credit counter. It is an internal compiler preparation
step, not a public assembly-authoring API or complete production route.

## Implemented boundary

The one-shot continuation uses the same pending root prefix and SSA state.
It initializes index slots, then slice slots, then advances the argument counter
to one, preserving the original three-statement order. State becomes terminal
before fallible work, so refusal or unwind cannot restart initialization.
The old factory refuses a non-dormant pending owner.

The checkpoint remains DATA. It does not assert that earlier Option, enum,
scalar, provenance, allocation or capability-effect writers ran in ordinary
source order. Those stages still need their authenticated, retained-state
continuations before the final joint bounds driver can be connected.

The typed resource additions account for 25 new rows and eight constructor/
guard deltas without changing the inherited 8,192-byte policy. Actual-source
qualification reran the shared pending-header growth and existing rich/local-use
observers; earlier measurements are not reused as evidence for the new layout.

## Qualification

- Fifteen new controls passed: fourteen argument-initialization controls and
  one dormant-owner control in the existing root-prefix module.
- Full regression passed 331 model and 2,957 backend tests, with 189 backend
  tests still ignored; backend build and 83 JavaScript controls passed.
- The genuine-source CPU ladder passed five actual rustc sessions, two positive
  sources, 36 positive numerical runs, two callback numerical runs, 32 request
  refusals, one source refusal and two ordinary-route refusals.
- Identity, swapped-input, callback-error and callback-panic sessions each emitted
  the new actual initialization marker exactly once: locals=31, operations=1,
  next_value=0, next_argument=1, before_writers=true, with reentry/error/panic
  controls passing.
- Both ordinary compilation ladders passed. All 38 lossless observation bodies
  and 52 artifacts match the preceding lazy-proof checkpoint exactly.

Regression receipt:
`730715d7b653bbdf4cd4579f5353733acfd48e5b66a5ec7bdf9cd518b9791ca2`.

Genuine-source receipt:
`9ac648203bdbb3a6e1a507c8adccd70ed4cd422c30cfd8e64a205e0376e10615`.

Genuine-source observation:
`6ae2615499c5e626fceb0376be2d421ebddd7a744bdbb6e25af28a7dc5b9d223`.

Normal-ladder receipt: `b3104fb2f78f395be982249afcd88aeca0c7915977e3a3cbccdf938fe0b5d65a`.
Normal-comparison receipt: `784ee5b801c1cdae3ce0420587a9cb5c5f0258753001886b849f9f7254b5a8d1`.

## Remaining work

The genuine-source ladder is numerical CPU evidence, not ordinary-route
qualification of this new continuation and not native GPU execution. Synthetic
foreign-owner, ledger and failure controls do not replace full genuine-factory
cutpoint coverage. The original rich observer's earlier enum-first ordering is
not proof of the ordinary Option-first prelude.

Physical proof-payload retirement through the enclosing factory's postflight,
drop before refund, retained model storage, all preceding writer chronology,
later strided/FIFO/GetMut writers, the joint bounds driver, final counters,
mandatory verification and production routing remain open.

No public capture gate or global compiler pin changes. Accepted broad exits
remain **M1/V1/V2/U1/U2/U3 (6/18)**.
