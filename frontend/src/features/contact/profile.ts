export type ProfileService = "github" | "x" | "youtube" | "telegram" | "xiaohongshu";
export type Profile = { service: ProfileService; handle: string; url: string };

type ProfileRule = {
  service: ProfileService;
  hosts: readonly string[];
  canonicalHost: string;
  path: RegExp;
};

/** 已知平台只接收个人主页，分享链接的跟踪参数不会进入公开页面。 */
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
  {
    service: "xiaohongshu",
    hosts: ["xiaohongshu.com"],
    canonicalHost: "www.xiaohongshu.com",
    path: /^\/user\/profile\/([a-fA-F0-9]{24})\/?$/,
  },
];

/** 根据白名单解析账号地址，并移除平台分享链接中的临时令牌。 */
export function parseProfile(input: string): Profile | null {
  if (input.length > 4096) return null;
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
  if (!handle) return null;
  const path = rule.service === "xiaohongshu" ? `user/profile/${handle}` : handle;
  return { service: rule.service, handle, url: `https://${rule.canonicalHost}/${path}` };
}
