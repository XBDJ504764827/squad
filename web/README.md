# 本地前后端联调

当前项目采用前后端分离开发：

- 前端：`web`，React + Vite
- 后端：`backend`，Rust + Axum + PostgreSQL

开发态下，前端统一请求 `/api/*`，再由 Vite 代理到 Rust 后端。

## 启动顺序

1. 启动 PostgreSQL，并确保数据库存在：

```text
postgres://squad:squad@127.0.0.1:5432/squad
```

2. 启动后端：

```bash
cd backend
cargo run
```

默认读取 `backend/.env`：

```env
DATABASE_URL=postgres://squad:squad@127.0.0.1:5432/squad
PORT=3000
DATABASE_MAX_CONNECTIONS=10
```

3. 启动前端：

```bash
cd web
npm run dev
```

默认开发地址一般为 `http://127.0.0.1:5173`。

## 联调规则

前端请求层默认把接口发到：

```text
/api
```

Vite 开发服务器会把 `/api` 代理到后端地址。默认目标地址来自：

- `web/.env.example` 中的 `BACKEND_PROXY_TARGET`
- 如果未单独指定，则回退到 `http://127.0.0.1:<backend PORT>`

当前默认即：

```text
http://127.0.0.1:3000
```

因此本地联调时，只要后端运行在 `3000` 端口，前端无需改代码即可直接访问：

- `GET /api/health`
- `GET /api/dashboard`
- `POST /api/servers`

## 可选环境变量

`web/.env.example`：

```env
BACKEND_PROXY_TARGET=http://127.0.0.1:3000
VITE_API_BASE_URL=/api
```

说明：

- `BACKEND_PROXY_TARGET`：只影响 Vite 开发代理的后端目标地址。
- `VITE_API_BASE_URL`：影响前端请求前缀，默认是 `/api`。

通常本地开发不需要改这两个值。只有在后端端口变更、或前端需要直连其他环境时再调整。

## 页面联调状态

前端页面右上角现在会明确显示联调状态：

- `连接中`：首次加载或刷新中
- `后端已连接` / 后端返回的在线状态文本：接口正常
- `后端未连接`：接口请求失败

如果后端未启动、数据库未连接、或接口返回错误，页面顶部会直接显示失败原因，便于排查。

## 常见问题

### 页面显示“后端未连接”

优先检查：

1. PostgreSQL 是否已启动
2. `backend/.env` 中的 `DATABASE_URL` 是否可用
3. 后端是否已执行 `cargo run`
4. 后端是否监听在 `127.0.0.1:3000`

### 前端请求地址不对

检查：

- `web/src/services/dashboardApi.js` 中默认前缀是否仍为 `/api`
- `web/vite.config.js` 中 `/api` 代理是否指向正确端口
- 是否误改了 `VITE_API_BASE_URL` 或 `BACKEND_PROXY_TARGET`
