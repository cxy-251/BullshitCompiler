# BSc 编译原理实验室

面向新手的**交互式编译原理课程**，以及一个把故作高深的"领导讲话"编译成大白话的**黑话编译器**。

- 用 Rust 编写，界面基于 [egui](https://github.com/emilk/egui)，同一份代码同时编译为原生桌面程序和 WebAssembly。
- 课程里的每个演示都由真实运行的编译器代码驱动：算法边运行边记录，界面逐步回放。

## 现在有什么

| 模块 | 状态 |
|---|---|
| 第 0 课「编译器是什么」：用一个完整的计算器编译器单步演示词法分析、语法分析、代码生成、执行 | 可用 |
| 实验台：输入算式，记号 / 语法树 / 指令 / 结果多栏联动，鼠标指向任意产物即高亮对应源码 | 可用 |
| 第 1 章「词法分析」6 课：手写扫描器、正则表达式、NFA（Thompson 构造）、DFA（子集构造）、DFA 最小化、从正则生成词法分析器——状态图随算法逐步生长 | 可用 |
| 第 2 章「语法分析」6 课：文法与推导、歧义与优先级、递归下降、Pratt 分析法、LL(1) 分析表、LR 分析 | 可用 |
| 第 3 章「语义分析」3 课：作用域与符号表、类型检查、类型推导（Hindley–Milner） | 可用 |
| 第 4 章「中间表示」4 课：三地址码、基本块与控制流图、支配树、SSA 形式 | 可用 |
| 第 3–7 章（语义、中间表示、优化、后端、运行时） | 规划中，目录已列出 |
| 黑话编译器 | 设计中（设计预览页可看） |

## 运行

```sh
# 原生桌面版
cargo run -p bsc-app --release

# Web 版（需要 trunk：cargo install trunk --locked；以及 rustup target add wasm32-unknown-unknown）
cd crates/bsc-app && trunk serve
```

也可以直接从 GitHub Releases 下载 Linux / Windows / macOS 的原生程序，或打开 GitHub Pages 上的 Web 版。

## 目录结构

```
crates/bsc-core   公共基础：源码位置、诊断信息
crates/bsc-calc   计算器语言：最小但完整的编译器
crates/bsc-automata  有限自动机：正则 → NFA → DFA → 最小 DFA → 词法分析器
crates/bsc-grammar   文法工具：Earley、FIRST/FOLLOW、LL(1)、LR(0)/SLR
crates/bsc-minilang  教学语言 mini-lang（词法、语法、语义分析，三地址码、控制流图、支配树、SSA）
crates/bsc-types     迷你 ML + Hindley–Milner 类型推导
crates/bsc-app    图形界面（原生 + Web）
```

## 许可

代码：MIT OR Apache-2.0。内置字体（Noto Sans SC、JetBrains Mono 的子集）：SIL Open Font License 1.1，见 `crates/bsc-app/assets/fonts/`。
