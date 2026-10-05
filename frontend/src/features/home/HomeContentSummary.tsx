import { Link } from "@tanstack/react-router";
import type { SiteSection } from "../../lib/api/types";

/** 首页简介下方的内容入口；数量来自完整列表，关闭作品集时不保留无效链接。 */
export function HomeContentSummary({
  articleCount,
  columnCount,
  projects,
}: {
  articleCount: number;
  columnCount: number;
  projects?: SiteSection;
}) {
  return (
    <nav
      className="home-content-summary"
      aria-label="站点内容统计"
      data-projects-enabled={projects?.enabled ?? false}
    >
      <Link
        to="/archive"
        search={{ page: 1 }}
        className="summary-card"
        style={{ animationDelay: "140ms" }}
      >
        <span className="summary-vignette summary-sheets" aria-hidden="true">
          <span />
          <span />
          <span />
        </span>
        <span className="summary-label">写作</span>
        <span className="summary-count">{articleCount} 篇文章</span>
      </Link>
      <Link to="/columns" className="summary-card" style={{ animationDelay: "190ms" }}>
        <span className="summary-vignette summary-books" aria-hidden="true">
          <span />
          <span />
          <span />
        </span>
        <span className="summary-label">小册</span>
        <span className="summary-count">{columnCount} 本小册</span>
      </Link>
      {projects?.enabled && (
        <Link to="/projects" className="summary-card" style={{ animationDelay: "240ms" }}>
          <span className="summary-vignette" aria-hidden="true">
            <ProjectSummaryMark />
          </span>
          <span className="summary-label">项目</span>
          <span className="summary-count">{projects.items.length} 个项目</span>
        </Link>
      )}
    </nav>
  );
}

/** 改编参考项目的施工定位线与交叉工具标记，来源与许可见第三方声明。 */
function ProjectSummaryMark() {
  return (
    <span className="summary-project-icon">
      <svg
        className="summary-construction-guides"
        viewBox="0 0 52 52"
        preserveAspectRatio="xMidYMid meet"
      >
        <g className="summary-guide-solid" fill="none">
          <path d="M26 0V52M0 26H52M0 0L52 52M52 0L0 52" />
          <circle cx="26" cy="26" r="21" />
        </g>
      </svg>
      {/* 悬停时分离铅笔与尺子，沿用参考项目的工具拆解效果。 */}
      <svg className="summary-project-mark" viewBox="0 0 18 18" width="30" height="30">
        <g
          fill="none"
          strokeLinecap="round"
          strokeLinejoin="round"
          strokeWidth="1.5"
          stroke="currentColor"
        >
          <g className="summary-explode-a">
            <path
              fillRule="evenodd"
              clipRule="evenodd"
              d="M14.1711 6.85988L11.1411 3.8299L3.766 11.205C2.82897 12.142 2.263 15.6755 2.25119 15.7498C2.2504 15.7499 2.25 15.75 2.25 15.75L2.251 15.751C2.251 15.751 2.25106 15.7506 2.25119 15.7498C2.32546 15.738 5.85897 15.172 6.796 14.235L14.1711 6.85988Z"
              fill="currentColor"
              fillOpacity="0.3"
              stroke="none"
            />
            <path d="M2.25 15.75C2.25 15.75 5.849 15.182 6.796 14.235C7.743 13.288 15.373 5.65799 15.373 5.65799C16.21 4.82099 16.21 3.46399 15.373 2.62799C14.536 1.79099 13.179 1.79099 12.343 2.62799C12.343 2.62799 4.713 10.258 3.766 11.205C2.819 12.152 2.251 15.751 2.251 15.751L2.25 15.75Z" />
            <path d="M11.121 3.84802L14.152 6.87902" />
          </g>
          <g className="summary-explode-b">
            <path d="M9.53299 5.43699L6.20199 2.10599C5.81099 1.71499 5.17799 1.71499 4.78799 2.10599L2.10599 4.78799C1.71499 5.17899 1.71499 5.81199 2.10599 6.20199L5.43699 9.53299" />
            <path d="M3.41599 7.513L5.18398 5.745" />
          </g>
          <g className="summary-explode-c">
            <path d="M10.487 14.584L12.255 12.816" />
            <path d="M8.46698 12.563L11.798 15.894C12.189 16.285 12.822 16.285 13.212 15.894L15.894 13.212C16.285 12.821 16.285 12.188 15.894 11.798L14.685 10.589" />
          </g>
        </g>
      </svg>
    </span>
  );
}
