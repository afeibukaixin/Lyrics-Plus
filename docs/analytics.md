# Lyrics Plus 匿名统计与 Axiom

## 数据链路

```text
Lyrics Plus → Cloudflare Worker /v1/events → Axiom US East dataset → Axiom MCP（只读查询）
```

应用不会携带 Axiom token。Axiom token 只存在于 `infra/axiom-ingest-worker` 的 Worker secret 中。

## 首次部署

1. 在 Axiom 创建 US East dataset：`lyrics-plus-events`。
2. 创建只允许 ingest 该 dataset 的 Basic API token。
3. 进入 `infra/axiom-ingest-worker`，按 README 配置 `AXIOM_INGEST_URL` 和 `AXIOM_INGEST_TOKEN` 后部署 Worker。
4. 当前生产 Worker 地址为 `https://stats.xiaoafei.cn/v1/events`。使用 `pnpm tauri build` 打包时，项目构建脚本会自动注入它；如需备用或测试 Worker，可在构建环境设置非空的 `LYRICS_PLUS_TELEMETRY_ENDPOINT` 覆盖默认值。`pnpm tauri dev` 会主动移除该变量，不向生产数据集发送统计。
5. 在 Codex 中使用项目级 `.codex/config.toml` 的 `https://mcp.axiom.co/mcp`，首次查询时完成 OAuth。

## 隐私边界

- 只有用户接受最新版使用须知后才启动统计服务。
- 默认发送匿名会话、日活、搜索汇总和歌词源健康汇总；不发送歌曲名、歌手、歌词、文件路径、轨道 ID、Token、原始错误或日志。
- 关于页面关闭统计后，会删除本机随机安装 ID、活跃日期和待发送队列。
- Axiom 查询只使用聚合结果，不展示或复述 `installId`。

## 推荐查询

查询模板位于 `.agents/skills/lyrics-plus-analytics/references/query-recipes.md`。正式查询前先通过 MCP 读取 dataset schema，再确认字段名和时间范围。
