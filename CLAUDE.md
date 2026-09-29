# BSc 编译原理实验室 / 黑话编译器

## 这个项目是什么
一个用 Rust 写的**跨平台图形应用**（原生桌面 + Web/GitHub Pages），有两个目标：

1. **编译原理学习（打基础，面向新手）**：知识点做成**可交互的界面**（单步演示、可视化、小测验），不写成长篇文档。由浅入深：每课先讲"入门"层（比喻 + 演示），再给"进阶"层（教材级的形式化内容，如 DFA 最小化、LL/LR、SSA、数据流分析、寄存器分配）。
2. **黑话编译器（主要产品，要有深度）**：把故作高深的"领导讲话"编译成平凡质朴的大白话。必须走真正的编译流水线，不是查字典替换。

用户是编译原理新手；界面和讲解**只用中文**。用户本机是 Steam Deck（SteamOS，可能没有系统 `cc`），所以原生版靠 CI 构建、Web 版部署到 GitHub Pages。

历史：最初由另一个 AI 写了一版（粗糙、名不副实），已**完全推翻重写**。旧代码和 `docs/001–012` 已删除，不要参考。

## 核心设计原则
- **演示即实现**：界面上的每个演示都由真实运行的编译器代码驱动。算法运行时记录下每一步（事件/快照），界面只负责回放。**绝不在界面里另写一套模拟**，也不能写"看起来在跑其实是写死"的内容。尚未实现的东西必须明确标注"规划中/设计示意"。
- **每个阶段的产物都是明确的数据结构**，带源码位置（`Span`），方便界面联动高亮（点指令 → 高亮源码）。
- 错误用结构化的 `bsc_core::Diagnostic`（消息 + 标注 + 注释 + 帮助），不用字符串；既能渲染成 rustc 风格文本，也能在界面上画。错误信息面向新手，给出具体改法。
- **测试从简（用户要求节省额度）**：每个核心算法只保留一组关键的正确性测试，优先"对照测试"（两种独立方法算同一件事，结果必须一致；或与教材经典例子对照）。不写零碎的单元测试和界面测试。浏览器截图检查也只看新页面的关键画面。

## 仓库结构
```
crates/
  bsc-core   公共基础：Span、行列换算、Diagnostic 及其终端渲染
  bsc-calc   计算器语言：最小完整编译器（lexer → 递归下降 parser → AST → 栈机代码生成 → VM），第 0 课和实验台用
  bsc-automata 正则 → NFA（Thompson）→ DFA（子集构造）→ 最小化（Moore 划分细化）→ 多规则词法分析器生成（最长匹配 + 优先级），全部带逐步记录；第 1 章用
  bsc-minilang 教学语言 mini-lang。目前只有手写词法分析器（带逐步记录，1.1 课用）；语法/语义/IR 随课程加入
  bsc-app    eframe/egui 图形界面，原生与 wasm 共用一份代码
    src/app.rs          顶栏 + 三个页面（学习 / 实验台 / 黑话编译器），状态持久化
    src/pages/learn/    课程目录（mod.rs 中的 COURSE）与各课内容（ch0.rs …）
    src/pages/lab.rs    类 Godbolt 的多栏联动视图
    src/pages/jargon.rs 黑话编译器页（目前是设计预览）
    src/calc_view.rs    计算器编译器各阶段的逐步回放视图
    src/automata_view.rs 第 1 章共用：正则流水线、自动机 → 状态图、正则输入框
    src/pages/learn/ch1/ 第 1 章各课（scanner/regex/nfa/dfa/minimize/lexgen）
    src/widgets/        通用组件：Stepper 播放器、源码高亮、树布局、状态图（graph_view：分层布局、自环、回边弧线）、
                        栈、测验、诊断展示、chip 小卡片、prose（反引号 → 行内代码）
    src/theme.rs        语义化配色（亮/暗两套），界面里只用 Palette 的字段，不直接写颜色
    assets/fonts/       裁剪后的思源黑体 + JetBrains Mono（OFL，裁剪方法见其 README）
    index.html          trunk 的入口页
```
计划新增：`bsc-grammar`（FIRST/FOLLOW、LL(1)、LR）、`bsc-jargon`（黑话编译器）；`bsc-minilang` 逐步补上语法分析、语义、CFG/SSA、优化、后端。

## 技术选型（已定）
- 界面：**egui / eframe 0.36**（glow 后端）。注意 0.36 的 API：`App::ui(&mut self, ui, frame)`；面板用 `egui::Panel::left/top(..).show(ui, ..)`、`CentralPanel::default().show(ui, ..)`（`show_inside` 已弃用）。
- 中文字体（约 2.4 MB，gzip 后 1.5 MB）：原生版打包进程序；**Web 版是单独的文件 `fonts/NotoSansSC-Subset.ttf`（文件名固定），启动时 fetch**，这样发新版本时浏览器仍能复用缓存的字体，只需重新下载约 1.3 MB 的 wasm。
- 没有启用 egui 的 `default_fonts`（省 0.8 MB）：所以 egui 自带的 emoji/图标都不可用（比如默认的主题切换按钮是太阳/月亮图标，已换成文字按钮）。界面里用到的所有非 ASCII 字符都必须在两个字体子集里——GB2312 汉字、拉丁/希腊字母、U+2000–206F 标点、U+2190–22FF 箭头与数学符号、U+2500–25FF 制表符与几何图形（▶◀■▲ 可用；✔⏸ 和上标 ⁿ 不可用）。
- egui 踩过的坑（都已在代码里处理，新代码照做）：
  - `ui.set_max_width(x)` 会把区域**撑大**，要写成 `ui.set_max_width(ui.available_width().min(x))`。
  - `ui.columns` 的每一栏是两端对齐布局，egui 两端对齐时会**删掉每行开头的空白**——显示代码要包一层 `Layout::top_down(Align::Min)`（`source_view` 已处理）。
  - `Frame` 是容器，放在 `horizontal_wrapped` 里**不会换行**；一排小卡片用 `widgets::chip`（基于 `Button`）。
  - `Grid` 的列宽取决于上一帧内容，里面的 `TextEdit` 要用 `ui.add_sized` 固定尺寸。
  - 嵌套的纵向 `ScrollArea` 高度不可靠，课程页里别用。
- 代码字体 JetBrains Mono 子集**去掉了连字**（否则 `<=` 显示成 `≤`）。
- Rust edition 2024，MSRV 1.95；`rustfmt.toml` 行宽 120。

## 常用命令
```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p bsc-app --target wasm32-unknown-unknown -- -D warnings
cargo fmt --all
cargo run -p bsc-app                                  # 原生桌面版
cd crates/bsc-app && trunk serve                      # Web 版本地预览（http://127.0.0.1:8080）
cd crates/bsc-app && trunk build --release --public-url ./   # Web 发布构建 → dist/
```
CI（`.github/workflows/`）：`ci.yml` 跑格式/clippy/测试/wasm 检查；`pages.yml` 在 main 分支上构建并发布到 GitHub Pages；`release.yml` 在打 `v*` 标签时构建 Linux/Windows/macOS 原生程序。

## 黑话编译器设计要点（大白话方向为主）
黑话高度公式化（词汇有限、句式固定、修饰语堆叠但不携带信息），所以把它当成一门有文法的"源语言"：
- 词法：黑话词典 + 分类（空洞动词、虚指名词、膨胀修饰语）；分词用 Aho-Corasick + 词图最大概率切分。
- 语法：句式文法（"以 X 为抓手""围绕 X 做好 Y""形成 X 闭环"……），用 Earley 分析器（容忍歧义）。
- 语义 / IR：事件 + 语义角色（谁、做什么、对象、目的、手段），与措辞无关。
- 优化 Pass：死代码消除（删无信息修饰语）、常量折叠（固定搭配 → 简单说法）、公共子表达式消除（排比合并）、降级 lowering（沿上位词图把抽象概念降到具体说法）。每个 Pass 前后可对比。
- 后端：朴素中文生成 + 每处改动的解释 + "水分"统计。
- 输入限定为受控范围（常见讲话句式），超出范围时像编译器一样给出诊断，而不是硬凑输出。
- 反向（大白话 → 黑话）是次要的娱乐功能，复用同一 IR、换后端。
- 它用到的每项技术都应先在课程里讲过。

## 里程碑
- [x] M0 骨架：workspace、eframe 应用（原生 + Web）、中文字体、CI/Pages/Release 工作流、计算器编译器 + 第 0 课 + 实验台 + 黑话编译器设计页
- [x] M1 第 1 章词法：1.1 手写扫描器（mini-lang 词法分析器）、1.2 正则、1.3 NFA/Thompson、1.4 DFA/子集构造、1.5 最小化、1.6 词法分析器生成；状态图组件
- [ ] M2 第 2 章语法：文法工具（FIRST/FOLLOW、LL(1) 表、LR 自动机）+ Pratt；mini-lang 前端
- [ ] M3 第 3 章语义：作用域、符号表、类型检查
- [ ] M4–M5 中间表示与优化：三地址码、CFG、支配树、SSA、数据流框架、各优化 Pass
- [ ] M6 起：黑话编译器 v1；第 6 章后端（字节码 VM、RISC-V 汇编 + 内置模拟器、寄存器分配）

## 协作约定
- 用户已授权技术决策由 Claude 决定；大的方向变化仍先和用户沟通。
- **直接在 `main` 分支上开发和推送**，不要另建分支（用户明确要求）。推送到 main 会自动触发 GitHub Pages 发布。
- 提交信息用中文或英文均可，说清改了什么和为什么。
