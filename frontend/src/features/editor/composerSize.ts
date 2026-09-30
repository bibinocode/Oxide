/** 按实际换行高度伸缩输入框，超过上限时保留内部滚动；删除内容后恢复单行。 */
export function resizeComposer(input: HTMLTextAreaElement) {
  input.style.height = "0px";
  const maximum = Number.parseFloat(getComputedStyle(input).maxHeight) || 180;
  const height = input.scrollHeight;
  input.style.height = Math.min(height, maximum) + "px";
  input.style.overflowY = height > maximum ? "auto" : "hidden";
}

/** 仅监听宽度变化，避免设置高度再次触发观察器形成反馈循环。 */
export function observeComposer(input: HTMLTextAreaElement, onResize: () => void) {
  let width = -1;
  const observer = new ResizeObserver(([entry]) => {
    if (!entry || entry.contentRect.width === width) return;
    width = entry.contentRect.width;
    resizeComposer(input);
    onResize();
  });
  observer.observe(input);
  return () => observer.disconnect();
}

/** 菜单进入浏览器顶层，避免被编辑器或侧栏的滚动容器裁剪。 */
export function positionComposerMenu(details: HTMLDetailsElement) {
  const menu = details.querySelector<HTMLElement>(".writing-composer-options");
  const trigger = details.querySelector("summary");
  if (!menu || !trigger) return;
  if (!details.open) {
    menu.hidePopover();
    return;
  }
  menu.showPopover();
  const anchor = trigger.getBoundingClientRect();
  menu.style.left = Math.max(8, Math.min(anchor.left, innerWidth - menu.offsetWidth - 8)) + "px";
  menu.style.top = Math.max(8, anchor.top - menu.offsetHeight - 8) + "px";
}
