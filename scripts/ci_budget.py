"""Bounded workflow-budget source inspection; never product qualification.

Actionlint owns YAML/expression syntax. This reader intentionally supports only
literal owning workflow job metadata and refuses unsupported budget shapes.
"""
from __future__ import annotations
import argparse
import json
from pathlib import Path
import re
import sys

SCHEMA = "egohygiene.ci-budget-plan/v1"
POLICY_SCHEMA = "egohygiene.ci-budget-policy/v1"
HOSTED = "(github.event_name == 'workflow_dispatch' || vars.FLOW_SUITE_HOSTED_CI == 'enabled')"
SCHEDULED = "(github.event_name != 'schedule' || vars.FLOW_SUITE_SCHEDULED_CI == 'enabled')"
BUDGET_GATE = HOSTED + " && " + SCHEDULED
MAX_REPORT = 32768
POLICY_KEYS = {"schema_version", "cheap_workflow", "cheap_timeout_minutes", "default_heavy_timeout_minutes",
               "maximum_heavy_timeout_minutes", "manual_authority_workflows", "retention_review_days"}
REPORT_KEYS = {"schema_version", "validation_scope", "event", "hosted_opt_in", "scheduled_opt_in",
               "product_qualified", "domain_evidence", "hosted_jobs_executed", "jobs", "manual_authority_exclusions",
               "retention_review", "qualification_state"}


def require(ok, message):
    if not ok:
        raise ValueError(message)


def json_document(path, maximum):
    data = Path(path).read_bytes()
    require(len(data) <= maximum, "JSON input exceeds bound")
    return json.loads(data)


def validate_policy(value):
    require(type(value) is dict and set(value) == POLICY_KEYS, "closed budget policy required")
    require(value["schema_version"] == POLICY_SCHEMA, "unsupported budget policy")
    require(value["cheap_workflow"] == "workflow-lint.yml", "owning cheap workflow required")
    for key in ("cheap_timeout_minutes", "default_heavy_timeout_minutes", "maximum_heavy_timeout_minutes", "retention_review_days"):
        require(type(value[key]) is int and value[key] > 0, "literal positive budget required")
    require(value["cheap_timeout_minutes"] <= 5, "cheap lane exceeds five minutes")
    require(value["default_heavy_timeout_minutes"] <= value["maximum_heavy_timeout_minutes"] <= 70,
            "heavy timeout exceeds policy")
    require(value["manual_authority_workflows"] == ["release.yml", "release-candidate.yml"],
            "manual publication exclusions must remain explicit")
    require(value["retention_review_days"] <= 30, "retention review threshold exceeds policy")
    return value


def scalar(lines, index):
    value = lines[index].split(":", 1)[1].strip()
    if value in {">", ">-", "|", "|-"}:
        pieces = []
        for line in lines[index + 1:]:
            if line.strip() and len(line) - len(line.lstrip()) <= 4:
                break
            if line.strip() and not line.lstrip().startswith("#"):
                pieces.append(line.strip())
        value = " ".join(pieces)
    if value.startswith("${{") and value.endswith("}}"):
        value = value[3:-2].strip()
    return value


def job_blocks(text):
    lines = text.splitlines()
    starts = []; active = False
    for index, line in enumerate(lines):
        if line == "jobs:": active = True; continue
        if active and line and not line.startswith((" ", "#")):
            break
        if active:
            match = re.fullmatch(r"  ([A-Za-z_][A-Za-z0-9_-]*):\s*", line)
            if match: starts.append((index, match[1]))
    require(starts, "literal job inventory required")
    result = []
    for position, (start, name) in enumerate(starts):
        end = starts[position + 1][0] if position + 1 < len(starts) else len(lines)
        block = lines[start:end]; condition = None; timeout = None
        for index, line in enumerate(block):
            if line.startswith("    if:"):
                require(condition is None, "duplicate job condition")
                condition = scalar(block, index)
            if line.startswith("    timeout-minutes:"):
                require(timeout is None, "duplicate job timeout")
                raw = line.split(":", 1)[1].strip()
                require(raw.isdecimal(), "literal job timeout required")
                timeout = int(raw)
        result.append({"id": name, "condition": condition, "timeout_minutes": timeout})
    return result


def outer_group(value):
    if not value.startswith("("): return False
    depth = 0; quote = None; escaped = False
    for index, char in enumerate(value):
        if quote:
            if escaped: escaped = False
            elif char == "\\": escaped = True
            elif char == quote: quote = None
            continue
        if char in "\"'": quote = char
        elif char == "(": depth += 1
        elif char == ")":
            depth -= 1
            if depth == 0: return index == len(value) - 1
            if depth < 0: return False
    return False


def has_budget_gate(condition):
    if condition == BUDGET_GATE: return True
    prefix = BUDGET_GATE + " && "
    return bool(condition and condition.startswith(prefix) and outer_group(condition[len(prefix):]))


def budget_eligible(event, hosted, scheduled):
    return (event == "workflow_dispatch" or hosted) and (event != "schedule" or scheduled)


def require_manual_authority_triggers(text):
    """Exempt only the reviewed literal manual trigger shape, never by filename."""
    lines = text.splitlines()
    starts = [index for index, line in enumerate(lines) if re.match(r"^on\s*:", line)]
    require(len(starts) == 1 and re.fullmatch(r"on:\s*(?:#.*)?", lines[starts[0]]) is not None,
            "manual authority requires explicit workflow_dispatch-only triggers")
    events = []
    for line in lines[starts[0] + 1:]:
        if not line.strip() or line.lstrip().startswith("#"): continue
        if not line.startswith(" "): break
        if line.startswith("  ") and not line.startswith("   "):
            match = re.fullmatch(r"  (workflow_dispatch):\s*(?:\{\})?\s*(?:#.*)?", line)
            require(match is not None, "manual authority requires explicit workflow_dispatch-only triggers")
            events.append(match[1])
        else:
            require(bool(events) and line.startswith("    "), "unsupported manual authority trigger shape")
    require(events == ["workflow_dispatch"], "manual authority requires explicit workflow_dispatch-only triggers")


def artifact_retention_review(text, workflow, threshold):
    """Report absent/default/dynamic retention as unknown costs, without guessing."""
    lines = text.splitlines(); observations = []
    for index, line in enumerate(lines):
        match = re.fullmatch(r"( *)(- )?uses:\s*['\"]?actions/upload-artifact@[^\s'\"]+['\"]?\s*(?:#.*)?", line)
        if match is None: continue
        step_indent = len(match[1]) if match[2] else len(match[1]) - 2
        end = index + 1
        while end < len(lines):
            other = lines[end]
            if other.strip() and not other.lstrip().startswith("#") and len(other) - len(other.lstrip()) <= step_indent: break
            end += 1
        retention = [(offset, other.split(":", 1)[1].strip())
                     for offset, other in enumerate(lines[index + 1:end], start=index + 1)
                     if re.fullmatch(r"\s+retention-days:.*", other)]
        days = None; state = "review_required_default_retention"; observed_line = index + 1
        if len(retention) == 1:
            offset, raw = retention[0]; observed_line = offset + 1
            literal = re.fullmatch(r"(?:([0-9]+)|'([0-9]+)'|\"([0-9]+)\")\s*(?:#.*)?", raw)
            if literal:
                parsed = int(next(part for part in literal.groups() if part is not None))
                if parsed > 0:
                    days = parsed; state = "review_required_existing_retention"
            else:
                state = "review_required_dynamic_retention"
        elif len(retention) > 1:
            state = "review_required_dynamic_retention"
        if days is None or days > threshold:
            observations.append({"workflow": workflow, "line": observed_line, "days": days, "state": state})
    require(len(observations) <= 256, "bounded artifact retention observations required")
    return observations


def inspect_workflows(root, policy):
    rows = []; retention = []; excluded = []
    paths = sorted((Path(root) / ".github/workflows").glob("*.y*ml"))
    require(0 < len(paths) <= 32, "bounded workflow inventory required")
    cheap_seen = False
    for path in paths:
        require(not path.is_symlink() and path.stat().st_size <= 524288, "bounded regular workflow required")
        text = path.read_text(); jobs = job_blocks(text)
        if path.name in policy["manual_authority_workflows"]:
            require_manual_authority_triggers(text)
            excluded.append(path.name); continue
        require("\nconcurrency:" in "\n" + text, "workflow concurrency required: " + path.name)
        cheap = path.name == policy["cheap_workflow"]
        if cheap:
            cheap_seen = True
            require(len(jobs) == 1 and jobs[0]["id"] == "workflow-expressions", "one owning cheap job required")
            on = re.search(r"(?m)^on:\n(.*?)(?=^[^ #\n]|\Z)", text, flags=re.S)
            require(on is not None, "explicit cheap workflow triggers required")
            triggers = on[1]
            require("  pull_request:" in triggers and "  push:" in triggers and "    branches: [main]" in triggers,
                    "cheap lane must run on every PR and main push")
            require("paths:" not in triggers and "paths-ignore:" not in triggers, "cheap lane must be path independent")
        for job in jobs:
            timeout = job["timeout_minutes"]
            limit = policy["cheap_timeout_minutes"] if cheap else policy["maximum_heavy_timeout_minutes"]
            require(type(timeout) is int and 0 < timeout <= limit, "missing/over-budget timeout: " + path.name + "/" + job["id"])
            require(job["condition"] is None if cheap else has_budget_gate(job["condition"]),
                    "absent or weakened budget gate: " + path.name + "/" + job["id"])
            rows.append({"workflow": path.name, "job_id": job["id"], "timeout_minutes": timeout, "cheap": cheap})
        retention.extend(artifact_retention_review(text, path.name, policy["retention_review_days"]))
    require(cheap_seen and len(rows) <= 256, "bounded inventory must include cheap workflow")
    return rows, retention, excluded


def make_plan(root, policy, event, hosted=False, scheduled=False):
    rows, retention, excluded = inspect_workflows(root, policy)
    eligible = budget_eligible(event, hosted, scheduled)
    for row in rows:
        row["budget_eligible"] = True if row["cheap"] else eligible
        row["original_conditions_evaluated"] = False
        row["state"] = "source_check_only" if row["cheap"] else ("unobserved" if eligible else "qualification_deferred")
    value = {"schema_version": SCHEMA, "validation_scope": "workflow_sources_only", "event": event,
             "hosted_opt_in": hosted, "scheduled_opt_in": scheduled, "product_qualified": False,
             "domain_evidence": False, "hosted_jobs_executed": False, "jobs": rows,
             "manual_authority_exclusions": excluded, "retention_review": retention,
             "qualification_state": "unobserved" if eligible else "qualification_deferred"}
    validate_report(value)
    return value


def validate_report(value):
    require(type(value) is dict and set(value) == REPORT_KEYS, "closed source budget report required")
    require(value["schema_version"] == SCHEMA and value["validation_scope"] == "workflow_sources_only", "unsupported source report")
    require(value["product_qualified"] is False and value["domain_evidence"] is False and value["hosted_jobs_executed"] is False,
            "source report cannot establish execution or product authority")
    require(value["qualification_state"] in {"qualification_deferred", "unobserved"}, "qualification cannot become green")
    require(type(value["jobs"]) is list and len(value["jobs"]) <= 256, "bounded job report required")
    require(value["event"] in {"pull_request", "push", "workflow_dispatch", "schedule", "release"}, "known event required")
    require(type(value["hosted_opt_in"]) is bool and type(value["scheduled_opt_in"]) is bool, "explicit opt-in booleans required")
    eligible = budget_eligible(value["event"], value["hosted_opt_in"], value["scheduled_opt_in"])
    require(value["qualification_state"] == ("unobserved" if eligible else "qualification_deferred"), "qualification state disagrees with policy")
    row_keys = {"workflow", "job_id", "timeout_minutes", "cheap", "budget_eligible", "original_conditions_evaluated", "state"}
    for row in value["jobs"]:
        require(type(row) is dict and set(row) == row_keys, "closed job source record required")
        require(type(row["cheap"]) is bool and type(row["budget_eligible"]) is bool, "explicit job policy booleans required")
        require(row["original_conditions_evaluated"] is False, "source plan cannot evaluate workflow conditions")
        require(re.fullmatch(r"[A-Za-z0-9_-]+\.ya?ml", row["workflow"]) is not None
                and re.fullmatch(r"[A-Za-z_][A-Za-z0-9_-]*", row["job_id"]) is not None, "literal job identity required")
        require(type(row["timeout_minutes"]) is int and 0 < row["timeout_minutes"] <= (5 if row["cheap"] else 70), "reported budget exceeds bound")
        require(row["budget_eligible"] == (True if row["cheap"] else eligible), "job eligibility disagrees with policy")
        require(row["state"] == ("source_check_only" if row["cheap"] else ("unobserved" if eligible else "qualification_deferred")), "source job cannot become green execution")
    require(type(value["manual_authority_exclusions"]) is list and set(value["manual_authority_exclusions"]) <= {"release.yml", "release-candidate.yml"}, "explicit manual exclusions required")
    require(type(value["retention_review"]) is list and len(value["retention_review"]) <= 256, "bounded retention review required")
    for row in value["retention_review"]:
        require(type(row) is dict and set(row) == {"workflow", "line", "days", "state"}
                and type(row["workflow"]) is str and re.fullmatch(r"[A-Za-z0-9_-]+\.ya?ml", row["workflow"]) is not None
                and type(row["line"]) is int and row["line"] > 0, "closed retention observation required")
        require((row["state"] == "review_required_existing_retention" and type(row["days"]) is int and row["days"] > 0)
                or (row["state"] in {"review_required_default_retention", "review_required_dynamic_retention"} and row["days"] is None),
                "retention observation cannot hide unknown/default days")
    require(len(json.dumps(value, ensure_ascii=False).encode()) <= MAX_REPORT, "report exceeds output bound")
    return value


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--policy", type=Path)
    parser.add_argument("--event", choices=["pull_request", "push", "workflow_dispatch", "schedule", "release"], default="pull_request")
    parser.add_argument("--hosted-enabled", choices=["true", "false"], default="false")
    parser.add_argument("--scheduled-enabled", choices=["true", "false"], default="false")
    args = parser.parse_args()
    policy = validate_policy(json_document(args.policy or args.root / "ci/budget-policy.json", 4096))
    result = make_plan(args.root, policy, args.event, args.hosted_enabled == "true", args.scheduled_enabled == "true")
    output = json.dumps(result, sort_keys=True, separators=(",", ":"), ensure_ascii=False) + "\n"
    require(len(output.encode()) <= MAX_REPORT, "report exceeds output bound")
    sys.stdout.write(output)


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, json.JSONDecodeError) as error:
        sys.stderr.write("workflow budget source check refused: " + str(error)[:1000] + "\n")
        raise SystemExit(2)
