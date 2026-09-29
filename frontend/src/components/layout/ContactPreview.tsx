import { useState } from "react";
import { CodeXml, Mail, Play, Send } from "lucide-react";
import type { SiteItem } from "../../lib/api/types";
import { getSocialPreview, type SocialPreviewData } from "../../features/contact/socialPreview";

type Service = "github" | "x" | "youtube" | "telegram" | "email" | "other";

/** 根据链接识别平台，悬停时加载该平台的公开资料。 */
export function ContactPreview({ item }: { item: SiteItem }) {
  const [profile, setProfile] = useState<SocialPreviewData | null>(null);
  const [loading, setLoading] = useState(false);
  const [failed, setFailed] = useState(false);
  const [avatarFailed, setAvatarFailed] = useState(false);
  let host = "";
  let handle = "";
  const email = item.url.startsWith("mailto:") ? item.url.slice(7).split("?")[0] : "";
  try {
    const url = new URL(item.url);
    host = url.hostname.toLowerCase().replace(/^www\./, "");
    const parts = url.pathname.split("/").filter(Boolean);
    handle =
      (host === "youtube.com" && ["channel", "c", "user"].includes(parts[0])
        ? parts[1]
        : parts[0]
      )?.replace(/^@/, "") ?? "";
  } catch {
    // 邮件和站内路径不需要解析域名。
  }
  const service: Service = email
    ? "email"
    : host === "github.com" && handle
      ? "github"
      : (host === "x.com" || host === "twitter.com") && handle
        ? "x"
        : (host === "youtube.com" || host === "youtu.be") && handle
          ? "youtube"
          : (host === "t.me" || host === "telegram.me") && handle
            ? "telegram"
            : "other";

  function load() {
    if (service === "email" || service === "other" || profile || loading || failed) return;
    setLoading(true);
    getSocialPreview({ data: item.url })
      .then((result) => (result ? setProfile(result) : setFailed(true)))
      .catch(() => setFailed(true))
      .finally(() => setLoading(false));
  }

  return (
    <span className="contact-preview" onPointerEnter={load} onFocus={load}>
      <a
        href={item.url}
        rel="noopener noreferrer"
        className="contact-preview-trigger"
        aria-label={item.title}
      >
        {item.title}
      </a>
      <span className={`contact-preview-card contact-card-${service}`} aria-hidden="true">
        {service === "email" ? (
          <span className="contact-envelope">
            <span className="contact-envelope-flap" />
            <span className="contact-envelope-return">
              FROM
              <br />
              {item.title}
            </span>
            <span className="contact-envelope-stamp">
              <Mail size={19} />
              <small>POST · 26</small>
            </span>
            <span className="contact-envelope-postmark" />
            <span className="contact-envelope-address">
              <small>TO</small>
              {email}
            </span>
          </span>
        ) : service === "github" ? (
          <>
            <span className="contact-profile-head">
              <strong>{profile?.name || item.title}</strong>
              <span>@{handle}</span>
            </span>
            {profile?.contributions ? (
              <span className="contact-contributions">
                {Array.from({ length: 182 }, (_, index) => (
                  <i
                    key={index}
                    data-level={profile.contributions?.levels[index] ?? 0}
                    style={{ animationDelay: `${index * 2}ms` }}
                  />
                ))}
              </span>
            ) : loading ? (
              <span className="contact-preview-loading skeleton-line" />
            ) : null}
            <span className="contact-card-foot">
              <span>
                {profile?.contributions ? (
                  <>
                    <b>{profile.contributions.total.toLocaleString()}</b> 次贡献
                  </>
                ) : loading ? (
                  "正在读取公开资料"
                ) : (
                  "GitHub"
                )}
                {profile?.followers != null && (
                  <>
                    {" "}
                    · <b>{profile.followers.toLocaleString()}</b> 关注者
                  </>
                )}
              </span>
              <CodeXml size={15} />
            </span>
          </>
        ) : service === "x" || service === "youtube" || service === "telegram" ? (
          <>
            <span className="contact-identity">
              <span className={`contact-identity-avatar contact-avatar-${service}`}>
                {profile?.avatarUrl && !avatarFailed ? (
                  <img src={profile.avatarUrl} alt="" onError={() => setAvatarFailed(true)} />
                ) : service === "x" ? (
                  "𝕏"
                ) : service === "youtube" ? (
                  <Play size={22} fill="currentColor" />
                ) : (
                  <Send size={20} />
                )}
              </span>
              <span className="contact-identity-names">
                <strong>{profile?.name || item.title}</strong>
                <small>@{handle}</small>
              </span>
              {service === "x" ? (
                <span className="contact-service-glyph">𝕏</span>
              ) : service === "youtube" ? (
                <Play className="contact-service-glyph" size={17} fill="currentColor" />
              ) : (
                <Send className="contact-service-glyph" size={17} />
              )}
            </span>
            {profile?.bio && service === "x" && (
              <span className="contact-identity-bio">{profile.bio}</span>
            )}
            {(loading ||
              profile?.followers != null ||
              profile?.following != null ||
              profile?.subscribers) && (
              <span className="contact-card-foot">
                {loading ? (
                  "正在读取公开资料"
                ) : service === "youtube" ? (
                  <>{profile?.subscribers} 订阅者</>
                ) : (
                  <>
                    {profile?.following != null && (
                      <>
                        <b>{profile.following.toLocaleString()}</b> 正在关注
                      </>
                    )}
                    {profile?.following != null && profile?.followers != null && " · "}
                    {profile?.followers != null && (
                      <>
                        <b>{profile.followers.toLocaleString()}</b> 关注者
                      </>
                    )}
                  </>
                )}
              </span>
            )}
          </>
        ) : (
          <>
            <strong>{item.title}</strong>
            <span className="contact-identity-bio">{item.url}</span>
          </>
        )}
      </span>
    </span>
  );
}
