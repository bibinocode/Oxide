import { PixelMark } from "../../components/layout/PrintMarks";
import { ContactPreview } from "../contact/components/ContactPreview";
import type {
  HomeIntroduction as HomeIntroductionValue,
  XiaohongshuCard,
} from "../../lib/api/types";
import { HalftonePortrait } from "./HalftonePortrait";
import Markdown from "react-markdown";
import { parseProfile } from "../contact/profile";

function linkText(children: React.ReactNode): string {
  if (typeof children === "string" || typeof children === "number") return String(children);
  if (Array.isArray(children)) return children.map(linkText).join("");
  if (children && typeof children === "object" && "props" in children) {
    return linkText(
      (children as React.ReactElement<{ children?: React.ReactNode }>).props.children,
    );
  }
  return "";
}

function IntroLink({
  href,
  children,
  portraitUrl,
  xiaohongshu,
}: {
  href?: string;
  children: React.ReactNode;
  portraitUrl: string;
  xiaohongshu?: XiaohongshuCard;
}) {
  if (!href || !/^(https?:\/\/|mailto:|\/(?!\/))/i.test(href)) return <>{children}</>;
  const title = linkText(children).trim() || "链接";
  const profile = parseProfile(href);
  const configured = xiaohongshu?.url ? parseProfile(xiaohongshu.url) : null;
  const card =
    profile?.service === "xiaohongshu" && profile.url === configured?.url ? xiaohongshu : undefined;
  return (
    <ContactPreview
      item={{ title, url: href, description: "", avatar_url: portraitUrl }}
      placement="bottom"
      trigger={children}
      xiaohongshuCard={card}
    />
  );
}

/** 首页与后台预览共用的个人介绍排版。 */
export function HomeIntroduction({
  name,
  description,
  value,
}: {
  name: string;
  description: string | null;
  value: HomeIntroductionValue;
}) {
  return (
    <div className="home-intro-container">
      <section aria-labelledby="site-title" className="home-intro">
        <div className="home-intro-copy">
          <div className="flex items-center gap-3">
            <h1 id="site-title" className="text-base font-semibold text-ink">
              {name || "站点名称"}
            </h1>
            <PixelMark />
          </div>
          <div className="home-intro-body">
            <Markdown
              components={{
                a: ({ href, children }) => (
                  <IntroLink
                    href={href}
                    portraitUrl={value.portrait_url}
                    xiaohongshu={value.xiaohongshu}
                  >
                    {children}
                  </IntroLink>
                ),
              }}
            >
              {value.body.trim() || description || "关于技术、设计与日常的记录。"}
            </Markdown>
          </div>
        </div>
        {value.portrait_url && (
          <HalftonePortrait src={value.portrait_url} alt={value.portrait_alt} />
        )}
      </section>
    </div>
  );
}
