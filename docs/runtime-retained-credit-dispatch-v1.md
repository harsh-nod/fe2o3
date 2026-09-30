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
selector. The 25 actual-body mutants are defined but a fresh signed campaign
with three full positives, including relocation, is still pending. No
historical packet is retroactively reclassified by the policy update.

The development recorders and raw packets currently live in the retained local
qualification directory, outside repository CI. The public component checker
accepts only `--calibrate` and refuses unsupported multi-body proof execution.
These results are not a self-contained published rerun package. A portable
campaign entry point and publication of its complete inputs remain separate
community-release work, even after source integration.

Older record/observer/constructor/fold/KFD/R75 source guards are compatibility
metadata updates only. Their executable proof closures, counts, mutant policies
and classifiers remain unchanged; no older solver campaign is newly qualified.
This document is outside the development build-source inventory and has a
separate evidence hash. Fresh group-closure observations cover only children
of each live recorder in its unchanged PID namespace, not host-wide or
historical process absence.
