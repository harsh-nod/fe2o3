# V6 Copy-Source Read Leases

CPU/test development above canonical base
`0c1bb9a51d177524ef95a961261137f8ec97410b`. Accepted checkpoints remain Native
R125 CPU/test, Admission R118B C1-C3 and Resources R116/V3. This packet does not
complete V6, A1/A2, issue #182 or HIP/HSA parity.

## Change

The bounded owning read-lease model prevents mutations and retirement from
bypassing source custody. Built-in same-device, graph and peer copies bind exact
original source records, byte ranges, epochs/lineages and fresh lease incarnations
to their consumer's original Context submission ID. Exact quiescence releases the
reader before writer settlement and notification; ambiguous execution retains
both owners and their credits. Multiple readers coexist, including across streams.
Prepared source identity is rechecked even without the optional journal.

Generated retirement checks reader exclusion before native DATA/submission
retirement and again before whole-shell disposal. Normal Unknown settlement and
destination disposal cannot discard unresolved source custody. Ordinary prepared
graph actions are boxed during preparation to avoid inflating generated/join
node storage. See the [contract](../../runtime-context-copy-read-leases-v1.md).

## Qualification

Frozen-source GNU and scoped musl each pass:

| Package | Passed | Ignored |
| --- | ---: | ---: |
| fe2o3-runtime | 1023 | 17 |
| fe2o3-host | 271 | 4 |
| fe2o3-runtime-model | 788 | 2 |

Each target has 2,082 passes, 23 ignored and zero failed, measured or filtered
tests. Matching complete rosters have 2,105 named entries, retaining every prior
entry and adding 23 passing tests: 13 copy integration, 9 model and 1 generated
shell retirement test. Strict all-feature/all-target Clippy, scoped formatting,
no-default checks, unsafe-source policy (5 passed, 1 ignored) and 87 doctests pass.
All sixteen serial qualification records exit zero. No full frozen campaign
failed; exploratory results below are separate from this archive.

The calibrated parser accepts both immutable baseline rosters and rejects ten
malformed-log mutations. GNU/musl roster SHA-256 is
`5d4ac09cdcd395283978eb9d20ee29bb3b3d83ede81b81f40fc01e6fe27c31ae`.
All six executed unit-test binaries are hashed and rechecked after quality gates.
Raw records preserve commands, UTC start/finish times, outputs and exit statuses.

Before freezing, 13 focused copy integration tests and 9 read-lease model tests
passed. Exploratory tests corrected an expired drain deadline and an invalid
journal-disabled fixture, which had removed the journal after enrollment.
Clippy found graph enum growth from the new source snapshot; the final layout
boxes the outer ordinary prepared action. Exploratory failures were corrected
before the archived full campaign, rather than represented as passing results.

The deferred-copy test backend applies bytes only at modeled completion, or at
an explicitly scripted initial Quiescent failure. Exact unpublished cancellation
discards the queued copy without changing destination bytes. Rejected, terminal
and panicking submissions have separate byte/custody assertions. Tests cover
local/peer exclusion, multiple readers, cross-stream destruction, all generic
terminal observation paths, dropped tokens, callback panic, repeated observations,
stale prepared IDs and backend-handle reuse, malformed handles, source-marker
substitution, capacity and Pending/Unknown source rejection, rejected/terminal
wait, and exact retained/quarantined credit accounting.

The model suite covers rejection atomicity over complete state/output snapshots,
same-allocation overlapping range batches, repeated consumer acquisitions,
incarnation/slot reuse, Success/NoEffect epoch gaps, writer/retirement exclusion
and a deterministic 4,000-step independent count, held-reference and slot-partition
oracle with stable backing storage. A compile-fail doctest rejects mutable access
to the enclosed writer journal.

The source manifest covers 24 changed files: 21 Rust and 3 docs. Its SHA-256 is
`2c96224c29bf5ab32b86503000a9343c49c166967c5749a36a0f11db110cacf1`.
The exact base-relative patch has SHA-256
`31153af06a7ccccf9b02d599659f0a81137cf30c35d0859541141d526d220c5d`.
Independent read-only reviews checked the production integration, model, test
oracles, documentation, complete unit logs, source coverage, patch identity and
binary/roster hashes. The seal checks serial record closure, source and binary
identities and the exact patch before manifesting every artifact. Sealed evidence
is immutable.

## Limits

No native GPU, formal solver or matched HIP/HSA benchmark was run. No remote
files or jobs were created. GNU/scoped musl coverage is runtime/host/model, not a
fresh complete lower-KFD campaign. Scoped musl disables HIP linkage through
`FE2O3_HIP_SYS_DISABLE=1`; unrestricted musl/HIP linkage is not qualified.

The generated reader test injects an inert model lease against private shell IDs
and exercises the final whole-shell preflight/retirement boundary. It does not
exercise the earlier native DATA/submission retirement prefixes. Those new call
sites have source review, not injected native-path or hardware qualification.
No generated-source sharing API or production protected authority is added.

The original issuance-only verification remains historical evidence for its
unchanged inner journal. New reader invariants and Rust/native composition are
not proved. Backend classifications remain explicit contracts. Lineage zero is
allowed; neither zero nor nonzero lineage authenticates initialized bytes.
General kernel input effects, available-input authority, aliases, ordered writers,
cross-run versions, content reuse, aggregate residency, native high-depth/overlap/
fault campaigns, Worker V3 and machine refinement remain open. No performance
gain or milestone completion is claimed.

The next identified implementation gap is pure `Read` typed-launch input custody:
backend bindings retain access descriptions, but Context currently creates no
reader root for those allocations. Batched reader roots must also support
all-read-only submissions without consuming a writer slot; `ReadWrite` allocations
remain under their existing exclusive writer. This is distinct from initialized
input or machine-refinement authority.
