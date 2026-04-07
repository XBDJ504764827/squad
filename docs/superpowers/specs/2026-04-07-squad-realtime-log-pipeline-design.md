## Goal

为当前服务器管理工作台接入真实的 Squad 游戏服务器实时日志流，支持管理后端与游戏服务器分机部署，并兼容 Linux 与 Windows 两类游戏服务器环境。

第一阶段目标是打通一条可联调、可继续开发的真实日志链路：

- 游戏服务器本地日志文件被采集
- 日志通过专业采集器推送到当前管理后端
- 管理后端为前端提供最近日志与实时流
- 前端“实时日志”页展示真实后端日志

## Scope

- 游戏服务器与管理后端不在同一台机器上。
- 游戏服务器操作系统可能是 Linux，也可能是 Windows。
- 前端保留当前“实时日志”页已有 UI 能力：
  - 级别筛选
  - 关键字搜索
  - 暂停追加
  - 自动滚动
  - 清空当前视图
- 第一阶段仅实现真实日志接入链路，不实现长期日志平台能力。

## Non-Goals

- 不接入 Elasticsearch、Loki、Grafana、Kibana 等完整日志平台。
- 不做长期日志存储与历史分页查询。
- 不做复杂全文检索与高级过滤。
- 不做控制台命令双向交互。
- 不做基于日志的自动告警规则。
- 不做 Squad 日志的深度结构化解析引擎。

## Recommendation

采用“专业采集器 + 当前管理后端 + 前端 SSE”的轻量落地方案：

- 游戏服务器侧使用 `Vector` 或 `Fluent Bit` 读取 Squad 本地日志文件
- 采集器将日志标准化后推送到管理后端
- 管理后端按服务器缓存最近日志，并通过 `SSE` 向前端推送实时日志
- 前端实时日志页改为真实订阅，而不是本地伪造日志

推荐该方案的原因：

- Linux 与 Windows 的差异被隔离在采集器配置层
- 前端与管理后端接口保持统一
- 不依赖 SSH、WinRM 等远程执行能力
- 实现复杂度明显低于完整日志平台
- 后续聊天记录、操作记录、事件记录等能力可以复用同类接入模式

## Architecture

### Components

- Squad 游戏服务器
  负责输出本地原始日志文件。
- 日志采集器
  部署在游戏服务器本机，读取日志文件并推送到管理后端。
- 管理后端
  提供日志接收、内存缓存、最近日志查询、实时日志流分发。
- 前端服务器工作台
  在“实时日志”页中拉取最近日志并订阅实时日志流。

### Data Flow

1. Squad 游戏服务器将控制台日志写入本地文件。
2. `Vector` 或 `Fluent Bit` 在游戏服务器本地监听日志文件。
3. 采集器将日志转换为统一字段后推送到管理后端。
4. 管理后端根据 `server_uuid` 将日志写入对应的内存环形缓冲区。
5. 管理后端将新增日志推送给已订阅该服务器的前端 SSE 连接。
6. 前端进入页面时先读取最近日志，再持续追加实时日志。

### Transport Choices

- 采集器到管理后端：HTTP 推送
- 管理后端到前端：SSE

选择 SSE 而非 WebSocket 的原因：

- 当前仅需后端到前端的单向实时推送
- 浏览器原生支持 `EventSource`
- Axum 中实现和维护成本更低
- 断线重连模型简单，适合日志流场景

## Backend Design

### Endpoints

#### `POST /api/servers/:server_uuid/logs/ingest`

用途：

- 给 `Vector` / `Fluent Bit` 推送日志使用

要求：

- 必须鉴权，不能匿名开放
- 支持批量日志写入
- 仅接收已存在的服务器 `server_uuid`

请求体示例：

```json
{
  "source": "vector",
  "host": "game-server-01",
  "entries": [
    {
      "timestamp": "2026-04-07T12:30:45Z",
      "level": "INFO",
      "channel": "server",
      "message": "LogSquad: Display: Match State Changed to InProgress",
      "raw": "LogSquad: Display: Match State Changed to InProgress"
    }
  ]
}
```

响应：

- 成功时返回接收条数
- 无效服务器或鉴权失败时返回错误

#### `GET /api/servers/:server_uuid/logs/recent`

用途：

- 页面首次进入时获取最近日志快照

要求：

- 默认返回最近 200 条
- 无日志时返回空数组
- 返回顺序按时间从旧到新，方便前端直接渲染

#### `GET /api/servers/:server_uuid/logs/stream`

用途：

- 前端实时订阅某个服务器日志流

要求：

- 返回 `text/event-stream`
- 每条事件承载一条标准化日志
- 支持浏览器断线自动重连
- 连接关闭后不影响其他订阅者

### Data Model

后端统一内部日志模型：

- `id`
- `server_uuid`
- `timestamp`
- `level`
- `channel`
- `message`
- `raw`
- `source`
- `source_host`

设计要点：

- `raw` 必须保留，避免前期因为 Squad 日志格式不稳定导致信息丢失
- `level` 与 `channel` 允许为空或回退值
- 前期不做复杂解析，只做轻量规范化

### Storage Strategy

第一阶段使用内存环形缓冲区，不落库：

- 每个服务器维护独立缓存
- 每个服务器保留最近 `1000` 到 `5000` 条日志
- 超上限时自动淘汰最旧日志

这样做的目的：

- 先把真实链路打通
- 降低实现复杂度
- 避免在日志格式尚不稳定时过早设计持久化结构

### Auth Strategy

采集器写入接口需要独立鉴权，例如：

- 每台服务器单独的 ingest token
- 或平台级共享 token + `server_uuid` 校验

第一阶段优先选择实现成本更低的方案，但必须满足：

- 不能让任意第三方伪造日志
- 能根据请求快速定位来源服务器

## Frontend Design

### Page Behavior

“实时日志”页改为真实行为：

1. 页面进入时调用 `GET /logs/recent`
2. 渲染最近日志
3. 建立 `EventSource` 到 `GET /logs/stream`
4. 新日志到达后实时追加
5. 页面离开或服务器切换时关闭旧连接

### UI States

页面至少支持以下状态：

- `连接中`
- `已连接`
- `重连中`
- `采集器离线`
- `拉取失败`

状态表现要求：

- 连接异常时不清空已展示日志
- 页面需要提供明显状态提示
- 用户可以主动重试最近日志拉取

### Existing UX To Preserve

保留现有实时日志页的交互能力：

- 级别筛选
- 关键字搜索
- 暂停追加
- 自动滚动
- 清空当前视图

约束：

- “清空当前视图”只影响前端显示，不删除后端缓存
- 过滤与搜索基于当前前端内存中的可见日志进行

### Display Rules

前期展示字段：

- 时间
- 级别
- 频道
- 消息内容

回退规则：

- 无 `level` 时显示 `UNKNOWN`
- 无 `channel` 时显示 `server`

## Cross-Platform Deployment

日志采集器需要同时适配 Linux 与 Windows：

- Linux：
  - 读取本地 Squad 日志文件
  - 关注文件滚动、权限与路径配置
- Windows：
  - 读取本地 Squad 日志文件
  - 关注路径格式、文件编码与日志轮转行为

平台差异只允许停留在采集器配置层，不向前端和管理后端扩散。

第一阶段交付时需要提供：

- Linux 采集配置样例
- Windows 采集配置样例
- 配置项说明文档

## Error Handling

### Ingest Failures

- 无效 `server_uuid`：拒绝写入
- 鉴权失败：拒绝写入
- 空日志批次：返回参数错误
- 单条日志字段缺失：允许按回退规则接收，避免整批丢弃

### Stream Failures

- 前端 SSE 断开后允许自动重连
- 后端不因单个订阅连接异常影响全局日志分发
- 当前服务器长时间无新日志时，前端应显示“采集器离线”或“暂无日志输入”状态

## Testing

### Backend Tests

- `POST /logs/ingest`
  - 正常接收批量日志
  - 缺少鉴权返回错误
  - 非法 `server_uuid` 返回错误
  - 空 entries 返回错误
- `GET /logs/recent`
  - 有日志时按顺序返回
  - 无日志时返回空数组
- `GET /logs/stream`
  - 建连成功
  - 新日志进入后能被订阅端收到
  - 一个订阅断开不影响其他订阅
- 环形缓冲区
  - 超上限后淘汰旧日志

### Frontend Tests

- 页面初始化会先拉取最近日志
- `EventSource` 建立后能追加实时日志
- 级别筛选与搜索功能在真实数据流下仍然有效
- 连接失败时页面显示对应状态
- 重连期间保留已有日志内容

## Delivery Boundary

第一阶段只交付：

- 真实日志采集接入链路
- 管理后端日志接收、缓存、流式分发
- 前端实时日志页接入真实后端
- Linux / Windows 采集配置样例

第一阶段不交付：

- 长期日志存储
- 日志平台部署
- 高级查询
- 双向控制台命令
- 自动告警系统

## Open Decisions Resolved

- 游戏服务器与管理后端分机部署：已纳入设计
- 游戏服务器可能为 Linux 或 Windows：已纳入设计
- 方案选择：采用方案 3 的轻量化落地方式，不直接引入完整外部日志平台
- 前端实时通信方式：使用 SSE
- 第一阶段存储策略：使用内存环形缓冲区
