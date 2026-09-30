import { createFileRoute } from "@tanstack/react-router";
import { ArticleEditor } from "../features/editor/components/ArticleEditor";

export const Route = createFileRoute("/admin/articles/$publicId")({
  component: () => (
    <ArticleEditor key={Route.useParams().publicId} publicId={Route.useParams().publicId} />
  ),
});
