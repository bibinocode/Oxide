import { load } from "cheerio";
import { ProxyAgent, fetch as outboundFetch } from "undici";
import type { SocialPreviewData } from "./socialPreview";

type Service = "github" | "x" | "youtube" | "telegram";
type Profile = { service: Service; handle: string; url: string };
type ProfileRule = {
  service: Service;
  hosts: readonly string[];
  canonicalHost: string;
  path: RegExp;
};

/** 平台规则集中维护；正则的第一组必须捕获规范账号路径。 */
const PROFILE_RULES: readonly ProfileRule[] = [
  {
    service: "github",
    hosts: ["github.com"],
    canonicalHost: "github.com",
    path: /^\/([a-zA-Z0-9](?:[a-zA-Z0-9-]{0,37}[a-zA-Z0-9])?)\/?$/,
  },
  {
    service: "x",
    hosts: ["x.com", "twitter.com"],
    canonicalHost: "x.com",
    path: /^\/([a-zA-Z0-9_]{1,15})\/?$/,
  },
  {
    service: "youtube",
    hosts: ["youtube.com"],
    canonicalHost: "www.youtube.com",
    path: /^\/(@[a-zA-Z0-9._-]{3,30})\/?$/,
  },
  {
    service: "youtube",
    hosts: ["youtube.com"],
    canonicalHost: "www.youtube.com",
    path: /^\/((?:channel|c|user)\/[a-zA-Z0-9_-]{1,80})\/?$/,
  },
  {
    service: "telegram",
    hosts: ["t.me", "telegram.me"],
    canonicalHost: "t.me",
    path: /^\/([a-zA-Z0-9_]{5,32})\/?$/,
  },
];

const cache = new Map<string, { expires: number; value: SocialPreviewData | null }>();
const proxyUrl = process.env.HTTPS_PROXY ?? process.env.https_proxy;
const dispatcher = proxyUrl ? new ProxyAgent(proxyUrl) : undefined;

/** 限定平台域名与个人主页路径，避免将服务端请求转发到任意地址。 */
export function parseProfile(input: string): Profile | null {
  if (input.length > 2048) return null;
  let url: URL;
  try {
    url = new URL(input);
  } catch {
    return null;
  }
  if (url.protocol !== "https:" || url.port || url.username || url.password) return null;
  const host = url.hostname.toLowerCase().replace(/^www\./, "");
  const rule = PROFILE_RULES.find(
    (candidate) => candidate.hosts.includes(host) && candidate.path.test(url.pathname),
  );
  if (!rule) return null;
  const handle = rule.path.exec(url.pathname)?.[1];
  return handle
    ? { service: rule.service, handle, url: `https://${rule.canonicalHost}/${handle}` }
    : null;
}

function text(value: unknown, maxLength = 500): string | undefined {
  return typeof value === "string" ? value.trim().slice(0, maxLength) || undefined : undefined;
}

function imageUrl(value: unknown): string | undefined {
  if (typeof value !== "string") return undefined;
  try {
    const url = new URL(value);
    return url.protocol === "https:" ? url.href : undefined;
  } catch {
    return undefined;
  }
}

/** HTML 使用解析器读取 Open Graph 元数据，避免依赖标签属性顺序。 */
async function fetchHtmlProfile(profile: Profile): Promise<SocialPreviewData | null> {
  const response = await outboundFetch(profile.url, {
    dispatcher,
    redirect: "manual",
    signal: AbortSignal.timeout(6000),
    headers: { "user-agent": "Mozilla/5.0", "accept-language": "en-US,en;q=0.9" },
  });
  if (!response.ok || !response.headers.get("content-type")?.includes("text/html")) return null;
  const html = await response.text();
  if (html.length > 3_000_000) return null;
  const $ = load(html);
  const meta = (property: string) => text($(`meta[property="${property}"]`).attr("content"));
  const title = meta("og:title");
  const description = meta("og:description");
  const name = title
    ?.replace(/\s*\(@[^)]+\)\s*(?:on X)?$/i, "")
    .replace(/\s*[-|].*$/, "")
    .trim();
  if (profile.service === "x" && (!name || /^x\b|^twitter\b/i.test(name))) return null;
  const result: SocialPreviewData = {
    name,
    bio: description,
    avatarUrl: imageUrl(meta("og:image")),
  };
  if (profile.service === "youtube") {
    result.subscribers = text(html.match(/([\d.,]+\s*[KMB]?) subscribers/i)?.[1], 24);
  }
  return result;
}

/** GitHub 的身份信息来自官方 API，贡献图来自公开贡献接口。 */
async function fetchGitHub(profile: Profile): Promise<SocialPreviewData | null> {
  const [user, contribution] = await Promise.allSettled([
    outboundFetch(`https://api.github.com/users/${profile.handle}`, {
      dispatcher,
      signal: AbortSignal.timeout(6000),
      headers: { accept: "application/vnd.github+json", "user-agent": "Oxide-blog" },
    }),
    outboundFetch(`https://github-contributions-api.jogruber.de/v4/${profile.handle}?y=last`, {
      dispatcher,
      signal: AbortSignal.timeout(6000),
    }),
  ]);
  const result: SocialPreviewData = {};
  if (user.status === "fulfilled" && user.value.ok) {
    const data = (await user.value.json()) as Record<string, unknown>;
    result.name = text(data.name, 80) ?? text(data.login, 80);
    result.bio = text(data.bio);
    result.avatarUrl = imageUrl(data.avatar_url);
    if (typeof data.followers === "number") result.followers = data.followers;
    if (typeof data.following === "number") result.following = data.following;
  }
  if (contribution.status === "fulfilled" && contribution.value.ok) {
    const data = (await contribution.value.json()) as {
      total?: { lastYear?: number };
      contributions?: { level?: number }[];
    };
    if (Array.isArray(data.contributions) && Number.isFinite(data.total?.lastYear)) {
      result.contributions = {
        total: data.total?.lastYear ?? 0,
        levels: data.contributions
          .slice(-182)
          .map((day) => Math.max(0, Math.min(4, day.level ?? 0))),
      };
    }
  }
  return Object.keys(result).length ? result : null;
}

/** 成功缓存六小时，失败缓存十分钟，避免悬停时反复请求平台。 */
export async function fetchSocialPreview(input: string): Promise<SocialPreviewData | null> {
  const profile = parseProfile(input);
  if (!profile) return null;
  const cached = cache.get(profile.url);
  if (cached && cached.expires > Date.now()) return cached.value;
  let value: SocialPreviewData | null = null;
  try {
    value =
      profile.service === "github" ? await fetchGitHub(profile) : await fetchHtmlProfile(profile);
  } catch {
    // 平台限流或屏蔽匿名访问时，页面仍可显示账号链接。
  }
  if (cache.size >= 128) cache.delete(cache.keys().next().value!);
  cache.set(profile.url, { expires: Date.now() + (value ? 6 * 60 * 60_000 : 10 * 60_000), value });
  return value;
}
