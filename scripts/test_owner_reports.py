"""Authored security regression cases; not executed in this source sprint."""
import gzip
import importlib.util
import io
from pathlib import Path
import tarfile
import unittest

spec = importlib.util.spec_from_file_location("owner_reports", Path(__file__).with_name("owner_reports.py"))
reports = importlib.util.module_from_spec(spec)
spec.loader.exec_module(reports)


class OwnerReportTests(unittest.TestCase):
    def archive(self, name, link=False):
        buf = io.BytesIO()
        with tarfile.open(fileobj=buf, mode="w", format=tarfile.USTAR_FORMAT) as tar:
            item = tarfile.TarInfo(name)
            if link:
                item.type, item.linkname = tarfile.SYMTYPE, "/outside"
                tar.addfile(item)
            else:
                item.size = 4
                tar.addfile(item, io.BytesIO(b"data"))
        return gzip.compress(buf.getvalue(), mtime=0)

    def test_archive_escape_and_symlink_refuse(self):
        for payload in [self.archive("../outside"), self.archive("safe", link=True)]:
            with self.assertRaises(ValueError):
                reports.members(payload, None, "portable")

    def test_source_file_does_not_become_a_native_binary(self):
        with self.assertRaises(ValueError):
            reports.members(self.archive("bin/" + reports.TOOL), "bin/" + reports.TOOL, "x86_64-unknown-linux-gnu")

    def test_declared_passed_flag_cannot_replace_selected_proofs(self):
        with self.assertRaises(ValueError):
            reports.qualified({"schema_version": "flow.qualified-owner-report-request/v1", "passed": True})


if __name__ == "__main__":
    unittest.main()
