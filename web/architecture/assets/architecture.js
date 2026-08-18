function initializeDocumentFilters() {
  const buttons = [...document.querySelectorAll("[data-filter]")];
  const cards = [...document.querySelectorAll("[data-category]")];
  const graphNodes = [...document.querySelectorAll("[data-node-id]")];

  if (!buttons.length || !cards.length) {
    return;
  }

  for (const button of buttons) {
    button.addEventListener("click", () => {
      const selected = button.dataset.filter;

      for (const candidate of buttons) {
        const active = candidate === button;
        candidate.classList.toggle("is-active", active);
        candidate.setAttribute("aria-pressed", String(active));
      }

      for (const card of cards) {
        card.hidden = selected !== "all" && card.dataset.category !== selected;
      }

      for (const node of graphNodes) {
        const category = [...node.classList]
          .find((className) => className.startsWith("category-"))
          ?.replace("category-", "");
        node.classList.toggle("is-filtered", selected !== "all" && category !== selected);
      }
    });
  }
}

function initializeGraphFocus() {
  const graph = document.querySelector(".architecture-graph");
  const nodes = [...document.querySelectorAll("[data-node-id]")];
  const edges = [...document.querySelectorAll("[data-from][data-to]")];

  if (!graph || !nodes.length) {
    return;
  }

  const clear = () => {
    graph.classList.remove("has-focus");
    for (const node of nodes) {
      node.classList.remove("is-connected", "is-focused");
    }
    for (const edge of edges) {
      edge.classList.remove("is-connected");
    }
  };

  const focus = (node) => {
    clear();
    const selected = node.dataset.nodeId;
    const connected = new Set([selected]);

    for (const edge of edges) {
      if (edge.dataset.from === selected || edge.dataset.to === selected) {
        edge.classList.add("is-connected");
        connected.add(edge.dataset.from);
        connected.add(edge.dataset.to);
      }
    }

    graph.classList.add("has-focus");
    node.classList.add("is-focused");
    for (const candidate of nodes) {
      if (connected.has(candidate.dataset.nodeId)) {
        candidate.classList.add("is-connected");
      }
    }
  };

  for (const node of nodes) {
    node.addEventListener("mouseenter", () => focus(node));
    node.addEventListener("focus", () => focus(node));
    node.addEventListener("mouseleave", clear);
    node.addEventListener("blur", clear);
  }
}

document.addEventListener("DOMContentLoaded", () => {
  initializeDocumentFilters();
  initializeGraphFocus();
});
