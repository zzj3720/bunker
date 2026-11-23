# Bunker

开源的 SPT（Single Player Tarkov）桌面启动器与 Mod 管理器。界面用 [Pion](https://github.com/AFK-surf/pion) + [invar](https://github.com/AFK-surf/invar)（Liquid Glass 液态玻璃原生 GPU 自绘 UI，Windows 上是 Direct3D 11 / DirectWrite 原生宿主，支持深浅外观切换），本地逻辑在 Rust 写的 core 里，两者编译进同一个 `Bunker.exe`，没有 Electron、没有 Web 运行时、没有后台服务。

An open-source desktop launcher and mod manager for SPT (Single Player Tarkov): a native Pion + invar Liquid Glass window (drawn on the GPU) with a Rust core linked into the same executable.

## 功能

- **多实例**：自动识别 SPT 3.11 / 4.0（`SPT/SPT.Server.exe`、`SPT/user/mods`）、4.1+（`SPT_Runtime/…`）和老版本根目录布局，读出 SPT、EFT、BepInEx 的版本（直接解析 PE 版本资源）。首次运行会在常见位置（各盘 `Games`、`SPT`）找已安装的 SPT。
- **启动与服务端**：拉起 `SPT.Server.exe` 并流式读取它的输出（按颜色和 `[WARN]`/`[ERROR]` 分级计数）；端口通了才算就绪；停止时先给它的控制台发 Ctrl+Break，让服务端自己保存存档，超时才强制结束。通过自签名 HTTPS 调启动器接口（`/launcher/profiles`、`/launcher/profile/login`、`/launcher/profile/register`，新版服务端的 `/launcher/v2/*` 也支持），然后像官方启动器一样带 `-token=<profileId> -config={'BackendUrl':…}` 启动 `EscapeFromTarkov.exe`。可选在启动前清理 `BepInEx/cache` 和 `%TEMP%\Battlestate Games\EscapeFromTarkov`。6969 端口上如果已有别的程序启动的服务端，Bunker 只会识别它，不会去动它。
- **服务端 / 客户端成对的 Mod 管理**：
  - 扫描已装的服务端 Mod（`user/mods`）和客户端插件（`BepInEx/plugins`），按名字配对成一个 Mod（例如 `Solarint-SAIN-ServerMod` ↔ `SAIN`、`WTT-ServerCommonLib` ↔ `WTT-ClientCommonLib`）；
  - 通过 Bunker 安装的 Mod 会写 `.bunker/manifests/<id>.json` 清单，记下每个文件；
  - 禁用：把 Mod 的目录和文件整体移到 `.bunker/disabled/<id>/`，再启用时原样移回，不留残留。卸载按清单删除所有文件；手动装的 Mod 第一次操作时会先生成清单；
  - 安装 `.zip` / `.7z`：先出计划（去掉多余的顶层目录，`SPT/user/mods` 和 `SPT_Runtime/user/mods` 自动换成当前实例的布局，散装 dll 按引用的程序集判断属于服务端还是客户端，列出冲突），确认后才写入；装之前会自动快照存档；手动装的同名文件被覆盖时，原文件移到 `.bunker/replaced/`，不会直接删掉；
  - 可以粘贴 GitHub Releases 链接直接下载安装，也能检查更新（来源接口可扩展，sp-mod 站点已经能识别，但还没有接入）；
  - 统计中文本地化：`ch.json` 和内联的 `"ch": {…}` 词条数，与英文对比，分为完整、部分、未汉化、无需汉化四种。
- **存档快照**：手动带备注快照、装 Mod 前自动快照、一键回滚（回滚前会再备份一次当前存档；服务端运行时拒绝回滚，因为它会用内存里的存档覆盖回去）。
- **配置编辑**：编辑 Mod 配置目录下的 `.json` / `.jsonc`（支持注释和尾逗号）和 `BepInEx/config/*.cfg`。保存前先校验语法，出错时报出行号；原文件保留为 `.bak`。

## 结构

```
bunker/
├── core/                 Rust：实例探测、服务端/游戏、Mod、快照、配置（staticlib，cargo test）
│   ├── src/api.rs        请求分发：{"op": "...", ...} → JSON 应答
│   ├── src/lib.rs        C ABI：bunker_set_post / bunker_call / bunker_call_data
│   ├── src/bin/fake_spt.rs   测试用的假服务端和假游戏（HTTPS、彩色日志、Ctrl+Break）
│   └── tests/e2e.rs      端到端：服务端、登录、启动游戏、GitHub 安装、C ABI
├── pion/
│   ├── cmd/app           入口：窗口（无系统标题栏、首帧画好才显示）、core 消息在每帧前取走
│   ├── cmd/shots         离屏出图：invar 的 Metal 渲染器按 3x 画出各页面和弹层
│   ├── packages/corelink C 桩：core 的应答进队列并立即唤醒窗口（mb_host_wake_now）
│   ├── packages/model    状态（signals）和对 core 的请求
│   └── packages/shell    界面（Pion-JSX + invar `lg`）：SplitShell 侧栏 + 四个 PageShell 页面、四个弹层（同一弹层内逐步切换）
├── assets/               Windows 资源：MBUI_ENV（MBUI_APP_ID=Bunker，着色器缓存到磁盘）
└── tools/                deps.sh（按固定提交拉 AFK-surf/invar 到 pion/deps/invar）、check.sh、build-win.sh、win-cc.c
```

core 在自己的线程上处理每个请求，应答和事件（服务端日志、服务端状态、游戏进程）进 `core_link.c` 的队列并唤醒窗口，窗口在下一帧前（`ui::on_before_frame`）取走，没有定时轮询。UI 只读写 signals，构建期不调 core。

界面按 invar `packages/lg` 的苹果两层结构：页面内容是实色的（`lg::surface()` 色板、`SurfaceCard` / `SurfaceSection` / `liquid_list_node`，里面的按钮都是 flat），液态玻璃只用在浮在上面的东西：顶栏药丸、搜索和筛选条、启动按钮、弹层和提示。

## 构建（Windows）

需要：Git Bash、LLVM（clang，MSVC 目标）+ Visual Studio Build Tools 的库和 Windows SDK、Rust（stable，MSVC）、自举好的 [pionc](https://github.com/AFK-surf/pion)。

```sh
sh tools/build-win.sh        # → out/Bunker.exe
```

脚本按 `tools/deps.sh` 的固定提交拉 invar、编 `core/`、用 clang 编 Pion 运行时和 `assets/app.rc`，再用 pionc 编 `pion/cmd/app`。

## 测试

```sh
cd core && cargo test                                   # core：单元测试和端到端测试（全部在临时目录里跑，不碰真实安装）
PIONC=pionc2 sh tools/check.sh                          # UI：静态门禁（不手写 Glass::liquid）+ pionc check + Tester 门禁
pionc2 build -o out/bunker_shots pion/cmd/shots && BUNKER_SHOTS=shots out/bunker_shots   # macOS：3x 离屏预览图
```

UI 的 Tester 门禁（`pion/packages/shell/src/shell_test.pion`）对四个页面 × 深浅色 × 宽窄窗口和每个弹层的每一步跑 `lg::assert_apple_ui`：玻璃不叠玻璃、内容层玻璃不超过 45%、滚动区没有失效、同心圆角、热区和文字不溢出。

CI（`.github/workflows/ci.yml`）在 GitHub 免费 runner 上跑 core 的测试。

## 数据位置

- 实例列表和设置：`%APPDATA%\Bunker\instances.json`（可用 `BUNKER_DATA` 环境变量改到别处）
- 每个实例自己的：`<实例>\.bunker\{manifests,disabled,snapshots,staging,replaced}`

## 许可

MIT
