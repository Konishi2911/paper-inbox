# UI設計

## 配置と責務

低彩度の青・グレーを基本に、折り畳み可能な左メニュー、中央の文献一覧、右の詳細・機能画面を維持する。装飾より情報密度・視認性・操作数を優先する。将来のInbox、お気に入り、未読、候補、タグ/コレクション、設定は、必要な保存・検索機能が揃ってから追加する。

MainWindowは状態、画面遷移、Rust接続、コンポーネント間の配線を担当する。子は表示と局所操作のみを担当し、callbackで意図を返す。DBや全体遷移を子へ移さない。`in` は親からの読込、`in-out` と `<=>` は共有編集値、`private` は子内部の状態に使う。`<=>` の片方向化は入力と保存値の食い違いを生むため、切り出し時に契約を検証する。

## 現在のファイルと接続

- `theme.slint`: 色トークン。OSテーマ/ダークモードの切替は未実装。
- `types.slint`: `NavigationAction`、`AuthorAction`、`DetailView`、共有行モデル（著者・文献・候補・提案・版）。MainWindowから再エクスポートし、Rustは生成Enumを利用する。
- `components/paper-card.slint`: タイトル・著者・年・キー、選択、hover、出力チェック。通常72px、出力108pxはpreferred-heightの目安。内容が長ければ暗黙のレイアウト最小サイズを優先して高さを伸ばし、見切れを防ぐ。
- `components/sidebar.slint`: 操作通知と表示。登録の入口は「文献を追加」1つ。遷移・検索は親へ残す。
- `components/splitter.slint`: ドラッグ開始時の幅とウィンドウ上のポインター位置から幅要求を返す。
- `components/paper-detail.slint`: 保存済みの読取専用詳細。掲載情報の1行要約、著者の省略表示、識別子のコピー・開く、BibTeXコピー、アブストラクトの展開、個人情報（読書状態・お気に入り・アーカイブ・メモ）のその場変更を通知する。表示値は正常読込のreplyでRustが更新する。
- `components/author-editor.slint`: 1行1著者（種類・ORCIDは行ごとに展開）、追加・削除・上下、まとめて入力の通知。モデルの変更と貼付文字列の解析は親のRust処理に委ねる。
- `components/paper-editor.slint`: 左ラベル2列の編集フォーム、AuthorEditorとの接続、保存（Ctrl+S）・取消・削除復元の通知、保存エラーの表示。
- `components/paper-list.slint`: 検索、sort、archiveフィルタ、カード、page。条件とoffsetはMainWindowと共有し、選択・検索を通知する。
- `views/import-view.slint`: 統合登録画面（lookup-new）と既存文献の識別子取得。取得値は読取専用の項目別表示、登録時の個人入力（引用キー・読書状態・お気に入り・メモ）、手動入力への切替を通知する。`candidate-editor.slint`: 候補の提示項目と入力。
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

「提案値を採用」「現在値を維持」は画面内の判断を変更し、「判断を適用」でまとめて保存する（Step 19.5で更新）。
正規版切替は通常の項目更新と別操作のまま維持する。
フィルタ・判断の仮保持・一括適用・カスタム入力・単語差分を実装済み。Three-wayは共通祖先を検証する比較基盤まで。
タグと添付の保存/UIをこの工程で追加していない。

検証: field_diff_contract、保存層candidate_contractの出典照会、metadata_window_contract、
metadata_diff_window_contractで型・接続・左右/上下・出典/長文展開・操作不可条件を確認する。
WSLgでは比較表示、出典展開、画面縮小を実描画して確認する。

Step 19.5以降の判断モデル・有効性・構造化比較・Three-way準備の詳細は[書誌情報の差分比較と判断](metadata-diff.md)を参照。

## U1: 統合登録画面

登録の入口をサイドバーの「＋ 文献を追加」1つにした。画面を開くと識別子欄へ`init`でフォーカスし、Enterで取得する。取得結果が届くと引用キー欄が生成され、その`init`でフォーカスを移すため、続けてEnterで登録できる。Slintでは条件付き要素（`if`）の内部要素を外から参照できないため、要素自身の`init`で生成時にフォーカスする方式を採用した。

取得した書誌項目は読取専用の`[LookupField]`で表示し、手動値に変換しない。手動値にすると出典が失われ、以後の外部更新がすべて承認待ちになるためである。読書状態・お気に入り・メモは書誌情報ではないので、`RegisterMetadataCandidate`と同じ一括トランザクションで`SetReadingStatus`等を適用する。引用キー不正などで失敗した場合は何も保存しない。

識別子のない文献は同じ画面の「手動で入力」から既存の手動フォーム（見出し「文献を追加（手動入力）」）へ移り、フォームからも「DOI / arXiv から取得に切り替え」で戻れる。未保存入力がある場合は既存の破棄確認を経る。閉じるボタンは確認なしで破棄するため、入力がある場合は「破棄して閉じる」と表示する。閉じた後は案内表示（Empty）に戻る。

検証: `unified_add_window_contract` で入口の一本化、初期フォーカスとキー入力、Enterのみの取得→登録、閉じる・手動入力への切替を、`lookup_window_contract` と `lookup_application_contract` で項目別表示・個人入力の一括保存・失敗時に何も残らないこと・出典維持を確認する。

## U3: 詳細画面と入力フォーム

詳細は保存済みの値だけを表示する。取得済みでも以前は表示されなかった出版社・巻・号・ページ・URL・アブストラクト・キーワードを表示し、掲載情報は「掲載先 巻(号), pp. ページ, 年・種別・出版社」の1行に要約する。要約、著者の省略（先頭3名＋他n名）、アブストラクトの300文字プレビューはRust（`detail.rs`）で計算し、正常読込のreplyで一度に反映する。

読書状態・お気に入り・アーカイブ・メモは書誌情報ではないため、詳細から直接保存する（`QuickChange`）。表示中の`PaperEditState`を期待状態として一括保存し、別の変更があれば競合として拒否する。トグル系の失敗は保存済みの値を再読込して表示を戻し、メモの失敗は入力を保持する。メモの編集中は既存の`dirty`を使うため、移動時の破棄確認がそのまま働き、即時操作は下書きを失わないよう無効にする。メモ編集状態（`notes-editing`）は正常再読込後にRustが解除する。

コピーは標準LineEdit/TextEditの`select-all()`と`copy()`を使い、クリップボード用の依存を増やさない。BibTeXは非表示の読取専用TextEditに保存済みエントリを置いてコピーする。リンクは検証済みDOI/arXivから`https://doi.org/`・`https://arxiv.org/abs/`を組み立て、URLはhttp(s)で空白・制御文字のないものだけを、Slint経由で既に依存している`webbrowser`でシェルを介さずに開く。

フォームは詳細と同じ順序（書誌→識別子→内容→自分の記録）で、左にラベル、右に入力を置く。ラベルのTextは入力と同じ名前を持つため`accessible-role: none`にして重複を避ける。著者は1行1名とし、種類とORCIDは行ごとの「⋯」で開く。「まとめて入力」は「姓, 名」を`;`・改行区切りで受け、カンマのない英字名は最後の語を姓、それ以外は最初の語を姓とみなして行を追加する（保存前に確認・修正できる）。保存エラーはステータス行に加えて保存ボタンの上に表示し、成功した再読込で消す。`FocusScope`で子の入力から伝播したCtrl+Sを受けて保存する。新規登録で引用キーが空なら通常の提案規則で決め、保存済みキーは自動生成しない。

編集で変更したと判定する項目だけを`EditBibliography`にし、前後空白の違いでは改訂を作らない。空欄化は明示的削除として扱う。ScrollViewは`content-width: self.visible-width`で内容幅を表示幅に固定し、長いURLや区切りのない語で右側の操作が表示外へ押し出されるのを防ぐ。

検証: `paper_form_contract`で新項目の往復、未変更保存で改訂を作らないこと、変更項目だけが手動出典になること、URLのみの変更でDOIを保持すること、即時操作の競合・削除済み拒否、著者解析、要約とリンク検証を確認する。`detail_window_contract`で取得項目の表示、コピー→貼付、即時保存、メモの破棄確認と保存、Ctrl+Sとエラー表示・入力保持、一括著者入力、引用キー自動提案、最小幅1080×680で操作が収まることを確認する。
