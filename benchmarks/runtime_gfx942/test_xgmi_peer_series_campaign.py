#!/usr/bin/env python3
"""CPU-only synthetic campaign input/receipt closure. Never launches a process."""

import copy
import hashlib
import unittest

import xgmi_peer_series_campaign as campaign
from test_xgmi_peer_series_results import encode, record


def digest(label):
    return hashlib.sha256(label.encode("ascii")).hexdigest()


def environment():
    return {
        "schema": "fe2o3.xgmi-peer-reviewed-environment-observation.v1",
        "identity": copy.deepcopy(campaign._IDENTITY),
        "source_sha256": copy.deepcopy(campaign._SOURCES),
        "evidence_sha256": {key: digest(key) for key in
                            ("kernel", "loaded_module", "installed_module", "package_and_source")},
        "environment_assumption": "reviewed-mi300x-amdgpu61613-ordinary-lifetime",
        "excluded_changes": ["administrative-repartition", "hive-reconfiguration", "hotplug",
                             "privileged-criu", "foreign-same-process-kfd-drm-mutation"],
    }


def inputs():
    endpoints = [
        {"physical_index": 1, "unique_id": "b7baafd0fb173d8e", "pci_bdf": "0000:26:00.0",
         "kfd_gpu_id": 101, "target": "gfx942:xnack-"},
        {"physical_index": 2, "unique_id": "10a254ce4987e716", "pci_bdf": "0000:46:00.0",
         "kfd_gpu_id": 202, "target": "gfx942:xnack-"},
    ]
    admission = {
        "schema": "fe2o3.xgmi-peer-series-admission-input.v1", "endpoints": endpoints,
        "routes": [
            {"source_gpu_id": 101, "destination_gpu_id": 202, "engine_id": 2,
             "engine_control": "kfd-topology-admitted-directional-id"},
            {"source_gpu_id": 202, "destination_gpu_id": 101, "engine_id": 3,
             "engine_control": "kfd-topology-admitted-directional-id"},
        ],
        "visibility": {backend: [{**endpoint, "visible_index": index}
                                 for index, endpoint in enumerate(endpoints)]
                       for backend in ("hip", "hsa")},
        "evidence_sha256": {key: digest(key) for key in
                            ("physical_inventory", "kfd_topology", "hip_visibility", "hsa_visibility")},
    }
    return {"physical_indices": [1, 2], "unique_ids": [endpoint["unique_id"] for endpoint in endpoints],
            "admission": admission, "environment_before": environment()}


def receipts(args=None):
    args = inputs() if args is None else args
    plan = campaign.trial_specs(**args)
    result = []
    for spec in plan["trials"]:
        fields = record(spec["backend"], spec["depth"])
        fields.update(warmups=str(plan["warmups"]), samples=str(plan["samples"]),
                      forward_samples=str(plan["samples"]), reverse_samples=str(plan["samples"]))
        result.append({**copy.deepcopy(spec), "returncode": 0, "stdout": encode(fields),
                       "stderr": b"", "execution_receipt_sha256": digest(spec["name"])})
    return result


def replay(rows, args=None, after=None):
    return campaign.replay_campaign(rows, environment_after=environment() if after is None else after,
                                    **(inputs() if args is None else args))


class SeriesCampaignTests(unittest.TestCase):
    def test_exact_eighteen_trial_order_and_real_visibility_controls(self):
        plan = campaign.trial_specs(**inputs())
        self.assertEqual(len(plan["trials"]), 18)
        self.assertEqual([spec["backend"] for spec in plan["trials"]],
                         ["kfd", "hsa", "hip", "hip", "hsa", "kfd"] * 3)
        self.assertEqual([spec["depth"] for spec in plan["trials"]], [1] * 6 + [16] * 6 + [32] * 6)
        for spec in plan["trials"]:
            backend = spec["backend"]
            shape = ["1048576", str(spec["depth"]), "2", "10"]
            uids = ["0xb7baafd0fb173d8e", "0x10a254ce4987e716"]
            self.assertEqual(spec["binary"], backend + "-series")
            self.assertIn("LD_PRELOAD", spec["clear_environment"])
            self.assertIn("HSA_ENABLE_SDMA", spec["clear_environment"])
            if backend == "kfd":
                self.assertEqual(spec["arguments"], [*uids, *shape, "--retained-pair-series-reviewed-mi300x"])
                self.assertEqual(spec["environment_overrides"], {"HSA_XNACK": "0"})
            else:
                self.assertEqual(spec["arguments"], ["0", "1", *shape, *uids, "--persistent-series"])
                self.assertEqual(spec["environment_overrides"],
                                 {"HSA_XNACK": "0", "HIP_VISIBLE_DEVICES" if backend == "hip"
                                  else "ROCR_VISIBLE_DEVICES": "1,2"})
        self.assertEqual(plan["engine_matching"], "not-claimed-hip-hsa-runtime-selected-unknown")
        self.assertEqual(plan["timing_scope"], "native-api-not-runtime-facade")
        self.assertEqual(plan["payload_staging"], "outside-samples-persistent-pattern-final-readback-only")
        self.assertEqual(plan["kfd_scope_entry_finish"], "outside-samples")
        self.assertEqual(plan["kfd_operational_fences"], "inside-samples")

    def test_complete_replay_preserves_separate_profiles_and_scope_metrics(self):
        result = replay(receipts())
        self.assertEqual(result["claim"], "input-and-receipt-consistency-only")
        self.assertEqual(len(result["trials"]), 18)
        self.assertEqual(len({row["execution_receipt_sha256"] for row in result["trials"]}), 18)
        for row in result["trials"]:
            fields = row["fields"]
            self.assertEqual(fields["forward_samples"], "10")
            self.assertEqual(fields["reverse_samples"], "10")
            if fields["backend"] == "kfd":
                self.assertEqual(fields["forward_engine"], "2")
                self.assertEqual(fields["reverse_engine"], "3")
                self.assertEqual(fields["forward_scope_entry_ns"], "1000000")
            else:
                self.assertNotIn("forward_engine", fields)
                self.assertEqual(fields["engine_parallelism"], "runtime-selected-unknown")

    def test_uid_physical_index_bdf_gpu_and_visible_index_mismatches(self):
        mutations = [
            lambda args: args["physical_indices"].reverse(),
            lambda args: args["unique_ids"].reverse(),
            lambda args: args["admission"]["endpoints"][0].update(physical_index=True),
            lambda args: args["admission"]["endpoints"][0].update(unique_id="0000000000000000"),
            lambda args: args["admission"]["endpoints"][0].update(pci_bdf="0000:46:00.0"),
            lambda args: args["admission"]["endpoints"][0].update(pci_bdf="0000:26:20.0"),
            lambda args: args["admission"]["endpoints"][0].update(kfd_gpu_id=202),
        ]
        for backend in ("hip", "hsa"):
            for field, value in (("visible_index", 1), ("physical_index", True),
                                 ("unique_id", "10a254ce4987e716"), ("kfd_gpu_id", 202),
                                 ("pci_bdf", "0000:46:00.0")):
                args = inputs()
                args["admission"]["visibility"][backend][0][field] = value
                with self.subTest(backend=backend, field=field), self.assertRaises(ValueError):
                    campaign.trial_specs(**args)
        for change in mutations:
            args = inputs()
            change(args)
            with self.assertRaises(ValueError):
                campaign.trial_specs(**args)

    def test_directional_engine_identity_is_not_a_comparator_engine_claim(self):
        for direction in (0, 1):
            for key, value in (("source_gpu_id", 777), ("destination_gpu_id", 888),
                               ("engine_id", True), ("engine_id", -1),
                               ("engine_control", "runtime-selected-unknown")):
                args = inputs()
                args["admission"]["routes"][direction][key] = value
                with self.assertRaises(ValueError):
                    campaign.trial_specs(**args)
        args = inputs()
        args["admission"]["routes"][0]["engine_id"] = 4
        with self.assertRaisesRegex(ValueError, "forward_engine"):
            replay(receipts(), args)

    def test_environment_identity_and_each_selected_source_are_pinned(self):
        self.assertEqual(len(campaign._SOURCES), 22)
        self.assertEqual(campaign._IDENTITY["loaded_build_id"],
                         "4cd22e1f91450b8d9da1fc7bbbc02ee412e202d9")
        self.assertEqual(campaign._SOURCES["amd/amdkfd/kfd_process.c"],
                         "d76db8cbb546aa23dffb33b1d04244037e12246b49b752303194c68dd685e409")
        for section in ("identity", "source_sha256"):
            for key in environment()[section]:
                for missing in (False, True):
                    wrong = environment()
                    if missing:
                        del wrong[section][key]
                    else:
                        wrong[section][key] = "different"
                    with self.subTest(section=section, key=key), self.assertRaises(ValueError):
                        campaign.validate_environment(wrong)
        for key, value in (("environment_assumption", "auto-detected-gfx942"),
                           ("excluded_changes", []), ("schema", "attested")):
            wrong = environment()
            wrong[key] = value
            with self.assertRaises(ValueError):
                campaign.validate_environment(wrong)

    def test_preflight_postflight_and_evidence_rosters_cannot_be_omitted(self):
        for key in environment():
            wrong = environment()
            del wrong[key]
            with self.assertRaises(ValueError):
                replay(receipts(), after=wrong)
        for place in ("environment_before", "admission"):
            for key in inputs()[place]["evidence_sha256"]:
                for value in (None, "", "A" * 64, "0" * 63):
                    args = inputs()
                    args[place]["evidence_sha256"][key] = value
                    with self.assertRaises(ValueError):
                        campaign.trial_specs(**args)
                args = inputs()
                del args[place]["evidence_sha256"][key]
                with self.assertRaises(ValueError):
                    campaign.trial_specs(**args)

    def test_omitted_duplicated_reordered_and_replayed_trials_are_rejected(self):
        valid = receipts()
        cases = [valid[:-1], valid + [valid[-1]], [valid[1], valid[0], *valid[2:]],
                 [valid[0], valid[0], *valid[2:]]]
        reused = copy.deepcopy(valid)
        reused[3]["execution_receipt_sha256"] = reused[2]["execution_receipt_sha256"]
        cases.append(reused)
        for rows in cases:
            with self.assertRaises(ValueError):
                replay(rows)

    def test_exact_commands_environment_and_success_are_required(self):
        for key, value in (("binary", "old-kfd"), ("arguments", []),
                           ("clear_environment", []), ("environment_overrides", {"HSA_XNACK": "1"}),
                           ("depth", True), ("returncode", 1), ("returncode", False),
                           ("stderr", b"warning\n"), ("stdout", "not bytes"),
                           ("execution_receipt_sha256", "missing")):
            rows = receipts()
            rows[0][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                replay(rows)
        for key in receipts()[0]:
            rows = receipts()
            del rows[0][key]
            with self.assertRaises(ValueError):
                replay(rows)

    def test_profile_schema_engine_and_directional_counts_cannot_drift(self):
        for key, value in (("qualification_profile", "old-full-fresh"),
                           ("qualification_policy_sha256", "0" * 64),
                           ("schema", "fe2o3.xgmi-peer-aggregate-hot-only-benchmark.v1"),
                           ("forward_engine", "3"), ("reverse_engine", "2"),
                           ("forward_samples", "9"), ("reverse_samples", "11"),
                           ("environment_assumption", "unreviewed")):
            rows = receipts()
            fields = record("kfd", 1)
            fields.update(warmups="2", samples="10", forward_samples="10", reverse_samples="10")
            fields[key] = value
            rows[0]["stdout"] = encode(fields)
            with self.subTest(key=key), self.assertRaises(ValueError):
                replay(rows)

    def test_final_canary_cleanup_and_timing_population_must_agree(self):
        for position in range(3):
            for key, value in (("canaries", "fail"), ("teardown", "implicit"),
                               ("validation", "every-sample"), ("lifetime_setup", "inside-samples"),
                               ("lifetime_finish", "inside-samples"), ("surface", "runtime-facade"),
                               ("forward_samples", "0"), ("reverse_samples", "9")):
                rows = receipts()
                fields = record(rows[position]["backend"], 1)
                fields.update(warmups="2", samples="10", forward_samples="10", reverse_samples="10")
                fields[key] = value
                rows[position]["stdout"] = encode(fields)
                with self.subTest(position=position, key=key), self.assertRaises(ValueError):
                    replay(rows)

    def test_noncanonical_and_overflowing_controls_are_refused(self):
        for key, values in {
            "physical_indices": ([1, 1], [True, 2], [-1, 2], [1]),
            "unique_ids": (["0xb7baafd0fb173d8e", "10a254ce4987e716"],
                           ["0000000000000000", "10a254ce4987e716"]),
            "copy_bytes": (0, True, 0x003F_FFE1), "warmups": (-1, True, 1 << 64),
            "samples": (0, "10", 1 << 64),
        }.items():
            for value in values:
                args = inputs()
                args[key] = value
                with self.subTest(key=key, value=value), self.assertRaises(ValueError):
                    campaign.trial_specs(**args)
        with self.assertRaises(ValueError):
            campaign.trial_specs(**inputs(), warmups=(1 << 64) - 10)


if __name__ == "__main__":
    unittest.main()
