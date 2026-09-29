# 010: 编译器可观测性体系、Rustc 级源码诊断与开发者体验深化方案

> **状态**: 已实现并在 CLI、WASM 内核与 Web 交互终端全量验证通过  
> **关联版本**: BSc v0.5.1 (`crates/bsc-classic`, `crates/bsc-wasm`, `crates/bsc-cli`, `web/`)  
> **文档定位**: 记录编译器由“静默黑盒”向“工业级可观测、强诊断、可配置、高交互”编译基础设施的架构升级与技术方案落地。

---

## 一、 演进背景与问题自检复盘

在双编译器架构（经典语言编译器 + 黑话长文本编译器）初步跑通后，用户对系统的实用性、可观测性与人机交互提出了尖锐且关键的要求：
1. **“编译的过程应该有日志打印，干了什么应该有打印的，你这个毛线都没有”**：
   - 编译器此前处于完全静默状态，用户点击编译后只能看到最终产物或冷冰冰的错误，内部经历了多少 Token 扫描、解析了多少 AST 节点、IR 生成了多少指令、优化 Pass 是否生效、解释虚拟机跑了多少步，全为盲区。
2. **“缺少基础功能，拿不上台面”**：
   - 错误处理极度粗糙：只抛出一行 `Expected type at 1:14`，缺乏源码行切片、缺乏 `^^^^` 错误标尺定位、缺乏修复建议；
   - 编辑器如同玩具：原生 `<textarea>`，无行号边栏、Tab 键会导致焦点丢失、无法做多行缩进；
   - 优化机制硬编码：缺乏 `-O0 / -O1 / -O2` 标准优化等级配置开关；
   - 制品链无法归档：用户只能手动复制单个文本框内容，无法一键打包完整的编译工件链。

本轮迭代针对上述硬伤全面重构，建立了**可观测性日志流**、**Rustc 级错误诊断引擎**、**三级优化调度系统**、**专业级编辑器行号/缩进体系**与**工件包一键打包导出能力**。

---

## 二、 全流水线可观测性体系 (Pipeline Telemetry & Logging)

### 1. 分级追踪规范与阶段定义

在 `crates/bsc-classic` 与 `crates/bsc-wasm` 中建立了标准化的流水线追踪事件：

```
                Classic 编译流水线执行阶段追踪 (Pipeline Tracing)
┌───────────┐   [STAGE 0: INIT] 源码字符数统计、优化等级注入 (-O0/-O1/-O2)
│ 源码输入  │─────────┐
└───────────┘         ▼
                [STAGE 1: LEXER] 识别 Tokens 数量，滤除空白与块/行注释
                      │
                      ▼
                [STAGE 2: PARSER] 递归下降分析，构建顶级函数与 AST 语法树节点
                      │
                      ▼
                [STAGE 3: SEMANTIC] 作用域树检查、类型相容性验证、符号表建立
                      │
                      ▼
                [STAGE 4: IR-GEN] SSA 静态单赋值中间代码生成，统计指令条数
                      │
                      ▼
                [STAGE 5: OPTIMIZER] 常量折叠、代数恒等式化简、死代码消除 (DCE)
                      │
                      ▼
                [STAGE 6: CODEGEN] 发射 Intel 语法 x86-64 汇编 (System V AMD64 ABI)
                      │
                      ▼
                [STAGE 7: VM-EXEC] 启动内部 SSA 控制流解释虚拟机执行入口 @main，统计执行步数
```

### 2. 多端呈现与跨语言导出
- **Rust 内核**: `ClassicCompileResponse` 与 `WasmCompileResponse` 均内置 `pub logs: Vec<String>`。
- **CLI 控制台**: `bsc classic` 与 `bsc compile` 在标准输出头部实时打印 `=== [编译流水线执行日志 (Compiler Pipeline Logs)] ===`。
- **Web 终端**: 
  - 经典编译器新增 `② 流水线日志` 全屏子选项卡，支持一键 `📋 复制全部日志`；
  - 虚拟机控制台新增即时执行进度快照；
  - 黑话长文本编译器新增黑色终端控制台，实时滚动展示每个阶段的执行细节。

---

## 三、 Rustc 级源码诊断引擎 (Compiler Diagnostics)

### 1. 架构与模块设计
新建独立模块 [`crates/bsc-classic/src/diagnostic.rs`](file:///home/deck/Games/agy/bullshit-compiler/crates/bsc-classic/src/diagnostic.rs)，定义 `DiagnosticRenderer`：
- **精准行/列解析**: 从词法、语法、语义错误中提取错误坐标（`Span`）；
- **源码切片提取**: 自动截取报错所在行的完整文本；
- **标尺与指示符计算**: 根据报错字符的词法长度动态计算 `^` 符号的宽度（如对于 `i64` 自动生成 `^^^`）；
- **错误码与阶段标签**: 赋予统一格式的错误代码（`E0101` 词法、`E0201` 语法、`E0301` 语义、`E0701` 虚拟机）；
- **智能修复建议**: 根据报错特征自动匹配追加 `= help:` 提示信息。

### 2. 诊断输出实测效果
当用户输入非经典语言类型 `fn main() -> i64 { return 0; }` 时，诊断引擎输出：

```text
error[E0201]: Syntax Analysis Error
 --> input.lang:1:14
  |
 1 | fn main() -> i64 { return 0; }
  |              ^^^ expected valid primitive type
  |
  = help: classic language primitive types are `int`, `bool`, and `void`.
```

当发生类型不相容或未声明变量时，输出：

```text
error[E0301]: Semantic & Type Check Error
 --> input.lang:3:16
  |
 3 |     return x + z;
  |                ^ not found in current scope
  |
  = help: declare variable before referencing it: `let <name>: <type> = <value>;`.
```

---

## 四、 优化等级系统 (-O0 / -O1 / -O2)

在 [`crates/bsc-classic/src/optimizer.rs`](file:///home/deck/Games/agy/bullshit-compiler/crates/bsc-classic/src/optimizer.rs) 中重构了优化器，支持标准优化等级枚举：

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OptLevel {
    O0, // 关闭所有机器无关优化 (保持原始三地址码)
    O1, // 基础常量折叠 (Constant Folding) 与静态分支修剪
    O2, // 激进优化：常量折叠 + 代数恒等式化简 + 全局死代码消除 (DCE)
}
```

### 优化效果对比示例

给定源码：
```rust
fn main() -> int {
    let x: int = 10 + 20;
    return x;
}
```

| 优化等级 | 优化后 SSA IR 指令 | 优化日志与消除记录 |
| :--- | :--- | :--- |
| **`-O0` (关闭)** | `%v0 = add 10, 20`<br>`%v1 = alloca i64, name "x"`<br>`store i64 %v0, *x`<br>`%v2 = load i64, *x`<br>`ret i64 %v2` | `[STAGE 5: OPTIMIZER] 消除 0 处冗余` |
| **`-O2` (激进)** | `%v1 = alloca i64, name "x"`<br>`store i64 30, *x`<br>`%v2 = load i64, *x`<br>`ret i64 %v2` | `[STAGE 5: OPTIMIZER] 常量折叠与代数化简消除 1 处冗余` |

- **CLI 使用**: `bsc classic input.lang -O 0` 或 `bsc classic input.lang -O 2`。
- **Web 使用**: 顶部操作栏提供优化级别下拉选择器，切换即时重编生效。

---

## 五、 开发者人机交互体验全面深化 (DX Deepening)

为彻底解决“原生 textarea 体验如同玩具”的问题，前端在不引入沉重第三方包的前提下实现了全套轻量级专业代码编辑器特性：

### 1. 动态双向行号边栏 (Line Numbers Gutter)
- 源码区采用 Flex 布局拆分为两列：
  - 左侧：`#classic-line-gutter`，动态生成 `1\n2\n3...\nN`，设置 `user-select: none`，右对齐；
  - 右侧：`#classic-source-input`，透明背景、完全等高的等宽字体行高对齐。
- **滚动联动**: 监听 `onscroll` 事件，实时将 `textarea.scrollTop` 赋值给 `gutter.scrollTop`，保持视口绝对对齐。

### 2. 键盘 Tab 键智能缩进与反缩进
- 拦截 `Tab` 键（`e.preventDefault()`）：
  - **按下 `Tab`**: 在当前光标位置插入 4 个空格，光标自动后移 4 位；
  - **按下 `Shift + Tab`**: 检查当前行首是否存在 4 个空格，存在则退格 4 位实现反缩进；
  - 缩进修改后自动更新字符行数统计与行号槽。

### 3. 光标行列定位器
- 实时跟踪光标选区 `selectionStart`，换算当前所处的绝对行号与列号，于编辑器状态条显示 `Ln X, Col Y`。

---

## 六、 编译工件全生命周期导出规范 (Artifacts Packaging)

Web 工作台两端新增 **`📦 导出工件包`** 按钮，支持将编译全流程产生的中间与目标制品一键下载为标准的工程 JSON 归档文件：

```json
{
  "meta": {
    "project": "Bullshit-Compiler ClassicLang",
    "version": "0.5.1",
    "timestamp": "2026-09-28T22:50:00.000Z",
    "opt_level": "-O2",
    "success": true,
    "instructions_executed": 3184,
    "vm_return_value": 55
  },
  "pipeline_artifacts": {
    "source_code": "fn fib(n: int) -> int { ... }",
    "tokens": [ ... ],
    "ast_json": { "functions": [ ... ] },
    "symbol_table": "; Symbol Table & Function Signatures: ...",
    "raw_ssa_ir": "; --- SSA Intermediate Representation --- ...",
    "optimized_ssa_ir": "; --- SSA Intermediate Representation --- ...",
    "target_x86_assembly": ".intel_syntax noprefix\n.globl _start ...",
    "pipeline_trace_logs": [
      "[STAGE 0: INIT] 启动经典语言编译流水线, 源码长度: 231 字符, 优化等级: O2",
      "[STAGE 1: LEXER] 词法扫描完成: 识别出 58 个记号单元 (Tokens)...",
      "[STAGE 7: VM-EXEC] 虚拟机正常执行完毕: 累计执行 3184 条指令, 返回值: Some(55)"
    ],
    "vm_stdout": ["55"]
  }
}
```

---

## 七、 验证结论与后续规划

经过全套验证：
1. `cargo test -p bsc-classic` 全部 3 个单元测试通过（流水线执行、诊断高亮标尺、优化等级验证）；
2. `cargo check --workspace` 整体工程 0 警告 0 报错；
3. `bsc_wasm.wasm` 重新编译完成并在 Node.js 与 Web 浏览器端验证通过；
4. CLI 命令 `bsc classic <file> -O 0/2 --dump-ir` 验证成功；
5. Web 端访问 `http://localhost:8088/` 实测：行号边栏、Tab 缩进、流水线日志追踪与工件导出均正常工作。

下一步将继续探索：
- x86-64 后端由栈溢出向线性扫描/图着色寄存器分配（RegAlloc）推进；
- 自然语言前端引入上下文无关文法（CFG）与多重意图树深度解析。
