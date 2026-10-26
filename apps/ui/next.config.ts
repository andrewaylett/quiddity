import type { NextConfig } from "next";

// The UI ships inside Tauri as static files, so there is no server: every
// page is prerendered, and entities are addressed by search params rather than
// dynamic path segments (which would need every id known at build time).
const config: NextConfig = {
  output: "export",
  trailingSlash: true,
  images: { unoptimized: true },
};

export default config;
