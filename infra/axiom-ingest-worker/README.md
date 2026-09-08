# Lyrics Plus Axiom ingest Worker

这个 Worker 是应用和 Axiom 之间的唯一写入代理。Axiom token 不进入应用安装包，只配置在 Worker secret 中。

## 部署

1. 在 Axiom 创建 US East 的 `lyrics-plus-events` dataset，并创建只允许 ingest 的 Basic API token。
2. 设置 secret：

```bash
npx wrangler secret put AXIOM_INGEST_URL
npx wrangler secret put AXIOM_INGEST_TOKEN
```

`AXIOM_INGEST_URL` 使用 Axiom 控制台为该 dataset 提供的 JSON ingest URL。US East 1 通常是 `https://us-east-1.aws.edge.axiom.co/v1/ingest/lyrics-plus-events`。

3. 部署：

```bash
npx wrangler deploy
```

4. 当前生产 Worker 地址为 `https://stats.xiaoafei.cn/v1/events`。应用通过 `pnpm tauri build` 打包时，项目脚本会自动注入它；如需备用或测试 Worker，可在构建环境设置非空的 `LYRICS_PLUS_TELEMETRY_ENDPOINT` 覆盖默认值。开发模式会主动移除该变量。

Worker 只接受固定事件名称和字段，单批最多 50 条、请求最多 64 KiB；事件中不允许歌曲名、歌手、歌词、路径或原始日志字段。
