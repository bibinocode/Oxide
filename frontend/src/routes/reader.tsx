import { useEffect, useState } from "react";
import { Link, createFileRoute } from "@tanstack/react-router";
import { ArrowLeft } from "lucide-react";
import { apiRequest } from "../lib/api/client";
import type { ReaderSession } from "../lib/api/types";

export const Route = createFileRoute("/reader")({
  validateSearch: (search: Record<string, unknown>) => ({
    next:
      typeof search.next === "string" &&
      search.next.startsWith("/") &&
      !search.next.startsWith("//") &&
      !search.next.includes("\\")
        ? search.next
        : "/columns",
  }),
  head: () => ({ meta: [{ title: "读者登录 · Oxide" }] }),
  component: ReaderPage,
});

/** 读者账号与管理员账号分离，登录后回到原专栏。 */
function ReaderPage() {
  const { next } = Route.useSearch();
  const [mode, setMode] = useState<"login" | "register">("login");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [pending, setPending] = useState(false);
  const [error, setError] = useState("");
  const [session, setSession] = useState<ReaderSession | null>(null);

  useEffect(() => {
    apiRequest<ReaderSession>("/api/v1/reader/session")
      .then(setSession)
      .catch(() => undefined);
  }, []);

  async function logout() {
    if (!session) return;
    try {
      await apiRequest("/api/v1/reader/logout", {
        method: "POST",
        headers: { "X-CSRF-Token": session.csrf_token },
      });
      window.location.reload();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "退出失败");
    }
  }

  async function submit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setPending(true);
    setError("");
    try {
      await apiRequest<ReaderSession>(`/api/v1/reader/${mode}`, {
        method: "POST",
        body: JSON.stringify({ username, password }),
      });
      window.location.assign(next);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "请稍后重试");
    } finally {
      setPending(false);
    }
  }

  return (
    <main className="reading-width pt-10 md:pt-14">
      <Link
        to="/columns"
        className="inline-flex items-center gap-2 text-xs text-muted hover:text-ink"
      >
        <ArrowLeft size={14} /> 小册
      </Link>
      <section className="reader-account">
        <p className="eyebrow">Reader</p>
        <h1>读者账号</h1>
        {session ? (
          <div className="mt-8 space-y-5">
            <p className="text-sm">{session.username}</p>
            <div className="flex items-center gap-5">
              <Link to="/columns" className="button-primary">
                查看小册
              </Link>
              <button type="button" className="text-sm text-muted hover:text-ink" onClick={logout}>
                退出登录
              </button>
            </div>
            {error && (
              <p role="alert" className="text-sm text-warm">
                {error}
              </p>
            )}
          </div>
        ) : (
          <>
            <div className="reader-mode" role="group" aria-label="账号操作">
              <button
                type="button"
                aria-pressed={mode === "login"}
                onClick={() => {
                  setMode("login");
                  setError("");
                }}
              >
                登录
              </button>
              <button
                type="button"
                aria-pressed={mode === "register"}
                onClick={() => {
                  setMode("register");
                  setError("");
                }}
              >
                注册
              </button>
            </div>
            <form onSubmit={submit} className="space-y-5">
              <label className="block text-sm">
                用户名
                <input
                  className="field mt-2"
                  value={username}
                  required
                  minLength={3}
                  maxLength={32}
                  pattern="[A-Za-z0-9_]+"
                  autoComplete="username"
                  onChange={(event) => setUsername(event.target.value)}
                />
              </label>
              <label className="block text-sm">
                密码
                <input
                  className="field mt-2"
                  type="password"
                  value={password}
                  required
                  minLength={12}
                  maxLength={128}
                  autoComplete={mode === "register" ? "new-password" : "current-password"}
                  onChange={(event) => setPassword(event.target.value)}
                />
              </label>
              <button className="button-primary" type="submit" disabled={pending}>
                {pending ? "请稍候…" : mode === "login" ? "登录" : "创建账号"}
              </button>
              {error && (
                <p role="alert" className="text-sm text-warm">
                  {error}
                </p>
              )}
            </form>
          </>
        )}
      </section>
    </main>
  );
}
