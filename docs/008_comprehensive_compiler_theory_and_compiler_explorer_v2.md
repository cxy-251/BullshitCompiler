# 008. 龙书级编译原理权威通识与真动态 WebAssembly 编译器重构

> **归档编号**：008  
> **关联前序**：[007_industrial_grade_compiler_overhaul_and_wasm_architecture.md](007_industrial_grade_compiler_overhaul_and_wasm_architecture.md)  
> **归档日期**：2026-09-28  
> **核心主题**：响应用户对“运行编译功能无用”与“编译原理一句话带过”的尖锐批评，全量重构编译原理六大阶段知识体系，接入 Rust 原生 WebAssembly 真实执行链路与 stdout 诊断控制台。

---

## 1. 用户反馈与核心问题复盘

### 1.1 问题一：“运行编译没有用”的成因分析
1. **WASM 内核断联**：前次构建中编译生成的 `web/bsc_wasm.wasm` 虽然存在，但 Web 端 JavaScript 并未真正通过 C-ABI（`bsc_alloc`、`bsc_compile_wasm`）进行内存分配与数据交换，编译操作完全脱离底层 Rust 编译器。
2. **自然语言词法/语法规则硬编码截断**：原规则未能涵盖常见的餐食（如“排骨”、“红烧肉”）、算法代码（如“冒泡排序”）及多样化的时间词（如“今天中午”），导致解析时将“中午”误当做标的物，生成如“围绕【高维【中午】载荷】”等逻辑崩塌的荒谬产物。
3. **缺乏执行过程可观测性**：点击「运行编译」后，仅有 100ms 的按钮文字变化，用户无法直观感知编译流水线（词法扫描、AST 构建、SSA 生成、Pass 优化、代码发射）的具体工作细节。

### 1.2 问题二：“编译原理大部头一句话带过”的成因分析
1. 编译原理（龙书）是涵盖形式文法、自动机理论、属性文法、支配树、半格数据流及图着色等深奥学科的经典巨著。
2. 前次实现中，每个阶段仅配置了一句简短的 summary，浮于表面，严重脱离了用户对“系统、完整、扎实介绍纯正编译原理”的根本诉求。

---

## 2. 编译原理六大阶段权威通识体系

针对编译原理通识模块进行了彻底重构，按编译流水线顺序建立大部头教学体系，全量剥离业务黑话，仅专注纯正编译原理学术与工业体系：

### 01. 词法分析 (Lexical Analysis)
- **形式文法**：乔姆斯基 3 型正规文法，正则表达式形式定义。
- **核心算法**：
  1. **Thompson 构造法**：将正则表达式归纳构建为带 $\epsilon$-跳转的非确定有限自动机 ($\epsilon$-NFA)。
  2. **子集构造法 (Subset Construction)**：求解 $\epsilon\text{-closure}(s)$ 与 $\text{move}(T, a)$，消除状态不确定性，产出 DFA。
  3. **Hopcroft 状态最小化算法**：基于状态等价类分割，在 $O(k \cdot n \log n)$ 复杂度下收敛至唯一最简 DFA。
- **工业级工程实现**：双缓冲区双指针扫描（`lexemeBegin` 与 `forward`）、最长匹配（Maximal Munch）、SourceSpan 物理行列源码坐标维护。

### 02. 语法分析 (Syntax Analysis)
- **形式文法**：乔姆斯基 2 型上下文无关文法（CFG），产生式系统 $G = (V_N, V_T, P, S)$，推导、句型、句柄与二义性消除。
- **自顶向下派系**：递归下降分析（Recursive Descent）、LL(1) 文法的 FIRST 集与 FOLLOW 集数学求解、左递归消除（直接与间接）与提取左公因子、预测分析表构建。
- **自底向上派系**：移进-归约（Shift-Reduce）核心机制，LR 族谱演进（LR(0) 项集规范族 $\to$ SLR(1) 用 FOLLOW 集消冲突 $\to$ LR(1) 携带向前看搜索符 $\to$ LALR(1) 合并同心项集），Action/Goto 表结构与冲突解决。
- **现代工业标准**：手写递归下降分析器结合 Pratt Parser（运算符优先级爬升算法），提供精细化语法报错与 Panic Mode 恢复。

### 03. 语义分析与类型系统 (Semantic Analysis & Type Systems)
- **上下文相关性**：乔姆斯基 1 型文法约束（变量声明先于使用、作用域可见性、类型兼容性）。
- **属性文法与语法制导翻译 (SDD / SDT)**：综合属性（自底向上 S-属性文法）与继承属性（自顶向下 L-属性文法），属性依赖图与拓扑排序求值。
- **嵌套符号表 (Scope Tree)**：层级树状作用域链，名字决议向外冒泡机制，变量遮蔽（Shadowing）与生命周期绑定。
- **类型推导与类型合一**：Robinson Unification 合一算法，Hindley-Milner 强类型推导系统。

### 04. 中间表示与 SSA 形式 (Intermediate Representation & SSA Form)
- **现代编译器中端核心**：静态单赋值（Static Single Assignment, SSA）形式。核心定律：每个虚拟变量仅被定义一次。消除别名干扰，暴露纯粹的显式数据流有向无环图（DAG）。
- **控制流图 (CFG)**：基本块（Basic Block）划分原则（Leader 定理）、前驱/后继控制边、Terminator 终结指令。
- **支配理论 (Dominance Theory)**：支配节点定义、立即支配者（IDOM）、Lengauer-Tarjan 快速支配树算法、支配边界（Dominance Frontier, DF）严谨数学定义。
- **$\Phi$ 节点插入与 De-SSA**：在变量定值块的支配边界处精准放置 $\Phi$ 节点；在后端生成前通过关键边分割（Critical Edge Splitting）将 $\Phi$ 还原为物理指令。

### 05. 机器无关代码优化 (Optimization Passes)
- **数据流分析框架**：半格理论 $\langle L, \sqcap, \sqsubseteq, \top, \bot \rangle$、单调有界传递函数 $f_B$、Kildall 不动点迭代算法。
- **四大经典数据流方程**：
  1. 到达定值 (Reaching Definitions)：Forward + 并集 ($\cup$)
  2. 可用表达式 (Available Expressions)：Forward + 交集 ($\cap$)
  3. 活跃变量 (Live Variables)：Backward + 并集 ($\cup$)
  4. 常量传播 (Constant Propagation)：半格前向迭代
- **现代工业 Pass 体系**：死代码消除 (DCE / ADCE)、稀疏有条件常量传播 (SCCP)、全局值编号 (GVN)、循环不变量外提 (LICM)、函数内联与逃逸分析。

### 06. 代码生成与后端体系 (Code Generation & Target Architecture)
- **指令选择 (ISel)**：树模式匹配与重写系统、Maximal Munch 贪心算法、动态规划最优代价覆盖 (BURS 算法)。
- **指令调度 (Instruction Scheduling)**：流水线数据冒险 (RAW / WAR / WAW)、DAG 关键路径列表调度算法。
- **寄存器分配 (Register Allocation)**：无限虚拟寄存器映射至有限物理寄存器（NP 完全问题）。Chaitin-Briggs 图着色四大阶段（Simplify $\to$ Spill $\to$ Select $\to$ Coalesce）与 JIT 线性扫描。
- **平台 ABI 与调用约定**：System V AMD64 ABI 与 ARM64 AAPCS 传参规则、栈帧布局（Prologue/Epilogue、Frame Pointer、Red Zone）。

---

## 3. 编译器工作台真动态执行重构

### 3.1 Rust WebAssembly 原生后端打通
- 更新了 `crates/bsc-frontend/src/lexer.rs` 与 `parser.rs`，支持日常餐食（排骨、红烧肉）、算法技术（冒泡排序、死循环）、时间短语（今天中午、昨天下午）等全量词汇，并具备通用 fallback 解析能力。
- 升级 `crates/bsc-backend/src/targets.rs`，动态绑定生成的升维概念，输出具备实质内容的黑话。
- 编译生成原生 release 目标 `web/bsc_wasm.wasm`（104KB），支持内存动态申请与 UTF-8 JSON 零依赖数据交互。

### 3.2 编译器工作台交互优化
1. **编译器诊断终端控制台 (stdout)**：在输出卡片下方新增黑色控制台终端，实时滚动输出编译器底层日志（词法切分耗时、AST 节点构建、SSA 寄存器分配、Pass 1/2/3 演进记录、代码发射大小）。
2. **Pass 逐级演化展示**：点击 `Raw IR`、`Pass 1`、`Pass 2`、`Pass 3` 选项卡时，中间表示与右侧生成代码同步展示该 Pass 对应的状态与文字。
3. **高亮边框渐变闪烁**：点击「运行编译」时，触发明确的 indigo 发光脉冲动画，杜绝界面无响应感知。
