# Private owned-queue health source

This MIT package contains six exact changes and one new header for ROCm/ROCdbgapi commit 06465e940698e8423d1b629c834c98bdd7753439. It is a private producer experiment for the fixed owned-runtime workflow, not general queue-error support or an installed ROCm replacement. Upstream notices and LICENSE.txt are preserved. The separate GPL debugger adapter is not included.

STATE/ERROR_REASON return only positively proven VALID/NONE: a newly enabled supported live runtime, successful continuous fault subscriptions, one empty baseline before runtime acknowledgement, known queue birth, unchanged queue geometry, a fresh complete exception-free snapshot, and no sticky lifecycle/fault/forwarding taint. Attached-existing, unknown, error, unsupported and exhausted cases refuse without fabricating clean status. This does not implement public ERROR results, nonzero ERROR_REASON, QUEUE_ERROR events, notification or acknowledgement.

The private driver path uses one capacity-one, clear-none, non-retrying ioctl for each admitted observation. One baseline and at most ten health queries are prepaid and sticky. Nonzero results, EINTR, unexpected ABI fields, unknown bits, queue flags and geometry changes refuse. Eligible suspension preserves exception bits so faults arriving after a preceding event query are not silently cleared. Forwarding a non-runtime exception taints first; a raw trap bit is not declared benign.

## Reproduce the selected-source transformation

Obtain the exact upstream commit in a separately owned source directory; do not use or overwrite the system installation. Run the read-only verifier against the original six files and the exact API/KFD headers before applying patches:

```sh
node verify-package.mjs
node verify-package.mjs preimages /canonical/upstream /canonical/amd-dbgapi.h /canonical/linux/kfd_ioctl.h
git -C /canonical/upstream apply --check /canonical/package/patches/0001-private-owned-queue-health.patch
git -C /canonical/upstream apply /canonical/package/patches/0001-private-owned-queue-health.patch
node verify-package.mjs postimages /canonical/upstream /canonical/amd-dbgapi.h /canonical/linux/kfd_ioctl.h
```

The patch changes only the six pinned leaves and adds src/queue-health-v1.h. The verifier validates all package pins, exact forward/inverse edits and mock-body source joins. Selected-file matches are not whole-checkout, compiler, library, loader or hardware attestation. The verifier itself never modifies a source tree.

## CPU controls

With trusted Node 22 and a C++17 compiler, use the exact API header directory and upstream source directory containing the pinned linux/kfd_ioctl.h:

```sh
node --test tests/source-tests.mjs
fixture_dir=$(mktemp -d)
c++ -std=c++17 -O2 -Wall -Wextra -Werror -I src tests/queue-health-core-test.cc -o "$fixture_dir/core"
"$fixture_dir/core"
c++ -std=c++17 -O2 -Wall -Wextra -Werror -I src -I /canonical/api/include tests/production-body-control.cc -o "$fixture_dir/production"
"$fixture_dir/production"
c++ -std=c++17 -O2 -Wall -Wextra -Werror -I /canonical/api/include -I /canonical/upstream/src tests/driver-body-control.cc -o "$fixture_dir/driver"
"$fixture_dir/driver"
c++ -std=c++17 -O2 -Wall -Wextra -Werror -I src -I /canonical/api/include tests/lifecycle-callsite-control.cc -o "$fixture_dir/lifecycle"
"$fixture_dir/lifecycle"
```

Expected checks: 8 Node groups, 172 portable, 45 production-body, 115 driver-body and 174 lifecycle-callsite checks. The fixtures do not link the provider or invoke real driver/API/GPU operations. Production and driver fixtures use exact extracted bodies plus class/enum/exception/output scaffolding; lifecycle fixtures cover exact changed blocks, not full process methods. API and KFD headers stay external with explicit pins; platform/compiler headers remain a trusted toolchain input. Packaged commands require a fresh run.

## Bounds and remaining qualification

Source verification uses one sticky cumulative account: 2 MiB requested file bytes including EOF probes, 256 content-read calls, 64 roles, 1 MiB/member, 64 KiB/manifest, 64 KiB chunks, 64 MiB declared logical work, 8 MiB declared reconstruction bytes and a 10-second checked deadline. It does not meter module-loader reads, metadata operations, JavaScript heap/RSS or compiler-internal work. Refusal has no refund or retry.

The provider's new observation domain is distinct from the debugger's API/file budget: at most 11 ioctl attempts and capacity-one rows, no new explicit driver heap, trace formatting, file reads or descriptors. With the qualified 32-byte arguments and 64-byte KFD row, conservative requested copy bounds are 1,056 bytes out and 352 in (1,408 bidirectional); this is not a kernel-work bound. Actual private build measurements were process +8 bytes and unchanged queue/OS/KFD row layouts; these are target-specific observations, not portable ABI promises or whole-process memory measurements.

A full private provider build, actual layouts/stack frames, exact closed loader paths, whole source/product custody and a fresh debugger startup must still be checked. Keep both private include roots for flat and nested header spellings. Do not use environment library injection or an empty/relative RUNPATH. Source/CPU success cannot grant a native lease, qualify a capture, increase debugger limits or complete a milestone.
