import { defineConfig } from "astro/config";
import icon from "astro-icon";

export default defineConfig({
  output: "static",
  publicDir: "../public",
  integrations: [
    icon({
      include: {
        "simple-icons": ["apple", "debian", "fedora", "github", "linux", "redhat", "rust", "ubuntu", "windows"]
      }
    })
  ],
  build: {
    format: "directory"
  }
});
