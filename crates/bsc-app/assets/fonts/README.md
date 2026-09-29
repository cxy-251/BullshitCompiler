# 字体

| 文件 | 来源 | 许可 | 说明 |
|---|---|---|---|
| `NotoSansSC-Subset.ttf` | Noto Sans SC Regular（Google Fonts） | SIL Open Font License 1.1 | 裁剪为 GB2312 全部汉字 + ASCII + 拉丁/希腊字母 + 常用标点、箭头、数学符号、制表符（约 8600 字） |
| `JetBrainsMono-Subset.ttf` | JetBrains Mono Regular | SIL Open Font License 1.1 | 裁剪为 ASCII + 拉丁/希腊字母 + 标点、箭头、数学符号、制表符；**去掉了连字**（否则 `<=` 会显示成 `≤`，初学者会误以为输入了别的字符） |

裁剪命令（fonttools）：

```sh
pyftsubset NotoSansSC-Regular.ttf --text-file=chars.txt --layout-features='*' --no-hinting --desubroutinize --output-file=NotoSansSC-Subset.ttf
pyftsubset JetBrainsMono-Regular.ttf --unicodes='U+0020-007E,U+00A0-017F,U+0370-03FF,U+2000-206F,U+2190-22FF,U+2500-25FF' --layout-features='kern' --no-hinting --output-file=JetBrainsMono-Subset.ttf
```

裁剪后再运行 `python3 add_glyphs.py NotoSansSC-Subset.ttf`：思源黑体没有 ⊤（U+22A4）和 ⊢（U+22A2），
脚本用 ⊥ 的轮廓上下翻转、旋转 90° 派生出这两个字形（格与类型规则的课要用）。

`chars.txt` = GB2312 全部字符 ∪ U+0020–007E ∪ U+00A0–017F ∪ U+0370–03FF ∪ U+2000–206F ∪ U+2190–22FF ∪ U+2500–25FF ∪ U+3000–303F ∪ U+FF00–FFEF。

版权声明：Noto Sans SC © 2014-2021 Adobe (http://www.adobe.com/), with Reserved Font Name 'Source'；
JetBrains Mono © 2020 The JetBrains Mono Project Authors。许可证全文见 `OFL.txt`。
