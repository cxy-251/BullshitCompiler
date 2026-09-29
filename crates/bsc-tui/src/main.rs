use std::io::{self, stdout};
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Tabs, Wrap, List, ListItem},
    Terminal,
};
use bsc_classic::{compile_classic_pipeline_with_opt, ClassicCompileResponse, OptLevel};
use bsc_decompiler::{Decompiler, DecompileResult};

const SAMPLES_CLASSIC: [(&str, &str); 5] = [
    (
        "斐波那契递归 (fib.lang)",
        "fn fib(n: int) -> int {\n    if (n <= 1) {\n        return n;\n    }\n    return fib(n - 1) + fib(n - 2);\n}\n\nfn main() -> int {\n    return fib(10);\n}\n",
    ),
    (
        "欧几里得辗转相除 (gcd.lang)",
        "fn gcd(a: int, b: int) -> int {\n    while (b != 0) {\n        let t: int = b;\n        b = a % b;\n        a = t;\n    }\n    return a;\n}\n\nfn main() -> int {\n    return gcd(48, 18);\n}\n",
    ),
    (
        "高斯累加求和 (sum.lang)",
        "fn sum_n(n: int) -> int {\n    let s: int = 0;\n    let i: int = 1;\n    while (i <= n) {\n        s = s + i;\n        i = i + 1;\n    }\n    return s;\n}\n\nfn main() -> int {\n    return sum_n(100);\n}\n",
    ),
    (
        "代数化简与死代码消除 (opt_test.lang)",
        "fn main() -> int {\n    let a: int = 10 * 5 + 2;\n    let b: int = a * 0;\n    let c: int = b + 42;\n    let dead: int = 999 * 2;\n    return c;\n}\n",
    ),
    (
        "语法错误与诊断波浪线演示 (err_demo.lang)",
        "fn main() -> int {\n    let x: int = 100 + ;\n    return x;\n}\n",
    ),
];

const SAMPLES_DECOMPILE: [(&str, &str); 4] = [
    (
        "全栈抗压岗位 JD",
        "要求具备全栈端到端交付能力，能够在高烈度敏捷战役中承压攻坚，具备主人翁意识与自驱力，快速落地闭环。",
    ),
    (
        "信创核心架构汇报",
        "依托分布式软总线与异构互联微内核基线，全面推进自主可控与新质生产力赋能，打通全链路技术底座。",
    ),
    (
        "大厂晋升汇报话术",
        "寻找业务核心抓手，深耕全链路痛点打法，打出降本增效组合拳，沉淀用户心智形成商业飞轮闭环。",
    ),
    (
        "降本增效裁员公告",
        "为了应对宏观经济周期与结构性战略调整，公司进行业务阵型优化聚焦与资源协同重构，赋能精细化运营。",
    ),
];

const THEORY_TITLES: [&str; 8] = [
    "01. 形式语言代数与词法分析 (Chomsky/Thompson/DFA)",
    "02. 上下文无关文法与现代语法分析 (LL/LR/Pratt)",
    "03. 语义分析、属性文法与类型理论 (HM/Type Safety)",
    "04. 中间表示体系与 SSA 形式 (CFG/Dominance/Cytron)",
    "05. 机器无关优化与数据流分析 (Kildall/SCCP/LICM)",
    "06. 目标代码生成与硬件架构适配 (Tree-Tiling/ABI)",
    "07. 寄存器分配经典理论与算法 (Graph-Coloring/Spill)",
    "08. 运行时系统与垃圾回收体系 (VM/Tri-color GC)",
];

const THEORY_TEXTS: [&str; 8] = [
    // Chapter 1
    r#"【专篇 01: 形式语言代数与词法分析 (Formal Grammars & Lexical Analysis)】

1. 形式语言理论与乔姆斯基四层谱系 (Chomsky Hierarchy):
   文法四元组 G = (V_N, V_T, P, S)。
   • 0型 (无限制文法): 产生式 α → β，由图灵机 (Turing Machine) 识别。
   • 1型 (上下文相关文法 CSG): αAβ → αγβ (|γ| ≥ |A|)，由线性界限自动机 (LBA) 识别。
   • 2型 (上下文无关文法 CFG): A → α (A ∈ V_N)，由非确定下推自动机 (PDA) 识别。
   • 3型 (正规文法 RG): A → aB 或 A → a，由有限状态自动机 (FA) 识别。
   * 正规语言泵引理边界: 有限自动机无状态计数栈，数学上绝对无法识别任意深度括号嵌套 L = { a^n b^n }，必须由 CFG 处理。

2. Thompson 构造法 (RE → ε-NFA):
   结构归纳法在 O(|r|) 线性空间构造单初态单终态 ε-NFA：
   • 连接 rs: N(r) 终态直连 N(s) 初态。
   • 选择 r|s: 新建起点分别以 ε 弧分支至 N(r) 与 N(s)，终态通过 ε 弧汇聚至新终态。
   • 闭包 r*: 引入跨越 ε 旁路 (匹配0次) 与终态回跳初态的 ε 回环 (匹配无限次)。

3. 子集构造法确定化 (Subset Construction):
   • ε-闭包方程: ε-closure(s) = {s} ∪ {t | ∃u ∈ ε-closure(s), u →ε t}。
   • 状态转移方程: Dtran[T, a] = ε-closure( move(T, a) )。工作表队列单调遍历收敛。

4. Hopcroft 算法 (DFA 状态极小化):
   • 初始划分 P = { F, S \ F } (终态组与非终态组)。
   • 依据输入字符 a 是否转移到同一划分组进行不可区分性分裂，输出全局唯一极小化 DFA。

5. 工业级扫描器工程:
   • 双缓冲区与 EOF 哨兵字符，单次循环消除越界检查。
   • 最长匹配原则 (Maximal Munch)，零拷贝 &'input str 与 Span 映射。"#,

    // Chapter 2
    r#"【专篇 02: 上下文无关文法与现代语法分析 (CFG & Syntax Analysis)】

1. CFG 二义性与不可判定性:
   • 文法四元组 G = (V_N, V_T, P, S)。若存在两棵不同的语法树则为二义文法。
   • 二义性不可判定性定理: 由 Post 对应问题 (PCP) 规约证明，无通用算法可判定任意 CFG 是否具有二义性。
   • 算符结合性与悬空 Else (Dangling Else) 需由产生式分层或贪婪规则消解。

2. 自顶向下 LL(1) 文法全套构造:
   • 消除左递归: A → Aα | β 改写为 A → βA', A' → αA' | ε。提取左公因子延迟决策。
   • FIRST(α) 集合: 串 α 能推导出的首终结符集合。
   • FOLLOW(A) 集合: 紧随 A 之后的终结符集合，开始符号初始置 $ 结束符。
   • SELECT(A → α) 集合: 若 α 无法推出 ε 则为 FIRST(α)，否则包含 FOLLOW(A)。
   • LL(1) 充要条件: 对同左部的任意候选式，SELECT 集合两两互斥。

3. 自底向上 LR 族演进谱系:
   • LR(0): 基础圆点项目 [A → α·β]，极易产生移进-规约/规约-规约冲突。
   • SLR(1): 仅在当前输入符号 a ∈ FOLLOW(A) 时才执行规约。
   • LR(1): 项目携带向前看符号 [A → α·β, a]，精准消除上下文误判，但状态表爆炸。
   • LALR(1): 合并同心集 (Core Items) 状态压缩体积 (Yacc/Bison 核心)，但注意合并可能引入规约-规约冲突！

4. 现代工业级实践: Pratt 算符优先级算法:
   • 结合力数学模型: 为算符分配 (left_bp, right_bp)。
   • 为什么 Rustc/Swift/V8/Clang 抛弃生成器全面拥抱手写递归下降 + Pratt: 错误恢复能力强、增量解析支持好、报错上下文精确。"#,

    // Chapter 3
    r#"【专篇 03: 语义分析、属性文法与类型理论 (Semantics, SDD/SDT & Type Theory)】

1. 语法制导定义 (SDD) 与属性文法:
   • 综合属性 (Synthesized) 与 S-属性文法: 节点值自子节点合成，天然嵌入自底向上规约动作。
   • 继承属性 (Inherited) 与 L-属性文法: 节点值依赖父节点或左侧兄弟，依赖图无后向有向环，允许深度优先前序单趟求值。

2. 嵌套作用域环境链模型 (Lexical Scoping):
   • 符号表以父指针树 (Parent-Pointer Tree) 组织。
   • enter_scope() / exit_scope() 栈式生命周期，变量沿父链冒泡检索，内层局部天然遮蔽 (Shadowing)。
   • 声明提前 (Hoisting) 依赖两趟语义分析 (先收集顶级签名，再深入函数体检查)。

3. 静态类型推导与 Hindley-Milner 算法:
   • 类型判断式: Γ ⊢ e : τ。
   • 参数多态 (Let-Polymorphism) 与算法 W (Algorithm W)。
   • Robinson 合一算法 (Unification) 与 Occurs Check 循环检查 (阻断 α = List(α) 无限递归死锁)。

4. 类型健全性理论证明 (Type Safety Theorems):
   • Robin Milner 定则: "Well-typed programs cannot go wrong"。
   • 进度定理 (Progress): 良类型闭合项要么是最终值，要么存在下一步求值状态 e → e'。
   • 保全定理 (Preservation / Subject Reduction): 若 Γ ⊢ e : τ 且 e → e'，则必有 Γ ⊢ e' : τ。

5. 仿射逻辑与 Rust 所有权借用检查:
   • 仿射子结构类型 (Affine Types): 资源在生命周期内至多使用一次。
   • 借用互斥性: 唯一可变借用 &mut T 与只读共享借用 &T 形式化互斥，根绝数据竞争。
   • 非词法作用域生命周期 (NLL) 基于控制流图点集区间精确求解。"#,

    // Chapter 4
    r#"【专篇 04: 中间表示体系与 SSA 静态单赋值形式 (IR Architectures & SSA Form)】

1. 编译器多级 IR 谱系:
   • 高层 IR (HIR/AST): 保留高级控制流、模式匹配与泛型特化，负责类型检查与语法脱糖。
   • 中层 IR (MIR/SSA / LLVM IR): 基本块控制流图，规整三地址码，是机器无关优化的核心载体。
   • 底层 IR (LIR): 暴露目标机器指令操作码、无限虚拟寄存器与显式调用栈，用于寄存器分配。

2. 控制流图 (CFG) 与基本块 (Basic Block):
   • Leaders 准则: 程序的首条指令、跳转目标指令、紧跟跳转指令之后的指令为 Leader。
   • 每个 Leader 到下一个 Leader 之前构成基本块，块间有向边构成 CFG = (V, E)。

3. 支配理论与 Lengauer-Tarjan 快速算法:
   • 支配 (d dom n): 若进入 n 的每条路径都必经 d，则 d 支配 n。
   • 直接支配者 (IDOM) 构成唯一的支配树 (Dominator Tree)。
   • Lengauer-Tarjan 算法: DFS 序结合半支配者 (Semi-dominator) 与带权并查集路径压缩，近线性求出 IDOM。

4. Cytron 经典 SSA 构造算法:
   • 支配边界 (Dominance Frontier, DF): DF(X) = { Y | ∃P ∈ Pred(Y), X dom P ∧ ¬(X sdom Y) }。
   • Φ 节点插入定理: 若变量在块 B 被定值，则其迭代支配边界 DF+(B) 内的每个块头部必须插入 Φ 节点。
   • 变量版本重命名: 深度优先遍历支配树与版本号计数栈。

5. SSA 析构 (Out-of-SSA):
   • 真实 CPU 无 Φ 指令。关键边分割 (Critical Edge Splitting) 在多后继到多前驱边上合成空跳转块。
   • 寄存器并行拷贝拓扑排序消除 Swap 死锁。"#,

    // Chapter 5
    r#"【专篇 05: 机器无关代码优化与数据流分析 (Machine-Independent Optimization & Dataflow Analysis)】

1. 数据流分析抽象数学框架:
   • 有界半格系统 <L, ∧, ⊤, ⊥>。⊤ 表示乐观无信息，⊥ 表示混乱非常量，∧ 为汇聚算子。
   • 传递函数 fB 的单调性条件: x ≤ y ⇒ f(x) ≤ f(y)。
   • Kildall 不动点迭代算法: 由 Tarski 不动点定理，在有限高度格上单调递减必停机收敛于 MFP 解。
   • 分配律成立时，MFP 严格等价于理想全路径汇聚解 MOP (Meet-Over-all-Paths)。

2. 三大经典数据流方程组对比:
   • 到达定值 (Reaching Defs): 前向/并集，Out[B] = Gen[B] ∪ (In[B] \ Kill[B])。构建 Use-Def 链。
   • 活跃变量 (Live Variables): 后向/并集，In[B] = Use[B] ∪ (Out[B] \ Def[B])。DCE 与寄存器分配基石。
   • 可用表达式 (Available Exprs): 前向/交集，Out[B] = Gen[B] ∪ (In[B] \ Kill[B])。公共子表达式消除。

3. 现代核心优化 Pass:
   • 稀疏条件常量传播 (SCCP): Wegman & Zadeck 算法，数据流工作表与控制流队列联动，单趟同时折叠常量并剪除死分支。
   • 全局值编号 (GVN): 基于支配树自顶向下遍历与哈希表传播，消除跨基本块冗余计算。
   • 激进死代码消除 (ADCE): 基于反向控制流后支配树的控制依赖图 (CDG) 标记-清除。
   • 循环优化: 自然循环识别、循环首部规范化 (Preheader)、循环不变量外提 (LICM)、归纳变量强度削弱。"#,

    // Chapter 6
    r#"【专篇 06: 目标代码生成与物理体系架构适配 (Target Code Generation & Architecture Lowering)】

1. 指令选择 (Instruction Selection):
   • 树重写系统与机器指令瓦片 (Tiles) 模式匹配。
   • 最大吞食算法 (Maximal Munch): 自顶向下贪心匹配覆盖最大的指令瓦片。
   • 动态规划最优覆盖算法 (Burg): 为每条机器指令赋予执行延迟代价，自底向上单调递推全局最低代价覆盖。

2. 指令调度与流水线冒险消除 (Instruction Scheduling):
   • 数据冒险 (RAW, WAR, WAW) 会导致硬件流水线插入气泡 (Bubble/Stall)。
   • 构建数据依赖有向无环图 (DAG)，计算关键路径长度。
   • 列表调度算法 (List Scheduling): 就绪队列优先发射关键路径最长指令，利用无关计算消除内存加载延迟。

3. System V AMD64 ABI 工业级栈帧规范:
   • 整型传参: rdi, rsi, rdx, rcx, r8, r9 (前6个)，超出部分逆序压栈。
   • 浮点传参: xmm0 ~ xmm7。返回值: rax (标量整型)。
   • Callee-saved (被调用者保存): rbx, rsp, rbp, r12, r13, r14, r15 (返回前必须复原)。
   • Caller-saved (调用者保存): rax, rcx, rdx, rsi, rdi, r8-r11 (跨调用易失)。
   • 16 字节对齐强制硬件约束: 在发出 call 指令前，栈指针必须满足 (rsp + 8) % 16 == 0，杜绝 AVX/SSE 崩溃。
   • 128 字节红区 (Red Zone): rsp 栈顶下方区域受保护，叶子函数无需调整 rsp 即可作为极速局部栈。"#,

    // Chapter 7
    r#"【专篇 07: 寄存器分配经典理论与算法演进 (Register Allocation Theory)】

1. 寄存器分配的数学本质与 NP-完全性:
   • 物理寄存器极其有限，虚拟变量无限。
   • 活跃变量分析求得变量活跃期 (Live Ranges)。同时活跃的变量在干涉图 G = (V, E) 中连边。
   • 将寄存器分配规约为图的 K-着色问题 (K 为可用物理寄存器总数)，严格证明为 NP-完全问题。

2. Chaitin-Briggs 图着色分配四阶段流水线:
   • 化简 (Simplify): 基于 Kempe 启发式，度数 < K 的节点其邻居着色后无论如何必然至少剩一个颜色，压栈并临时移出图。
   • 凝聚 (Coalesce): 合并无冲突 move 指令节点 (Briggs / George 保守合并准则，避免产生度数 ≥ K 节点)。
   • 溢出 (Spill): 当所有节点度数均 ≥ K 时，按 Cost(v) / Deg(v) 挑选代价最小的节点标记溢出并写入内存栈。
   • 选择 (Select): 逆序出栈贪心分配不重叠的物理寄存器，乐观分配成功挽救潜在溢出。

3. 线性扫描寄存器分配 (Linear Scan):
   • Poletto & Sarkar 算法，复杂度仅 O(N)。
   • 按生存期起始时间单趟扫描推进活跃集合，牺牲全局最优性换取极高编译吞吐，成为 JIT 编译器标准 (Java C1, V8 Sparkplug)。

4. 现代 LLVM 工业级贪婪分配器 (Greedy Allocator):
   • 核心精髓在于活跃区间拆分 (Live Interval Splitting): 在高寄存器压力基本块边界切断长区间，局部赋寄存器，全局低频区溢出。"#,

    // Chapter 8
    r#"【专篇 08: 现代运行时系统与垃圾回收体系 (Runtime Systems & Garbage Collection)】

1. 执行引擎全景对比:
   • 静态编译 AOT (C++, Rust, Go) vs 字节码解释虚拟机 (Bytecode VM)。
   • 栈式虚拟机 (Stack VM): 指令短小紧凑，编译器生成极简，但访存带宽压力大 (JVM, WASM, Python)。
   • 寄存器式虚拟机 (Register VM): 指令携带寄存器索引，指令条数减少约 35%，分发效率大幅提升 (Lua 5.1+, Dalvik)。
   • JIT 即时编译: 分层编译 (Tiered Compilation)、栈上替换 (OSR) 与基于画像的推测性去优化 (Deoptimization)。

2. 堆内存自动回收 (GC) 核心算法与模型:
   • 引用计数 (RC): 确定性即时销毁；循环引用需靠弱引用 (Weak References) 或试探性染色破环。
   • 根集合 (GC Roots): 全局变量、线程调用栈、CPU 寄存器活跃指针。
   • 三色标记抽象模型 (Tri-color Marking):
     - 白色 (White): 未被访问的候选垃圾对象，流程结束仍为白则回收。
     - 灰色 (Gray): 自身已被访问，但引用的子对象尚未遍历完毕。
     - 黑色 (Black): 确认为存活对象，自身及直接引用的子对象全部标记完毕。

3. 并发写屏障机制 (Write Barriers):
   • 漏标/对象丢失充分必要条件: 黑色对象指向白色对象 且 灰色对象到该白色对象的引用链被切断。
   • Dijkstra 插入写屏障: 拦截黑写白，强制染灰打破条件1。
   • Yuasa 删除写屏障: 拦截删除引用，强制染灰旧对象保持弱三色不变性。
   • 弱分代假说 (Weak Generational Hypothesis): >90% 对象朝生夕灭。新生代 Cheney 复制，老年代标记-整理，卡表 (Card Table) 记录跨代引用指针规避全堆扫描。"#,
];

struct App {
    main_tab: usize, // 0: Classic, 1: Decompile, 2: Theory
    // Classic state
    classic_sample_idx: usize,
    classic_code: String,
    opt_level: OptLevel,
    classic_subtab: usize, // 0: Tokens, 1: AST, 2: SSA IR, 3: Bytecode, 4: Logs & Output
    classic_result: Option<ClassicCompileResponse>,
    // Decompile state
    decompile_sample_idx: usize,
    decompile_input: String,
    decompile_result: Option<DecompileResult>,
    // Theory state
    theory_chapter_idx: usize,
    theory_scroll: u16,
    // Status
    status_bar: String,
}

impl App {
    fn new() -> Self {
        let default_code = SAMPLES_CLASSIC[0].1.to_string();
        let default_jargon = SAMPLES_DECOMPILE[0].1.to_string();
        let mut app = Self {
            main_tab: 0,
            classic_sample_idx: 0,
            classic_code: default_code,
            opt_level: OptLevel::O2,
            classic_subtab: 0,
            classic_result: None,
            decompile_sample_idx: 0,
            decompile_input: default_jargon,
            decompile_result: None,
            theory_chapter_idx: 0,
            theory_scroll: 0,
            status_bar: "就绪 | [F5/Enter] 编译运行 | [O] 切换优化级别 | [S] 切换用例 | [Tab] 切换分栏 | [1-3] 切换主页面 | [Q] 退出".to_string(),
        };
        app.run_classic_compile();
        app.run_decompile();
        app
    }

    fn run_classic_compile(&mut self) {
        let resp = compile_classic_pipeline_with_opt(&self.classic_code, self.opt_level);
        if resp.success {
            self.status_bar = format!(
                "编译成功! 执行指令数: {}, 返回值: {:?}, 优化级别: {:?}",
                resp.instructions_executed, resp.vm_return_val, self.opt_level
            );
        } else {
            self.status_bar = format!("编译未通过! 错误诊断已高亮捕获 (按右键切到 [5. 运行与诊断])");
        }
        self.classic_result = Some(resp);
    }

    fn run_decompile(&mut self) {
        let decompiler = Decompiler::new();
        let res = decompiler.decompile(&self.decompile_input);
        self.decompile_result = Some(res);
        self.status_bar = "反编译脱水分析完成! 直击大白话残酷真相".to_string();
    }

    fn next_classic_sample(&mut self) {
        self.classic_sample_idx = (self.classic_sample_idx + 1) % SAMPLES_CLASSIC.len();
        self.classic_code = SAMPLES_CLASSIC[self.classic_sample_idx].1.to_string();
        self.run_classic_compile();
    }

    fn next_decompile_sample(&mut self) {
        self.decompile_sample_idx = (self.decompile_sample_idx + 1) % SAMPLES_DECOMPILE.len();
        self.decompile_input = SAMPLES_DECOMPILE[self.decompile_sample_idx].1.to_string();
        self.run_decompile();
    }

    fn cycle_opt_level(&mut self) {
        self.opt_level = match self.opt_level {
            OptLevel::O0 => OptLevel::O1,
            OptLevel::O1 => OptLevel::O2,
            OptLevel::O2 => OptLevel::O0,
        };
        self.run_classic_compile();
    }
}

fn main() -> Result<(), io::Error> {
    enable_raw_mode()?;
    let mut stdout_handle = stdout();
    execute!(stdout_handle, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout_handle);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();

    loop {
        terminal.draw(|f| ui(f, &app))?;

        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => break,
                KeyCode::Char('1') => app.main_tab = 0,
                KeyCode::Char('2') => app.main_tab = 1,
                KeyCode::Char('3') => app.main_tab = 2,
                KeyCode::Tab => {
                    if app.main_tab == 0 {
                        app.classic_subtab = (app.classic_subtab + 1) % 5;
                    } else if app.main_tab == 2 {
                        app.theory_chapter_idx = (app.theory_chapter_idx + 1) % 8;
                        app.theory_scroll = 0;
                    }
                }
                KeyCode::BackTab => {
                    if app.main_tab == 0 {
                        app.classic_subtab = (app.classic_subtab + 4) % 5;
                    } else if app.main_tab == 2 {
                        app.theory_chapter_idx = (app.theory_chapter_idx + 7) % 8;
                        app.theory_scroll = 0;
                    }
                }
                KeyCode::F(5) | KeyCode::Enter => {
                    if app.main_tab == 0 {
                        app.run_classic_compile();
                    } else if app.main_tab == 1 {
                        app.run_decompile();
                    }
                }
                KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    if app.main_tab == 0 {
                        app.run_classic_compile();
                    } else if app.main_tab == 1 {
                        app.run_decompile();
                    }
                }
                KeyCode::Char('o') => {
                    if app.main_tab == 0 {
                        app.cycle_opt_level();
                    }
                }
                KeyCode::Char('s') => {
                    if app.main_tab == 0 {
                        app.next_classic_sample();
                    } else if app.main_tab == 1 {
                        app.next_decompile_sample();
                    }
                }
                KeyCode::Left => {
                    if app.main_tab == 0 {
                        app.classic_subtab = (app.classic_subtab + 4) % 5;
                    }
                }
                KeyCode::Right => {
                    if app.main_tab == 0 {
                        app.classic_subtab = (app.classic_subtab + 1) % 5;
                    }
                }
                KeyCode::Up => {
                    if app.main_tab == 2 {
                        if app.theory_scroll > 0 {
                            app.theory_scroll -= 1;
                        }
                    }
                }
                KeyCode::Down => {
                    if app.main_tab == 2 {
                        app.theory_scroll = app.theory_scroll.saturating_add(1);
                    }
                }
                KeyCode::PageUp => {
                    if app.main_tab == 2 {
                        app.theory_scroll = app.theory_scroll.saturating_sub(10);
                    }
                }
                KeyCode::PageDown => {
                    if app.main_tab == 2 {
                        app.theory_scroll = app.theory_scroll.saturating_add(10);
                    }
                }
                _ => {}
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    println!("BSc TUI 已安全退出。感谢使用！");
    Ok(())
}

fn ui(f: &mut ratatui::Frame, app: &App) {
    let size = f.area();

    // 垂直切分布局: 顶部标题栏(3) + 主工作区(自适应) + 底部状态栏(3)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(size);

    // 1. 顶部主标签栏
    let titles = vec![
        " [1] 经典代码编译器 (Classic Workbench) ",
        " [2] 照妖镜反编译器 (Bullshit Decompiler) ",
        " [3] 编译原理通识大百科 (Dragon Book Academic Treatise) ",
    ];
    let tabs = Tabs::new(titles)
        .block(Block::default().borders(Borders::ALL).title(" Bullshit Compiler (BSc) v0.5.0 - 原生跨平台终端编译器工作台 "))
        .select(app.main_tab)
        .style(Style::default().fg(Color::DarkGray))
        .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));
    f.render_widget(tabs, chunks[0]);

    // 2. 主工作区渲染
    match app.main_tab {
        0 => render_classic_tab(f, chunks[1], app),
        1 => render_decompile_tab(f, chunks[1], app),
        2 => render_theory_tab(f, chunks[1], app),
        _ => {}
    }

    // 3. 底部快捷键与状态栏
    let status_text = format!(" ● {}", app.status_bar);
    let status_bar = Paragraph::new(status_text)
        .block(Block::default().borders(Borders::ALL).title(" 系统状态与快捷键提示 "))
        .style(Style::default().fg(Color::Cyan));
    f.render_widget(status_bar, chunks[2]);
}

fn render_classic_tab(f: &mut ratatui::Frame, area: Rect, app: &App) {
    // 水平分割: 左侧源码 (45%) + 右侧多阶段探测器 (55%)
    let h_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(area);

    // 左侧: 源码面板
    let opt_badge = match app.opt_level {
        OptLevel::O0 => "-O0 (无优化)",
        OptLevel::O1 => "-O1 (代数化简)",
        OptLevel::O2 => "-O2 (全优化+DCE)",
    };
    let sample_name = SAMPLES_CLASSIC[app.classic_sample_idx].0;
    let code_title = format!(" 源码编辑区 [{}] | 优化: {} (按 O 切换) ", sample_name, opt_badge);
    
    // 源码加行号渲染
    let mut code_lines = Vec::new();
    for (i, line) in app.classic_code.lines().enumerate() {
        code_lines.push(Line::from(vec![
            Span::styled(format!("{:2} │ ", i + 1), Style::default().fg(Color::DarkGray)),
            Span::styled(line, Style::default().fg(Color::White)),
        ]));
    }
    let code_paragraph = Paragraph::new(code_lines)
        .block(Block::default().borders(Borders::ALL).title(code_title))
        .style(Style::default());
    f.render_widget(code_paragraph, h_chunks[0]);

    // 右侧: 垂直切分 子Tab(3) + 检查内容(自适应)
    let r_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(5)])
        .split(h_chunks[1]);

    let sub_titles = vec![
        " [1. Tokens] ",
        " [2. AST 语法树] ",
        " [3. SSA 三地址码] ",
        " [4. 汇编字节码] ",
        " [5. 运行与诊断] ",
    ];
    let sub_tabs = Tabs::new(sub_titles)
        .block(Block::default().borders(Borders::ALL).title(" 编译器多阶段内部探测器 (按 Tab / 左右键切换) "))
        .select(app.classic_subtab)
        .style(Style::default().fg(Color::DarkGray))
        .highlight_style(Style::default().fg(Color::Green).add_modifier(Modifier::BOLD));
    f.render_widget(sub_tabs, r_chunks[0]);

    // 内容区
    let content_text = if let Some(ref resp) = app.classic_result {
        match app.classic_subtab {
            0 => {
                let mut lines = Vec::new();
                lines.push(Line::from(vec![Span::styled(
                    format!("● 词法记号识别总数: {} 个\n", resp.tokens.len()),
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                )]));
                for (i, tok) in resp.tokens.iter().enumerate() {
                    lines.push(Line::from(vec![
                        Span::styled(format!("[{:03}] ", i), Style::default().fg(Color::DarkGray)),
                        Span::styled(format!("{:?} ", tok.kind), Style::default().fg(Color::Cyan)),
                        Span::styled(format!("\"{}\" ", tok.raw), Style::default().fg(Color::White)),
                        Span::styled(format!("@ L{}:C{}", tok.span.line, tok.span.col), Style::default().fg(Color::Gray)),
                    ]));
                }
                Text::from(lines)
            }
            1 => {
                let ast = resp.ast_json.clone().unwrap_or_else(|| "// AST 未生成".to_string());
                Text::raw(ast)
            }
            2 => {
                let mut text = String::new();
                if let Some(ref raw) = resp.raw_ir {
                    text.push_str("=== 优化前原始三地址码 (Raw TAC / SSA IR) ===\n");
                    text.push_str(raw);
                    text.push_str("\n\n");
                }
                if let Some(ref opt) = resp.optimized_ir {
                    text.push_str(&format!("=== 经过 {:?} 优化的三地址码 (Optimized IR) ===\n", app.opt_level));
                    text.push_str(opt);
                }
                Text::raw(text)
            }
            3 => {
                let asm = resp.assembly_x86.clone().unwrap_or_else(|| "// 目标字节码未生成".to_string());
                Text::raw(asm)
            }
            4 => {
                let mut lines = Vec::new();
                if resp.success {
                    lines.push(Line::from(vec![Span::styled(
                        "✔ 编译执行成功 (Execution Finished)",
                        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
                    )]));
                    lines.push(Line::from(vec![
                        Span::styled("虚拟机返回值: ", Style::default().fg(Color::White)),
                        Span::styled(format!("{:?}", resp.vm_return_val), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                        Span::styled(format!("  (累计执行指令: {} 条)", resp.instructions_executed), Style::default().fg(Color::DarkGray)),
                    ]));
                    lines.push(Line::from(""));
                    lines.push(Line::from(Span::styled("【标准输出 (VM Stdout)】", Style::default().fg(Color::Cyan))));
                    if resp.vm_stdout.is_empty() {
                        lines.push(Line::from(Span::styled("(无标准输出)", Style::default().fg(Color::DarkGray))));
                    } else {
                        for out in &resp.vm_stdout {
                            lines.push(Line::from(Span::styled(format!("> {}", out), Style::default().fg(Color::Green))));
                        }
                    }
                } else {
                    lines.push(Line::from(vec![Span::styled(
                        "✘ 编译诊断报告 (Rustc-style Diagnostic)",
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    )]));
                    if let Some(ref err) = resp.error {
                        lines.push(Line::from(Span::styled(err.clone(), Style::default().fg(Color::Red))));
                    }
                }
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled("【编译流水线执行日志 (Pipeline Logs)】", Style::default().fg(Color::DarkGray))));
                for log in &resp.logs {
                    lines.push(Line::from(Span::styled(format!("• {}", log), Style::default().fg(Color::DarkGray))));
                }
                Text::from(lines)
            }
            _ => Text::raw("未知子标签"),
        }
    } else {
        Text::raw("请按 F5 或 Enter 开始编译...")
    };

    let inspector = Paragraph::new(content_text)
        .block(Block::default().borders(Borders::ALL).title(" 探测工件视图 "))
        .wrap(Wrap { trim: false });
    f.render_widget(inspector, r_chunks[1]);
}

fn render_decompile_tab(f: &mut ratatui::Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
        .split(area);

    let sample_name = SAMPLES_DECOMPILE[app.decompile_sample_idx].0;
    let in_title = format!(" 输入大厂黑话/招聘JD/战略汇报文案 [{}] (按 S 切换用例，按 Enter 反编译) ", sample_name);
    let in_box = Paragraph::new(app.decompile_input.as_str())
        .block(Block::default().borders(Borders::ALL).title(in_title))
        .style(Style::default().fg(Color::White))
        .wrap(Wrap { trim: true });
    f.render_widget(in_box, chunks[0]);

    let res_text = if let Some(ref res) = app.decompile_result {
        let mut lines = Vec::new();
        lines.push(Line::from(vec![Span::styled(
            "【识别出的大厂包装套路与词汇属性】",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )]));
        if res.extracted_key_aspects.is_empty() {
            lines.push(Line::from("• 未识别出典型包装词汇，属于普通日常文本。"));
        } else {
            for asp in &res.extracted_key_aspects {
                lines.push(Line::from(vec![
                    Span::styled("• ", Style::default().fg(Color::Yellow)),
                    Span::styled(asp, Style::default().fg(Color::White)),
                ]));
            }
        }
        lines.push(Line::from(""));
        lines.push(Line::from(vec![Span::styled(
            "【脱水后的大白话残酷真相 (Brutal Plain Truth)】",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )]));
        lines.push(Line::from(vec![Span::styled(
            &res.brutal_truth,
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
        )]));
        Text::from(lines)
    } else {
        Text::raw("正在等待反编译分析...")
    };

    let out_box = Paragraph::new(res_text)
        .block(Block::default().borders(Borders::ALL).title(" 照妖镜反编译脱水深度报告 "))
        .style(Style::default())
        .wrap(Wrap { trim: false });
    f.render_widget(out_box, chunks[1]);
}

fn render_theory_tab(f: &mut ratatui::Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(38), Constraint::Min(20)])
        .split(area);

    // 左侧: 8大专篇目录树
    let items: Vec<ListItem> = THEORY_TITLES
        .iter()
        .enumerate()
        .map(|(i, title)| {
            let style = if i == app.theory_chapter_idx {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };
            ListItem::new(format!("{:02}. {}", i + 1, title)).style(style)
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(" 龙书教材目录 (按 Tab/数字键切换) "));
    f.render_widget(list, chunks[0]);

    // 右侧: 学术专篇正文阅读器
    let text = THEORY_TEXTS[app.theory_chapter_idx];
    let title = format!(" 正文阅读: {} (按方向键上下翻阅) ", THEORY_TITLES[app.theory_chapter_idx]);
    let reader = Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL).title(title))
        .style(Style::default().fg(Color::White))
        .wrap(Wrap { trim: false })
        .scroll((app.theory_scroll, 0));
    f.render_widget(reader, chunks[1]);
}
