# CodeForge Engine

面向本地命令行的代码分析、变更预览、可配置验证与证据记录工具。

[English](README.md) · [文档目录](docs/README.md) · [已发布 v1.1.1](https://github.com/styayur/codeforge-engine/releases/tag/v1.1.1) · [变更记录](CHANGELOG.md)

**阶段：Alpha。** 已实现流程有自动验证，但独立用户安装、升级与持续使用验收尚未完成。下列示例使用已发布的 v1.1.1 CLI；默认分支后续改动不会自动进入该版本。

## 功能与边界

CodeForge 将分析、按需上下文、变更事务、项目检查与执行证据放进同一个本地流程。它不是 IDE 或自主编码代理；内置分析可离线运行，可选编译器与分析器由用户另行安装。

| 范围 | 已实现能力 | 边界 |
|---|---|---|
| 有回归覆盖 | 多语言解析与审阅 | Tree-sitter 与指定规则，不同语言覆盖范围不同 |
| 有回归覆盖 | 变更预览、事务与回滚 | 必须审阅补丁，验证要求依工作流而定 |
| 有回归覆盖 | 可配置检查、基线、Fleet、SARIF 与证据包 | 缺失或跳过的工具不会自动算通过；Fleet 不是跨仓库原子事务 |
| Experimental | 上下文提取、影响与测试建议 | 精确片段配合启发式关系图，覆盖有界，不代表完整语义分析 |
| Experimental | 外部 SARIF 导入、可选 CodeQL CLI 适配 | 外部证据不可信，真实 CodeQL 集成尚未验证 |
| Future | 增量解析缓存、更广语义解析与验证器集成 | 尚未承诺实现、安装或执行 |

英文版表格中的 Stable 仅表示特定流程已有实现与回归覆盖，不把整体项目升为稳定产品，也不表示规则或变换均已得到语义证明。各语言和工具状态见[分析引擎](docs/ENGINES.md)。

```text
仓库 → 分析引擎 → 诊断与已审核基线
                  ↓ 按需上下文
             变更预览 → 事务
                  ↓
        项目配置的语法/构建/测试等检查
                  ↓
        证据与策略决定 → 应用或回滚
```

CLI 与桌面共用核心；上下文不是每次扫描的必经步骤。详见[架构](docs/ARCHITECTURE.md)。发现问题不代表证明修复正确，成功执行某个等价性命令也不是工具内置的程序等价证明。自动应用前阅读[证据模型](docs/EVIDENCE_MODEL.md)。

## 安装 / 快速开始

从 [v1.1.1 Release](https://github.com/styayur/codeforge-engine/releases/tag/v1.1.1) 下载对应平台的 CLI，按 `checksums.txt` 校验，解压 `codeforge`（Windows 为 `codeforge.exe`）。同页提供桌面包；Windows 桌面需要 WebView2，Linux 桌面需要 WebKitGTK。软件包不自动安装可选语言工具。

从 Tag 源码安装 CLI：

```bash
git clone --branch v1.1.1 --depth 1 https://github.com/styayur/codeforge-engine.git
cd codeforge-engine
cargo install --locked --path crates/codeforge-cli
codeforge --version
```

workspace 声明 Rust 1.90 为最低版本，CI 对该 MSRV 有实际检查；其他平台前提见[开发说明](docs/DEVELOPMENT.md)。在待检查仓库中运行：

```bash
codeforge scan .
codeforge review . --json
codeforge review . --sarif > codeforge.sarif
codeforge refactor .
codeforge apply . --rule RS-PERFORMANCE-001 --file src/lib.rs
codeforge apply . --rule RS-PERFORMANCE-001 --file src/lib.rs --apply --allow-unsafe
codeforge history .
codeforge undo . --transaction <transaction-id>
codeforge verify .
codeforge baseline show --file .codeforge/baseline.json
codeforge ci . --changed --baseline .codeforge/baseline.json --fail-on new
```

`refactor` 展示建议；`apply` 默认仅预览，拒绝歧义选择。加入 `--apply` 后通过语法检查再记录可回滚事务；需人工审核的修复还要求显式 `--allow-unsafe`。这些不是首次查看代码时必须执行的写入操作。`undo` 不会覆盖应用后又被改动的文件。

`verify` 执行可信项目配置的命令，可能写构建产物或联网。在陌生仓库执行前先审核配置，参阅[验证](docs/VERIFICATION.md)和[基线设置](docs/baseline.md)。不能仅为了让 CI 变绿而生成基线。

v1.1.1 还提供以下实验性入口：

```bash
codeforge context . --query verification --limit 10 --max-bytes 32768
codeforge impact . --depth 4 --graph
codeforge import-sarif report.sarif --root . --language rust
codeforge fleet audit --config fleet.toml
codeforge fleet refactor --config fleet.toml --risk low --dry-run
```

SARIF 文件或 [Fleet 配置](docs/FLEET.md)需自行提供。Context 和 impact 读取源码而不应用修改，测试建议不能替代完整验证策略。[SARIF 说明](docs/SARIF.md)记录外部证据边界。

## 测试 / CI / 平台支持

[![CI](https://github.com/styayur/codeforge-engine/actions/workflows/ci.yml/badge.svg)](https://github.com/styayur/codeforge-engine/actions/workflows/ci.yml)

[开发说明](docs/DEVELOPMENT.md)列出当前测试和构建命令，现有 CI 检查 Windows、Ubuntu 与 Rust 1.90，安全工作流检查 Rust/前端依赖和秘密扫描。检查成功不等于所有桌面安装环境均通过验收。扩展规则应复用原生解析器、协议、进程运行器和事务边界；[插件说明](docs/PLUGINS.md)区分清单校验与进程隔离，任意清单安装不构成完整 SDK。

## 已知限制

- 上下文与影响关系是有界启发式，不是完整语义解析；外部 SARIF 不可信，真实 CodeQL 集成未验证。
- 外部命令继承宿主权限与环境，超时和输出上限不是 OS 沙箱。工具发现可能执行版本探测。
- 证据包可能包含路径、源码和命令输出，分享前需审核；Fleet 发布到 GitHub 必须显式选择 PR 模式。
- 无需账户、遥测、托管服务或模型；桌面跨机器安装与持续使用仍需独立验收。

安全细节见[安全模型](docs/SECURITY.md)，性能报告见[方法与测量](docs/benchmarking.md)，不承诺默认扫描提速。

## 路线图与贡献

[路线图](docs/ROADMAP.md)区分发布门禁与未来方向，不承诺交付日期。阅读 [CONTRIBUTING.md](CONTRIBUTING.md)及[开发说明](docs/DEVELOPMENT.md)，漏洞按 [SECURITY.md](SECURITY.md) 报告。

## 许可证

CodeForge 可独立使用，不依赖 LCE 运行时或 crate。代码采用 [Apache-2.0](LICENSE)，可选工具保留[各自许可](THIRD_PARTY_LICENSES.md)。当前 LICENSE 恢复规范的 Apache 正文与附录，并保留原有版权声明；这是非标准抄录的修正，不是新的许可选择。历史 Release 和 Tag 未修改，详见[许可历史](docs/LICENSE_HISTORY.md)。
