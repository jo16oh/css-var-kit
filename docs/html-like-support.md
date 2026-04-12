# HTML-like 言語対応 実装計画

Vue / Svelte / Astro などの HTML スーパーセット言語に対応する。
`<style>` ブロックを素朴なスキャナで抽出し、その内容を既存の CSS パーサーに渡す。

---

## 設計方針

### `<style>` ブロック抽出

- `<style` を探し、属性部分をスキャンする
- `lang` 属性が `css` または `scss` のとき、あるいは `lang` 属性がないときのみ対象とする
- それ以外 (`less`, `stylus` 等) はスキップする
- 1 ファイルに複数の `<style>` ブロックがある場合はすべて対象とする

### 位置情報の扱い

CSS パーサーが返す `Property` の `line`/`column` は、ファイル全体での絶対位置でなければならない。  
これは `diagnostic_renderer` が `source` の `n` 行目を表示するためと、LSP が UTF-16 カラムに変換するために使用するため。

- `line_offset` : `<style>` の内容が始まる行（0-indexed）
- `column_offset` : `<style>` の内容が始まる列（0-indexed）。**CSS コンテンツの最初の行にのみ適用する**

例:
```html
<p>foo</p><style>.a {   ← line=1, column_offset=17 が適用される
  width: 1px;           ← column_offset は不要（行頭から始まる）
}</style>
```

### `Property.source` の扱い

`source` には HTML ファイル全体の内容を格納する。  
`diagnostic_renderer` はコンテキスト表示のために `source.lines().nth(line)` を使用するため、  
CSS ブロックだけでなくファイル全体が必要。

---

## 変更・作成ファイル一覧

| ファイル | 種別 | 内容 |
|---|---|---|
| `src/parser/html_like.rs` | 新規作成 | `<style>` 抽出ロジック |
| `src/parser.rs` | 変更 | `html_like` モジュール追加 |
| `src/parser/css.rs` | 変更 | `parse_with_offset()` 追加 |
| `src/commands/lint.rs` | 変更 | 拡張子対応・解析分岐 |
| `src/commands/lsp.rs` | 変更 | `load_all_sources` 拡張 |
| `src/commands/lsp/diagnostics.rs` | 変更 | `publish_diagnostics` 拡張 |
| `src/commands/lsp/file_watcher.rs` | 変更 | glob パターン拡張 |

---

## 詳細設計

### 1. `src/parser/html_like.rs`（新規）

```rust
pub struct StyleBlock<'src> {
    pub content: &'src str,   // <style>～</style> の内側のCSS文字列
    pub line_offset: u32,     // content の開始行（ファイル全体での行番号、0-indexed）
    pub column_offset: u32,   // content の最初の行の開始列（0-indexed）
}

/// HTML-like ファイルから <style> ブロックを抽出する
pub fn extract_style_blocks(source: &str) -> Vec<StyleBlock<'_>>

/// HTML-like ファイルをパースして ParseResult のリストを返す
/// source は HTML ファイル全体の内容
pub fn parse_html_like<'src>(
    source: &'src str,
    file_path: &'src Path,
) -> Vec<css::ParseResult<'src>>
```

**スキャンロジック:**

1. バイト列を先頭から走査し、`<style` を探す
2. 見つかったら属性部分をスキャン:
   - `lang="..."` または `lang='...'` を探す
   - `lang` なし → 許可
   - `lang` の値が `css` または `scss` → 許可
   - それ以外 → この `<style>` をスキップ
3. `>` を見つけたら直後が CSS コンテンツの開始
   - その時点の行・列を `line_offset` / `column_offset` として記録
4. `</style>` を探してそこまでを `content` とする

**`parse_html_like` の実装:**

各 `StyleBlock` に対して `css::parse_with_offset(block.content, file_path, source, block.line_offset, block.column_offset)` を呼び出し、結果を flat_map してまとめる。

---

### 2. `src/parser/css.rs` への追加

現在:
```rust
pub fn parse<'a>(css: &'a str, file_path: &'a Path) -> ParseResult<'a> {
    parse_impl(css, file_path, 0)
}
```

追加:
```rust
/// HTML-like ファイルの <style> ブロックをパースする際に使用する
/// full_source: HTML ファイル全体の内容（Property.source に格納するため）
/// line_offset / column_offset: CSS コンテンツの開始位置（ファイル全体での絶対位置）
pub fn parse_with_offset<'src>(
    css: &'src str,
    file_path: &'src Path,
    full_source: &'src str,
    line_offset: u32,
    column_offset: u32,
) -> ParseResult<'src>
```

**`Scanner` の変更:**

```rust
struct Scanner<'a> {
    bytes: &'a [u8],
    pos: usize,
    line: u32,
    col: u32,
    line_offset: u32,
    column_offset: u32,  // CSS の最初の行にのみ加算する
}

impl<'a> Scanner<'a> {
    fn new_with_offset(css: &'a str, line_offset: u32, column_offset: u32) -> Self {
        Self {
            bytes: css.as_bytes(),
            pos: 0,
            line: line_offset,
            col: column_offset,
            line_offset,
            column_offset,
        }
    }
}
```

`col` の初期値を `column_offset` にすることで最初の行のオフセットを自動的に適用できる。  
2 行目以降は `advance()` の `\n` 処理で `col = 0` にリセットされるため、自然に対応できる。

また `parse_with_offset` では `Property.source` に `full_source` を格納する。

---

### 3. `src/parser.rs`

```rust
pub mod css;
pub mod html_like;
```

---

### 4. `src/commands/lint.rs`

**ファイル収集の拡張:**

```rust
const HTML_LIKE_EXTENSIONS: &[&str] = &["vue", "svelte", "astro"];

fn collect_source_files_recursive(dir: &Path, files: &mut Vec<PathBuf>) {
    // 既存の .css 収集ロジックを拡張
    // .vue / .svelte / .astro も収集する
}
```

**解析の分岐:**

```rust
let parse_results: Vec<_> = sources
    .iter()
    .flat_map(|(path, content)| {
        match path.extension().and_then(|e| e.to_str()) {
            Some("css") => vec![parser::css::parse(content, path)],
            Some("vue" | "svelte" | "astro") => {
                parser::html_like::parse_html_like(content, path)
            }
            _ => vec![],
        }
    })
    .collect();
```

---

### 5. `src/commands/lsp.rs`

`load_all_sources` 関数の `collect_css_files` を、拡張子が増えた新しい収集関数に差し替える。

---

### 6. `src/commands/lsp/diagnostics.rs`

`publish_diagnostics` の解析部分に同様の拡張子分岐を追加する。

---

### 7. `src/commands/lsp/file_watcher.rs`

クライアント側ウォッチャーのグロブパターンを拡張:

```rust
// 変更前
GlobPattern::String("**/*.css".to_owned())

// 変更後（複数パターン）
watchers: vec!["**/*.css", "**/*.vue", "**/*.svelte", "**/*.astro"]
    .into_iter()
    .map(|pattern| FileSystemWatcher {
        glob_pattern: GlobPattern::String(pattern.to_owned()),
        kind: None,
    })
    .collect()
```

サーバー側ウォッチャーの拡張子フィルターも同様に更新する。

---

## テスト方針

- `tests/fixtures/` に `html-like/` フィクスチャを追加
  - `basic.vue` / `basic.svelte` / `basic.astro` — CSS カスタムプロパティを含む
  - 複数 `<style>` ブロックを持つファイル
  - `lang="scss"` / `lang="less"` を含むファイル
  - 1 行の `<style>` ブロック（column_offset のテスト）
- `tests/lint.rs` に HTML-like ファイルの lint テストを追加
- `parser/html_like.rs` にユニットテストを追加
  - `lang` フィルタリング
  - `line_offset` / `column_offset` の正確性
