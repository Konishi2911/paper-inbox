# Paper Inbox 開発ルール

## 作業の進め方

- 作業前に Git の状態と関連実装を確認する。ユーザーの未コミット変更を破棄・無断で取り込んでコミットしない。履歴を書き換えない。
- 過去の設計案やロードマップの完了表記より、現在の実装・テストを優先する。アプリを作り直さない。
- 原則一工程ずつ進め、目的・対象・方針を示し、検証結果を日本語で報告する。
- 重要なアーキテクチャ変更、非互換DB変更、未確定の製品仕様は選択肢とトレードオフを提示して判断を求める。局所的な判断は合理的に進め、説明する。
- 機能追加と構造整理を分け、不要な抽象化・ファイル分割・大規模変更を避ける。

## アーキテクチャ・Rust

- Rust Edition 2024 の Cargo workspace を維持する。
- `paper-core`: モデル、サービス、コマンド、クエリ、ポート。SQLite・Slint・HTTP実装に依存させない。
- `paper-storage`: 正規化SQLite・履歴・トランザクション。`paper-metadata`: HTTP取得。`paper-desktop`: アプリケーション層とUI。
- `model.rs` + `model/` 形式を使い、`mod.rs` は作らない。公開範囲を絞り、必要なAPIを明示的に再エクスポートする。
- 新しい安易なglob import、入力や外部応答に対する `unwrap` / `panic` を避ける。既存コードの一括置換はしない。
- ID・識別子は既存newtypeを利用し、Deserializeからも検証を迂回させない。エラーは責務の近くで定義する。
- `MetadataCandidate` は `model/bibliography.rs`、`UserMetadata` は `model/user_metadata.rs` に置く。

## 保護すべき契約

- ローカルファースト。書誌情報は正規化SQLで保存し、全体JSONへの置換をしない。
- 外部の欠落値と明示的削除を区別する。初回未入力は補完可能、手動値と明示的クリアは承認なしに上書きしない。
- 取得日時だけで鮮度を決めず、出典・レコード・情報源更新日時・現在の改訂を検査する。
- 出版社版を優先するが、正規版切替と項目採用は別々の承認。異なる文献を自動マージしない。
- 保存済み引用キーを外部更新で変更しない。削除済みもキーを保持する。
- 一括保存の原子性、古い提案の承認防止、候補再投入の冪等性、履歴保持を崩さない。

## Slint

- 子は表示と局所操作、親は画面遷移と接続、Rustはユースケースと永続化を担当する。
- 読取専用は `in property`、共有編集値は必要な `in-out` / 双方向バインディング、内部状態は `private property`、操作通知はcallback。
- Rust側の接続をMainWindowへ集約する。操作EnumはSlintの生成型を利用し、Rustに二重定義しない。
- 色はThemeへ集約。排他画面のEnum化と独立したbusy/dirty等を区別する。
- Binding Loopを放置しない。親子のサイズ制約を循環させず、Splitterはウィンドウ上のポインター位置で移動量を計算する。
- 未保存確認前に画面状態を変えない。保存失敗は入力・編集を維持し、正常再読込後に閲覧へ戻す。
- 新しいSlint固有の仕組みは、採用理由と動作を日本語で説明する。

## 検証とGit

- 変更に応じた単体・結合・画面操作テストを追加する。Slintと生成Rust APIを一緒に検証する。
- 各工程で `cargo fmt --all --check`、`cargo check --workspace`、`cargo test --workspace`、`cargo clippy --workspace --all-targets -- -D warnings` を確認する。取得済み依存では `--offline --locked` を利用できる。
- コンパイル成功だけでUIを確認済みとしない。実操作、サイズ変更、入力保護、非同期処理中の制御を確認し、環境上の制約や既存失敗は区別して報告する。
- コミットは論理的に分け、英語の説明に `[ADD]` / `[UPDATE]` / `[FIX]` / `[REFACTOR]` / `[UI]` / `[PERF]` / `[STYLE]` / `[REMOVE]` / `[DOCS]` / `[TEST]` / `[BUILD]` / `[CI]` / `[CHORE]` を付ける。
- このセッションでは工程別コミットが承認済み。pushは明示的な指示に従う。
- 独自コードは `MIT OR Apache-2.0`。Slint等の第三者条件を維持する。

## 設計資料

- [全体設計](docs/design.md)
- [UI設計](docs/ui-design.md)
- [工程と実装監査](docs/roadmap.md)
- [確定判断と未解決事項](docs/decisions.md)
- 詳細契約の入口は[README](README.md)。将来計画を実装済みと表記しない。
