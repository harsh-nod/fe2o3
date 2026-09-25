# Fresh Conditional Source Replay

Follow-up to [conditional coverage and native V4](conditional-call-coverage-native-v4-20260925.md)
for #272. No milestone closes and no kernel gains end-to-end qualification.

## Frozen Inputs

The compiler candidate is `f504548b67ef1fa8d737082ff2b710aeb8ab1540`.
Fresh backend/verifier test-artifact selection and genuine annotated Vecadd
input capture passed on one unchanged source/tool snapshot: 8,161 files,
SHA256 `e163f1ed38e3653b5e382b3e1c92550d14466ce0a3a2e52317aede4f89b74f2f`.
This is a file inventory, not a Git tree hash. The later documentation changes
are not part of the candidate.

The pinned nightly `2026-04-03` guard used locked offline dependencies, one
Cargo job/test thread, disabled HIP, hidden GPUs, a 12 GiB virtual-memory limit
and a 1,200-second deadline. The capture test passed in 487.71 seconds and
prepared distinct actual compiler invocations for gfx942 and gfx950.

| Guard | Log SHA256 |
| --- | --- |
| `conditional-protected-next-backend-artifact-r1` | `fd0eb0711fd7f2926e5c31690c536f8e16fc47e33d22729e96c1fec045de58eb` |
| `conditional-protected-next-verifier-artifact-r1` | `49a6c06fefd0d97909819b92ad0a2b952ca2925694fb553c143962d50ce05359` |
| `conditional-protected-next-capture-r1` | `4ef132c630285cf32108c403d81146fab5cdc297e321b47feb7279b3eb160da9` |

Bounded staging and the reviewed r9 runner's complete provenance validation
passed. The preserved input archive passed `gzip -t`.

- Request SHA256: `f781b2c1308d248c0bdcc681635e3307422a3eccd23960dfe3e06411387872ca`.
- Input archive SHA256: `7135507cc69d8ce546d481012c05236f2b4a17d8b57b5a0707afffd248f630a5`.
- Runner SHA256: `35a7efed1d80d38f85421a784b7b78da993f15515bc6ee1c36e0e6a6a2fea82a`.
- All 69 offline runner regression tests passed. They are not protected proof tests.

## Base Image Boundary

Independent read-only review found that the reproducible 99-package base lacks
`awk`, `cmp` and `find`, required by the mandatory installed-runtime audit;
`diff` is also absent from its failure-diagnostic path. Package identity and
reproducibility checks alone did not test this executable dependency closure.
That base was rejected before proof execution. No audit check was skipped.

This replay instead selects the previously validated external-image bundle,
recovered from its preserved archive without changing any shared image or
runtime. Its checksums match the reviewed r9 pins:

- BASE-INFO SHA256: `8718276f444c4929f404c050c836f492b2edead0ff5ebfbce50defe8aaf107d6`.
- Image SHA256: `93258b1fee71e6f6808c519f4088633837f5ce237f5909b94b3dbda927528520`.

The external runner always reports production qualification and launch authority
as false. The pinned base's missing audit dependencies require a separate fix;
using this diagnostic image does not establish the production qualification lane.

## Execution

The single MI350 run terminated with exit 1. The installed-runtime audit and
all three protected controls passed: installed-closure audit, real Verus proof,
and rejection of a false proof. The gfx942 invocation reached the generated-field
assertions after retained proof replay, then failed a test-only assumption that
`semantic_local == source_argument + 1` (actual local 3, expected 1). Semantic
locals are identity-sorted; the argument role carries the source ordinal.
The parent failed and gfx950 was not attempted. This is not a passing replay.

- Results archive SHA256: `46beea57e987a03d1b6c2f329566e70aa6bacf019ddb5024d49120731b75256d`.
- Inside report SHA256: `07c3e57215c3b562c2ac7a91c8a572a7ae6917e06708e59ec1b0f30b06cf7052`.
- Drained state SHA256: `e339d5d18ac2a0117f6f53f43a439d6152f18b3e333bc8e270da5dc397f37bc2`.

Reports were archived and checked before cleanup. The owned run, incoming inputs,
bootstrap directory, service and slice were removed; the saved state confirms
an empty cgroup and no main/control PIDs. Shared runtime and images were untouched.
No GPU device was exposed and no hardware result is claimed.

The follow-up test fix records argument roles, local IDs and type IDs directly
from the selected source body, independently of generated-field and read rows.
It checks root/body selection and rejects even coherent substitutions in both
argument and read observations. It does not alter production admission or
retroactively turn this failed run into a pass. A fresh protected replay remains
required for the corrected test executable.

All eight passive-observer tests passed under the same bounded local guard,
including independent source-role matching, coherent local/type substitutions,
missing/duplicate roles, read order, unwind and event bounds. The unchanged
test snapshot contained 8,163 files, SHA256
`7831fe8fe8c2f7dd7fc4e34122decc235f026a5c62d7e603cadce612ed7d1e17`;
log SHA256 `d8da4c514d1a16d25aa9abc1be6fcc889fe3b9682a052f8b0aa60fb10bb55937`.
This paragraph was added afterward and is not part of that snapshot.

Conditional source-to-final-graph custody, exact native semantic recovery and
the full tutorial matrix remain required. A physical descriptor match alone
cannot replace those checks.
