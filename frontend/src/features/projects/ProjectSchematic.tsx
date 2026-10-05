/** 参考站的圆规与尺寸线装饰，仅作为不可交互的淡背景。 */
export function ProjectSchematic({ className }: { className?: string }) {
  return (
    <svg
      className={className ? `project-schematic ${className}` : "project-schematic"}
      viewBox="0 0 260 300"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.2"
      strokeLinecap="round"
      aria-hidden="true"
      focusable="false"
    >
      <path d="M150 14v12M146 18h8M146 22h8" />
      <circle cx="150" cy="40" r="7" />

      <path d="M147 47 96 210M96 210l-3 16M153 47l40 158M193 205l6 19" />
      <path d="M126 118l45 13" />

      <path d="M180 165a106 106 0 0 1 0 122" />

      <path d="M85 226h16M93 218v16" />
      <circle cx="93" cy="226" r="4" />

      <path d="M28 262h58M28 258v8M86 258v8" />
    </svg>
  );
}
