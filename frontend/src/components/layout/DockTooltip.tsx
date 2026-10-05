/** 导航悬停及键盘聚焦提示；按键表示先按 G，再按目标字母。 */
export function DockTooltip({ label, goKey }: { label: string; goKey?: string }) {
  return (
    <span className="dock-tooltip" aria-hidden="true">
      <span>{label}</span>
      {goKey && (
        <span className="dock-tooltip-keys">
          <kbd>G</kbd>
          <kbd>{goKey}</kbd>
        </span>
      )}
    </span>
  );
}
