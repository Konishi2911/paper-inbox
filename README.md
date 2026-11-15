# Paper Inbox

ローカルの SQLite に文献を保存する、Rust + Slint 製の文献管理アプリです。DOI / arXiv ID からの書誌情報取得、引用キーの提案、手動編集、更新候補の承認、BibTeX 出力を行えます。

現在は開発中です。動作確認は Ubuntu 22.04 / WSL2 + WSLg、Rust 1.100.0 で行っています。Windows・macOS 向けの保存先には対応していますが、各 OS の動作検証や配布パッケージの整備はまだ行っていません。

## 起動

Rust / Cargo と、Slint の描画バックエンドに必要なビルド・GUI 環境が必要です。WSL2 では WSLg を利用してください。依存バージョンは [Cargo.lock](Cargo.lock) に固定しています。

リポジトリのルートで実行します。

```sh
cargo run -p paper-desktop --locked
```

保存先を明示する場合:

```sh
cargo run -p paper-desktop --locked -- --database /path/to/library.sqlite
```

`--database` には相対パスも指定できます。未作成の親フォルダーは起動時に作成します。対応する起動オプションは `--database` のみです。

## 基本操作

- 左のメニュー: DOI / arXiv からの登録、手動登録、通常・削除済み一覧、BibTeX 出力を開きます。左上のボタンで折り畳めます。
- 中央の一覧: タイトル・著者・引用キーで検索し、並べ替えやページ移動を行います。文献カードをクリックすると右側に詳細を表示します。
- 右の作業欄: 詳細編集、新規登録、取得結果の確認、更新候補・版の承認を行います。未保存入力を破棄して別の文献へ移るときは確認します。

### 書誌情報の取得と引用キー

「DOI / arXiv から登録」で識別子を入力し、「書誌情報を取得」を押します。DOI は Crossref、arXiv ID は arXiv 公式 API へ送信します。取得結果と引用キーを確認し、「この内容で登録」を押すまで保存しません。バックグラウンドでの自動取得は行いません。

引用キーは `Smith2024` → 衝突時 `Smith2024Graph` → さらに衝突時 `Smith2024Graph2` の順で提案します。削除済み文献のキーも使用済みとして扱います。保存前に編集でき、保存済みキーを自動で変更することはありません。手動入力画面でも「引用キーを提案」を利用できます。名前の整形・年不明時の規則は[詳細仕様](docs/metadata-lookup-and-keys.md#citation-key-rule)を参照してください。

既存文献への取得は「候補・版を確認」から行います。手動値の上書きは承認待ちにし、正規版の切替と書誌項目の採用は別々に判断します。初回登録の未入力項目は補完できますが、ユーザーが明示的に消した値は保護します。

### BibTeX 出力

左の「BibTeX 出力」を開き、一覧のチェックで対象文献を選びます。右欄で `.bib` の保存先と項目を指定し、プレビュー・警告を確認して出力します。既存ファイルを置き換える場合は確認を挟みます。出力対象は保存済みの内容です。

## データの保存

| 環境 | 標準保存先 |
| --- | --- |
| Linux / WSL | `$XDG_DATA_HOME/paper-inbox/library.sqlite`。未指定・相対パスの場合は `~/.local/share/paper-inbox/library.sqlite` |
| Windows | `%LOCALAPPDATA%/paper-inbox/library.sqlite` |
| macOS | `~/Library/Application Support/paper-inbox/library.sqlite` |

書誌情報、著者、履歴、更新候補、版は正規化した SQL テーブルで保存します。書誌情報全体を JSON にまとめて保存する方式ではありません。削除は復元可能な論理削除です。画面の保存は一括トランザクションで行い、他の編集との競合を検出すると全体を拒否します。

データベースは対応する旧スキーマから自動移行します。バックアップは、アプリと同じ DB を使う他のプロセスを終了してから取得してください。アプリ内のバックアップ・復元機能は未実装です。

## 現在の範囲

実装済み: 文献の登録・編集・検索、著者順序の保持、メモ・読書状態・お気に入り・アーカイブ、削除・復元、DOI / arXiv の検証、書誌情報取得、引用キー提案、候補と版の承認、BibTeX 出力。

未実装: PDF の添付・取得・閲覧、タグ・コレクション、文献の重複統合、遠隔同期、DataCite や任意の出版社サイトからの取得。保存層に同期用イベントの記録はありますが、同期通信は実装していません。検索の大文字・小文字同一視は SQLite の ASCII 範囲です。画面にない書誌項目も保存時に保持しますが、全項目を画面から編集できるわけではありません。

## 開発と検証

```sh
cargo test --workspace --locked
cargo check --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo fmt --all --check
```

依存取得済みなら `--offline` を追加できます。通常テストは書誌情報の外部 API を呼び出しません。公開サンプルを使う外部 API の確認は、明示的に実行します。

```sh
cargo test -p paper-metadata --test live_contract --locked -- --ignored
```

| パス | 役割 |
| --- | --- |
| [paper-core](crates/paper-core/README.md) | モデル、コマンド・クエリ、更新判定、出力、外部機能の契約 |
| [paper-storage](crates/paper-storage/README.md) | 正規化 SQLite、履歴、競合検査、候補・版の永続化 |
| [paper-metadata](crates/paper-metadata/README.md) | Crossref / arXiv の取得と応答変換 |
| [desktop](apps/desktop/README.md) | Slint UI とアプリケーション処理 |

設計資料:

- [全体設計](docs/design.md)
- [UI設計](docs/ui-design.md)
- [実装工程・現在の監査結果](docs/roadmap.md)
- [設計判断](docs/decisions.md)

- [識別子と BibTeX 出力](docs/identifiers-and-export.md)
- [文献の同一性判定](docs/identity-assessment.md)
- [更新候補と版の承認](docs/metadata-candidates.md)
- [書誌情報取得と引用キー](docs/metadata-lookup-and-keys.md)
- [保存層の契約](docs/storage-contracts.md)

## ライセンス

本プロジェクトの独自コードは **MIT OR Apache-2.0** です。利用者は [MIT](LICENSE-MIT) または [Apache License 2.0](LICENSE-APACHE) のいずれかを選択できます。第三者のコードや素材はそれぞれの条件に従います。

Slint は本プロジェクト独自コードのライセンスには含まれません。デスクトップアプリでは Slint Royalty-free 2.0 の条件に従い、メニューの「このアプリについて」から公式 `AboutSlint` 表示を開けます。[第三者ライセンスの説明](docs/licenses.md)も参照してください。
