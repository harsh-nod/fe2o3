# Authentic fixed-query comparison checkpoint — 2026-09-28

The test-only original fixed-query oracle and candidate now consume the same
authenticated source CFG and original resource budget. The original proof owner
remains alive while the candidate runs; complete bounded checked rows and ordered
cache entries are compared through live, retired and final observations.
This is an internal comparison checkpoint, not ordinary production admission.

## Implemented

- Independent original queries retain their own expected content; candidate
  content never supplies the expected answer.
- Closed limits cover 32 source blocks / checked rows and 128 entries per cache.
  Refusal or unavailable data is not equality.
- Three original and three candidate content reads validate exact source
  coordinates, fixed-index extents, cache keys, values and insertion order.
  Allocation identity is checked within each owner, not between implementations.
- Owned allocations remain charged through postflight; payloads and owning
  errors are destroyed before refunds, including sticky-denial paths.
- Compare, callback-error, callback-panic and original-query-panic observations
  use the same original budget. Invalid custody is not repaired.
- The older observation modes and production route remain unchanged. Literal
  skip classification does not claim independent literal-value equality, and
  logical storage rows are not a physical stack/RSS measurement.

The local catch/resume path preserves its panic payload. This does not promise
that every outer wrapper retains that payload through all later postflights.

## Qualification

The 15 new component controls passed. Full CPU regression passed 356 model tests
and 3,174 backend tests (197 ignored), followed by backend/extractor builds.
Receipt:
`1990b8b3081201061f4414f7892e6c21f016c44c6d80445bf49ae78a351be799`.

The existing genuine-source ladder also passed five fresh rustc sessions:
identity, swapped return components, wrong launch, callback error and callback
panic. The two positive helper cases completed 36 independent numerical CPU
runs; the ordinary unsupported production route continued to reject.
Receipt:
`6201a5c2880b4b9f036e40df65a86e9d79991f67a28a5a06eb1bec713bdb7390`.
Complete ladder report:
`35f31d77b139e75c49607888964d79319f8e9c9a0cb672aae05d33bdecfbab49`.

These real-source prefixes contain zero Fixed queries. Their successful
comparison establishes the genuine connection and empty-domain behavior,
**not genuine nonempty Fixed-proof coverage**. In particular, the original-query
panic mode correctly skips when no Fixed query exists.

An additional isolated Rust array-bounds fixture was attempted. It was rejected
before nominal-owner materialization with `BF16 semantic Assert is unavailable`;
its comparison hook never ran. That failed attempt is retained as a dependency
finding, not reclassified as a positive comparison:
`17e9c5a7e6cfbb723ef1107d4efc66f18bb2fee82e43bf328a1b75a1f14e7ea8`.
The experimental fixture is not part of this delivered source slice.

The existing fixture lockfile was refreshed offline for two missing local
workspace dependency edges. No package version changed and `--locked` was not
weakened.

## Remaining work

Genuine nonempty Fixed coverage, the source-ordered Option-first prelude,
remaining argument writers, joint bounds ownership, mandatory verification and
production routing remain open. An earlier source-only observation cannot
substitute for a materialized nominal-owner comparison.

No GPU execution, native debugger capture, complete physical-register kernel,
new public route or global compiler pin is qualified here. Accepted broad exits
remain **M1/V1/V2/U1/U2/U3 (6/18)**.
