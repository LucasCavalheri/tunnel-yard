import assert from "node:assert/strict";
import test from "node:test";
import { fetchLatestRelease, fetchStarCount, validateRelease } from "../src/data/release-utils.mjs";

const repoUrl = "https://github.com/LucasCavalheri/tunnel-yard";

test("star count falls back to zero when GitHub returns an error", async () => {
  const count = await fetchStarCount(async () => new Response("rate limited", { status: 403 }));
  assert.equal(count, 0);
});

test("star count accepts a valid API response", async () => {
  const count = await fetchStarCount(async () =>
    new Response(JSON.stringify({ stargazers_count: 12.6 }), { status: 200 }),
  );
  assert.equal(count, 13);
});

test("release fetch authenticates with the optional build token", async () => {
  let request;
  const release = await fetchLatestRelease(async (url, options) => {
    request = { url, options };
    return new Response(
      JSON.stringify({
        tag_name: "v3.0.6",
        html_url: `${repoUrl}/releases/tag/v3.0.6`,
        assets: [{ name: "installer", browser_download_url: `${repoUrl}/releases/download/v3.0.6/installer` }],
      }),
      { status: 200 },
    );
  }, "read-only-build-token");

  assert.equal(request.url, "https://api.github.com/repos/LucasCavalheri/tunnel-yard/releases/latest");
  assert.equal(request.options.headers.Authorization, "Bearer read-only-build-token");
  assert.equal(release.tag_name, "v3.0.6");
});

test("release fetch still reports rate limit errors", async () => {
  await assert.rejects(
    fetchLatestRelease(async () => new Response("rate limited", { status: 403 }), "read-only-build-token"),
    /GitHub HTTP 403/,
  );
});

test("release validation still rejects an untrusted download URL", () => {
  assert.throws(
    () =>
      validateRelease(
        {
          tag_name: "v3.0.6",
          html_url: `${repoUrl}/releases/tag/v3.0.6`,
          assets: [{ name: "installer", browser_download_url: "https://example.com/installer" }],
        },
        repoUrl,
      ),
    /URL de instalador inválida/,
  );
});
