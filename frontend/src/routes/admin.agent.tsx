import { createFileRoute } from "@tanstack/react-router";
import { AgentWorkspace } from "../features/admin/agent/AgentWorkspace";

/** 独立 Agent 工作区；保留可直接分享的配置分类地址。 */
export const Route = createFileRoute("/admin/agent")({
  validateSearch: (search: Record<string, unknown>): { tab: "skills" | "models" | "tools" } => ({
    tab: search.tab === "models" || search.tab === "tools" ? search.tab : "skills",
  }),
  component: AgentPage,
});

function AgentPage() {
  const { tab } = Route.useSearch();
  const navigate = Route.useNavigate();
  return (
    <AgentWorkspace tab={tab} onTabChange={(next) => void navigate({ search: { tab: next } })} />
  );
}
