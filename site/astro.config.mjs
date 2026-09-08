import { defineConfig } from "astro/config";
import icon from "astro-icon";

export default defineConfig({
  site: "https://tunnelyard.lucascavalheri.com.br",
  output: "static",
  publicDir: "../public",
  i18n: {
    defaultLocale: "en",
    locales: ["en", "pt-BR"],
    routing: {
      // English owns the bare "/" route; Portuguese lives under "/pt-br/".
      prefixDefaultLocale: false
    }
  },
  integrations: [
    icon({
      include: {
        "simple-icons": ["apple", "debian", "fedora", "github", "linux", "redhat", "rust", "ubuntu", "windows"],
        "circle-flags": ["br", "us"]
      }
    })
  ],
  build: {
    format: "directory"
  }
});
