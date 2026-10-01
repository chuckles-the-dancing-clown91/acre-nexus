/** @type {import('next').NextConfig} */
const nextConfig = {
  reactStrictMode: true,
  // Emit a self-contained server bundle for a slim production Docker image (#66).
  output: "standalone",
  // The console and public site refuse to be framed by other sites; only the
  // /embed widgets may be, so a client's website can show them.
  async headers() {
    return [
      {
        // Private and utility pages stay out of search results, whatever links to them.
        source:
          "/(console|embed|login|auth|account|set-password|forgot-password|sign)/:path*",
        headers: [{ key: "X-Robots-Tag", value: "noindex, nofollow" }],
      },
      {
        source: "/(login|account|set-password|forgot-password)",
        headers: [{ key: "X-Robots-Tag", value: "noindex, nofollow" }],
      },
      {
        source: "/((?!embed).*)",
        headers: [
          { key: "Content-Security-Policy", value: "frame-ancestors 'self'" },
          { key: "X-Frame-Options", value: "SAMEORIGIN" },
        ],
      },
    ];
  },
};

export default nextConfig;
