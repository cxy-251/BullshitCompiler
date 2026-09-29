# 011: 经典编译原理知识全景图谱与理论深潜方案

> **状态**: 已规划并在 Web 交互终端完成万字知识体系全面上线  
> **学术基准**: Aho, Lam, Sethi, Ullman《Compilers: Principles, Techniques, and Tools》(龙书) / Appel《Modern Compiler Implementation》(虎书) / Cooper & Torczon《Engineering a Compiler》  
> **文档定位**: 彻底去除所有非核心业务修饰，纯粹聚焦编译原理正规学术体系，系统性阐述从词法、语法、语义、SSA-IR、数据流优化到目标代码生成与物理寄存器分配的全套理论推导、数学证明与经典算法。

---

## 阶段一：词法分析 (Lexical Analysis)

词法分析器（扫描器，Scanner）是编译流水线的前哨站。其形式使命是将源程序无结构的**字符流（Character Stream）**投影为规整的**记号流（Token Stream）**：
$$\Sigma^* \xrightarrow{\text{Lexer}} \langle \text{TokenKind}, \text{AttributeValue}, \text{Span} \rangle^*$$

### 1. 形式语言与乔姆斯基谱系 (Chomsky Hierarchy)
词法分析的理论基石是 **3 型文法（正规文法，Regular Grammar）**：
- 产生式形式受限于 $A \to aB$ 或 $A \to a$（右线性正规文法）；
- 表达能力等价于**正规表达式（Regular Expression, RE）**与**有限自动机（Finite Automata, FA）**；
- 无法处理括号任意嵌套匹配（需要 2 型上下文无关文法）。

### 2. 算子优先级与代数运算律
正规表达式基于字母表 $\Sigma$，满足下列代数性质：
- **选择（Union）**: $r|s$，满足交换律 $r|s = s|r$ 与结合律；
- **连接（Concatenation）**: $rs$，不满足交换律，满足结合律；
- **克林闭包（Kleene Closure）**: $r^*$，表示 0 次或多次重复，$r^* = \epsilon | r | rr | \dots$；
- **正闭包（Positive Closure）**: $r^+ = rr^*$。

### 3. Thompson 构造算法：RE $\to$ NFA
为每个正规式归纳构造含 $\epsilon$-转移的非确定有限自动机（NFA）：
- **基础原子**: 
  - 匹配空字 $\epsilon$: 状态 $s_0 \xrightarrow{\epsilon} s_1$；
  - 匹配符号 $a \in \Sigma$: 状态 $s_0 \xrightarrow{a} s_1$。
- **选择复合 $r|s$**: 引入新初态 $s_0$ 通过 $\epsilon$ 分别指向 $N(r)$ 与 $N(s)$ 初态；引入新终态 $s_f$，$N(r)$ 与 $N(s)$ 的终态通过 $\epsilon$ 汇聚到 $s_f$。
- **连接复合 $rs$**: 将 $N(r)$ 的终态与 $N(s)$ 的初态合并。
- **闭包复合 $r^*$**: 引入新初态与终态，增加绕过 $N(r)$ 的 $\epsilon$ 边，增加终态回跳初态的 $\epsilon$ 边。

### 4. 子集构造法 (Subset Construction)：NFA $\to$ DFA
NFA 的不确定性使得匹配需要回溯，子集构造法通过消除 $\epsilon$-转移将 NFA 状态集合映射为 DFA 的单个状态：
1. **$\epsilon$-闭包计算**:
   $$\epsilon\text{-closure}(s) = \{s\} \cup \{t \mid \exists u \in \epsilon\text{-closure}(s), u \xrightarrow{\epsilon} t\}$$
2. **状态转移计算**:
   $$\text{move}(T, a) = \{t \mid \exists s \in T, s \xrightarrow{a} t\}$$
   $$Dtran[T, a] = \epsilon\text{-closure}(\text{move}(T, a))$$
3. **算法迭代**: 从 $D_0 = \epsilon\text{-closure}(s_0)$ 出发，若新状态未标记，重复计算各个符号的转移子集，直至状态集不再膨胀。

### 5. Hopcroft 状态最小化算法：DFA 最小化
计算拥有最少状态数的等价确定有限自动机：
1. **初始划分**: 将所有状态划分为终态集合 $F$ 与非终态集合 $S - F$：$P = \{F, S - F\}$；
2. **划分细化**: 对于 $P$ 中的每个组 $C$ 和输入符号 $a$：若 $C$ 中状态在输入 $a$ 下转移到的目标分属不同组，则将 $C$ 分裂为 $C_1, C_2$；
3. **收敛终止**: 当所有划分组均不可再分裂时，将每组状态合并为一个超节点。

### 6. 工业级工程实现要点
- **双缓冲区与哨兵机制（Buffer Pair & Sentinels）**: 采用两个 4KB/8KB 互补缓冲区配合 `lexemeBegin` 与 `forward` 双指针；以 `EOF` 作为哨兵字符，使得内层循环只需一次比较即可判断越界与字符内容。
- **最长匹配原则（Maximal Munch / Longest Match）**: 遇到前缀冲突（如 `>` 与 `>=`、标识符 `if_var` 与关键字 `if`）时，贪婪扫描至无法匹配为止，以最长的前缀构成 Token。
- **冲突消解**: 关键字表通常先作为普通标识符扫描，随后通过极速哈希表（Perfect Hash / `phf`）判定是否提升为保留字 Token。

---

## 阶段二：语法分析 (Syntax Analysis)

语法分析器（Parser）负责将 Token 流转化为表达语法树形层次结构的**抽象语法树（AST）**。

### 1. 上下文无关文法 (CFG) 形式定义
文法 $G$ 定义为四元组：
$$G = (V_N, V_T, P, S)$$
- $V_N$: 非终结符有限集（Non-terminals）；
- $V_T$: 终结符有限集（Terminals / Tokens），$V_N \cap V_T = \emptyset$；
- $P$: 产生式集合，形式为 $A \to \alpha$，其中 $A \in V_N, \alpha \in (V_N \cup V_T)^*$；
- $S \in V_N$: 开始符号（Start Symbol）。

### 2. 文法二义性消除
文法若能对同一个句子产生两棵不同的语法分析树，则该文法是**二义的（Ambiguous）**。
- **算符优先级与结合性消除法**:
  将表达式文法改写为多层产生式：
  $$E \to E + T \mid T$$
  $$T \to T * F \mid F$$
  $$F \to (E) \mid \mathbf{id}$$
  越靠近叶子节点的产生式（如 $T * F$）拥有更高的运算符优先级；左递归产生式引入左结合。
- **悬空 else（Dangling Else）问题**:
  文法规定 `else` 与最近尚未匹配的 `if` 闭合配对。

### 3. 自顶向下：LL(1) 分析法与预测分析表
LL(1) 即：从左至右扫描（Left-to-right）、最左推导（Leftmost derivation）、向前看 1 个 Token（1-token lookahead）。

#### (1) FIRST 集合与 FOLLOW 集合计算
- **$\text{FIRST}(\alpha)$**: 符号串 $\alpha$ 能推导出的首终结符集合。
  - 若 $X \in V_T$，则 $\text{FIRST}(X) = \{X\}$；
  - 若 $X \to \epsilon$，则 $\epsilon \in \text{FIRST}(X)$；
  - 若 $X \to Y_1 Y_2 \dots Y_k$，若 $Y_1 \dots Y_{i-1} \Rightarrow^* \epsilon$，则 $\text{FIRST}(Y_i) \setminus \{\epsilon\} \subseteq \text{FIRST}(X)$。
- **$\text{FOLLOW}(A)$**: 在推导过程中紧跟非终结符 $A$ 之后的终结符集合。
  - 将输入结束符 $\$$ 放入 $\text{FOLLOW}(S)$；
  - 若有产生式 $A \to \alpha B \beta$，则 $\text{FIRST}(\beta) \setminus \{\epsilon\} \subseteq \text{FOLLOW}(B)$；
  - 若 $A \to \alpha B$ 或 $A \to \alpha B \beta$ 且 $\beta \Rightarrow^* \epsilon$，则 $\text{FOLLOW}(A) \subseteq \text{FOLLOW}(B)$。

#### (2) SELECT 集合与 LL(1) 判定条件
对于产生式 $A \to \alpha$：
$$\text{SELECT}(A \to \alpha) = \begin{cases} \text{FIRST}(\alpha), & \epsilon \notin \text{FIRST}(\alpha) \\ (\text{FIRST}(\alpha) \setminus \{\epsilon\}) \cup \text{FOLLOW}(A), & \epsilon \in \text{FIRST}(\alpha) \end{cases}$$
**LL(1) 文法充要条件**: 对于同一非终结符 $A$ 的任意两个不同产生式 $A \to \alpha$ 与 $A \to \beta$，必有：
$$\text{SELECT}(A \to \alpha) \cap \text{SELECT}(A \to \beta) = \emptyset$$

### 4. 自底向上：LR 族分析法
LR 即：从左至右扫描、最右推导的逆过程（规范规约，Rightmost derivation in reverse）。

```
        LR 族分析技术谱系与表达力递增关系
┌────────┐      ┌────────┐      ┌────────┐      ┌────────┐
│ LR(0)  │ ───► │ SLR(1) │ ───► │LALR(1) │ ───► │ LR(1)  │
└────────┘      └────────┘      └────────┘      └────────┘
表达力最弱       借助 FOLLOW     合并同心集       表达力最强
无向前看符号    简单冲突消解    工业标准(Yacc)   状态数极其庞大
```

- **LR(0) 项目**: 产生式右部带有圆点的形式，如 $A \to \alpha \cdot \beta$。
  - 移进项目: $A \to \alpha \cdot a \beta$；
  - 待约项目: $A \to \alpha \cdot B \beta$；
  - 规约项目: $A \to \alpha \cdot$。
- **项目集闭包（CLOSURE）与 GOTO 转移**:
  - 活前缀（Viable Prefix）识别自动机。
- **冲突类型**:
  - **移进-规约冲突（Shift-Reduce Conflict）**: 状态中同时包含移进项目与规约项目；
  - **规约-规约冲突（Reduce-Reduce Conflict）**: 状态中同时包含两个不同产生式的规约项目。
- **SLR(1)**: 规约项目 $A \to \alpha \cdot$ 仅当输入字符 $a \in \text{FOLLOW}(A)$ 时才触发规约。
- **LR(1)**: 项目扩充为包含显式搜索符（Lookahead）的二元组 $[A \to \alpha \cdot \beta, a]$，消除伪冲突。
- **LALR(1)**: 合并核心相同（Core-identical）的 LR(1) 状态，状态数骤降至 LR(0) 级别，但偶发性可能引入原本不存在的规约-规约冲突。

### 5. Pratt 解析法（优先级爬升，Precedence Climbing）
现代编译系统（如 Rustc、V8）在解析中缀表达式时放弃传统的深层递归文法，采用 Pratt Parser：
- 为每个操作符赋予**结合力（Binding Power）**：`(left_bp, right_bp)`；
- 算法核心：`parse_expr(min_bp)` 循环比较当前操作符的 `left_bp` 是否大于当前上下文的 `min_bp`，在单一循环中优雅完成结合律与优先级的 AST 嵌套构造。

---

## 阶段三：语义分析与类型理论 (Semantic Analysis & Type Theory)

语法分析构建了程序的抽象骨架，语义分析负责为骨架赋予严密的类型环境与名字绑定，杜绝“语法合法但荒谬”的代码。

### 1. 语法制导翻译 (SDT) 与属性文法 (Attribute Grammars)
- **综合属性（Synthesized Attributes）**: 节点的值完全由其子节点的属性计算而来（自底向上计算）。仅含综合属性的文法称为 **S-属性定义**。
- **继承属性（Inherited Attributes）**: 节点的值依赖于其父节点或兄弟节点的属性（自顶向下/横向流动）。
- **L-属性定义**: 属性依赖图不存在有向环，且继承属性只依赖于左侧兄弟节点与父节点，允许在单趟遍历中求值。

### 2. 作用域树与符号表 (Symbol Table & Lexical Scoping)
- **环境链模型（Environment Chain）**:
  每个局部代码块拥有独立符号表帧 `Frame`，维护 `parent: Option<Arc<Scope>>` 指针；
- **操作原语**:
  - `enter_scope()`: 压栈新建当前作用域；
  - `exit_scope()`: 弹出当前作用域；
  - `define_symbol(name, sym)`: 当前作用域插入符号，检测同作用域重复定义；
  - `lookup_symbol(name)`: 递归沿父指针向上回溯，首次命中即返回（变量遮蔽机制，Shadowing）。

### 3. 形式化静态类型推导体系
类型系统以数学判断式（Judgment）形式表示：
$$\Gamma \vdash e : \tau$$
表示在符号类型上下文 $\Gamma$ 下，表达式 $e$ 拥有类型 $\tau$。

#### 核心类型规则推导范式
- **变量规则 (Var)**:
  $$\frac{x : \tau \in \Gamma}{\Gamma \vdash x : \tau}$$
- **加法算术规则 (Add)**:
  $$\frac{\Gamma \vdash e_1 : \mathbf{Int} \quad \Gamma \vdash e_2 : \mathbf{Int}}{\Gamma \vdash e_1 + e_2 : \mathbf{Int}}$$
- **条件分支规则 (If)**:
  $$\frac{\Gamma \vdash e_c : \mathbf{Bool} \quad \Gamma \vdash e_1 : \tau \quad \Gamma \vdash e_2 : \tau}{\Gamma \vdash \mathbf{if}\; e_c \;\{e_1\}\;\mathbf{else}\;\{e_2\} : \tau}$$

### 4. Hindley-Milner (HM) 类型推导与合一算法 (Unification)
- **类型变量与类型替换**: $\tau = \alpha \mid \mathbf{Int} \mid \tau_1 \to \tau_2$；
- **Robinson 合一算法**: 给定两类型表达式 $\tau_1, \tau_2$，寻找最一般合一子（Most General Unifier, MGU）$\theta$，使得 $\theta(\tau_1) = \theta(\tau_2)$；
- **Occurs Check**: 防止构造出无限递归类型（如 $\alpha = \alpha \to \mathbf{Int}$）。

### 5. 类型健全性定理 (Type Safety)
Milner 经典定则：*Well-typed programs cannot go wrong*。
- **进度定理（Progress）**: 若表达式 $e$ 类型良好（$\vdash e : \tau$），则 $e$ 要么本身是一个值，要么可以按照求值步进一步化简（$e \to e'$）；
- **保全定理（Preservation / Subject Reduction）**: 若 $\vdash e : \tau$ 且 $e \to e'$，则必有 $\vdash e' : \tau$。

---

## 阶段四：中间表示与 SSA 形式 (Intermediate Representation & SSA)

编译器中端（Middle-end）脱离具体源语言与目标硬件，在独立抽象的**中间表示（IR）**上实施变换。

### 1. 中间表示体系谱系
- **AST（抽象语法树）**: 树形结构，保留语言语法特性，适合语义检查与宏展开；
- **线性 3-地址码 (3AC / Quadruples)**: 扁平化指令序列，每条指令至多包含 2 个操作数和 1 个目的寄存器：$x = y \;\mathbf{op}\; z$；
- **控制流图 (Control Flow Graph, CFG)**: 节点为基本块（Basic Block），边表示分支跳转与转移。

### 2. 静态单赋值形式 (SSA Form)
在 SSA 形式中，**程序中每一个变量有且仅有一次定值赋值**：
- 消除别名干扰，使每个使用点（Use）严格对应唯一的定值点（Def）；
- 显式构建 Use-Def 链与 Def-Use 链。

### 3. 支配理论与 Cytron 算法
为在控制流汇聚处合并变量的不同定义版本，需引入 $\Phi$ (Phi) 节点。

#### (1) 支配节点 (Dominance)
- 若从入口基本块到达节点 $n$ 的所有路径均经过节点 $d$，则称 $d$ **支配（Dominates）** $n$，记作 $d \;\mathbf{dom}\; n$；
- **严格支配 (Strict Dominance)**: $d \;\mathbf{sdom}\; n \iff d \;\mathbf{dom}\; n \land d \neq n$；
- **直接支配者 (Immediate Dominator, IDOM)**: 严格支配 $n$ 的所有节点中距离 $n$ 最近的节点。

#### (2) 支配边界 (Dominance Frontier, DF)
节点 $X$ 的支配边界 $DF(X)$ 定义为：$X$ 支配了其前驱节点，但并不严格支配该节点本身：
$$DF(X) = \{ Y \mid \exists P \in \text{Pred}(Y), X \;\mathbf{dom}\; P \land \neg (X \;\mathbf{sdom}\; Y) \}$$

#### (3) $\Phi$ 函数放置判据 (Cytron et al.)
若基本块 $B$ 中对变量 $v$ 进行了定值，则必须在 $B$ 的**迭代支配边界（Iterated Dominance Frontier, $DF^+$）**中的每个基本块头部插入该变量的 $\Phi$ 节点：
$$\Phi(v) \text{ placed at } DF^+(S_v), \quad \text{where } S_v = \{ B \mid v \in \text{Def}(B) \}$$

### 4. SSA 析构 (Out-of-SSA / De-SSA)
物理硬件不支持 $\Phi$ 指令，代码发射前需进行 De-SSA：
- **关键边（Critical Edge）**: 从拥有多个后继的块指向拥有多个前驱的块的控制流边；
- **关键边分割（Critical Edge Splitting）**: 在关键边中插入合成空块，并在该块中发射移动（Copy）指令，防止出现“Lost-copy”与“Swap”死锁问题。

---

## 阶段五：机器无关代码优化 (Optimization Passes)

代码优化旨在在**严格保持程序语义不变（Preserve Semantics）**的前提下，减小运行时钟周期数或代码体积。

### 1. 数据流分析半格理论 (Semilattice Framework)
数据流分析是大部分优化的数学基石：
- **半格 (Meet-Semilattice)**: 三元组 $\langle L, \sqcap, \sqsubseteq \rangle$，满足幂等律、交换律、结合律；
- **偏序关系 $\sqsubseteq$**: $a \sqsubseteq b \iff a \sqcap b = a$；
- **极值点**: $\top$（Top，代表完全无信息/未执行），$\bot$（Bottom，代表过约束/未知）；
- **转移函数 $f_B$**: 单调函数，$x \sqsubseteq y \implies f_B(x) \sqsubseteq f_B(y)$；
- **Kildall 不动点迭代算法**: 在有限高度半格中，迭代计算 $\text{IN}[B]$ 与 $\text{OUT}[B]$，必在有限步内收敛于**最大不动点（Maximal Fixed Point, MFP）**。

### 2. 经典数据流方程一览表

| 分析问题 | 方向 | 交运算 ($\sqcap$) | 边界条件 | 传递方程 | 典型优化应用 |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **到达定值 (Reaching Defs)** | 前向 | 并集 ($\cup$) | $\text{OUT}[\text{entry}] = \emptyset$ | $\text{OUT}[B] = \text{Gen}[B] \cup (\text{IN}[B] - \text{Kill}[B])$ | Use-Def 链、死代码消除 |
| **可用表达式 (Available Exprs)** | 前向 | 交集 ($\cap$) | $\text{OUT}[\text{entry}] = \emptyset$ | $\text{OUT}[B] = \text{Gen}[B] \cup (\text{IN}[B] - \text{Kill}[B])$ | 公共子表达式消除 (CSE) |
| **活跃变量 (Live Variables)** | 后向 | 并集 ($\cup$) | $\text{IN}[\text{exit}] = \emptyset$ | $\text{IN}[B] = \text{Def}[B] \cup (\text{OUT}[B] - \text{Use}[B])$ | 死代码消除、寄存器分配 |

### 3. 核心优化 Pass 机制
- **稀疏有条件常量传播 (SCCP)**:
  利用 SSA 图将常量值与控制流可达性双网联立，一次性识别死分支并折叠级联常量；
- **全局值编号 (GVN)**:
  给程序中的每个计算值分配代数等价类编号，识别跨基本块的全局冗余计算；
- **循环不变量外提 (LICM)**:
  求解循环自然循环（Natural Loop），分析定值不在循环内变化的操作，将其提升至循环前置首部（Preheader）；
- **死代码消除 (DCE)**:
  基于活跃变量分析，删除未被使用的临时赋值与不可达基本块。

---

## 阶段六：目标代码生成与物理寄存器分配 (Target Codegen & RegAlloc)

将抽象中间表示转换为可在目标物理微架构上高效运转的机器指令。

### 1. 指令选择 (Instruction Selection)
将高层树形 IR 匹配为目标机指令集（ISA）模式：
- **最大咀嚼树覆盖（Maximal Munch）**: 贪婪算法，自顶向下挑选能覆盖当前 IR 树最大子树且代价最低的目标指令；
- **动态规划代码生成（Burg / DP Code Gen）**: 自底向上计算每个树节点的最佳覆盖成本，实现全局时钟周期代价最优。

### 2. 物理寄存器分配 (Register Allocation)
真实机器的物理寄存器极其有限（如 x86-64 仅 16 个通用寄存器），必须将成百上千的虚拟寄存器合理映射到物理寄存器。

#### (1) 线性扫描算法 (Linear Scan, Poletto & Sarkar)
- 基于变量生命周期区间（Live Intervals）按起始位置排序；
- 遇到新区间时分配空闲物理寄存器；若寄存器枯竭，挑选结束时间最远的区间溢出（Spill）到内存栈帧；
- 算法复杂度 $O(N)$，广泛用于 JIT 编译器（如 Java HotSpot Client, V8）。

#### (2) Chaitin-Briggs 图着色分配法 (Graph Coloring)
- **冲突图 (Interference Graph) 构建**: 节点为虚拟变量，若两个变量生命周期重叠，则在它们之间连一条无向冲突边；
- **$K$-着色问题**: 若冲突图能用 $K$ 种颜色（$K$ 为物理寄存器数）着色且相邻节点颜色不同，则分配成功；
- **Kempe 启发式算法流程**:
  1. **Simplify（简化）**: 寻找度数 $< K$ 的节点从图中移出并压入栈；
  2. **Spill（溢出候选）**: 若所有节点度数 $\ge K$，根据循环嵌套深度与使用频率挑选成本最小的节点作为溢出候选；
  3. **Select（着色选择）**: 节点逐个出栈，选取与其邻居不冲突的可用物理寄存器颜色。

### 3. 现代 ABI 与调用约定 (System V AMD64 ABI)
- **参数传递寄存器序列**:
  前 6 个整型/指针参数依次使用 `%rdi, %rsi, %rdx, %rcx, %r8, %r9`，超出部分逆序压栈；
- **返回值寄存器**:
  64 位结果放入 `%rax`；
- **寄存器保存策略**:
  - **Callee-saved（被调用者保存）**: `%rbx, %rsp, %rbp, %r12, %r13, %r14, %r15`（函数返回前必须恢复原值）；
  - **Caller-saved（调用者保存）**: 其余寄存器（调用者跨函数调用需自行备份）。
- **16 字节栈对齐要求**:
  执行 `call` 指令前，栈指针 `%rsp` 必须满足 `(rsp + 8) % 16 == 0`，以确保进入函数后 `%rsp` 为 16 字节对齐（满足 SSE/AVX 向量指令要求）。
- **红区 (Red Zone)**:
  `%rsp` 下方的 128 字节区域，叶子函数无需移动 `%rsp` 即可直接作为局部临时暂存区。

---

## 结论

编译原理的核心魅力在于：**从最底层的自动机状态转移与形式文法，到图论中的支配树与图着色，再到抽象代数中的半格理论与不动点迭代**，全链路展现了计算机科学中严密的数学自洽性与工程极致优化。该知识体系构成了现代软件工业全部基础设施（编译器、虚拟机、静态分析器、代码优化引擎）的理论基石。
