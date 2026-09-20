// Resolve at build time: never advertise an unpublished version or invent asset URLs.
export const repoUrl = "https://github.com/LucasCavalheri/tunnel-yard";
interface Release {
  tag_name: string;
  html_url: string;
  assets: { name: string; browser_download_url: string }[];
}
const response = await fetch("https://api.github.com/repos/LucasCavalheri/tunnel-yard/releases/latest", {
  headers: { Accept: "application/vnd.github+json" },
  signal: AbortSignal.timeout(15000),
});
if (!response.ok) throw new Error(`Não foi possível confirmar os downloads: GitHub HTTP ${response.status}. Tente o build novamente.`);
export const release: Release = await response.json();
if (!release.tag_name || !Array.isArray(release.assets) || !release.html_url?.startsWith(`${repoUrl}/releases/`)) {
  throw new Error("Resposta de release inválida.");
}
for (const asset of release.assets) {
  if (!asset.browser_download_url?.startsWith(`${repoUrl}/releases/download/`)) throw new Error("URL de instalador inválida.");
}

const repoResponse = await fetch("https://api.github.com/repos/LucasCavalheri/tunnel-yard", {
  headers: { Accept: "application/vnd.github+json" },
  signal: AbortSignal.timeout(15000),
});
if (!repoResponse.ok) {
  throw new Error(`Não foi possível ler o repositório: GitHub HTTP ${repoResponse.status}.`);
}
const repo = (await repoResponse.json()) as { stargazers_count?: unknown };
if (typeof repo.stargazers_count !== "number" || !Number.isFinite(repo.stargazers_count) || repo.stargazers_count < 0) {
  throw new Error("Contagem de estrelas inválida.");
}
export const starCount: number = Math.round(repo.stargazers_count);
