# Fixed disabled physical-V2 source contract

The manifest is a closed versioned integrity selector, not a source/native owner. Its canonical JSON digest is fixed in the verifier. All upstream/predecessor/reader/patch/license/API-header bytes are exact. Unknown keys, missing/duplicate/reordered source rows, altered hashes, stage substitutions, source activation and coordinated manifest changes refuse.

The60 ordered source rows form one external-source acceptance boundary. All stages pin the published R4 false activation header. The measured extended stages are:

| Stage | Present files | Exact bytes |
| --- | ---: | ---: |
| one-stop-disabled-r4 |56|2,135,571|
| physical-snapshot-disabled-v1 |58|2,148,853|
| physical-publication-disabled-v2 |60|2,162,093|

The R4 stage here has eight extra context rows beyond the old parent's48; it is not the old verifier's1,963,940-byte roster. The reviewed NEW source-only maximum is2112*1024. The final union exceeds the OLD2MiB by64,941, including the R3 initializer's10 bytes; it leaves595 under the new ceiling. All new sink-proof contexts (ui-file.c,utils.c,ui.c,posix-hdep.c,ui-out.h,mi/mi-out.h) contribute to the SAME cumulative sum. Do not drop contexts or split a pass into independently accepted partitions if it grows. Refuse on any overage. Per-file512KiB/metadata64KiB and every native runtime counter remain unchanged.

Snapshot patch changes7 exact source leaves. Corrected V2 patch changes9 from that intermediate. Nine final standalone hooks are copied under src; full upstream amd-dbgapi-target.c/ui-file.h/pager.h/mi-interp.h are patch-only. The upstream MI default null initializer makes the output check independent of old allocator contents; gdbsupport/new-op.cc only explained the historical defect and is not needed in the corrected source acceptance proof. It is not secretly read outside the combined cap.

The old V1 parent read-only verifier remains unchanged and should reject final changed source as its own R4 stage. Choose the explicit new verifier. It validates matching selected files only; unselected checkout bytes, live concurrent mutations between files, toolchain/loader closure, build result, source/ABI equivalence and hardware safety are not established. A passed source check cannot construct a runtime profile or current stop.

Package helpers never execute a child or modify a tree. External source placement tests import only the bounded source reader and inspect text. Mandatory false activation/capture/publication and exact MI initializer are checked on the actual final tree. Parent licenses/source manifests/reused reader stay pinned; parent README append is intentionally not a source input.
