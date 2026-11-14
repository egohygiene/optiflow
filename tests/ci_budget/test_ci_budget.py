"""Synthetic source-only budget regressions: no workflow/product is dispatched."""
import copy
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("owning_ci_budget", ROOT / "scripts/ci_budget.py")
budget = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(budget)
POLICY = json.loads((ROOT / "ci/budget-policy.json").read_text())


class WorkflowBudgetTests(unittest.TestCase):
    def setUp(self):
        self.private = tempfile.TemporaryDirectory()
        self.addCleanup(self.private.cleanup)
        self.root = Path(self.private.name)
        self.workflows = self.root / ".github/workflows"
        self.workflows.mkdir(parents=True)
        (self.workflows / "workflow-lint.yml").write_text((ROOT / ".github/workflows/workflow-lint.yml").read_text())
        self.good = "name: CI\non:\n  pull_request:\nconcurrency:\n  group: sample\n  cancel-in-progress: false\njobs:\n  qualify:\n    if: " + budget.BUDGET_GATE + "\n    runs-on: ubuntu-latest\n    timeout-minutes: 30\n    steps:\n      - run: ':'\n"
        self.path = self.workflows / "ci.yml"
        self.path.write_text(self.good)

    def plan(self, event="pull_request", hosted=False, scheduled=False):
        return budget.make_plan(self.root, budget.validate_policy(copy.deepcopy(POLICY)), event, hosted, scheduled)

    def test_default_source_plan_defers_hosted_qualification(self):
        report = self.plan()
        self.assertEqual(report["qualification_state"], "qualification_deferred")
        self.assertFalse(report["product_qualified"])
        self.assertFalse(next(row for row in report["jobs"] if not row["cheap"])["budget_eligible"])

    def test_manual_and_scheduled_opt_ins_have_distinct_truth_tables(self):
        for event, hosted, scheduled, eligible in [
            ("pull_request", False, False, False), ("push", True, False, True),
            ("workflow_dispatch", False, False, True), ("schedule", True, False, False),
            ("schedule", False, True, False), ("schedule", True, True, True),
        ]:
            with self.subTest(event=event, hosted=hosted, scheduled=scheduled):
                report = self.plan(event, hosted, scheduled)
                row = next(row for row in report["jobs"] if not row["cheap"])
                self.assertEqual(row["budget_eligible"], eligible)
                self.assertFalse(row["original_conditions_evaluated"])
                self.assertEqual(row["state"], "unobserved" if eligible else "qualification_deferred")

    def test_absent_or_weakened_gates_are_refused(self):
        for replacement in ["true", "true || " + budget.BUDGET_GATE, budget.BUDGET_GATE + " && (true) || true"]:
            with self.subTest(replacement=replacement):
                self.path.write_text(self.good.replace(budget.BUDGET_GATE, replacement))
                with self.assertRaisesRegex(ValueError, "budget gate"):
                    self.plan()

    def test_missing_or_excessive_timeouts_are_refused(self):
        for replacement in ["", "    timeout-minutes: 71\n", "    timeout-minutes: '${{ inputs.timeout }}'\n"]:
            with self.subTest(timeout=replacement):
                self.path.write_text(self.good.replace("    timeout-minutes: 30\n", replacement))
                with self.assertRaises(ValueError): self.plan()

    def test_original_condition_remains_a_complete_conjunct(self):
        value = budget.BUDGET_GATE + " && (github.event_name == 'pull_request' || inputs.tier == 'pr')"
        self.path.write_text(self.good.replace(budget.BUDGET_GATE, value))
        self.plan()

    def test_cheap_gate_cannot_be_path_filtered(self):
        path = self.workflows / "workflow-lint.yml"
        path.write_text(path.read_text().replace("  pull_request:\n", "  pull_request:\n    paths: ['docs/**']\n"))
        with self.assertRaisesRegex(ValueError, "path independent"): self.plan()

    def test_manual_authority_workflows_are_explicitly_excluded(self):
        (self.workflows / "release.yml").write_text("name: Release\non:\n  workflow_dispatch:\njobs:\n  publish:\n    runs-on: ubuntu-latest\n    steps:\n      - run: ':'\n")
        report = self.plan()
        self.assertIn("release.yml", report["manual_authority_exclusions"])
        self.assertFalse(any(row["workflow"] == "release.yml" for row in report["jobs"]))

    def test_manual_authority_exclusion_refuses_other_or_unknown_triggers(self):
        for trigger in ["on:\n  push:\n  workflow_dispatch:\n", "on:\n  workflow_dispatch:\n  schedule:\n    - cron: '0 0 * * *'\n",
                        "on: [workflow_dispatch]\n", "on: workflow_dispatch\n", "on:\n  workflow_dispatch: *events\n",
                        "on:\n  workflow_dispatch:\n  workflow_dispatch:\n"]:
            with self.subTest(trigger=trigger):
                path = self.workflows / "release-candidate.yml"
                path.write_text("name: Release\n" + trigger + "jobs:\n  publish:\n    runs-on: ubuntu-latest\n    steps:\n      - run: ':'\n")
                with self.assertRaisesRegex(ValueError, "manual authority"):
                    self.plan()

    def test_existing_long_retention_requires_visible_review(self):
        self.path.write_text(self.good + "      - uses: actions/upload-artifact@v4\n        with:\n          retention-days: 90\n")
        self.assertEqual(self.plan()["retention_review"][0]["days"], 90)

    def test_absent_default_dynamic_and_literal_artifact_retention_are_visible(self):
        for setting, state, days in [("", "review_required_default_retention", None),
                                     ("          retention-days: 0\n", "review_required_default_retention", None),
                                     ("          retention-days: '${{ inputs.days }}'\n", "review_required_dynamic_retention", None),
                                     ("          retention-days: '90' # qualification history\n", "review_required_existing_retention", 90)]:
            with self.subTest(setting=setting):
                self.path.write_text(self.good + "      - name: Upload source fixture\n        uses: actions/upload-artifact@v4\n        with:\n          name: sample\n" + setting)
                rows = self.plan()["retention_review"]
                self.assertEqual(len(rows), 1)
                self.assertEqual((rows[0]["state"], rows[0]["days"]), (state, days))
                self.assertGreater(rows[0]["line"], 0)
        self.path.write_text(self.good + "      - uses: actions/upload-artifact@v4\n        with:\n          retention-days: 7\n      - uses: actions/upload-artifact@v4\n        with:\n          name: second-with-default\n")
        rows = self.plan()["retention_review"]
        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0]["state"], "review_required_default_retention")

    def test_retention_report_is_closed_and_cannot_forge_unknown_days(self):
        self.path.write_text(self.good + "      - uses: actions/upload-artifact@v4\n")
        report = self.plan(); report["retention_review"][0]["days"] = 7
        with self.assertRaisesRegex(ValueError, "unknown/default"): budget.validate_report(report)
        report = self.plan(); report["retention_review"][0]["unbounded"] = "extra"
        with self.assertRaisesRegex(ValueError, "closed retention"): budget.validate_report(report)

    def test_source_report_cannot_claim_authority_or_success(self):
        for field in ("product_qualified", "domain_evidence", "hosted_jobs_executed"):
            report = self.plan(); report[field] = True
            with self.subTest(field=field):
                with self.assertRaisesRegex(ValueError, "authority"): budget.validate_report(report)
        report = self.plan(); report["jobs"][0]["state"] = "passed"
        with self.assertRaisesRegex(ValueError, "green execution"): budget.validate_report(report)
        report = self.plan(); report["private_capture"] = "not eligible"
        with self.assertRaisesRegex(ValueError, "closed"): budget.validate_report(report)


if __name__ == "__main__":
    unittest.main()
