# Portable historical loaded-file review APIs

The six pure review/planning modules do not open files or launch a process. Historical absolute path strings in their bindings are inert provenance; they are not fixture search locations or permission to read those paths.

This directory deliberately ships no raw host receipts or 14 MB fixture archive. It separates fixture-free API/admission checks from historical semantic replay.

## Fixture-free checks

From the repository root:

```sh
node --test tools/debugger/loaded-profile/portable-api.test.mjs
```

These controls cover public entry availability, admission/JSON boundaries, immutable fixture-manifest pins and role/path validation. They do not claim historical startup, module/map lineage, debugger execution or loaded-file semantic qualification.

## Explicit historical controls

The historical-profile-controls.mjs and historical-selection-controls.mjs filenames are intentionally not *.test.mjs and are not auto-discovered as default tests. They preserve the original 173 profile and 77 planner controls. To run them, supply all 76 complete historical fixture files through an explicit manifest:

```sh
FE2O3_LOADED_REVIEW_FIXTURES=/absolute/path/fixtures.json \
  node --test tools/debugger/loaded-profile/historical-profile-controls.mjs \
  tools/debugger/loaded-profile/historical-selection-controls.mjs
```

The environment variable is required. Missing configuration, missing files, changed bytes, reordered/duplicate roles, alias paths or incomplete reads fail the command; no test silently skips or falls back to a path embedded in a historical binding.

The manifest is ordinary UTF-8 JSON with exactly two keys: schema (the string fe2o3-loaded-review-fixtures-v1) and files (exactly 76 entries). Each entry has exactly role and path. Roles must follow the immutable FIXTURE_ROLES array exported by loaded-fixture-manifest.mjs: first the 72 profile roles, then the four selector roles. Paths must be distinct canonical absolute locations of your supplied fixture files. Content lengths and SHA-256 hashes come only from the source-owned bindings; the manifest cannot override them.

For example, the beginning of the files array is a list of {role: FIXTURE_ROLES[i], path: actualLocalFixturePathForRole(i)} records. Construct all 76 records from your explicit local mapping; a partial example is not a valid manifest. Files may live on a different host or under a different directory, but their complete contents must match the retained historical pins. There is no download, search, source import, evaluation or automatic reconstruction step.

## Fixture-reader bounds and custody

The shared fixture reader admits a manifest of at most 64 KiB, fixture members of at most 8 MiB, and an inclusive 32 MiB aggregate / 1,024 content-call ceiling for the manifest and all 76 fixtures. The fixed fixture payload is 13,854,356 bytes; with EOF reservations it is 13,854,432 bytes and 347 exact-or-refuse content calls. The nonempty manifest adds its actual byte length plus one EOF byte and two content calls. These are per historical-control process; running both control modules loads the fixture set independently in each process.

The manifest and all 76 fixture files are checked as canonical regular named paths, opened read-only/no-follow/nonblocking, and read in exact-length-or-refuse 64 KiB chunks, followed by one EOF probe. All 76 fixture contents are checked against their immutable source-owned SHA-256 pins. The manifest is bounded, read, parsed and identity-checked, not hash-checked by this reader; a root qualification gate may independently pin it. Short reads do not retry. Named and descriptor device/inode/size/full mode/uid/gid/mtime/ctime are bracketed; all fixture names and the manifest are revalidated after the complete set. Descriptors close in finally. All fixture bytes, including any .mjs source fixtures, remain data rather than imported modules.

The limits are source-policy bounds, not an exact V8 heap or global process-IO theorem. Metadata syscalls and Node's module-loader IO are not included in the content counter. The historical profile/planner make their own complete private copies and full pin checks. An external scheduler owns test timeout.

Passing these historical controls is historical source/data qualification only. It is not a fresh read of the old operational binaries, an active successor input graph, a larger gate approval, a lease, GDB startup permission, inferior execution, GPU dispatch or physical capture. The previous failed startup remains preserved in the historical profile.
