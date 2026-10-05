import { useState } from "react";
import { Link } from "@tanstack/react-router";
import { useAdminComments } from "./hooks/useAdminComments";
import type { AdminComment } from "./types";

/** 最终状态与 Agent 建议分开展示，人工能纠正自动决定。 */
function statusLabel(comment: AdminComment) {
  if (comment.status === "approved") return "已通过";
  if (comment.status === "rejected") return "已拒绝";
  if (comment.review_source === "manual") return "已隐藏";
  return comment.agent_review ? "待人工复核" : "待审核";
}
export function CommentManagement() {
  const [filter, setFilter] = useState("all");
  const { comments, page, setPage, loading, busy, error, reload, act } = useAdminComments(filter);
  const visible = comments;
  return (
    <section className="pt-9">
      <p className="eyebrow">Moderation</p>
      <h2 className="mt-2 text-base font-semibold">评论管理与审核</h2>
      <p className="mt-3 text-sm leading-7 text-muted">
        绑定评论审核模型后，Agent
        自动评审待审评论。无法确定或模型调用失败时保留待审，人工处理结果优先。
      </p>
      <div className="mt-5 flex flex-wrap items-center gap-3">
        <Link to="/admin/agent" search={{ tab: "models" }} className="button-secondary">
          配置评论审核 Agent
        </Link>
        <button
          type="button"
          className="button-secondary"
          disabled={loading || !!busy}
          onClick={reload}
        >
          刷新评论
        </button>
        <select
          aria-label="评论状态筛选"
          className="field !w-auto"
          value={filter}
          onChange={(event) => {
            setFilter(event.target.value);
            setPage(1);
          }}
        >
          <option value="all">全部评论</option>
          <option value="pending">待审核 / 复核</option>
          <option value="approved">已通过</option>
          <option value="rejected">已拒绝</option>
          <option value="hidden">已隐藏</option>
        </select>
      </div>
      {error && (
        <p role="alert" className="mt-3 text-sm text-warm">
          {error}
        </p>
      )}
      {loading ? (
        <p role="status" className="py-8 text-sm text-muted">
          正在加载评论…
        </p>
      ) : (
        <div className="mt-6 border-t border-line">
          {visible.map((comment) => (
            <article key={comment.public_id} className="border-b border-line py-5">
              <div className="flex flex-wrap items-center gap-3 text-sm">
                <strong>{comment.nickname}</strong>
                <span className="text-muted">
                  {statusLabel(comment)} ·{" "}
                  {comment.review_source === "agent"
                    ? "Agent 审核"
                    : comment.reviewed_at
                      ? "人工处理"
                      : "等待审核"}
                </span>
                <time className="text-xs text-muted">
                  {new Intl.DateTimeFormat("zh-CN").format(new Date(comment.created_at))}
                </time>
              </div>
              <Link
                to="/admin/articles/$publicId"
                params={{ publicId: comment.article_public_id }}
                className="mt-2 inline-block text-xs text-muted hover:underline"
              >
                文章：{comment.article_title}
              </Link>
              <p className="mt-3 whitespace-pre-wrap break-words text-sm leading-7">
                {comment.body}
              </p>
              {comment.agent_review && (
                <div className="mt-3 border-l border-line pl-3 text-xs leading-6 text-muted">
                  <p>
                    Agent {comment.review_source === "manual" ? "上次建议" : "审核依据"}：
                    {comment.agent_review.reason}
                  </p>
                  <p>
                    模型：{comment.agent_review.model_id}
                    {comment.review_source === "manual" && " · 最终状态已由人工处理"}
                  </p>
                </div>
              )}
              <div className="mt-4 flex flex-wrap gap-2">
                <button
                  type="button"
                  className="button-secondary"
                  disabled={!!busy || comment.status === "approved"}
                  onClick={() => void act(comment, "approved")}
                >
                  通过
                </button>
                <button
                  type="button"
                  className="button-secondary"
                  disabled={!!busy || comment.status === "rejected"}
                  onClick={() => void act(comment, "rejected")}
                >
                  拒绝
                </button>
                <button
                  type="button"
                  className="button-secondary"
                  disabled={
                    !!busy || (comment.status === "pending" && comment.review_source === "manual")
                  }
                  onClick={() => void act(comment, "pending")}
                >
                  隐藏
                </button>
                <button
                  type="button"
                  className="button-secondary"
                  disabled={!!busy}
                  onClick={() => {
                    if (
                      comment.status !== "approved" ||
                      window.confirm("重新交给 Agent 审核？审核完成前这条评论将不公开展示。")
                    )
                      void act(comment, "agent");
                  }}
                >
                  交给 Agent 审核
                </button>
                <button
                  type="button"
                  className="button-secondary text-warm"
                  disabled={!!busy}
                  onClick={() => {
                    if (window.confirm("永久删除这条评论及其回复？此操作不可撤销。"))
                      void act(comment, "delete");
                  }}
                >
                  删除
                </button>
                {busy === comment.public_id && (
                  <span role="status" className="text-xs text-muted">
                    正在处理…
                  </span>
                )}
              </div>
            </article>
          ))}
          {visible.length === 0 && <p className="py-8 text-sm text-muted">暂无符合条件的评论。</p>}
        </div>
      )}
      <div className="mt-5 flex items-center gap-3">
        <button
          type="button"
          className="button-secondary"
          disabled={loading || !!busy || page <= 1}
          onClick={() => setPage(page - 1)}
        >
          上一页
        </button>
        <span className="text-xs text-muted">第 {page} 页</span>
        <button
          type="button"
          className="button-secondary"
          disabled={loading || !!busy || comments.length < 50}
          onClick={() => setPage(page + 1)}
        >
          下一页
        </button>
      </div>
    </section>
  );
}
