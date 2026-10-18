/** Static export: `npm run build` writes plain files to out/, ready for any web server. */
const nextConfig = {
  output: 'export',
  trailingSlash: true,
  images: { unoptimized: true },
  reactStrictMode: true,
  // This folder sits inside the launcher repo, which has its own lockfile; build from here.
  turbopack: { root: import.meta.dirname },
  outputFileTracingRoot: import.meta.dirname,
}

export default nextConfig
