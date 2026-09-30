import { Link, Outlet, createFileRoute, useRouterState } from "@tanstack/react-router";
import { useState } from "react";
import {
  ArrowUpRight,
  FileText,
  Images,
  LogOut,
  MessageSquare,
  Settings,
  BookOpen,
  Bot,
} from "lucide-react";
import { AdminSessionProvider, csrfHeaders, useAdminSession } from "../features/admin/AdminSession";
import { apiRequest } from "../lib/api/client";
import type { Session } from "../lib/api/types";
import { PixelMark } from "../components/layout/PrintMarks";
import { ThemeControl } from "../components/layout/ThemeControl";

export const Route = createFileRoute("/admin")({
  component: () => (
    <AdminSessionProvider>
      <AdminLayout />
    </AdminSessionProvider>
  ),
});

function AdminLayout() {
  const { session, loading, setSession } = useAdminSession();
  const isEditor = useRouterState({
    select: (state) => state.location.pathname.startsWith("/admin/articles/"),
  });
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState("");
  const [pending, setPending] = useState(false);

  async function login(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setPending(true);
    setError("");
    try {
      setSession(
        await apiRequest<Session>("/api/v1/admin/login", {
          method: "POST",
          body: JSON.stringify({ username, password }),
        }),
      );
      setPassword("");
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "登录失败");
    } finally {
      setPending(false);
    }
  }

  async function logout() {
    if (!session) return;
    await apiRequest("/api/v1/admin/logout", {
      method: "POST",
      headers: csrfHeaders(session),
    }).catch(() => undefined);
    setSession(null);
  }

  if (loading) return <main className="admin-login text-sm text-muted">正在读取会话…</main>;
  if (!session)
    return (
      <main className="admin-login">
        <form onSubmit={login} className="w-full max-w-sm space-y-5">
          <div className="mb-9 flex items-center justify-between border-b border-line pb-5">
            <div>
              <p className="font-mono text-xs text-muted">OXIDE / ADMIN</p>
              <h1 className="mt-2 text-xl font-semibold">管理登录</h1>
            </div>
            <PixelMark />
          </div>
          <label className="block text-sm">
            用户名
            <input
              className="field mt-2"
              autoComplete="username"
              required
              value={username}
              onChange={(event) => setUsername(event.target.value)}
            />
          </label>
          <label className="block text-sm">
            密码
            <input
              className="field mt-2"
              type="password"
              autoComplete="current-password"
              required
              value={password}
              onChange={(event) => setPassword(event.target.value)}
            />
          </label>
          <button type="submit" disabled={pending} className="button-primary">
            登录
          </button>
          <p role="alert" className="text-sm text-warm">
            {error}
          </p>
        </form>
      </main>
    );

  return (
    <div className={`admin-app ${isEditor ? "admin-app-writing" : ""}`}>
      <aside className="admin-sidebar">
        <div className="admin-sidebar-brand">
          <div>
            <p className="font-mono text-[11px] text-muted">OXIDE / STUDIO</p>
            <h1 className="mt-1 text-base font-semibold">内容管理</h1>
          </div>
          <PixelMark />
        </div>
        <nav className="admin-sidebar-nav" aria-label="管理导航">
          <Link
            to="/admin"
            activeOptions={{ exact: true }}
            activeProps={{ className: "is-active" }}
            className={isEditor ? "is-active" : undefined}
          >
            <FileText size={17} /> 文章
          </Link>
          <Link to="/admin/comments" activeProps={{ className: "is-active" }}>
            <MessageSquare size={17} /> 评论
          </Link>
          <Link to="/admin/notion" activeProps={{ className: "is-active" }}>
            <BookOpen size={17} /> Notion 导入
          </Link>
          <Link to="/admin/assets" activeProps={{ className: "is-active" }}>
            <Images size={17} /> 素材
          </Link>
          <Link
            to="/admin/agent"
            search={{ tab: "skills" }}
            activeProps={{ className: "is-active" }}
          >
            <Bot size={17} /> Agent
          </Link>
          <Link to="/admin/settings" activeProps={{ className: "is-active" }}>
            <Settings size={17} /> 设置
          </Link>
        </nav>
        <div className="admin-sidebar-footer">
          <span className="truncate text-xs text-muted">{session.username}</span>
          <ThemeControl compact />
          <Link to="/" className="flex items-center gap-2 text-sm hover:text-ink">
            <ArrowUpRight size={16} /> 返回站点
          </Link>
          <button
            type="button"
            onClick={logout}
            className="flex items-center gap-2 text-sm hover:text-warm"
          >
            <LogOut size={16} /> 退出登录
          </button>
        </div>
      </aside>
      <main className={`admin-main ${isEditor ? "admin-main-editor" : "admin-main-standard"}`}>
        <Outlet />
      </main>
    </div>
  );
}
