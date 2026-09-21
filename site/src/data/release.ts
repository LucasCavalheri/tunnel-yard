// Resolve at build time: never advertise an unpublished version or invent asset URLs.
import { fetchStarCount, validateRelease } from "./release-utils.mjs";

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
export const release: Release = validateRelease(await response.json(), repoUrl);

// The star count is decorative. Do not make a successful site build depend on
// the unauthenticated GitHub API rate limit, especially for concurrent PRs.
export const starCount = await fetchStarCount(fetch);
