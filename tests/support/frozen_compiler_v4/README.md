# Historical Sourceful V4 Test Inputs

These are test-only canonical preimages from genuine compiler revision
`149019b405aa3219e6dc78f6c9de75b8406ed1c6` (`149019b40`), using its pinned
`nightly-2026-04-03` toolchain. They exercise legacy source-proof and semantic
debug-map validation after the current lowerer moved to KIR V13. No artifact is
projected from V13, and these inputs do not establish current native pipeline
completion or production proof authority.

## Capture

`capture.rs` is the exact standalone test helper used in the historical checkout.
Copy it to `crates/fe2o3-hsaco-finalize/tests/capture_legacy_v4.rs` in a clean
checkout of the revision above. That helper must be the only added source file;
do not copy the current shared fixture implementation into the old checkout.
From that historical checkout, with its normal build prerequisites installed:

```sh
FE2O3_FROZEN_CAPTURE_DIRECTORY=/absolute/path/to/new-capture-directory \
  cargo +nightly-2026-04-03 test --offline --locked \
  -p fe2o3-hsaco-finalize --test capture_legacy_v4 -- --nocapture
```

The destination must not exist. The helper creates it and seven case directories,
writes each file with `create_new`, and prints its SHA-256 digest. Copy the seven
directories unchanged into this directory. `capture.rs` is not compiled or run
by the current test loader.

Each case contains the five associated artifacts from one historical fixture
invocation: `semantic-mir.bin`, `middle-end.bin`, `kernel-ir.bin`,
`correspondence.bin`, and `formal-memory.bin`.

| Case | Historical constructor | Seed |
| --- | --- | --- |
| `noop-20` | `canonical_compiler_proof_inputs_v4` | `0x20` |
| `noop-40` | `canonical_compiler_proof_inputs_v4` | `0x40` |
| `elementwise-20` | `canonical_compiler_proof_inputs_v4_with_sourceful_family`, `Elementwise` | `0x20` |
| `elementwise-21` | Same, `Elementwise` | `0x21` |
| `elementwise-40` | Same, `Elementwise` | `0x40` |
| `collective-20` | Same, `WorkgroupCollective` | `0x20` |
| `tiled-20` | Same, `Tiled` | `0x20` |

## Current Replay Derivative

`loader.rs` accepts only those exact seed/family keys. It embeds the original
files unchanged, then creates a separate in-memory derivative by rerunning the
current original-coordinate induction analysis on the captured semantic MIR.
Only the complete final nested V1 induction report in V4 correspondence is
replaced. Source/function identities, examined-addition counts, all certificate
contents and coordinates, report length, and every enclosing correspondence byte
must remain unchanged; only analysis work accounting may differ.

The other four artifacts are retained byte for byte. The finalizer's existing
validators still check all stages, their bindings, and exact current induction
replay. The previous hex-encoded historical fixture in the shared support file
is not modified. Neither fixture import nor replay grants authority by itself.

## Captured SHA-256 Digests

Captured on MI300X on 2026-09-09. The historical capture test passed (one test),
and all 35 transferred files matched the SHA-256 values printed by that run.
These hashes describe the original binary files, not current replay derivatives.
Paths below are relative to this directory.

```text
419c7a79a4f93ece7f7d9d8022378ea867e6821c1ae6764d2e6d4cc409ec37d3  collective-20/correspondence.bin
017323329d5076c65df0d779073e08a00481a301d7311292bfdce2b91c71e8dc  collective-20/formal-memory.bin
6daf17e66bc1f317b4ffb70aa85f3c7800555deac90992d60adf8114905d46b7  collective-20/kernel-ir.bin
8962d1fba6cdd6d35597c4f626d801fc2c412e28b49d5e0ecd0caa7c9287ff0d  collective-20/middle-end.bin
4db6dd66f8bb95c73fd3dfd7c566eea0cf3a1602b27df47caa8246210c0a6774  collective-20/semantic-mir.bin
7ff98acafac60ebb64b94fac7b9d1a523d52bd5b0af235423166e403e5409ccc  elementwise-20/correspondence.bin
d0e233105b34fe7bf8d68d857685a2bae020c25ac2d87f8a7272067a261940d9  elementwise-20/formal-memory.bin
93a2e42397e340b354c490f51191bc75f593aacefcff6e9d553d9eb5d94bf77a  elementwise-20/kernel-ir.bin
6d7e27e5fd97433fc806154283bbb22bab910230b3576b5f69b89ccf4495846c  elementwise-20/middle-end.bin
012c04664f5b0b2ec3253927de40f4250639d06315436b60994e6127de146c91  elementwise-20/semantic-mir.bin
6ad8ba524745e4ea71e53d002d4a3af8c6af1b5781cf2d7facc8fec2240b6dc1  elementwise-21/correspondence.bin
115bd549db58715a8c672e5703cdbb9f1675a5abfff902b673a670d2f758c0b0  elementwise-21/formal-memory.bin
9a2b3d557351e0a2b319e4d1bac3ec0c4057ad0e7441bba951d8d79561145127  elementwise-21/kernel-ir.bin
1593f77b049ce1f3669cca6836e55800c4d17e6b0c33fe70929010f46ad089ec  elementwise-21/middle-end.bin
0094dc02ee8c58aa512b38e85a6a7e048ddc4538925d491dd6a5c171db8e0a57  elementwise-21/semantic-mir.bin
a127a32fc38f8fd872903b054c3e9c0d16559099bfcc8884a2f3c51cf9f5f24f  elementwise-40/correspondence.bin
a01001617c17c751194886f9fb75cd2713cfd9c4082c90682fdb0093382898bc  elementwise-40/formal-memory.bin
0bd3bb7a55e37df1e69a5ea2c70f3c9b222caaa9f6ae87bc2a3f503143c94954  elementwise-40/kernel-ir.bin
4fb1eeb2df11951383b75624cf80aee6bd9ebb11d8ef81564a40ffe7e62c1276  elementwise-40/middle-end.bin
ea127df05815d1d7d6b3e983c99ed08ee7c11cbe4ce6096e0063a779935d4334  elementwise-40/semantic-mir.bin
fd171f9ffecc60cc4118b8e84c1b084b3374389b432f6b3478f04cff8a4c2d4d  noop-20/correspondence.bin
5043d1e15302e616ece0e5634a0b273e9331f7f80e752a6485a487ba5729fa1e  noop-20/formal-memory.bin
63da82e8bf8cce2af014502b9dcf0de798a463313e3d9a464a53a85bd0558c34  noop-20/kernel-ir.bin
4d423d6167bce51eb303543e3eca5e596702cc3cb1866e63bb68ae369a6ff917  noop-20/middle-end.bin
8d96f7486ca30a7591c4126dcdc41962dceee8dba244dd52dc872261be215db3  noop-20/semantic-mir.bin
29dcef9b0e2fc8cd453a23ced7925e182fdde9dbaddf9fbf172fd9796cee1495  noop-40/correspondence.bin
8729552073459bf6e672ce50abef491b9a87b1c86131b0b6c98729346def086a  noop-40/formal-memory.bin
b6d7f518f7e9ad23dbdc69965d299acc73b1c048f250fd7e4df18a2d2e1f0406  noop-40/kernel-ir.bin
97e06be61a67898ae013256e06c48b0a30f5c4650ecd8895b306d01c8f0c1557  noop-40/middle-end.bin
8d4b378f56a09a29ea95f35e9afd537c3b9e35ffc134cd015d7a519e933c5a8f  noop-40/semantic-mir.bin
9c30aa5c5bb1c608380d9d4c271fb99ab0e832ab8cb7e1122f15cf13672a11a5  tiled-20/correspondence.bin
ba554d58f182158c3cebe06c45c872d3b835e4ad4f2e52ec74c4161fc14caeb9  tiled-20/formal-memory.bin
cd0ae62020b966684443ab8eb25e9f0b2e04fe019d22689ff8b79fcae3106e5a  tiled-20/kernel-ir.bin
5001533c018c8a0308dd29713106966c00c8aef58c6a6a3ace17c0e5938d9997  tiled-20/middle-end.bin
25e6d7d26eb894964f1a41e4d56a62e4359db730bd8a205636df049c28a1fd16  tiled-20/semantic-mir.bin
```
