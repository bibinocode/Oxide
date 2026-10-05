import { useEffect, useRef, useState, type CSSProperties } from "react";
import { Link } from "@tanstack/react-router";
import { ArrowUpRight } from "lucide-react";
import { Euler, Vector3 } from "three";
import type { ColumnSummary } from "../../../lib/api/types";

const BOOK_HEIGHT = 210;
const COVER_WIDTH = 148;
const SPINE_WIDTH = 24;
const SHELF_GAP = 1;
const DURATION = 650;
const BOOKS_PER_SHELF = 8;

/** 单本书的投影位置；与封面、书脊的 CSS 变换共用同一旋转顺序。 */
interface BookPose {
  left: number;
  width: number;
  offset: number;
  rotation: number;
  tilt: number;
}

/** 依据标题得到稳定的轻微倾斜，刷新或切换视图不会改变书本朝向。 */
function baseLean(index: number, title: string): number {
  let hash = 0;
  for (let i = 0; i < title.length; i++) hash = (hash * 31 + title.charCodeAt(i)) | 0;
  return (0.65 + (Math.abs(hash) % 90) / 100) * (index % 2 ? -1 : 1);
}

/** 相邻书本向展开书本两侧倾斜，角度随距离衰减。 */
function targetTilts(columns: ColumnSummary[], open: number): number[] {
  return columns.map((column, index) => {
    if (index === open) return 0;
    const distance = Math.abs(index - open);
    const angle =
      Math.abs(baseLean(index, column.title)) * Math.max(0.7, 1 - 0.04 * distance) +
      Math.max(0, 1.85 - 0.26 * distance);
    return Math.min(3.4, angle) * (index < open ? -1 : 1);
  });
}

/** Three.js 投影封面与书脊角点，避免旋转后重叠；动画只更新变换，不触发布局。 */
function shelfPoses(progress: number[], tilts: number[]): BookPose[] {
  let left = 0;
  const point = new Vector3();
  const rotation = new Euler();
  return progress.map((value, index) => {
    const rotateY = 90 * (1 - value);
    const tilt = tilts[index] * (1 - value);
    rotation.set(0, (rotateY * Math.PI) / 180, (tilt * Math.PI) / 180, "ZYX");
    let min = Infinity;
    let max = -Infinity;
    for (const [depth, width] of [
      [0, SPINE_WIDTH],
      [SPINE_WIDTH, COVER_WIDTH],
    ]) {
      for (const x of [0, width]) {
        for (const y of [-BOOK_HEIGHT / 2, BOOK_HEIGHT / 2]) {
          point.set(x, y, depth).applyEuler(rotation);
          min = Math.min(min, point.x);
          max = Math.max(max, point.x);
        }
      }
    }
    const pose = { left, width: max - min, offset: -min, rotation: rotateY, tilt };
    left += pose.width + SHELF_GAP;
    return pose;
  });
}

/** 单层手风琴书架：点击书脊展开，键盘左右键切换，文字链接进入小册。 */
function Shelf({ columns, showDetails }: { columns: ColumnSummary[]; showDetails: boolean }) {
  const [open, setOpen] = useState(0);
  const viewport = useRef<HTMLDivElement>(null);
  const frames = useRef<Array<HTMLLIElement | null>>([]);
  const controls = useRef<Array<HTMLButtonElement | null>>([]);
  const inners = useRef<Array<HTMLSpanElement | null>>([]);
  const current = useRef<{ progress: number[]; tilts: number[] }>({
    progress: columns.map((_, index) => (index === 0 ? 1 : 0)),
    tilts: targetTilts(columns, 0),
  });
  const initial = shelfPoses(current.current.progress, current.current.tilts);
  // 预留所有展开状态的最大宽度，切换小册时层板和下方信息保持固定尺寸。
  const maxWidth = Math.max(
    ...columns.map((_, index) => {
      const poses = shelfPoses(
        columns.map((__, i) => (i === index ? 1 : 0)),
        targetTilts(columns, index),
      );
      const last = poses[poses.length - 1];
      return last.left + last.width;
    }),
  );

  useEffect(() => {
    const node = viewport.current;
    if (!node) return;
    const resize = () =>
      node.style.setProperty("--bookshelf-scale", String(Math.min(1, node.clientWidth / maxWidth)));
    resize();
    const observer = new ResizeObserver(resize);
    observer.observe(node);
    return () => observer.disconnect();
  }, [maxWidth]);

  useEffect(() => {
    const from = current.current;
    const target = {
      progress: columns.map((_, index) => (index === open ? 1 : 0)),
      tilts: targetTilts(columns, open),
    };
    const reduced = window.matchMedia("(prefers-reduced-motion: reduce)");
    const started = performance.now();
    let frame = 0;
    const tick = (now: number) => {
      const elapsed = reduced.matches ? 1 : Math.min(1, (now - started) / DURATION);
      const eased = elapsed < 0.5 ? 4 * elapsed ** 3 : 1 - (-2 * elapsed + 2) ** 3 / 2;
      const progress = from.progress.map(
        (value, i) => value + (target.progress[i] - value) * eased,
      );
      const tilts = from.tilts.map((value, i) => value + (target.tilts[i] - value) * eased);
      current.current = { progress, tilts };
      shelfPoses(progress, tilts).forEach((pose, index) => {
        const node = frames.current[index];
        const inner = inners.current[index];
        if (!node || !inner) return;
        node.style.transform = `translateX(${pose.left - index * (SPINE_WIDTH + SHELF_GAP)}px)`;
        node.style.zIndex = progress[index] > 0.01 ? "2" : "1";
        node.style.setProperty("--book-contact-scale", String(pose.width / COVER_WIDTH));
        node.style.setProperty("--book-projected-width", `${pose.width}px`);
        inner.style.transform = `translateX(${pose.offset}px) rotate(${pose.tilt}deg) rotateY(${pose.rotation}deg)`;
      });
      if (elapsed < 1) frame = requestAnimationFrame(tick);
    };
    frame = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(frame);
  }, [columns, open]);

  const selected = columns[open];
  /** 统一处理鼠标与键盘选书，重复选择当前小册时不更新状态。 */
  function selectBook(index: number) {
    if (index !== open) {
      setOpen(index);
    }
  }
  return (
    <div className="column-bookshelf">
      <div
        ref={viewport}
        className="bookshelf-viewport"
        style={{ "--bookshelf-layout-width": `${maxWidth}px` } as CSSProperties}
      >
        <ul
          className="shelf3"
          aria-label="小册书架"
          onClickCapture={(event) => {
            // 书脊的触摸热区可能交叠，按真实投影边界分配点击；键盘保留原生激活行为。
            if (
              event.detail === 0 ||
              !(event.target instanceof Element) ||
              !event.target.closest(".book3")
            )
              return;
            const shelf = event.currentTarget;
            const scale = shelf.getBoundingClientRect().width / shelf.offsetWidth || 1;
            const x = (event.clientX - shelf.getBoundingClientRect().left) / scale;
            let nearest = 0;
            let distance = Infinity;
            shelfPoses(current.current.progress, current.current.tilts).forEach((pose, index) => {
              const delta = Math.max(pose.left - x, x - pose.left - pose.width, 0);
              if (delta < distance) {
                nearest = index;
                distance = delta;
              }
            });
            event.preventDefault();
            event.stopPropagation();
            selectBook(nearest);
            controls.current[nearest]?.focus();
          }}
        >
          {columns.map((column, index) => {
            const pose = initial[index];
            return (
              <li
                key={column.public_id}
                ref={(node) => {
                  frames.current[index] = node;
                }}
                className="book3-frame"
                style={
                  {
                    transform: `translateX(${pose.left - index * (SPINE_WIDTH + SHELF_GAP)}px)`,
                    zIndex: index === open ? 2 : 1,
                    "--book-contact-scale": pose.width / COVER_WIDTH,
                    "--book-projected-width": `${pose.width}px`,
                    "--book-color": `var(--book-color-${index % 6})`,
                    "--book-ink": `var(--book-ink-${index % 6})`,
                  } as CSSProperties
                }
              >
                <span className="book3-contact-shadow" aria-hidden="true" />
                <button
                  ref={(node) => {
                    controls.current[index] = node;
                  }}
                  type="button"
                  className="book3"
                  aria-label={column.title}
                  title={column.title}
                  aria-pressed={index === open}
                  tabIndex={index === open ? 0 : -1}
                  data-open={index === open || undefined}
                  onClick={() => selectBook(index)}
                  onKeyDown={(event) => {
                    let next = index;
                    if (event.key === "ArrowLeft")
                      next = (index - 1 + columns.length) % columns.length;
                    else if (event.key === "ArrowRight") next = (index + 1) % columns.length;
                    else if (event.key === "Home") next = 0;
                    else if (event.key === "End") next = columns.length - 1;
                    else return;
                    event.preventDefault();
                    selectBook(next);
                    controls.current[next]?.focus();
                  }}
                >
                  <span
                    ref={(node) => {
                      inners.current[index] = node;
                    }}
                    className="book3-inner"
                    style={{
                      transform: `translateX(${pose.offset}px) rotate(${pose.tilt}deg) rotateY(${pose.rotation}deg)`,
                    }}
                  >
                    <span className="book3-cover">
                      <span className="book3-cover-blank">
                        <span className="book3-cover-number">
                          {String(index + 1).padStart(2, "0")} / 小册
                        </span>
                        <strong>{column.title}</strong>
                        <span className="book3-cover-imprint">OXIDE</span>
                      </span>
                    </span>
                    <span className="book3-spine">
                      <span className="book3-spine-title">{column.title}</span>
                      <span className="book3-spine-author">OXIDE</span>
                    </span>
                  </span>
                </button>
              </li>
            );
          })}
        </ul>
      </div>
      <span className="room-shelf-plank" aria-hidden="true" />
      <div className={`bookshelf-caption${showDetails ? "" : " bookshelf-caption-compact"}`}>
        <Link to="/columns/$slug" params={{ slug: selected.slug }} className="shelf-annotation">
          <span>{selected.title}</span>
          <ArrowUpRight size={14} aria-hidden="true" />
        </Link>
        {showDetails && (
          <>
            <p className="bookshelf-description">{selected.description}</p>
            <span className="bookshelf-price">¥{(selected.price_cents / 100).toFixed(2)}</span>
          </>
        )}
      </div>
    </div>
  );
}

/** 按层展示真实小册，每层最多八本，避免长目录缩小到难以点击的尺寸。 */
export function ColumnBookshelf({
  columns,
  showDetails = true,
}: {
  columns: ColumnSummary[];
  /** 首页只保留名称入口，目录页可展示简介和价格。 */
  showDetails?: boolean;
}) {
  if (!columns.length) return <p className="py-6 text-sm text-muted">暂无小册。</p>;
  const shelves: ColumnSummary[][] = [];
  for (let start = 0; start < columns.length; start += BOOKS_PER_SHELF) {
    shelves.push(columns.slice(start, start + BOOKS_PER_SHELF));
  }
  return (
    <div className="column-bookshelves">
      {shelves.map((items) => (
        <Shelf
          key={items.map((item) => item.public_id).join(",")}
          columns={items}
          showDetails={showDetails}
        />
      ))}
    </div>
  );
}
