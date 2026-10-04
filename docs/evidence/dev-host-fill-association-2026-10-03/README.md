# Conditional Fill Current-Publication Association

Parent: `09b59144a6d4c011eec8b9ee7c4042c5a7547642`.
This checkpoint advances the first ordinary multi-GPU application admission.
It does not grant launch authority or claim a GPU execution.

## Implementation

The host request now checks the exact conditional compiler owners, target replay
and authenticated fill-analysis owner together inside its existing safe
current-publication audit. The result borrows all three owners; it is neither
cloneable nor an executable. Audit evidence cannot retain these temporary borrows.

The join compares the complete receipt preimages and proof identity, singleton
canonical compiler descriptor, semantic root's cryptographic kernel binding,
existing target-lineage table, gfx942/xnack-/wave64 profile, full finalized HSACO,
finalizer identity, physical descriptor and entry symbol. It reuses the existing
target-lineage checks without relaxing unconditional admission.

Native qualification exposed an existing importer bug: V5 correspondence was
decoded and then reduced to its nested V4, losing the exact outer receipt. Both
validated compiler owner types now retain a private V4/V5 enum. Structural V4
accessors remain unchanged; exact receipt comparisons use the complete original
schema. V5 function names, ordinals, roles and definitions are also checked
against the independently decoded Kernel IR. No wire version or proof body changes.

## Qualification Scope

The new ignored integration test uses the unchanged genuine compiler handoff,
actual native Worker bootstrap/replay/finalizer, publication, V2 recovery and safe
host audit. Inside the audit it independently imports the conditional compiler
proof, replays target lowering and executes authenticated analysis on the exact
request bytes. It checks the genuine 272-byte kernarg layout and revalidates the
finalizer derivation.

V2 carriage and the marker are explicitly test-only. The marker is checked against
the captured canonical compiler descriptor, not presented as generated argument
ABI authority. The Worker is measured for this test, not admitted by a deployment
compiler-origin policy. There is no protected verifier positive path or launch.

Negative controls reject genuine fill owners joined to a foreign compiler request,
a genuinely analyzed ELF with changed non-executable comment bytes, a wrong marker,
and a publication directory replaced during the callback. Original owners remain
usable after foreign-owner rejection; the restored publication revalidates.
The old unconditional compiler importer still rejects the conditional handoff.

`qualified-final` passes 1/1 on MI300X (2026-10-04 UTC). Both audit captures retain
the same 6160-byte finalized HSACO, SHA256
`8b6c2e5b67ba2f76bb57d5f42aedbaf968f8dc12bb121daa5d7853cc55d158c6`.
The rebuilt Rust test executable hash matches the uploaded executable, and all
three executable hashes remain unchanged after the run. The native Worker was
built with ROCm 7.2.4 LLVM/LLD using two jobs; its machine-effect CTest passes 1/1.
All 18 archived Worker source files match the repository. No GPU queues or
allocations were created, and no performance measurement is claimed.

| Campaign | Result |
| --- | --- |
| Legacy V3 compiler import | 6 passed |
| V4 compiler import, including V4/V5 retention and roster substitutions | 28 passed |
| Conditional fill program unit tests | 8 passed, 156 filtered |
| Host library | 211 passed, 1 default ignore |
| Host doctests | 40 passed |
| Selected Worker V3 vertical integration | 46 passed, 4 default ignores, 15 filtered |
| Complete V2 load-envelope integration | 31 passed, 3 default ignores |
| Finalizer integration | 23 passed, 3 default ignores |

The vertical campaign excludes `strict_v3_` and
`cargo_supervisor_and_static_host_consumer`; it does not claim the nested static
application-build lane. The native audit is explicitly run despite its default
ignore. Other default ignores are not claimed as tested. V5 negatives are validly
encoded name/ordinal substitutions with freshly rebound outer receipts, so they
exercise the bound-KIR roster check rather than merely a malformed codec.

Development failures are separate from final qualification: `qualified-1`
exposed the V5-to-V4 wrapper erasure. An intermediate duplicate-Debug compile
failure and two interrupted host builds were resolved before the final run.
No formal proof body changed and no new adapter proof is claimed.

Strict verifier-library, host-library and vertical-integration Clippy checks pass.
An additional verifier integration Clippy run reports the pre-existing unused
`WorkgroupCollective` and `Tiled` shared-fixture variants; its diagnostic is retained
and that lint target is not counted as passing. Targeted rustfmt and whitespace
checks pass. Final build executables match the native qualification hashes.

## Remaining Multi-GPU Gate

Implement the actual protected conditional machine-refinement producer and retain
its execution together with authenticated compiler origin/currentness in a distinct
pending artifact. Then consume exact packed/prepared coverage, full64 geometry,
patched storage, selected device and publication currentness into the private
invocation authority. Do not convert conditional receipts into unconditional V4
evidence or substitute offline proof-campaign success for a retained execution.

The first hardware target remains admitted fill on each selected GPU, completed
output -> staging -> settled H2D -> PUBLIC XGMI -> guarded readback in both
directions. Native routing, adoption and issue paths are reused. Broader opcode
support and performance work remain deferred until this path works.

## Reproduction

The [qualification archive](qualification.tar.xz) contains the code-only patch,
matching source snapshots, build/test scripts, CPU logs, native build records,
genuine handoff/finalizer/analyzer captures, development diagnostics and review
summary. The native script records the explicit executable and build identities;
reruns require fresh owned directories. Archive SHA256:
`29cbf4e7be4fce183d06c5aebc98f34301756860b43e18d26b3c6be07aa92569`.

Code-only patch SHA256:
`8ce8a1314df8ac4def0bfc10be5da873714193f233582aaa62b3e848ab16e047`.
The complete 123-entry archive listing and extracted patch digest were checked.
The owned MI300X scratch was removed and its absence verified after native records
were copied and checked. No unrelated shared-host path was removed.
