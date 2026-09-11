# MediaRemoteAdapter 固定资源

该归档原样取自 `media-remote` 0.3.8 发布包的 `assets/mediaremote-adapter.tar.gz`。
上游：https://github.com/nohackjustnoobb/media-remote
适配器源码：https://github.com/ungive/mediaremote-adapter

- 归档 SHA-256：`87b19e480a213ee591b7794942c2111f3ad58e7f0a1f18ec62c581d8e80e0a94`
- 脚本 SHA-256：`984d622eeebbcb17656d157a49272b02fb741593ae2ec624d1926c12d955c8a1`
- Framework 可执行文件 SHA-256：`21547fea1012a1c64db71804a16dc7cc20afee682ca9998e1f976e34223f09cd`

`adapter.rs` 通过 `include_bytes!` 内置归档，初始化时验证归档摘要，再解压到该实例持有的临时目录。
监听进程、主动查询和控制使用同一份资源；退出时回收工作线程与监听进程后删除目录。
构建和运行无需从 Cargo 缓存读取适配器资源，也不修改进程级 TMPDIR 或适配器环境选项。

许可证见同目录 `LICENSE`（BSD 3-Clause），第三方声明见仓库根目录 `THIRD_PARTY_NOTICES.md`。
