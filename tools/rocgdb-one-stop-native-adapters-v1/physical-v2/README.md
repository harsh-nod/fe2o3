# Disabled physical snapshot and V2 publication — separate GPL overlay

This opt-in source package layers on the exact published disabled R4 parent. It is not an in-place V1 protocol replacement. Parent V1 source, manifests, patches and verifiers remain unchanged; only its README links here. New and changed standalone GPL hooks are shipped, with upstream changes represented solely by exact patches. No full upstream checkout, built debugger, acquisition, patch-application, install or launch automation is bundled.

All THREE published source gates remain false: R4 selection, physical capture and V2 publication. The Rust [V2 consumer](../../gfx950-one-stop-controller-v1/physical-v2/README.md) separately retains PROFILE=None and null runtime bindings. These packages are not operational debugger tools or a way to enable a target. No older private R5/R6/R7/R8 ELF or startup receipt is transferred.

## Source chain

Use exact ROCgDB commit48b1d324e389d2ed5e19822d377ff9050770233d plus the existing runtime/stopped-wave/disabled-R4 parent layers. Then the explicit two-patch series adds the disabled snapshot and the corrected disabled V2 publication. The second patch coherently includes formatter R1, sticky-stdio sink R2 and MI saved_raw_stdout initialization R3. No intermediate R1 output-sink state is presented as the final package.

The actual provisional capture/confirm/retire/seal/revoke ordering, same native owners, sole event ACK and three full native query censuses are preserved. Publication has at most16 rows and the original fixed1536-byte formatter. The retired same-stop row carries actual4/272 bytes or typed unavailability; golden/oracle values never fill a sample. Every next MI command revokes live snapshot access. The later target4096B validation is a separate report, not another memory sample. No read exception, extra target command, dbgapi client/query/read/ACK or resume path is added.

The corrected sink requires the exact selected UI/interpreter/raw pager/borrowed stdio FILE relation, no logging/tee/unknown wrapper or pending wrap, and a defined initial null saved_raw_stdout. Sticky ferror brackets the original ONE put and ONE flush. Failure poisons/revokes and never retries. A successful flush establishes checked submission only, not delivery or consumer acceptance. Its separate256-byte transient reservation and128 work/row fit the original native caps:129240/131072 source-estimated work. Native logical storage65536, API192, read65536/calls256 and all existing lifetime deadlines are unchanged.

## Explicit bounded source verification

See SOURCE-CONTRACT.md. New V2 selected source has a SINGLE combined60-row roster and source-only2112KiB ceiling (2,162,688 B); old V1 remains2MiB. This is an explicit reviewed source-check bound change, not proof the old ceiling passed. Final exact input is2,162,093 B:64,941 over2MiB,595 below the new ceiling. All six sink-proof contexts are included in that same sum. No second hidden acceptance partition exists. Per-file512KiB and metadata64KiB are unchanged. Native runtime caps above are unrelated and unchanged.

Read-only usage: node verify-source.mjs <canonical-absolute-source-root> <one-stop-disabled-r4|physical-snapshot-disabled-v1|physical-publication-disabled-v2>. No default source or activation stage exists. The inherited bounded reader uses no-follow regular file reads with growth/EOF/identity checks and strict UTF8/hash/size validation. Exact stage absences, predecessor/reader/patch pins and literal false gates are closed. The verifier does not attest every checkout file, writer exclusion, a built/loaded ELF, runtime ownership or native authority.

## Root-owned CPU/static checks only

Run the inert package metadata controls separately from external-tree placement controls; the latter require FE2O3_ROCGDB_TEST_SOURCE to name the exact FINAL FALSE projection and refuse if absent. Root can compile tests/publication.cc, tests/output.cc and tests/resource.cc with strict C++17 flags and include this src/tests plus the unchanged parent src. Tests use inert owners and the actual private submit helper; /dev/full is solely a CPU stdio-fault fixture. No test launches GDB/target or invokes dbgapi. Optional external API header verification only reads the exact pinned header.

Historical producer CPU source passed12 real-stdio groups +4 resource groups +16 inherited formatter groups and8 new +7 inherited Node controls. That R2 result precedes the R3 initialization layer; root separately passed the five additional R3 source controls under the retained combined CPU receipt. Source provenance records both exact attributions. New relocated controls and combined source still require their own root run. Full debugger build/startup/native fields remain null unless a separately completed exact receipt is added later. No uncompleted R9 or earlier R8 build is claimed here.

A fresh disabled full build may be root-reviewed separately using this exact false projection. No command to run that debugger is provided. Future activation, startup closure, same-client attach/ACK/TTMP/trap/sampling obligations, owned family and genuine stop must all be reviewed independently. This package does not close V4 or change overall6/18 accepted exits.
