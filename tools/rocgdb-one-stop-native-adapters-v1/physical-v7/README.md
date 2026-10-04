# Disabled checkpoint-guard diagnostics

This additive GPL-3.0-or-later physical-v7 package preserves physical-v6. It labels the first failed checkpoint guard with a stable numeric site, without changing any of the 38 original Boolean predicates, native queries, target reads, refusal decisions, cleanup, retry rules, or public activation/capture/publication gates.

A message such as `refused (11); commit-site=0; checkpoint-site=35` identifies the sole-queue comparison guard in the shipped [site roster](tests/checkpoint-sites.json). It does not identify a particular failed subexpression or report a queue value. Refusal 11 means checkpoint consistency failed; commit-site zero is normal for a non-commit failure. A site is diagnostic data, never permission to continue.

The guard captures owner validity before evaluating the original expression exactly once. This preserves labels when an owner method itself records the first failure. Earlier failures stay sticky; exceptions propagate unchanged. Paths without an observed false wrapped predicate may keep the generic diagnostic. No missing observation is invented.

## Reproduce source and CPU checks

Start from a separately owned source tree that passes adjacent physical-v6, based on ROCm/ROCgdb commit 48b1d324e389d2ed5e19822d377ff9050770233d and its exact selected-source ancestry. Apply the single patch only to exact preimages, then verify the new disabled stage. The tools do not acquire, patch, build, install, launch, or attach a debugger.

From this directory, with trusted Node 22 and a C++17 compiler:

```sh
node --test tests/source-account-tests.mjs tests/source-io-tests.mjs tests/offset-transforms-tests.mjs tests/package-tests.mjs tests/fixture-tests.mjs tests/checkpoint-tests.mjs
fixture_dir=$(mktemp -d)
c++ -std=c++17 -O2 -Wall -Wextra -Werror -pedantic tests/diagnostic-core-test.cc -o "$fixture_dir/native-result"
"$fixture_dir/native-result"
c++ -std=c++17 -O2 -Wall -Wextra -Werror -pedantic tests/checkpoint-core-test.cc -o "$fixture_dir/checkpoint"
"$fixture_dir/checkpoint"
c++ -std=c++17 -O2 -Wall -Wextra -Werror -pedantic -fsanitize=undefined -fno-sanitize-recover=all tests/checkpoint-core-test.cc -o "$fixture_dir/checkpoint-ubsan"
"$fixture_dir/checkpoint-ubsan"
node verify-source.mjs /canonical/absolute/source physical-checkpoint-site-diagnostic-disabled-v7
node verify-api-header.mjs /canonical/absolute/amd-dbgapi.h
```

Expected CPU checks are 64 Node groups, 12 retained native-result checks, and 56 new checkpoint checks. The checkpoint fixture compiles exact production note/helper sections with an explicitly mocked owner/refusal path. These are not GDB callback, provider, loader, physical-capture, or GPU tests. Run the packaged tests freshly; no earlier private result transfers to this package.

## Bounds, provenance, and limits

The exact selected contract retains all 63 source roles and all three disabled gates. Seven leaves change, through 44 reversible byte-range transforms; the full preimages and postimages and an ordinary patch are included. The new selected byte total is 2,254,118, below the unchanged 2,293,760-byte cap. All predecessor native and verifier ceilings remain unchanged.

The new retained note is two bytes. A separate logical scratch reservation covers the diagnostic fields, a compile-time maximum of 32 reference slots for each predicate closure, and a 224-byte error payload. Its actual compiled size and adapter padding must be measured; this is not a full machine-stack, allocator, or RSS guarantee. No target or API read is added.

A future private build must preserve every old layout type and explicitly extend the existing 16-type layout contract to 18 for the new note and scratch types. It must remeasure the actual adapter and logical reservation under the unchanged 64 KiB native cap. A full build, whole-source/product currentness, exact loader binding, owned-process prerequisites, and a separate finite one-attempt native lease remain required. Do not enable gates, increase limits, or ignore a failed predicate merely to obtain a sample.

The bounded verifier authenticates selected source and declared package payloads only. It is not a whole-checkout or executable verifier, atomic snapshot, continuous exclusion proof, or runtime authority. Its filesystem metadata calls are not included in its content-read counter. Neither this source package nor a successful CPU check completes a native-capture milestone. No private paths, leases, binaries, or runtime receipts are added here.
