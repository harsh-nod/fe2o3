# Lazy fixed-proof ownership qualification — 2026-09-26

This checkpoint adds a private source-ordered proof owner. It does not redirect
ordinary compilation, complete the bounds driver, or expose a public authoring API.

## First-use timing and retained state

The owner keeps the original exclusive preparation-resource loan. Other
terminators, fully literal bounds pairs, and non-fixed boundaries do not eagerly
construct assertion proofs. Canonical assertion shape is checked first; length
lookup precedes the conditional index lookup. The first fixed-bound event
consumes the original loan once and creates the existing strict proof resources.

The state moves through Pending, Building and Ready. Building owns its checked
index and any initialized caches before later fallible work. Refusal or caught
unwind leaves those allocations attached if the caller retains the owner.
Strict-constructor refusal is terminal and creates no proof buffers or replacement
adapter. Ready reuses the existing fixed-guard algorithm and caches. Duplicate,
skipped or failed visits cannot restart the component.

The preparation facet preserves the qualified reserve policy. Its retained push
takes a caller-owned Option<T>: frame admission, push work and exact reservation
all happen before taking the candidate. Earlier failure leaves the same object
in that slot; successful transfer does not clone it or run its destructor.
External vector/candidate provenance remains the caller's responsibility.

## Typed accounting and test corrections

New selected-source frame rows name copied inputs, ledger values, borrows,
nested model accessors and complete return transfers. The checked equation is
locals + twice the return representation + twice its Result representation.
There is no new opaque fixed allowance standing in for omitted locals. Existing
strict/evaluator/cache policies remain unchanged. This is logical source-level
accounting, not native stack, RSS, allocator-internal or GPU performance proof.

Eighteen ownership/ordering controls and eight typed-frame controls passed.
They cover the original frozen fixed-guard oracle, first-use timing, terminal
failure, retained partial state, candidate identity/destructors, caught unwind,
foreign source/ledger refusal and measured exact/one-short limits.

Qualification found and corrected an explicit source-type import and the test
callback's Copy-result boundary. Three deliberate sticky-denial tests now require
the existing enclosing Resource(Accounting) refusal, while retaining all inner
payload and cleanup assertions. Four source-surface expectations use the exact
formatted type spellings. Failed runs are preserved; no runtime denial check was
weakened and the ordinary algorithm was not changed.

## Qualification

- 26 new controls passed.
- 331 model and 2,942 backend tests passed; 189 backend tests remain ignored.
- Backend build and 83 JavaScript controls passed.
- Both ordinary compilation ladders passed. All 38 lossless observation bodies
  and 52 artifacts match the preceding policy/comparison checkpoint exactly.

Regression receipt: `b6e603ed3582f5a4f0780839843f60c3a44dfb863ee237323351c7535f8277e3`.
Normal-ladder receipt: `edc632976af2082a866b1c802eef86ad571bf96bd1b7dd7fc14974d212dcbab7`.
Normal-comparison receipt: `6dcd44c2f2fcc07c8f6faeee3fd3efe5e59962f8a2793f58cb700690cac31e45`.

## Remaining integration

NonFixedBoundary is DATA, not evidence that dynamic slice decisions ran. Fixed
events return guard-authentication data, not completed bounds operations. A
single joint source-ordered driver must finish each intervening stage before
advancing. Actual producer custody, the final argument/extent counter and
mandatory verification are still required.

The active proof borrows source/resource views. The enclosing factory still
needs physical buffer retention through its original postflight and drop before
refund. A separate one-way inert-payload retirement design is being implemented;
it is not part of this checkpoint. No live proof, raw Budget or authenticated
all-producer continuation escapes here.

Authentic argument initialization, preceding effect chronology, later writers,
the joint bounds driver and production routing remain separate work. No target
dispatch, stopped-wave capture, public capture gate or global compiler pin is
changed. Accepted broad exits remain **M1/V1/V2/U1/U2/U3 (6/18)**.
