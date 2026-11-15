# UI設計

## 配置と責務

低彩度の青・グレーを基本に、折り畳み可能な左メニュー、中央の文献一覧、右の詳細・機能画面を維持する。装飾より情報密度・視認性・操作数を優先する。将来のInbox、お気に入り、未読、候補、タグ/コレクション、設定は、必要な保存・検索機能が揃ってから追加する。

MainWindowは状態、画面遷移、Rust接続、コンポーネント間の配線を担当する。子は表示と局所操作のみを担当し、callbackで意図を返す。DBや全体遷移を子へ移さない。`in` は親からの読込、`in-out` と `<=>` は共有編集値、`private` は子内部の状態に使う。`<=>` の片方向化は入力と保存値の食い違いを生むため、切り出し時に契約を検証する。

## 現在のファイルと接続

- `theme.slint`: 色トークン。OSテーマ/ダークモードの切替は未実装。
- `types.slint`: `NavigationAction`、`AuthorAction`、`DetailView`、共有行モデル（著者・文献・候補・提案・版）。MainWindowから再エクスポートし、Rustは生成Enumを利用する。
- `components/paper-card.slint`: タイトル・著者・年・キー、選択、hover、出力チェック。通常72px、出力108pxはpreferred-heightの目安。内容が長ければ暗黙のレイアウト最小サイズを優先して高さを伸ばし、見切れを防ぐ。
- `components/sidebar.slint`: 操作通知と表示。遷移・検索は親へ残す。
- `components/splitter.slint`: ドラッグ開始時の幅とウィンドウ上のポインター位置から幅要求を返す。
- `components/paper-detail.slint`: 保存済みの読取専用詳細。著者・識別子警告を含め、正常読込のreplyで表示値を更新する。
- `components/author-editor.slint`: 個人/団体/Literal、ORCID、追加・削除・上下の表示と通知。モデルの変更は親のRust処理に委ねる。
- `components/paper-editor.slint`: 編集フォーム、AuthorEditorとの接続、保存・取消・削除復元の通知。
- `components/paper-list.slint`: 検索、sort、archiveフィルタ、カード、page。条件とoffsetはMainWindowと共有し、選択・検索を通知する。
- `views/import-view.slint`: 取得入力・確認。`candidate-editor.slint`: 候補の提示項目と入力。
- `views/metadata-view.slint` / `proposal-view.slint`: 出典・版と現在/提案値、承認通知。承認判定はcore/storageに残す。
- `views/export-view.slint`: 出力設定。`dialogs/export-dialog.slint`: 書込み前のpreview・警告・上書き確認。
- `dialogs/about-dialog.slint` / `confirm-dialog.slint`: 謝辞と共通確認表示。実際の状態変更はMainWindowへ通知する。
- `main.slint`: 3ペイン構成、画面選択、親状態、各コンポーネントの接続。`ui.rs` がworker要求とreplyを接続する。

著者操作の引数順は `(AuthorAction, row_index)`。移動・削除には実際の行番号を渡す。著者フィールド指定の数値は現在残っているが、本工程で無関係なAPI変更を一括導入しない。

## 閲覧・編集の契約（Step 8実装済み）

1. 既存文献を選ぶと保存済みの読取専用詳細を表示する。著者を含む表示値はRustの同一replyから更新する。
2. 「編集」で編集へ、新規登録は直接フォームへ進む。削除済みは編集不可で復元可能な表示を維持する。
3. 保存失敗ではフォームと入力を維持する。保存成功かつ正常再読込が届いた時点で閲覧へ戻る。
4. 保存後の再読込失敗は保存済みであることを示し、再読込まで重複保存しない。成功表示だけで閲覧へ移さない。
5. キャンセルは未保存確認とDB再読込を使い、保存前の値を閲覧に残さない。破棄を取り消したらフォーム・入力を保持する。
6. 別文献、新規、候補画面、再接続でも前文献の編集状態を持ち越さない。

Rustの `apply_reply` は完全なEditorViewが届いたときだけ `edit-mode` と著者表示を更新する。既存IDは閲覧、新規は編集とし、エラーreplyでは入力とmodeを保持する。保存成功後の再読込失敗は `load-required` と既存フォームを保持する。単体試験でreplyの境界を、`editor_window_contract` で実ワーカー・画面操作の保存/取消/競合/削除復元を検証している。

## PaperEditorの接続（Step 9）

編集値とdirtyは `MainWindow <=> PaperEditor` の双方向バインディングで共有する。タイトル入力のアクセシビリティ操作から親の値とdirtyが変わることを結合テストで確認する。読取専用フラグやAuthorRowモデルは `in property` とし、著者行の変更は生成AuthorActionと実際の行番号をcallbackで通知する。

PaperEditorにはDB・画面遷移・編集モード決定を置かない。保存、再読込、候補確認、取消、削除/復元、削除確認への応答をMainWindowへ通知し、親が既存のrequest-navigationとRust callbackを使う。AuthorRowは循環importを避けてtypes.slintへ移し、MainWindowから再エクスポートするのでRustの公開名は維持する。Step 10で著者をAuthorEditorへ分離し、同じcallbackの経路を維持した。

## 画面状態（Step 13）

MainWindowの共有 `detail-view: DetailView` だけで右ペインの表示を選ぶ。RustはSlintの生成Enumを使い、別のcurrent-screenやEnumを二重定義しない。

| DetailView | 表示 |
| --- | --- |
| Empty | 初回準備前等の案内 |
| PaperDetail | 保存済み文献の閲覧 |
| PaperEditor | 新規・編集。削除済みは同じフォームを読取専用にして復元を提供 |
| Import | 新規/既存への取得。lookup-newは取得用途を表すデータcontext |
| MetadataReview | 候補・提案・版の確認 |
| MetadataCandidateEditor | 同じMetadataView内の候補入力 |
| ExportSettings | 出力対象・保存先・項目の設定 |
| ExportPreview | 確認待ちpreview |

旧 `lookup-open`、`metadata-open`、`candidate-form-open`、`export-confirm`、`export-settings-open`、`editor-visible`、`edit-mode` は `out property` の読取専用派生値で、独立した可変状態ではない。既存の読込API名を維持するためのもので、Rust側のsetterは使わない。実際の表示条件はDetailViewを直接比較する。

`busy`、`ready`、各draftの`dirty`、`load-required`、About/削除/破棄/関連付け確認は画面選択と独立した状態として維持する。現在の閲覧文献とBibTeX選択集合も独立し、一般の複数選択化はStep 14で仕様を決める。

### 成功・失敗・戻る

- ローカルな「編集」や出力設定の開閉はMainWindowでEnumを変更する。保存・再読込・取得先切替の結果はRustのreply反映後に一度だけ画面へ適用する。
- DB読込を要求しただけで現在の画面を消さない。未保存破棄を承認しても、移動先の読込が失敗したら元の画面・入力・dirtyを保持する。成功時に新しい読込値とdirtyを反映する。
- `export-return-view` は設定を開く前の戻り先、`preview-return-view` はpreviewを開く前の戻り先。表示判定に使う別のcurrent-screenではなく、履歴だけを保持する。
- 編集/候補/取得から出力設定→preview→キャンセル→設定を閉じる、という入れ子でも元のdraftを復元する。アプリcallbackが編集から直接previewを開いた場合も実際の元画面へ戻す。
- 書誌情報取得の取消は、新規なら新規フォーム、既存なら候補確認へ戻す。保存失敗では現在の画面と入力を維持する。保存成功・再読込失敗では編集を維持して重複保存を防ぐ。

`detail_view_window_contract` はEnumと実際に存在する画面コンポーネントを両方確認し、右ペインの排他性、戻り先、dirty保持、失敗遷移を試験する。`paper_list_window_contract` は子の入力から親の検索条件・offset・通知が変化することを検証する。

## 幅変更とBinding Loop

一覧は `preferred-width`、一覧最小260px、詳細最小360px、Splitter幅8pxを使う。親幅に基づく上限は `resize-requested` 内で命令的にclampする。子幅のリアクティブな上限が親の最小幅を再計算して循環する構造を避ける。

Splitterはローカルmouse-x単独ではなくabsolute-positionとの和を使う。ドラッグ、縮小、サイドバー開閉、長文・多数著者で最小幅と追従性を検証する。静的な式の確認やcargo checkのみでBinding Loop解消済みとは判断しない。

## 継続的なUI検証

実ポインター/キーボード入力、accessible-label、focus、文字入力のショートカット競合、空状態、エラー、非同期中制御を各工程で検証する。実機確認とheadlessテストを区別し、できなかった確認は明記する。実施結果と今後の工程は[工程表](roadmap.md)を参照する。


## Step 19.1〜19.4: 書誌情報の比較と出典

既存のMetadataView / ProposalViewを利用し、MetadataDiffCardを追加した。
現在値・提案値は右ペイン600px以上で左右、それ未満で上下に配置する。
配置方向は親画面の幅から渡し、ScrollView内のカード自身の幅によるサイズ制約の循環を避ける。
色に加えて「追加・削除・変更・変更なし」と承認理由を表示する。
ヘッダーには文献タイトル、引用キー、承認待ち件数を表示し、件数には正規版切替も含むと明示する。

比較はpaper-coreのcompare_field_valuesで型付き値を評価する。未提示はNone、
明示的削除は提示された未設定値、空文字列はSome("")として区別する。
has_initial_valueは登録時の履歴作成規則なので、差分判定には流用しない。
著者は順序・ORCIDを含む既存の構造値、識別子は既存の正規化済み型で比較する。
この変更種別は説明用であり、承認可否や自動更新ポリシーを置き換えない。

MetadataProposalDetailのcurrent_revisionsは現在の全headの改訂を保持する。
複数headの出典を一つに決めつけず、各改訂ID・情報源・レコード・取得日時・情報源更新日時を表示する。
候補側は既存の候補改訂IDとProvenanceを利用し、提案IDに結び付ける。
不明な日時は推測しない。詳細はカード内のボタンで展開する。
長文はUnicode文字境界で600文字のプレビューを生成し、全文を展開できる。
展開状態は局所表示状態で、承認やDB変更を発生させない。

「提案値を採用」「現在値を維持」は既存の採用・拒否へ接続し、項目ごとに即時保存する。
正規版切替は通常の項目更新と別操作のまま維持する。
フィルタ・判断の仮保持・一括適用・カスタム入力・単語差分・Three-way diffは未実装。
タグと添付の保存/UIをこの工程で追加していない。

検証: field_diff_contract、保存層candidate_contractの出典照会、metadata_window_contract、
metadata_diff_window_contractで型・接続・左右/上下・出典/長文展開・操作不可条件を確認する。
WSLgでは比較表示、出典展開、画面縮小を実描画して確認する。
