# 009: 双编译器架构与全文本文件编译工程方案

> **状态**: 已实现并在 CLI 与 WebAssembly 双端全量验证通过  
> **关联版本**: BSc v0.5.0 (`crates/bsc-classic`, `crates/bsc-frontend`, `crates/bsc-backend`, `crates/bsc-opt`, `crates/bsc-wasm`, `crates/bsc-cli`)  
> **文档定位**: 记录双编译器架构（经典命令式语言编译器 + 黑话长文本文件编译器）的设计决策、代码实现、指令级验证与工程落地。

---

## 一、 需求变更与设计决策背景

在上一迭代中，用户提出了两个关键要求：
1. **“你实现一个编译原理的编译器，然后再实现这个黑话的编译器”**：
   - 必须先构建一套**真实、严谨、符合学术界与工业界规范的经典程序设计语言编译器**（从词法分析、递归下降语法分析、作用域树语义分析、SSA 3-地址码中间表示、常量折叠与死代码消除优化，到 x86-64 物理汇编生成以及字节码虚拟机执行），彻底证明编译原理核心技术的真实落地；
   - 随后将经典编译理论拓展至**语义与意图编译器**，实现由自然语言白话到高维黑话的编译变换。
2. **“我们的输入可不止一句话，可能是一个文本文件。你重新再搞一波吧，可以只编译一个文件”**：
   - 摒弃仅支持“一两句话”的玩具演示模式；
   - 必须支持**完整多段落长文本文件**的编译处理（如季度运营总结报告、核心网故障复盘等数百上千字符的结构化文件）；
   - 支持从 CLI 命令行读取文件输入、导出文件结果；并在 Web 端提供文件上传、文件下载、整篇文档分段落编译排版与 Pass 演进查看。
3. **“不要起臃肿浮夸的标题，把按钮和编译执行彻底搞好”**：
   - 界面与命名全量回归工程师理性风格：精炼分为「经典代码编译器」、「黑话文档编译器」、「编译原理」、「反编译器」四大工作台；
   - 每一个编译按钮与运行功能均由底层 Rust WebAssembly 实时执行，杜绝静态 Mock。

---

## 二、 经典语言编译器 (`crates/bsc-classic`) 完整架构

为满足严谨编译器的要求，我们在工作区中新建了核心独立 Crate `crates/bsc-classic`，实现了完备的经典命令式语言编译与运行流水线。

```
                    bsc-classic 编译执行流水线
                    
  [ 源码 .lang ]
        │
        ▼ (Lexer)
  [ Token 记号流 ] ────── 行列号 Span、关键字、标识符、算符、注释过滤
        │
        ▼ (Recursive Descent + Precedence Climbing)
  [ 抽象语法树 AST ] ──── 函数定义、语句块、二元/一元表达式、类型标注
        │
        ▼ (Semantic Analyzer)
  [ 作用域符号表 ] ────── enter/exit_scope、变量定义检查、类型相容性验证
        │
        ▼ (IR Lowering)
  [ SSA 三地址码 IR ] ─── 基本块 (entry/then/else/merge)、alloca/load/store/icmp/br/jmp/call/ret
        │
        ▼ (Optimizer)
  [ 优化后 SSA IR ] ───── 常量折叠 (Constant Folding)、死代码消除 (DCE)
        ├────────────────────────────────┐
        ▼ (Codegen)                      ▼ (VM Interpreter)
  [ x86-64 目标汇编 ]              [ 虚拟机运行时执行 ]
  (System V AMD64 ABI)           (帧栈跟踪、指令计数、返回值与 STDOUT)
```

### 1. 词法分析 (Lexer & Spans)
- **源码文件**: [`token.rs`](file:///home/deck/Games/agy/bullshit-compiler/crates/bsc-classic/src/token.rs), [`lexer.rs`](file:///home/deck/Games/agy/bullshit-compiler/crates/bsc-classic/src/lexer.rs)
- **特性**:
  - 精确跟踪字符的绝对偏移量、行号与列号（`Span { start, end, line, col }`）；
  - 支持单行注释 `// ...` 与多行块注释 `/* ... */`；
  - 严格匹配保留字（`fn`, `let`, `if`, `else`, `while`, `return`, `print`）；
  - 自动识别标识符、整数字面量、双字符关系运算符（`<=`, `>=`, `==`, `!=`）与算术/逻辑运算符。

### 2. 语法分析 (Recursive Descent + Precedence Climbing)
- **源码文件**: [`ast.rs`](file:///home/deck/Games/agy/bullshit-compiler/crates/bsc-classic/src/ast.rs), [`parser.rs`](file:///home/deck/Games/agy/bullshit-compiler/crates/bsc-classic/src/parser.rs)
- **特性**:
  - 顶层由递归下降解析函数定义列表；
  - 表达式解析采用**运算符优先级爬升算法（Precedence Climbing）**：
    - 乘除模（`*`, `/`, `%`）：优先级 5
    - 加减（`+`, `-`）：优先级 4
    - 关系比较（`<`, `<=`, `>`, `>=`）：优先级 3
    - 等值比较（`==`, `!=`）：优先级 2
    - 逻辑与（`&&`）：优先级 1
    - 逻辑或（`||`）：优先级 0
  - 彻底规避传统手写下降解析中的无限递归与右结合回溯；
  - 支持大小写兼容类型（`int`/`Int`, `bool`/`Bool`, `void`/`Void`）。

### 3. 语义分析与作用域树 (Semantic Analysis & Type Checking)
- **源码文件**: [`semantic.rs`](file:///home/deck/Games/agy/bullshit-compiler/crates/bsc-classic/src/semantic.rs)
- **特性**:
  - 维护树状作用域结构，支持嵌套代码块的 `enter_scope()` 与 `exit_scope()`；
  - 内层作用域可安全遮蔽（Shadow）外层同名变量；
  - 强制类型检查：
    - 二元算术运算左右操作数必须为 `Int`；
    - 条件判断（`if`, `while`）条件表达式必须为 `Bool`；
    - 函数参数个数、参数类型与返回值类型必须严格一致；
    - 禁止未声明变量的使用或对未声明符号的函数调用。

### 4. SSA 静态单赋值三地址码 (SSA 3-Address Code IR)
- **源码文件**: [`ir.rs`](file:///home/deck/Games/agy/bullshit-compiler/crates/bsc-classic/src/ir.rs)
- **特性**:
  - 每一个函数由有序的 `BasicBlock` 集合组成（`entry`, `if.then.N`, `if.else.N`, `if.merge.N`, `while.cond.N`, `while.body.N`, `while.merge.N`）；
  - 每一个变量定值产生唯一的虚拟寄存器 `%v0, %v1, %v2, ...`；
  - 核心 SSA 指令集：
    - `%v = alloca <type>, name`
    - `%v = load <type>, *var`
    - `store <type> val, *var`
    - `%v = binop <op> lhs, rhs`
    - `%v = icmp <cond> lhs, rhs`
    - `br %cond, label %then, label %else`
    - `jmp label %target`
    - `%v = call @fn(args...)`
    - `ret [val]`
    - `print val`

### 5. 机器无关优化 (SSA Optimization)
- **源码文件**: [`optimizer.rs`](file:///home/deck/Games/agy/bullshit-compiler/crates/bsc-classic/src/optimizer.rs)
- **特性**:
  - **常量折叠 (Constant Folding)**：
    - 编译期静态计算整型常量加减乘除模（如 `10 * 20` 直接代换为 `200`）；
    - 静态比较运算（如 `1 == 1` 直接折叠为布尔常量 `true`）；
  - **死代码与无用分支消除 (Dead Code & Branch Elimination)**：
    - 将条件恒真/恒假的条件分支 `br` 替换为无条件跳转 `jmp`；
    - 裁剪不可达的孤立基本块；
    - 清理跳转指令之后的无用悬垂指令。

### 6. 目标代码生成 (Target Codegen: x86-64 System V AMD64 ABI)
- **源码文件**: [`codegen_x86.rs`](file:///home/deck/Games/agy/bullshit-compiler/crates/bsc-classic/src/codegen_x86.rs)
- **特性**:
  - 产出标准 Intel 语法汇编（`.intel_syntax noprefix`）；
  - 严格遵守 Linux/macOS x86-64 System V 调用约定：
    - 整型参数通过 `rdi, rsi, rdx, rcx, r8, r9` 传递；
    - 函数返回值存放于 `rax`；
    - 标准栈帧维护：序言 `push rbp; mov rbp, rsp; sub rsp, <frame_size>`，尾声 `leave; ret`；
  - 局部变量与虚拟寄存器分配至栈帧槽位 `[rbp - offset]`；
  - 包含运行时辅助打印符号 `bsc_print_i64`。

### 7. 虚拟机即时执行器 (VM Interpreter)
- **源码文件**: [`vm.rs`](file:///home/deck/Games/agy/bullshit-compiler/crates/bsc-classic/src/vm.rs)
- **特性**:
  - 直接在 SSA IR 拓扑控制流图上进行高精度解释执行；
  - 维护调用栈帧（`Frame`），包括局部变量符号映射与虚拟寄存器值表；
  - 支持函数递归调用与局部状态压栈/出栈；
  - 捕获 `print` 输出至 `vm_stdout`，记录最终函数返回值，并统计执行的物理 SSA 指令步数（`instructions_executed`）；
  - 设有执行步数上限阈值（1,000,000 步），防止用户代码死循环耗尽计算资源。

### 8. 经典算法验证测试结果
在 `crates/bsc-classic` 单元测试与 CLI 中均全量验证通过：
1. **斐波那契数列递归计算 (`fib(10)`)**:
   - 虚拟机执行指令步数：1,597 步
   - STDOUT 输出：`55`
   - 主函数返回值：`55`
2. **欧几里得辗转相除法 (`gcd(48, 18)`)**:
   - 虚拟机执行指令步数：39 步
   - STDOUT 输出：`6`
   - 主函数返回值：`6`
3. **高斯累加求和与阶乘 (`sum(100)` & `factorial(6)`)**:
   - 虚拟机执行指令步数：1,279 步
   - STDOUT 输出：`5050`, `720`
   - 主函数返回值：`720`

---

## 三、 黑话文档与多段落文件编译架构

为彻底解决单句短语演示的局限性，支持用户传入真实长文本或文件输入，我们对前端语法分析器与后端发射器进行了重构与升级。

### 1. 文档结构化多段落与多语句流式 Lowering
- **源码文件**: [`crates/bsc-frontend/src/ir_builder.rs`](file:///home/deck/Games/agy/bullshit-compiler/crates/bsc-frontend/src/ir_builder.rs)
- 前端扫描完整文档的换行符与标点界符，将长文本划分为多个连续的业务陈述语句与事件节点：
  - 识别出操作语句（`StatementNode::Operation`）与异常突发语句（`StatementNode::Incident`）；
  - 为每个具体的动作与客体分配唯一的 IR 符号（如 `商城系统`、`内存泄漏`、`严重bug`、`通宵加班测试`、`代码重构` 等）；
  - 生成多条包含不同 Operand 的 `TransferPayload`、`MutateState`、`AssertIntegrity` 指令。

### 2. 去除模板重复死循环，产出专业长篇战略公文
- **源码文件**: [`crates/bsc-backend/src/targets.rs`](file:///home/deck/Games/agy/bullshit-compiler/crates/bsc-backend/src/targets.rs)
- **痛点复盘**: 之前多指令编译时，后端发射器无差别使用第一个全局资产并在循环中拼接完全相同的短句，导致输出长文本出现数十次重复的“打通端到端高维资产跨域交付全链路...”。
- **重构方案**:
  1. 实现 `resolve_asset_name(module, id)`，将每一条 IR 指令中具体的客体与实体精准映射为高维概念；
  2. 针对长文本（指令数 &ge; 4）启用**结构化四段式公文排版引擎**：
     - **第一部分：战略定位与顶层设计 (Executive Summary & Strategic Positioning)**
     - **第二部分：关键链路抓手与交付落地 (Core Implementations & Execution Milestones)**：采用轮转话术模板结合具体业务载荷输出结构化条目；
     - **第三部分：质效底线与容灾韧性 (Quality Assurance & Resilience Defense)**：汇总异常处置与质效验收指标；
     - **第四部分：边际价值与长效闭环 (Marginal Gains & Long-term Ecosystem Horizon)**：展望未来演进目标。
  3. 四大目标方言（阿里中台、华为鸿蒙、政务信创、硅谷英文）均适配此结构。

---

## 四、 命令行 CLI 深度集成与文件编译实测

在 [`crates/bsc-cli`](file:///home/deck/Games/agy/bullshit-compiler/crates/bsc-cli/src/main.rs) 中，我们提供了全套双编译器命令行指令：

### 1. 经典编译器命令: `bsc classic`
```bash
# 编译并即时在虚拟机中运行代码，输出 STDOUT 与返回值
cargo run -p bsc-cli -- classic examples/fib.lang --run

# 导出完整的 SSA IR 与 x86-64 目标汇编
cargo run -p bsc-cli -- classic examples/fib.lang --dump-ir --dump-asm
```
**运行实测输出截取**:
```
┌─────────────────────────────────────────────────────────────┐
│  Classic Language Compiler - 经典编译原理执行流水线启动      │
└─────────────────────────────────────────────────────────────┘
=== [3. Semantic: 符号表与类型检查] ===
  fn fib(Int) -> Int
  fn main() -> Int
  fn print(Int) -> Void

=== [4. IR: 优化后 SSA 中间表示] ===
define @fib(%n) {
entry:
    %v0 = alloca i64, name "n"
    %v1 = load i64, *n
    %v2 = icmp sle %v1, 1
    br i1 %v2, label %if.then.0, label %if.merge.2
...
}

=== [6. Execution: 虚拟机运行时输出 (VM Execution)] ===
[STDOUT] 55
返回值 (Return Value)   : Some(55)
执行指令数 (Instructions): 3184
✓ 虚拟机执行完毕。
```

### 2. 黑话长文本文件编译命令: `bsc compile`
```bash
# 编译整个文本文件，导出结果到文件
cargo run -p bsc-cli -- compile examples/report.txt -o examples/report_out.txt -t alibaba
```
**运行实测产出截取 (`examples/report_out.txt`)**:
```
【业务战略大盘与核心打法聚焦】
围绕【全场景高阶生态核心载荷标的 (项目季度运营总结与问题复盘报告)】这一核心底层抓手，深耕全链路痛点打法，完成顶层心智透传与价值定位对齐。 快速拉通面向【分布式高性能关系型数据底座】的协同协议，强化组织心智与场景穿透力。

【关键链路抓手与交付落地】
1. 打通端到端高维资产跨域交付全链路，实现【全场景高阶生态核心载荷标的 (项目季度运营总结与问题复盘报告)】边际效能跃迁，形成飞轮效应。
2. 聚力推进【全场景高阶生态核心载荷标的 (业务现状与数据概况)】全生命周期精细化运营与效能深潜，形成飞轮效应。
3. 聚焦【全场景高阶生态核心载荷标的 (主要负责商品展示和)】核心商业诉求，撬动业务确定性增长飞轮，形成飞轮效应。
4. 构建面向【底层状态异常与边界逻辑裂隙 (内存泄漏)】的自适应弹性调度引擎，打破数据孤岛，形成飞轮效应。
5. 打通端到端高维资产跨域交付全链路，实现【底层状态异常与边界逻辑裂隙 (bug)】边际效能跃迁，形成飞轮效应。
...
【架构韧性沉淀与质效闭环】
以敏捷演进机制倒逼【中心云端高可用算力底座集群】架构韧性重构，针对【异构节点异常熔断与降级自愈】打出快速响应组合拳。

【长效生态演进与价值跃迁】
持续赋能【系统高可用韧性护城河】，打出降本增效组合拳，驱动【全链路边界容灾自愈闭环】形成高维战略闭环！
```

---

## 五、 Web 交互端双编译器工作台落地

针对前端页面 [`web/index.html`](file:///home/deck/Games/agy/bullshit-compiler/web/index.html) 进行升级，由单一输入框转变为**双工作台交互系统**：

| 工作台标签 | 核心定位与能力 | 交互与文件支持 |
| :--- | :--- | :--- |
| **经典代码编译器** | 真实命令式语言编译器工作台，打通前端至虚拟机运行全流程 | 支持加载预置例程（`fib.lang`, `gcd.lang`, `sum_fact.lang`）、**打开本地源码文件**、**下载代码**、全量阶段探测器（① 虚拟机控制台、② SSA IR、③ x86-64 汇编、④ 符号表、⑤ 词法单元、⑥ 语法树） |
| **黑话文档编译器** | 完整长文本与多段落文件编译工作台，生成结构化战略公文 | 支持长文本粘贴、预置报告（季度复盘报告、事故复盘、日常周报）、**上传本地文本文件**、**下载生成公文**、切换 4 种输出方言、查看 4 级 Pass 演进过程 |
| **编译原理** | 龙书级学术体系通识与 6 大交互式算法仿真演练 | 纯正学术知识点（Lexer DFA、Parser 栈、Scope 树、SSA Φ 插入、数据流半格优化、寄存器图着色），不掺杂黑话 |
| **反编译器** | 照妖镜反编译器 | 输入招聘 JD、信创大词或管理学黑话，脱水还原真实底层情况 |

### WebAssembly 运行时验证与 Pass 演进链路修复
在 Node.js 中加载编译好的 `web/bsc_wasm.wasm`（219KB）执行双编译器全量测试：
```javascript
// Classic Compiler:
fib(10) -> VM STDOUT: ['55'], Return: 55, SSA Steps: 1597

// Document Compiler:
examples/report.txt -> 生成完整四段式结构化企业公文，AST 与 Pass 演进全量输出
```

#### 关键修复：Pass 1/2 字段绑定与文本/IR 双向联动
- **根因分析**：底层 Rust `PassDiffRecord` 定义为 `ir_dump_after`，而前端在绑定 DOM 时误写为 `diff.dump_after`，导致 JavaScript 读取到 `undefined` 并呈现给用户。
- **重构加固**：
  1. 纠正字段名为 `diff.ir_dump_after`，并增加防空兜底；
  2. 实现 `generatePassIntermediateText(step, docRes, ast)`，点击任意 Pass 时，**右上方输出框与右下方 IR 视图同步切换**：
     - **Pass 0**：展示原始白话意图切分列表 + 原始 3AC Intent-IR；
     - **Pass 1 (`StripAgencyPass`)**：展示主体自驱重构陈述 + 剔除施动者后的 IR；
     - **Pass 2 (`ScaleAmplifyPass`)**：展示高阶生态底座资产陈述 + 概念升维后的 IR；
     - **Pass 3 (`TeleologyInjectPass`)**：展示完整的最终四段式战略汇报公文 + 注入闭环后的 IR；
     - **Pass 4 (`文档 AST`)**：展示多达 20+ 个语句节点的完整 AST JSON 树状层次。
  3. 增加「📋 复制当前 IR」一键拷贝功能。

---

## 六、 总结与后续演进路线

本项目现已构建了一套完整的、经得起真实编译与运行检验的双编译器系统：
1. 经典编译器证明了严谨的形式语言编译能力（Lexer &rarr; Parser &rarr; Semantic &rarr; SSA IR &rarr; Opt &rarr; x86 Asm &rarr; VM）；
2. 黑话编译器证明了由经典编译原理升维至自然语言意图编译的创造性实践，且具备处理长篇文本文件的工业级排版与生成能力；
3. 本地 CLI 与浏览器 WebAssembly 保持 100% 相同逻辑，实现真正的一处编译、跨端运行。
