# AI 批量注释编排器设计

## 1. 背景

当前我们已经完成了最关键的可行性验证：

1. 已成功定位 GoLand AI 插件的真实请求链路。
2. 已成功复放 `POST /v2/chat/completions`。
3. 已确认响应为 `text/event-stream`，真正可写回的内容来自 `choices[0].delta.content`。
4. 已确认 `reasoning_content` 不应进入最终文件。

这意味着“批量给项目文件添加中文注释”的难点已经从“能不能抓到接口”转移为“如何把这个能力工程化、产品化，并且安全、可恢复、可回滚地跑起来”。

本设计不再考虑 UI 自动化方案，而是直接以 `HTTP API 复用 + 文件遍历 + 任务编排 + 审阅/回滚` 作为唯一主路线。

## 2. 目标与非目标

### 2.1 目标

1. 在现有 Tauri 桌面应用中新增一个独立工具页，作为 `AI 批量注释编排器`。
2. 支持多项目任务队列，而不是只支持单项目单次运行。
3. 支持为不同项目分别配置目录、扩展名白名单、排除目录、Bearer Token 来源、并发数、重试次数、最大处理文件数等参数。
4. 支持 `auto` 与 `review` 两种运行模式。
5. 支持并发 worker 批量处理大量彼此独立的文件。
6. 支持中途中断、应用崩溃或关闭后的可恢复执行。
7. 支持 `.json` 这类天然不支持注释的文件生成旁路说明文件，而不是破坏原文件格式。
8. 支持失败重试、待审队列、外部 diff 工具查看，以及按运行批次回滚。
9. 保持写回安全：不让模型的解释性文本、Markdown 围栏、结构性污染直接覆盖源码。

### 2.2 非目标

1. 本次不实现 GoLand 插件侧的自动登录、自动续期或 token 刷新。
2. 本次不做 UI 自动化备用通道。
3. 本次不内嵌 diff 编辑器。
4. 本次不把该工具做成 Git 替代品；回滚只覆盖本工具明确写入过的文件。
5. 本次不引入云端任务同步或分布式 worker。

## 3. 已确认的产品决策

本设计基于以下已确认约束：

1. 工具是 `多语言通用注释器`，不是只处理 Go。
2. 第一版处理范围覆盖：
   - 源码：`.go .java .py .ts .js .vue .sh`
   - 配置模板：`.yaml .yml .json .xml .properties .tpl`
3. 写回策略采用“折中模式”：
   - 默认自动写回；
   - 但必须先通过强校验；
   - 对高风险结果转入人工审阅或旁路产物。
4. Bearer Token 第一版采用手工提供，不做自动刷新。
5. 工具同时支持 `--auto` 与 `--review` 两种运行方式，对应到桌面 UI 上就是“自动模式”和“审阅模式”。
6. `.json` 不改原文件，只生成结构化旁路说明。
7. 模型被允许做“轻微整理”，但不允许改变业务逻辑、函数语义或重要结构。
8. 需要支持：
   - 外部 diff 工具调用；
   - 多并发 worker；
   - 多项目任务队列；
   - 按运行批次回滚。

## 4. 核心用户流程

### 4.1 创建项目配置

用户在新工具页中新增一个项目 profile：

1. 选择项目根目录。
2. 选择 Bearer Token 来源。
3. 设置包含扩展名与排除目录。
4. 设置默认运行模式、默认并发数、默认重试次数、默认最大处理文件数。
5. 设置是否允许轻微整理，以及 `.json` 旁路说明策略。

### 4.2 将项目加入队列

用户基于某个项目 profile 创建一个运行实例（run）：

1. 选择 `auto` 或 `review`。
2. 选择本次最大处理文件数。
3. 选择本次并发数。
4. 选择本次失败重试次数。
5. 决定是否立即开始，还是只加入全局队列等待。

### 4.3 运行中控制

用户可以在队列页或运行详情页中：

1. 暂停某个 run。
2. 恢复某个 run。
3. 取消某个 run。
4. 查看当前 worker 正在处理的文件。
5. 查看已完成、失败、待审、剩余数量。
6. 查看最近错误和最近运行事件。

### 4.4 审阅与差异查看

对 `review_needed` 文件，用户可以：

1. 查看候选结果摘要。
2. 调起 Beyond Compare、TortoiseMerge、WinMerge 等外部 diff 工具。
3. 接受候选结果并落盘。
4. 拒绝结果并跳过。
5. 重新生成单个文件。

### 4.5 回滚

用户可以在历史页选择某次 run：

1. 查看该 run 改动过哪些文件。
2. 对整次 run 执行回滚。
3. 查看哪些文件回滚成功、哪些文件因为后续人工修改而冲突。

## 5. 总体架构

本功能应当被设计为一个新的、独立的工具模块，而不是零散加在现有页面中。

### 5.1 前端

前端沿用现有项目模式：

1. `Vue 3 + TypeScript`
2. 继续使用当前基于 `reactive` 的 store 风格，而不是额外引入 Pinia
3. 命令调用继续走 `src/lib/tauri.ts`
4. 新增一个独立页面：
   - `src/pages/CommentOrchestratorPage.vue`
5. 新增一组功能聚焦的前端模块：
   - `src/lib/commenterTypes.ts`
   - `src/lib/commenterStore.ts`
   - `src/lib/commenterView.ts`
   - `src/components/commenter/...`

### 5.2 Tauri / Rust

后端新增一个独立模块：

1. `src-tauri/src/commenter/mod.rs`
2. `src-tauri/src/commenter/models.rs`
3. `src-tauri/src/commenter/db.rs`
4. `src-tauri/src/commenter/config.rs`
5. `src-tauri/src/commenter/scanner.rs`
6. `src-tauri/src/commenter/prompt.rs`
7. `src-tauri/src/commenter/http.rs`
8. `src-tauri/src/commenter/sse.rs`
9. `src-tauri/src/commenter/validate.rs`
10. `src-tauri/src/commenter/artifacts.rs`
11. `src-tauri/src/commenter/rollback.rs`
12. `src-tauri/src/commenter/scheduler.rs`
13. `src-tauri/src/commenter/commands.rs`
14. `src-tauri/src/commenter/events.rs`

### 5.3 设计原则

1. 前端不直接访问数据库，不直接操作文件。
2. 所有状态机逻辑集中在 Rust service 层。
3. Tauri command 只做薄入口。
4. SQLite 负责结构化状态，磁盘目录负责大产物。
5. `run` 与 `job` 分层建模，避免多项目队列与文件级执行耦合混乱。

## 6. 前端信息架构

### 6.1 页面结构

新增工具应至少包含以下视图：

1. `项目配置页`
   - 管理 project profile
   - 管理 credential profile 引用
2. `队列总览页`
   - 展示所有 run
   - 支持开始、暂停、继续、取消、回滚入口
3. `运行详情页`
   - 展示当前文件、worker 状态、最近事件、失败与待审统计
4. `待审页`
   - 展示 `review_needed` 文件
   - 支持打开外部 diff、接受、拒绝、重试
5. `历史与回滚页`
   - 展示历史 run 与回滚结果

### 6.2 全局可见信息

队列总览和详情页中必须始终可见：

1. 当前 run 状态
2. 当前全局剩余 worker 数
3. 当前 run 占用 worker 数
4. 已完成、失败、待审、剩余数量
5. 当前文件
6. 最近错误
7. 是否触发熔断
8. 是否已达到“最多处理多少个文件就停”

## 7. 数据模型与状态机

### 7.1 主要实体

建议使用 SQLite 持久化以下实体：

1. `app_settings`
2. `credential_profiles`
3. `project_profiles`
4. `queue_runs`
5. `file_jobs`
6. `artifacts`
7. `run_events`
8. `review_actions`
9. `rollback_actions`

### 7.2 运行级状态

`queue_runs.status` 建议使用：

1. `queued`
2. `scanning`
3. `ready`
4. `running`
5. `pausing`
6. `paused`
7. `stopped_by_limit`
8. `completed`
9. `completed_with_issues`
10. `cancelled`
11. `failed`
12. `rollback_ready`
13. `rolled_back`
14. `rollback_failed`

### 7.3 文件级状态

`file_jobs.status` 建议使用：

1. `pending`
2. `leased`
3. `requesting`
4. `validating`
5. `writing`
6. `done`
7. `review_needed`
8. `retry_waiting`
9. `failed`
10. `skipped`
11. `rolled_back`

### 7.4 中断恢复原则

应用重启时：

1. 所有 `running` / `scanning` / `leased` / `requesting` / `writing` 的 run 统一回退为 `paused`。
2. 悬挂的文件任务回退到 `pending` 或 `review_needed`。
3. 已经写出了 `before` 但没完成安全落盘的文件优先进入 `review_needed`。

## 8. 文件处理流水线

单文件标准处理链路如下：

1. 领取 job，建立 lease。
2. 读取原文件，计算 `sha256`，记录编码、大小、mtime。
3. 分类：
   - A 类：可直接注释的文件
   - B 类：只生成旁路说明的文件（当前为 `.json`）
   - C 类：必须跳过的文件
4. 根据文件类型构建 prompt。
5. 调用 `/v2/chat/completions`。
6. 解析 SSE，只拼接 `delta.content`。
7. 规范化返回结果。
8. 执行强校验与软校验。
9. 生成候选产物或 sidecar。
10. 根据模式和风险决策：
   - 自动写回
   - 转待审
   - 失败重试
11. 写入产物目录、数据库和事件流。

## 9. Prompt 策略

本功能不应使用单一万能 prompt，而应采用三层策略：

1. `全局规则`
   - 不改业务逻辑
   - 不删除代码
   - 只返回完整结果
   - 不输出 Markdown 围栏
   - 注释为中文
2. `语言族模板`
   - Go / Java
   - Python
   - TS / JS / Vue
   - Shell
   - YAML / XML / Properties / Tpl
   - JSON 旁路说明
3. `项目覆盖`
   - 基础设施项目
   - 后端服务项目
   - 模板项目
   - 其他业务域自定义补充

## 10. API 集成与安全

### 10.1 请求头策略

第一版采用“保守保留、后续最小化”的策略：

1. 必需头：
   - `Authorization`
   - `Content-Type`
2. 建议保留头：
   - `User-Agent`
   - `X-Agent-Intent`
   - `X-IDE-Type`
   - `X-IDE-Name`
   - `X-IDE-Version`
   - `X-Product-Version`
   - `X-Enterprise-Id`
   - `X-Tenant-Id`
   - `X-Domain`
   - `X-Product`
   - `X-Env-ID`
   - `X-User-Id`
3. 会话追踪头：
   - `X-Conversation-ID`
   - `X-Conversation-Request-ID`
   - `X-Conversation-Message-ID`
   - `X-Request-ID`
   - `X-Request-Trace-Id`
   - `b3` 相关头
   这些由系统每次动态生成，不要求固定复用。

### 10.2 Token 安全

1. 第一版支持：
   - 环境变量引用
   - 本地配置文件
   - UI 直接输入
2. token 不进入：
   - `queue_runs`
   - `file_jobs`
   - `run_events`
   - 导出报告
3. 请求日志默认脱敏。
4. 完整原始请求快照只作为调试开关，并默认关闭。

## 11. 并发、重试与熔断

### 11.1 并发模型

并发应同时受三层限制：

1. `global_max_workers`
2. `run_max_workers`
3. `api_concurrency_limit`

调度采用：

1. 全局 `优先级 + FIFO`
2. 同优先级下跨 run 轮转，避免大项目独占所有 worker

### 11.2 重试模型

可重试错误包括：

1. 网络超时
2. SSE 中断
3. `429`
4. 临时 `5xx`
5. 空响应
6. 疑似偶发输出污染

不可重试错误包括：

1. token 缺失
2. 配置非法
3. 文件不可写
4. 编码无法处理
5. 强校验稳定失败

重试采用：

1. 每文件独立计数
2. 指数退避 + 抖动
3. 超过上限后转 `failed`

### 11.3 熔断

需要支持运行级熔断：

1. `401/403` 认证熔断
2. `429` 限流熔断
3. 大量结构异常响应熔断
4. 磁盘或产物目录异常熔断

熔断触发后：

1. 停止分发新 job
2. 让进行中的 job 尽量收尾
3. 将 run 标记为 `paused` 或 `completed_with_issues`
4. 在 UI 中明确显示触发原因

## 12. 审阅、差异查看与回滚

### 12.1 审阅

以下情况自动进入 `review_needed`：

1. 改动比例过大
2. 非注释改动过多
3. 可疑重命名
4. 语言结构异常
5. 结果虽可读，但不满足安全直写条件

### 12.2 外部 diff

系统不内嵌 diff 编辑器，只负责：

1. 为每个 job 准备 `before` 与 `after_candidate`
2. 依据用户配置的命令模板调起：
   - Beyond Compare
   - TortoiseMerge
   - WinMerge
3. 记录“已打开 diff”事件

### 12.3 回滚

回滚以 `run_id` 为单位：

1. 只有真正改写过原文件的文件进入回滚清单。
2. `.json` sidecar 可单独清理，但不参与原文件回滚。
3. 回滚前再次校验当前文件 hash：
   - 若仍匹配本工具上次写回结果，则允许自动回滚；
   - 若用户后续手改过，则转 `rollback_conflict`。

## 13. 产物目录布局

建议数据根目录如下：

```text
<data_root>/
  app.db
  runs/
    <run_id>/
      manifest/
        run-config.json
        file-list.json
      before/
      candidates/
      sidecars/
      request/
      response/
      logs/
```

规则如下：

1. `before/` 是回滚真相源。
2. `candidates/` 保存模型候选结果，不等于一定已写回。
3. `sidecars/` 保存 `.json` 等旁路说明。
4. `request/response/` 默认只保留脱敏版。
5. 所有大文件产物在磁盘，数据库只存路径和元数据。

## 14. 验收标准

设计完成后，功能应满足：

1. 可以配置多个项目 profile，并把它们分别加入全局队列。
2. 每个项目 run 都可以单独开始、暂停、继续、取消。
3. 可以设置并发数、失败重试次数、最大处理文件数。
4. 达到“最多处理 N 个文件就停”后，run 进入 `stopped_by_limit`。
5. `.json` 不改原文件，只生成旁路说明。
6. 自动模式下，只有通过强校验和软校验的结果才会落盘。
7. 审阅模式下，用户可以对待审文件逐个查看、接受、拒绝或重试。
8. 中断后重新打开应用，队列仍可恢复。
9. 可以按运行批次回滚，并识别回滚冲突。

## 15. 风险与取舍

### 15.1 风险

1. 服务端可能后续增加 header 或风控要求。
2. 并发数过高时更容易触发 `429`。
3. 多语言 prompt 的稳定性不一致。
4. 回滚时用户可能已手改文件。
5. 多项目队列会显著提升状态同步复杂度。

### 15.2 取舍

1. 为了稳定性，第一版不做自动 token 刷新。
2. 为了安全性，第一版不做“无校验直接覆盖”。
3. 为了控制复杂度，第一版只做外部 diff，不做内嵌编辑器。
4. 为了兼容非 Git 项目，回滚不依赖 Git。

## 16. 实施建议

实现应按以下顺序推进：

1. 建立 Rust 侧 `commenter` 领域模型、SQLite 与产物目录层。
2. 完成扫描、prompt、SSE 客户端、验证与写回链路。
3. 完成 scheduler、worker pool、暂停/恢复/重试/熔断。
4. 接入 Tauri commands 与 events。
5. 完成前端项目配置、队列总览、详情、待审与回滚页面。
6. 补齐外部 diff、异常恢复和批次回滚。

---

本设计的核心目标不是“把单文件调用脚本包一个壳”，而是把现成可复放的 AI 接口能力提升为一个可排队、可恢复、可审阅、可回滚的桌面级批量注释编排系统。
