# Disabled scalar checkpoint presentation

SPDX-License-Identifier: GPL-3.0-or-later

This separate physical-v8 package keeps physical-v7 unchanged. It retains the
first-failure checkpoint diagnostics and corrects one scalar host presentation
check: require lane mask 1 (lane zero active), instead of mask 0.

The selected host is little-endian x86_64. GDB initializes a breakpoint status
with mask 0, but assigns thread_info::active_simd_lanes_mask() during condition
processing, before presentation. Its hookless scalar implementation returns 1.
The correction changes only this presentation requirement; it does not change
GPU masks, earlier status checks, architecture restrictions, other guard clauses,
queries, reads, ownership, cleanup, retry rules, or any of the three disabled
activation/capture/publication gates.

A checkpoint-site=11 failure identifies the compound presentation guard, not
which operand failed. Source establishes the scalar contract mismatch; neither
that diagnostic nor this correction proves the other operands passed or a
subsequent physical capture succeeds. The [fixture contract](tests/scalar-contract.json)
records source provenance and the limits of its CPU scaffolding.

## Reproduce source and CPU checks

The single patch is cumulative from the exact adjacent physical-v6 selected
source contract, based on ROCm/ROCgdb commit
48b1d324e389d2ed5e19822d377ff9050770233d. It composes the 44 elementary v7
diagnostic edits with the one scalar correction, in 44 normalized, reversible,
non-overlapping ranges across seven files. The scalar edit is inside an existing
range. Do not apply this cumulative patch on top of v7. Complete preimages,
postimages, transforms, and the intermediate v7 owner are provided.

From this directory, with trusted Node 22 and a C++17 compiler:

```sh
node --test tests/source-account-tests.mjs tests/source-io-tests.mjs tests/offset-transforms-tests.mjs tests/package-tests.mjs tests/fixture-tests.mjs tests/checkpoint-tests.mjs tests/scalar-tests.mjs
fixture_dir=$(mktemp -d)
c++ -std=c++17 -O2 -Wall -Wextra -Werror -pedantic tests/diagnostic-core-test.cc -o "$fixture_dir/native-result"
"$fixture_dir/native-result"
c++ -std=c++17 -O2 -Wall -Wextra -Werror -pedantic tests/checkpoint-core-test.cc -o "$fixture_dir/checkpoint"
"$fixture_dir/checkpoint"
c++ -std=c++17 -O2 -Wall -Wextra -Werror -pedantic -fsanitize=undefined -fno-sanitize-recover=all tests/checkpoint-core-test.cc -o "$fixture_dir/checkpoint-ubsan"
"$fixture_dir/checkpoint-ubsan"
c++ -std=c++17 -O2 -Wall -Wextra -Werror -pedantic tests/scalar-presentation-test.cc -o "$fixture_dir/scalar"
"$fixture_dir/scalar"
c++ -std=c++17 -O2 -Wall -Wextra -Werror -pedantic -fsanitize=undefined -fno-sanitize-recover=all tests/scalar-presentation-test.cc -o "$fixture_dir/scalar-ubsan"
"$fixture_dir/scalar-ubsan"
node verify-source.mjs /canonical/absolute/source physical-scalar-checkpoint-presentation-disabled-v8
node verify-api-header.mjs /canonical/absolute/amd-dbgapi.h
```

Expected controls: 72 Node groups, 12 retained native-result checks, 56 retained
checkpoint checks, and 300 scalar checks. The scalar fixture includes the exact
full old/new predicates and complete thread method, with explicit mocked GDB
objects, thread/architecture hooks, and assertions. It covers masks 0 through
255, every retained predicate clause, hook values and exceptions, and the
constructor-to-condition assignment order. These are not actual GDB callback,
target, provider, loader, or GPU tests. Run this package's checks freshly;
historical private or v7 results do not transfer.

## Bounds and limitations

All 63 selected roles remain, totaling 2,254,118 bytes under the unchanged
2,293,760-byte ceiling. All source-verifier and native ceilings remain unchanged.
Every packaged fixture and provenance document is separately charged by the
single existing source account; ancestor code is not imported recursively.
Only the adjacent physical-v6 manifest is an external package dependency.

The scalar correction adds no fields, allocations, native calls, or reads.
A future private build must still preserve and remeasure all 18 diagnostic
layout types and the logical reservation under the unchanged 64 KiB native cap.
Prior object sizes are not a new measurement. A fresh complete build, whole
source/product currentness, exact loader binding, startup and owned-process
prerequisites, and a separate finite one-attempt native lease remain mandatory.
No automatic retry is enabled.

Selected-source checks require exact disabled source, not a private-enabled
tree. Upstream full-file pins in the scalar fixture are provenance only: they
do not silently expand the selected63 verifier into a whole-source checker.
The verifier authenticates declared package payloads and selected files, not an
atomic snapshot, all metadata calls, RSS, a debugger binary, continuous isolation,
runtime acceptance, or milestone completion. No private paths, leases, binaries,
or runtime receipts are included. Original GPL notices and COPYING are retained;
the separately packaged private ROCdbgapi provider remains under its own license.
