# Source/native and runtime-origin integration, 2026-09-19

This is qualification evidence, not completion of #280/#281/#282 or protected
artifact/GPU execution authority. M1, V1 and U1 remain the only closed original
milestones. Public source promotion, recipe policy placement, resource lifecycle
and origin-aware query interfaces still require their concrete owner contracts.

## Integrated source and peer history

Signed source-promotion, local-order and runtime-origin commits e9b681d8,
40fbbaa2 and b8f7abf8 were integrated with source-machine/retention commit
7f1e60a8. Normal merge 6f865d179ccec5efece74ba8a967686f4f0e6125 preserves the
published peer SDK vendor-fixture/target-roster refresh
7dfbe5cc453f12c1b8b32eeb49f08778876bf6ce. No peer history was rewritten.

The merged Rust/Cargo/crate-README census was
995bd1072ae961b4379af9600086421a901d75f7b7ebecfddd3126777c351ec2,
3,766 files / 73,622,314 bytes. The phase9 runner uses a different documented
framing for those same inputs: digest
9b0860ccbe4ac20d123b2edb4d3dd0f452eb4e84cc51ba9cd32b10f1997cd53e.
Do not interchange these digest domains.

## Merged qualification results

All paths below are relative to the retained task root on mi350-2:
`/home/harmenon/fe2o3-authoring-280-282.FEW3gj`.

| Gate / retained label | Observed result |
| --- | --- |
| phase11 source-format r5 / source-policy r3 | Pass |
| source-backend-full r4 | 1,315 passed, 49 intentionally ignored |
| bitselect-roundtrip r4 | Actual source ladder passed |
| local-order-source r2 | 11 actual callbacks, 180 positive full-kernel simulations, 8 exact refusals |
| source-machine r4 | 7 actual callbacks, 90 positive full-kernel simulations, 3 exact refusals |
| source-machine-native r2 | Edited r4 LLVM, O0/O3 final decoding and descriptor-capacity checks passed |
| debug-retention r5 | 15 passed |
| debug-binaries r8 / runtime-source r9 | Fresh normal loop/helper export; 6 contextual + 6 opt-out runs, 32 helper activations |
| phase9 source-isa r18 | 130 passed |
| phase9 libs r18 | 3,859 passed, 15 intentionally ignored |
| phase9 docs r18 | 222 passed |
| phase11 tutorial r10 | Strict manifest, pairs, gfx942/gfx950 matrix emission and 100-control matrix harness passed |

The debugger/protocol/CLI full regression r2 (311 passed, 3 ignored) and simulator
full regression r3 (273 passed) ran on the preceding integrated source census
8788c228... before the four-file peer vendor refresh. They are not relabeled as
runs on merge 6f865d17. The merged library and actual-source checks above are
separate evidence.

## Exact new observation payloads

- source-machine-r4/observation.json: 49,584 bytes,
  SHA-256 2bffe1b50129cf5e75cf955edb41bfddb6d3e043416dd97f242f639a761a50cb.
- source-machine-native-r2/receipt.json: 92,354 bytes,
  SHA-256 04acf650829c16357977cba03957e032482fbffb0804ce370dbb6dbb744d6d8a.
- source-machine-native-r2/native-observation.json: 10,387 bytes,
  SHA-256 b306eb931bc7115bd18fabf2a998715245866349c2907b34df2fa762b33933bb.
- runtime-source-r9/receipt.json: 210,475 bytes,
  SHA-256 e371e328ca4e1cffe09ca28b6ba998a4211d8c75cf98fc80238c3429f175c6a3.

Directory names above carry the phase11- prefix. Earlier failed and successful
runs remain retained with their original labels and identities.

## Hygiene follow-up and publication boundary

The hygiene r18 gate correctly flagged panic assertions in the test-only
retention fixture because its path was not recognized as test source. The fix
renames fixtures.rs to fixtures_tests.rs and updates its private path hook;
assertion behavior and hygiene policy are unchanged. A documentation comment
makes the existing cfg(test) boundary explicit. Fresh retention r6 (15 tests)
and format r6 pass after this naming-only change. Their receipt includes the
unstaged rename deletion marker and is not confused with the earlier census.

Publication is separately constrained by the inherited unsigned peer commit
7dfbe5cc. The current DCO rule has no signed-follow-up or note remediation and
the existing two exact exceptions do not cover it. A signed normal merge does
not supply the missing author's trailer. This note does not grant a new
exception, narrow the checked publication range, or claim that DCO passed.
The exact author/maintainer disposition remains a separate gate.

No safety cap, production policy, wire format, proof requirement or protected
finalizer condition was weakened. See the
[owner-review supplement](../assembly-authoring-contract-review-20260919.md),
[source/native details](../source-candidate-machine-qualification-v1.md) and
[retention details](../runtime-origin-retention-qualification-v1.md).
