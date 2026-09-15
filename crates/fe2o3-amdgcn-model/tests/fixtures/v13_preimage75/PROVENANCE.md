# Frozen V13 Preimage75

These files were captured before Composite73 was mounted. They are immutable regression expectations, not source-authentication, operational-proof, or launch evidence. The tests never generate or replace them.

## Baseline

Capture used the existing cached production strict V13 canonical owner, target closure and complete LLVM writer. No dependency was rebuilt. The tiny capture executable and linked replay ran on the host; neither compiled LLVM nor launched GPU work.

- Initial executable SHA-256: `61abe6e3c1bde6152100166ec8c844a140edb6fdf850e84db83dd854a1e44411`.
- Linked-attested replay executable SHA-256: `61abe6e3c1bde6152100166ec8c844a140edb6fdf850e84db83dd854a1e44411`.
- Compiler: rustc 1.96.0-nightly (55e86c996 2026-04-02), LLVM 22.1.2, nightly-2026-04-03.
- Fixture source `crates/fe2o3-amdgcn-model/tests/v13_lowering.rs`: `d3475ee64d4f656d8afe97274cc5c208c4855d96fdc78bd053b02b0a57146329`; exact invocation `module_with_capability(Some(64), "gfx942")`.
- Fixture source `crates/fe2o3-amdgcn-model/tests/lowering.rs`: `d998b9accbd6affdcef4215a8afc99f14c759f46464ab5262c85fe51ea916267`; exact invocation `fill_module()`.
- Strict canonical-owner source SHA-256: `07b6e6050745aa35d90ddc84712d8bd3ca8b68b3f5d97fc4fcb11f409a5db06a`.
- Production lowering source SHA-256: `872317e107e9c795de640ccda4d152cedd670129714b54261925722d1927410d`.
- Target-closure source SHA-256: `cb57657c3691c21585742ab2434549d3c6c72a21a40c4758c3043ee7f17633d4`.

- `libfe2o3_amd_target-96c442d2ce719c8f.rlib`: `180c9608841055347fecc4f4f97459d39eadaf77264e4576acac1adddc3fd1cd`.
- `libfe2o3_amdgcn_model-5d9e525a1c9e06f1.rlib`: `5e6e1b6abf4a96bdcfc9bcc4ba21108e056a51076afaf2c9c2fbc8c89f58e629`.
- `libfe2o3_kernel_ir-3fb32cafa47078a3.rlib`: `cfc3e5943759ff4c4ce6bcab91c4f9a279e313ef95316eb0030cf5eb3af3986e`.

The full source/metadata/link-archive hashes and exact commands are retained in ART/pascal-v13-preimage75. They record observed source hashes and cached binaries, not a fresh reproducible-build attestation. All input hashes were unchanged through the capture. The link dep-info also named libstd.so; its hash was recorded after link discovery and before replay. All actual link inputs were pinned before replay.

- `baseline-linked-run-inputs.json` SHA-256: `064a80fb350d37bedc54e312a5537e83b59467b3e59e7c793bdde84dbe19a4bd`.
- `fixture-provenance.json` SHA-256: `3b5981ba6d15994b7fc32343d8f3c428a9ecdf9712c054e03709f69834d3caa7`.
- `capture-evidence.json` SHA-256: `a49076396cf24e3c8246400fba10405a3cf6ac3e61d094a12fd4d2dc452490dc`.
- `linked-evidence.json` SHA-256: `078b2e38398bb8f375d583c836552bf03fcc221e8640468eef365948923e95ad`.

## Corpus

| Fixture | Canonical bytes | Byte SHA-256 | V13 owner identity |
| --- | ---: | --- | --- |
| hierarchy | 1985 | 8d3c8f707703b168ba61088ccbaafb0825df4da38004b9252ff158b3355038ca | d904fff1f5af2c24a7c22cf91b75a2c30efa82fae824b7812bff53f9eb707573 |
| fill | 320 | 74c36734bb21b5d1e4af31bf4d7b7b9801d5b605968a097e82947dcd5b2d3bc9 | b1193b3b2e6072aea15b84868eafb2dd80be0d14b9afed763ffbe03ab63981aa |

The hierarchy fixture covers checked context/workgroup/subgroup operations and source identities. The fill fixture covers physical slice ABI, invocation index, comparison, conditional branches and a global store. Constructor bodies were extracted byte-for-byte from existing tests; only capture-only imports and public invocation wrappers were added. Both targets use the same fixture bytes and epoch 3.

*.kir.hex and *.closure.hex are mechanically encoded lowercase hex of the captured bytes, with one final newline. Each *.identities file has exactly four lines: canonical-owner identity, launch-evidence identity, gfx942 closure identity, gfx950 closure identity. LLVM files are byte-exact output, with no normalization.

The original captures had incomplete operational derivations (hierarchy: four unsupported entries; fill: nine) and no load/launch authority. The regression asserts incompleteness and absent authority; it does not convert writer success into proof completion.

## Verification

Executed: both fixtures on gfx942 and gfx950, exact second-run comparison, and a separately linked replay with all outputs identical. Candidate common-facade tests were formatted/parsed but not compiled or run against the unmounted Composite73 APIs. Parent central command after composition: `cargo test -p fe2o3-amdgcn-model --test v13_preimage75`.

Two discarded candidates remain in ART evidence: an unchanged multi-entry fixture rejected V13 device-FFI export roles; an unchanged legacy Wave64 fixture lacked the exact SubgroupWidth query axis. Neither rejection was waived or included in this positive corpus.
