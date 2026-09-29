import { useRef } from "react";
import { useScrollReveal } from "../../features/article/hooks/useScrollReveal";

/** 列表内容按行进入视口，首屏行维持直接显示。 */
export function RevealContent({
  children,
  className,
}: {
  children: React.ReactNode;
  className?: string;
}) {
  const ref = useRef<HTMLDivElement>(null);
  useScrollReveal(ref, children);
  return (
    <div ref={ref} className={className}>
      {children}
    </div>
  );
}
