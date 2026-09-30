---
name: react-hooks-pattern-ts
description: React 18 / 19 + TypeScript 的自定义 Hooks 编写规范，覆盖列表分页、异步竞态、派生状态、并发特性、React 19 行为变化与 Hooks 类型建模。适用于函数组件 + Hooks + TS 的前端项目。
---

# React Hooks 规范（React 18 / 19 + TypeScript）

## 适用范围

在以下情况优先抽出自定义 Hook：

- 页面内多个组件共享同一份状态或业务动作。
- 列表分页、筛选、选中行、弹窗流程、表单提交等逻辑需要封装。
- 数据获取需要处理 loading、错误、竞态、卸载清理。
- 页面入口文件（`index.tsx` / `Page.tsx`）开始堆积业务逻辑。

反向约束：纯展示组件、纯格式化函数、单个 `useState` 能解决的事情，不要抽 Hook。

## 目录约定

```text
features/xxx/
  hooks/
    useXxxList.ts        # 列表：分页 + 筛选 + 请求
    useXxxDetail.ts      # 详情
    useXxxActions.ts     # 业务动作：提交 / 删除 / 状态流转
  api/
    xxxApi.ts            # 只做请求
  types.ts               # 领域类型

src/hooks/               # 跨 feature 复用
  usePagedList.ts
  useAsync.ts
  useDebouncedValue.ts
  useLatest.ts
  useToggle.ts
```

规则：

- Hook 文件名与导出名一致，必须以 `use` 开头（否则 `rules-of-hooks` / `exhaustive-deps` 规则失效）。
- 页面私有 Hook 放在页面目录内，只有真正跨页面复用的才上提到共享目录。
- Hook 只写逻辑与状态，不写 JSX。需要渲染的复用逻辑用组件，不用 Hook。
- 一个 Hook 只做一件事；返回对象超过 10 个字段时考虑拆分。

## 列表分页 Hook

分页列表是最常见的模式。要点：**请求参数集中、并发请求只认最后一次、依赖稳定**。

```ts
// src/hooks/usePagedList.ts
import { useCallback, useEffect, useRef, useState } from "react"

export interface PageQuery {
	pageNum: number
	pageSize: number
}

export interface PageResult<T> {
	list: T[]
	total: number
}

export interface PagedListOptions<F> {
	initialFilters?: Partial<F>
	initialPageSize?: number
	immediate?: boolean
}

export interface PagedListState<T, F> {
	dataSource: T[]
	loading: boolean
	total: number
	pageNum: number
	pageSize: number
	filters: Partial<F>
	reload: () => void
	search: (filters: Partial<F>) => void
	reset: () => void
	changePage: (pageNum: number, pageSize: number) => void
}

export function usePagedList<T, F extends object>(
	fetcher: (query: PageQuery & Partial<F>) => Promise<PageResult<T>>,
	options: PagedListOptions<F> = {}
): PagedListState<T, F> {
	const { initialFilters = {}, initialPageSize = 10, immediate = true } = options

	const [dataSource, setDataSource] = useState<T[]>([])
	const [loading, setLoading] = useState(false)
	const [total, setTotal] = useState(0)
	const [pageNum, setPageNum] = useState(1)
	const [pageSize, setPageSize] = useState(initialPageSize)
	const [filters, setFilters] = useState<Partial<F>>(initialFilters)
	const [version, setVersion] = useState(0)

	// 只接受最后一次请求的结果，避免快速切页时的竞态覆盖
	const requestId = useRef(0)
	const skipFirstRun = useRef(!immediate)

	const fetchData = useCallback(async () => {
		const currentId = requestId.current + 1
		requestId.current = currentId
		setLoading(true)
		try {
			const result = await fetcher({ pageNum, pageSize, ...filters })
			if (currentId !== requestId.current) return
			setDataSource(result.list)
			setTotal(result.total)
		} catch (error) {
			if (currentId !== requestId.current) return
			setDataSource([])
			setTotal(0)
		} finally {
			if (currentId === requestId.current) setLoading(false)
		}
	}, [fetcher, pageNum, pageSize, filters])

	useEffect(() => {
		if (skipFirstRun.current) {
			skipFirstRun.current = false
			return
		}
		fetchData()
	}, [fetchData, version])

	const reload = useCallback(() => setVersion((v) => v + 1), [])

	const search = useCallback((next: Partial<F>) => {
		setPageNum(1)
		setFilters(next)
	}, [])

	const reset = useCallback(() => {
		setPageNum(1)
		setFilters(initialFilters)
	}, [initialFilters])

	const changePage = useCallback((nextPageNum: number, nextPageSize: number) => {
		setPageNum(nextPageNum)
		setPageSize(nextPageSize)
	}, [])

	return {
		dataSource,
		loading,
		total,
		pageNum,
		pageSize,
		filters,
		reload,
		search,
		reset,
		changePage
	}
}
```

使用要点：

- `fetcher` 必须是**稳定引用**（模块级函数，或 `useCallback` 包一层），否则 `fetchData` 每次重建会造成死循环。
- 调用方只拿最终可用数据，字段适配、枚举翻译放在 `fetcher` 所在的 API / 转换层，不放进 JSX。
- 现有项目若已有自己的分页 Hook 基建，**优先复用**，不要在新页面里再造一个。
- 选中行、弹窗开关等与请求无关的状态不要塞进这个 Hook。

## 标准异步 Hook 模板

```ts
import { useCallback, useState } from "react"

export interface AsyncOptions<TData, TParams extends unknown[]> {
	initialData?: TData
	onSuccess?: (data: TData, params: TParams) => void
	onError?: (error: Error, params: TParams) => void
}

export function useAsync<TData, TParams extends unknown[]>(
	request: (...params: TParams) => Promise<TData>,
	options: AsyncOptions<TData, TParams> = {}
) {
	const { initialData, onSuccess, onError } = options
	const [data, setData] = useState<TData | undefined>(initialData)
	const [loading, setLoading] = useState(false)
	const [error, setError] = useState<Error | null>(null)

	const run = useCallback(
		async (...params: TParams) => {
			setLoading(true)
			setError(null)
			try {
				const result = await request(...params)
				setData(result)
				onSuccess?.(result, params)
				return result
			} catch (err) {
				const normalized = err instanceof Error ? err : new Error(String(err))
				setError(normalized)
				onError?.(normalized, params)
				return undefined
			} finally {
				setLoading(false)
			}
		},
		[request, onSuccess, onError]
	)

	return { data, loading, error, run, setData }
}
```

注意：

- `onSuccess` / `onError` 若由调用方内联传入，会让 `run` 每轮渲染重建。要么让调用方 `useCallback`，要么用「最新值 ref」模式（见下文 `useLatest`）解耦回调身份。
- 不要在 Hook 里直接弹 toast；把反馈交给调用方或统一的错误层，方便测试与替换。

## 业务动作 Hook

```ts
import { useCallback, useState } from "react"

export interface PurchaseRecord {
	id: string
	orderCode: string
}

export interface ActionFeedback {
	confirm: (options: { title: string; content: string; onOk: () => Promise<void> }) => void
	success: (content: string) => void
	error: (content: string) => void
}

export function useRecordActions(
	publish: (id: string) => Promise<{ code: number; message?: string }>,
	feedback: ActionFeedback,
	onSuccess?: () => void
) {
	const [submitting, setSubmitting] = useState(false)
	const latestOnSuccess = useLatest(onSuccess)

	const handlePublish = useCallback(
		(record: PurchaseRecord) => {
			feedback.confirm({
				title: "确认发布",
				content: `确定要发布单据 ${record.orderCode} 吗？`,
				onOk: async () => {
					setSubmitting(true)
					try {
						const res = await publish(record.id)
						if (res.code !== 0) {
							feedback.error(res.message || "操作失败")
							return
						}
						feedback.success("发布成功")
						latestOnSuccess.current?.()
					} finally {
						setSubmitting(false)
					}
				}
			})
		},
		[publish, feedback, latestOnSuccess]
	)

	return { submitting, handlePublish }
}
```

反馈实现解耦原则：

- `feedback` 通过参数注入，Hook 不直接依赖具体 UI 库。
- 若项目使用 antd 5，**不要在模块顶层直接 `import { message, Modal } from "antd"` 后调用静态方法**（脱离 `ConfigProvider` / `App` 上下文，主题与国际化会丢），应通过 `App.useApp()` 拿到 `message` / `modal` 再注入进来。

## 异步竞态与清理

任何依赖参数的异步副作用都要能取消、能忽略过期结果。

```ts
useEffect(() => {
	const controller = new AbortController()

	const load = async () => {
		setLoading(true)
		try {
			const detail = await fetchDetail(id, { signal: controller.signal })
			setData(detail)
		} catch (error) {
			if (error instanceof DOMException && error.name === "AbortError") return
			setError(error instanceof Error ? error : new Error(String(error)))
		} finally {
			if (!controller.signal.aborted) setLoading(false)
		}
	}

	load()
	return () => controller.abort()
}, [id])
```

- React 18/19 的 `StrictMode` 在开发环境会**挂载 → 卸载 → 再挂载**，Effect 会跑两次：必须保证 Effect 幂等，且清理函数真正释放资源（取消订阅、`abort()`、`clearTimeout`、`removeEventListener`）。
- 请求层不支持 `signal` 时，用递增 `requestId` / `useRef` 标记忽略过期响应，**绝不允许**后发请求被先发请求覆盖。
- React 18 起 state 更新自动批处理，无需手动 `unstable_batchedUpdates`。

## 派生状态与 Effect 使用边界

- **能在渲染期算出来的，就不要放进 state**：

```ts
// ✅ 派生值直接计算，必要时 useMemo
const visibleRows = useMemo(
	() => rows.filter((row) => row.name.includes(keyword)),
	[rows, keyword]
)

// ❌ 用 effect 同步派生状态，多一轮渲染且易不同步
const [visibleRows, setVisibleRows] = useState<Row[]>([])
useEffect(() => {
	setVisibleRows(rows.filter((row) => row.name.includes(keyword)))
}, [rows, keyword])
```

- **不要用 useEffect 响应 props 变化来「重置」状态**，用 `key` 强制重建更安全：

```tsx
<EditForm key={record.id} record={record} />
```

- 不要「props 抄进 state」，需要变换时在渲染期用变量承接。
- Effect 只用于**同步到外部系统**（DOM 命令式操作、订阅、埋点、请求）。渲染期能完成的事不该进 Effect。
- 依赖数组必须完整，**禁止**用 `// eslint-disable-next-line react-hooks/exhaustive-deps` 压制。
- 需要通过依赖数组触发逻辑但不想因某值重建函数时，用 `useLatest` / `useEffectEvent` 模式：

```ts
// src/hooks/useLatest.ts
import { useRef } from "react"

export function useLatest<T>(value: T) {
	const ref = useRef(value)
	// 在 render 期赋值（React 并发模式下 ref 写入需谨慎，这里仅存最新值，不做渲染依据）
	ref.current = value
	return ref
}
```

```ts
import { useEffect, useEffectEvent, useState } from "react"

// React 19.2+ 内置；低版本用 useLatest 或其他方案替代
export function useRoomConnection(roomId: string, onMessage: (msg: string) => void) {
	const handleMessage = useEffectEvent((msg: string) => {
		onMessage(msg) // 始终读到最新回调，且不需要出现在依赖里
	})

	useEffect(() => {
		const socket = connect(roomId)
		socket.on("message", handleMessage)
		return () => socket.close()
	}, [roomId])
}
```

## 并发特性

按需使用，不要为了「新」而用：

| Hook | 用途 | 典型场景 |
|------|------|----------|
| `useTransition` | 把非紧急更新降级 | 大列表筛选、复杂 Tab 切换 |
| `useDeferredValue` | 延迟使用某个值 | 搜索框实时过滤长列表 |
| `useSyncExternalStore` | 安全订阅外部 store | 浏览器 API、非 React 状态库接入 |
| `useId` | 生成稳定唯一 id | 表单 `label`/`htmlFor`、SSR 一致性 |
| `useInsertionEffect` | 注入样式（库作者用） | CSS-in-JS |

```ts
const [isPending, startTransition] = useTransition()
const handleSearch = (value: string) => {
	setKeyword(value) // 紧急：输入框保持响应
	startTransition(() => setActiveKeyword(value)) // 非紧急：结果列表
}
```

- `useTransition` 的 `isPending` 用于降级提示，**不要**用它替代数据请求的 loading。
- 外部 store 订阅必须走 `useSyncExternalStore`，避免 `useEffect` 订阅导致的撕裂（tearing）。

## React 19 行为变化

- **`ref` 可作为普通 prop 传递**，函数组件不再需要 `forwardRef`。为兼容 React 18 仍可保留 `forwardRef`，但新代码在 19 环境下可直接 `function Input({ ref, ...props })`。
- **ref 回调可返回清理函数**：

```tsx
<div
	ref={(node) => {
		node?.addEventListener("click", handleClick)
		return () => node?.removeEventListener("click", handleClick)
	}}
/>
```

- **`useRef` 必须传初始值**（TS 类型强制）：`useRef<HTMLDivElement>(null)`，不要再写 `useRef<HTMLDivElement>()`。
- **`use()`** 可读取 Promise 与 Context，且允许条件调用（但仍需在组件/Hook 内）：

```tsx
const theme = use(ThemeContext)
```

- **Context 可直接作为 Provider**：`<ThemeContext value={theme}>`。
- **`useActionState`** 替代「手写 loading + error state」的表单提交：

```ts
const [state, submit, isPending] = useActionState(
	async (_prev: FormState, formData: FormData) => saveUser(formData),
	{ status: "idle" } as FormState
)
```

- **`useOptimistic`** 处理乐观更新（点赞、批量删除、状态流转）。
- **`useEffectEvent`**（19.2+）解决闭包过期问题，替代「用 ref 存最新回调」的写法；低版本回退到 `useLatest`。
- **已移除的 API**：`ReactDOM.render`、`ReactDOM.hydrate`、`unmountComponentAtNode`、字符串 `ref`、函数组件的 `defaultProps`、legacy context、`propTypes`。
- React 18 新增的 `useId`、`useSyncExternalStore`、`useInsertionEffect`、自动批处理在 19 中继续有效。

## TypeScript 建模规范

- Props 显式声明类型，**不使用 `React.FC`**（18 起不再隐式注入 `children`；需要时显式声明 `children?: React.ReactNode`）。
- Hook 返回值定义具名接口，避免调用方靠推断猜测字段。
- 用**判别联合**建模状态，而不是一堆 boolean：

```ts
type AsyncState<T> =
	| { status: "idle" }
	| { status: "loading" }
	| { status: "success"; data: T }
	| { status: "error"; error: Error }
```

- 泛型 Hook 的泛型参数不要超过 3 个；超过说明职责过重。
- 外部数据用 `unknown` 接收 + 类型守卫 / 校验，禁止 `any` 穿透：

```ts
function isPageResult<T>(value: unknown): value is PageResult<T> {
	return typeof value === "object" && value !== null && "list" in value && "total" in value
}
```

- 事件类型明确：`React.ChangeEvent<HTMLInputElement>`、`React.MouseEvent<HTMLButtonElement>`、`React.FormEvent<HTMLFormElement>`。
- 只读数据用 `readonly T[]` 表达，防止子组件误改 props。
- 慎用 `as` 断言；`as const` 用于字面量收敛是允许的。
- 建议开启 `strict`、`noUncheckedIndexedAccess`、`exactOptionalPropertyTypes`、`noImplicitOverride`，索引访问结果按可能为 `undefined` 处理。
- `useCallback` / `useMemo` 的泛型依赖由 Hook 自身签名约束，不要为「看起来严谨」给每个回调手写泛型。

## 性能规则

- 表格 columns、图表配置等大对象用 `useMemo` 或提到模块级常量；`scroll`、`pagination` 这类内联对象同样要稳定引用。
- 传给子组件的回调用 `useCallback`；纯展示且高频渲染的子组件用 `React.memo`。
- 避免在 render 中创建大量对象、数组、内联函数。
- 长列表优先虚拟滚动，不要靠 `useMemo` 硬扛。
- `useMemo` / `useCallback` 不是默认选项：**有明确收益或需要稳定引用时才用**，否则只是增加噪音与内存。
- 用 React DevTools Profiler 定位真实瓶颈，不要凭感觉优化。

## 常见反模式

| 反模式 | 问题 | 正确做法 |
|--------|------|----------|
| `useEffect` + `setState` 存派生值 | 多一轮渲染、易不同步 | 渲染期直接计算 / `useMemo` |
| Effect 内请求不处理竞态与清理 | 快速切参时数据错乱、卸载后 setState | `AbortController` 或 `requestId` + 清理函数 |
| props 抄进 state | 数据双份来源 | 直接用 props，或用 `key` 重建 |
| Hook 名不以 `use` 开头 | lint 规则失效，条件调用不被发现 | 统一 `useXxx` 命名 |
| 依赖数组漏项 / 屏蔽 lint | 闭包过期、逻辑不触发 | 补全依赖或用 `useLatest` / `useEffectEvent` |
| 循环或条件中调用 Hook | 违反 Hooks 调用顺序 | 上提到顶层，用标志位分流 |
| 在 render 期间 setState | 死循环 | 移到事件回调或 Effect |
| 无条件堆砌 `useMemo` / `useCallback` | 噪音与开销大于收益 | 有明确收益再用 |
| 自定义 Hook 里写 JSX | 逻辑与视图耦合，无法复用 | 拆成组件 |
| Hook 返回 20 个字段 | 职责不清 | 按「数据 / 动作 / UI 状态」拆分 |

## 检查清单

- [ ] Hook 是否以 `use` 开头，文件名与导出名一致？
- [ ] 是否只做一件事，返回字段是否可读？
- [ ] 异步逻辑是否处理了 loading、错误、竞态、卸载清理？
- [ ] 依赖数组是否完整，有无 `eslint-disable` 压制？
- [ ] 派生状态是否避免用 `useEffect + useState`？
- [ ] 传入的回调与配置对象是否引用稳定（避免死循环）？
- [ ] 返回类型是否有显式 TypeScript 声明，有无 `any` 穿透？
- [ ] 是否使用了判别联合而非多个 boolean 表达状态？
- [ ] React 19 相关写法（`ref` prop、ref cleanup、`useRef` 必传参）是否落地？
- [ ] 是否用 Profiler 验证过性能优化的必要性？
