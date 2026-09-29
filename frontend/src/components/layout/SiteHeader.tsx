import { Link } from "@tanstack/react-router";
import { Archive, Home, Rss, Search, UserRound } from "lucide-react";

export function SiteHeader() {
  return (
    <nav className="site-dock" aria-label="主导航">
      <Link to="/" title="首页" aria-label="首页">
        <Home size={18} />
      </Link>
      <Link to="/archive" search={{ page: 1 }} title="归档" aria-label="归档">
        <Archive size={18} />
      </Link>
      <Link to="/search" search={{ q: "", page: 1 }} title="搜索" aria-label="搜索">
        <Search size={18} />
      </Link>
      <Link to="/about" title="关于" aria-label="关于">
        <UserRound size={18} />
      </Link>
      <a href="/feed.xml" title="RSS 订阅" aria-label="RSS 订阅">
        <Rss size={18} />
      </a>
    </nav>
  );
}
