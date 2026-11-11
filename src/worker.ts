import handler, {
  BucketCachePurge,
  DOQueueHandler,
  DOShardedTagCache,
} from "../.open-next/worker.js";
import builtRoutes from "../.next/routes-manifest.json";
import notFoundHtml from "../.next/server/app/_not-found.html";

type RouteManifest = {
  caseSensitive: boolean;
  staticRoutes: { page: string; regex: string }[];
  dynamicRoutes: { page: string; regex: string }[];
  redirects: { internal?: boolean; regex: string }[];
};

const manifest = builtRoutes as RouteManifest;
const routes = [
  ...manifest.staticRoutes,
  ...manifest.dynamicRoutes,
].filter(({ page }) => !page.startsWith("/_"));
const routePatterns = [
  ...routes,
  ...manifest.redirects.filter(({ internal }) => !internal),
].map(({ regex }) => new RegExp(regex, manifest.caseSensitive ? "" : "i"));

export default {
  fetch(request, env, ctx) {
    const pathname = new URL(request.url).pathname;
    // Existing assets bypass this Worker. Missing assets and unknown routes
    // must not load Next's page renderer just to produce a 404.
    if (pathname !== "/_next/image" && !routePatterns.some((route) => route.test(pathname))) {
      return new Response(request.method === "HEAD" ? null : notFoundHtml, {
        status: 404,
        headers: {
          "Content-Type": "text/html; charset=utf-8",
          "Cache-Control": "no-store",
          "X-Content-Type-Options": "nosniff",
        },
      });
    }
    return handler.fetch(request, env, ctx);
  },
} satisfies ExportedHandler<CloudflareEnv>;

export { BucketCachePurge, DOQueueHandler, DOShardedTagCache };
