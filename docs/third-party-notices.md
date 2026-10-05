# 参考代码与许可

文章排版、编号小节及相纸封面样式参考并改编自 [CaliCastle/cali.so](https://github.com/CaliCastle/cali.so/tree/dev) 的 `app/globals.css`、`docs/design-language.md` 和 `app/_views/blog-post-page.tsx`。没有复制其个人文章、肖像或封面图片。

小册书架的木质层板、立体书脊、手风琴选择与投影布局改编自同仓库的 `components/bookshelf.tsx` 和 `app/globals.css` 中 Room shelves / Bookshelf 样式。书名、简介、价格使用本站数据；排版封面不使用上游图书图片。

底部导航的名称与连续按键提示、偏好面板的音效开关参考同仓库 `components/dock.tsx`、`components/preferences.tsx` 与 `app/globals.css`。本站直接使用参考项目同版本的 MIT 音效库 `cuelume@0.2.2`，导航使用 `chime`、偏好切换使用 `success`，音效由 Web Audio 实时合成。

页面微暖灰底色与上下弧形标尺参考同仓库 `app/globals.css` 和 `components/arc-rulers.tsx`，本站使用语义 token 并适配视口尺寸。

独立项目索引的三列列表与移动端布局参考 `app/_views/projects-page.tsx` 和项目列表样式；本站展示管理员维护的项目，使用 Lucide 图标。

项目页圆规线稿、悬停网格与定位线改编自 `components/ghost-schematic.tsx`、`components/hidden-list-stage.tsx`、`components/projects-blueprint-field.tsx` 和配套 CSS。动态图形使用相同 FlowField/Grid 参数；项目图标由管理员配置，不使用上游品牌图片。

文章列表的叠纸缩略图尺寸、偏转角度与圆角改编自 `components/post-row.tsx` 及 `app/globals.css` 的 print-pile 样式。本站使用独立实现的 Canvas 有序抖动与悬停显露原图，不复制上游文章图片。

文章主图与标题的共享过渡、相片柔焦显影及正文分阶段入场参考 `components/post-transition-link.tsx` 和 `app/globals.css` 的 route morph / enter-develop 样式。路由导航使用 TanStack Router 的原生 View Transitions 接口，并提供减少动态效果与浏览器兼容回退。

首页写作列表的中心向两侧错开入场，改编自 `app/_views/home-page.tsx` 和 `app/globals.css` 的 enter-swing 样式；返回首页时按相同节奏重新播放。

首页内容统计的三栏分割、纸张扇形、施工工具图标与悬停动画改编自 `components/nav-cards.tsx` 和 `app/globals.css`。中间入口由照片改为本站小册，采用原创书本扇形图标；统计数量来自本站公开内容，不复制上游个人数据或照片。

页面分割线遵循上游 `--border-hairline` 的分辨率规则：普通屏幕 1px，高密度屏幕 0.5px；浅色与暗色采用相同的语义边线色阶。小册书脊按上游默认 24px 展示。

MIT License


Copyright (c) 2022-2026 Cali Castle

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.

上游许可不覆盖个人文章、摄影、插画、身份标识与第三方素材。本站主图由管理员上传；AI 生成图片接入后亦进入本站素材库。

## Cuelume 音效库

来源：[cuelume](https://www.npmjs.com/package/cuelume)，采用 MIT 许可。

MIT License

Copyright (c) 2026 Daniel Belyi

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
