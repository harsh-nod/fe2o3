# Compute-Owned XGMI Developer Tests

This packet records CPU development checks for the native peer-queue attachment
API. It does not qualify successful hardware attachment, peer mapping or copies,
a compute/XGMI pipeline, performance, HIP/HSA parity, or formal refinement.

- All 19 selected KFD tests passed, including 11 new attachment/paired-loan tests;
  1,839 tests were filtered out. Exact filters and output are retained.
- Strict all-feature KFD library/test Clippy completed with exit 0.
- All 32 existing `source-controls` workflow commands passed after integration.
  Their process groups closed, and 6,597 recorded source hashes remained unchanged.
- Four source guards changed only five identity hashes. All 22 unique executable
  proof-closure files, contracts, and qualification counts remain unchanged.
  No solver or GPU command ran as part of these checks.

The separate full 1,858-test KFD run was unfinished when this packet was prepared;
it is not a passing result. Integrated runtime-test records are deliberately not
included here and must be reported separately when that run terminates.

`records.tar.gz` contains complete source-CI stdout/stderr, command receipts,
inputs, closing source hashes, results and the original recorder; the guard
proposal report; and focused KFD/Clippy commands, outputs and exit statuses.
It excludes executable binaries, full source files and the candidate patch.
The recorder retains its original local paths; the exact portable CI commands
are also listed in `source-ci/inputs.json` and the repository workflow.

Archive: 677,027 bytes. SHA256:

```
a961dd6649484e5499e661eaa3fa124dd59a5b8c8e700881066b54e4e26f0277  records.tar.gz
```

`guards/proposal.json` identifies all ten frozen KFD files and all four guard
updates. Focused KFD and Clippy checks used the candidate based on
`dd5e61712b3e7f16c43c7bf9ecb90eaac46bbaaf`, staged tree
`13988ffe5280e02f4096592c7f68b9f501b99e08`. The source-CI run used those exact ten
KFD files integrated onto `5778c9d09711055271c7f9c8f01afd53d056ad82`, with the
four guard updates. This is developer evidence, not a signed-source proof or
hardware campaign.

`manifest.json` records each archived file's size and SHA256 and the credential
pattern scan. The scan covers every archived file and this README; it is not a
general proof that arbitrary sensitive data cannot occur.
