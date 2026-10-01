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
  bsc-grammar  上下文无关文法工具：文法文本解析、Earley（任意 CFG，给出全部语法树）、最左推导、随机造句、FIRST/FOLLOW、LL(1)、LR(0)/SLR；第 2 章用。Earley 以后也给黑话编译器用
  bsc-minilang 教学语言 mini-lang：手写词法分析器（1.1 课）+ 语法分析器（语句递归下降、表达式 Pratt、恐慌模式与插入式错误恢复；2.3/2.4 课）+ 语义分析 sema.rs（作用域链、符号表、名字解析、类型检查，Error 类型防连锁报错；3.1/3.2 课）
               + ir（三地址码）→ cfg（基本块、控制流图）→ dom（支配树、支配边界）→ ssa（半剪枝 SSA）；interp 直接执行控制流图，用来对照测试（第 4 章）
               + dataflow（通用 gen/kill 工作表求解：活跃变量、到达定值）→ opt（局部常量/复制传播、折叠、代数化简、分支折叠、DCE）→ sccp（第 5 章）
               + bytecode（栈式字节码 + VM）、rv（RISC-V 指令选择）→ regalloc（线性扫描/图着色、溢出、栈帧）→ sim（RV64 子集模拟器）（第 6 章）。AST 是统一的节点数组（NodeKind + children）
  bsc-runtime  垃圾回收模拟器：小脚本描述堆操作，引用计数 / 标记-清除 / 三色增量标记（写屏障可开关），每步堆快照；第 7 章用
  bsc-types    迷你 ML（不写类型的函数式小语言）+ Hindley–Milner 类型推导（合一、occurs check、实例化、let 泛化，全程事件记录）；3.3 课用
  bsc-jargon   黑话编译器：lexicon/（分层词典文本：core.txt 手工核心层 + ai-NNN.txt AI 扩充批次）→ lexicon.rs（加载与校验）
               → ac.rs（Aho-Corasick）→ segment.rs（分句、词图最小代价切分）→ grammar.rs + earley.rs（句式文法、加权 Earley）
               → ir.rs（语义角色）→ passes.rs（常量折叠 / 死代码消除 / 降级+句式改写 / 公共子表达式消除，记录每处改动）
               → emit.rs（拼回中文）；lib.rs 的 Compiler 串起全流程，含示例 PRESETS、示例问题批次、AI 提示词
               corpus.rs：语料 → 待办词表（未收录片段两遍统计 + 四字对齐切分；词典词 + 未知字拼成的四字格），
               参考译文集 reference/reference.txt（人写的大白话，界面上对照；原文也跑不变式测试）
  bsc-app    eframe/egui 图形界面，原生与 wasm 共用一份代码
    src/app.rs          顶栏 + 三个页面（学习 / 实验台 / 黑话编译器），状态持久化
    src/pages/learn/    课程目录（mod.rs 中的 COURSE）与各课内容（ch0.rs …）
    src/pages/lab.rs    实验台：mini-lang 全流程浏览器（编辑器带语法着色；记号/语法树/语义/三地址码/控制流图/SSA/优化/RISC-V/运行 各阶段标签页，指向产物高亮源码，五种执行方式对照输出）
    src/pages/jargon.rs 黑话编译器页：输入/译文/水分统计 + 翻译过程、分词词图、句式树、语义角色、词典、扩充词典（校验 + 回归对比 + AI 提示词）、原理
    src/calc_view.rs    计算器编译器各阶段的逐步回放视图
    src/automata_view.rs 第 1 章共用：正则流水线、自动机 → 状态图、正则输入框
    src/pages/learn/ch1/ 第 1 章各课（scanner/regex/nfa/dfa/minimize/lexgen）
    src/pages/learn/ch2/ 第 2 章各课（grammar/ambiguity/rd/pratt/ll1/lr；replay.rs 回放 mini-lang 分析事件）
    src/pages/learn/ch3/ 第 3 章各课（scope/typeck/infer；mod.rs 里的 Front 跑一遍 mini-lang 前端）
    src/pages/learn/ch4/ 第 4 章各课（tac/cfg/dom/ssa；mod.rs 里的 IrFront、控制流图画法、基本块卡片）
    src/pages/learn/ch5/ 第 5 章各课（fold/dataflow/dce/lattice/sccp；mod.rs 里的例子程序和 run_check：用解释器对照优化前后）
    src/pages/learn/ch6/ 第 6 章各课（vm/isel/regalloc/calling）
    src/pages/learn/ch7/ 第 7 章各课（rc/mark/tricolor；mod.rs 里的 GcDemo：脚本 + 回放 + 堆图）
    src/grammar_view.rs 第 2 章共用：文法编辑器、句子切分、产生式列表
    src/widgets/        通用组件：Stepper 播放器、源码高亮、树布局、状态图（graph_view：分层布局、自环、回边弧线）、
                        栈、测验、诊断展示、chip 小卡片、prose（反引号 → 行内代码）
    src/theme.rs        语义化配色（亮/暗两套），界面里只用 Palette 的字段，不直接写颜色
    assets/fonts/       裁剪后的思源黑体 + JetBrains Mono（OFL，裁剪方法见其 README）
    index.html          trunk 的入口页
```

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
  - 说明文字里用了反引号或 `**强调**` 的，必须用 `widgets::prose`（`ui.label` 会原样显示反引号）。
- 代码字体 JetBrains Mono 子集**去掉了连字**（否则 `<=` 显示成 `≤`）。
- 子集范围内的码位**不一定真有字形**（思源黑体只有 GB2312 里的数学符号，比如有 ⊥ 没有 ⊤）。新用了特殊符号，用下面的命令检查源码里有没有字体缺的字符（没有输出才对）：
  `python3 -c "from fontTools.ttLib import TTFont;import glob;m=TTFont('crates/bsc-app/assets/fonts/NotoSansSC-Subset.ttf').getBestCmap();print({c for f in glob.glob('crates/*/src/**/*.rs',recursive=True) for c in open(f).read() if ord(c)>127 and ord(c) not in m})"`
  缺的符号可以像 `assets/fonts/add_glyphs.py` 那样从现有字形派生。
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
CI（`.github/workflows/`）：`ci.yml` 跑格式/clippy/测试/wasm 检查，**只能手动运行**（用户不想每次推送都跑、失败发邮件）；`pages.yml` **只在版本号变化时**构建并发布到 GitHub Pages；`release.yml` 在**次版本号变化**时构建 Linux/Windows/macOS 原生程序并建立 GitHub Release（标签 `v主.次.修订`，自动创建）。
**在 Steam Deck 上不要本地编译/测试**（资源不够；在 Mac 上能不能编译先问用户）。平时推送不触发任何构建；想让用户看效果时改修订号，Pages 构建会顺带检查能不能编译，用 `gh run` 看结果。本地最多跑一下 `cargo fmt --all`。不要主动去手动运行 ci.yml。

## 版本号与发布（用户定的规则）
版本号只有一处：根目录 `Cargo.toml` 的 `[workspace.package] version`（界面右上角显示 `v0.1.x`）。格式 `主.次.修订`，从 0.1.0 开始。
- **平时提交不改版本号** → 只跑 CI，不部署 Pages。
- **修订号（第三位）+1** → 触发 Pages 部署。**攒够一波功能再改**（用户要求，不要每做完一块就部署），由 Claude 来改。
- **次版本号（第二位）+1、修订号归零** → 阶段性进展，**由用户说了才改**；除了部署 Pages，还会发布 GitHub Release（三个平台的原生程序）。
- 主版本号（第一位）暂不动。
- Release 的附件必须是**各平台原生安装包**（用户明确要求，不要只发压缩包）：Windows `.msi` + NSIS 安装程序 `.exe`、macOS `.dmg`、
  Linux `.AppImage` + `.deb`；另附各平台**便携版**（解压即用）和 `SHA256SUMS.txt`。不附更新日志（用户不想公开提交信息）。
  目标是"装上就能用，不用另装环境"：Windows 静态链接 C 运行库（crt-static），字体打包在程序里。
  安装包由 cargo-packager 生成（配置在 `crates/bsc-app/Cargo.toml` 的 `[package.metadata.packager]`）。
- `pages.yml` 比较推送前后的版本号，不同才构建部署；也可以在 Actions 页面手动运行。

## 黑话编译器设计要点（大白话方向为主）
两层结构（用户定的方向）：**底层映射关系**（词典）可以让便宜的 AI 批量生成、随时扩充；Claude 负责给这层关系定"模子"，
并做好编译框架——把逐词替换 / 普通 AI 改写做不好的事做好，同时扛住映射关系快速膨胀带来的问题。
- **词典格式**：一行 `词 | 类别 | 映射 | 备注`。类别：修饰（删）、空动 / 空转 / 虚名（映射写上位词，降级时沿链走到朴素说法）、
  固定 / 固定动（映射写整个搭配的大白话）、实动、内容（普通词，用来保护里面的单字不被切成功能词）、功能词（只在核心层）、
  缩略（放管服、三农：映射写展开后的完整说法，常量折叠时展开，不删）、
  模板（`坚持{X}不动摇 | 模板 | 一直坚持{X}`：带槽位的套话句式，加载时变成句式文法规则 `套话 -> 「坚持」 槽 「不动摇」`；
  字面部分没收录的自动登记成"内容"词，单字字面部分必须已在词典里 E3016；槽位格式 E3014；映射里每个槽位正好一次 E3015）。
  模板让 Earley 每个位置可以有多个候选终结符（类别 + 字面终结符），由代价选（套话块 4 < 普通块 10）。
  新批次放 `crates/bsc-jargon/lexicon/ai-NNN.txt` 并加进 `lexicon::builtin_layers()`。
- **膨胀对策（加载时校验，错误码 E/W/N30xx）**：层优先级（核心层优先，后批覆盖前批，W3004/N3004）、成环 E3007、
  链过长 N3009、映射终点仍含黑话 W3008（用编译器自己的分词器切，不用子串匹配）、单字/功能词只能核心层 E3010/E3003、
  映射过长 N3011；AC 匹配与词典大小无关；界面上每加一批自动做回归对比。
- **流水线只依赖类别，不依赖具体的词**：句式文法的终结符是类别（动、名、修饰、以、为……），扩充词典不用改文法和代码。
- 分词：AC 找候选词 → 词图 DP（词典词 10、未知字 12、空白 0）→ 连续未知字合并成普通词（原样保留）。
- 句式：加权 Earley（块 10；无动词块 +8；话题在前 +3；以 X 不带为 +3；修饰当名词、情态进谓词 +1）。
  分析失败 → W3101，**退回逐词处理**（ir::fallback：整句一个块，只删修饰语、折叠固定搭配、降级单词，不做句式改写/空块删除/动词堆叠/排比合并）——真实公文总有认不出的句子，任何输入都要有输出。
- 真实文档格式：换行原样保留（标点串里带着 \n）；行首编号/项目符号（一是、二要、1.、（三）、第一、首先、•）拆成 marker 原样输出；数字后的 `.` 不算句号。
- Pass 顺序：常量折叠（含套话模板整块替换、缩略语展开）→ 死代码消除（修饰语；"空转 + 只有虚名"整块删；悬空的"的/和"）→ 降级（先句式改写：以 X 为抓手 → 从 X 入手、
  以 X 为引擎 → 靠 X 推动、形成业务闭环 → 让业务有完整流程；再沿上位词链）→ 公共子表达式消除（动词堆叠、排比合并、重复分句）。
- 生成：没改过的块按原词序拼接，所以未优化 IR 生成的就是原文（自检）；整句删光时把"我们要"交给下一个分句。
- 不变式测试（lib.rs）：未优化 IR 还原原文、无残留黑话、未收录的内容词一个不少、编译幂等（再编译一遍不变）。
  幂等测试很有用：它抓到过"一起使劲"被切成"使→让"这类问题——修法是在核心层用"内容"类登记保护词。
- 应用功能：粘贴或把 txt 拖进窗口（Web 版异步读取）；复制译文；原生版可保存「原名.大白话.txt」（含每处改动说明），
  本地词典文件夹 `eframe::storage_dir("BSc 编译原理实验室")/lexicon/*.txt` 启动时自动加载，粘贴的批次可一键存成 batch-NNN.txt。
- 规划中：反向（大白话 → 黑话），复用同一 IR、换后端；可选的"AI 改写 + 编译器检查"模式。

## 里程碑
- [x] M0 骨架：workspace、eframe 应用（原生 + Web）、中文字体、CI/Pages/Release 工作流、计算器编译器 + 第 0 课 + 实验台 + 黑话编译器设计页
- [x] M1 第 1 章词法：1.1 手写扫描器（mini-lang 词法分析器）、1.2 正则、1.3 NFA/Thompson、1.4 DFA/子集构造、1.5 最小化、1.6 词法分析器生成；状态图组件
- [x] M2 第 2 章语法：2.1 文法与推导、2.2 歧义与优先级、2.3 递归下降、2.4 Pratt、2.5 LL(1)、2.6 LR；mini-lang 语法分析器
- [x] M3 第 3 章语义：3.1 作用域与符号表、3.2 类型检查、3.3 类型推导（HM）
- [x] M4 第 4 章中间表示：4.1 三地址码、4.2 基本块与控制流图、4.3 支配树、4.4 SSA
- [x] M5 第 5 章优化：5.1 常量折叠与代数化简、5.2 数据流分析、5.3 死代码消除、5.4 格与不动点、5.5 SCCP
- [x] M6 第 6 章后端：6.1 栈式虚拟机、6.2 指令选择、6.3 寄存器分配、6.4 调用约定；第 7 章运行时：7.1 引用计数、7.2 标记-清除、7.3 三色标记与写屏障（课程第 0–7 章全部完成）
- [x] M7 黑话编译器 v1：分层可扩充词典（带校验）、AC 分词 + 词图、加权 Earley 句式分析、语义角色 IR、四个优化 Pass、生成与自检、黑话编译器页
- [x] 0.2.0 阶段收尾：逐词处理兜底、真实文档格式（分段/编号/标题）、拖入 txt、复制/保存译文、本地词典文件夹
- [x] 0.2.1–0.2.2 词典扩充框架：语料 → 待办词表、参考译文集、缩略语、套话模板
- [ ] 之后：见 `docs/黑话词典计划.md`（下一步是命令行工具 `bsc-jargon todo/check/run`，让 agy 全程自己扩词典）；反向编译（大白话 → 黑话）

## 协作约定
- 用户已授权技术决策由 Claude 决定；大的方向变化仍先和用户沟通。
- **直接在 `main` 分支上开发和推送**，不要另建分支（用户明确要求）。推送到 main 会自动触发 GitHub Pages 发布。
- 提交信息用中文或英文均可，说清改了什么和为什么。
