# Conditional Compact Replay Framing

Date: 2026-09-27. Base: `bc9cb1658d92f7bf20c7a94698d8bde73d35ac74`.
Integration prerequisite for [issue #272](https://github.com/harsh-nod/fe2o3/issues/272).

**This adds restart framing and retention in the native Cargo continuation.
It does not complete conditional restart recovery, activate the production
profile, complete an M0-M7 milestone, or add protected/GPU qualification.**

## Implementation

The existing native compact codec now shares its private framing implementation
with a distinct conditional V5 transcript. The native magic, version, checksum
and identity domains remain unchanged. V5 uses its own magic, version, domains,
outer length limit and public coordinate types. Typed entrypoints choose the
schema; untrusted bytes cannot select or downgrade it. Both families retain the
same derivation-capable Worker tail and reject legacy tails.

The conditional constructor reads the actual retained conditional finalizer,
source, binding and transaction coordinates only after prepayment. It neither
clones the source/artifact nor fabricates a consumed publication. Shared decode
keeps one owned canonical byte buffer, with fallible allocation and bounded
resource quotes. The conservative scratch frame now includes the input adapter;
wire compatibility does not imply an unchanged logical scratch quote.

Cargo's existing native continuation now mandatorily retains the transcript
beside its artifact, compiler receipt and original readiness budget borrow. It
reserves the additional transcript storage on that account and includes it in
the retained floor. Revalidation charges for fixed coordinate comparisons and
joins the transcript to the retained artifact before existing readiness checks.
No detached authority, fresh budget, or new production selector is introduced.

The transcript remains inert. Checksums and coordinate comparisons do not
authenticate compiler execution, replay semantic proofs or Worker exchanges,
or authorize publication, loading or launch. Artifact/provider extraction keeps
the existing separately bounded artifact domain; the codec's logical storage
quote is not a whole-process, allocator-capacity or RSS bound.

## Validation

Pinned `nightly-2026-04-03`, offline dependencies, HIP disabled, one bounded
Cargo command at a time, compiled inputs frozen during each command:

| Command / Filter | Result |
| --- | --- |
| Finalizer library: `native_worker_compact_replay` | 19 passed |
| Finalizer library: `native_worker` | 57 passed |
| Complete finalizer library | 195 passed |
| Compact replay doctests | 7 compile-fail and 1 compile-pass passed |
| Cargo native continuation account-floor test | 1 passed |
| Four-package `--all-targets` check | Passed; warnings remain |

Focused selections overlap the full library suite; counts are not additive.
The all-target check covers `cargo-fe2o3`, `rustc-codegen-fe2o3`,
`fe2o3-hsaco-finalize` and `fe2o3-compiler-execution-client`.

Seven new codec tests cover independently encoded fixtures, cross-family
rejection, exact coordinates, every truncated prefix, malformed tails, domain
separation, maximum provider/option counts, exact/short/unpaid resource limits,
original ledger retention, prepayment before observation, and unwind cleanup.
The compile-pass doctest controls the V5 outer-identity API; compile-fail cases
reject cloning, type downgrades and V4 outer-identity substitution. These are
component and type tests, not a fabricated positive protected execution.

Rustfmt and whitespace checks passed. This patch adds no unsafe code; the
finalizer crate denies unsafe code. The preexisting unsafe inventory failure
recorded in [the preceding checkpoint](shared-compiler-slot-admission-20260927.md)
was not waived or rebaselined, and was not rerun here. All build/test processes
terminated and private `/tmp/fe272replay.EnBAfJBc` scratch was removed.

Logs under `/home/harsh/work/fe2o3-issue272-production-next-evidence-20260921/`:

```text
4b95f920c9ca841bba78ab9e48558615df7cfc6ee3fe125ea91475109c790aa0  conditional-compact-replay-tests-r1-20260927.log
999f5975b44c0cfd0a5f84fb9eefea1669b132c0c5577138ecbfdcabe95b383c  conditional-compact-native-regressions-20260927.log
7105f348f7f326afcc0936d239999a69bfbdd807e83774053580488d4da77d55  conditional-compact-finalizer-lib-tests-20260927.log
ce189dc22c97fb75c376f678142ec3aeba4f9359762d23bf9f3ddd413809e19f  conditional-compact-doctests-20260927.log
7391c4a33221da8bb02b38b94824a0b5ef8cac85ceaecc23599c3aebb4c561cc  conditional-compact-cargo-continuation-tests-20260927.log
82b5003203911e8779faa7cba37b988cf6efebf6f238ef70cfd06b69eefc5070  conditional-compact-all-target-check-20260927.log
```

## Remaining Work

Conditional restart must recover the actual V5 source/final graph/catalog from
independently admitted policy, rederive occurrence coordinates, reconstruct both
Worker exchanges, rerun shared finalization and compare exact artifact bytes.
Recovered transcript custody must remain distinct from fresh consumption.
Durable publication, the parent/profile/broker/driver migration, sealed verifier
and generated host integration, positive protected execution, applicable machine
and numerical refinement, the 47/47 target-matched matrix and release gates are
still open. Nominal V5 structural publication is not a conditional replacement.

Native agent spawning hit the thread limit; this checkpoint was implemented and
reviewed locally. SSH aliases and Git fetch failed DNS from the execution
environment; the public issue remained readable through the browser tool.
No remote job or file was created. Push and issue-update outcomes are reported
separately, not implied by this local evidence.
