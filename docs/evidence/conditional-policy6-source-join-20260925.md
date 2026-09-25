# Conditional Policy6 Source Agreement

Follow-up to the [failed protected replay](conditional-protected-replay-20260925.md)
for #272. No milestone closes and no tutorial kernel gains end-to-end credit.

## Integrated Boundary

Compiler candidate: `3ef6db57ec7535dd8270cf54abec4e6157e19ec9`.
The Direct policy6 path now checks a live conditional source request against
the existing target-bound and optimized graph before the unchanged
`FE2O3-COND-FINALIZER-001` refusal.

- Recheck target coordinate preservation and the sealed optimization prefix.
- Independently derive conditional coverage from the actual optimized output.
- Join ordered reads, the output store, source arguments and runtime premises
  through the checked occurrence maps, not by assuming source operation IDs survive.
- Keep the original source and target accounts distinct; release temporary
  storage on return or unwind without refunding work or replacing either account.
- Use the existing conditional proof-replay callback, retained receipt reimport
  and exact generated-contract comparison. Do not manufacture ordinary evidence.

The check returns no verification owner. It does not establish final native
graph F, native V4 source recovery, publication, safe launch or machine refinement.
The updated annotated Vecadd test requires Direct source ownership and calls
the fixed production policy6 facade rather than the older policy4 test dispatch.
Its fresh protected execution subsequently
[passed for both targets](conditional-policy6-protected-replay-20260925.md).
Neither component tests nor that bounded replay establish the full production
transaction.

## Local Validation

The five test guards below used one unchanged 8,169-file source inventory,
SHA256 `be5dae2d5551a96624167c7b0ca7f14a490802964159f7fb1ef5ac0ab512939c`.
This is a file inventory, not a Git tree hash. This report was added afterward.
Pinned nightly `2026-04-03`, locked offline dependencies, one Cargo job/test
thread, disabled HIP, hidden GPUs, a 12 GiB virtual-memory ceiling and a
1,200-second deadline were used. Each source/tool snapshot stayed unchanged.

| Guard | Result | Log SHA256 |
| --- | --- | --- |
| `conditional-policy6-lower-tests-r1` | 8 passed | `1f50e1b6ca1241b5addaf78fec91b28d5e15274fd9b0cbed520a36d509e7812e` |
| `conditional-policy6-lower-regressions-r1` | 56 passed | `67bfac87fc33de9d4ddb20c218dc7ae1e2370f51c11909ed552c3205927d8d57` |
| `conditional-policy6-authority-doctest-r1` | 1 compile-fail passed | `617932c345b1acea0a0ab17a15774b166b8c6a4975b96e750b662c8a5bbe92b2` |
| `conditional-policy6-backend-tests-r1` | 40 passed | `b5b05932849fb0da7121d3f317e052a828f975c495cb37001e51890ffbc5f823` |
| `conditional-policy6-field-regressions-r1` | 36 passed, 2 ignored | `c16018e681b25c2a8c07d358ce925df09a22c9f25df97aed49eb327fbce4a571` |

Coverage includes gfx942/gfx950 component positives, source/target/prefix
substitutions, foreign output owners, reordered or omitted reads, weakened
premises, exact and one-short resource limits, account replacement, unwind,
ordinary Direct/Erased regressions and independent source-body argument roles.
The compile-fail test produced the intended E0308: `()` is not a formal memory
owner. The two ignored tests require actual compiler captures and protected
proof execution; they are not passing integration tests.

The normal backend library check also passed after correcting missing public
error documentation; its log SHA256 is
`7e231ae6dd5f74520dfe8abee04c66225b2cb1372ff5a954ec813c357353f199`.
Its earlier source snapshot predates the Vecadd test-route change.
Changed Rust formatting and whitespace checks passed. Existing compiler
warnings remain; this is not a warning-free or whole-workspace test claim.

## Reproducible Audit Dependencies

The [qualification base builder](../../scripts/build-compiler-execution-qualification-base.sh)
now pins `mawk`, `diffutils` and `findutils`, preserving all original 99 package
records. Extraction creates the deterministic `awk -> mawk` link without running
maintainer scripts. Static checks cover the auditor's commands, interpreter and
transitive ELF dependencies in both the extracted tree and final image.

Two isolated, non-root builds of exact commit
`70d404a30b1f56bae27b538a885cc569cde1f15b`, epoch `1790356631`, produced
byte-identical three-member bundles with 102 packages and unchanged schema V1.
The provenance commit is retained in the compiler candidate's ancestry;
the merge changed no source files and does not relabel either build.

| Member | Bytes | SHA256 |
| --- | ---: | --- |
| `qualification-base-v1.squashfs` | 32301056 | `1289c6b71d8ff0de74d88ce10fc465fe5705c4bad7a650b191ffc83e3466ed0f` |
| `BASE-INFO` | 11333 | `4c94e59f051f88225eeacca2a5453bd6152d108480cf752ad4bb29d15149b8de` |
| `SHA256SUMS` | 173 | `11bf92759c0614f8bf49491f51243833a6f582d45cc4a9703e06b79830b3ca7c` |

Both bundle checks passed nine helper tests and inspected 18 command names /
24 ELF objects per image. The old image was correctly rejected for its missing
`awk` link and remained unchanged. The verified two-bundle archive SHA256 is
`7f2f26a8aa28484698e1abdf52afaa3633f5bc5fa3c8a5aec1416f4469759da6`.

Builds took 430.207 and 434.772 seconds under one-CPU/nice limits and 590-second
deadlines. All owned process groups were confirmed absent and disposable build
scratch was removed. Source/configuration, bundles and reports are preserved;
completed private worker worktrees were removed. No host package installation,
shared runtime/image mutation or GPU execution occurred.

This is construction, static inspection and reproducibility evidence, not
runtime qualification. The older external r9 runner's 99-package branch must
not be treated as admitting this new base. A reviewed r10 integration later
used the exact base for the linked protected policy6 replay. Final-graph custody,
native recovery, safe launch and the tutorial matrix remain incomplete.
