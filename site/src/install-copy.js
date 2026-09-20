export const INSTALL_COMMAND =
  "curl -fsSL https://tunnelyard.lucascavalheri.com.br/install.sh | bash";

export function copyButtonLabel(copied, labels) {
  return copied ? labels.copied : labels.copy;
}

export async function copyText(text, clipboard) {
  const write =
    clipboard?.writeText?.bind(clipboard) ||
    (typeof navigator !== "undefined" ? navigator.clipboard?.writeText?.bind(navigator.clipboard) : null);
  if (!write) throw new Error("clipboard unavailable");
  await write(text);
  return text;
}

export function bindInstallCopy(root = document) {
  root.querySelectorAll("[data-install-copy]").forEach((button) => {
    if (button.dataset.bound === "true") return;
    button.dataset.bound = "true";
    const command = button.dataset.command || INSTALL_COMMAND;
    const idle = button.dataset.labelCopy || "Copy";
    const copied = button.dataset.labelCopied || "Copied";
    const label = button.querySelector("[data-copy-label]");
    let timer = 0;
    const setCopied = (on) => {
      button.classList.toggle("is-copied", on);
      button.setAttribute("aria-pressed", on ? "true" : "false");
      if (label) label.textContent = copyButtonLabel(on, { copy: idle, copied });
    };
    button.addEventListener("click", async () => {
      try {
        await copyText(command);
        setCopied(true);
        window.clearTimeout(timer);
        timer = window.setTimeout(() => setCopied(false), 2200);
      } catch {
        const slot = root.querySelector("[data-install-command]");
        if (slot) {
          const range = document.createRange();
          range.selectNodeContents(slot);
          const selection = window.getSelection();
          selection?.removeAllRanges();
          selection?.addRange(range);
        }
      }
    });
  });
}
