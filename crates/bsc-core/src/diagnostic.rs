use std::fmt::Write as _;

use crate::span::{Span, display_width, line_col};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Note,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Error => "错误",
            Severity::Warning => "警告",
            Severity::Note => "提示",
        }
    }
}

/// 指向源码某处的一条标注。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Label {
    pub span: Span,
    pub message: String,
    /// 主标注（用 `^^^` 画）说明"问题出在这"；次标注（用 `---` 画）提供上下文。
    pub primary: bool,
}

/// 一条结构化的编译诊断信息。
///
/// 好的错误信息回答三个问题：出了什么问题（`message`）、在哪里（`labels`）、
/// 怎么改（`help`）。这里刻意把它们分开存放，而不是拼成一个字符串——
/// 这样终端可以渲染成 rustc 风格的文本，图形界面可以直接在源码上画波浪线。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: Option<&'static str>,
    pub message: String,
    pub labels: Vec<Label>,
    pub notes: Vec<String>,
    pub help: Option<String>,
}

impl Diagnostic {
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            code: None,
            message: message.into(),
            labels: Vec::new(),
            notes: Vec::new(),
            help: None,
        }
    }

    pub fn warning(message: impl Into<String>) -> Self {
        Self { severity: Severity::Warning, ..Self::error(message) }
    }

    pub fn note(message: impl Into<String>) -> Self {
        Self { severity: Severity::Note, ..Self::error(message) }
    }

    pub fn with_code(mut self, code: &'static str) -> Self {
        self.code = Some(code);
        self
    }

    pub fn with_primary(mut self, span: Span, message: impl Into<String>) -> Self {
        self.labels.push(Label { span, message: message.into(), primary: true });
        self
    }

    pub fn with_secondary(mut self, span: Span, message: impl Into<String>) -> Self {
        self.labels.push(Label { span, message: message.into(), primary: false });
        self
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }

    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    /// 主标注的位置（没有标注时返回 `None`）。
    pub fn primary_span(&self) -> Option<Span> {
        self.labels.iter().find(|l| l.primary).or(self.labels.first()).map(|l| l.span)
    }

    /// 渲染成 rustc 风格的终端文本。
    ///
    /// ```text
    /// 错误[E0102]: 这里应该是一个数字或左括号
    ///  --> 输入:1:5
    ///   |
    /// 1 | 1 + * 2
    ///   |     ^ 遇到了 `*`
    ///   |
    ///   = 帮助: ……
    /// ```
    pub fn render(&self, source: &str, file_name: &str) -> String {
        let mut out = String::new();
        match self.code {
            Some(code) => writeln!(out, "{}[{}]: {}", self.severity.as_str(), code, self.message),
            None => writeln!(out, "{}: {}", self.severity.as_str(), self.message),
        }
        .unwrap();

        let mut labels: Vec<&Label> = self.labels.iter().collect();
        labels.sort_by_key(|l| (l.span.start, !l.primary));

        if let Some(first) = self.primary_span() {
            let lc = line_col(source, first.start);
            let max_line = labels.iter().map(|l| line_col(source, l.span.start).line).max().unwrap_or(1);
            let gutter = max_line.to_string().len();
            let pad = " ".repeat(gutter);

            writeln!(out, "{pad}--> {}:{}:{}", file_name, lc.line, lc.col).unwrap();
            writeln!(out, "{pad} |").unwrap();
            let mut last_line = None;
            for label in labels {
                let lc = line_col(source, label.span.start);
                let line_start = source[..label.span.start.min(source.len())].rfind('\n').map_or(0, |i| i + 1);
                let line_end = source[line_start..].find('\n').map_or(source.len(), |i| line_start + i);
                let before = &source[line_start..label.span.start.min(line_end)];
                let marked = &source[label.span.start.min(line_end)..label.span.end.min(line_end)];
                let mark_char = if label.primary { "^" } else { "-" };
                let mark_len = display_width(marked).max(1);

                // 同一行上的多个标注共用一行源码。
                if last_line != Some(lc.line) {
                    writeln!(out, "{:>gutter$} | {}", lc.line, &source[line_start..line_end]).unwrap();
                    last_line = Some(lc.line);
                }
                let mut underline =
                    format!("{pad} | {}{}", " ".repeat(display_width(before)), mark_char.repeat(mark_len));
                if !label.message.is_empty() {
                    underline.push(' ');
                    underline.push_str(&label.message);
                }
                writeln!(out, "{}", underline.trim_end()).unwrap();
            }
            writeln!(out, "{pad} |").unwrap();
            for note in &self.notes {
                writeln!(out, "{pad} = 注: {note}").unwrap();
            }
            if let Some(help) = &self.help {
                writeln!(out, "{pad} = 帮助: {help}").unwrap();
            }
        } else {
            for note in &self.notes {
                writeln!(out, "  = 注: {note}").unwrap();
            }
            if let Some(help) = &self.help {
                writeln!(out, "  = 帮助: {help}").unwrap();
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_rustc_style() {
        let src = "1 + * 2";
        let d = Diagnostic::error("这里应该是一个数字或左括号")
            .with_code("E0102")
            .with_primary(Span::new(4, 5), "遇到了 `*`")
            .with_help("在 `*` 前面补上一个数字");
        let expected = "\
错误[E0102]: 这里应该是一个数字或左括号
 --> 输入:1:5
  |
1 | 1 + * 2
  |     ^ 遇到了 `*`
  |
  = 帮助: 在 `*` 前面补上一个数字
";
        assert_eq!(d.render(src, "输入"), expected);
    }

    #[test]
    fn underline_aligns_after_cjk() {
        let src = "（1+2";
        let d = Diagnostic::error("x").with_primary(Span::new(3, 4), "");
        let rendered = d.render(src, "f");
        // 全角括号占 2 列，所以 ^ 前面应该有 2 个空格
        assert!(rendered.contains("  |   ^\n"), "{rendered}");
    }

    #[test]
    fn secondary_labels_on_other_lines() {
        let src = "(1\n+ 2";
        let d = Diagnostic::error("缺少右括号")
            .with_primary(Span::empty_at(src.len()), "这里需要 `)`")
            .with_secondary(Span::new(0, 1), "左括号在这里");
        let r = d.render(src, "f");
        assert!(r.contains("1 | (1\n  | - 左括号在这里\n"), "{r}");
        assert!(r.contains("2 | + 2\n  |    ^ 这里需要 `)`\n"), "{r}");
    }
}
