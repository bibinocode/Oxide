/** 归档、素材目录与日期标签统一按站点时区展示，避免 SSR 与浏览器跨年分组不一致。 */
export const SITE_TIME_ZONE = "Asia/Shanghai";

const calendar = new Intl.DateTimeFormat("en-CA", {
  timeZone: SITE_TIME_ZONE,
  year: "numeric",
  month: "2-digit",
});

/** 保留组内输入顺序；年份、月份使用固定宽度标识，缺失日期单独置于末尾。 */
export interface DateGroup<T> {
  key: string;
  label: string;
  items: T[];
}

/** 按已发布或上传时间分组，与是否关联主题分类无关。 */
export function groupByDate<T>(
  items: readonly T[],
  dateOf: (item: T) => string | null,
  precision: "year" | "month",
): DateGroup<T>[] {
  const groups = new Map<string, DateGroup<T>>();
  for (const item of items) {
    const value = dateOf(item);
    const date = value ? new Date(value) : null;
    let key = "undated";
    let label = "日期未定";
    if (date && Number.isFinite(date.getTime())) {
      const parts = calendar.formatToParts(date);
      const year = parts.find((part) => part.type === "year")!.value;
      const month = parts.find((part) => part.type === "month")!.value;
      key = precision === "year" ? year : `${year}-${month}`;
      label = precision === "year" ? year : `${year} 年 ${month} 月`;
    }
    const group = groups.get(key);
    if (group) group.items.push(item);
    else groups.set(key, { key, label, items: [item] });
  }
  return [...groups.values()].sort((a, b) => {
    if (a.key === "undated") return 1;
    if (b.key === "undated") return -1;
    return b.key.localeCompare(a.key);
  });
}
