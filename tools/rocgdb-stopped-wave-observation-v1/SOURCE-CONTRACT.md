# Native source joins and integration contract

SPDX-License-Identifier: GPL-3.0-or-later

## Base and unchanged predecessor

This patch applies to the exact lifecycle stage of the sibling runtime observation package on ROCm/ROCgdb commit 48b1d324e389d2ed5e19822d377ff9050770233d. source-manifest.json pins all seven shared preimages, four added leaves, eight unchanged context files, the sibling runtime header/checker and external AMD API header. The predecessor package is not changed.

Only standalone hook/header additions and a deterministic patch are shipped. Includes from existing translation units need no build-system registration or second AMD client. Exact source checks and static builds do not establish execution authority.

## Actual event lineage

1. The existing successful amd_dbgapi_process_attach path supplies the actual inferior-info owner to the new binder. Positive signed inferior/PID checks occur before conversion. Existing attach completion calls the new attached hook too. A preloaded runtime during attach is a refusal.
2. The actual native event wrapper queries the AMD event's process and kind before handling. Its existing event_ack remains the only caller of amd_dbgapi_event_processed. The new unwind guard is destroyed before that sole ACK fallback and records refusal; it never acknowledges independently.
3. Actual runtime loaded-success completion, code-object completion and real breakpoint callback identity/action joins feed this ledger, without an importer or caller-selected event plan. Unknown events invalidate.
4. In process_one_event, the original R6 noqueue WAVE_STOP refusal is untouched. After the real WAVE_STOP handler creates/finds its actual GPU thread and queues the wait status, the new query obtains its actual event wave and complete tuple. The existing ACK executes exactly once; only success permits staging.
5. MI's on_normal_stop has already printed/rewound/flushed *stopped before the new hook runs. The old noqueue safe-point flush remains in place. The new hook requires the actual selected GPU thread and complete second native tuple equality before publishing kind 5.
6. Native rows only drain from existing event/MI safe points. Prompt output does not publish or renew a stop. Notification failure consumes the attempted row and invalidates; no retry reconstructs success.
7. Both real mi_execute_command overloads invalidate at entry, before parse/dispatch/selection, including the separate Python execute_mi path. The raw-string entry also covers CLI-through-MI and EOF. There is no register/memory command exception.
8. Native resume/store/xfer-write and thread changes are defense in depth. Existing lifecycle invalidations broadcast to the new ledger. The original direct pre-bind invalidation behavior and lifecycle conditional behavior remain separate helpers; a new observation cannot accidentally change old noqueue reset semantics.

## Exact native query contract

Only fixed-size get_info queries are added. The pinned API explicitly defines the sizes and enum meanings:

- event process/kind/wave;
- wave state, stop reason, process, workgroup, dispatch, queue, agent, architecture, lane count (size_t), group coordinate (uint32[3]), wave number;
- workgroup process/queue/dispatch/agent/architecture/coordinate;
- dispatch process/queue/agent/architecture, packet ID, dimension count, grid uint32[3], workgroup uint16[3], private/group byte counts;
- queue process/agent/architecture, OS ID, type/state/error-reason;
- agent process/architecture/OS ID/supported state;
- architecture ELF machine.

Every parent edge is joined to the wave's actual IDs and actual retained native process. Zero OS queue/packet IDs are legitimate and retained. Opaque library IDs and OS agent ID must be nonzero. No public address, PC, EXEC value, list, register, memory or allocated architecture-name query is used. The profile checks supported agent, no queue exception, exact dimensions and geometry, stopped DEBUG_TRAP Wave64/gfx950 and zero segment sizes. Initial and final state/reason must match, and the actual GDB process target must still resolve the identical thread object.

Same-thread normal-stop requery repeats 41 queries. The query path before ACK includes 44 calls (two existing event owner/kind checks, one event-wave query, then 41 tuple checks). Failure at every query position has a synthetic CPU control; no performance/timing or external-writer atomicity claim follows. Native API NOT_AVAILABLE fails closed.

## Currentness and authority

Native opaque handles are library-lifetime identities; their equality is not source identity. There is one lifetime and one stop generation. Any stop change, command, resume, selection/write, unknown event, lifecycle failure, unsupported profile, capacity failure or output failure expires the observation. No method can construct a read/dispatch capability from a row.

The selected single-wave geometry alone does not prove an owned queue, owned code object, matching compiler entry or safe resource lifetime. This donor deliberately exports queue-provenance unavailable even for kind 5 and omits such success fields. Actual source/code-object address joins, closed physical register roster/lane semantics, owned readable memory extents/digests, trap/CWSR/TTMP release, and queue lifetime/teardown are all future prerequisites.

## Integration and qualification

Follow README.md for the explicit separate-source, external-header and CPU boundaries. Historical overlay CPU/static-build receipts are identified in historical-evidence.json; package adaptations need independent tests. No native invocation is supplied. Queue creation, fake-success DTOs, address/read helpers and sample admission constructors are absent. This is observation infrastructure, not stopped-wave sampling or completion of hardware-debugger milestone V4.
