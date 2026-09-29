use std::fmt;
use std::ops::Range;

/// 源码中的一段位置，用 UTF-8 字节偏移表示的半开区间 `[start, end)`。
///
/// 用字节而不是"第几个字"来记位置，是因为 Rust 字符串按 UTF-8 存储，
/// 按字节切片是 O(1) 的；需要给人看的"第几行第几列"再用 [`line_col`] 换算。
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub const fn new(start: usize, end: usize) -> Self {
        debug_assert!(start <= end);
        Self { start, end }
    }

    /// 长度为 0 的位置，常用来指"这里缺了点什么"（比如文件末尾缺右括号）。
    pub const fn empty_at(pos: usize) -> Self {
        Self { start: pos, end: pos }
    }

    /// 覆盖两段位置的最小区间。
    pub fn to(self, other: Span) -> Span {
        Span::new(self.start.min(other.start), self.end.max(other.end))
    }

    pub const fn len(self) -> usize {
        self.end - self.start
    }

    pub const fn is_empty(self) -> bool {
        self.start == self.end
    }

    pub const fn contains(self, other: Span) -> bool {
        self.start <= other.start && other.end <= self.end
    }

    pub fn range(self) -> Range<usize> {
        self.start..self.end
    }

    /// 取出这段位置对应的源码文本。
    pub fn text(self, source: &str) -> &str {
        &source[self.range()]
    }
}

impl fmt::Debug for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}..{}", self.start, self.end)
    }
}

/// 人类可读的位置：从 1 开始的行号和列号（列按字符数计）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LineCol {
    pub line: usize,
    pub col: usize,
}

/// 把字节偏移换算成行列号。`offset` 超出范围时按文件末尾处理。
pub fn line_col(source: &str, offset: usize) -> LineCol {
    let offset = floor_char_boundary(source, offset.min(source.len()));
    let before = &source[..offset];
    let line = before.matches('\n').count() + 1;
    let line_start = before.rfind('\n').map_or(0, |i| i + 1);
    let col = source[line_start..offset].chars().count() + 1;
    LineCol { line, col }
}

fn floor_char_boundary(s: &str, mut i: usize) -> usize {
    while !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

/// 文本在等宽终端里占的列数：中日韩文字和全角符号占 2 列，其余占 1 列。
///
/// 终端里画 `^^^` 指向错误位置时要用它，否则一遇到中文就对不齐。
pub fn display_width(s: &str) -> usize {
    s.chars().map(char_width).sum()
}

fn char_width(c: char) -> usize {
    let wide = matches!(c as u32,
        0x1100..=0x115F
        | 0x2E80..=0x303E
        | 0x3041..=0x33FF
        | 0x3400..=0x4DBF
        | 0x4E00..=0x9FFF
        | 0xA000..=0xA4CF
        | 0xAC00..=0xD7A3
        | 0xF900..=0xFAFF
        | 0xFE30..=0xFE4F
        | 0xFF00..=0xFF60
        | 0xFFE0..=0xFFE6
        | 0x20000..=0x3FFFD
    );
    if wide { 2 } else { 1 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_col_counts_chars_not_bytes() {
        let src = "ab\n中文x";
        assert_eq!(line_col(src, 0), LineCol { line: 1, col: 1 });
        assert_eq!(line_col(src, 3), LineCol { line: 2, col: 1 });
        // "中文" 各占 3 个字节，x 在第 2 行第 3 列
        assert_eq!(line_col(src, 9), LineCol { line: 2, col: 3 });
        assert_eq!(line_col(src, 999), LineCol { line: 2, col: 4 });
    }

    #[test]
    fn display_width_of_cjk() {
        assert_eq!(display_width("a中（"), 5);
    }

    #[test]
    fn span_ops() {
        let a = Span::new(2, 4);
        let b = Span::new(6, 9);
        assert_eq!(a.to(b), Span::new(2, 9));
        assert!(a.to(b).contains(b));
        assert_eq!(Span::new(1, 3).text("abcd"), "bc");
    }
}
