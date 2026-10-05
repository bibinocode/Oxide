import { ArticleManagementList } from "../articles/ArticleManagementList";

/** 小册目录与全部文章共用分页和操作，包含草稿及已下架章节。 */
export function ColumnContents({ columnId }: { columnId: string }) {
  return <ArticleManagementList columnId={columnId} />;
}
