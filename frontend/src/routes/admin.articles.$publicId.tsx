import { createFileRoute } from "@tanstack/react-router";
import { ArticleEditor } from "../features/editor/components/ArticleEditor";

export const Route = createFileRoute("/admin/articles/$publicId")({
  component: () => <ArticleEditor publicId={Route.useParams().publicId} />,
});
