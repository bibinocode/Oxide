/** 参考站点的印刷标记语汇，保持每页只出现一个彩色像素。 */
export function PixelMark() {
  return (
    <span className="pixel-mark" aria-hidden="true">
      {Array.from({ length: 9 }, (_, index) => (
        <i key={index} />
      ))}
    </span>
  );
}

/** 带序号和排线的小节标题。 */
export function SectionTag({ index, children }: { index: string; children: React.ReactNode }) {
  return (
    <h2 className="section-tag">
      <span className="section-tag-index" aria-hidden="true">
        {index}
      </span>
      <span className="section-tag-hatch" aria-hidden="true" />
      <span className="section-tag-label">{children}</span>
    </h2>
  );
}
