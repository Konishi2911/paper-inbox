# Paper Inbox

ローカルの SQLite に文献を保存する、Rust + Slint 製の文献管理アプリです。DOI / arXiv ID からの書誌情報取得、引用キーの提案、手動編集、更新候補の承認、BibTeX 出力を行えます。

現在は v0.1.0 に向けて整備中です。Windows・macOS 向けに署名なしのバイナリを配布する予定で、CI での確認は [`.github/workflows`](.github/workflows) にあります。開発時の動作確認は Ubuntu 22.04 / WSL2 + WSLg と Windows で行っています。macOS での操作確認は配布前に行います。

## ダウンロードと初回起動

[Releases](https://github.com/Konishi2911/paper-inbox/releases) から、お使いの OS 向けの zip を展開し、Windows は `paper-desktop.exe`、macOS は `Paper Inbox.app` を起動します。インストーラーはなく、削除は展開したフォルダーとデータ（下記「データの保存」）を消すだけです。

実行ファイルには署名がないため、初回は OS の警告が出ます。配布元を確認したうえで次のとおり進めてください。

- **Windows**: 「Windows によってPCが保護されました」と出たら、「詳細情報」→「実行」を選びます。
- **macOS**: 「開発元を確認できません」と出たら、Finder で `Paper Inbox.app` を右クリックして「開く」を選びます。ブロックされた場合は「システム設定」→「プライバシーとセキュリティ」の「このまま開く」を使います。端末から `xattr -dr com.apple.quarantine "Paper Inbox.app"` でも解除できます。

## ソースからの起動

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

- 左のメニュー: 「＋ 文献を追加」、通常・削除済み一覧を開きます。左上のボタンで折り畳めます。
- 中央の一覧: タイトル・著者・引用キーで検索し、並べ替えます。一覧は下端までスクロールすると続きを自動で読み込みます。文献カードをクリックすると右側に詳細を表示します。 詳細ではタグを付け外しでき、サイドバーのタグで絞り込めます。
- 右の作業欄: 詳細の閲覧と編集、新規登録、取得結果の確認、更新候補・版の承認を行います。詳細では読書状態・お気に入り・アーカイブ・メモをその場で変更でき、タイトルの下の操作ボタンの行から、PDF を開く、DOI（なければ arXiv・URL）をブラウザーで開く、BibTeX エントリと引用キーをコピーする操作が 1 クリックでできます。DOI・arXiv ID・URL のコピーなど、その他の操作は「⋯」のメニューにあります。未保存入力を破棄して別の文献へ移るときは確認します。

### PDF の添付

詳細画面の操作ボタンの行で「PDFを添付…」を押し、ファイルを選びます（先頭に `%PDF-` があるファイルだけ受け付けます）。添付後は同じボタンが「PDFを開く」になり、元のファイル名の一時コピーを作って OS 既定のビューアで開きます（複数あるときは最初のファイル。一覧の各行からも開けます）。一覧の「除外」は行内の確認の後に文献から外しますが、ファイルはライブラリに残り、左のメニューの「未使用PDFの整理」で件数と容量を確認してから削除します。PDF は DB には入れず、データベースと同じフォルダーの `<DB名>-files/`（標準の `library.sqlite` なら `library-files/`）に内容のハッシュ名で保存します。バックアップは `library.sqlite` と `library-files/` の両方をコピーしてください。同じ PDF は 1 つの実体を共有します。

#### PDF のドラッグ＆ドロップと識別子の検出

ウィンドウに PDF をドラッグすると、全体に案内の枠が出ます（Windows と macOS。Linux は X11 のみで、Wayland ではドロップが届きません。「PDFを添付…」ボタンは常に使えます）。文献の詳細を開いているときにドロップすると、その文献へ添付します。何も開いていない画面か追加画面でドロップすると（追加画面の「PDFから追加」でも同じ）、追加画面でそのPDFを読み、先頭3ページと PDF の情報から DOI・arXiv ID を探して、最も確からしいものを識別子欄に入れます（入力済みなら上書きせず、ほかの候補はボタンで選べます）。書誌情報の取得は Enter を押したときだけで、PDF の内容は外へ送りません。登録すると、そのPDFが新しい文献に添付されます。スキャン画像のPDFは文字がないため識別子を探せません。

### 書誌情報の取得と引用キー

「＋ 文献を追加」を開くと識別子欄にフォーカスが移ります。DOI / arXiv ID / 公式 URL を貼り付けて Enter（または「書誌情報を取得」）を押すと、DOI は Crossref、arXiv ID は arXiv 公式 API へ送信します。取得結果は項目別に表示され、引用キー欄へフォーカスが移るので、続けて Enter（または「登録」）で登録できます。読書状態・お気に入り・メモも同じ画面で入力でき、取得した書誌情報と一つのトランザクションで保存します。取得した書誌項目は出典を保つため画面上では変更せず、登録後に編集します。識別子がない文献は同じ画面の「手動で入力」から登録します。「登録」を押すまで保存せず、バックグラウンドでの自動取得も行いません。

引用キーは `Smith2024` → 衝突時 `Smith2024Graph` → さらに衝突時 `Smith2024Graph2` の順で提案します。削除済み文献のキーも使用済みとして扱います。保存前に編集でき、保存済みキーを自動で変更することはありません。手動入力画面でも「引用キーを提案」を利用できます。名前の整形・年不明時の規則は[詳細仕様](docs/metadata-lookup-and-keys.md#citation-key-rule)を参照してください。

既存文献への取得は「候補・版を確認」から行います。手動値の上書きは承認待ちにし、正規版の切替と書誌項目の採用は別々に判断します。初回登録の未入力項目は補完できますが、ユーザーが明示的に消した値は保護します。

### BibTeX 出力

一覧で文献を2件以上選び（Ctrl/Cmd クリック、Shift クリック、Ctrl+A）、上のバーの「BibTeX」を押します。右欄で `.bib` の保存先と項目を指定し、プレビュー・警告を確認して出力します。既存ファイルを置き換える場合は確認を挟みます。出力対象は保存済みの内容です。

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

未実装: スキャン画像のPDFからの識別子検出（OCR）、コレクション、文献の重複統合（登録時に同じ DOI・arXiv ID を警告する機能はあります）、遠隔同期、DataCite や任意の出版社サイトからの取得。保存層に同期用イベントの記録はありますが、同期通信は実装していません。検索の大文字・小文字同一視は SQLite の ASCII 範囲です。画面にない書誌項目（月・論文番号・ISBN・ISSN・編者）も保存時に保持しますが、画面からは編集できません。

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
