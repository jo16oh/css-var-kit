# `textDocument/documentColor` のクロスファイル更新問題

## 背景

css-var-kit の LSP サーバーは `var(--name)` 使用箇所にインラインカラースウォッチを表示するため `textDocument/documentColor` を実装している。CSS Custom Properties は別ファイルで定義されることが多いため、**「定義ファイルを編集したとき、それを使用している別ファイルのスウォッチも更新したい」** というクロスファイル更新が必要になる。

しかしこの更新は VSCode では正しく動作するが、Helix と Zed では動作しない。本ドキュメントは原因と対応方針を記録する。

## 観測された問題

`tokens.css` で `--brand: #ff0000` を `#00ff00` に変更したとき:

| クライアント | 同一ファイル内の使用箇所 | 別ファイルの使用箇所 |
|---|---|---|
| VSCode | ✅ 即座に更新 | ✅ 即座に更新 |
| Helix | ✅ 即座に更新 | ❌ 更新されない |
| Zed | ✅ 即座に更新 | ❌ 更新されない |

サーバー側は LSP プロトコルレベルで正しい `ColorInformation` を返している（統合テスト `color_updates_when_definition_in_other_file_changes` で検証済み）。問題は **クライアントが別ファイルに対して再クエリしない** こと。

## クライアント側の再クエリ条件

ソース調査の結果:

| クライアント | `documentColor` を再クエリするトリガー |
|---|---|
| **VSCode (Monaco)** | `didOpen`, `didChange`, **`client/registerCapability` による provider 再登録** |
| **Helix 25.07+** | `didOpen`, `didChange` (250ms debounce), LSP サーバーの初期化/終了 |
| **Zed** | `didChange`（`didOpen` でも本来は再クエリすべきだが Issue #32989 で報告中の不具合あり） |

Helix と Zed のいずれも、サーバーからの「再クエリしてください」というプッシュ型の通知を **そもそも実装していない**。

## 現在の対応

VSCode 向けには [`client/registerCapability`](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#client_registerCapability) を使った dynamic capability re-registration で解決済み（commit `25cdf85`）:

1. クライアントが `textDocument.colorProvider.dynamic_registration: true` を advertise している場合、初期化時に動的登録を行う
2. 定義ファイル変更 / `didChangeWatchedFiles` / 設定リロードのたびに、unregister → register を発行
3. VSCode の `colorProviderRegistry.onDidChange` が発火し、`ColorDetector` が全エディタを再評価

Helix と Zed はこの onDidChange リスナーを持たないため、同じトリックは効かない。

## 根本原因: LSP 仕様のギャップ

LSP 仕様には以下の "refresh" リクエストが定義されている:

| 機能 | refresh 追加バージョン | 想定する依存関係 |
|---|---|---|
| `codeLens` | 3.16 | ワークスペース横断 |
| `semanticTokens` | 3.16 | 型推論 |
| `diagnostic` | 3.17 | 横断的解析 |
| `inlayHint` | 3.17 | 型推論 |
| `inlineValue` | 3.17 | デバッガ状態 |
| `documentColor` | **無し** | (ローカルのみと想定) |

`textDocument/documentColor` は LSP 3.6 (2018年頃) に追加された機能で、当時の想定ユースケースは:

- CSS の `#ff0000` や `rgb(...)` リテラル
- JS/TS 内の色文字列リテラル

いずれも **「色はファイル内のテキストから純粋に決まる」** という前提だった。「ファイルが変わらなければ色も変わらない」が自明に成立するため、refresh の必要性が認識されなかったと思われる。

CSS Custom Properties (CSS Variables) のような **「同じトークンの解決値が別ファイルの編集で変わる」** ユースケースは、後発の機能（diagnostic, semanticTokens など）では考慮されているが、documentColor では取り残されている。

## ユーザー向けワークアラウンド

Helix / Zed では、定義ファイル編集後にスウォッチを更新するには以下のいずれかを行う:

1. 該当ファイルで何か編集する（例: スペース挿入→削除）
2. Helix の場合: `:lsp-restart` で言語サーバーを再起動

## 検討した実装上の選択肢

### A. ドキュメント記載のみ（現状の方針）

LSP 仕様と上流クライアントの問題なので、サーバー側の正攻法では解決不能。制約として README に記載する。

### B. `workspace/applyEdit` で no-op edit を送って `didChange` を擬似発火

サーバーから空の挿入（`range.start == range.end`, `newText == ""`）を applyEdit で送ることで、クライアントに `didChange` イベントを擬似発火させる。

**問題点**:

- ファイルが dirty 状態になる可能性
- undo 履歴に積まれる
- カーソル位置/選択範囲がずれる
- Helix のトランザクションが no-op を「変更なし」と判定して再クエリしないかもしれない（要検証）
- そもそも仕様外のハック

### C. 上流に提案する

- [microsoft/language-server-protocol](https://github.com/microsoft/language-server-protocol) に `workspace/colorProvider/refresh` の追加を提案する。css-var-kit はまさにこの機能の motivating example になり得る。
- Helix / Zed 側で `client/registerCapability` を refresh トリガーとして扱うよう実装してもらう。

長期的には C が本筋。短期的には A で許容する。

## 参考

- [LSP 3.17 specification — Document Color](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_documentColor)
- [Helix 25.07 Release Notes (documentColor 対応)](https://helix-editor.com/news/release-25-07-highlights/)
- [Zed Issue #32989 — Inline LSP color previews not initiating on file open](https://github.com/zed-industries/zed/issues/32989)
- [Zed Issue #4678 — documentColor support](https://github.com/zed-industries/zed/issues/4678)
- 関連コミット: `4bb2a14` (機能追加), `25cdf85` (VSCode 向け refresh 修正)

## 結論: hover ベースのアプローチに切り替え

最終的に `textDocument/documentColor` を撤回し、`textDocument/hover` で `var(--name)` の解決値を表示する方式に切り替えた。

### 切り替えの根拠

- hover はカーソル移動のたびにクライアントが毎回新しいリクエストを送るため、サーバーから refresh を push する必要がない。VSCode / Helix / Zed すべてで同一のコードパスでクロスファイル更新が動作する。
- スウォッチに加えて、解決後の値そのもの（`16px` のような非カラー値も含む）を表示できるのでユーザー体験としてもむしろ拡張されている。
- 将来 LSP 仕様に `workspace/colorProvider/refresh` が追加されたら、インラインスウォッチ + hover の両立を再検討する価値はある。

### 撤回の手段

`git revert 4bb2a14 25cdf85` で履歴を残しながら 2 commit を打ち消し、その上に hover ハンドラを実装した。`crates/css-var-kit/src/color_value.rs` の `parse_to_rgba` は hover 側でも使うので同じ内容で再追加している。
