function initializeMenu() {
  const toggle = document.querySelector("[data-menu-toggle]");
  const menu = document.querySelector("[data-menu]");

  if (!toggle || !menu) {
    return;
  }

  const closeMenu = () => {
    toggle.setAttribute("aria-expanded", "false");
    menu.classList.remove("is-open");
  };

  toggle.addEventListener("click", () => {
    const isOpen = toggle.getAttribute("aria-expanded") === "true";
    toggle.setAttribute("aria-expanded", String(!isOpen));
    menu.classList.toggle("is-open", !isOpen);
  });

  menu.addEventListener("click", (event) => {
    if (event.target.closest("a")) {
      closeMenu();
    }
  });

  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape") {
      closeMenu();
    }
  });
}

function initializeCopyButton() {
  const button = document.querySelector("[data-copy]");

  if (!button) {
    return;
  }

  button.addEventListener("click", async () => {
    const label = button.querySelector(".copy-label");

    try {
      await navigator.clipboard.writeText(button.dataset.copy);
      label.textContent = "Copied";
      window.setTimeout(() => {
        label.textContent = "Copy";
      }, 1800);
    } catch {
      label.textContent = "Select command";
    }
  });
}

document.addEventListener("DOMContentLoaded", () => {
  initializeMenu();
  initializeCopyButton();
});
