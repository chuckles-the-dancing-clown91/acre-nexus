/** @type {import('next').NextConfig} */
const nextConfig = {
  reactStrictMode: true,
  // Emit a self-contained server bundle for a slim production Docker image (#66).
  output: "standalone",
};

export default nextConfig;
