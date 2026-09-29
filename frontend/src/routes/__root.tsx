import {
  HeadContent,
  Outlet,
  Scripts,
  createRootRoute,
  useRouterState,
} from "@tanstack/react-router";
import { SiteHeader } from "../components/layout/SiteHeader";
import { SiteFooter } from "../components/layout/SiteFooter";
import { getSiteData } from "../lib/api/server";
import tailwindCss from "../styles/tailwind.css?url";
import appScss from "../styles/app.scss?url";

export const Route = createRootRoute({
  loader: ({ location }) =>
    location.pathname.startsWith("/admin") ? null : getSiteData().catch(() => null),
  head: () => ({
    meta: [
      { charSet: "utf-8" },
      { name: "viewport", content: "width=device-width, initial-scale=1" },
      { title: "Oxide · 个人博客" },
      { name: "description", content: "文章、思考与记录。" },
    ],
    links: [
      { rel: "stylesheet", href: tailwindCss },
      { rel: "stylesheet", href: appScss },
      { rel: "alternate", type: "application/rss+xml", title: "Oxide RSS", href: "/feed.xml" },
    ],
  }),
  component: () => <Outlet />,
  shellComponent: RootDocument,
});

function RootDocument({ children }: { children: React.ReactNode }) {
  const site = Route.useLoaderData();
  const isAdmin = useRouterState({
    select: (state) => state.location.pathname.startsWith("/admin"),
  });
  return (
    <html lang="zh-CN" suppressHydrationWarning>
      <head>
        <script
          dangerouslySetInnerHTML={{
            __html: `try{let t=localStorage.getItem('oxide-theme');document.documentElement.dataset.theme=t==='light'||t==='dark'?t:matchMedia('(prefers-color-scheme: dark)').matches?'dark':'light'}catch{document.documentElement.dataset.theme='light'}`,
          }}
        />
        <HeadContent />
      </head>
      <body className="font-sans antialiased">
        {!isAdmin && (
          <>
            <div className="column-guides" aria-hidden="true" />
            <div className="viewport-edge-fade viewport-edge-fade-top" aria-hidden="true" />
            <div className="viewport-edge-fade viewport-edge-fade-bottom" aria-hidden="true" />
            <SiteHeader />
          </>
        )}
        {children}
        {!isAdmin && <SiteFooter site={site} />}
        <Scripts />
      </body>
    </html>
  );
}
