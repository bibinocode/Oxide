import { PixelMark } from "./PrintMarks";

/** 公开索引页共用的印刷式页眉。 */
export function PublicPageHeader({ title, detail }: { title: string; detail?: string }) {
  return (
    <header>
      <div className="flex items-center justify-between">
        <h1 className="eyebrow">{title}</h1>
        <PixelMark />
      </div>
      {detail && <p className="mt-5 text-sm text-muted">{detail}</p>}
    </header>
  );
}
