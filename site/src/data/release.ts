// Resolve at build time: never advertise an unpublished version or invent asset URLs.
import { fetchLatestRelease, fetchStarCount } from "./release-utils.mjs";

export const repoUrl = "https://github.com/LucasCavalheri/tunnel-yard";
const buildToken = (globalThis as typeof globalThis & {
  process?: { env?: Record<string, string | undefined> };
}).process?.env?.GITHUB_TOKEN;
interface Release {
  tag_name: string;
  html_url: string;
  assets: { name: string; browser_download_url: string }[];
}
export const release: Release = await fetchLatestRelease(fetch, buildToken);

// The star count is decorative. Do not make a successful site build depend on
// the unauthenticated GitHub API rate limit, especially for concurrent PRs.
export const starCount = await fetchStarCount(fetch);
