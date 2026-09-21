const defaultRepoApiUrl = "https://api.github.com/repos/LucasCavalheri/tunnel-yard";

export const validateRelease = (value, repoUrl) => {
  if (!value?.tag_name || !Array.isArray(value.assets) || !value.html_url?.startsWith(`${repoUrl}/releases/`)) {
    throw new Error("Resposta de release inválida.");
  }

  for (const asset of value.assets) {
    if (!asset.browser_download_url?.startsWith(`${repoUrl}/releases/download/`)) {
      throw new Error("URL de instalador inválida.");
    }
  }

  return value;
};

const parseStarCount = (value) => {
  if (typeof value !== "number" || !Number.isFinite(value) || value < 0) return 0;
  return Math.round(value);
};

export const fetchStarCount = async (fetchImpl, repoApiUrl = defaultRepoApiUrl) => {
  try {
    const response = await fetchImpl(repoApiUrl, {
      headers: { Accept: "application/vnd.github+json" },
      signal: AbortSignal.timeout(15000),
    });
    if (!response.ok) return 0;

    const repo = await response.json();
    return parseStarCount(repo?.stargazers_count);
  } catch {
    return 0;
  }
};
