# Retained Credit Dispatch V1

This component shares the actual serialized retained-credit dispatch bodies
between production Rust and a bounded Verus root. It does not establish live
accounting authority, concurrent freshness, or HIP/HSA parity.

## Shared Decisions

Five production functions now invoke their corresponding shared bodies:

- Context allocation admission reads the account and retained-entry lookups.
  `(None, None)` permits ordinary, unaccounted mode; it is not credit evidence.
  One-sided presence refuses, and two present entries reach the real account
  dispatch with the canonical request charge.
- Runtime dispatch checks the complete device identity before the typed
  General/Composed match. General retains arbitrary exact 19-coordinate
  vectors; Composed requires the complete canonical request shape.
- General account dispatch checks the borrowed token and concrete account
  variant/identity, performs the reached raw lock, and invokes the actual
  Independent or Domain observer.
- Domain dispatch checks root identity and the reached lock, then forwards
  the actual ancestry/profile/record fields to the existing Domain observer.
- Typed KFD dispatch checks request-account identity and forwards the actual
  inner token with the canonical charge for the supplied byte count.

The extraction preserves native short-circuit order and error/refusal behavior.
It adds no public API, owner clone, raw-session escape, or release permission.

## Proof Boundary

`retained_credit_dispatch_v1.rs` uses a fourteen-file executable closure:
the five new bodies plus the actual record predicate, Independent and Domain
observers, arena declarations/bodies, vector declaration, and charge constructor.
The unchanged Domain proof is an exact byte-bound prefix because its inner
attribute and private declarations prevent a direct nested include. This is
deliberate declaration reuse, not a second independent Domain theorem source.

The five new dispatch contracts characterize the returned Boolean from full
typed identities, complete vectors, reached locked-state observations, and the
ordinary/accounted Context lookup distinction. Concrete proof adapters expose
scalar identity, immutable lock-result state, and single-key lookup projections.
Their implementation is checked, but their correspondence to live Arc pointers,
Mutex acquisition/poison/deref/drop, or HashMap entries remains an explicit
boundary. There is no assumed `credit_ok` answer in place of the real predicate.

The whole root has 41 verified obligations, not 41 independent runtime
properties. That count also includes inherited observers, ancestry and equality
support, constructor checks, and adapter obligations. Compiler lowering and
pinned Verus/vstd specifications remain trusted; this is not an ISA proof.
There is no proof of concurrent freshness, conservation, native custody,
automatic release, or the exact number of calls performed by native execution.

## Development Evidence

The retained unsigned development results are separate component evidence:

| Check | Observed Result |
| --- | --- |
| CPU | 107 distinct cases: 7 resource-credit, 3 admission, 18 KFD composed, and all 79 accounting library cases |
| Full no-cheating discovery | 41 verified, 0 errors; entire crate; both release measurements passed |
| Static | Four no-default library checks, strict all-feature/all-target Clippy for the four affected crates, twelve-path format check, and tracked-path whitespace check |
| Selector capture | Two further full 41/0 positives and five actual-body diagnostic observations, bracketed by release measurements |

CPU cases exercise real General Independent/Domain and KFD composed fixtures.
They do not fabricate runtime native-Composed admission. No full runtime/KFD
suite or immutable retained-ELF qualification is claimed by these recorders.
The first proof attempt remains rejected for private `open` spec declarations;
the accepted retry removes only those three publication markers and updates
the proof pin. No native or test bytes changed for that retry.

All five captured selector families emitted exactly
`verifying root module (selected functions)`. Those packets remain diagnostic
observations, not qualified logical negatives. The selector
`*ResourceCreditAccountV1::matches_retained_charge_v1` has suffix overlap with
`RuntimeResourceCreditAccountV1`; it must not be described as selecting exactly
one function. Its observed failure is on the General account contract and
expands the mutated accounting body. The capture summary reports one verified
obligation and one error, without a per-function attribution of that count.

The source-pinned policy now permits only the observed singleton for each
selector. At that development checkpoint the 25 actual-body mutants were
defined but the signed campaign was pending. No historical capture is
retroactively reclassified by the policy update.

The development recorders and raw packets currently live in the retained local
qualification directory, outside repository CI. They remain historical,
host-specific evidence, not an attestation of the portable runner below.

Older record/observer/constructor/fold/KFD/R75 source guards are compatibility
metadata updates only. Their executable proof closures, counts, mutant policies
and classifiers remain unchanged; no older solver campaign is newly qualified.
This document is outside the development build-source inventory and has a
separate evidence hash. Fresh group-closure observations cover only children
of each live recorder in its unchanged PID namespace, not host-wide or
historical process absence.

## Signed Qualification

Signed candidate `c9937ee54` retains the production extraction and development
byte binding. Its first campaign remains rejected: the sixth mutation used a
leading Boolean block without parentheses, causing Rust E0308 rather than a
logical proof failure. Two positive runs and five logical negatives in that
rejected campaign do not count toward acceptance.

Signed child `cdebb1707` changes only the mutation generator's parentheses and
a paired source control. All fourteen executable proof inputs and all 3,096
selected native Rust inputs remain unchanged. Its fresh campaign passes all
33 stages: three full 41-obligation runs, including a relocated root, all 25
logical negatives, and both pinned release checks. Each negative joins the
expected family's postcondition to the actual mutated body expansion. This
matters because the General-account selector also matches the Runtime suffix.

All 33 newly recorded process groups close in the recorder's unchanged PID
namespace. No historical groups are probed and no host-wide absence is claimed.
The retained local result is
`signed-campaign-attempt-2/results.json`, SHA-256
`ef7e3cc8bd5d73d8bd5470b3094289f04e84306827d57e45cb879dae2c6aa0db`;
its closing census is
`d49d2d2e113afbe794dc73046c68ce3a4ddab21a5929d0f6c3fa3e1b20509a40`.
These live under the retained local qualification directory, not a portable
published evidence bundle.

Integration at `637d6124c` and `aa74e2b37` preserves the exact proof closure.
The merged source guards also retain the independently integrated publication
model and retained-routing sources. Their compatibility hashes are refreshed;
no old theorem or combined Rust suite is newly qualified by those hashes.
All twelve merged-tree CI source-control commands pass locally. The CI job
adds the source-only dispatch control, not a solver campaign or hosted result.
CPU/static evidence remains the earlier exact-byte development reuse. Live
freshness, conservation, native execution, A0-A7 and HIP/HSA parity remain open.

## Portable Campaign

The repository-contained runner is a new host-tool profile, not a reuse of
S2's host-tool qualification. Its source/synthetic controls exercise the
orchestration without running a compiler or solver. Signed candidate
`b27f19993e50a8e99faddeee5cd6dfb79a6c8629`, integrated at `0c38a20c6`,
completed a fresh public campaign from a clean relocated checkout with no
private recorder imports: all 33 stages passed, including three full 41/0
positives and all 25 family-bound logical negatives. All 33 fresh groups closed,
with source, tools, raw records, namespace and generated trees unchanged.
The [published packet](evidence/dev-retained-dispatch-portable-2026-09-30/README.md)
includes the exact signed candidate bundle and raw results. This is a new
measured-host profile, not reuse of the historical S2 execution. The merged
tree preserves all fourteen proof inputs but has no separate solver rerun.

Source-only calibration remains available:

```sh
python3 -I -B crates/fe2o3-runtime-model/verus/check-retained-credit-dispatch.py --calibrate
python3 -I -B crates/fe2o3-runtime-model/verus/test-run-retained-credit-dispatch.py
```

For proof execution, install the exact Linux x86_64 Verus release specified by
`crates/fe2o3-runtime-model/verus/pins/VERUS_CLOSURE_MANIFEST` and the Rustup
`1.97.1-x86_64-unknown-linux-gnu` toolchain. A full Git checkout is required;
sparse checkouts may omit only paths outside the selected source inventory.
Use Python 3.12 or later, GNU timeout/coreutils, Git with SSH signature support,
OpenSSH ssh-keygen, and readable Linux procfs. `CARGO_HOME` and `RUSTUP_HOME`
may locate existing installations; neither downloads nor Cargo builds occur.

```sh
python3 -I -B crates/fe2o3-runtime-model/verus/check-retained-credit-dispatch.py \
  --campaign --verus /absolute/release/verus --output /absolute/new-evidence
```

The direct `run-retained-credit-dispatch.py` entry point accepts the same
campaign arguments. Output must be absent, canonical, outside both checkout
and verifier, and have an existing parent. Missing/unknown/mixed mode flags
fail closed. Existing evidence is never overwritten or resumed.

The runner requires a clean commit signed by the explicitly pinned ED25519
key `SHA256:q8oGVYZ11904aFzlMkSiEwyeSP+6hbuiZGbNVGRZVCg` for
`harmenon@amd.com`. Git/ssh-keygen cryptographically verify the commit; every
selected working file is independently joined to its signed Git blob and
SHA-256 digest. This inventory covers crates, examples, Cargo configuration,
root build manifests, this document, and the two imported evidence helpers.
It does not assert authenticity of every other documentation or vendor file.
Forks signed by another key are deliberately refused, not implicitly trusted.

The entire 190-file Verus/vstd/Z3 release is checked against the pinned manifest
before and in a `finally` closing stage. Direct release executable identities
are checked at each stage. Host Git, ssh-keygen, Python, timeout, shell, closure
utilities, Rustup and the named rustc executable are instead measured on this
machine and checked for path/hash continuity. They are not pinned to S2's
historical tool bytes. Python/system libraries, Rust compiler libraries,
dynamic linking, kernel and procfs remain trust boundaries; this is not a
hermetic build or a compiler/ISA proof.

The 33-stage plan uses the unchanged repository-owned process-group owner and
strict diagnostic classifiers. A generated tree contains exactly the fourteen
proof inputs; each of the 25 negative trees changes only its named body. A
negative must fail logically on that family's postcondition and carry the
matching body excerpt, macro call and macro definition. In particular, the
overlapping account selector cannot qualify an unrelated Runtime failure.
Timeouts, frontend errors, unexpected diagnostics and count drift fail closed.

Source, tool, raw-log, namespace and generated-tree observations are independent
closing checks. A fresh-only census examines only this invocation's recorded
groups in its unchanged PID namespace. It makes no historical or host-wide
absence claim. A failed campaign retains its logs, attempts the closing release
check when opening release was attempted, and reports `accepted: false`; no
later positive is run after rejection. Source/CPU/native/performance milestone
gates are not closed by a component proof campaign.
Acceptance requires both a normal zero exit and a complete `accepted: true`
receipt. Evidence-write failures propagate as errors, may leave incomplete
receipts, and still restore the caller's working directory and signal handlers.
