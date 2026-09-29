import { createServerFn } from "@tanstack/react-start";

export interface SocialPreviewData {
  name?: string;
  bio?: string;
  avatarUrl?: string;
  followers?: number;
  following?: number;
  subscribers?: string;
  contributions?: { total: number; levels: number[] };
}

/** 只向服务端传账号链接，平台数据由服务端获取并缓存。 */
export const getSocialPreview = createServerFn({ method: "GET" })
  .validator((url: string) => url)
  .handler(async ({ data: url }): Promise<SocialPreviewData | null> => {
    const { fetchSocialPreview } = await import("./socialPreview.server");
    return fetchSocialPreview(url);
  });
