# Fallible Owner Construction Correspondence

This developer checkpoint connects the journal, stable-read and producer-read
constructors to independent logical execution. It extends the
[Unknown disposal checkpoint](../dev-owner-disposal-2026-09-23/README.md).

## Scope

- Ordinary Rust and actual-owner Verus methods instantiate the same nine bodies:
  public forwarding, error propagation, reservation, three initialization loops,
  and the three owner constructors. Public Rust signatures remain unchanged.
- The journal validates generation before allocation/writer capacities. The
  stable-read constructor validates read capacity before calling the journal;
  the producer-read constructor delegates to that stable-read constructor.
  All three error variants and their precedence are preserved exactly.
- Journal, stable-read and producer-read construction make up to 7, 10 and 13
  ordered reservation attempts respectively. Each observation contains its site
  and requested capacity. Execution stops at the first failed attempt; an empty
  or exhausted outcome sequence supplies failure. Unused outcomes are unchanged.
- Actual and logical construction execute independently. The logical executor
  does not invoke the shared production bodies. Both preserve arbitrary incoming
  attempt prefixes, subject to append headroom, and leave outcome sequences intact.
- Success establishes complete owner contents: vacant slots, reverse free stacks,
  zero counts and watermark, and initial read incarnations. Successful producer
  construction establishes the producer invariant. Issued custody additionally
  requires the supplied ghost storage to satisfy its admission predicate.
- Constructor-derived executable witnesses reach enrollment, registration, Begin,
  retained reads, Success/NoEffect settlement and release, and Unknown disposal.
  They start from paired constructor results rather than synthetic owner contents.
  These are finite witnesses, not a general constructor-origin trace theorem.

Eighteen added CPU tests compare candidate constructors with frozen baselines from
`f1022d0d3d22f4773a6b40d8ed1e040051ce512f`. Baseline changes are limited to
prescribed method redirects and observable reservation hooks. Tests cover all
13 explicit failure positions, truncated/empty and surplus outcome sequences,
validation precedence, typed site/capacity order, full initialized contents,
surplus capacity and subsequent lifecycles. Buffer addresses and capacities are
checked separately for stability within each owner, not compared across distinct
allocations. Injected failures do not qualify native OOM or panic cleanup.

Initialization remains linear in admitted capacities, not allocator-provided
spare capacity. With allocation capacity A, writer capacity W and read capacity R,
successful construction performs 2W+5A initialization pushes for the journal,
2W+6A+2R for the stable owner, and 2W+7A+4R for the producer owner. These structural
counts are not elapsed-time or HIP/HSA performance measurements.

Two inherited proof-only changes isolate an existing release lemma and decompose
an existing retirement fixture. Their original contracts and execution order
are preserved; no solver resource limit was raised. The added retirement helper
accounts for the disposal regression's increase from 1,085 to 1,086 obligations.

## Qualification

Source: `8a522fde853e2683aa7ee2b283dd00787df563ff` (also recorded in `SOURCE`).
The source-bound campaign completed with 422 authenticated input files:

- Paired whole-root proofs: 1,134 verified, zero errors, before and after controls.
- All 74 scoped mutations failed logically as intended, without frontend errors,
  arithmetic failures, timeouts or solver resource exhaustion.
- Raw-owner regression: 370 verified, zero errors.
- Unknown-disposal regression: 1,086 verified, zero errors.
- CPU suite: 1,014 unit tests and 27 doctests passed; 18 unit tests were ignored.
- Formatting, warnings-denied all-target Clippy and the optimized test build passed.
- Recorder tests rejected 29 malformed/conflicting operations, eight malformed
  retirement diagnostics, and 24 malformed constructor diagnostic, independence
  and source-authentication cases. All 74 mutation anchors were authenticated.
- Both pinned Verus distribution checks matched: 190 files, 129019839 bytes.

All 86 serial commands have terminal receipts and controller acceptance. Complete
campaign replay left all 262 record files byte-identical. Raw stdout/stderr are
packaged unchanged. The packet contains 262 record files and 266 files overall;
`SHA256SUMS` covers the other 265. No exploratory or interrupted command is counted
as qualification.

## Replay

Run `crates/fe2o3-runtime-model/verus/check-owner-constructor.py` from the `SOURCE`
checkout with `--repo`, `--verus`, `--output` and `--target`, using fresh output
and owned Cargo target directories. The recorder authenticates frozen baseline
reconstruction, exact Rust adapters, independent logical execution, include
envelopes, the two reviewed inherited proof changes, the pinned roadmap and the
closed source roster. It brackets source hashes and the pinned Verus distribution.

Use `--stop-after <case>` and `--resume` with the same source and arguments for
bounded checkpoints. An incomplete prefix is not qualification. Resume accepts
only exact-source, controller-accepted records under an exclusive controller
lock. Interrupted or unaccepted commands are not silently retried or discarded.
Controller acceptance records normal terminal cleanup and semantic validation;
it is not independent execution attestation.

The packet auditor reads source from Git objects with `--repo <git-repository>`;
`--selftest` additionally exercises rehashed-corruption controls. It checks record
integrity and semantics, but does not rerun Verus or verify commit signatures.
Its 33 rehashed packet-corruption controls cover source identity/roster, proof
scope and results, terminal receipts, controller acceptance, diagnostic policy,
CPU inventory and artifact closure, including unexpected empty directories.
Two roster controls preserve cardinality and require the exact roster rejection.
A separate source-document mutation must fail the explicit document pin. The
selftest also runs a positive audit against a bare Git repository without a source
checkout; that is distinct from running all corruption controls against bare Git.
Historical execution, staged mutation bytes, environment and process-group
disappearance remain recorder observations, not independent auditor attestations.

## Limits

This is normal-return observation/content correspondence, not physical Vec
refinement. The native allocator is a separate, source-pinned Rust adapter.
Allocation success, actual buffer capacity and addresses, nonallocating pushes,
partial-object destruction and panic/unwind cleanup are not formally established.
The logical reservation model does not model partial initialization or destruction
order. Context-generation freshness authority, general lifecycle reachability,
remaining inspection/trait correspondence and native pending-consumer admission
remain open. The CPU environment is not a hermetic build attestation. No GPU,
XGMI, HIP/HSA timing or parity claim follows from these records; Gate 1 remains open.
