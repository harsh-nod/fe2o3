# Disabled native-result diagnostic source

This GPL-3.0-or-later physical-v6 source successor preserves physical-v5. It adds a bounded first-failure note for failed native scalar queries and target-memory reads, plus a retained debug-type hook. It does not change refusal decisions, retry calls, enable capture or publish snapshots.

The exact 63-file selected-source contract has 2,249,418 bytes; only two leaves change from v5 (+3,817 bytes). All v5 native limits and source-verifier ceilings are unchanged. The note is embedded in the adapter; the separate diagnostic scratch and actual compiled layout must be measured before live use. This selected-source check does not authenticate an entire checkout, binary or runtime.

## Reproduce source and CPU checks

Use a separately owned canonical ROCgdb source stage matching adjacent physical-v5, based on ROCm/ROCgdb commit 48b1d324e389d2ed5e19822d377ff9050770233d. First verify that stage with the v5 verifier; apply the sole patch with exact preimages and no fuzzy substitutions. This package performs no acquisition, installation or mutation.

From this directory, with trusted Node 22 and a C++17 compiler:

```sh
node --test tests/source-account-tests.mjs tests/source-io-tests.mjs tests/offset-transforms-tests.mjs tests/package-tests.mjs tests/fixture-tests.mjs
fixture_dir=$(mktemp -d)
c++ -std=c++17 -O2 -Wall -Wextra -Werror tests/diagnostic-core-test.cc -o "$fixture_dir/diagnostic"
"$fixture_dir/diagnostic"
node verify-source.mjs /canonical/absolute/source physical-native-result-diagnostic-disabled-v6
node verify-api-header.mjs /canonical/absolute/amd-dbgapi.h
```

Expected CPU controls: 56 Node groups and 12 diagnostic checks. These packaged commands require their own fresh execution; source review and earlier private controls are not that execution. The portable fixture covers the exact note body, not GDB callbacks, provider behavior or a GPU.

Compile upstream and adapter header spellings with both private include directories: `-I<provider>/include/amd-dbgapi -I<provider>/include`, derived from one pinned header. Link and observed-loader qualification are separate obligations; do not fall back to the installed provider. See the separate MIT provider package for its restricted positive-health semantics.

Every verification report retains native and milestone authority false. A full fresh build, actual layout, whole-source/product currentness, observed loader startup and separately authorized same-stop native session remain required. No binaries, machine paths, leases or historical runtime receipts are shipped here.
