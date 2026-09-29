use bsc_core::{ActionKind, AgentKind, EntityKind, MediumKind, OutcomeKind, PayloadKind, Span};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    Agent(AgentKind),
    Medium(MediumKind),
    Entity(EntityKind),
    Payload(PayloadKind),
    Action(ActionKind),
    Outcome(OutcomeKind),
    Prep(String),      // 用, 通过, 基于
    Punctuation(char), // ， 。 ！
    Text(String),
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub raw: String,
    pub span: Span,
}

pub struct Lexer<'a> {
    input: &'a str,
    chars: Vec<(usize, char)>,
    pos: usize,
    line: usize,
    col: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        let chars: Vec<(usize, char)> = input.char_indices().collect();
        Self {
            input,
            chars,
            pos: 0,
            line: 1,
            col: 1,
        }
    }

    pub fn tokenize(&mut self) -> Vec<Token> {
        let mut tokens = Vec::new();
        while self.pos < self.chars.len() {
            let (idx, ch) = self.chars[self.pos];
            if ch.is_whitespace() {
                if ch == '\n' {
                    self.line += 1;
                    self.col = 1;
                } else {
                    self.col += 1;
                }
                self.pos += 1;
                continue;
            }

            if ch == '，' || ch == '。' || ch == '！' || ch == ',' || ch == '.' || ch == '!' {
                tokens.push(Token {
                    kind: TokenKind::Punctuation(ch),
                    raw: ch.to_string(),
                    span: Span::new(idx, idx + ch.len_utf8(), self.line, self.col),
                });
                self.pos += 1;
                self.col += 1;
                continue;
            }

            // Greedy matching against known synonym dictionaries
            if let Some(token) = self.match_keyword() {
                tokens.push(token);
            } else {
                // Collect contiguous characters as generic text
                let start_idx = idx;
                let start_col = self.col;
                let mut text = String::new();
                while self.pos < self.chars.len() {
                    let (_, c) = self.chars[self.pos];
                    if c.is_whitespace() || c == '，' || c == '。' || c == '！' || c == ',' || c == '.' || c == '!' {
                        break;
                    }
                    if self.is_keyword_ahead() {
                        break;
                    }
                    text.push(c);
                    self.pos += 1;
                    self.col += 1;
                }
                if !text.is_empty() {
                    let end_idx = if self.pos < self.chars.len() {
                        self.chars[self.pos].0
                    } else {
                        self.input.len()
                    };
                    tokens.push(Token {
                        kind: TokenKind::Text(text.clone()),
                        raw: text,
                        span: Span::new(start_idx, end_idx, self.line, start_col),
                    });
                }
            }
        }
        tokens
    }

    fn is_keyword_ahead(&self) -> bool {
        self.try_peek_keyword().is_some()
    }

    fn match_keyword(&mut self) -> Option<Token> {
        if let Some((kind, word, len)) = self.try_peek_keyword() {
            let start_idx = self.chars[self.pos].0;
            let start_col = self.col;
            self.pos += len;
            self.col += len;
            let end_idx = if self.pos < self.chars.len() {
                self.chars[self.pos].0
            } else {
                self.input.len()
            };
            Some(Token {
                kind,
                raw: word.to_string(),
                span: Span::new(start_idx, end_idx, self.line, start_col),
            })
        } else {
            None
        }
    }

    fn try_peek_keyword(&self) -> Option<(TokenKind, &'static str, usize)> {
        let remaining: String = self.chars[self.pos..].iter().map(|(_, c)| *c).collect();

        // Multi-character keywords dictionary
        let table: &[(&str, TokenKind)] = &[
            // Outcome
            ("看了一下没问题", TokenKind::Outcome(OutcomeKind::SuccessNormal)),
            ("看了下没问题", TokenKind::Outcome(OutcomeKind::SuccessNormal)),
            ("没问题了", TokenKind::Outcome(OutcomeKind::SuccessNormal)),
            ("修好了", TokenKind::Outcome(OutcomeKind::SuccessNormal)),
            ("跑通了", TokenKind::Outcome(OutcomeKind::SuccessNormal)),
            ("恢复了", TokenKind::Outcome(OutcomeKind::SuccessNormal)),
            ("搞定了", TokenKind::Outcome(OutcomeKind::SuccessNormal)),
            ("挂了", TokenKind::Outcome(OutcomeKind::FailureError)),
            ("崩了", TokenKind::Outcome(OutcomeKind::FailureError)),
            ("死机了", TokenKind::Outcome(OutcomeKind::FailureError)),
            // Temporal words
            ("昨天晚上", TokenKind::Prep("昨天晚上".into())),
            ("昨天下午", TokenKind::Prep("昨天下午".into())),
            ("昨天上午", TokenKind::Prep("昨天上午".into())),
            ("今天下午", TokenKind::Prep("今天下午".into())),
            ("今天中午", TokenKind::Prep("今天中午".into())),
            ("今天上午", TokenKind::Prep("今天上午".into())),
            ("刚才", TokenKind::Prep("刚才".into())),
            ("上周", TokenKind::Prep("上周".into())),
            ("昨天", TokenKind::Prep("昨天".into())),
            ("今天", TokenKind::Prep("今天".into())),
            ("明天", TokenKind::Prep("明天".into())),
            ("下午", TokenKind::Prep("下午".into())),
            ("上午", TokenKind::Prep("上午".into())),
            ("中午", TokenKind::Prep("中午".into())),
            ("晚上", TokenKind::Prep("晚上".into())),
            ("刚刚", TokenKind::Prep("刚刚".into())),
            // Quantifiers / Preps
            ("一杯", TokenKind::Prep("一杯".into())),
            ("一个", TokenKind::Prep("一个".into())),
            ("一条", TokenKind::Prep("一条".into())),
            ("一份", TokenKind::Prep("一份".into())),
            ("两杯", TokenKind::Prep("两杯".into())),
            ("两个", TokenKind::Prep("两个".into())),
            ("通过", TokenKind::Prep("通过".into())),
            ("基于", TokenKind::Prep("基于".into())),
            ("使用", TokenKind::Prep("使用".into())),
            ("用", TokenKind::Prep("用".into())),
            // Agent
            ("我们研发团队", TokenKind::Agent(AgentKind::Organization)),
            ("研发团队", TokenKind::Agent(AgentKind::Organization)),
            ("研发组", TokenKind::Agent(AgentKind::Organization)),
            ("工程师", TokenKind::Agent(AgentKind::ThirdPerson)),
            ("小明", TokenKind::Agent(AgentKind::ThirdPerson)),
            ("我们", TokenKind::Agent(AgentKind::FirstPersonPlural)),
            ("大家", TokenKind::Agent(AgentKind::FirstPersonPlural)),
            ("团队", TokenKind::Agent(AgentKind::Organization)),
            ("他们", TokenKind::Agent(AgentKind::ThirdPerson)),
            ("我", TokenKind::Agent(AgentKind::FirstPersonSingular)),
            // Medium
            ("蓝牙4.0", TokenKind::Medium(MediumKind::Bluetooth)),
            ("蓝牙", TokenKind::Medium(MediumKind::Bluetooth)),
            ("BLE", TokenKind::Medium(MediumKind::Bluetooth)),
            ("wifi", TokenKind::Medium(MediumKind::WiFi)),
            ("WiFi", TokenKind::Medium(MediumKind::WiFi)),
            ("无线网", TokenKind::Medium(MediumKind::WiFi)),
            ("串口", TokenKind::Medium(MediumKind::SerialPort)),
            ("UART", TokenKind::Medium(MediumKind::SerialPort)),
            ("网线", TokenKind::Medium(MediumKind::Ethernet)),
            ("以太网", TokenKind::Medium(MediumKind::Ethernet)),
            // Entity
            ("开发板", TokenKind::Entity(EntityKind::EmbeddedBoard)),
            ("单片机", TokenKind::Entity(EntityKind::EmbeddedBoard)),
            ("板子", TokenKind::Entity(EntityKind::EmbeddedBoard)),
            ("RK3588", TokenKind::Entity(EntityKind::EmbeddedBoard)),
            ("树莓派", TokenKind::Entity(EntityKind::EmbeddedBoard)),
            ("服务器", TokenKind::Entity(EntityKind::ServerCluster)),
            ("集群", TokenKind::Entity(EntityKind::ServerCluster)),
            ("数据库", TokenKind::Entity(EntityKind::Database)),
            ("电脑", TokenKind::Entity(EntityKind::TerminalClient)),
            ("读卡器", TokenKind::Entity(EntityKind::PeripheralDevice)),
            ("打印机", TokenKind::Entity(EntityKind::PeripheralDevice)),
            ("传感器", TokenKind::Entity(EntityKind::PeripheralDevice)),
            // Food & Lifestyle Payloads
            ("红烧肉", TokenKind::Payload(PayloadKind::Custom("红烧肉".into()))),
            ("排骨", TokenKind::Payload(PayloadKind::Custom("排骨".into()))),
            ("炸鸡", TokenKind::Payload(PayloadKind::Custom("炸鸡".into()))),
            ("牛肉", TokenKind::Payload(PayloadKind::Custom("牛肉".into()))),
            ("汉堡", TokenKind::Payload(PayloadKind::Custom("汉堡".into()))),
            ("午饭", TokenKind::Payload(PayloadKind::Custom("午饭".into()))),
            ("晚饭", TokenKind::Payload(PayloadKind::Custom("晚饭".into()))),
            ("美式咖啡", TokenKind::Payload(PayloadKind::Custom("美式咖啡".into()))),
            ("生椰拿铁", TokenKind::Payload(PayloadKind::Custom("生椰拿铁".into()))),
            ("拿铁", TokenKind::Payload(PayloadKind::Custom("拿铁".into()))),
            ("咖啡", TokenKind::Payload(PayloadKind::Custom("咖啡".into()))),
            ("奶茶", TokenKind::Payload(PayloadKind::Custom("奶茶".into()))),
            // Code & Algorithmic Payloads
            ("冒泡排序", TokenKind::Payload(PayloadKind::Custom("冒泡排序".into()))),
            ("死循环bug", TokenKind::Payload(PayloadKind::Custom("死循环bug".into()))),
            ("死循环", TokenKind::Payload(PayloadKind::Custom("死循环".into()))),
            ("内存泄漏", TokenKind::Payload(PayloadKind::Custom("内存泄漏".into()))),
            ("死锁", TokenKind::Payload(PayloadKind::Custom("死锁".into()))),
            ("bug", TokenKind::Payload(PayloadKind::Custom("bug".into()))),
            ("漏洞", TokenKind::Payload(PayloadKind::Custom("漏洞".into()))),
            ("照片", TokenKind::Payload(PayloadKind::Image)),
            ("图片", TokenKind::Payload(PayloadKind::Image)),
            ("文件", TokenKind::Payload(PayloadKind::Document)),
            ("文档", TokenKind::Payload(PayloadKind::Document)),
            ("日志", TokenKind::Payload(PayloadKind::LogStream)),
            ("固件", TokenKind::Payload(PayloadKind::BinaryCode)),
            ("代码", TokenKind::Payload(PayloadKind::BinaryCode)),
            ("需求对齐会", TokenKind::Payload(PayloadKind::Custom("需求对齐会".into()))),
            ("项目评审会", TokenKind::Payload(PayloadKind::Custom("项目评审会".into()))),
            ("周报", TokenKind::Payload(PayloadKind::Custom("周报".into()))),
            // Actions
            ("吃了一顿", TokenKind::Action(ActionKind::Process)),
            ("吃了", TokenKind::Action(ActionKind::Process)),
            ("喝了", TokenKind::Action(ActionKind::Process)),
            ("买了", TokenKind::Action(ActionKind::Process)),
            ("编写了", TokenKind::Action(ActionKind::Process)),
            ("写了", TokenKind::Action(ActionKind::Process)),
            ("写出", TokenKind::Action(ActionKind::Process)),
            ("开发了", TokenKind::Action(ActionKind::Process)),
            ("部署了", TokenKind::Action(ActionKind::Process)),
            ("测试了", TokenKind::Action(ActionKind::Verify)),
            ("排查了", TokenKind::Action(ActionKind::Verify)),
            ("检查了", TokenKind::Action(ActionKind::Verify)),
            ("重构了", TokenKind::Action(ActionKind::Process)),
            ("重启了一下", TokenKind::Action(ActionKind::Restart)),
            ("重启了", TokenKind::Action(ActionKind::Restart)),
            ("重启", TokenKind::Action(ActionKind::Restart)),
            ("修好了", TokenKind::Action(ActionKind::Process)),
            ("修复了", TokenKind::Action(ActionKind::Process)),
            ("修改了", TokenKind::Action(ActionKind::Process)),
            ("连上了", TokenKind::Action(ActionKind::Connect)),
            ("连上", TokenKind::Action(ActionKind::Connect)),
            ("连接", TokenKind::Action(ActionKind::Connect)),
            ("接入", TokenKind::Action(ActionKind::Connect)),
            ("传给了", TokenKind::Action(ActionKind::Transfer)),
            ("传给", TokenKind::Action(ActionKind::Transfer)),
            ("传了", TokenKind::Action(ActionKind::Transfer)),
            ("发送", TokenKind::Action(ActionKind::Transfer)),
            ("同步", TokenKind::Action(ActionKind::Transfer)),
            ("传", TokenKind::Action(ActionKind::Transfer)),
            ("开了", TokenKind::Action(ActionKind::Process)),
            ("吃", TokenKind::Action(ActionKind::Process)),
            ("喝", TokenKind::Action(ActionKind::Process)),
            ("买", TokenKind::Action(ActionKind::Process)),
            ("写", TokenKind::Action(ActionKind::Process)),
            ("修", TokenKind::Action(ActionKind::Process)),
        ];

        for (kw, kind) in table {
            if remaining.starts_with(kw) {
                return Some((kind.clone(), kw, kw.chars().count()));
            }
        }
        None
    }
}
