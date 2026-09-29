//! 字符集：一条边上可以接受的字符，比如 `a`、`[a-z]`、`.`。
//!
//! 用若干个互不重叠、按顺序排列的闭区间表示。`[a-z0-9]` 就是
//! `['0', '9'], ['a', 'z']` 两个区间。

use std::fmt;

/// 代理区（surrogate）不是合法的 `char`，从所有集合里排除掉，
/// 这样区间端点总是合法字符。
const SURROGATES: (u32, u32) = (0xD800, 0xDFFF);

#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct CharSet {
    ranges: Vec<(u32, u32)>,
}

impl CharSet {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn single(c: char) -> Self {
        Self { ranges: vec![(c as u32, c as u32)] }
    }

    pub fn range(lo: char, hi: char) -> Self {
        Self::from_ranges([(lo, hi)])
    }

    /// 由若干区间构造，自动排序、合并重叠或相邻的区间。
    pub fn from_ranges(ranges: impl IntoIterator<Item = (char, char)>) -> Self {
        Self::normalize(ranges.into_iter().map(|(a, b)| (a as u32, b as u32)).collect())
    }

    /// 除换行外的任意字符（正则里的 `.`）。
    pub fn any_but_newline() -> Self {
        Self::normalize(vec![(0, '\n' as u32 - 1), ('\n' as u32 + 1, char::MAX as u32)])
    }

    /// `\d`：数字。
    pub fn digit() -> Self {
        Self::range('0', '9')
    }

    /// `\w`：字母、数字、下划线。
    pub fn word() -> Self {
        Self::from_ranges([('0', '9'), ('A', 'Z'), ('_', '_'), ('a', 'z')])
    }

    /// `\s`：空白字符。
    pub fn space() -> Self {
        Self::from_ranges([('\t', '\n'), ('\r', '\r'), (' ', ' ')])
    }

    fn normalize(mut ranges: Vec<(u32, u32)>) -> Self {
        ranges.retain(|&(a, b)| a <= b);
        ranges.sort_unstable();
        let mut merged: Vec<(u32, u32)> = Vec::with_capacity(ranges.len());
        for (a, b) in ranges {
            match merged.last_mut() {
                Some(last) if a <= last.1.saturating_add(1) => last.1 = last.1.max(b),
                _ => merged.push((a, b)),
            }
        }
        // 去掉代理区
        let mut out = Vec::with_capacity(merged.len());
        for (a, b) in merged {
            if b < SURROGATES.0 || a > SURROGATES.1 {
                out.push((a, b));
            } else {
                if a < SURROGATES.0 {
                    out.push((a, SURROGATES.0 - 1));
                }
                if b > SURROGATES.1 {
                    out.push((SURROGATES.1 + 1, b));
                }
            }
        }
        Self { ranges: out }
    }

    pub fn is_empty(&self) -> bool {
        self.ranges.is_empty()
    }

    pub fn contains(&self, c: char) -> bool {
        let c = c as u32;
        self.ranges
            .binary_search_by(|&(a, b)| {
                if b < c {
                    std::cmp::Ordering::Less
                } else if a > c {
                    std::cmp::Ordering::Greater
                } else {
                    std::cmp::Ordering::Equal
                }
            })
            .is_ok()
    }

    pub fn union(&self, other: &CharSet) -> CharSet {
        Self::normalize(self.ranges.iter().chain(&other.ranges).copied().collect())
    }

    /// 补集（相对于全部字符）。
    pub fn complement(&self) -> CharSet {
        let mut out = Vec::new();
        let mut next = 0u32;
        for &(a, b) in &self.ranges {
            if a > next {
                out.push((next, a - 1));
            }
            next = b + 1;
        }
        if next <= char::MAX as u32 {
            out.push((next, char::MAX as u32));
        }
        Self::normalize(out)
    }

    pub fn intersect(&self, other: &CharSet) -> CharSet {
        let mut out = Vec::new();
        let (mut i, mut j) = (0, 0);
        while i < self.ranges.len() && j < other.ranges.len() {
            let (a1, b1) = self.ranges[i];
            let (a2, b2) = other.ranges[j];
            let (lo, hi) = (a1.max(a2), b1.min(b2));
            if lo <= hi {
                out.push((lo, hi));
            }
            if b1 < b2 { i += 1 } else { j += 1 }
        }
        Self { ranges: out }
    }

    /// 集合里的第一个字符（测试和示例用）。
    pub fn first(&self) -> Option<char> {
        self.ranges.first().and_then(|&(a, _)| char::from_u32(a))
    }

    /// 集合里一共有多少个字符。
    pub fn len(&self) -> u64 {
        self.ranges.iter().map(|&(a, b)| u64::from(b - a + 1)).sum()
    }

    pub fn ranges(&self) -> impl Iterator<Item = (char, char)> + '_ {
        self.ranges.iter().map(|&(a, b)| (char::from_u32(a).unwrap(), char::from_u32(b).unwrap()))
    }

    /// 给人看的写法：单个字符写字符本身，否则写成 `[a-z0-9]`；
    /// 字符特别多时写成"除了 … 之外的任意字符"。
    pub fn label(&self) -> String {
        if self.is_empty() {
            return "∅".to_owned();
        }
        if self.len() > 0x10000 {
            let rest = self.complement();
            return if rest.is_empty() {
                "任意字符".to_owned()
            } else if rest == CharSet::single('\n') {
                ".".to_owned()
            } else {
                format!("[^{}]", rest.inner_label())
            };
        }
        if let [(a, b)] = self.ranges[..]
            && a == b
        {
            return show_char(char::from_u32(a).unwrap());
        }
        format!("[{}]", self.inner_label())
    }

    fn inner_label(&self) -> String {
        let mut s = String::new();
        for (a, b) in self.ranges() {
            s.push_str(&show_char(a));
            if b as u32 == a as u32 + 1 {
                s.push_str(&show_char(b));
            } else if b != a {
                s.push('-');
                s.push_str(&show_char(b));
            }
        }
        s
    }
}

fn show_char(c: char) -> String {
    match c {
        '\n' => "\\n".to_owned(),
        '\t' => "\\t".to_owned(),
        '\r' => "\\r".to_owned(),
        ' ' => "空格".to_owned(),
        c if c.is_control() => format!("\\u{{{:x}}}", c as u32),
        c => c.to_string(),
    }
}

impl fmt::Debug for CharSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label())
    }
}

/// 把若干（可能互相重叠的）字符集切分成两两不相交的"原子"，
/// 使得每个原来的集合都恰好是若干原子的并。
///
/// 子集构造需要它：NFA 的边上可能有 `[a-z]` 和 `[a-f]` 这样重叠的字符集，
/// 而 DFA 的每条出边必须互不重叠，所以先把字母表切成 `[a-f]`、`[g-z]` 两块。
pub fn partition(sets: &[CharSet]) -> Vec<CharSet> {
    // 所有区间的起点和终点+1 都是"切割点"，相邻切割点之间的字符属于完全相同的一组集合。
    let mut cuts: Vec<u32> = sets.iter().flat_map(|s| s.ranges.iter().flat_map(|&(a, b)| [a, b + 1])).collect();
    cuts.sort_unstable();
    cuts.dedup();

    // 签名（属于哪些集合）→ 具有这个签名的区间
    let mut groups: std::collections::BTreeMap<Vec<bool>, Vec<(u32, u32)>> = Default::default();
    for w in cuts.windows(2) {
        let (lo, hi) = (w[0], w[1] - 1);
        let probe = match char::from_u32(lo) {
            Some(c) => c,
            None => continue,
        };
        let signature: Vec<bool> = sets.iter().map(|s| s.contains(probe)).collect();
        if !signature.iter().any(|&b| b) {
            continue;
        }
        groups.entry(signature).or_default().push((lo, hi));
    }
    let mut atoms: Vec<CharSet> = groups.into_values().map(CharSet::normalize).collect();
    atoms.retain(|a| !a.is_empty());
    atoms.sort();
    atoms
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels() {
        assert_eq!(CharSet::single('a').label(), "a");
        assert_eq!(CharSet::range('a', 'z').label(), "[a-z]");
        assert_eq!(CharSet::word().label(), "[0-9A-Z_a-z]");
        assert_eq!(CharSet::from_ranges([('a', 'b')]).label(), "[ab]");
        assert_eq!(CharSet::any_but_newline().label(), ".");
        assert_eq!(CharSet::digit().complement().label(), "[^0-9]");
        assert_eq!(CharSet::single(' ').label(), "空格");
    }

    #[test]
    fn set_operations() {
        let az = CharSet::range('a', 'z');
        let af = CharSet::range('a', 'f');
        assert!(az.contains('m') && !az.contains('A'));
        assert_eq!(az.intersect(&af), af);
        assert_eq!(af.union(&CharSet::range('g', 'z')), az);
        assert_eq!(az.complement().complement(), az);
        assert!(!CharSet::any_but_newline().contains('\n'));
    }

    #[test]
    fn partition_splits_overlaps() {
        let atoms = partition(&[CharSet::range('a', 'z'), CharSet::range('a', 'f'), CharSet::single('x')]);
        assert_eq!(
            atoms,
            vec![CharSet::range('a', 'f'), CharSet::from_ranges([('g', 'w'), ('y', 'z')]), CharSet::single('x')]
        );
        // 同一个签名的不连续区间合并成一个原子
        let atoms = partition(&[CharSet::from_ranges([('0', '9'), ('a', 'a')]), CharSet::single('5')]);
        assert_eq!(atoms.len(), 2);
    }
}
