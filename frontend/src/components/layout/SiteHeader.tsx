import { useRef } from "react";
import { DockTooltip } from "./DockTooltip";
import { useDockShortcuts } from "../../hooks/useDockShortcuts";
import { playUiSound } from "../../lib/sound";
import { Link, useLocation } from "@tanstack/react-router";
import { Archive, BookOpen, FolderCode, UserRound } from "lucide-react";
import { SitePreferences } from "./SitePreferences";
import homeAvatar from "../../assets/horse-selfie-avatar.svg";

export function SiteHeader({ projectsEnabled = false }: { projectsEnabled?: boolean }) {
  const dock = useRef<HTMLElement>(null);
  const pathname = useLocation({ select: (location) => location.pathname });
  const inTaxonomy = pathname.startsWith("/categories/") || pathname.startsWith("/tags/");
  useDockShortcuts(dock);
  return (
    <nav
      ref={dock}
      onClick={(event) => {
        if (event.target instanceof Element && event.target.closest("a")) playUiSound();
      }}
      className="site-dock"
      data-projects-enabled={projectsEnabled}
      aria-label="主导航"
    >
      <Link to="/" data-go-key="H" aria-label="首页，先按 G 再按 H">
        <img
          src={homeAvatar}
          className="site-dock-home-avatar"
          width={28}
          height={28}
          alt=""
          aria-hidden="true"
        />
        <DockTooltip label="首页" goKey="H" />
      </Link>
      <span className="site-dock-divider" aria-hidden="true" />
      <Link
        to="/archive"
        search={{ page: 1 }}
        activeOptions={{ includeSearch: false }}
        data-go-key="W"
        data-section-active={inTaxonomy || undefined}
        aria-label="归档，先按 G 再按 W"
      >
        <Archive size={18} aria-hidden="true" />
        <DockTooltip label="归档" goKey="W" />
      </Link>
      <Link to="/columns" data-go-key="B" aria-label="小册，先按 G 再按 B">
        <BookOpen size={18} aria-hidden="true" />
        <DockTooltip label="小册" goKey="B" />
      </Link>
      {projectsEnabled && (
        <Link to="/projects" data-go-key="P" aria-label="项目，先按 G 再按 P">
          <FolderCode size={18} aria-hidden="true" />
          <DockTooltip label="项目" goKey="P" />
        </Link>
      )}
      <Link to="/about" data-go-key="A" aria-label="关于，先按 G 再按 A">
        <UserRound size={18} aria-hidden="true" />
        <DockTooltip label="关于" goKey="A" />
      </Link>
      <span className="site-dock-divider" aria-hidden="true" />
      <SitePreferences />
    </nav>
  );
}
