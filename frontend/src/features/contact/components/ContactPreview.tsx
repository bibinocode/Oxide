import { useRef, useState } from "react";
import { CodeXml, Mail, Play, Send } from "lucide-react";
import type { SiteItem, XiaohongshuCard } from "../../../lib/api/types";
import { getSocialPreview, type SocialPreviewData } from "../socialPreview";
import { parseProfile, type ProfileService } from "../profile";
import { apiRequest } from "../../../lib/api/client";

type Service = ProfileService | "email" | "other";
interface WebPreview {
  title: string;
  description: string | null;
  domain: string;
  icon_url: string | null;
}

/** 根据链接识别平台，悬停时加载该平台的公开资料。 */
export function ContactPreview({
  item,
  placement = "top",
  trigger,
  xiaohongshuCard,
}: {
  item: SiteItem;
  placement?: "top" | "bottom";
  trigger?: React.ReactNode;
  xiaohongshuCard?: XiaohongshuCard;
}) {
  const wrapper = useRef<HTMLSpanElement>(null);
  const [profile, setProfile] = useState<SocialPreviewData | null>(null);
  const [webPreview, setWebPreview] = useState<WebPreview | null>(null);
  const [loading, setLoading] = useState(false);
  const [failed, setFailed] = useState(false);
  const [avatarFailed, setAvatarFailed] = useState(false);
  const [cardLeft, setCardLeft] = useState(-14);
  const email = item.url.startsWith("mailto:") ? item.url.slice(7).split("?")[0] : "";
  const parsed = parseProfile(item.url);
  const service: Service = email ? "email" : (parsed?.service ?? "other");
  const handle = parsed?.handle.replace(/^@/, "") ?? "";
  const href = parsed?.url ?? item.url;
  const cardName =
    service === "xiaohongshu"
      ? xiaohongshuCard?.name || profile?.name || item.title
      : profile?.name || item.title;
  const cardBio = service === "xiaohongshu" ? xiaohongshuCard?.bio || profile?.bio : profile?.bio;
  const cardAvatar =
    service === "xiaohongshu"
      ? (xiaohongshuCard ? item.avatar_url : undefined) || profile?.avatarUrl
      : profile?.avatarUrl;

  function load() {
    const bounds = wrapper.current?.getBoundingClientRect();
    if (bounds) {
      setCardLeft(Math.max(16 - bounds.left, Math.min(-14, window.innerWidth - bounds.left - 310)));
    }
    if (service === "email" || profile || webPreview || loading || failed) return;
    if (service === "other") {
      if (!/^https?:\/\//.test(href)) return;
      setLoading(true);
      apiRequest<WebPreview>(`/api/v1/link-preview?url=${encodeURIComponent(href)}`)
        .then(setWebPreview)
        .catch(() => setFailed(true))
        .finally(() => setLoading(false));
      return;
    }
    setLoading(true);
    getSocialPreview({ data: href })
      .then((result) => (result ? setProfile(result) : setFailed(true)))
      .catch(() => setFailed(true))
      .finally(() => setLoading(false));
  }

  return (
    <span ref={wrapper} className="contact-preview" onPointerEnter={load} onFocus={load}>
      <a
        href={href}
        target={service === "email" || href.startsWith("/") ? undefined : "_blank"}
        rel="noopener noreferrer"
        className="contact-preview-trigger"
        aria-label={item.title}
      >
        {trigger ?? item.title}
      </a>
      <span
        className={`contact-preview-card contact-card-${service} ${placement === "bottom" ? "contact-preview-card-bottom" : ""}`}
        style={{ left: cardLeft }}
        aria-hidden="true"
      >
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
        ) : service === "x" ||
          service === "youtube" ||
          service === "telegram" ||
          service === "xiaohongshu" ? (
          <>
            <span className="contact-identity">
              <span className={`contact-identity-avatar contact-avatar-${service}`}>
                {cardAvatar && !avatarFailed ? (
                  <img src={cardAvatar} alt="" onError={() => setAvatarFailed(true)} />
                ) : service === "x" ? (
                  "𝕏"
                ) : service === "youtube" ? (
                  <Play size={22} fill="currentColor" />
                ) : service === "xiaohongshu" ? (
                  "书"
                ) : (
                  <Send size={20} />
                )}
              </span>
              <span className="contact-identity-names">
                <strong>{cardName}</strong>
                <small>
                  {service === "xiaohongshu"
                    ? xiaohongshuCard?.handle
                      ? `小红书号 ${xiaohongshuCard.handle}`
                      : "小红书"
                    : `@${handle}`}
                </small>
              </span>
              {service === "x" ? (
                <span className="contact-service-glyph">𝕏</span>
              ) : service === "youtube" ? (
                <Play className="contact-service-glyph" size={17} fill="currentColor" />
              ) : service === "xiaohongshu" ? (
                <span className="contact-xhs-wordmark">小红书</span>
              ) : (
                <Send className="contact-service-glyph" size={17} />
              )}
            </span>
            {cardBio && (service === "x" || service === "xiaohongshu") && (
              <span className="contact-identity-bio">{cardBio}</span>
            )}
            {(service === "xiaohongshu" ||
              loading ||
              profile?.followers != null ||
              profile?.following != null ||
              profile?.subscribers) && (
              <span className="contact-card-foot">
                {loading && !(service === "xiaohongshu" && xiaohongshuCard) ? (
                  "正在读取公开资料"
                ) : service === "xiaohongshu" ? (
                  <span>
                    {xiaohongshuCard?.followers || profile?.followers != null ? (
                      <>
                        <b>{xiaohongshuCard?.followers || profile?.followers?.toLocaleString()}</b>{" "}
                        粉丝
                      </>
                    ) : null}
                    {xiaohongshuCard?.likes &&
                    (xiaohongshuCard.followers || profile?.followers != null)
                      ? " · "
                      : null}
                    {xiaohongshuCard?.likes ? (
                      <>
                        <b>{xiaohongshuCard.likes}</b> 获赞与收藏
                      </>
                    ) : null}
                    {!xiaohongshuCard?.followers &&
                    !xiaohongshuCard?.likes &&
                    profile?.followers == null
                      ? "小红书个人主页"
                      : null}
                  </span>
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
            <span className="contact-preview-heading">
              <strong>{webPreview?.title || item.title}</strong>
              {webPreview?.icon_url && (
                <img src={webPreview.icon_url} width={16} height={16} alt="" />
              )}
            </span>
            <span className="contact-identity-bio">
              {webPreview?.description || (loading ? "正在读取网页信息" : href)}
            </span>
          </>
        )}
      </span>
    </span>
  );
}
