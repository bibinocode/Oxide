import { createFileRoute } from "@tanstack/react-router";
import { Check, X } from "lucide-react";
import { useEffect, useState } from "react";
import { csrfHeaders, useAdminSession } from "../features/admin/AdminSession";
import { apiRequest } from "../lib/api/client";

interface AdminComment {
  public_id: string;
  article_public_id: string;
  nickname: string;
  body: string;
  status: "pending" | "approved" | "rejected";
  created_at: string;
}

export const Route = createFileRoute("/admin/comments")({ component: AdminComments });

function AdminComments() {
  const { session } = useAdminSession();
  const [comments, setComments] = useState<AdminComment[]>([]);
  const [error, setError] = useState("");

  useEffect(() => {
    apiRequest<AdminComment[]>("/api/v1/admin/comments")
      .then(setComments)
      .catch((cause) => setError(cause.message));
  }, []);

  async function review(publicId: string, status: "approved" | "rejected") {
    if (!session) return;
    try {
      const updated = await apiRequest<AdminComment>(`/api/v1/admin/comments/${publicId}`, {
        method: "PATCH",
        headers: csrfHeaders(session),
        body: JSON.stringify({ status }),
      });
      setComments((previous) =>
        previous.map((comment) => (comment.public_id === publicId ? updated : comment)),
      );
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "审核失败");
    }
  }

  return (
    <section className="pt-9">
      <p className="eyebrow">Moderation</p>
      <h2 className="mt-2 text-base font-semibold">评论审核</h2>
      <p role="alert" className="mt-3 text-sm text-warm">
        {error}
      </p>
      <div className="mt-8 border-t border-line">
        {comments.map((comment) => (
          <article
            key={comment.public_id}
            className="grid gap-4 border-b border-line py-5 md:grid-cols-[170px_1fr_140px]"
          >
            <div>
              <strong className="text-sm">{comment.nickname}</strong>
              <p className="mt-2 text-xs text-muted">
                {new Intl.DateTimeFormat("zh-CN").format(new Date(comment.created_at))}
              </p>
            </div>
            <p className="whitespace-pre-wrap break-words text-sm leading-7">{comment.body}</p>
            <div className="flex items-start gap-2">
              {comment.status === "pending" ? (
                <>
                  <button
                    type="button"
                    title="通过"
                    aria-label="通过"
                    onClick={() => review(comment.public_id, "approved")}
                    className="button-secondary !p-2"
                  >
                    <Check size={16} />
                  </button>
                  <button
                    type="button"
                    title="拒绝"
                    aria-label="拒绝"
                    onClick={() => review(comment.public_id, "rejected")}
                    className="button-secondary !p-2"
                  >
                    <X size={16} />
                  </button>
                </>
              ) : (
                <span className="text-sm text-muted">
                  {comment.status === "approved" ? "已通过" : "已拒绝"}
                </span>
              )}
            </div>
          </article>
        ))}
        {comments.length === 0 && !error && <p className="py-10 text-sm text-muted">暂无评论。</p>}
      </div>
    </section>
  );
}
