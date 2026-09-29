# 007: 工业级编译器核心破局——Rust WASM原生全栈运行时、依存句法分析与交互式编译实验室

> **工程全称**：`Bullshit Compiler System`（工程目录：`bullshit-compiler`，CLI：`bsc`）  
> **文档编号**：`DOC-007`  
> **创建时间**：2026-09-28  
> **状态**：深度自省缺陷全量攻关与工程落地方案（正式实施稿）  
> **核心破局诉求**：
> 彻底解决在上一轮自查中发现的五大“塑料感/上不了台面”的致命硬伤：
> 1. **消灭纯 JS 仿真“两张皮”**：利用 `wasm32-unknown-unknown` 将 Rust 真实编译器代码全量编译为 WebAssembly 挂载至 Web 工作台，保证前端执行的每一行都是真正的 Rust 编译算法；
> 2. **消灭弱智正则匹配**：引入中文依存句法分析（Dependency Grammar）与复合从句抽取，精准解析包含主从复合句、量词修饰语、倒装与多动词连接的复杂真实自然语言；
> 3. **消灭字符串摆拍 IR**：构建真正的内存对象拓扑图（Memory SSA Graph），实现真正的 Use-Def 链遍历、常量折叠与 Pass 级图重写；
> 4. **消灭僵死填空模板**：构建多维修辞语法框架（Rhetoric Grammatical Frames）与分层语义本体图（Hierarchical Ontology Graph），支持动态句法重构与语用弹性；
> 5. **消灭静态教材文字搬运**：在通识知识库中全面引入 Canvas/SVG **可交互、可单步调试的活体仿真器**（DFA 自动机跳转机、递归下降分析栈动态演练、SSA 支配树与 CFG 可视化、图着色寄存器分配仿真台）。

---

## 目录
1. [五大自查致命缺陷与根治方案映射表](#1-五大自查致命缺陷与根治方案映射表)
2. [Rust WASM 原生全栈编译与前端桥接架构](#2-rust-wasm-原生全栈编译与前端桥接架构)
3. [自然语言依存句法分析与复合从句抽取引擎 (Dependency Parser)](#3-自然语言依存句法分析与复合从句抽取引擎-dependency-parser)
4. [真实内存 SSA 图数据结构与 PassManager 重写算法](#4-真实内存-ssa-图数据结构与-passmanager-重写算法)
5. [分层语义本体知识库与修辞句法重构引擎 (Rhetoric Engine)](#5-分层语义本体知识库与修辞句法重构引擎-rhetoric-engine)
6. [通识实验室可交互动态仿真器集群设计](#6-通识实验室可交互动态仿真器集群设计)
   - 6.1 交互式 DFA 状态机步进转移机 (SVG Live State Runner)
   - 6.2 递归下降调用栈与 AST 树动态生长器 (Parser Call Stack Visualizer)
   - 6.3 SSA 控制流图 (CFG) 与支配边界探测器 (Dominance Graph Explorer)
   - 6.4 Chaitin-Briggs 寄存器冲突图 $K$-着色演算台 (Graph Coloring Simulator)

---

## 1. 五大自查致命缺陷与根治方案映射表

| 序号 | 暴露的致命缺点 | 根因与破绽 | 007 核心根治与重构方案 |
| :--- | :--- | :--- | :--- |
| **FLAW-1** | **前后端严重割裂两张皮** | 浏览器跑的是单独手写的 JS 模拟器，Rust Crates 成了无人调用的空架子。 | **全栈 WASM 原生挂载**：编译 `bsc-wasm` 为 `.wasm`，Web 端直接加载该二进制模块，调用 Rust 原生导出的 `compile_pipeline()`。 |
| **FLAW-2** | **前端是伪正则匹配** | 无法解析包含修饰从句（如“我昨天吃了我妈买的排骨，但是有点咸”）、多量词、多动作复合句。 | **轻量级依存句法分词器**：在 Rust 核心实现基于词性（POS）与依存弧（Dependency Arc）的分析器，识别主干谓词与修饰成分。 |
| **FLAW-3** | **IR 与 Pass 是字符串摆拍** | 界面上的 IR 是由模板拼接的，无内存图结构，Pass 演进无真正计算。 | **强类型 SSA 内存图对象**：真实构建 `Instruction`, `BasicBlock`, `Value`, `UseDefChain`，Pass 真正进行节点迭代并由 `IRPrinter` 真实 Dump。 |
| **FLAW-4** | **黑话模板僵死单调** | 遇到生僻词直接拼接，方言句式永远是固定的一句话填空，套路感极重。 | **修辞句法框架库 (Grammar Frames)**：支持倒装排比、穿透叙事等多套骨架；配合多层本体图谱，支持任意未登录词的广义语义升维。 |
| **FLAW-5** | **通识模块成静态教材** | 龙书理论全是文字大卡片，缺乏直观心理模型与动手调试趣味。 | **四维互动仿真试验台**：开发 DFA 状态跳转、递归下降进栈出栈、SSA 控制流图、图着色冲突分配的可交互动画实验。 |

---

## 2. Rust WASM 原生全栈编译与前端桥接架构

通过 `wasm32-unknown-unknown` 工具链，在 `crates/bsc-wasm` 中暴露统一接口：

```rust
// crates/bsc-wasm/src/lib.rs
use wasm_bindgen::prelude::*;
use bsc_frontend::UniversalParser;
use bsc_opt::PassManager;
use bsc_backend::DialectTarget;

#[wasm_bindgen]
pub struct WasmCompilationResult {
    tokens_json: String,
    ast_json: String,
    raw_ir: String,
    pass1_ir: String,
    pass2_ir: String,
    pass3_ir: String,
    emitted_code: String,
    metrics_json: String,
}

#[wasm_bindgen]
pub fn bsc_compile(input: &str, target: &str) -> Result<WasmCompilationResult, JsValue> {
    // 1. Rust 原生通用依存词法与语法分析
    let mut parser = UniversalParser::new(input);
    let ast = parser.parse()?;

    // 2. Rust 原生 Lowering 到 SSA 内存模型
    let mut module = bsc_core::lower_ast_to_ir(&ast);
    let raw_ir = module.to_ssa_string();

    // 3. Rust 原生 PassManager 真实图重写流水线
    let mut pm = PassManager::new();
    let pass1_ir = pm.run_pass_step(&mut module, 0); // StripAgencyPass
    let pass2_ir = pm.run_pass_step(&mut module, 1); // ScaleAmplifyPass
    let pass3_ir = pm.run_pass_step(&mut module, 2); // TeleologyInjectPass

    // 4. 多方言修辞框架代码生成
    let dialect = target.parse::<DialectTarget>()?;
    let emitted = bsc_backend::emit_dialect(&module, dialect);

    Ok(WasmCompilationResult { ... })
}
```

前端加载逻辑：
1. 异步加载 `bsc_wasm_bg.wasm`；
2. 页面就绪后，状态灯切换为 **“Rust 原生 WASM 内核在线 (Core: Native)”**；
3. 用户输入触发时，直接进入 WebAssembly 线性内存执行，获得微秒级高性能与绝对纯正的编译器语义。

---

## 3. 自然语言依存句法分析与复合从句抽取引擎 (Dependency Parser)

为了攻克“我昨天吃了我妈给我买的排骨，后来喝了杯水”等复杂长难句，前端分析器摒弃单层 Flat 正则，升级为**两阶段依存树解析模型（Two-Stage Dependency Tree Model）**：

```
       [ 根谓词 ROOT: 吃 ] ──(辅谓词 conj)──> [ 谓词: 喝 ]
         │          │                            │
   (状语 advmod) (宾语 obj)                  (宾语 obj)
         │          │                            │
     [ 昨天 ]    [ 排骨 ]                     [ 水 ]
                    │                            │
              (定语从句 acl)               (数量词 nummod)
                    │                            │
               [ 妈买的 ]                     [ 一杯 ]
```

* **阶段 1：词法标注与词性分词 (POS Tagging)**：
  切分出时间词（$T$）、人称代词（$A$）、动作主谓词（$V$）、定语引导词（$D$）、名词实体（$N$）、量词（$Q$）、结果补语（$C$）。
* **阶段 2：修饰弧绑定与复合子句划分**：
  * 若名词前存在带“的”修饰串，自动将其归入该实体的内部属性修饰子树（Attributive Clause）；
  * 若存在连词（“后来”、“又”、“并且”）或逗号并列动词，自动构建并列复合语句节点（Compound Action Nodes）。

---

## 4. 真实内存 SSA 图数据结构与 PassManager 重写算法

消灭 JS 字符串模板，在 `crates/bsc-core` 中落地完整的对象图模型：

```rust
// 真实的内存指令与操作数
pub struct Instruction {
    pub result_reg: VirtualRegister, // %0, %1, %2
    pub opcode: IROpcode,
    pub operands: Vec<Operand>,
    pub annotations: HashMap<String, String>,
}

pub struct BasicBlock {
    pub label: String,
    pub instructions: Vec<Instruction>,
    pub terminator: TerminatorInst,
}

pub struct IntentModule {
    pub blocks: Vec<BasicBlock>,
    pub symbol_table: SymbolTable,
}
```

* **Pass 真实的图变换**：
  * `StripAgencyPass`：扫描所有操作数的 `operands`，凡类型为 `Operand::Actor(Human)` 者，将其指针重定向为新分配的 `Operand::Actor(AutonomousKernel)`，并在基本块首部插入守护进程初始化指令；
  * `ScaleAmplifyPass`：查询本体知识图，将每个 `Resource` 节点的内部类型从低阶微观属性替换为高维生态底座属性；
  * `TeleologyInjectPass`：在 `TerminatorInst::Return` 之前动态插入 `Opcode::SynthesizeValueLoop` 节点，分配新寄存器 `%reg_final`。
* **IRPrinter**：
  实现统一的 Trait `Display`，实时根据内存中真实的图结构递归打印标准 LLVM 规范的 SSA 文本。用户在前端切换 Pass 看到的 Diff，是内存结构真正发生变化的物理投射！

---

## 5. 分层语义本体知识库与修辞句法重构引擎 (Rhetoric Engine)

针对“模板僵死套路化”问题，开发后端**修辞句法重构系统**：

1. **分层本体树（Hierarchical Ontology Tree）**：
   * 建立多级分类谱系：`顶级根本体` $\to$ `领域（饮食/计算/商务/消费/运动）` $\to$ `子类` $\to$ `概念叶子`；
   * 未登录词通过语义聚类与部首/词素启发式算法自动归入最近邻分支（例如：“吃烧烤” $\to$ “有机美拉德高温聚合高能蛋白”，绝不生硬套用工业大词）。
2. **多套句法骨架（Syntax Frames）矩阵**：
   后端不再使用唯一一段话填空，而是配备 4 种句法修辞模式，根据输入句式复杂度自动匹配：
   * **框架 A（主客观转置与能力沉淀式）**：以客体资产为叙事核心，强调主体能力的自律闭环；
   * **框架 B（排比因果递进式）**：多动作复合句专用，以“一抓... 二促... 三打通...”铺陈；
   * **框架 C（痛点反思与战略突破式）**：含有故障、耗时、异常等语境时，突出自愈与韧性护城河；
   * **框架 D（高维赋能与全域穿透式）**：标准商务与协同语境。

---

## 6. 通识实验室可交互动态仿真器集群设计

彻底告别静态排版文字，在通识 Tab 中直接内嵌 4 大核心编译算法的 **Canvas/SVG 交互式动态试验台**：

1. **DFA 状态机可视化仿真器**：
   用户可在输入框手敲任意测试字符串（如 `0101` 或标识符），点击“单步执行”或“自动播放”，观察圆圈状态（$S_0 \to S_1 \to S_2$）之间的高亮跳跃动画，边缘连接线实时发光，展示接受/拒绝终态。
2. **递归下降调用栈动态生长器**：
   输入任意算术表达式（如 `3 + 4 * 2`），动态以甘特图/堆栈帧展示 `parse_expr()` $\to$ `parse_term()` $\to$ `parse_factor()` 的进栈出栈过程，并在右侧动态生长出 SVG 语法树节点。
3. **SSA 支配树与 CFG 控制流图探索器**：
   展示带分支（`if-else`）和循环（`loop`）的控制流拓扑图，鼠标悬停时动态高亮该节点的支配边界（Dominance Frontier），直观呈现为什么此处必须插入 $\Phi$ 节点。
4. **Chaitin-Briggs 寄存器冲突图着色试验台**：
   提供预设变量活跃区间干涉图，用户选择物理寄存器数量 $K$（如 2 或 3），单步点击观测度数判断、推入简化栈（Simplify）、评估溢出（Spill）及最终节点自动着色的全过程。

---

通过上述五维核心攻关，彻底打破“外表宏大、内里塑料”的困境，使 Bullshit Compiler 成为真正兼具硬核学术深度、工业级运行内核与极致可玩性的标杆工程。
