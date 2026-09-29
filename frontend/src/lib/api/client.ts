import type { ApiError } from "./types";

export class ApiRequestError extends Error {
  constructor(
    message: string,
    readonly code: string,
    readonly status: number,
  ) {
    super(message);
  }
}

/** 浏览器写请求使用 Vite/部署反向代理的同源路径。 */
export async function apiRequest<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(path, {
    credentials: "same-origin",
    ...init,
    headers: {
      ...(init?.body instanceof FormData ? {} : { "Content-Type": "application/json" }),
      ...init?.headers,
    },
  });
  if (!response.ok) {
    const error = (await response
      .json()
      .catch(() => ({ code: "http_error", message: `请求失败 (${response.status})` }))) as ApiError;
    throw new ApiRequestError(error.message, error.code, response.status);
  }
  if (response.status === 204) return undefined as T;
  return response.json() as Promise<T>;
}

/** SSR 只访问内部 API 地址，不把容器地址发送给浏览器。 */
export async function serverApi<T>(path: string): Promise<T> {
  const base = process.env.API_INTERNAL_URL ?? "http://127.0.0.1:3001";
  const response = await fetch(`${base}${path}`);
  if (!response.ok) throw new Error(`API 请求失败 (${response.status})`);
  return response.json() as Promise<T>;
}
