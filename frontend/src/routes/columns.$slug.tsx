import { useEffect, useState } from "react";
import { Link, createFileRoute } from "@tanstack/react-router";
import { QRCodeCanvas } from "qrcode.react";
import { ArrowLeft, LockKeyhole, X } from "lucide-react";
import { apiRequest } from "../lib/api/client";
import type { CheckoutOrder, ReaderSession } from "../lib/api/types";
import { getColumnData } from "../lib/api/server";

export const Route = createFileRoute("/columns/$slug")({
  loader: ({ params }) => getColumnData({ data: params.slug }),
  head: ({ loaderData }) => ({
    meta: [{ title: loaderData ? `${loaderData.title} · Oxide` : "小册 · Oxide" }],
  }),
  component: ColumnPage,
});

/** 微信通知入库后才刷新阅读权益，二维码本身不代表支付成功。 */
function ColumnPage() {
  const column = Route.useLoaderData();
  const [reader, setReader] = useState<ReaderSession | null>(null);
  const [order, setOrder] = useState<CheckoutOrder | null>(null);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState("");

  useEffect(() => {
    apiRequest<ReaderSession>("/api/v1/reader/session")
      .then(setReader)
      .catch(() => setReader(null));
  }, []);

  useEffect(() => {
    if (!order) return;
    const timer = window.setInterval(() => {
      apiRequest<{ status: string }>(`/api/v1/reader/orders/${order.order_id}`)
        .then((value) => {
          if (value.status === "paid") window.location.reload();
        })
        .catch(() => undefined);
    }, 3000);
    return () => window.clearInterval(timer);
  }, [order]);

  async function checkout() {
    if (!reader) return;
    setPending(true);
    setError("");
    try {
      setOrder(
        await apiRequest<CheckoutOrder>(
          `/api/v1/columns/${encodeURIComponent(column.slug)}/checkout`,
          {
            method: "POST",
            headers: { "X-CSRF-Token": reader.csrf_token },
          },
        ),
      );
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "无法创建订单");
    } finally {
      setPending(false);
    }
  }

  return (
    <main className="reading-width pt-10 md:pt-14">
      <Link
        to="/columns"
        className="inline-flex items-center gap-2 text-xs text-muted hover:text-ink"
      >
        <ArrowLeft size={14} /> 小册
      </Link>
      <header className="column-heading">
        <p className="eyebrow">Collection</p>
        <h1>{column.title}</h1>
        <p>{column.description}</p>
        <div className="column-purchase">
          <strong>¥{(column.price_cents / 100).toFixed(2)}</strong>
          {column.subscribed ? (
            <span role="status">已订阅 · 可阅读全文</span>
          ) : !column.payment_available ? (
            <button type="button" className="button-secondary" disabled>
              暂未开放订阅
            </button>
          ) : reader ? (
            <button type="button" className="button-primary" disabled={pending} onClick={checkout}>
              {pending ? "正在创建订单…" : "微信扫码订阅"}
            </button>
          ) : (
            <Link
              to="/reader"
              search={{ next: `/columns/${column.slug}` }}
              className="button-primary"
            >
              登录后订阅
            </Link>
          )}
        </div>
        {error && (
          <p role="alert" className="text-sm text-warm">
            {error}
          </p>
        )}
      </header>
      <section aria-label="小册目录" className="column-chapters">
        <h2>
          目录 <span>{column.articles.length} 篇</span>
        </h2>
        {column.articles.map((article, index) => (
          <Link
            key={article.public_id}
            to="/articles/$slug"
            params={{ slug: article.slug }}
            className="column-chapter"
          >
            <span className="column-chapter-number">{String(index + 1).padStart(2, "0")}</span>
            <span>
              <strong>{article.title}</strong>
              {article.summary && <small>{article.summary}</small>}
            </span>
            {article.subscriber_only && !column.subscribed ? (
              <span className="column-free-label">
                <LockKeyhole size={15} aria-hidden="true" /> 试看 30%
              </span>
            ) : (
              <span className="column-free-label">
                {article.subscriber_only ? "已解锁" : "免费全文"}
              </span>
            )}
          </Link>
        ))}
      </section>
      {order && (
        <div
          className="column-checkout-backdrop"
          role="presentation"
          onClick={() => setOrder(null)}
        >
          <section
            className="column-checkout"
            role="dialog"
            aria-modal="true"
            aria-label="微信扫码付款"
            onClick={(event) => event.stopPropagation()}
          >
            <button
              type="button"
              className="column-checkout-close"
              title="关闭"
              aria-label="关闭"
              onClick={() => setOrder(null)}
            >
              <X size={18} />
            </button>
            <h2>微信扫码付款</h2>
            <p>
              {column.title} · ¥{(column.price_cents / 100).toFixed(2)}
            </p>
            <QRCodeCanvas value={order.code_url} size={216} marginSize={2} />
            <p role="status">等待微信确认付款</p>
          </section>
        </div>
      )}
    </main>
  );
}
