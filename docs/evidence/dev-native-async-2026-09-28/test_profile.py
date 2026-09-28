#!/usr/bin/env python3
"""Synthetic lifecycle calibration; never GPU evidence."""
import copy
from pathlib import Path
import runpy
from types import SimpleNamespace
import unittest

p = SimpleNamespace(**runpy.run_path(str(Path(__file__).with_name("profile.py"))))


def sealed(events, scope):
    rows = [{"sequence": i, "identity": p.event_identity(scope, i, event), "origin": "observed", "event": event}
            for i, event in enumerate(events)]
    return {"schema": "fe2o3-kfd-runtime-profile-v1", "schema_version": 1, "capture_scope": scope,
            "device": {"identity": p.domain("device", [p.UID.to_bytes(8, "little"), b"gfx942:xnack-", (64).to_bytes(2, "little")]),
                       "target_profile": "gfx942:xnack-", "wave_width": 64}, "host_content_mode": "range_only",
            "events": rows, "coverage": {"origin": "observed", "observed_events": len(rows), "dropped_events": 0,
                                         "complete_runtime_operation_history": True}, "unavailable": p.UNAVAILABLE}


def fixture(mode):
    variants = {"owner": ["Long", "Short"], "timeout": ["Long"], "drop": ["Long"],
                "backpressure": ["Short", "Long"]}[mode]
    scope = ("78" if mode == "owner" else "79") * 32
    ids = [{key: p.resource(scope, tag, 10 + i) for tag, key in
            ((2, "stream"), (3, "allocation"), (4, "module"), (5, "kernel"), (6, "dispatch"))}
           for i in range(len(variants))]
    queues = [p.resource(scope, 1, i + 1) for i in range(2 if mode == "owner" else 1)]
    events = []

    def add(kind, **fields):
        events.append({"kind": kind, **fields})

    def host(kind, i):
        add(kind, allocation=ids[i]["allocation"], byte_offset=0, content={"state": "range_only", "byte_len": 384})

    def publish(i):
        row = ids[i]
        add("dispatch_published", dispatch=row["dispatch"], queue=queues[i if mode == "owner" else 0],
            stream=row["stream"], kernel=row["kernel"], dispatch_shape=p.content(bytes([i + 1]) * 32),
            launch={"grid": [64, 1, 1], "workgroup": [64, 1, 1], "dynamic_shared_bytes": 0},
            bindings=[{"allocation": row["allocation"], "access": "read_write", "byte_offset": 0,
                       "byte_len": 384, "kernarg_byte_offset": 0}])

    def complete(i):
        add("dispatch_completed", dispatch=ids[i]["dispatch"], host_timing=dict.fromkeys(p.TIMINGS, 0))

    for i, variant in enumerate(variants):
        row = ids[i]
        add("module_loaded", module=row["module"], artifact=p.ARTIFACTS[variant])
        add("kernel_resolved", kernel=row["kernel"], module=row["module"],
            name=p.content(("mixed_" + variant.lower()).encode()), signature=p.SIGNATURE)
        add("stream_created", stream=row["stream"])
        add("allocation_created", allocation=row["allocation"], memory_kind="host_visible", byte_len=384, alignment=4)
        host("host_write", i)
    add("native_queue_created", queue=queues[0])
    publish(0)
    if mode == "owner":
        add("native_queue_created", queue=queues[1])
        publish(1)
        complete(1)
        complete(0)
    elif mode == "backpressure":
        complete(0)
        host("host_read", 1)
        publish(1)
        complete(1)
    else:
        complete(0)
    for i in range(len(variants)):
        host("host_read", i)
    if mode == "owner":
        for row in ids[::-1]:
            add("submission_released", dispatch=row["dispatch"])
        for row in ids:
            add("allocation_released", allocation=row["allocation"])
            add("stream_destroyed", stream=row["stream"])
            add("module_unloaded", module=row["module"])
    else:
        for kind, field in (("stream_destroyed", "stream"), ("submission_released", "dispatch"),
                            ("module_unloaded", "module"), ("allocation_released", "allocation")):
            for row in ids:
                add(kind, **{field: row[field]})
    for queue in queues:
        add("native_queue_destroyed", queue=queue)
    return sealed(events, scope)


class ProfileTests(unittest.TestCase):
    def altered(self, mode, mutate):
        value = copy.deepcopy(fixture(mode))
        events = [row["event"] for row in value["events"]]
        mutate(events)
        value = sealed(events, value["capture_scope"])
        with self.assertRaises((ValueError, KeyError, TypeError)):
            p.mixed(value, mode)

    def test_complete_positive_profiles(self):
        for mode, size in (("owner", 28), ("timeout", 14), ("drop", 14), ("backpressure", 27)):
            self.assertEqual(p.mixed(fixture(mode), mode)["events"], size)
        value = fixture("owner")
        events = [row["event"] for row in value["events"]]
        for event in events:
            if event["kind"] == "dispatch_completed":
                event["host_timing"] = dict.fromkeys(p.TIMINGS, 2**64 - 1)
        p.mixed(sealed(events, value["capture_scope"]), "owner")

    def test_content_claims(self):
        self.assertEqual(p.ARTIFACTS["Long"]["digest"], "6c36d14355611d30257395eec8a8ceb3b72fc0754e6af2b9a5e06910c9da5be4")
        self.assertEqual(p.ARTIFACTS["Short"]["digest"], "dfd59e7661c9a410eda9bedb6933b9b970b6037ff43d0e216ed18897c31712bc")
        self.assertEqual(p.SIGNATURE["digest"], "4c5852a5f29f1d7701366c795c338dfd0fc9a5779ae6e2ade883cee9ef2e8fc9")

    def test_semantic_mutations_with_recomputed_identities(self):
        def change(kind, field, value):
            return lambda events: next(row for row in events if row["kind"] == kind).__setitem__(field, value)
        for mutate in (
            change("module_loaded", "artifact", p.ARTIFACTS["Short"]),
            change("kernel_resolved", "signature", p.content(b"wrong")),
            change("kernel_resolved", "name", p.content(b"mixed_short")),
            change("dispatch_completed", "dispatch", "1" * 64),
            change("submission_released", "dispatch", "1" * 64),
            change("allocation_created", "byte_len", 383),
            change("host_read", "content", {"state": "range_only", "byte_len": 383}),
            change("dispatch_published", "launch", {"grid": [32, 1, 1], "workgroup": [32, 1, 1], "dynamic_shared_bytes": 0}),
            lambda events: events.pop(), lambda events: events.append(copy.deepcopy(events[-1])),
        ):
            self.altered("owner", mutate)
        for field in ("queue", "kernel", "stream"):
            def duplicate(events, field=field):
                pubs = [row for row in events if row["kind"] == "dispatch_published"]
                pubs[1][field] = pubs[0][field]
            self.altered("owner", duplicate)
        for field, value in (("access", "read"), ("byte_len", 380), ("byte_offset", 4), ("kernarg_byte_offset", 8)):
            def binding(events, field=field, value=value):
                next(row for row in events if row["kind"] == "dispatch_published")["bindings"][0][field] = value
            self.altered("owner", binding)

    def test_reordered_lifetimes(self):
        for kind in ("host_read", "module_unloaded", "stream_destroyed", "native_queue_destroyed", "allocation_released"):
            def early(events, kind=kind):
                row = next(row for row in events if row["kind"] == kind)
                events.remove(row)
                events.insert(next(i for i, item in enumerate(events) if item["kind"] == "dispatch_completed"), row)
            self.altered("owner", early)
        def late_preread(events):
            row = next(row for row in events if row["kind"] == "host_read")
            events.remove(row)
            pubs = [i for i, item in enumerate(events) if item["kind"] == "dispatch_published"]
            events.insert(pubs[1] + 1, row)
        self.altered("backpressure", late_preread)
        def early_preread(events):
            row = next(row for row in events if row["kind"] == "host_read")
            events.remove(row)
            events.insert(next(i for i, item in enumerate(events) if item["kind"] == "dispatch_completed"), row)
        self.altered("backpressure", early_preread)

    def test_resource_and_canonical_nested_schema(self):
        for kind, field in (("host_read", "content"), ("dispatch_published", "launch")):
            def reverse(events, kind=kind, field=field):
                event = next(row for row in events if row["kind"] == kind)
                event[field] = dict(reversed(list(event[field].items())))
            self.altered("owner", reverse)
        def rename(events, replacement):
            original = next(row for row in events if row["kind"] == "stream_created")["stream"]
            for row in events:
                if row.get("stream") == original:
                    row["stream"] = replacement
        def alias(events):
            replacement = next(row for row in events if row["kind"] == "module_loaded")["module"]
            rename(events, replacement)
        self.altered("owner", alias)
        value = fixture("owner")
        events = [row["event"] for row in value["events"]]
        rename(events, "1" * 64)
        p.mixed(sealed(events, value["capture_scope"]), "owner")

    def test_schema_and_truth(self):
        for mutate in (lambda v: v["coverage"].__setitem__("dropped_events", 1),
                       lambda v: v["coverage"].__setitem__("observed_events", True),
                       lambda v: v["events"][0].__setitem__("identity", "0" * 64),
                       lambda v: v.__setitem__("schema_version", True),
                       lambda v: v["events"][0].__setitem__("origin", "unavailable"),
                       lambda v: v["device"].__setitem__("identity", "1" * 64)):
            value = copy.deepcopy(fixture("owner"))
            mutate(value)
            with self.assertRaises(ValueError):
                p.mixed(value, "owner")


if __name__ == "__main__":
    unittest.main()
