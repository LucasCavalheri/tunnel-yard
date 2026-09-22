import assert from "node:assert/strict";
import test from "node:test";
import { fetchStarCount, validateRelease } from "../src/data/release-utils.mjs";

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

test("release validation still rejects an untrusted download URL", () => {
  assert.throws(
    () =>
      validateRelease(
        {
          tag_name: "v3.0.5",
          html_url: `${repoUrl}/releases/tag/v3.0.5`,
          assets: [{ name: "installer", browser_download_url: "https://example.com/installer" }],
        },
        repoUrl,
      ),
    /URL de instalador inválida/,
  );
});
