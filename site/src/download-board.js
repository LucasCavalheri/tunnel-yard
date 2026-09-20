import { applyDownloadChoice } from "./download-picker.js";

const PACKAGES = {
  deb: { x64: "_amd64.deb", arm64: "_arm64.deb" },
  rpm: { x64: ".x86_64.rpm", arm64: ".aarch64.rpm" },
  arch: { x64: "-x86_64.pkg.tar.zst", arm64: "-aarch64.pkg.tar.zst" },
  apk: { x64: "-x86_64.apk", arm64: "-aarch64.apk" },
  tar: { x64: "-linux-x64.tar.gz", arm64: "-linux-arm64.tar.gz" }
};

export function packageSuffix(family, arch) {
  return PACKAGES[family]?.[arch] || "";
}

export function urlsFromBoard(board) {
  return {
    "deb-x64": board.dataset.urlDebX64 || "",
    "deb-arm64": board.dataset.urlDebArm64 || "",
    "rpm-x64": board.dataset.urlRpmX64 || "",
    "rpm-arm64": board.dataset.urlRpmArm64 || "",
    "arch-x64": board.dataset.urlArchX64 || "",
    "arch-arm64": board.dataset.urlArchArm64 || "",
    "apk-x64": board.dataset.urlApkX64 || "",
    "apk-arm64": board.dataset.urlApkArm64 || "",
    "tar-x64": board.dataset.urlTarX64 || "",
    "tar-arm64": board.dataset.urlTarArm64 || ""
  };
}

export function resolveDownload(family, arch, urls) {
  if (!family || !arch || !urls) return "";
  return urls[`${family}-${arch}`] || "";
}

export function bothPicksReady(family, arch) {
  return Boolean(family) && Boolean(arch);
}

export function selectedDownload(board) {
  const distro = board.querySelector('[data-distro][aria-checked="true"]');
  const arch = board.querySelector('[data-arch][aria-checked="true"]');
  return resolveDownload(
    distro?.dataset.family,
    arch?.dataset.arch,
    urlsFromBoard(board)
  );
}

function setRadio(group, selected) {
  group.forEach((item) => {
    const on = item === selected;
    item.setAttribute("aria-checked", on ? "true" : "false");
    item.classList.toggle("is-selected", on);
  });
}

function refresh(board) {
  const distro = board.querySelector('[data-distro][aria-checked="true"]');
  const arch = board.querySelector('[data-arch][aria-checked="true"]');
  const url = selectedDownload(board);
  applyDownloadChoice(board, url);
  const status = board.querySelector("[data-distro-status]");
  if (status) {
    const name = distro?.dataset.name || "";
    const pack = distro?.dataset.pack || "";
    const template = board.dataset.selectedHint || "{distro} · {package}";
    if (distro && arch) {
      status.textContent = template.replace("{distro}", name).replace("{package}", pack);
    } else if (distro) {
      status.textContent = board.dataset.pickArchHint || board.dataset.pickHint || "";
    } else if (arch) {
      status.textContent = board.dataset.pickDistroHint || board.dataset.pickHint || "";
    } else {
      status.textContent = board.dataset.pickHint || "";
    }
  }
}

export function bindDownloadBoard(root = document) {
  root.querySelectorAll("[data-download-board]").forEach((board) => {
    if (board.dataset.bound === "true") return;
    board.dataset.bound = "true";

    const distros = [...board.querySelectorAll("[data-distro]")];
    const arches = [...board.querySelectorAll("[data-arch]")];

    distros.forEach((tile, index) => {
      tile.addEventListener("click", () => {
        setRadio(distros, tile);
        refresh(board);
      });
      tile.addEventListener("keydown", (event) => {
        if (event.key !== "ArrowRight" && event.key !== "ArrowLeft") return;
        event.preventDefault();
        const delta = event.key === "ArrowRight" ? 1 : -1;
        const next = distros[(index + delta + distros.length) % distros.length];
        next.focus();
        setRadio(distros, next);
        refresh(board);
      });
    });

    arches.forEach((chip) => {
      chip.addEventListener("click", () => {
        setRadio(arches, chip);
        refresh(board);
      });
    });

    refresh(board);
  });
}
