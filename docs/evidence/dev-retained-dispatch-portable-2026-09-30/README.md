# Portable Retained-Credit Dispatch Campaign

Signed candidate: `b27f19993e50a8e99faddeee5cd6dfb79a6c8629`.
Integration: `0c38a20c6`. The fourteen executable proof inputs are unchanged;
integration preserves the newer merged source guard and development history.

The repository-contained public entry point ran from outside a clean relocated
checkout and completed all **33 stages**: three full 41/0 positives, including
generated-source relocation, all 25 family-bound logical negatives, signature,
15 synthetic controls, source calibration and both pinned release checks.
All 6,314 selected working files matched signed Git blobs. Independent raw
replay checked all 165 stage artifacts and all proof classifications. All 33
fresh groups closed in the recorder's unchanged PID namespace; no historical
groups were probed and no host-wide absence was claimed.

Source, tool, raw-log, namespace and generated-tree closing checks all pass.
This is a newly measured host-tool profile, not reuse of the earlier S2 host
qualification. Verus/vstd/Z3 are pinned by the 190-file release manifest. Host
Python/system libraries, dynamic linking, Rust compiler/libraries, kernel and
procfs remain explicit trust boundaries. No Cargo build or GPU work occurred.

## Artifacts

`raw.tar.xz` contains the complete `campaign-1` output, including generated proof
trees, every command/result/raw receipt and the opening/closing inventories.
Original absolute paths remain in the historical receipts; relocation creates
new receipts rather than editing these. The full checkout, installed tools and
the older rejected S1/S2-development packets are not included.
Archive readback matches every original file. Its SHA-256 is
`929e14d685842d3c71d9230f3a7e26a3992a0cbd5908baf282f4388f5721c9dc`.

`signed-candidate.bundle` preserves the exact signed candidate and its original
S1/S2 ancestry above base `219cd1dbde253ff397d4d0a1ccbeb73e0807a8c1`.
That base is available in the runtime work branch. Bundle verification passes;
its SHA-256 is `0fbbfab3c3156a0f7dc728c49c56def08f23464f1bffcb138fb5b6f6be09cf9f`.

| Receipt | SHA-256 |
| --- | --- |
| `results.json` | `5956a10ae5cb10340c63e694991b63abdd76fd0904af5c9d365647e67d340673` |
| `source-before.json`, `source-after.json` | `51d4820c2f86fafa25c985b51dd584a1f5783bb621d028f0254a19cce6c4b9f0` |
| `tools-before.json`, `tools-after.json` | `88689d8aa68f818680683ef66d8aaf58b7880c50b75c1e5eb476af084497d150` |
| `raw-results.json` | `ae0859e443d46f2465b95d7938b35ccc177666c8ad7d1f66abd418444c65e190` |
| `census-after.json` | `ee30f1ea00abc5f6f5125f13ed14ca279c4ebee4f00ba23ce68de58b90152a04` |

## Rerun

From a checkout containing the prerequisite base, import the candidate bundle:

```sh
git bundle verify docs/evidence/dev-retained-dispatch-portable-2026-09-30/signed-candidate.bundle
git fetch docs/evidence/dev-retained-dispatch-portable-2026-09-30/signed-candidate.bundle HEAD
git worktree add --detach /absolute/clean-checkout b27f19993e50a8e99faddeee5cd6dfb79a6c8629
```

Install the pinned Linux x86_64 Verus release and Rustup 1.97.1 toolchain, plus
the host prerequisites in [the component documentation](../../runtime-retained-credit-dispatch-v1.md#portable-campaign).
Run the public entry point, using a fresh output directory outside the checkout
and verifier:

```sh
python3 -I -B /absolute/clean-checkout/crates/fe2o3-runtime-model/verus/check-retained-credit-dispatch.py \
  --campaign --verus /absolute/release/verus --output /absolute/new-evidence
```

No private qualification helper or historical result is consumed. Success
requires process exit zero and all fresh campaign checks. This packet does not
qualify concrete Arc/Mutex/map correspondence, concurrent freshness, native
authority, CPU behavior, performance, HIP/HSA parity or any A0-A7 milestone exit.
