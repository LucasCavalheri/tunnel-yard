export function fileNameFromUrl(url) {
  if (!url) return "";
  try {
    return decodeURIComponent(String(url).split("/").pop() || "");
  } catch {
    return String(url);
  }
}

export function downloadButtonState(url, labels) {
  const enabled = Boolean(url);
  return {
    enabled,
    buttonLabel: enabled ? labels.ready : labels.idle,
    fileLabel: enabled ? fileNameFromUrl(url) : labels.hint,
  };
}

export function moveActiveIndex(current, delta, count) {
  if (count <= 0) return 0;
  if (current < 0) return delta > 0 ? 0 : count - 1;
  return (current + delta + count * 10) % count;
}

export function applyDownloadChoice(card, url) {
  const link = card.querySelector("[data-file-download]");
  if (!link) return downloadButtonState(url, { idle: "", ready: "", hint: "" });
  const state = downloadButtonState(url, {
    idle: link.dataset.labelIdle || "",
    ready: link.dataset.labelReady || "",
    hint: link.dataset.fileHint || "",
  });
  const label = link.querySelector("span");
  const fileName = card.querySelector("[data-file-name]");
  if (state.enabled) {
    link.href = url;
    link.removeAttribute("aria-disabled");
    link.removeAttribute("tabindex");
  } else {
    link.removeAttribute("href");
    link.setAttribute("aria-disabled", "true");
    link.tabIndex = -1;
  }
  if (label) label.textContent = state.buttonLabel;
  if (fileName) fileName.textContent = state.fileLabel;
  return state;
}

function closePicker(picker) {
  const trigger = picker.querySelector(".picker-trigger");
  const menu = picker.querySelector(".picker-menu");
  picker.classList.remove("is-open");
  trigger?.setAttribute("aria-expanded", "false");
  if (menu) menu.hidden = true;
}

function openPicker(picker) {
  document.querySelectorAll("[data-download-picker].is-open").forEach((open) => {
    if (open !== picker) closePicker(open);
  });
  const trigger = picker.querySelector(".picker-trigger");
  const menu = picker.querySelector(".picker-menu");
  picker.classList.add("is-open");
  trigger?.setAttribute("aria-expanded", "true");
  if (menu) menu.hidden = false;
  const options = [...(menu?.querySelectorAll('[role="option"]') ?? [])];
  const selected = options.find((option) => option.getAttribute("aria-selected") === "true");
  options.forEach((option) => option.classList.toggle("is-active", option === (selected || options[0])));
}

function setActiveOption(options, index) {
  options.forEach((option, i) => option.classList.toggle("is-active", i === index));
  options[index]?.scrollIntoView({ block: "nearest" });
}

export function bindDownloadCard(card) {
  const picker = card.querySelector("[data-download-picker]");
  if (!picker || picker.dataset.bound === "true") return;
  picker.dataset.bound = "true";

  const trigger = picker.querySelector(".picker-trigger");
  const menu = picker.querySelector(".picker-menu");
  const valueLabel = picker.querySelector("[data-picker-label]");
  const options = [...(menu?.querySelectorAll('[role="option"]') ?? [])];
  if (!trigger || !menu) return;

  const select = (option) => {
    const url = option?.getAttribute("data-value") || "";
    options.forEach((item) => item.setAttribute("aria-selected", item === option ? "true" : "false"));
    if (valueLabel && option) valueLabel.textContent = option.textContent.trim();
    applyDownloadChoice(card, url);
    closePicker(picker);
    trigger.focus();
  };

  trigger.addEventListener("click", () => {
    if (picker.classList.contains("is-open")) closePicker(picker);
    else openPicker(picker);
  });

  options.forEach((option) => {
    option.addEventListener("click", () => select(option));
  });

  trigger.addEventListener("keydown", (event) => {
    const open = picker.classList.contains("is-open");
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      if (!open) openPicker(picker);
      const current = options.findIndex((option) => option.classList.contains("is-active"));
      const next = moveActiveIndex(current, event.key === "ArrowDown" ? 1 : -1, options.length);
      setActiveOption(options, next);
    } else if (event.key === "Enter" || event.key === " ") {
      if (open) {
        event.preventDefault();
        const active = options.find((option) => option.classList.contains("is-active")) || options[0];
        select(active);
      }
    } else if (event.key === "Escape" && open) {
      event.preventDefault();
      closePicker(picker);
    }
  });

  applyDownloadChoice(card, "");
}

export function bindDownloadPickers(root = document) {
  root.querySelectorAll("[data-download-card]").forEach((card) => bindDownloadCard(card));
}

export function closeDownloadPickersOnOutside(event, root = document) {
  root.querySelectorAll("[data-download-picker].is-open").forEach((picker) => {
    if (!picker.contains(event.target)) closePicker(picker);
  });
}
