import importlib.util
from pathlib import Path
import unittest

SPEC = importlib.util.spec_from_file_location("audit_oem_il", Path(__file__).with_name("audit-oem-il.py"))
AUDIT = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(AUDIT)


def module(fields, methods):
    body = "\n".join("    .field private int32 " + field for field in fields)
    for name, instructions in methods:
        body += f"\n    .method public void test() cil managed\n    {{\n{instructions}\n    }} // end of method {name}\n"
    return AUDIT.Module("// IL code: Test\n.class public Test\n{\n" + body + "\n} // end of class Test\n", "test")


def inputs():
    fnkey = module(["SupportTpColor", "IsDeepS5Enabled"], [
        ("Features::GetDeepS5Status", "IL_0000: stfld bool FnKey.Features::IsDeepS5Enabled"),
        ("Form1::WndProc", "IL_0000: ldfld bool FnKey.Features::IsDeepS5Enabled"),
    ])
    cpuoc = module(["Memory_XMP_Profile", "Memory_XMP_Profile_From_WMI18", "Memory_Current_Voltage_From_WMI18"], [
        ("OverClockModel::Read_package_WMI18", "IL_0000: stfld object CPUOC.Model::Memory_XMP_Profile_From_WMI18\n"
         "IL_0001: ldfld object CPUOC.Model::Memory_XMP_Profile_From_WMI18\n"
         "IL_0002: stfld uint16 CPUOC.Model::Memory_Current_Voltage_From_WMI18"),
    ])
    codes = []
    for index, value in enumerate([240, 241, 242, 243, 246]):
        codes.extend([f"IL_{index * 2:04x}: ldc.i4 {value}", f"IL_{index * 2 + 1:04x}: stloc.1"])
    kbled = module([], [("RGBKB::SetColor", "\n".join(codes)),
        ("RGBKB::set_TP", "IL_0000: call void CC.InsydeAcpiProt::WriteAcpiTPColor()"),
        ("InsydeAcpiProt::WriteAcpiTPColor", "IL_0000: ret"),
        ("RGBKB::FirstRun", "IL_0000: call void CC.RGBKB::set_TP()"),
        ("RGBKB::ReadAllReg", "IL_0000: call void CC.RGBKB::set_TP()"),
        ("RGBKB::WriteAllReg", "IL_0000: call void CC.InsydeAcpiProt::WriteAcpiTPColor()"),
        *[(name, "IL_0000: call void CC.RGBKB::SetColor()")
          for name in ["RGBKB::SetCustomColor", "RGBKB::SetBreathMode", "RGBKB::SetAllColor"]],
    ])
    return fnkey, cpuoc, kbled


class EvidenceTests(unittest.TestCase):
    def test_current_boundaries_distinguish_metadata_decode_and_runtime_state(self):
        report = AUDIT.assess(*inputs())
        self.assertEqual(report["publicXmp"]["references"], [])
        self.assertEqual(report["keyboardColorSelectors"], [240, 241, 242, 243, 246])
        self.assertEqual({ref["method"] for ref in report["decodedXmp"]["references"]}, {"OverClockModel::Read_package_WMI18"})

    def test_empty_truncated_and_unbalanced_il_never_proves_absence(self):
        for text in ["", "// IL code: Test\n", "// IL code: Test\n.class X\n{",
                     "// IL code: Test\n.class X\n{\n.method x\n{\n} // end of class X"]:
            with self.assertRaises(ValueError):
                AUDIT.Module(text, "broken")

    def test_metadata_suffix_and_strings_do_not_become_field_reads(self):
        data = module(["Target", "TargetExtra"], [("Other::Read", "IL_0000: ldfld int32 X::TargetExtra\nIL_0001: ldstr \"X::Target\"")])
        field = data.field("Target")
        self.assertEqual(field["references"], [])
        self.assertEqual(len(field["stringReferences"]), 1)
        with self.assertRaises(ValueError):
            data.field("Missing")

    def test_new_tp_write_or_reflection_name_requires_new_review(self):
        for opcode, operand in [("stfld", "bool X::SupportTpColor"), ("ldstr", '"SupportTpColor"')]:
            fnkey, cpuoc, kbled = inputs()
            fnkey.methods.append({"name": "Other::Enable", "instructions": [{"line": 1, "offset": "IL_0000", "opcode": opcode, "operand": operand}]})
            with self.assertRaises(ValueError):
                AUDIT.assess(fnkey, cpuoc, kbled)

    def test_new_xmp_consumer_is_not_misclassified_as_decoder(self):
        fnkey, cpuoc, kbled = inputs()
        cpuoc.methods.append({"name": "Page::ApplyXmp", "instructions": [
            {"line": 1, "offset": "IL_0000", "opcode": "ldfld", "operand": "object X::Memory_XMP_Profile_From_WMI18"}]})
        with self.assertRaises(ValueError):
            AUDIT.assess(fnkey, cpuoc, kbled)

    def test_new_deep_s5_producer_or_reader_invalidates_current_scope(self):
        for opcode in ["stfld", "ldfld"]:
            fnkey, cpuoc, kbled = inputs()
            fnkey.methods.append({"name": "Page::ChangeS5", "instructions": [
                {"line": 1, "offset": "IL_0000", "opcode": opcode, "operand": "bool X::IsDeepS5Enabled"}]})
            with self.assertRaises(ValueError):
                AUDIT.assess(fnkey, cpuoc, kbled)

    def test_additional_color_selector_requires_review_instead_of_silent_tp_assumption(self):
        fnkey, cpuoc, kbled = inputs()
        kbled.methods[0]["instructions"].extend([
            {"line": 1, "offset": "IL_0100", "opcode": "ldc.i4", "operand": "244"},
            {"line": 2, "offset": "IL_0101", "opcode": "stloc.1", "operand": ""}])
        with self.assertRaises(ValueError):
            AUDIT.assess(fnkey, cpuoc, kbled)

    def test_new_tp_ui_sender_or_method_delegate_requires_review(self):
        for target in ["RGBKB::set_TP", "InsydeAcpiProt::WriteAcpiTPColor", "RGBKB::SetColor"]:
            for opcode in ["call", "callvirt", "ldftn", "ldvirtftn"]:
                fnkey, cpuoc, kbled = inputs()
                kbled.methods.append({"name": "Page::ApplyTp", "instructions": [
                    {"line": 1, "offset": "IL_0000", "opcode": opcode, "operand": f"void CC.{target}()"}]})
                with self.assertRaises(ValueError):
                    AUDIT.assess(fnkey, cpuoc, kbled)

    def test_method_strings_and_suffixes_are_not_real_calls(self):
        data = module([], [("RGBKB::SetColor", "IL_0000: ret"),
            ("Page::Render", 'IL_0000: ldstr "RGBKB::SetColor()"\n'
             'IL_0001: call void CC.OtherRGBKB::SetColor()\n'
             'IL_0002: call void CC.RGBKB::SetColorExtra()')])
        self.assertEqual(data.calls("RGBKB::SetColor"), [])


if __name__ == "__main__":
    unittest.main()
