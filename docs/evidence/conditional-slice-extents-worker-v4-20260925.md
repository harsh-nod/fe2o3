# Slice Extents and Structural Worker V4

Continuation of [V4 artifacts and context fields](conditional-v4-context-validation-20260925.md)
for #272. **No milestone closes and no kernel gains end-to-end qualification.**
The ordinary slice-extent projection and structural Worker recovery tests pass.
The actual-source Vecadd integration still fails, now at canonical conditional
output coverage rather than the previous unknown ranked extent.

## Implementation

Ordinary MIR slice metadata now shares the intrinsic pass's stable argument origins,
extent cache and ranked-argument allocator. These are analysis parameters, not new
physical kernel arguments. Only exact metadata operations on source-stable slice
references and same-type copy/move chains qualify. Casts, escaped or multiply
defined locals, projected metadata, and unsupported shapes do not establish the
relation. The metadata result must be unsigned and match the admitted pointer width;
both structural and nominal pointer-sized type records are supported.

Cached extents require successful validation of the current query. Existing local
identities cannot be replaced after use, including an index/length self-comparison.
Private scratch is charged to the original resource account and released after
disposal on success, error and unwind. Work charges and denial history persist.
The bounds prover and subsequent proof/authority gates have not been weakened.

Descriptor V4 now survives the existing strict Worker V3 finalization, compact
replay and durable intent recovery. Exact ABI/contract bytes, export closure and
transaction identities remain checked. Typed V1/V3 recovery rejects V4; native
Worker schema selection still rejects V4. These owners remain structural only:
there is no V4 publication bridge, host load envelope or launch authority.

## Validation

The pinned nightly `2026-04-03` guard uses locked offline dependencies, one Cargo
job/test thread, disabled HIP, hidden GPUs, a 12 GiB virtual-memory limit and a
1,200-second deadline. All listed source/tool inventories remained stable.

- A: 8,149 files, `8a11d4c59ab5817f2cbabead246ad2c09e526899cfba4159f51c76d99bde4b99`.
- B: 8,150 files, `c5b1c6468de4c910ffea88c44ab26ac54985519ac7d8af234183120e7645944e`.
- Guard SHA256: `20f8ea8ce430f72e9f2925dc4d79b28739229f36dd6d0c46a45b430caffd3642`.

These are source inventories, not Git tree hashes. Documentation and test formatting
were updated after the guards; production Rust code was unchanged from B.

| Guard | Snapshot | Result | Log SHA256 |
| --- | --- | --- | --- |
| `conditional-worker-v4-tests-r1` | A | 177 library, 54 public API and 42 Worker tests passed; one Worker fixture failed; 17 ignored | `9b3b9f5d4aa1968d631c2d6500775dcf73e74b9f20ef0482496299d0868ae506` |
| `conditional-worker-v4-recovery-r2` | B | Corrected Worker fixture passed | `012ee86bc856c072747b4476eecab7da191a3cfc5c0b2fdd517b71af7e4a9686` |
| `conditional-worker-v4-doctests-r1` | B | 30 compile-fail API tests passed | `6e8a7f9df15e53117511f441dfdc21b28da36f5387643a1dc9cbf1f83ea89294` |
| `conditional-slice-extent-projection-tests-r3` | B | 520 projection tests passed, including 11 new metadata tests | `3008beff2b9f83be1183add501ac6f58bd5bdc937c51a210c60b804f2888a073` |
| `conditional-slice-extent-production-check-r3` | B | Backend library check without test-only proof features passed | `5e323f5b8375d9113e751013f539b5e6c5d93dece6e32a616774a902b21a3b57` |
| `conditional-worker-v4-targets-r1` | B | Finalizer all-targets check passed | `874bb805ba780f3dda61f293d67f6892eaabe8ab531e591c6bb27efc88c08cb9` |
| `conditional-slice-extent-source-prepared-r2` | B | Actual annotated Vecadd source failed at canonical coverage on gfx942; gfx950 not reached | `91db718493f14e5ada22ac9ec14f3de3affbcbc9dbcc0c3023734108730fd392` |

Across these runs, 794 distinct selected unit/integration tests and 30 API doctests
passed. This is not a workspace-wide test run or a passing actual-source integration.
Ignored tests receive no pass credit. Backend unit tests use
`fe2o3-pliron/internal-proof-staging`; the production check does not.

The initial Worker fixture incorrectly expected structural evidence to authorize
build completion. The corrected test asserts the exact completion refusal and
subsequent terminal-phase recovery refusal. No production gate changed.
Projection r1 passed 517 tests but failed an invalid-layout fixture and two component
fixtures missing storage custody. Those fixtures now retain valid layouts and real
resource accounts without changing their semantic assertions. Projection r2 then
failed to compile its new test adapter; r3 corrects its budget constructors and
concrete callback type. Existing unused-code warnings remain.

## Source Failure and Remaining Work

The actual-source test captures ordinary Rust and exercises the existing production
transaction, using the real Vecadd body with an auxiliary CPU-reference annotation.
It is not the default unannotated manifest selection. On gfx942 the canonical CPU
oracle observed ten matching scenarios, two short-input refusals and no semantic
counterexamples. Preparation passed the previous unknown-extent rejection, then
failed with:

```text
actual canonical output does not establish conditional coverage
```

The observer does not yet retain the specific unsupported-coverage reason. That
reason and the exact canonical graph must be inspected next; coverage must not be
assumed or replaced with an unconditional success. No protected proof ran, no root
artifact or authority was issued, and no GPU was exposed. Fresh protected replay,
conditional final-graph custody, applicable machine/numerical refinement, authenticated
publication, generated safe host launch and the full 47-kernel matrix remain.

The separate MI350 qualification-base preflight found that the existing 99-package
lock has 13 versions absent from current indexes and 11 installed-only entries
without download sources. No base was built, the lock was not relaxed, and no host
package state changed. The exact package bytes must be recovered from an appropriate
archive before qualification. Its preserved evidence archive SHA256 is
`54280b8dc9c3749aaa45ed33ee2df5ab18966a2970412205dd9c4dbd1a3936d0`.
The agent's private remote scratch was removed after archive verification.

No new protected-runtime or hardware validation is claimed by this checkpoint.

## Main Integration

Both remotes advanced to `13e424bdef9f0cb9d0a073c86d8bebc8f190741a` during testing.
Their row-kernel work and fixture-lock updates were preserved by a conflict-free
merge, `97d05e819331a6b39b3878edd766572e1b38fb18`. On that combined tree, the same
guard retained a stable 8,156-file inventory:
`82ae93c1ad2763ed7c6db996409fbec2377e5c69449a3d816af98a4f879a3010`.

| Guard | Result | Log SHA256 |
| --- | --- | --- |
| `conditional-slice-extent-merged-projection-r1` | All 520 projection tests passed again | `20889db81bd762704362befbaa7f439ba026ce4ed206566e6daafc63cd15bbb9` |
| `conditional-slice-extent-merged-production-r1` | Normal backend library check passed | `a770f0c8d69294d87706d73c4da28cb48c641e11ca480238ed82e24da52bdad4` |
| `conditional-worker-v4-merged-targets-r1` | Finalizer all-targets check passed | `3e8dc183d5a0994af9e0911f95e35764b79645b546800f56be2975c6e662d1f6` |

These repeated tests do not increase the distinct-test total above. Formatting,
workspace dependency policy, source hygiene and commit sign-offs also passed.
The actual-source integration and hardware matrix were not rerun after the merge;
the source-coverage failure remains open. Only this documentation addition followed
the combined-tree guards.
