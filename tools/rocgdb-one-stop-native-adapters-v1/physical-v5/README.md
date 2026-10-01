# Disabled debugger maintenance source package

This GPL-3.0-or-later package provides the reviewed callback, breakpoint-refresh,
terminal and unload maintenance source for the physical-v5 debugger adapter.
It is a source and CPU-test package, not an enabled live debugger backend.
Selection, capture and publication remain disabled in the actual source.

The package preserves physical-v3 and physical-v4 unchanged. Its explicit
63-file source contract includes all 38 selected stock ROCgdb files and 25
adapter files. Matching these files does not verify an entire checkout,
a compiled debugger, runtime libraries, a GPU session or a milestone.

## Run the CPU controls

Use a trusted Node 22 executable and a C++17 compiler. From this directory:

```sh
node --test tests/source-account-tests.mjs tests/source-io-tests.mjs \
  tests/offset-transforms-tests.mjs tests/package-tests.mjs tests/fixture-tests.mjs
fixture_dir=$(mktemp -d)
c++ -std=c++17 -O2 -I src tests/breakpoint-refresh.cc -o "$fixture_dir/breakpoint-refresh"
"$fixture_dir/breakpoint-refresh"
c++ -std=c++17 -O2 -I src tests/unload-maintenance.cc -o "$fixture_dir/unload-maintenance"
"$fixture_dir/unload-maintenance"
```

The expected results are 59 Node checks, 46 breakpoint-refresh mock groups and
54 unload-maintenance mock groups. These commands passed on MI350's host CPU
on 2026-10-01. The mocks exercise exact extracted method bodies against inert
doubles; they do not reproduce GDB object lifetimes or GPU callbacks.

## Verify the selected source and API

Start with a fresh, separately owned source stage matching the adjacent
physical-v4 manifest. Apply the single patch listed in `patches/series` only
to that stage, with exact preimages and no fuzzy substitutions. The verifier
does not acquire or patch sources for you.

```sh
node verify-source.mjs /canonical/absolute/source \
  physical-callback-terminal-maintenance-disabled-v5
node verify-api-header.mjs /canonical/absolute/amd-dbgapi.h
```

Both source and API checks passed for the exact packaged contract. Every report
keeps native authority and milestone completion false. Missing, altered,
noncanonical or incomplete inputs fail; a partial report is not success.

One command uses one cumulative account for its own manifest, the directly
pinned parent manifest, declared package payloads and requested source/API
files. It does not invoke ancestor validators with fresh budgets. Ten changed
files have exact forward and inverse byte-range checks. A short read fails
without retry or refunded credit.

The selected source is 2,245,601 bytes under the separately versioned
2,293,760-byte ceiling. The stronger combined source-and-API path reserves
4,551,516 requested bytes, 234 reads and 46,041,508 logical work units.
All original native resource limits remain unchanged. These are declared
payload and logical-work bounds, not JavaScript heap, RSS, compiler-internal
I/O or total debugger memory measurements.

## Remaining live qualification

A fresh full debugger build, actual compiled type layout, startup/runtime
checks and a separately qualified same-stop GPU session are still required
before live capture can be accepted. No file, environment variable, successful
CPU test or source report enables these public gates.

The guide is documentation, not an additional authority-bearing payload in
the closed runtime manifest. See `source-manifest.json` for exact selected
pins and `LICENSE.md` for the license boundary.
