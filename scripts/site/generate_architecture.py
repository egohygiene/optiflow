#!/usr/bin/env python3
"""Generate the OptiFlow architecture portal from canonical repository truth."""

from __future__ import annotations

import argparse
import hashlib
import html
import json
import re
from dataclasses import dataclass
from pathlib import Path
from urllib.parse import quote


CANONICAL_DOCUMENTS = (
    "PURPOSE.md",
    "VISION.md",
    "PRINCIPLES.md",
    "PILLARS.md",
    "MANIFESTO.md",
    "EPISTEMOLOGY.md",
    "ONTOLOGY.md",
    "PERSONAL_MODEL.md",
    "AI_CONSTITUTION.md",
    "FOUNDATIONS.md",
    "SYSTEM.md",
    "ARCHITECTURE.md",
    "DESIGN.md",
    "DESIGN_SYSTEM.md",
    "METHODOLOGY.md",
    "DECISIONS.md",
    "ROADMAP.md",
    "META.md",
)

REQUIRED_METADATA = {
    "schema",
    "id",
    "title",
    "kind",
    "version",
    "status",
    "owners",
    "created",
    "updated",
    "governed_by",
    "depends_on",
    "related",
    "supersedes",
}

LIST_METADATA = {
    "owners",
    "governed_by",
    "depends_on",
    "related",
    "supersedes",
}


@dataclass(frozen=True)
class ArchitectureDocument:
    path: str
    document_id: str
    title: str
    version: str
    status: str
    owners: tuple[str, ...]
    governed_by: tuple[str, ...]
    depends_on: tuple[str, ...]
    related: tuple[str, ...]
    supersedes: tuple[str, ...]


def parse_arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--repository-root",
        required=True,
        type=Path,
        help="Path to the OptiFlow repository root.",
    )
    parser.add_argument(
        "--config",
        required=True,
        type=Path,
        help="Path to the architecture portal configuration.",
    )
    parser.add_argument(
        "--output",
        required=True,
        type=Path,
        help="Directory that receives index.html and architecture.json.",
    )
    return parser.parse_args()


def parse_frontmatter(path: Path) -> dict[str, str | list[str]]:
    text = path.read_text(encoding="utf-8")
    if not text.startswith("---\n"):
        raise ValueError(f"{path.name}: missing opening frontmatter delimiter")

    try:
        raw_frontmatter = text.split("---\n", 2)[1]
    except IndexError as error:
        raise ValueError(f"{path.name}: missing closing frontmatter delimiter") from error

    metadata: dict[str, str | list[str]] = {}
    active_list: str | None = None

    for line_number, line in enumerate(raw_frontmatter.splitlines(), start=2):
        list_item = re.fullmatch(r"  - (.+)", line)
        if list_item:
            if active_list is None:
                raise ValueError(f"{path.name}:{line_number}: orphaned list item")
            value = metadata[active_list]
            if not isinstance(value, list):
                raise ValueError(f"{path.name}:{line_number}: invalid list value")
            value.append(list_item.group(1).strip())
            continue

        scalar = re.fullmatch(r"([a-z_]+):(?: (.*))?", line)
        if not scalar:
            raise ValueError(f"{path.name}:{line_number}: unsupported frontmatter syntax")

        key, raw_value = scalar.groups()
        if key in metadata:
            raise ValueError(f"{path.name}:{line_number}: duplicate metadata key {key}")

        if key in LIST_METADATA:
            if raw_value in {None, ""}:
                metadata[key] = []
            elif raw_value == "[]":
                metadata[key] = []
            else:
                raise ValueError(
                    f"{path.name}:{line_number}: {key} must use a block list or []"
                )
            active_list = key
        else:
            if raw_value in {None, ""}:
                raise ValueError(f"{path.name}:{line_number}: {key} requires a value")
            metadata[key] = raw_value
            active_list = None

    keys = set(metadata)
    if keys != REQUIRED_METADATA:
        missing = ", ".join(sorted(REQUIRED_METADATA - keys)) or "none"
        unexpected = ", ".join(sorted(keys - REQUIRED_METADATA)) or "none"
        raise ValueError(
            f"{path.name}: invalid metadata fields; missing={missing}; unexpected={unexpected}"
        )

    if metadata["schema"] != "aether.architecture-document/v1":
        raise ValueError(f"{path.name}: unsupported architecture document schema")
    if metadata["kind"] != "architecture-document":
        raise ValueError(f"{path.name}: unsupported document kind")

    return metadata


def require_list(metadata: dict[str, str | list[str]], key: str) -> tuple[str, ...]:
    value = metadata[key]
    if not isinstance(value, list):
        raise ValueError(f"{key} must be a list")
    return tuple(value)


def require_scalar(metadata: dict[str, str | list[str]], key: str) -> str:
    value = metadata[key]
    if not isinstance(value, str):
        raise ValueError(f"{key} must be a scalar")
    return value


def load_documents(repository_root: Path) -> list[ArchitectureDocument]:
    documents: list[ArchitectureDocument] = []
    seen_ids: set[str] = set()

    for relative_path in CANONICAL_DOCUMENTS:
        path = repository_root / relative_path
        if not path.is_file():
            raise ValueError(f"missing canonical architecture document: {relative_path}")

        metadata = parse_frontmatter(path)
        document_id = require_scalar(metadata, "id")
        if document_id in seen_ids:
            raise ValueError(f"duplicate architecture document id: {document_id}")
        seen_ids.add(document_id)

        documents.append(
            ArchitectureDocument(
                path=relative_path,
                document_id=document_id,
                title=require_scalar(metadata, "title"),
                version=require_scalar(metadata, "version"),
                status=require_scalar(metadata, "status"),
                owners=require_list(metadata, "owners"),
                governed_by=require_list(metadata, "governed_by"),
                depends_on=require_list(metadata, "depends_on"),
                related=require_list(metadata, "related"),
                supersedes=require_list(metadata, "supersedes"),
            )
        )

    return documents


def validate_graph(documents: list[ArchitectureDocument]) -> dict[str, int]:
    by_id = {document.document_id: document for document in documents}

    for document in documents:
        for relationship, references in (
            ("depends_on", document.depends_on),
            ("related", document.related),
            ("supersedes", document.supersedes),
        ):
            for reference in references:
                if reference not in by_id:
                    raise ValueError(
                        f"{document.path}: unresolved {relationship} reference {reference}"
                    )

    visiting: set[str] = set()
    visited: set[str] = set()
    ranks: dict[str, int] = {}

    def visit(document_id: str) -> int:
        if document_id in visiting:
            raise ValueError(f"architecture dependency cycle contains {document_id}")
        if document_id in visited:
            return ranks[document_id]

        visiting.add(document_id)
        dependencies = by_id[document_id].depends_on
        rank = 0 if not dependencies else max(visit(dependency) for dependency in dependencies) + 1
        visiting.remove(document_id)
        visited.add(document_id)
        ranks[document_id] = rank
        return rank

    for document in documents:
        visit(document.document_id)

    return ranks


def load_config(path: Path, document_ids: set[str]) -> dict[str, object]:
    config = json.loads(path.read_text(encoding="utf-8"))
    if config.get("schema") != "optiflow.architecture-portal-config.v1":
        raise ValueError("unsupported architecture portal configuration schema")

    expected_top_level = {
        "$schema",
        "schema",
        "repository",
        "product",
        "categories",
        "runtimeLayers",
        "platformBoundaries",
    }
    if set(config) != expected_top_level:
        raise ValueError("architecture portal configuration has unexpected top-level fields")

    categories = config["categories"]
    if not isinstance(categories, list) or not categories:
        raise ValueError("architecture portal requires at least one category")

    assigned: list[str] = []
    category_ids: set[str] = set()
    for category in categories:
        if not isinstance(category, dict):
            raise ValueError("architecture category must be an object")
        category_id = category.get("id")
        if not isinstance(category_id, str) or not category_id:
            raise ValueError("architecture category requires an id")
        if category_id in category_ids:
            raise ValueError(f"duplicate architecture category id: {category_id}")
        category_ids.add(category_id)
        document_group = category.get("documentIds")
        if not isinstance(document_group, list):
            raise ValueError(f"architecture category {category_id} requires documentIds")
        assigned.extend(document_group)

    if len(assigned) != len(set(assigned)):
        raise ValueError("an architecture document is assigned to multiple categories")
    if set(assigned) != document_ids:
        missing = ", ".join(sorted(document_ids - set(assigned))) or "none"
        unexpected = ", ".join(sorted(set(assigned) - document_ids)) or "none"
        raise ValueError(
            f"architecture categories do not cover the corpus; missing={missing}; "
            f"unexpected={unexpected}"
        )

    for collection_name in ("runtimeLayers", "platformBoundaries"):
        collection = config[collection_name]
        if not isinstance(collection, list) or not collection:
            raise ValueError(f"architecture portal requires {collection_name}")
        ids = [item.get("id") for item in collection if isinstance(item, dict)]
        if len(ids) != len(collection) or any(not isinstance(item, str) for item in ids):
            raise ValueError(f"every {collection_name} item requires an id")
        if len(ids) != len(set(ids)):
            raise ValueError(f"duplicate id in {collection_name}")

    return config


def source_fingerprint(
    repository_root: Path,
    config_path: Path,
) -> str:
    digest = hashlib.sha256()
    digest.update(config_path.read_bytes())
    for relative_path in CANONICAL_DOCUMENTS:
        digest.update(relative_path.encode("utf-8"))
        digest.update((repository_root / relative_path).read_bytes())
    return digest.hexdigest()


def document_category_map(config: dict[str, object]) -> dict[str, str]:
    mapping: dict[str, str] = {}
    for category in config["categories"]:
        for document_id in category["documentIds"]:
            mapping[document_id] = category["id"]
    return mapping


def source_url(config: dict[str, object], path: str) -> str:
    repository = config["repository"]
    return (
        f"{repository['sourceUrl']}/blob/{quote(repository['revision'], safe='')}/"
        f"{quote(path)}"
    )


def build_dataset(
    documents: list[ArchitectureDocument],
    ranks: dict[str, int],
    config: dict[str, object],
    fingerprint: str,
) -> dict[str, object]:
    categories_by_document = document_category_map(config)
    edges = [
        {"from": dependency, "to": document.document_id}
        for document in documents
        for dependency in document.depends_on
    ]

    return {
        "$schema": "/schemas/architecture-portal-v1.schema.json",
        "schema": "optiflow.architecture-portal.v1",
        "sourceFingerprint": fingerprint,
        "repository": config["repository"],
        "product": config["product"],
        "stats": {
            "documents": len(documents),
            "categories": len(config["categories"]),
            "dependencies": len(edges),
            "layers": len(config["runtimeLayers"]),
        },
        "categories": config["categories"],
        "runtimeLayers": config["runtimeLayers"],
        "platformBoundaries": config["platformBoundaries"],
        "documents": [
            {
                "id": document.document_id,
                "title": document.title,
                "shortTitle": document.title.removeprefix("OptiFlow "),
                "path": document.path,
                "category": categories_by_document[document.document_id],
                "version": document.version,
                "status": document.status,
                "owners": list(document.owners),
                "governedBy": list(document.governed_by),
                "dependsOn": list(document.depends_on),
                "related": list(document.related),
                "supersedes": list(document.supersedes),
                "rank": ranks[document.document_id],
                "sourceUrl": source_url(config, document.path),
            }
            for document in documents
        ],
        "edges": edges,
    }


def render_graph(dataset: dict[str, object]) -> str:
    documents = dataset["documents"]
    by_id = {document["id"]: document for document in documents}
    ranks: dict[int, list[dict[str, object]]] = {}
    for document in documents:
        ranks.setdefault(document["rank"], []).append(document)

    for rank_documents in ranks.values():
        rank_documents.sort(key=lambda document: document["shortTitle"])

    node_width = 184
    node_height = 70
    horizontal_gap = 64
    vertical_gap = 24
    padding = 32
    column_width = node_width + horizontal_gap
    maximum_rows = max(len(rank_documents) for rank_documents in ranks.values())
    width = padding * 2 + len(ranks) * node_width + (len(ranks) - 1) * horizontal_gap
    height = padding * 2 + maximum_rows * node_height + (maximum_rows - 1) * vertical_gap

    coordinates: dict[str, tuple[float, float]] = {}
    for rank, rank_documents in sorted(ranks.items()):
        column_height = len(rank_documents) * node_height + (len(rank_documents) - 1) * vertical_gap
        y_offset = padding + (height - padding * 2 - column_height) / 2
        for index, document in enumerate(rank_documents):
            coordinates[document["id"]] = (
                padding + rank * column_width,
                y_offset + index * (node_height + vertical_gap),
            )

    parts = [
        f'<svg class="architecture-graph" viewBox="0 0 {width} {height}" '
        'role="img" aria-labelledby="graph-title graph-description">',
        '<title id="graph-title">OptiFlow architecture document dependency graph</title>',
        '<desc id="graph-description">Dependencies flow from left to right. '
        'Each node links to its document card below.</desc>',
        '<defs><marker id="arrow" viewBox="0 0 10 10" refX="8" refY="5" '
        'markerWidth="5" markerHeight="5" orient="auto-start-reverse">'
        '<path d="M 0 0 L 10 5 L 0 10 z"></path></marker></defs>',
        '<g class="graph-edges" aria-hidden="true">',
    ]

    for edge in dataset["edges"]:
        start_x, start_y = coordinates[edge["from"]]
        end_x, end_y = coordinates[edge["to"]]
        x1 = start_x + node_width
        y1 = start_y + node_height / 2
        x2 = end_x
        y2 = end_y + node_height / 2
        control = max(28, (x2 - x1) * 0.45)
        parts.append(
            f'<path data-from="{html.escape(edge["from"])}" '
            f'data-to="{html.escape(edge["to"])}" '
            f'd="M {x1:.1f} {y1:.1f} C {x1 + control:.1f} {y1:.1f}, '
            f'{x2 - control:.1f} {y2:.1f}, {x2:.1f} {y2:.1f}"></path>'
        )

    parts.append("</g><g class=\"graph-nodes\">")
    for document in documents:
        x, y = coordinates[document["id"]]
        title = html.escape(document["shortTitle"])
        metadata = html.escape(f"v{document['version']} · {document['status']}")
        document_id = html.escape(document["id"])
        category = html.escape(document["category"])
        parts.extend(
            [
                f'<a class="graph-node category-{category}" '
                f'data-node-id="{document_id}" href="#document-{document_id}" '
                f'aria-label="Open {title} document details">',
                f'<rect x="{x:.1f}" y="{y:.1f}" width="{node_width}" '
                f'height="{node_height}" rx="12"></rect>',
                f'<text class="node-title" x="{x + 14:.1f}" y="{y + 29:.1f}">{title}</text>',
                f'<text class="node-meta" x="{x + 14:.1f}" y="{y + 50:.1f}">{metadata}</text>',
                "</a>",
            ]
        )

    parts.append("</g></svg>")
    return "".join(parts)


def render_category_filters(dataset: dict[str, object]) -> str:
    buttons = [
        '<button type="button" class="filter-button is-active" data-filter="all" '
        'aria-pressed="true">All documents <span>18</span></button>'
    ]
    for category in dataset["categories"]:
        buttons.append(
            f'<button type="button" class="filter-button" data-filter="{html.escape(category["id"])}" '
            f'aria-pressed="false">{html.escape(category["label"])} '
            f'<span>{len(category["documentIds"])}</span></button>'
        )
    return "".join(buttons)


def render_document_cards(dataset: dict[str, object]) -> str:
    category_by_id = {category["id"]: category for category in dataset["categories"]}
    cards: list[str] = []

    for document in dataset["documents"]:
        category = category_by_id[document["category"]]
        dependencies = document["dependsOn"]
        dependency_markup = (
            "".join(
                f'<a href="#document-{html.escape(dependency)}">'
                f'{html.escape(dependency.removeprefix("optiflow-").replace("-", " "))}</a>'
                for dependency in dependencies
            )
            if dependencies
            else '<span class="root-document">root document</span>'
        )
        cards.append(
            f'<article class="document-card category-{html.escape(document["category"])}" '
            f'id="document-{html.escape(document["id"])}" '
            f'data-category="{html.escape(document["category"])}">'
            '<div class="document-card-top">'
            f'<span class="category-label">{html.escape(category["label"])}</span>'
            f'<span class="document-version">v{html.escape(document["version"])}</span>'
            "</div>"
            f'<h3>{html.escape(document["shortTitle"])}</h3>'
            f'<p>{html.escape(category["question"])}</p>'
            '<div class="dependency-list"><span>Depends on</span>'
            f'<div>{dependency_markup}</div></div>'
            '<div class="document-card-footer">'
            f'<code>{html.escape(document["id"])}</code>'
            f'<a href="{html.escape(document["sourceUrl"])}">Read source <span aria-hidden="true">↗</span></a>'
            "</div></article>"
        )

    return "".join(cards)


def render_runtime_layers(dataset: dict[str, object]) -> str:
    layers: list[str] = []
    for index, layer in enumerate(dataset["runtimeLayers"], start=1):
        components = "".join(
            f"<code>{html.escape(component)}</code>" for component in layer["components"]
        )
        layers.append(
            f'<article class="layer-card" data-state="{html.escape(layer["state"])}">'
            f'<span class="layer-index">{index:02d}</span>'
            '<div class="layer-copy">'
            f'<div><span class="layer-state">{html.escape(layer["state"])}</span>'
            f'<h3>{html.escape(layer["label"])}</h3></div>'
            f'<p>{html.escape(layer["summary"])}</p>'
            f'<div class="component-list">{components}</div>'
            "</div></article>"
        )
    return "".join(layers)


def render_boundaries(dataset: dict[str, object]) -> str:
    cards: list[str] = []
    for boundary in dataset["platformBoundaries"]:
        items = "".join(f"<li>{html.escape(item)}</li>" for item in boundary["items"])
        cards.append(
            f'<article class="boundary-card boundary-{html.escape(boundary["id"])}">'
            f'<span class="boundary-kicker">{html.escape(boundary["eyebrow"])}</span>'
            f'<h3>{html.escape(boundary["label"])}</h3>'
            f'<p>{html.escape(boundary["summary"])}</p>'
            f'<ul>{items}</ul></article>'
        )
    return "".join(cards)


def render_html(dataset: dict[str, object]) -> str:
    product = dataset["product"]
    stats = dataset["stats"]
    fingerprint = dataset["sourceFingerprint"][:12]
    graph = render_graph(dataset)

    return f"""<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <meta name="theme-color" content="#080b10">
    <meta name="description" content="{html.escape(product['description'])}">
    <meta property="og:type" content="website">
    <meta property="og:site_name" content="optiflow">
    <meta property="og:title" content="OptiFlow architecture">
    <meta property="og:description" content="{html.escape(product['description'])}">
    <meta property="og:url" content="https://optiflow.egohygiene.io/architecture/">
    <title>architecture · optiflow</title>
    <link rel="icon" href="/assets/mark.svg" type="image/svg+xml">
    <link rel="stylesheet" href="assets/architecture.css">
    <script src="assets/architecture.js" defer></script>
  </head>
  <body>
    <a class="skip-link" href="#main">Skip to architecture</a>
    <div class="ambient ambient-one" aria-hidden="true"></div>
    <div class="ambient ambient-two" aria-hidden="true"></div>

    <header class="site-header">
      <div class="shell navigation">
        <a class="brand" href="/" aria-label="OptiFlow home">
          <img src="/assets/mark.svg" width="28" height="28" alt="">
          <span>optiflow</span>
          <span class="brand-surface">architecture</span>
        </a>
        <nav aria-label="Primary">
          <a href="#system">System</a>
          <a href="#graph">Graph</a>
          <a href="#documents">Documents</a>
          <a href="/docs/">Docs</a>
          <a href="https://github.com/egohygiene/optiflow">GitHub</a>
        </nav>
      </div>
    </header>

    <main id="main">
      <section class="hero shell">
        <div class="hero-copy">
          <p class="eyebrow"><span></span>{html.escape(product['eyebrow'])}</p>
          <h1>{html.escape(product['title'])}</h1>
          <p class="hero-summary">{html.escape(product['summary'])}</p>
          <div class="hero-actions">
            <a class="button button-primary" href="#graph">Explore the graph <span aria-hidden="true">↓</span></a>
            <a class="button" href="architecture.json">Download architecture JSON</a>
          </div>
          <p class="source-note">Generated from repository truth · fingerprint <code>{fingerprint}</code></p>
        </div>
        <div class="architecture-console" aria-label="Architecture corpus summary">
          <div class="console-bar"><span></span><span></span><span></span><code>architecture / v1</code></div>
          <dl>
            <div><dt>canonical documents</dt><dd>{stats['documents']:02d}</dd></div>
            <div><dt>knowledge categories</dt><dd>{stats['categories']:02d}</dd></div>
            <div><dt>dependency edges</dt><dd>{stats['dependencies']:02d}</dd></div>
            <div><dt>structural layers</dt><dd>{stats['layers']:02d}</dd></div>
          </dl>
          <div class="console-state"><span></span> graph resolved · no cycles</div>
        </div>
      </section>

      <section class="principle-strip" aria-label="Architecture principles">
        <div class="shell">
          <p><span>01</span>Repository truth is canonical</p>
          <p><span>02</span>Dependencies remain explicit</p>
          <p><span>03</span>Generated views are disposable</p>
        </div>
      </section>

      <section id="system" class="section shell">
        <div class="section-heading">
          <div><p class="kicker">System boundary</p><h2>One product, explicit authority.</h2></div>
          <p>OptiFlow owns evidence and reviewable plans. Platform capabilities compose around it without leaking cloud or orchestration concerns into the domain.</p>
        </div>
        <div class="boundary-grid">{render_boundaries(dataset)}</div>
      </section>

      <section class="section shell layer-section">
        <div class="section-heading">
          <div><p class="kicker">Structural model</p><h2>Dependencies point toward meaning.</h2></div>
          <p>Infrastructure implements explicit boundaries. Interfaces coordinate application use cases. Domain and evidence rules remain independent from deployment technology.</p>
        </div>
        <div class="layer-list">{render_runtime_layers(dataset)}</div>
      </section>

      <section id="graph" class="section graph-section">
        <div class="shell">
          <div class="section-heading graph-heading">
            <div><p class="kicker">Architecture knowledge graph</p><h2>Every decision has context.</h2></div>
            <p>The graph is generated from each document's <code>depends_on</code> metadata. Select a node to jump to its source contract.</p>
          </div>
          <div class="graph-frame">
            <div class="graph-toolbar">
              <span>dependency direction</span>
              <span class="direction">foundations <i aria-hidden="true">→</i> decisions</span>
            </div>
            <div class="graph-scroll" tabindex="0" aria-label="Scrollable architecture dependency graph">{graph}</div>
          </div>
        </div>
      </section>

      <section id="documents" class="section shell document-section">
        <div class="section-heading">
          <div><p class="kicker">Canonical corpus</p><h2>Eighteen documents. One connected system.</h2></div>
          <p>Each document owns one kind of meaning. Filters change this projection only; the repository metadata remains the source of truth.</p>
        </div>
        <div class="filter-row" role="group" aria-label="Filter architecture documents">{render_category_filters(dataset)}</div>
        <div class="document-grid" aria-live="polite">{render_document_cards(dataset)}</div>
      </section>

      <section class="section shell generation-section">
        <div class="generation-panel">
          <div>
            <p class="kicker">Reusable publication contract</p>
            <h2>The page is an output, not another authority.</h2>
            <p>The build reads canonical frontmatter, resolves the dependency graph, combines repository-owned presentation configuration, and produces replaceable HTML and JSON projections.</p>
          </div>
          <ol>
            <li><span>01</span>Read canonical documents</li>
            <li><span>02</span>Resolve and validate relationships</li>
            <li><span>03</span>Project graph, layers, and inventory</li>
            <li><span>04</span>Compose under <code>/architecture/</code></li>
          </ol>
          <div class="generation-links">
            <a href="/docs/architecture/">Runtime architecture</a>
            <a href="/docs/cloud-native-placement/">Cloud-native placement</a>
            <a href="https://github.com/egohygiene/optiflow/blob/main/META.md">Meta architecture</a>
          </div>
        </div>
      </section>
    </main>

    <footer class="site-footer">
      <div class="shell footer-grid">
        <div><a class="brand" href="/"><img src="/assets/mark.svg" width="24" height="24" alt=""><span>optiflow</span></a><p>Evidence-first media understanding.</p></div>
        <nav aria-label="Footer"><a href="/">Product</a><a href="/docs/">Documentation</a><a href="architecture.json">Architecture JSON</a><a href="https://github.com/egohygiene/optiflow">Source</a></nav>
        <p>Generated deterministically from <code>aether.architecture-document/v1</code>.</p>
      </div>
    </footer>
  </body>
</html>
"""


def main() -> int:
    arguments = parse_arguments()
    repository_root = arguments.repository_root.resolve()
    config_path = arguments.config.resolve()
    output = arguments.output.resolve()

    documents = load_documents(repository_root)
    ranks = validate_graph(documents)
    config = load_config(config_path, {document.document_id for document in documents})
    fingerprint = source_fingerprint(repository_root, config_path)
    dataset = build_dataset(documents, ranks, config, fingerprint)

    output.mkdir(parents=True, exist_ok=True)
    (output / "architecture.json").write_text(
        json.dumps(dataset, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    (output / "index.html").write_text(render_html(dataset), encoding="utf-8")

    print(
        f"generated architecture portal from {len(documents)} documents, "
        f"{len(dataset['edges'])} dependency edges, and fingerprint {fingerprint[:12]}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
