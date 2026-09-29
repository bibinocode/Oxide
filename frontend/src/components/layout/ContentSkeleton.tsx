/** 路由数据等待时保持文章列表和正文的尺寸，避免页面跳动。 */
export function ContentSkeleton({ article = false }: { article?: boolean }) {
  return (
    <main
      className={article ? "reading-width pt-10 md:pt-14" : "public-width pt-10 md:pt-14"}
      aria-busy="true"
      aria-label="内容加载中"
    >
      <div className="skeleton-line mb-12 h-4 w-24" />
      {article ? (
        <>
          <div className="skeleton-line h-10 w-4/5" />
          <div className="skeleton-line mt-5 h-4 w-2/5" />
          <div className="skeleton-article mt-14 space-y-4">
            {Array.from({ length: 8 }, (_, index) => (
              <div
                key={index}
                className="skeleton-line h-4"
                style={{ width: `${index % 3 === 2 ? 68 : 100}%` }}
              />
            ))}
          </div>
        </>
      ) : (
        <div className="space-y-2">
          {Array.from({ length: 6 }, (_, index) => (
            <div key={index} className="skeleton-row">
              <span className="skeleton-line h-4" style={{ width: `${68 - (index % 3) * 13}%` }} />
              <span className="skeleton-line h-3 w-12" />
            </div>
          ))}
        </div>
      )}
    </main>
  );
}
