import { createFileRoute } from "@tanstack/react-router";
import { getColumns } from "../lib/api/server";
import { PublicPageHeader } from "../components/layout/PublicPageHeader";
import { ColumnBookshelf } from "../features/columns/components/ColumnBookshelf";

export const Route = createFileRoute("/columns/")({
  loader: () => getColumns(),
  head: () => ({ meta: [{ title: "小册 · Oxide" }] }),
  component: ColumnsPage,
});

/** 专栏目录只展示公开元数据，读者无需登录即可比较内容。 */
function ColumnsPage() {
  const columns = Route.useLoaderData();
  return (
    <main className="reading-width pt-10 md:pt-14">
      <PublicPageHeader title="小册" />
      <div className="mt-10">
        <ColumnBookshelf columns={columns} />
      </div>
    </main>
  );
}
