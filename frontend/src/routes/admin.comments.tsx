import { createFileRoute } from "@tanstack/react-router";
import { CommentManagement } from "../features/admin/comments/CommentManagement";
export const Route = createFileRoute("/admin/comments")({ component: CommentManagement });
