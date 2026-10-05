import { Link } from "@tanstack/react-router";
import type { SiteSettings } from "../../lib/api/types";
import { ContactPreview } from "../../features/contact/components/ContactPreview";

/** 页脚内容来自站点配置，与首页模块共用显示开关。 */
export function SiteFooter({ site }: { site?: SiteSettings | null }) {
  const config = site?.presentation;
  return (
    <footer className="public-width site-footer">
      <div className="site-footer-grid">
        <div className="site-footer-introduction">
          <p className="font-mono text-xs">
            © {new Date().getFullYear()} {site?.site_name ?? "Oxide"}
          </p>
          <p className="mt-3 text-xs leading-6">{config?.footer_text || "写作、创造与记录。"}</p>
        </div>
        {config?.contacts.enabled && config.contacts.items.length > 0 && (
          <div className="site-footer-contacts">
            <h2>联系</h2>
            <ul>
              {config.contacts.items.map((item, i) => (
                <li key={i}>
                  <ContactPreview item={item} />
                </li>
              ))}
            </ul>
          </div>
        )}
        <div className="site-footer-index">
          <h2>索引</h2>
          <ul>
            <li>
              <Link to="/">首页</Link>
            </li>
            <li>
              <Link to="/archive" search={{ page: 1 }}>
                写作
              </Link>
            </li>
            {config?.projects.enabled && (
              <li>
                <Link to="/projects">作品集</Link>
              </li>
            )}
            {config?.services.enabled && config.services.items.length > 0 && (
              <li>
                <a href="/#services">服务</a>
              </li>
            )}
            <li>
              <a href="/feed.xml">RSS</a>
            </li>
            <li>
              <Link to="/about">关于我</Link>
            </li>
          </ul>
        </div>
      </div>
    </footer>
  );
}
