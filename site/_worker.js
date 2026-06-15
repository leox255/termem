// Cloudflare Pages advanced-mode worker.
// Serves the static site, plus /api/github: a cached aggregate of the repo's
// live GitHub data (README + metadata) so the landing page mirrors the repo
// without per-visitor GitHub rate limits. Cached 5 min at the edge.

const REPO = "leox255/termem";
const GH = "https://api.github.com";

export default {
  async fetch(request, env, ctx) {
    const url = new URL(request.url);
    if (url.pathname === "/api/github") {
      return githubData(request, env, ctx);
    }
    return env.ASSETS.fetch(request);
  },
};

function pageFromLink(res, fallback) {
  const link = res.headers.get("Link") || "";
  const m = link.match(/[?&]page=(\d+)>;\s*rel="last"/);
  return m ? parseInt(m[1], 10) : fallback;
}

async function githubData(request, env, ctx) {
  const cache = caches.default;
  const cacheKey = new Request(new URL("/api/github", request.url).toString(), { method: "GET" });
  const hit = await cache.match(cacheKey);
  if (hit) return hit;

  const base = { "User-Agent": "termem-landing", "Accept": "application/vnd.github+json" };
  if (env.GITHUB_TOKEN) base["Authorization"] = `Bearer ${env.GITHUB_TOKEN}`;
  const get = (path, accept) =>
    fetch(`${GH}/repos/${REPO}${path}`, {
      headers: accept ? { ...base, Accept: accept } : base,
      cf: { cacheTtl: 300, cacheEverything: true },
    });

  const out = { ok: false };
  try {
    const [repoR, readmeR, contribR, langR, commitsR, releasesR] = await Promise.all([
      get(""),
      get("/readme", "application/vnd.github.html"),
      get("/contributors?per_page=12"),
      get("/languages"),
      get("/commits?per_page=1"),
      get("/releases?per_page=1"),
    ]);

    if (repoR.ok) {
      const r = await repoR.json();
      out.repo = {
        description: r.description, homepage: r.homepage, topics: r.topics || [],
        stars: r.stargazers_count, watchers: r.subscribers_count, forks: r.forks_count,
        defaultBranch: r.default_branch,
      };
    }
    if (readmeR.ok) out.readmeHtml = await readmeR.text();
    if (contribR.ok) {
      const c = await contribR.json();
      if (Array.isArray(c))
        out.contributors = c.filter((x) => x.type !== "Bot")
          .map((x) => ({ login: x.login, avatar: x.avatar_url, url: x.html_url }));
    }
    if (langR.ok) out.languages = await langR.json();
    if (commitsR.ok) {
      const cm = await commitsR.json();
      if (Array.isArray(cm) && cm[0]) {
        const c = cm[0];
        const coauthors = (c.commit.message.match(/Co-authored-by:\s*([^<\n]+)/g) || [])
          .map((s) => s.replace(/Co-authored-by:\s*/, "").trim());
        out.latestCommit = {
          sha: c.sha.slice(0, 7), message: c.commit.message.split("\n")[0],
          author: c.author ? c.author.login : (c.commit.author ? c.commit.author.name : null),
          authorAvatar: c.author ? c.author.avatar_url : null,
          date: c.commit.committer.date, coauthors,
        };
        out.commitCount = pageFromLink(commitsR, cm.length);
      }
    }
    if (releasesR.ok) {
      const rel = await releasesR.json();
      out.releasesCount = pageFromLink(releasesR, rel.length);
      if (rel[0]) out.latestRelease = { name: rel[0].name || rel[0].tag_name, tag: rel[0].tag_name, url: rel[0].html_url, date: rel[0].published_at };
    }
    out.ok = true;
  } catch (e) {
    out.ok = false;
    out.error = String(e);
  }

  const resp = new Response(JSON.stringify(out), {
    headers: {
      "Content-Type": "application/json; charset=utf-8",
      "Cache-Control": "public, max-age=300, s-maxage=300",
      "Access-Control-Allow-Origin": "*",
    },
  });
  if (out.ok) ctx.waitUntil(cache.put(cacheKey, resp.clone()));
  return resp;
}
