#!/usr/bin/env python3
"""Verify the composed static site and its local link contract."""

from __future__ import annotations

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
    "docs/index.html",
    "docs/getting-started/index.html",
    "docs/architecture/index.html",
    "docs/safety-model/index.html",
    "schemas/command-result.schema.json",
    "schemas/config-v1.schema.json",
    "schemas/effective-policy-v1.schema.json",
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


class LinkCollector(HTMLParser):
    def __init__(self) -> None:
        super().__init__()
        self.links: list[str] = []

    def handle_starttag(
        self,
        tag: str,
        attributes: list[tuple[str, str | None]],
    ) -> None:
        if tag not in {"a", "link", "script", "img"}:
            return

        attribute_name = "href" if tag in {"a", "link"} else "src"
        for name, value in attributes:
            if name == attribute_name and value:
                self.links.append(value)


def resolve_local_target(site_root: Path, document: Path, raw_link: str) -> Path | None:
    parts = urlsplit(raw_link)
    if parts.scheme or parts.netloc or raw_link.startswith(("#", "mailto:", "tel:")):
        return None

    path = unquote(parts.path)
    if not path:
        return None

    if path.startswith("/"):
        target = site_root / path.lstrip("/")
    else:
        target = document.parent / path

    if path.endswith("/"):
        target /= "index.html"

    return target


def verify(site_root: Path) -> list[str]:
    errors: list[str] = []
    resolved_root = site_root.resolve()

    for relative_path in REQUIRED_PATHS:
        if not (site_root / relative_path).is_file():
            errors.append(f"missing required site path: {relative_path}")

    landing_path = site_root / "index.html"
    if landing_path.is_file():
        landing = landing_path.read_text(encoding="utf-8")
        for expected in REQUIRED_COPY:
            if expected not in landing:
                errors.append(f"landing page is missing required copy: {expected!r}")
        for forbidden in FORBIDDEN_COPY:
            if forbidden in landing:
                errors.append(f"landing page contains template copy: {forbidden!r}")

    for document in sorted(site_root.rglob("*.html")):
        parser = LinkCollector()
        parser.feed(document.read_text(encoding="utf-8"))

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
                errors.append(f"{document.relative_to(site_root)}: broken local link: {raw_link}")

    return errors


def main() -> int:
    site_root = Path(sys.argv[1] if len(sys.argv) > 1 else "dist")
    if not site_root.is_dir():
        print(f"site root is not a directory: {site_root}", file=sys.stderr)
        return 1

    errors = verify(site_root)
    if errors:
        for error in errors:
            print(error, file=sys.stderr)
        return 1

    html_count = sum(1 for _ in site_root.rglob("*.html"))
    print(f"verified {html_count} HTML documents under {site_root}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
