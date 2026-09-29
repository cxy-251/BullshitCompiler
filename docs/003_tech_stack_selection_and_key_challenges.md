# 003: 核心技术栈精细化选型、产品双阶段 UI 体系与关键技术攻坚规范

> **工程全称**：`Bullshit Compiler System`（工程目录：`bullshit-compiler`，CLI：`bsc`）  
> **文档编号**：`DOC-003`  
> **状态**：技术架构与重难点分析（正式审定稿）  
> **核心决策**：
> 1. 全面确立 **Rust** 作为系统核心底层实现语言（兼顾原生跨平台最高性能与 WebAssembly 纯客户端直跑能力）；
> 2. 产品形态确立**双阶段 UI 体系**：**第一阶段（编译原理沉浸式互动教学实验室）** $\to$ **第二阶段（BSc 语义升维与去魅编译器控制台）**；
> 3. 对工程中涉及的**形式文法模糊性、SSA 语义 IR 正交性、Pass 依赖调度图、WASM 极致轻量化与高频跨端联动**等 5 大核心技术重难点进行深度穿透分析并给出工业级攻坚方案。

---

## 目录
1. [产品双阶段交互式 UI 体系设计 (Two-Stage Web Experience)](#1-产品双阶段交互式-ui-体系设计-two-stage-web-experience)
2. [全栈技术选型与架构分工矩阵 (Full Tech Stack Matrix)](#2-全栈技术选型与架构分工矩阵-full-tech-stack-matrix)
3. [核心重难点深度剖析与攻坚方案 (Deep Dive: Key Technical Challenges)](#3-核心重难点深度剖析与攻坚方案-deep-dive-key-technical-challenges)
   - 3.1 难点一：自然语言模糊歧义性 vs 编译器确定性语法 (NLP-to-Compiler Ambiguity)
   - 3.2 难点二：语义中间表示 (Intent-IR) 的规范度与正交抽象性 (IR Orthogonality & SSA Design)
   - 3.3 难点三：中端 Pass 流水线的依赖管理与中间状态回滚 (Pass Dependency & Invalidation)
   - 3.4 难点四：Rust 编译至 WebAssembly 的极致体积压缩与轻量化 (WASM Footprint & 60fps Profiling)
   - 3.5 难点五：三栏联动高亮 (Cross-Highlighting) 与源码映射追踪 (Source Spans Tracking)
4. [工程跨平台交付形态规划](#4-工程跨平台交付形态规划)
5. [下一步具体执行计划](#5-下一步具体执行计划)

---

## 1. 产品双阶段交互式 UI 体系设计 (Two-Stage Web Experience)

为满足用户提出的“**首先通过 UI 界面完整介绍编译原理、底层逻辑，再进入我们的 BSc 核心编译器**”的要求，Web 前端被设计为一个渐进式、沉浸式的双阶段应用：

```mermaid
flowchart LR
    Entry["Web 应用入口 (URL Root)"] --> Stage1["阶段一：编译原理互动通识实验室\n(Compiler Theory Interactive Lab)"]
    Stage1 -->|"点击『启动 BSc 引擎』或完成关卡"| Stage2["阶段二：BSc 编译器工作台\n(BSc Compiler Explorer Studio)"]
    Stage2 -.->|"顶部随时切换『原理导览』"| Stage1
```

### 1.1 阶段一：编译原理沉浸式互动教学实验室 (The Interactive Theory Lab)
这一部分旨在用最直观、动画化、可交互的方式，向用户（包括初学者和极客）解构**计算机如何把一段字符一步步变成目标机器码/高阶表达**：

```
+---------------------------------------------------------------------------------------------------------+
|  [BSc Lab]  模块一：经典编译原理通识导览                 [当前阶段：语法分析 AST]   [跳过，直接进入 BSc 编译器 ->] |
+---------------------------------------------------------------------------------------------------------+
|                                                                                                         |
|    [1. 词法分析] ──────> [2. 语法分析] ──────> [3. 语义分析] ──────> [4. 中间表示] ──────> [5. 优化 Pass]  |
|    (Lexing)              (Parsing)            (Semantic)           (IR Emit)           (Optimizer)      |
|                                                                                                         |
+---------------------------------------------------------------------------------------------------------+
| 【互动演示区 (Interactive Playground)】                                                                  |
|                                                                                                         |
|  输入测试语句：[ let total = count + 42; ] 或 [ 我用蓝牙连上开发板 ]                                         |
|                                                                                                         |
|  +-------------------------------------+   +---------------------------------------------------------+  |
|  | 词法切分状态 (Token Stream):         |   | 语法树实时推导 (AST Live Visualizer):                    |  |
|  |                                     |   |                                                         |  |
|  | [Token: LET      | Span: 0..3]      |   |              AssignStatement                            |  |
|  | [Token: IDENT    | val: "total"]    |   |                /          \                             |  |
|  | [Token: EQUAL    | Span: 10..11]    |   |        Identifier(total)   BinaryOp(+)                  |  |
|  | [Token: IDENT    | val: "count"]    |   |                             /        \                  |  |
|  | [Token: PLUS     | Span: 18..19]    |   |                     Identifier(count)  Literal(42)      |  |
|  | [Token: INT      | val: 42]         |   |                                                         |  |
|  +-------------------------------------+   +---------------------------------------------------------+  |
|                                                                                                         |
|  【原理解析面板】：                                                                                      |
|  * 什么是递归下降？什么是 Pratt Parsing 优先级算法？                                                        |
|  * 为什么编译器需要中间表示 (IR)？为什么不直接从 AST 生成机器汇编？                                           |
|  * 动画演示：看优化器如何将常数折叠 (Constant Folding)，消除死代码 (Dead Code Elimination)。                |
+---------------------------------------------------------------------------------------------------------+
```

* **核心特性**：
  1. **流水线进度条**：点击上方流水线的 5 个核心阶段，动态切换对应的原理详解与交互动画；
  2. **动效可视化**：采用 D3.js / SVG 动态渲染 AST 树的自顶向下构建过程、SSA 的支配树图（Dominator Tree）；
  3. **启发式对比**：并列对比“传统 C 代码编译（LLVM 流水线）”与“人类白话到大厂黑话编译（BSc 流水线）”的奇妙对仗关系。

---

### 1.2 阶段二：BSc 语义升维与去魅编译器控制台 (BSc Studio)
当用户理解了底层逻辑后，一键切换至正式的工业级 **Compiler Explorer** 界面，开展人话与黑话的双向编译与调试。

* **功能完备性**：
  * **模式一：正向升维编译**（大白话 $\to$ 语义 AST $\to$ Intent-IR $\to$ Pass 逐步优化 $\to$ 目标黑话）；
  * **模式二：反向去魅反编译**（大厂黑话/离谱 JD $\to$ 解构 Truth-IR $\to$ 揭露大白话残酷真相）；
  * **Pass 级 Diff 步进器**：支持一键回放每个 Pass 对 IR 指令的篡改过程。

---

## 2. 全栈技术选型与架构分工矩阵 (Full Tech Stack Matrix)

确定核心采用 **Rust** 结合现代前端，具体细分技术栈选型如下：

| 系统层级 | 选型组件 | 选型原因与技术优势 |
| :--- | :--- | :--- |
| **编译器核心 (Core)** | **Rust 2021 Edition** | 极致的内存安全、零成本抽象、模式匹配（极为适合 AST / IR 遍历与重写）、强大的类型系统与编译期检查。 |
| **词法分析器 (Lexer)** | **手写词法器 + 零拷贝切片 (`&str`)** | 不引入笨重庞大的外部宏，纯手写高效有限状态机（DFA），保留完整的源码位置信息 (`Span`) 与换行偏移。 |
| **语法分析器 (Parser)** | **递归下降 (Recursive Descent) + Pratt Parsing** | 编译界最主流且最优雅的解析方案（Rustc、Clang 均采用手写递归下降），支持友好的错误恢复与多语法分支回溯。 |
| **中间表示与 Passes** | **自研 SSA-like Intent-IR + PassManager** | 显式 BasicBlock、SSA 虚拟寄存器赋值、显式类型系统；PassManager 支持依赖校验与中间结果诊断 Dump。 |
| **WASM 跨平台绑定** | **`wasm-bindgen` + `serde-wasm-bindgen`** | 将 Rust 编译的底层 AST、IR 与输出无损导出为高效的 JavaScript TypedArray / JSON 对象，供前端 60 帧无卡顿调用。 |
| **Web 前端框架** | **Vue 3 + TypeScript + Vite + Pinia** | 响应式状态管理（Pinia）、组合式 API（Composition API）非常适合多栏联动数据流，打包体积轻，开发体验现代。 |
| **UI 样式与排版** | **TailwindCSS + Lucide Icons** | 快速构建高质感、现代深色/浅色极客风主题（类似 Vercel / GitHub Dark 风格）。 |
| **代码与 IR 编辑器** | **Monaco Editor (VS Code 核心)** | 支持语法高亮、只读锁、双编辑器 Diff 对比视图（用于展示 Pass 前后变化）、源码光标位置事件监听。 |
| **图结构可视化** | **D3.js / WebGL / SVG** | 用于在“编译原理实验室”和“AST 视图”中平滑缩放、拖拽展示语法树节点与控制流图（CFG）。 |
| **本地命令行 (CLI)** | **`clap` (v4 with derive) + `axum`** | CLI 工具 `bsc` 内置单文件嵌入式 Web 服务器，执行 `bsc serve` 时秒级启动并自动在浏览器唤醒控制台。 |

---

## 3. 核心重难点深度剖析与攻坚方案 (Deep Dive: Key Technical Challenges)

构建一个“基于严谨编译原理的大型语义编译器”，工程复杂度远超普通的文本转换，必须直面并解决以下 **5 大核心技术重难点**：

---

### 3.1 难点一：自然语言模糊歧义性 vs 编译器确定性语法 (NLP-to-Compiler Ambiguity)

#### 痛点本质：
传统编程语言（如 C、Rust）的语法是**确定性上下文无关文法（LL/LR 可解析）**，每个关键字、括号分号都有严格界定；  
而人类的大白话充满了**省略、倒装、同义词丰富、无显式分隔符**的特征（例如：“*昨天照片传板子了*” 省略了介词“用”、主语“我”、连词“给”）。

#### 攻坚解决方案：
我们不采用概率黑盒模型，而是构建一套**“容错式语义槽位下降分析法（Fault-Tolerant Semantic Slot Parser）”**：
1. **轻量规则词法规范化（Lexer Canonicalization）**：
   * 词法分析器输出的不仅仅是静态字符串，而是经过**同义词族归一化（Synonym Family Normalization）**的抽象 Token：
     * `["传", "发", "投递", "同步", "推送"]` $\to$ `TokenKind::Verb(ActionFamily::Dispatch)`
     * `["蓝牙", "BLE", "蓝牙4.0"]` $\to$ `TokenKind::Medium(MediumFamily::Bluetooth)`
2. **槽位驱动的贪心递归下降（Slot-Driven Greedy Descent）**：
   * 语法分析器定义一套**松散但保序的语法推导树**：
     $$Statement \to [Time] \times [Subject] \times [Medium] \times Action \times [Payload] \times [Target] \times [Outcome]$$
   * 任何非核心槽位（如时间、主语、介词）允许缺省（Option），解析器按算符优先级贪婪捕获有效实体，未声明的节点由语义分析阶段（Sema）自动填充**隐式默认符号（Implicit Fallback Symbols）**。

---

### 3.2 难点二：语义中间表示 (Intent-IR) 的规范度与正交抽象性 (IR Orthogonality & SSA Design)

#### 痛点本质：
如果 IR 设计得太贴近中文，它就会退化成“中文拼音语法”；如果设计得太贴近底层汇编，它又无法承载“高维架构叙事”。IR 必须是**语言无关、目标无关、正交且具备强数学美感**的。

#### 攻坚解决方案：
引入经典的 **SSA（Static Single Assignment，静态单赋值）** 规范，设计三地址语义指令：

```rust
// 核心 IR 实体与操作抽象定义 (Rust 数据结构模型)
pub enum IROpcode {
    // 拓扑与通道类操作
    EstablishSession { session_id: ValueId, medium: ValueId, target: ValueId },
    // 资产与载荷流动操作
    TransferPayload { result_id: ValueId, payload: ValueId, dest: ValueId, via_session: ValueId },
    // 状态断言与度量操作
    AssertIntegrity { report_id: ValueId, target: ValueId, expected_state: StateId },
    // 价值闭环合成指令 (中端 Pass 注入)
    SynthesizeTeleology { loop_id: ValueId, strategic_domain: DomainId, cadence: CadenceMode },
}

pub struct BasicBlock {
    pub label: String,
    pub instructions: Vec<IROpcode>,
    pub terminator: TerminatorInstruction,
}
```

* **正交性保证**：
  * **操作码（Opcode）**负责表达“动词与动作关系”（如连接、转移、断言）；
  * **值引用（ValueId）**负责表达“被操作的名词实体”（如通道、端点、资产）；
  * 任何操作只能通过输入 ValueId 产生唯一的输出 ValueId，支持构建精密的**数据流图（Dataflow Graph）**与**使用-定义链（Use-Def Chains）**。

---

### 3.3 难点三：中端 Pass 流水线的依赖管理与中间状态回滚 (Pass Dependency & Invalidation)

#### 痛点本质：
黑话编译需要跑 4~6 个不同的优化 Pass（去人性化 $\to$ 概念升维 $\to$ 拓扑网状化 $\to$ 价值闭环注入 $\to$ 四字格律动）。  
如果 Pass 之间顺序错乱（比如在概念升维前先进行了闭环注入），或者前一个 Pass 破坏了 CFG 结构导致后续 Pass 崩溃，整个编译器就会报错。

#### 攻坚解决方案：
参考 LLVM 的新版 `PassManager` 体系，构建显式的 **Pass 依赖图与分析守护契约**：

```rust
pub trait TransformPass {
    fn name(&self) -> &'static str;
    // 声明当前 Pass 所依赖的前置分析结果
    fn required_analyses(&self) -> Vec<AnalysisKey>;
    // 执行 IR 重写变换，返回是否改变了 IR (is_modified)
    fn run(&mut self, module: &mut IRModule, am: &AnalysisManager) -> PassResult<bool>;
}
```

* **Pass 依赖自动拓扑排序**：
  * `PassManager` 在启动前，对所有已注册的 Passes 进行**拓扑排序（Topological Sort）**，若检测到循环依赖直接在编译期报错；
* **IR 状态快照与单步回放（Snapshot & Replay）**：
  * 每一个 Pass 执行完毕后，PassManager 自动为当前的 `IRModule` 生成轻量级快照（Snapshot Hash）；
  * 这不仅为后续的 Web 界面提供了**分步 Diff 对比**的数据源，还能在某个 Pass 执行异常时安全回滚。

---

### 3.4 难点四：Rust 编译至 WebAssembly 的极致体积压缩与轻量化 (WASM Footprint)

#### 痛点本质：
Rust 编译出 `.wasm` 产物如果不做针对性优化，动辄 10MB~20MB，这在移动端（iPad Safari）加载会带来明显的白屏等待；同时不能引入任何 C/C++ 动态链接库或系统调用（如直接读写本地磁盘）。

#### 攻坚解决方案：
1. **无操作系统依赖设计（Zero Syscall Core）**：
   * 编译器核心（Core, Frontend, Opt, Backend）完全运行在内存中，输入为 `&str`，输出为 AST/IR/目标字符串，不调用任何本地文件 IO，确保完全兼容 `wasm32-unknown-unknown` 目标平台；
2. **构建脚本体积深度压缩策略**：
   ```toml
   [profile.release]
   opt-level = "z"     # 针对二进制体积进行极致优化
   lto = true          # 开启跨 Crate 链接时优化 (Link-Time Optimization)
   codegen-units = 1   # 降低并行度以获取最大优化收益
   panic = "abort"     # 发生 Panic 时直接中止，剥离昂贵的回溯堆栈代码
   strip = true        # 彻底剥离所有符号表与调试元数据
   ```
3. **搭配 `wasm-opt` 工具二次瘦身**：
   * 通过 Binaryen 的 `wasm-opt -Oz` 进一步死代码剔除与指令紧凑化，将最终编译出来的 WASM 文件控制在 **800KB 以内**，确保在 iPad 浏览器上毫秒级秒开运行。

---

### 3.5 难点五：三栏联动高亮 (Cross-Highlighting) 与源码映射追踪 (Source Spans Tracking)

#### 痛点本质：
用户在 Web 上鼠标划选左栏的“蓝牙”，中栏的 `%channel = Resource(...)` 以及右栏的“全场景近场分布式软总线”必须**毫秒级无延迟同步高亮**。  
由于中端优化 Pass 对 IR 进行了大量的节点分裂、重写与合成，原始源码的 `Span` 在经过多级重构后极容易丢失或映射错位。

#### 攻坚解决方案：
借鉴工业级调试符号 **DWARF / SourceMap** 思想，设计**语义血统追踪系统（Semantic Lineage Tracker）**：

```rust
pub struct LineageMetadata {
    pub origin_source_span: Span,      // 原始人话字符区间 [start, end]
    pub ir_value_id: ValueId,          // 对应的 IR 虚拟寄存器 ID
    pub target_emitted_spans: Vec<Span>, // 最终输出在目标黑话中的对应字符区间
}
```

* **溯源代币（Lineage Tokens）穿透传递**：
  * 当前端 Parser 生成 AST 节点时，贴上不可变的主键 ID（`NodeId`）；
  * 当中端 Pass 将原始节点升维重写时，**新生成的 IR 指令必须保留原始父级节点的 `NodeId`**；
  * 当后端代码发射器将指令渲染为文字时，将文字的起始偏移量与 `NodeId` 登记在全局映射表 `SourceMapRegistry` 中；
  * Web 前端接收到这套完整的 SourceMap，直接通过 Monaco Editor 的 `deltaDecorations` API 实现**零延迟光标同步双向高亮**！

---

## 4. 工程跨平台交付形态规划

整个项目完成后，将具备三大完整交付形态：

1. **终端开发者模式 (Native CLI)**：
   * 在 Linux (Steam Deck)、macOS (Mac Studio)、Windows 终端中：
     ```bash
     # 编译单句话
     bsc compile --target=huawei "昨天用蓝牙连上板子"
     
     # 反编译黑话
     bsc decompile "依托全场景近场分布式软总线..."
     
     # 启动本地可视化 Compiler Explorer
     bsc serve --port 8080
     ```
2. **离线独立 Web 应用 (Pure WASM Client)**：
   * 静态打包发布在 GitHub Pages 或部署在任何轻量 HTTP 服务上；
   * iPad 用户无需安装任何后端服务，直接通过 Safari 访问，所有的词法分析、语法树推导、Pass 优化全部在 **iPad 本地浏览器沙盒内瞬时计算完成**。

---

## 5. 下一步具体执行计划

按照用户指示，后续工程推进严格遵循：
* **当前状态**：技术栈与重难点分析（`DOC-003`）已完成，整体方案与理论基础已彻底筑牢。
* **下一步任务（004 方案或工程落地）**：
  * 开始构建项目根目录下的 **Cargo Workspace 骨架**与 `crates/` 核心模块；
  * 优先实现 `bsc-core` 中的 AST 抽象语法树与 `Intent-IR` 结构体代码。
