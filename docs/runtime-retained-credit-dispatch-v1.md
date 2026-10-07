# Retained Credit Dispatch V1

Status: successor source controls implemented; fresh logical qualification is
pending. Neither this document nor a refreshed source capture closes A1, A2,
A3, native execution, or HIP/HSA parity.

This required input to `run-retained-credit-dispatch.py` was absent from the
production-main port. The historical document remains available in commit
`2761f359468d60c6814e265ddaa21bff50b0c499`; its development and signed-campaign
results are not results for this successor.

## Exact Component Boundary

Five production methods and the Verus root expand the same executable bodies:

- Context looks up the exact device account and allocation credit. Both absent
  selects ordinary mode, not credit evidence; one-sided presence refuses.
- Runtime checks the complete device identity, then General/Composed shape.
- General accounting checks the retained token, variant, identity, reached lock,
  and Independent/Domain observer.
- Domain accounting checks root identity and the reached lock, then passes the
  original ancestry/profile/record fields to its observer.
- KFD request accounting checks the typed account identity and canonical charge.

The fourteen-file proof closure includes those five bodies and the existing
record, observer, arena, resource-vector and request-charge definitions. The
Domain proof remains an exact prefix. All 41 expected obligations are unchanged;
they are not 41 independent whole-runtime properties.

Scalar identity, raw lock-result state and exact-key lookup are explicit
projections. This is not a refinement of live Arc addresses, Mutex operations,
HashMap implementation, concurrent freshness, native custody, release, compiler
lowering, ISA execution, or GPU behavior. No new opaque fields or success
assumptions have been added to these projections.

## Successor Correspondence Audit

The predecessor was recovered from Git objects at
`2761f359468d60c6814e265ddaa21bff50b0c499`, not inferred from the current files.
Its two complete captures reproduce the old checker pins exactly. The successor
is based on `64ba4c359baa9e62d98132dcc7fa60d7ab332666` with the separately reviewed
cooperative-fairness changes and the duplicate-allocation abort test correction
included. That correction shares the existing bounded child harness and changes
only tests and `cfg(test)` module visibility; the production abort is unchanged.
The final capture also includes four Clippy-cleanup files: owning result/function
type aliases, two explicit inline-owner lint rationales, and equivalent
short-circuit condition chains. None changes the correspondence files or the
complete identity macro pinned below.

| Capture | Predecessor | Successor | Unchanged / Changed / Added / Removed |
| --- | ---: | ---: | --- |
| Runtime Rust routing capture | 413 | 507 | 337 / 76 / 94 / 0 |
| Dispatch production/dependency capture, excluding proof root | 760 | 1019 | 619 / 141 / 259 / 0 |

The broad changes include real async scopes, native-family adapters, epochs,
copy/graph/cancellation behavior and engineering target additions, not only
module movement. Capturing these bytes does not verify their behavior.

The narrower executable correspondence is independently checked:

- Routing's complete production owner, two shared macro files and proof root
  are all byte-identical to the predecessor. Their four-file digest is
  `9a672158fd53a9450a058832481b7c9198a2e05703ad88df1113b01d2ed320bd`.
- All fourteen credit proof inputs, four complete credit owners, the allocation
  witness and the resource-kind implementation are byte-identical. The fifth
  owner, `context/allocation_admission.rs`, has only the added panic-policy
  comment; its executable tokens and declaration are unchanged. The resulting
  21-file correspondence digest is
  `be7346ca406901c79719d6223116e6a416e36d17ebd502383f9ad1a3b074d611`.
- The complete 611-byte `runtime_id` macro, including derive/equality, fields,
  getter and constructor, is byte-identical. Its digest is
  `f4d9eebd7707202b1e85c6c0cea32c623fbd80846865b0cc4076c6bf7f788c8a`.
  Device and allocation identities still instantiate this same macro.

The full source gates remain closed exact captures, including all new files.
Independent correspondence gates now refuse changed owner schemas, getters,
typed adapters, shared bodies or proof assumptions even when a test deliberately
rebinds the surrounding capture hash. Existing shared-entry and Domain-prefix
controls additionally rebind that correspondence hash to continue exercising
their structural checks. These are source controls, not solver executions.

## Required Qualification

Source and synthetic runner controls, with no compiler or solver:

```sh
python3 -I -B crates/fe2o3-runtime-model/verus/test-retained-pair-routing.py
python3 -I -B crates/fe2o3-runtime-model/verus/test-retained-credit-dispatch.py
python3 -I -B crates/fe2o3-runtime-model/verus/test-run-retained-credit-dispatch.py
```

Fresh routing qualification still requires all three full 4-obligation positives
and eleven actual-body logical negatives under its unchanged controller.
Fresh credit qualification requires the unchanged 33-stage portable campaign:
three full 41-obligation positives, 25 family-bound logical negatives, signed
source verification, source/runner controls and two release-closure checks.
Historical discovery counts and selector diagnostics are acceptance expectations,
not proof that these successor runs occurred.

```sh
python3 -I -B crates/fe2o3-runtime-model/verus/check-retained-credit-dispatch.py \
  --campaign --verus /absolute/pinned-release/verus --output /absolute/fresh-evidence
```

The runner requires a clean tracked checkout signed by its existing pinned
ED25519 signer, the exact 190-file Verus/vstd/Z3 release, Rust 1.97.1, and its
unchanged host-tool continuity checks. Output must be fresh and outside the
checkout and verifier. Every negative must join its expected postcondition to
the actual changed body; compiler errors and timeouts are refusals, not logical
negatives. A normal zero exit and complete `accepted: true` receipt are both
required. Failed logs and incomplete receipts must be retained.

No logical campaign, Rust suite, native run, freshness/conservation theorem or
performance result is supplied by this source-only successor update.
