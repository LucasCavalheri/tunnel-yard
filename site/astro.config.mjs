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
        "simple-icons": [
          "almalinux",
          "alpinelinux",
          "archlinux",
          "debian",
          "elementary",
          "fedora",
          "gentoo",
          "github",
          "kalilinux",
          "linux",
          "linuxmint",
          "manjaro",
          "nixos",
          "opensuse",
          "popos",
          "raspberrypi",
          "redhat",
          "rockylinux",
          "rust",
          "ubuntu",
          "voidlinux",
          "zorin"
        ],
        "circle-flags": ["br", "us"]
      }
    })
  ],
  build: {
    format: "directory",
    // Keep hashed JS under /assets (not /_astro): Vercel sends
    // Cache-Control: immutable on 404s for that prefix, so a deploy race
    // sticks in the browser for a year. CSS is inlined so the landing page
    // cannot render as unstyled markup when a stylesheet 404s.
    assets: "assets",
    inlineStylesheets: "always"
  }
});
