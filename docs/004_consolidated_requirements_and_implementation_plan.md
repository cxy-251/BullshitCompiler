# 004: 用户需求归档整理、技术规范收敛与系统工程实施记录

> **工程全称**：`Bullshit Compiler System`（项目根目录：`bullshit-compiler`，CLI：`bsc`）  
> **文档编号**：`DOC-004`  
> **创建时间**：2026-09-28  
> **文档属性**：需求更正归档、工程环境审计与落地实施跟踪表

---

## 目录
1. [用户新增与更正需求系统归档](#1-用户新增与更正需求系统归档)
2. [本地宿主环境审计与基础设施发现](#2-本地宿主环境审计与基础设施发现)
3. [关键工程挑战与应对策略 (Issue Tracker)](#3-关键工程挑战与应对策略-issue-tracker)
4. [工程全景落地架构与分工拆解](#4-工程全景落地架构与分工拆解)
5. [第一阶段交付计划与代码落地清单](#5-第一阶段交付计划与代码落地清单)

---

## 1. 用户新增与更正需求系统归档

依据用户的最新指示，对前期方案中不够严谨、粗糙的部分进行全面纠偏并建立永久归档：

| 需求编号 | 提出时间 | 变更性质 | 原始状态 / 痛点 | 更正与收敛后的正式标准 |
| :--- | :--- | :--- | :--- | :--- |
| **REQ-01** | 16:55 | **项目命名规范** | 目录及方案使用了非正式缩写 `bsc`，含义不明确 | 根目录更正为全称 **`bullshit-compiler`**；文档与代码中全面阐明全称 *Bullshit Compiler System*；`bsc` 仅保留作为编译器的命令行二进制调用名。 |
| **REQ-02** | 16:55 | **工程量级定位** | 警惕脚本级、地摊式的粗糙拼接玩具 | **大型工业级工程标准**：严守编译原理（形式文法 EBNF $\to$ AST $\to$ 强类型 SSA 语义 IR $\to$ Pass 流水线 $\to$ 模式匹配发射），架构设计对标 LLVM / Rustc。 |
| **REQ-03** | 16:55 | **交互形态要求** | 缺乏统一的图形交互展示 | **跨平台 Web 交互展示**：打造类似 Godbolt 的 Compiler Explorer 分屏联动系统，支持三栏代码与高亮联动、Pass 状态回放。 |
| **REQ-04** | 17:04 | **编程语言选型** | 在 Rust、Go、Python 间权衡讨论 | **终极敲定：全面采用 Rust**。兼顾无 GC 原生性能与纯浏览器端 WebAssembly (WASM) 运行能力。 |
| **REQ-05** | 17:04 | **产品呈现形态升级**| 直接进入黑话编译器可能让非编译专业者难以理解底层精妙 | **确立“双阶段沉浸式体验”体系**：<br>① **阶段一：编译原理通识实验室**（动画化、交互式全面科普词法、AST、SSA IR、优化 Pass 底层逻辑）；<br>② **阶段二：BSc 编译器工作台**（正向升维编译、Pass Diff 调试、反向去魅反编译）。 |
| **REQ-06** | 17:08 | **文档演进规范** | 方案分散，变更缺乏连续性 | **按序号（001 $\to$ 002 $\to$ 003 $\to$ 004...）顺延归档**，遇到环境或技术阻碍一律白纸黑字沉淀至 docs 中，闭环跟踪。 |

---

## 2. 本地宿主环境审计与基础设施发现

在工程正式落地（创建 Cargo Workspace 与 Web 工程）的过程中，对宿主机环境进行了实地探测：

* **操作系统**：Linux x86_64（SteamOS / Arch Linux 系，Steam Deck 终端环境）；
* **用户与路径**：用户为 `deck`，工程目录位于 `/home/deck/Games/agy/bullshit-compiler/`；
* **现有运行时储备**：
  * **Node.js**：`/home/deck/.local/bin/node`（已就绪，可驱动 Web 前端开发与 Vite 构建）；
  * **npm**：`/home/deck/.local/bin/npm`（已就绪）；
  * **Python**：`/usr/bin/python3`（已就绪，Python 3.13）；
  * **Go**：`/home/deck/.local/go/bin/go`（版本 1.27.1，已就绪）；
  * **Rust / Cargo**：未预装系统级 Rust，正在通过用户空间 `rustup` 进行免 root 独立部署。

---

## 3. 关键工程挑战与应对策略 (Issue Tracker)

在大型系统工程落地过程中，记录遇到的环境与技术卡点及解决方案：

### 【Issue #001】：宿主系统缺乏全局 GCC/Clang 对 Rust 原生编译的影响
* **现象发现**：SteamOS 默认采用不可变只读根文件系统（Read-Only Rootfs），未预装全局 `gcc` 或 `clang` 作为 C 链接器；
* **技术风险**：标准 Rust 在 Linux 目标平台（`x86_64-unknown-linux-gnu`）下编译二进制时，默认依赖系统 `cc` 作为 Linker，若缺失可能导致编译最终阶段报 `linker 'cc' not found`；
* **工程攻坚方案**：
  1. **方案 A（内置 Linker）**：在 Cargo 配置 `.cargo/config.toml` 中配置使用 Rust 自带的 LLVM 链接器 `rust-lld`：
     ```toml
     [target.x86_64-unknown-linux-gnu]
     rustflags = ["-C", "linker=rust-lld"]
     ```
     彻底摆脱对系统 `gcc/clang` 的外部依赖！
  2. **方案 B（WASM 无链接器依赖）**：编译目标为 `wasm32-unknown-unknown` 时，Rust 编译器完全使用内置的 `lld`，天然零环境依赖；
  3. **方案 C（纯前端沙盒优先）**：由于用户要求“Web 方式展示效果”，Web 端通过 Vite + WASM 直接在浏览器内运行内核，不受宿主本地 C 库限制。

---

## 4. 工程全景落地架构与分工拆解

工程采用标准 Monorepo 模块化拆分规范：

```text
bullshit-compiler/
├── docs/                                          # 文档与需求归档库
│   ├── 001_compiler_architecture_design.md        # 001 初版概念方案
│   ├── 002_large_scale_cross_platform_compiler_and_web_architecture.md # 002 工业级架构规范
│   ├── 003_tech_stack_selection_and_key_challenges.md                  # 003 技术栈与重难点分析
│   └── 004_consolidated_requirements_and_implementation_plan.md        # 004 本工程实施规划
├── crates/                                        # Rust 编译核心 Workspace
│   ├── bsc-core/                                  # AST 语法树、SSA IR 指令集、Span 源码位置
│   ├── bsc-frontend/                              # 词法切分、容错递归下降语法分析器
│   ├── bsc-opt/                                   # PassManager、去人性化Pass、时空膨胀Pass
│   ├── bsc-backend/                               # 指令选择器、华为/阿里/国企/硅谷发射器
│   ├── bsc-decompiler/                            # 照妖镜反编译器
│   └── bsc-wasm/                                  # 供 Web 端调用的 WASM 导出层
├── web/                                           # 交互式 Web 应用 (Vue 3 + TS + TailwindCSS)
│   ├── src/
│   │   ├── modules/
│   │   │   ├── theory-lab/                        # 【阶段一】：编译原理沉浸式互动教学实验室
│   │   │   └── compiler-studio/                   # 【阶段二】：BSc 语义升维与去魅编译器工作台
│   │   ├── components/                            # Monaco 编辑器、AST 树渲染器、Diff 步进器
│   │   └── wasm/                                  # 桥接 Rust WASM 编译内核
│   └── package.json
└── Cargo.toml                                     # Rust Workspace 统一配置
```

---

## 5. 第一阶段交付计划与代码落地清单

1. **环境与配置落地**：
   * ✅ 部署轻量级 C 编译器工具链 `cc` 并安装 Rust 工具链；
   * ✅ 初始化根目录 `Cargo.toml` 工作区；
2. **核心编译模块落地（`crates/`）**：
   * ✅ `bsc-core`：已实现 `Span`、`ASTNode`、`IROpcode` 与 `IntentModule`（SSA 形式）；
   * ✅ `bsc-frontend`：已实现词法切分 `Lexer`、递归下降语法解析 `Parser` 与 `IRBuilder`；
   * ✅ `bsc-opt`：已实现 `PassManager`、`StripAgencyPass`、`ScaleAmplifyPass` 与 `TeleologyInjectPass`；
   * ✅ `bsc-backend`：已实现多目标指令发射（`huawei`, `alibaba`, `state_owned`, `silicon_valley`）；
   * ✅ `bsc-decompiler`：已实现照妖镜反编译器；
   * ✅ `bsc-cli`：已编译输出二进制 `target/debug/bsc`，端到端编译与反编译通过测试；
3. **Web 前端双阶段系统落地（`web/`）**：
   * ✅ 已构建完成包含【阶段一：编译原理通识实验室】、【阶段二：BSc 编译器工作台】与【阶段三：照妖镜反编译器】的全景 Web 应用；
   * ✅ 本地服务已常驻运行于 `http://localhost:8088`，浏览器与 iPad 均可随时访问交互体验！

