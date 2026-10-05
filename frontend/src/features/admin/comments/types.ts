/** 管理员可见的评论与审核依据；邮箱标识不进入管理响应。 */
export interface AdminComment {
  public_id: string;
  article_public_id: string;
  article_title: string;
  nickname: string;
  body: string;
  status: "pending" | "approved" | "rejected";
  created_at: string;
  reviewed_at: string | null;
  review_source: "manual" | "agent" | null;
  agent_review: {
    decision: "approved" | "rejected" | "manual" | "error";
    reason: string;
    provider_id: string;
    model_id: string;
    at: string;
  } | null;
}
