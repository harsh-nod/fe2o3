#!/usr/bin/env python3
"""Controls require the same explicit pinned archive; no downloads."""
import argparse
import copy
import importlib.util
import io
import json
import os
from pathlib import Path
import tempfile
import unittest
import warnings
import zipfile

SPEC = importlib.util.spec_from_file_location("catalog", Path(__file__).with_name("amd-isa-catalog-v1.py"))
catalog = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(catalog)
ARCHIVE = None


def stripped(node):
    return [node.tag, dict(sorted(node.attrib.items())), (node.text or "").strip(),
            [stripped(child) for child in node if child.tag != "Description"]]


def decode(node, strings):
    tag, attrs, text, children = node
    return [strings[tag], {strings[attrs[i]]: strings[attrs[i + 1]]
                          for i in range(0, len(attrs), 2)}, strings[text],
            [decode(child, strings) for child in children]]


class CatalogControls(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.manifest = catalog.read_json(catalog.DATA / "manifest.json")
        cls.raw = catalog.read_regular(ARCHIVE, 3 * 1024 * 1024)
        cls.members = catalog.checked_archive(cls.raw, cls.manifest)
        cls.root = catalog.parse_xml(cls.members["cdna3"])
        cls.pin = cls.manifest["members"][0]

    def mutated(self, action, match):
        root = copy.deepcopy(self.root)
        action(root)
        with self.assertRaisesRegex(ValueError, match):
            catalog.validate(root, self.pin, self.manifest)

    def test_actual_pinned_members_counts_and_all_references(self):
        for pin in self.manifest["members"]:
            raw = self.members[pin["architecture"]]
            self.assertEqual(catalog.sha(raw), pin["sha256"])
            sections, _, _, missing = catalog.validate(catalog.parse_xml(raw), pin, self.manifest)
            self.assertEqual(len(sections["instructions"]), pin["instructions"])
            self.assertEqual(len(missing), pin["missing_default_condition_references"])
            self.assertTrue(all(not row["condition_definition_available"] for row in missing))

    def test_archive_truncation_same_size_mutation_and_member_pin(self):
        for raw in [self.raw[:-1], bytes([self.raw[0] ^ 1]) + self.raw[1:]]:
            with self.assertRaisesRegex(ValueError, "archive size/hash"):
                catalog.checked_archive(raw, self.manifest)
        for key, value in [("sha256", "0" * 64), ("bytes", catalog.MAX_XML + 1)]:
            manifest = copy.deepcopy(self.manifest)
            manifest["members"][0][key] = value
            with self.assertRaisesRegex(ValueError, "XML member"):
                catalog.checked_archive(self.raw, manifest)

    def test_duplicate_zip_member_and_excess_count_are_rejected(self):
        names = ["amdgpu_isa_cdna" + str(i) + ".xml" for i in range(1, 6)]
        names += ["amdgpu_isa_rdna" + x + ".xml" for x in ["1", "2", "3", "3_5", "4"]]
        for malformed in [names + ["extra.xml"], names[:-1] + [names[0]]]:
            buffer = io.BytesIO()
            with warnings.catch_warnings():
                warnings.simplefilter("ignore", UserWarning)
                with zipfile.ZipFile(buffer, "w", compression=zipfile.ZIP_DEFLATED) as archive:
                    for name in malformed:
                        archive.writestr(name, b"<Spec/>")
            raw = buffer.getvalue()
            manifest = copy.deepcopy(self.manifest)
            manifest["archive"].update(bytes=len(raw), sha256=catalog.sha(raw))
            with self.assertRaisesRegex(ValueError, "ZIP member count|duplicate ZIP member"):
                catalog.checked_archive(raw, manifest)

    def test_regular_read_rejects_fifo_symlink_directory_and_size(self):
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            regular = base / "input"
            regular.write_bytes(b"ab")
            (base / "link").symlink_to(regular)
            os.mkfifo(base / "fifo")
            for path in [base / "link", base / "fifo", base]:
                with self.assertRaises((ValueError, OSError)):
                    catalog.read_regular(path, 20)
            with self.assertRaisesRegex(ValueError, "size"):
                catalog.read_regular(regular, 1)
            self.assertEqual(catalog.read_regular(regular, 2), b"ab")

    def test_xml_entities_utf8_depth_size_and_node_bounds(self):
        cases = [
            b'<!DOCTYPE Spec [<!ENTITY x "not evaluated">]><Spec>&x;</Spec>',
            b"<Spec>\xff</Spec>",
            b"<x>" * 50 + b"</x>" * 50,
            b"x" * (catalog.MAX_XML + 1),
        ]
        for raw in cases:
            with self.assertRaises((ValueError, UnicodeError)):
                catalog.parse_xml(raw)
        previous = catalog.MAX_NODES
        try:
            catalog.MAX_NODES = 2
            with self.assertRaisesRegex(ValueError, "structural"):
                catalog.parse_xml(b"<x><y/><z/></x>")
        finally:
            catalog.MAX_NODES = previous

    def test_duplicate_instruction_and_alias_collision(self):
        self.mutated(lambda r: r.find("ISA/Instructions").append(
            copy.deepcopy(r.find("ISA/Instructions/Instruction"))), "duplicate")
        def alias_collision(root):
            node = root.find("ISA/Instructions/Instruction/AliasedInstructionNames/InstructionName")
            node.text = root.findtext("ISA/Instructions/Instruction/InstructionName")
        self.mutated(alias_collision, "alias")

    def test_reference_mutations_are_not_rebound(self):
        paths = [
            ("InstructionEncodings/InstructionEncoding/EncodingName", "unknown encoding"),
            ("InstructionEncodings/InstructionEncoding/EncodingCondition", "unknown encoding condition"),
            ("InstructionEncodings/InstructionEncoding/Operands/Operand/OperandType", "unknown operand type"),
            ("InstructionEncodings/InstructionEncoding/Operands/Operand/DataFormatName", "unknown data format"),
            ("InstructionEncodings/InstructionEncoding/Operands/Operand/FieldName", "unknown operand field"),
        ]
        for path, message in paths:
            with self.subTest(path=path):
                self.mutated(lambda r: setattr(r.find("ISA/Instructions/Instruction/" + path),
                                              "text", "UNKNOWN_REFERENCE"), message)
        self.mutated(lambda r: setattr(r.find("ISA/OperandTypes/OperandType/Subtypes/Subtype"),
                                      "text", "UNKNOWN_TYPE"), "unknown operand subtype")

    def test_reviewed_missing_count_is_not_extensible(self):
        manifest = copy.deepcopy(self.manifest)
        manifest["members"][0]["missing_default_condition_references"] -= 1
        with self.assertRaisesRegex(ValueError, "missing-condition count"):
            catalog.validate(self.root, manifest["members"][0], manifest)
        def mutate(root):
            for instruction in root.findall("ISA/Instructions/Instruction"):
                for alternative in instruction.findall("InstructionEncodings/InstructionEncoding"):
                    if alternative.findtext("EncodingName") == "ENC_FLAT_GLBL":
                        alternative.find("EncodingCondition").text = "unknown_default"
                        return
        self.mutated(mutate, "unknown encoding condition")

    def test_duplicate_condition_exception_requires_exact_count_and_tree(self):
        def conditions(root):
            return next(n for n in root.findall("ISA/Encodings/Encoding")
                        if n.findtext("EncodingName") == "ENC_FLAT").find("EncodingConditions")
        self.mutated(lambda r: conditions(r).remove(conditions(r)[-1]), "duplicate condition")
        self.mutated(lambda r: conditions(r)[1].find("CondtionExpression/lit").set("val", "2"),
                     "duplicate condition differs")
        def all_changed(root):
            for node in conditions(root):
                node.find("CondtionExpression/lit").set("val", "2")
        self.mutated(all_changed, "expression changed")
        def other_duplicate(root):
            parent = root.find("ISA/Encodings/Encoding/EncodingConditions")
            parent.append(copy.deepcopy(parent[0]))
        self.mutated(other_duplicate, "unreviewed duplicate")

    def test_fields_flags_and_widths_are_bounded(self):
        self.mutated(lambda r: setattr(r.find("ISA/Encodings/Encoding/MicrocodeFormat/"
                                            "BitMap/Field/BitLayout/Range/BitOffset"),
                                      "text", "9999"), "number range|outside encoding")
        self.mutated(lambda r: setattr(r.find("ISA/Instructions/Instruction/"
                                            "InstructionFlags/IsBranch"), "text", "YES"),
                     "invalid boolean")
        self.mutated(lambda r: setattr(r.find("ISA/Instructions/Instruction/"
                                            "InstructionEncodings/InstructionEncoding/"
                                            "Operands/Operand/OperandSize"), "text", "65536"),
                     "number range")

    def test_lossless_non_prose_tree_and_ranges_with_anomalies(self):
        for pin in self.manifest["members"]:
            root = catalog.parse_xml(self.members[pin["architecture"]])
            sections, _, _, missing = catalog.validate(root, pin, self.manifest)
            raw, strings, ranges = catalog.metadata(root, pin, self.manifest, sections, missing)
            document = json.loads(raw)
            self.assertEqual(strings, sorted(set(strings)))
            for kind, entries in sections.items():
                self.assertEqual(len(document["sections"][kind]), len(entries))
                for name, element in entries.items():
                    start, end = ranges[(kind, name)]
                    self.assertEqual(decode(json.loads(raw[start:end]), strings), stripped(element))
            self.assertEqual(document["duplicate_condition_declarations"],
                             [self.manifest["reviewed_duplicate_condition"]])
            self.assertEqual(len(document["unresolved_conditions"]),
                             pin["missing_default_condition_references"])

    def test_independent_vop2_numbers_fields_registers_and_words(self):
        instructions = {n.findtext("InstructionName"): n for n in
                        self.root.findall("ISA/Instructions/Instruction")}
        encoding = next(n for n in self.root.findall("ISA/Encodings/Encoding")
                        if n.findtext("EncodingName") == "ENC_VOP2")
        fields = {n.findtext("FieldName"): n for n in encoding.findall("MicrocodeFormat/BitMap/Field")}
        expected_fields = {"OP": (25, 6), "VDST": (17, 8), "VSRC1": (9, 8), "SRC0": (0, 9)}
        for name, expected in expected_fields.items():
            part = fields[name].find("BitLayout/Range")
            self.assertEqual((int(part.findtext("BitOffset")), int(part.findtext("BitCount"))), expected)
        for name, opcode, dst, src0, src1, word in [
            ("V_XOR_B32", 21, 32, 34, 35, 0x2A404722),
            ("V_ADD_U32", 52, 33, 32, 36, 0x68424920),
        ]:
            alternative = next(n for n in instructions[name].findall(
                "InstructionEncodings/InstructionEncoding")
                if n.findtext("EncodingName") == "ENC_VOP2")
            self.assertEqual(int(alternative.findtext("Opcode")), opcode)
            self.assertEqual((opcode << 25) | (dst << 17) | (src1 << 9) | (256 + src0), word)
            self.assertEqual((word >> 25) & 63, opcode)
        operand = next(n for n in self.root.findall("ISA/OperandTypes/OperandType")
                       if n.findtext("OperandTypeName") == "OPR_SRC")
        values = {n.findtext("Name"): n.findtext("Value")
                  for n in operand.findall("OperandPredefinedValues/PredefinedValue")}
        self.assertEqual(int(values["v34"]), 290)

    def test_generation_is_exactly_deterministic_and_checked_in(self):
        first = catalog.generate(ARCHIVE)
        second = catalog.generate(ARCHIVE)
        self.assertEqual(first, second)
        catalog.publish(first, check=True)
        self.assertTrue(all(len(raw) <= catalog.MAX_OUTPUT for raw in first.values()))
        self.assertTrue(all(len(raw.splitlines()) <= 1200 for path, raw in first.items()
                            if path.suffix == ".rs"))

    def test_overlay_cannot_promote_memory_or_encoding_authority(self):
        original = catalog.read_json(catalog.DATA / "reviewed-coverage.json")
        catalog.validate_coverage(original)
        changed = copy.deepcopy(original)
        changed["instruction_names"][0] = "DS_ADD_U32"
        changed["instruction_names"].sort()
        with self.assertRaisesRegex(ValueError, "exact reviewed six"):
            catalog.validate_coverage(changed)
        for key in ["encoding_selection", "encoding_level_authoring",
                    "encoding_level_simulation", "encoding_level_proof", "hardware_qualification"]:
            changed = copy.deepcopy(original)
            changed[key] = "available"
            with self.assertRaisesRegex(ValueError, "must not grant"):
                catalog.validate_coverage(changed)

    def test_check_refuses_stale_outputs_and_write_refuses_symlink(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "output"
            path.write_bytes(b"original")
            with self.assertRaisesRegex(ValueError, "generated output mismatch"):
                catalog.publish({path: b"changed"}, check=True)
            self.assertEqual(path.read_bytes(), b"original")
            link = Path(directory) / "link"
            link.symlink_to(path)
            with self.assertRaises(OSError):
                catalog.publish({link: b"changed"}, check=False)
            self.assertEqual(path.read_bytes(), b"original")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archive", required=True, type=Path)
    args, rest = parser.parse_known_args()
    ARCHIVE = args.archive
    unittest.main(argv=[__file__, *rest])
