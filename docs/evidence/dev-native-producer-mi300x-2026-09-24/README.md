# Native producer-chain qualification

Status: both bounded native producer witnesses pass on MI300X GPU 1,
`0000:26:00.0`, UID `0xab83d2ffef0d3cdf`. Source is the signed
`f3f8206fe4727f1f535ad820721fff7e7bdd4a39` CPU-qualified witness checkpoint.
The two cases use the unchanged R57 authority for queued and already published
native producers. The historical runtime matrix remains unchanged.

The cold musl ELF passes all 1,400 CPU tests with 22 hardware-only ignores.
Both exact ignored tests then pass natively: four kernel launches total, two
authority calls per case, 1,048,576 checked bytes per case, and no persistent
input materializations. All six strict endpoint observations pass, with no
refusals, across ten remote commands. All eleven local build and eight local
campaign commands pass with closed process groups. This is shared-host
qualification, not an exclusive reservation.

The sibling protocol runs only the two appended tests and requires complete
A/B/C/D byte oracles, exact retained input and completion custody, released
public events, consumer-first observation and explicit shutdown. Both tests
distinguish backend retirement from logical Context reconciliation.

The retained protocol derives from the earlier current-source matrix. New
controls capture helper bytes before execution, bind the new CPU source/roster
and two-case suffix, and harden local collection with a pinned verifier and a
full immediate pre-deletion owner/type/device/inode/content/process check.
Separate before/after cleanup receipts preserve removal history.

`audit1` records eighteen passing protocol-test groups and five passing cleanup
calibrations using disposable local trees only. The native controller collects
all remote artifacts byte-exactly before marker-bound removal and an independent
path/process absence check. `collect.py` retains 138 source, executable and
receipt files before removing 530,509,824 path-accounted allocated bytes of
exact-owned local scratch. Separate immutable before/after retention receipts
and independent retained replay pass. The final `audit.py` invocation requires
full replay and all sixteen hostile-record/cleanup test groups without skips.

The retained ELF SHA-256 is
`d509551a409e1de521926c586f4b7ce693a745acaa294a7dc459ce008670e3c6`.
Reproduce the offline checks from the repository root with:

```sh
python3 -I -B docs/evidence/dev-native-producer-mi300x-2026-09-24/verify.py
python3 -I -B docs/evidence/dev-native-producer-mi300x-2026-09-24/test_verify.py
```

See `PROTOCOL.md` for strict endpoint windows and rejection semantics. Protocol
helper bytes are captured and bracketed separately; they are not represented as
blobs in the runtime source commit. No physical overlap, fault campaign, new
formal refinement, aggregate memory, matched performance or A1/A2 closure is
claimed by these two bounded native witnesses. Native R125, Admission R118B
C1/C2/C3, Resources R116/V3 and broader milestone acceptance remain unchanged.
