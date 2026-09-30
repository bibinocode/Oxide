import { useEffect, useState } from "react";
import { apiRequest } from "../../../../lib/api/client";
import type { SkillCatalog } from "../../../../lib/api/types";

const endpoint = "/api/v1/admin/agent/skills";

/** 扫描技能目录，并在重新扫描或卸载后维护目录列表。 */
export function useSkillCatalog() {
  const [catalog, setCatalog] = useState<SkillCatalog>({ skills: [], warnings: [] });
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState("");
  const [reload, setReload] = useState(0);

  useEffect(() => {
    const controller = new AbortController();
    setLoading(true);
    setLoadError("");
    apiRequest<SkillCatalog>(endpoint, { signal: controller.signal })
      .then((next) => {
        if (!controller.signal.aborted) setCatalog(next);
      })
      .catch((error: unknown) => {
        if (!controller.signal.aborted)
          setLoadError(error instanceof Error ? error.message : "技能目录加载失败");
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false);
      });
    return () => controller.abort();
  }, [reload]);

  return { catalog, setCatalog, loading, loadError, rescan: () => setReload((value) => value + 1) };
}
