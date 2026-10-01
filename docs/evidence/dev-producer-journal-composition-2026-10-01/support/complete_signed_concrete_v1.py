#!/usr/bin/env python3
"""Explicit 109+3 completion; the original signed112 packet stays rejected."""
import hashlib
import json
from pathlib import Path
import sys
import time
import types

BASE = Path(__file__).resolve().parent
OWNER_SHA = "54c7951f454d3463c8eab232b602e9584b1928ea3a06f7a887a57b44903c6ba8"
AUDITOR_SHA = "5635ffe856c94dd01e1f97fe4c43fc52012f8313b078abdf53533d542ff2a998"
AUDIT_CONTROLS_SHA = "ebd78514beeb5fb57d48934a959d74df1fabb4ce4585e5c1c3d9fa6ee8758a3a"
REPORT_SHA = "c5b3bda8d16123440f8a960a6fd8df25222be1d9fa9f616381349515e315ee1d"
REPORT = BASE / "rejected-signed-concrete-post-audit-v1.json"
RESOURCE_SHA = "35591285a2e039284f8804116bc640ac2ac4dd6ccc019e83ba29358d1ccb46a7"
OUTPUT = Path("/run/shm/fe2o3-a2-signed-concrete-completion-20261001-attempt-1")
DURABLE = Path("/mnt/c") / OUTPUT.name
PREPARED = BASE / "prepared-signed-concrete-completion-v1.json"
WAIT_BUDGET_NS, POLL_NS = 300 * 10**9, 10**9
RAM_LINE = '    need(available_memory(Path("/proc/meminfo").read_text()) >= RAM_FLOOR, "at least 16 GiB available local RAM")\n'
MAIN_EDITS = (
    ('    rows, generated = [], {}\n', '    rows, generated = [], {}\n    admission = AdmissionBudget(m)\n'),
    ('        m.footprint()\n\n    def stage(spec):', '        prefix_unchanged(x)\n        m.footprint()\n\n    def stage(spec):'),
    ('                row["owned_launch_attempted"] = True\n',
     '                admission.wait(folder / "admission.json")\n                row["owned_launch_attempted"] = True\n'),
    ('"cases": len(x.cases)', '"retained_prefix_cases": len(x.cases)'),
    ('"signed_campaign_checks_passed": checks_pass', '"composed_completion_checks_passed": checks_pass, "original_campaign_qualified": False'),
    ('"signed_concrete_campaign_qualified": accepted, "qualified_fresh_negative_cases": 89 if accepted else 0',
     '"composed_signed_concrete_qualified": accepted, "original_campaign_qualified": False, '
     '"qualified_unique_prefix_negative_cases": 89 if accepted else 0, "fresh_closing_stages": 3 if accepted else 0'),
)
SCOPE = (
    "Explicitly composed109+3 evidence, never success of the original RAM-rejected112-stage campaign. "
    "The independently read-back immutable prefix contains89 unique actual-body logical observations and8 positives; "
    "only the original missing110 closing214 positive,111 release and112 signature are freshly executed. "
    "Composed acceptance requires exact source/tools/pre-sign/signed blobs, all prefix raw classifications/92 projections/109 closed "
    "groups, all3 new accepted closed groups and complete durable custody. No calibration capture is adopted or rerun. "
    "Eight forwarding result-equality faults and one ghost-trace-only fault retain their exact scope; no inner query-count claim. "
    "Live/credit/Arc/mutex/interior-state/compiler/ISA/hardware/performance/parity remain outside this theorem; old CPU is custody only. "
    "The16GiB RAM minimum is admission-only, immediately before every new command, with one cumulative300-second monotonic "
    "wait budget and1-second polls. Budget is checked after observing memory and before accepting recovery, including oversleep. "
    "No solver starts below the floor, no command retry, no mid-command pause or proof-bound relaxation. Ordinary source/tool/raw/"
    "disk/packet/census continuity has no post-command RAM success condition. All original4-thread120-inner/130-owned proof "
    "bounds and130/60 release/signature bounds remain exact. Admission waits have a lifetime budget; there is no hard total-owner "
    "deadline. Disk/packet checks remain boundary-only, not continuous quotas. Originals and tmpfs remain retained; new complete "
    "archive is member-verified and directory-fsynced. Publication remains a separate security-reviewed action."
)


def need(value, message):
    if not value:
        raise ValueError(message)


def sha(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def module(path, digest, name):
    need(path.resolve() == path and not path.is_symlink() and sha(path) == digest, "exact ordinary reviewed helper")
    result = types.ModuleType(name)
    result.__file__ = str(path)
    exec(compile(path.read_bytes(), str(path), "exec"), result.__dict__)
    return result


class AdmissionBudget:
    def __init__(self, resources, clock=time.monotonic_ns, sleep=time.sleep, observe=None):
        self.resources, self.clock, self.sleep = resources, clock, sleep
        self.observe = observe or (lambda: resources.available_memory(Path("/proc/meminfo").read_text()))
        self.spent_ns = 0

    def wait(self, path):
        started, before = self.clock(), self.spent_ns
        row = {"budget_ns": WAIT_BUDGET_NS, "spent_ns_before": before, "floor_bytes": self.resources.RAM_FLOOR,
               "started_monotonic_ns": started, "samples": [], "accepted": False}
        try:
            while True:
                available = self.observe()
                elapsed = self.clock() - started
                row["samples"].append({"elapsed_ns": elapsed, "available_bytes": available})
                need(type(available) is int and available >= 0 and elapsed >= 0, "typed available-memory/monotonic observation")
                need(before + elapsed < WAIT_BUDGET_NS, "cumulative RAM admission wait budget exhausted")
                if available >= self.resources.RAM_FLOOR:
                    row["accepted"] = True
                    return
                self.sleep(min(POLL_NS, WAIT_BUDGET_NS - before - elapsed) / 10**9)
        except BaseException as error:
            row["error"] = type(error).__name__ + ": " + str(error)
            raise
        finally:
            self.spent_ns = before + self.clock() - started
            row["spent_ns_after"] = self.spent_ns
            late = row["accepted"] and self.spent_ns >= WAIT_BUDGET_NS
            if late:
                row["accepted"] = False
                row["error"] = "ValueError: cumulative RAM admission wait budget exhausted at final check"
            self.resources.save(path, row)
            need(not late, "cumulative RAM admission wait budget exhausted at final check")


def disk_only(resources):
    path = Path(resources.__file__)
    need(sha(path) == RESOURCE_SHA, "exact reviewed resource helper")
    text = path.read_text()
    text = text[text.index("def footprint(startup=False):"):text.index("\n\ndef member_manifest(")]
    need(text.count(RAM_LINE) == 1, "remove only the admission-specific RAM predicate")
    exec(compile(text.replace(RAM_LINE, ""), str(path), "exec"), resources.__dict__)


def prefix_unchanged(x):
    need(sha(REPORT) == REPORT_SHA and all(sha(Path(path)) == digest for path, digest in x.audit.TERMINAL_PINS.items()),
         "exact independently reviewed prefix and terminal custody")
    need(x.resources.member_manifest(x.audit.PACKET) == x.prefix_members, "complete unchanged original prefix files and modes")


def closing_plan(original):
    rows = original["commands"][109:]
    need(len(original["commands"]) == 112 and [row["name"] for row in rows] ==
         ["110-concrete-after", "111-tool-release-after", "112-signature-after"]
         and [row["kind"] for row in rows] == ["positive", "release", "signature"]
         and [row["timeout"] for row in rows] == [130, 130, 60], "only exact three missing prepared commands")
    return rows


def composed_qualified(base, original, prefix, commands, rows, errors, closed):
    return (commands == closing_plan(original) and len(rows) == 3 and prefix["signed_campaign_checks_passed"] is False
            and base.qualified(original["commands"], prefix["stages"] + rows, errors, closed))


def build():
    path = BASE / "signed_concrete_campaign_v1.py"
    base = module(path, OWNER_SHA, "completion_original_owner")
    w = module(path, OWNER_SHA, "completion_adapted_owner")
    audit = module(BASE / "audit_rejected_signed_concrete_v1.py", AUDITOR_SHA, "completion_rejected_prefix")
    w.OUTPUT, w.DURABLE, w.PREPARED, w.SCOPE = OUTPUT, DURABLE, PREPARED, SCOPE
    w.AdmissionBudget, w.prefix_unchanged = AdmissionBudget, prefix_unchanged

    def components():
        x = base.components()
        x.original = json.loads(json.dumps(base.prepare(x)))
        need(sha(base.PREPARED) == audit.PREPARED_SHA and json.loads(base.PREPARED.read_bytes()) == x.original,
             "complete original signed source/tool/command preparation unchanged")
        need(sha(BASE / "test_audit_rejected_signed_concrete_v1.py") == AUDIT_CONTROLS_SHA and sha(REPORT) == REPORT_SHA,
             "reviewed seven rejection controls and root-matched independent report")
        report = json.loads(REPORT.read_bytes())
        need(report["rejected_signed_concrete_readback_passed"] is True and report["signed_campaign_qualified"] is False
             and report["qualified_fresh_negative_cases"] == 0 and report["saved_original_groups"] == 109
             and report["observed_fresh_negative_cases"] == base.ROSTER
             and report["positive_counts"] == {"leaf": [42] * 3, "conditional": [64] * 3, "concrete": [214] * 2}
             and report["signed_commit"] == base.SIGNED and report["prepared_sha256"] == audit.PREPARED_SHA,
             "exact109-prefix scope, never already qualified")
        x.audit, x.prefix_members = audit, json.loads((audit.DURABLE / "members.json").read_bytes())
        prefix_unchanged(x)
        x.prefix = json.loads((audit.PACKET / "results.json").read_bytes())
        audit.rejection(x.prefix)
        audit.roster(x.original["commands"], x.prefix["stages"])
        need(not (audit.PACKET / "closing-inputs.json").exists()
             and all(not (audit.PACKET / row["name"]).exists() for row in closing_plan(x.original)),
             "original closing stages and source record remain absent")
        x.pins.update({str(base.PREPARED): audit.PREPARED_SHA, str(REPORT): REPORT_SHA,
            str(BASE / "audit_rejected_signed_concrete_v1.py"): AUDITOR_SHA,
            str(BASE / "test_audit_rejected_signed_concrete_v1.py"): AUDIT_CONTROLS_SHA,
            str(BASE / "complete_signed_concrete_v1.py"): sha(BASE / "complete_signed_concrete_v1.py"),
            str(BASE / "test_complete_signed_concrete_v1.py"): sha(BASE / "test_complete_signed_concrete_v1.py")})
        x.pins.update({path: digest for path, digest in audit.TERMINAL_PINS.items() if not path.endswith("packet.tar")})
        x.resources.OUTPUT, x.resources.DURABLE = OUTPUT, DURABLE
        disk_only(x.resources)
        w.qualified = lambda commands, rows, errors, closed: composed_qualified(base, x.original, x.prefix, commands, rows, errors, closed)
        return x

    def prepare(x):
        frozen = dict(x.original, commands=closing_plan(x.original), projections={}, raw_pins=x.pins,
                      scope=SCOPE, durable_archive=str(DURABLE))
        frozen["prefix"] = {"path": str(audit.PACKET), "durable_path": str(audit.DURABLE),
            "report_sha256": REPORT_SHA, "terminal_pins": audit.TERMINAL_PINS, "members": x.prefix_members,
            "original_campaign_qualified": False, "prior_stages": 109, "unique_observed_negatives": 89,
            "prior_positives": 8, "fresh_closing_stages": 3, "original_prepared_sha256": audit.PREPARED_SHA}
        frozen["storage"] = dict(x.original["storage"], projection_bytes=0,
            raw_archive_bytes_before_prepared=sum(Path(path).stat().st_size for path in x.pins),
            startup_and_stage_mem_available_bytes=None, command_admission_mem_available_bytes=base.RAM_FLOOR)
        frozen["bounds"] = dict(x.original["bounds"], command_timeout_sum_seconds=320,
            cumulative_admission_wait_ns=WAIT_BUDGET_NS, admission_poll_ns=POLL_NS, post_command_ram_success_condition=False)
        need(frozen["storage"]["raw_archive_bytes_before_prepared"] < base.LIMIT // 2, "unchanged bounded nonrecursive raw archive")
        return frozen

    w.components, w.prepare = components, prepare
    text = path.read_text()
    text = text[text.index("def main():"):text.index('\n\nif __name__ == "__main__":')]
    for before, after in MAIN_EDITS:
        need(text.count(before) == 1 and after not in text, "one exact reviewed main adaptation")
        text = text.replace(before, after)
    exec(compile(text, str(path), "exec"), w.__dict__)
    return w


if __name__ == "__main__":
    owner = build()
    if "--prepare" in sys.argv:
        need(not PREPARED.exists(), "fresh completion preparation")
    owner.main()
