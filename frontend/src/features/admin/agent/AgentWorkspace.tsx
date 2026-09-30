import { useState } from "react";
import { useBlocker } from "@tanstack/react-router";
import { Bot, Layers3, SlidersHorizontal, Wrench } from "lucide-react";
import { AgentSettings } from "./AgentSettings";
import { SkillManager } from "./SkillManager";
import { AgentTools } from "./AgentTools";

type AgentTab = "skills" | "models" | "tools";

/** Skill、模型与工具共用配置入口，统一保护尚未保存的编辑。 */
export function AgentWorkspace({
  tab,
  onTabChange,
}: {
  tab: AgentTab;
  onTabChange: (tab: AgentTab) => void;
}) {
  const [dirty, setDirty] = useState(false);
  useBlocker({
    shouldBlockFn: () => dirty && !window.confirm("Agent 配置有未保存的修改，确定离开？"),
    enableBeforeUnload: dirty,
  });
  return (
    <section className="agent-workspace">
      <header className="agent-workspace-header">
        <div>
          <p className="eyebrow">AGENT STUDIO</p>
          <h1 className="mt-2 flex items-center gap-2 text-2xl font-semibold">
            <Bot size={24} /> Agent 配置
          </h1>
          <p className="mt-3 text-sm text-muted">
            安装完整技能包，统一配置模型、任务权限与 Agent 工具。
          </p>
        </div>
        <span className="agent-label">文件型 Skill · 按需加载</span>
      </header>
      <nav className="agent-tabs" aria-label="Agent 配置分类">
        {(
          [
            ["skills", "Skills", Layers3],
            ["models", "模型与任务", SlidersHorizontal],
            ["tools", "工具", Wrench],
          ] as const
        ).map(([key, label, Icon]) => (
          <button
            key={key}
            type="button"
            aria-current={tab === key ? "page" : undefined}
            className={tab === key ? "button-primary" : "button-secondary"}
            onClick={() => onTabChange(key)}
          >
            <Icon size={16} />
            {label}
          </button>
        ))}
      </nav>
      {tab === "skills" && <SkillManager onDirtyChange={setDirty} />}
      {tab === "models" && <AgentSettings />}
      {tab === "tools" && <AgentTools onDirtyChange={setDirty} />}
    </section>
  );
}
