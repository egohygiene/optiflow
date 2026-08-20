#!/usr/bin/env python3
"""Verify the composed static site and its local link contract."""

from __future__ import annotations

import json
import sys
from html.parser import HTMLParser
from pathlib import Path
from urllib.parse import unquote, urlsplit


REQUIRED_PATHS = (
    "index.html",
    ".nojekyll",
    "assets/mark.svg",
    "assets/site.css",
    "assets/site.js",
    "architecture/index.html",
    "architecture/architecture.json",
    "architecture/assets/architecture.css",
    "architecture/assets/architecture.js",
    "docs/index.html",
    "docs/getting-started/index.html",
    "docs/architecture/index.html",
    "docs/safety-model/index.html",
    "schemas/command-result.schema.json",
    "schemas/config-v1.schema.json",
    "schemas/effective-policy-v1.schema.json",
    "schemas/architecture-portal-config-v1.schema.json",
    "schemas/architecture-portal-v1.schema.json",
)

INTELLIGENCE_PATHS = (
    "intelligence/index.html",
    "intelligence/styles.css",
    "intelligence/explorer.js",
    "intelligence/summary.json",
)

REQUIRED_COPY = (
    "Know what is <em>actually</em> on disk.",
    "v0.1 · read-only by design",
    "Prebuilt releases are not published yet.",
    "LaunchKit",
)

FORBIDDEN_COPY = (
    "Lorem ipsum",
    "Trusted by fast-growing companies",
    "Sign Up",
    "Request a Demo",
)

REQUIRED_ARCHITECTURE_COPY = (
    "Architecture as executable context",
    "Generated from repository truth",
    "Eighteen documents. One connected system.",
    "The page is an output, not another authority.",
)


class LinkCollector(HTMLParser):
    def __init__(self) -> None:
        super().__init__()
        self.links: list[str] = []
        self.identifiers: list[str] = []

    def handle_starttag(
        self,
        tag: str,
        attributes: list[tuple[str, str | None]],
    ) -> None:
        for name, value in attributes:
            if name == "id" and value:
                self.identifiers.append(value)

        if tag not in {"a", "link", "script", "img"}:
            return

        attribute_name = "href" if tag in {"a", "link"} else "src"
        for name, value in attributes:
            if name == attribute_name and value:
                self.links.append(value)


def resolve_local_target(site_root: Path, document: Path, raw_link: str) -> Path | None:
    parts = urlsplit(raw_link)
    if parts.scheme or parts.netloc or raw_link.startswith(("mailto:", "tel:")):
        return None

    path = unquote(parts.path)
    if not path:
        return document if parts.fragment else None

    if path.startswith("/"):
        target = site_root / path.lstrip("/")
    else:
        target = document.parent / path

    if path.endswith("/"):
        target /= "index.html"

    return target


def verify_architecture(site_root: Path) -> list[str]:
    errors: list[str] = []
    data_path = site_root / "architecture/architecture.json"
    page_path = site_root / "architecture/index.html"
    if not data_path.is_file() or not page_path.is_file():
        return errors

    page = page_path.read_text(encoding="utf-8")
    for expected in REQUIRED_ARCHITECTURE_COPY:
        if expected not in page:
            errors.append(f"architecture page is missing required copy: {expected!r}")

    try:
        data = json.loads(data_path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as error:
        return [f"architecture data is not valid JSON: {error}"]

    if data.get("schema") != "optiflow.architecture-portal.v1":
        errors.append("architecture data has an unexpected schema")

    documents = data.get("documents")
    edges = data.get("edges")
    stats = data.get("stats")
    if not isinstance(documents, list) or len(documents) != 18:
        errors.append("architecture data must contain exactly 18 canonical documents")
        return errors
    if not isinstance(edges, list):
        errors.append("architecture data edges must be an array")
        return errors
    if not isinstance(stats, dict):
        errors.append("architecture data stats must be an object")
        return errors

    document_ids = [document.get("id") for document in documents]
    if any(not isinstance(document_id, str) for document_id in document_ids):
        errors.append("every architecture document requires a string id")
        return errors
    if len(document_ids) != len(set(document_ids)):
        errors.append("architecture document ids must be unique")

    expected_ids = set(document_ids)
    graph: dict[str, set[str]] = {document_id: set() for document_id in document_ids}
    for edge in edges:
        if not isinstance(edge, dict):
            errors.append("architecture edge must be an object")
            continue
        source = edge.get("from")
        target = edge.get("to")
        if source not in expected_ids or target not in expected_ids:
            errors.append(f"architecture edge does not resolve: {source!r} -> {target!r}")
            continue
        graph[target].add(source)

    visiting: set[str] = set()
    visited: set[str] = set()

    def visit(document_id: str) -> None:
        if document_id in visiting:
            errors.append(f"architecture dependency cycle contains {document_id}")
            return
        if document_id in visited:
            return
        visiting.add(document_id)
        for dependency in graph[document_id]:
            visit(dependency)
        visiting.remove(document_id)
        visited.add(document_id)

    for document_id in document_ids:
        visit(document_id)

    if stats.get("documents") != len(documents):
        errors.append("architecture document count does not match stats")
    if stats.get("dependencies") != len(edges):
        errors.append("architecture dependency count does not match stats")
    if page.count('class="document-card ') != len(documents):
        errors.append("architecture page document cards do not match architecture data")
    if page.count('class="graph-node ') != len(documents):
        errors.append("architecture graph nodes do not match architecture data")

    return errors


def verify(site_root: Path, *, allow_missing_intelligence: bool = False) -> list[str]:
    errors: list[str] = []
    resolved_root = site_root.resolve()

    for relative_path in REQUIRED_PATHS:
        if not (site_root / relative_path).is_file():
            errors.append(f"missing required site path: {relative_path}")

    intelligence_root = site_root / "intelligence"
    if not intelligence_root.exists() and not allow_missing_intelligence:
        errors.append("missing required site path: intelligence/")
    elif intelligence_root.exists():
        for relative_path in INTELLIGENCE_PATHS:
            if not (site_root / relative_path).is_file():
                errors.append(f"missing required intelligence path: {relative_path}")
        summary_path = intelligence_root / "summary.json"
        if summary_path.is_file():
            try:
                intelligence = json.loads(summary_path.read_text(encoding="utf-8"))
            except json.JSONDecodeError as error:
                errors.append(f"intelligence summary is not valid JSON: {error}")
            else:
                if not isinstance(intelligence, dict):
                    errors.append("intelligence summary must be a JSON object")
                elif intelligence.get("schema") != "egohygiene.repository-intelligence-dashboard/v3":
                    errors.append("intelligence summary has an unexpected schema")
                elif intelligence.get("schema_version") != 1:
                    errors.append("intelligence summary has an unexpected schema version")

    landing_path = site_root / "index.html"
    if landing_path.is_file():
        landing = landing_path.read_text(encoding="utf-8")
        for expected in REQUIRED_COPY:
            if expected not in landing:
                errors.append(f"landing page is missing required copy: {expected!r}")
        for forbidden in FORBIDDEN_COPY:
            if forbidden in landing:
                errors.append(f"landing page contains template copy: {forbidden!r}")

    errors.extend(verify_architecture(site_root))

    identifier_cache: dict[Path, set[str]] = {}

    for document in sorted(site_root.rglob("*.html")):
        parser = LinkCollector()
        parser.feed(document.read_text(encoding="utf-8"))
        identifiers = set(parser.identifiers)
        identifier_cache[document.resolve()] = identifiers
        if len(parser.identifiers) != len(identifiers):
            errors.append(f"{document.relative_to(site_root)}: duplicate HTML id")

        for raw_link in parser.links:
            target = resolve_local_target(site_root, document, raw_link)
            if target is None:
                continue

            try:
                target.resolve().relative_to(resolved_root)
            except ValueError:
                errors.append(f"{document.relative_to(site_root)}: link escapes site root: {raw_link}")
                continue

            if target.is_dir():
                target /= "index.html"

            if not target.exists():
                if (
                    allow_missing_intelligence
                    and urlsplit(raw_link).path.rstrip("/") == "/intelligence"
                ):
                    continue
                errors.append(f"{document.relative_to(site_root)}: broken local link: {raw_link}")
                continue

            fragment = unquote(urlsplit(raw_link).fragment)
            # Zensical's generated 404 template retains the Material skip link
            # while omitting the normal page-content target.
            if document.name == "404.html" and fragment == "__skip":
                continue
            if fragment and target.suffix == ".html":
                resolved_target = target.resolve()
                target_identifiers = identifier_cache.get(resolved_target)
                if target_identifiers is None:
                    target_parser = LinkCollector()
                    target_parser.feed(target.read_text(encoding="utf-8"))
                    target_identifiers = set(target_parser.identifiers)
                    identifier_cache[resolved_target] = target_identifiers
                if fragment not in target_identifiers:
                    errors.append(
                        f"{document.relative_to(site_root)}: broken local anchor: {raw_link}"
                    )

    return errors


def main() -> int:
    arguments = sys.argv[1:]
    allow_missing_intelligence = "--allow-missing-intelligence" in arguments
    paths = [argument for argument in arguments if not argument.startswith("--")]
    site_root = Path(paths[0] if paths else "dist")
    if not site_root.is_dir():
        print(f"site root is not a directory: {site_root}", file=sys.stderr)
        return 1

    errors = verify(
        site_root,
        allow_missing_intelligence=allow_missing_intelligence,
    )
    if errors:
        for error in errors:
            print(error, file=sys.stderr)
        return 1

    html_count = sum(1 for _ in site_root.rglob("*.html"))
    print(f"verified {html_count} HTML documents under {site_root}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
