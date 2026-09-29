# Bullshit Compiler System (BSc)

## 背景
本项目由另一个 AI agent 起步，用户认为**当前实现很粗糙，完全没达到预期**，现交由 Claude 接手重做/提升。
原始需求过程文档（docs/001–012）只保存在用户本地，未入库；下面是其要点。动手做大改动前，先和用户确认方向与优先级。

## 产品目标
1. **黑话编译器（核心卖点）**：用真正的编译器流水线（而不是关键词字典替换）把"大白话"编译成"大厂黑话"。
   - 前端：词法 → 递归下降语法分析 → AST → 语言无关的语义中间表示 **Intent-IR（SSA 形式）**
   - 中端：PassManager + 优化 Pass（如 StripAgency 去主体化、ScaleAmplify 规模膨胀、TeleologyInject 目的论注入）
   - 后端：多目标"指令选择/发射"（huawei / alibaba / state_owned / silicon_valley 等风格）
   - **反编译器（"照妖镜"）**：黑话 → 大白话
2. **经典编译器（bsc-classic）**：一门微型命令式语言的完整流水线 Lexer → Parser → 语义/类型 → 3AC/IR → 优化 → x86 codegen / 字节码 VM，用于教学对照。示例见 `examples/*.lang`。
3. **编译原理教学实验室**：面向非专业者的交互式科普（词法、AST、SSA、优化 Pass 等），用户要求理论深度达到教材级（龙书/虎书/EaC/TAPL 水准：DFA 最小化、LL/LR、HM 类型推导、支配树与 SSA 构造、数据流分析、寄存器分配、GC 等），不要浅表占位内容。
4. **Web 展示**：类似 Godbolt 的 Compiler Explorer 分屏联动（源码 / IR / 输出三栏高亮联动、Pass 前后 Diff 回放），浏览器内通过 WASM 运行 Rust 内核。

## 硬性要求
- 语言：**Rust**（Cargo workspace），Web 端经 WASM 调用同一内核。
- 定位：工业级工程质量，架构对标 LLVM / rustc，拒绝脚本式拼凑的玩具。
- 跨平台：WASM + 原生 CLI（`bsc`）+ 原生 TUI（`crates/bsc-tui`）。
- 用户本机是 Steam Deck（SteamOS，只读根文件系统，可能无系统 cc）。

## 仓库结构
- `crates/bsc-core` AST / Intent-IR / Span / 类型
- `crates/bsc-frontend` 黑话编译器前端（lexer, parser, ir_builder）
- `crates/bsc-opt` Pass 管理与各 Pass
- `crates/bsc-backend` 各风格发射器
- `crates/bsc-decompiler` 反编译器
- `crates/bsc-classic` 经典编译器全流程
- `crates/bsc-cli` / `bsc-tui` / `bsc-wasm` 各入口
- `web/index.html` + `web/bsc_wasm.wasm` 单页 Web 前端（Tailwind CDN）

## 常用命令
```
cargo build
cargo test
cargo run -p bsc-cli -- --help
```
