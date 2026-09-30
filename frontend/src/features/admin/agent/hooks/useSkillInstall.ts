import { useState, type FormEvent } from "react";
import { csrfHeaders } from "../../AdminSession";
import { apiRequest } from "../../../../lib/api/client";
import type { Session, SkillDetail } from "../../../../lib/api/types";

const endpoint = "/api/v1/admin/agent/skills";
type InstallMode = "zip" | "folder" | "github";

interface SkillInstallOptions {
  session: Session | null;
  locked: boolean;
  onInstalled: (detail: SkillDetail) => void;
  notify: (message: string, failed?: boolean) => void;
}

/** 管理第三方技能包安装表单及其提交，不修改技能目录内的文件内容。 */
export function useSkillInstall({ session, locked, onInstalled, notify }: SkillInstallOptions) {
  const [mode, setMode] = useState<InstallMode>("zip");
  const [files, setFiles] = useState<File[]>([]);
  const [githubUrl, setGithubUrl] = useState("");
  const [revision, setRevision] = useState("");
  const [skillPath, setSkillPath] = useState("");
  const [overwrite, setOverwrite] = useState(false);
  const [submitting, setSubmitting] = useState(false);

  async function install(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!session || locked || submitting) return;
    if (overwrite && !window.confirm("替换会覆盖同名技能的全部文件，并恢复为禁用状态。确定继续？"))
      return;
    setSubmitting(true);
    notify("");
    try {
      let next: SkillDetail;
      if (mode === "github") {
        next = await apiRequest<SkillDetail>(endpoint + "/install/github", {
          method: "POST",
          headers: csrfHeaders(session),
          body: JSON.stringify({
            url: githubUrl,
            revision: revision || null,
            skill_path: skillPath,
            overwrite,
          }),
        });
      } else {
        if (!files.length || (mode === "zip" && files.length !== 1))
          throw new Error("请选择一个 ZIP 或完整技能文件夹");
        const total = files.reduce((sum, file) => sum + file.size, 0);
        if (total > (mode === "zip" ? 32 : 128) * 1048576) throw new Error("文件超过上传大小预算");
        const form = new FormData();
        form.append("skill_path", skillPath);
        form.append("overwrite", String(overwrite));
        for (const file of files)
          form.append(
            mode === "zip" ? "file" : "files",
            file,
            mode === "zip" ? file.name : file.webkitRelativePath || file.name,
          );
        next = await apiRequest<SkillDetail>(endpoint + "/install", {
          method: "POST",
          headers: csrfHeaders(session),
          body: form,
        });
      }
      onInstalled(next);
      setFiles([]);
      setOverwrite(false);
      notify("技能包已安装，保留 " + next.files.length + " 个文件；启用后供模型按需加载");
    } catch (error) {
      notify(error instanceof Error ? error.message : "安装失败", true);
    } finally {
      setSubmitting(false);
    }
  }

  return {
    form: { mode, files, githubUrl, revision, skillPath, overwrite },
    update: { setMode, setFiles, setGithubUrl, setRevision, setSkillPath, setOverwrite },
    submitting,
    install,
  };
}
