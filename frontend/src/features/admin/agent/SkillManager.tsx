import { useEffect, useRef, useState } from "react";
import {
  Download,
  FileCode2,
  FolderOpen,
  Package,
  Plus,
  Search,
  Trash2,
  Upload,
} from "lucide-react";
import { csrfHeaders, useAdminSession } from "../AdminSession";
import { apiRequest } from "../../../lib/api/client";
import type { SkillDetail, SkillSummary } from "../../../lib/api/types";
import { useSkillCatalog } from "./hooks/useSkillCatalog";
import { useSkillInstall } from "./hooks/useSkillInstall";

const endpoint = "/api/v1/admin/agent/skills";
function messageOf(error: unknown) {
  return error instanceof Error ? error.message : "操作失败，请重试";
}
function sizeLabel(size: number) {
  return size < 1024
    ? size + " B"
    : size < 1048576
      ? (size / 1024).toFixed(1) + " KiB"
      : (size / 1048576).toFixed(1) + " MiB";
}

/** 安装完整文件包并浏览资源；管理状态与原包内容分离，不存数据库。 */
export function SkillManager({ onDirtyChange }: { onDirtyChange: (dirty: boolean) => void }) {
  const { session } = useAdminSession();
  const { catalog, setCatalog, loading, loadError, rescan } = useSkillCatalog();
  const [detail, setDetail] = useState<SkillDetail | null>(null);
  const [document, setDocument] = useState("");
  const [filePath, setFilePath] = useState("SKILL.md");
  const [preview, setPreview] = useState("");
  const [installing, setInstalling] = useState(false);
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState("all");
  const [busy, setBusy] = useState(false);
  const [reading, setReading] = useState(false);
  const [message, setMessage] = useState("");
  const [failed, setFailed] = useState(false);
  const request = useRef<AbortController | null>(null);
  const dirty = !!detail && document !== detail.document;
  const locked = loading || busy || reading || !!loadError;
  const installation = useSkillInstall({
    session,
    locked,
    onInstalled: select,
    notify,
  });
  const { mode, files, githubUrl, revision, skillPath, overwrite } = installation.form;
  const { setMode, setFiles, setGithubUrl, setRevision, setSkillPath, setOverwrite } =
    installation.update;
  const operationLocked = locked || installation.submitting;

  useEffect(() => {
    onDirtyChange(dirty);
  }, [dirty, onDirtyChange]);
  useEffect(
    () => () => {
      onDirtyChange(false);
      request.current?.abort();
    },
    [onDirtyChange],
  );

  function notify(text: string, isError = false) {
    setMessage(text);
    setFailed(isError);
  }
  function canDiscard() {
    return !dirty || window.confirm("SKILL.md 有未保存修改，确定放弃？");
  }
  function select(next: SkillDetail) {
    setDetail(next);
    setDocument(next.document);
    setFilePath("SKILL.md");
    setPreview("");
    setInstalling(false);
    setCatalog((old) => ({
      ...old,
      skills: [...old.skills.filter((item) => item.name !== next.skill.name), next.skill].sort(
        (left, right) => left.name.localeCompare(right.name),
      ),
    }));
  }
  function startInstall() {
    if (!canDiscard()) return;
    setDocument(detail?.document ?? "");
    setInstalling(true);
    notify("");
  }
  async function open(skill: SkillSummary) {
    if (!canDiscard()) return;
    request.current?.abort();
    const controller = new AbortController();
    request.current = controller;
    setReading(true);
    notify("");
    try {
      const next = await apiRequest<SkillDetail>(endpoint + "/" + skill.name, {
        signal: controller.signal,
      });
      if (!controller.signal.aborted) select(next);
    } catch (error) {
      if (!controller.signal.aborted) notify(messageOf(error), true);
    } finally {
      if (!controller.signal.aborted) setReading(false);
    }
  }
  async function toggle(skill: SkillSummary) {
    if (!session || operationLocked) return;
    setBusy(true);
    notify("");
    try {
      await apiRequest(endpoint + "/" + skill.name + "/enabled", {
        method: "PATCH",
        headers: csrfHeaders(session),
        body: JSON.stringify({ enabled: !skill.enabled }),
      });
      setCatalog((old) => ({
        ...old,
        skills: old.skills.map((item) =>
          item.name === skill.name ? { ...item, enabled: !skill.enabled } : item,
        ),
      }));
      if (detail?.skill.name === skill.name)
        setDetail({ ...detail, skill: { ...detail.skill, enabled: !skill.enabled } });
      notify(skill.enabled ? "Skill 已停用" : "Skill 已启用，下次请求会发现技能，并按需读取文件");
    } catch (error) {
      notify(messageOf(error), true);
    } finally {
      setBusy(false);
    }
  }
  async function save() {
    if (!session || !detail || operationLocked) return;
    setBusy(true);
    notify("");
    try {
      select(
        await apiRequest<SkillDetail>(endpoint + "/" + detail.skill.name, {
          method: "PUT",
          headers: csrfHeaders(session),
          body: JSON.stringify({ document }),
        }),
      );
      notify("SKILL.md 已保存，其余包文件保持不变");
    } catch (error) {
      notify(messageOf(error), true);
    } finally {
      setBusy(false);
    }
  }
  async function remove() {
    if (
      !session ||
      !detail ||
      operationLocked ||
      !window.confirm("卸载 " + detail.skill.name + "？将删除整个技能目录及配套文件，不影响文章。")
    )
      return;
    setBusy(true);
    try {
      await apiRequest(endpoint + "/" + detail.skill.name, {
        method: "DELETE",
        headers: csrfHeaders(session),
      });
      setCatalog((old) => ({
        ...old,
        skills: old.skills.filter((item) => item.name !== detail.skill.name),
      }));
      setDetail(null);
      setDocument("");
      notify("技能包已卸载");
    } catch (error) {
      notify(messageOf(error), true);
    } finally {
      setBusy(false);
    }
  }
  async function showFile(path: string, size: number) {
    if (!detail || operationLocked) return;
    setFilePath(path);
    setPreview("");
    if (path === "SKILL.md") return;
    if (size > 262144) {
      setPreview("资源文件较大，请下载查看。文件完整保留在技能包内。");
      return;
    }
    const controller = new AbortController();
    request.current?.abort();
    request.current = controller;
    setReading(true);
    try {
      const response = await fetch(
        endpoint + "/" + detail.skill.name + "/files?path=" + encodeURIComponent(path),
        { credentials: "same-origin", signal: controller.signal },
      );
      if (!response.ok) throw new Error("文件读取失败");
      const bytes = await response.arrayBuffer();
      if (bytes.byteLength > 262144) throw new Error("资源文件较大，请下载查看");
      try {
        setPreview(new TextDecoder("utf-8", { fatal: true }).decode(bytes));
      } catch {
        setPreview("这是二进制资源，请下载查看。文件完整保留，不会被转换成文本。");
      }
    } catch (error) {
      if (!controller.signal.aborted) setPreview(messageOf(error));
    } finally {
      if (!controller.signal.aborted) setReading(false);
    }
  }
  const visible = catalog.skills.filter(
    (skill) =>
      (skill.name + " " + skill.description).toLowerCase().includes(query.toLowerCase()) &&
      (filter === "all" || (filter === "enabled" ? skill.enabled : skill.manual_only)),
  );

  return (
    <div>
      <div className="agent-skill-intro">
        <div>
          <h2 className="text-lg font-semibold">已安装的技能包</h2>
          <p className="mt-2 text-sm text-muted">
            SKILL.md + scripts / references /
            assets。文件保存在技能目录，模型先发现用途，再按需读取。
          </p>
        </div>
        <button
          type="button"
          className="button-primary"
          disabled={operationLocked}
          onClick={startInstall}
        >
          <Plus size={16} />
          安装 Skill
        </button>
      </div>
      {message && (
        <p
          role={failed ? "alert" : "status"}
          className={failed ? "agent-notice text-warm" : "agent-notice"}
        >
          {message}
        </p>
      )}
      {catalog.warnings.map((warning) => (
        <p key={warning} role="alert" className="agent-notice text-warm">
          {warning}
        </p>
      ))}
      <div className="agent-skills-grid" aria-busy={operationLocked}>
        <aside className="agent-skill-library" aria-label="已安装 Skill 列表">
          <div className="flex items-center justify-between">
            <h3 className="font-semibold">技能目录</h3>
            <span className="text-xs text-muted">
              {catalog.skills.filter((skill) => skill.enabled).length} 启用 /{" "}
              {catalog.skills.length} 个
            </span>
          </div>
          <label className="agent-search">
            <Search size={16} />
            <input
              aria-label="搜索 Skill"
              placeholder="搜索名称或使用场景"
              value={query}
              onChange={(event) => setQuery(event.target.value)}
            />
          </label>
          <select
            className="field text-sm"
            aria-label="筛选 Skill"
            value={filter}
            onChange={(event) => setFilter(event.target.value)}
          >
            <option value="all">全部技能包</option>
            <option value="enabled">已启用</option>
            <option value="manual">仅手动调用</option>
          </select>
          {loading ? (
            <p role="status" className="agent-library-message">
              正在扫描技能目录…
            </p>
          ) : loadError ? (
            <div className="agent-library-message">
              <p role="alert">{loadError}</p>
              <button type="button" className="button-secondary mt-3" onClick={rescan}>
                重新扫描
              </button>
            </div>
          ) : !visible.length ? (
            <p className="agent-library-message">
              {catalog.skills.length
                ? "没有匹配的技能"
                : "还未安装技能，可从 ZIP、文件夹或 GitHub 安装。"}
            </p>
          ) : (
            <ul className="agent-skill-list">
              {visible.map((skill) => (
                <li
                  key={skill.name}
                  className={detail?.skill.name === skill.name && !installing ? "is-selected" : ""}
                >
                  <button
                    type="button"
                    className="agent-skill-select"
                    disabled={operationLocked}
                    aria-pressed={detail?.skill.name === skill.name && !installing}
                    onClick={() => void open(skill)}
                  >
                    <span className="flex items-center gap-2 font-mono text-sm">
                      <Package size={15} />
                      {skill.name}
                    </span>
                    <span className="agent-skill-description">{skill.description}</span>
                    <span className="text-xs text-muted">
                      {skill.manual_only ? "仅 /" + skill.name + " 手动调用" : "根据用途自动发现"}
                    </span>
                  </button>
                  <button
                    type="button"
                    role="switch"
                    aria-checked={skill.enabled}
                    aria-label={"启用 " + skill.name}
                    className="agent-skill-switch"
                    disabled={operationLocked}
                    onClick={() => void toggle(skill)}
                  >
                    {skill.enabled ? "已启用" : "未启用"}
                  </button>
                </li>
              ))}
            </ul>
          )}
          <p className="mt-6 border-t border-line pt-4 text-xs leading-6 text-muted">
            文件型存储，不写数据库。新安装或替换后默认禁用。技能脚本不会因安装而自动执行。
          </p>
        </aside>
        <div className="agent-skill-editor">
          {installing ? (
            <form onSubmit={(event) => void installation.install(event)}>
              <h3 className="text-lg font-semibold">安装第三方技能包</h3>
              <p className="mt-2 text-sm text-muted">
                保留目录内所有普通文件；多技能仓库可指定要安装的子目录。
              </p>
              <fieldset disabled={operationLocked} className="agent-skill-fields">
                <div className="flex flex-wrap gap-2">
                  {(
                    [
                      ["zip", "ZIP 压缩包"],
                      ["folder", "本地文件夹"],
                      ["github", "GitHub 仓库"],
                    ] as const
                  ).map(([key, label]) => (
                    <button
                      key={key}
                      type="button"
                      aria-pressed={mode === key}
                      className={mode === key ? "button-primary" : "button-secondary"}
                      onClick={() => {
                        setMode(key);
                        setFiles([]);
                      }}
                    >
                      {label}
                    </button>
                  ))}
                </div>
                {mode === "github" ? (
                  <>
                    <label>
                      GitHub 地址
                      <input
                        type="url"
                        required
                        className="field mt-2"
                        placeholder="https://github.com/owner/repo/tree/main/skills/example"
                        value={githubUrl}
                        onChange={(event) => setGithubUrl(event.target.value)}
                      />
                    </label>
                    <label>
                      分支或 revision（可选）
                      <input
                        className="field mt-2"
                        placeholder="tree 链接中的分支，仓库主页默认 main"
                        value={revision}
                        onChange={(event) => setRevision(event.target.value)}
                      />
                    </label>
                  </>
                ) : (
                  <label className="agent-package-upload">
                    <Upload size={28} className="mb-3 text-muted" />
                    <span>{mode === "zip" ? "选择 ZIP 文件" : "选择完整技能文件夹"}</span>
                    <input
                      key={mode}
                      type="file"
                      required
                      accept={mode === "zip" ? ".zip" : undefined}
                      multiple={mode === "folder"}
                      {...(mode === "folder" ? { webkitdirectory: "", directory: "" } : {})}
                      onChange={(event) => setFiles(Array.from(event.target.files ?? []))}
                    />
                    <span className="text-xs text-muted">
                      {files.length
                        ? files.length +
                          " 个文件 · " +
                          sizeLabel(files.reduce((sum, file) => sum + file.size, 0))
                        : mode === "zip"
                          ? "ZIP ≤ 32 MiB，展开 ≤ 128 MiB"
                          : "保留相对路径，文件合计 ≤ 128 MiB"}
                    </span>
                  </label>
                )}
                <label>
                  技能子目录（可选）
                  <input
                    className="field mt-2 font-mono"
                    placeholder="例如 skills/pdf；单技能包留空"
                    value={skillPath}
                    onChange={(event) => setSkillPath(event.target.value)}
                  />
                </label>
                <label className="flex items-center gap-2">
                  <input
                    type="checkbox"
                    checked={overwrite}
                    onChange={(event) => setOverwrite(event.target.checked)}
                  />
                  替换已安装的同名技能（覆盖完整目录）
                </label>
                <p className="text-xs leading-6 text-muted">
                  包根目录必须包含有效 SKILL.md（YAML name、description +
                  Markdown）。scripts、references、assets 等原样保存；Shell、动态命令与 hooks
                  暂不执行。只支持公开 GitHub 仓库，不接收访问令牌。
                </p>
                <div className="agent-editor-actions">
                  <button type="submit" className="button-primary">
                    <Upload size={16} />
                    {installation.submitting ? "安装中…" : "安装技能包"}
                  </button>
                  <button
                    type="button"
                    className="button-secondary"
                    onClick={() => setInstalling(false)}
                  >
                    取消
                  </button>
                </div>
              </fieldset>
            </form>
          ) : detail ? (
            <>
              <div className="flex flex-wrap items-center justify-between gap-3">
                <h3 className="flex items-center gap-2 font-mono text-lg font-semibold">
                  <Package size={20} />
                  {detail.skill.name}
                </h3>
                <span className="agent-label">
                  {dirty ? "SKILL.md 未保存" : detail.files.length + " 个文件"}
                </span>
              </div>
              <p className="mt-3 text-sm leading-6 text-muted">{detail.skill.description}</p>
              <p className="mt-3 text-xs text-muted">
                来源：{detail.source ?? "本地目录"}
                {detail.skill.license ? " · 许可：" + detail.skill.license : ""}
              </p>
              {detail.skill.compatibility && (
                <p className="mt-2 text-xs text-muted">环境要求：{detail.skill.compatibility}</p>
              )}
              {detail.skill.warnings.map((warning) => (
                <p key={warning} className="agent-notice mt-4 text-warm">
                  {warning}
                </p>
              ))}
              <div className="agent-package-actions">
                <button
                  type="button"
                  className="button-secondary"
                  disabled={operationLocked}
                  onClick={() => void toggle(detail.skill)}
                >
                  {detail.skill.enabled ? "停用" : "启用"}技能
                </button>
                <a
                  className="button-secondary"
                  href={endpoint + "/" + detail.skill.name + "/export"}
                  download={detail.skill.name + ".zip"}
                >
                  <Download size={16} />
                  导出完整 ZIP
                </a>
                <button
                  type="button"
                  className="button-secondary text-warm"
                  disabled={operationLocked}
                  onClick={() => void remove()}
                >
                  <Trash2 size={16} />
                  卸载
                </button>
              </div>
              <div className="agent-package-files">
                <nav aria-label="包内文件">
                  <p className="mb-3 flex items-center gap-2 text-xs text-muted">
                    <FolderOpen size={14} />
                    完整包目录
                  </p>
                  {detail.files.map((file) => (
                    <button
                      key={file.path}
                      type="button"
                      disabled={operationLocked}
                      aria-current={filePath === file.path ? "page" : undefined}
                      onClick={() => void showFile(file.path, file.size)}
                    >
                      <span>
                        <FileCode2 size={13} />
                        {file.path}
                      </span>
                      <small>{sizeLabel(file.size)}</small>
                    </button>
                  ))}
                </nav>
                <div className="min-w-0">
                  <div className="mb-3 flex items-center justify-between gap-3">
                    <span className="break-all font-mono text-xs">{filePath}</span>
                    <a
                      className="text-xs underline"
                      href={
                        endpoint +
                        "/" +
                        detail.skill.name +
                        "/files?path=" +
                        encodeURIComponent(filePath)
                      }
                      download={filePath.split("/").pop()}
                    >
                      下载文件
                    </a>
                  </div>
                  {filePath === "SKILL.md" ? (
                    <>
                      <textarea
                        aria-label="SKILL.md 原文"
                        className="field agent-instructions"
                        rows={20}
                        value={document}
                        disabled={operationLocked}
                        spellCheck={false}
                        onChange={(event) => setDocument(event.target.value)}
                      />
                      <p className="mt-2 text-xs text-muted">
                        编辑原始 YAML 与 Markdown；name 必须与目录一致，附属文件保持不变。
                      </p>
                      <div className="agent-editor-actions mt-4">
                        <button
                          type="button"
                          className="button-primary"
                          disabled={operationLocked || !dirty}
                          onClick={() => void save()}
                        >
                          保存 SKILL.md
                        </button>
                        <button
                          type="button"
                          className="button-secondary"
                          disabled={operationLocked || !dirty}
                          onClick={() => {
                            if (canDiscard()) setDocument(detail.document);
                          }}
                        >
                          放弃修改
                        </button>
                      </div>
                    </>
                  ) : (
                    <pre className="agent-file-preview" aria-busy={reading}>
                      {reading ? "正在读取文件…" : preview}
                    </pre>
                  )}
                </div>
              </div>
            </>
          ) : (
            <div className="agent-empty">
              <Package size={36} className="mx-auto text-muted" />
              <h3 className="mt-5 text-lg font-semibold">安装一个完整 Skill</h3>
              <p className="mx-auto mt-3 max-w-md text-sm leading-7 text-muted">
                每个技能是带 SKILL.md 的目录，可以拥有脚本、参考资料、模板和其他资源文件。
              </p>
              <button
                type="button"
                className="button-primary mt-6"
                disabled={operationLocked}
                onClick={startInstall}
              >
                <Plus size={16} />
                安装第三方技能
              </button>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
