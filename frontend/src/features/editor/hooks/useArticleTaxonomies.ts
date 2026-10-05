import { useEffect, useState } from "react";
import { apiRequest } from "../../../lib/api/client";
import type { Taxonomy } from "../../../lib/api/types";

/** 读取发布面板的分类和标签；卸载时忽略过期响应。 */
export function useArticleTaxonomies() {
  const [categories, setCategories] = useState<Taxonomy[]>([]);
  const [tags, setTags] = useState<Taxonomy[]>([]);
  const [error, setError] = useState("");

  useEffect(() => {
    const controller = new AbortController();
    Promise.all([
      apiRequest<Taxonomy[]>("/api/v1/admin/categories", { signal: controller.signal }),
      apiRequest<Taxonomy[]>("/api/v1/admin/tags", { signal: controller.signal }),
    ])
      .then(([categoryItems, tagItems]) => {
        if (controller.signal.aborted) return;
        setCategories(categoryItems);
        setTags(tagItems);
      })
      .catch((cause: unknown) => {
        if (!controller.signal.aborted)
          setError(cause instanceof Error ? cause.message : "分类和标签加载失败");
      });
    return () => controller.abort();
  }, []);

  return { categories, tags, error };
}
