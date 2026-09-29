import { useState } from "react";
import { Send } from "lucide-react";
import { SectionTag } from "../../../components/layout/PrintMarks";
import { apiRequest } from "../../../lib/api/client";
import type { Comment } from "../../../lib/api/types";

interface Props {
  slug: string;
  initialComments: Comment[];
}

export function Comments({ slug, initialComments }: Props) {
  const [comments, setComments] = useState(initialComments);
  const [replyTo, setReplyTo] = useState<string | null>(null);
  const [nickname, setNickname] = useState("");
  const [email, setEmail] = useState("");
  const [body, setBody] = useState("");
  const [pending, setPending] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const [message, setMessage] = useState("");

  async function submit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setPending(true);
    setMessage("");
    try {
      await apiRequest(`/api/v1/articles/${encodeURIComponent(slug)}/comments`, {
        method: "POST",
        body: JSON.stringify({ nickname, email, body, parent_public_id: replyTo }),
      });
      setBody("");
      setReplyTo(null);
      setMessage("评论已提交，审核通过后会显示。");
    } catch (error) {
      setMessage(error instanceof Error ? error.message : "提交失败");
    } finally {
      setPending(false);
    }
  }

  async function refresh() {
    setRefreshing(true);
    try {
      setComments(
        await apiRequest<Comment[]>(`/api/v1/articles/${encodeURIComponent(slug)}/comments`),
      );
    } catch {
      setMessage("评论刷新失败。");
    } finally {
      setRefreshing(false);
    }
  }

  return (
    <section className="mt-16 border-t border-line pt-6">
      <div className="flex items-center justify-between">
        <SectionTag index="01">评论</SectionTag>
        <span className="font-mono text-xs text-muted">{comments.length} 条</span>
        <button
          type="button"
          onClick={refresh}
          disabled={refreshing}
          className="text-sm text-accent"
        >
          刷新
        </button>
      </div>
      <div className="mt-7 space-y-0" aria-busy={refreshing}>
        {refreshing &&
          Array.from({ length: Math.max(2, Math.min(comments.length, 4)) }, (_, index) => (
            <div key={index} className="border-b border-line py-5">
              <div className="skeleton-line h-4 w-36" />
              <div className="skeleton-line mt-4 h-4 w-4/5" />
              <div className="skeleton-line mt-2 h-4 w-2/5" />
            </div>
          ))}
        {!refreshing &&
          comments.map((comment) => (
            <article
              key={comment.public_id}
              className={`border-b border-line py-5 ${comment.parent_public_id ? "ml-10" : ""}`}
            >
              <div className="flex items-center gap-3">
                <img
                  src={comment.avatar_url}
                  alt=""
                  width="36"
                  height="36"
                  className="h-9 w-9 rounded-sm"
                />
                <div>
                  <strong className="text-sm">{comment.nickname}</strong>
                  <time className="ml-3 text-xs text-muted">
                    {new Intl.DateTimeFormat("zh-CN").format(new Date(comment.created_at))}
                  </time>
                </div>
              </div>
              <p className="mt-3 whitespace-pre-wrap break-words text-sm leading-7">
                {comment.body}
              </p>
              {!comment.parent_public_id && (
                <button
                  type="button"
                  className="mt-2 text-xs text-accent"
                  onClick={() => setReplyTo(comment.public_id)}
                >
                  回复
                </button>
              )}
            </article>
          ))}
        {!refreshing && comments.length === 0 && (
          <p className="py-8 text-sm text-muted">还没有评论。</p>
        )}
      </div>
      <form onSubmit={submit} className="mt-9 space-y-4">
        <h3 className="font-semibold">发表评论</h3>
        {replyTo && (
          <div className="flex items-center gap-3 text-sm text-muted">
            正在回复评论{" "}
            <button type="button" onClick={() => setReplyTo(null)} className="text-accent">
              取消
            </button>
          </div>
        )}
        <div className="grid gap-4 sm:grid-cols-2">
          <label className="block text-sm">
            昵称
            <input
              className="field mt-2"
              required
              maxLength={40}
              value={nickname}
              onChange={(event) => setNickname(event.target.value)}
            />
          </label>
          <label className="block text-sm">
            邮箱
            <input
              className="field mt-2"
              required
              type="email"
              value={email}
              onChange={(event) => setEmail(event.target.value)}
            />
          </label>
        </div>
        <label className="block text-sm">
          评论
          <textarea
            className="field mt-2 min-h-28"
            required
            maxLength={2000}
            value={body}
            onChange={(event) => setBody(event.target.value)}
          />
        </label>
        <div className="flex flex-wrap items-center gap-4">
          <button disabled={pending} className="button-primary" type="submit">
            <Send size={16} /> 提交评论
          </button>
          <span role="status" className="text-sm text-muted">
            {message}
          </span>
        </div>
      </form>
    </section>
  );
}
