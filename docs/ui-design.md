# UI設計

## 配置と責務

低彩度の青・グレーを基本に、折り畳み可能な左メニュー、中央の文献一覧、右の詳細・機能画面を維持する。装飾より情報密度・視認性・操作数を優先する。将来のInbox、お気に入り、未読、候補、タグ/コレクション、設定は、必要な保存・検索機能が揃ってから追加する。

MainWindowは状態、画面遷移、Rust接続、コンポーネント間の配線を担当する。子は表示と局所操作のみを担当し、callbackで意図を返す。DBや全体遷移を子へ移さない。`in` は親からの読込、`in-out` と `<=>` は共有編集値、`private` は子内部の状態に使う。`<=>` の片方向化は入力と保存値の食い違いを生むため、切り出し時に契約を検証する。

## 現在のファイルと接続

- `theme.slint`: 色トークン。OSテーマ/ダークモードの切替は未実装。
- `types.slint`: `NavigationAction`、`AuthorAction`、共有行モデル `AuthorRow`。MainWindowから再エクスポートし、Rustは生成Enumを利用する。
- `components/paper-card.slint`: タイトル・著者・年・キー、選択、hover、出力チェック。通常72px、出力108pxはpreferred-heightの目安。内容が長ければ暗黙のレイアウト最小サイズを優先して高さを伸ばし、見切れを防ぐ。
- `components/sidebar.slint`: 操作通知と表示。遷移・検索は親へ残す。
- `components/splitter.slint`: ドラッグ開始時の幅とウィンドウ上のポインター位置から幅要求を返す。
- `components/paper-detail.slint`: 保存済みの読取専用詳細。著者・識別子警告を含め、正常読込のreplyで表示値を更新する。
- `components/paper-editor.slint`: 編集フォーム、著者行、保存・取消・削除復元の表示と通知。
- `main.slint`: 一覧、取得、候補・版、出力、About/確認、親状態とEditor/Detailの接続。`ui.rs` がworker要求とreplyを接続する。

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

PaperEditorにはDB・画面遷移・編集モード決定を置かない。保存、再読込、候補確認、取消、削除/復元、削除確認への応答をMainWindowへ通知し、親が既存のrequest-navigationとRust callbackを使う。AuthorRowは循環importを避けてtypes.slintへ移し、MainWindowから再エクスポートするのでRustの公開名は維持する。著者の独立コンポーネント化はStep 10に残す。

## 状態と操作制御

現在は `lookup-open` / `metadata-open` / `candidate-form-open` / `export-settings-open` / `export-confirm` 等のboolで排他表示する。Step 13で生成 `DetailView` Enumへ段階移行し、RustとSlintで競合する画面選択を持たない。具体的なvariantはその工程で既存状態遷移を調べて確定する。

`busy`、`ready`、`dirty`、`load-required`、ダイアログ状態は独立して扱う。未保存確認の前に画面を隠したり入力を破棄しない。確認中・処理中は無効操作を防ぎ、必要な取消は残す。現在の閲覧文献とBibTeX用選択集合は独立し、一般の複数選択化はStep 14で仕様を決める。

## 幅変更とBinding Loop

一覧は `preferred-width`、一覧最小260px、詳細最小360px、Splitter幅8pxを使う。親幅に基づく上限は `resize-requested` 内で命令的にclampする。子幅のリアクティブな上限が親の最小幅を再計算して循環する構造を避ける。

Splitterはローカルmouse-x単独ではなくabsolute-positionとの和を使う。ドラッグ、縮小、サイドバー開閉、長文・多数著者で最小幅と追従性を検証する。静的な式の確認やcargo checkのみでBinding Loop解消済みとは判断しない。

## 継続的なUI検証

実ポインター/キーボード入力、accessible-label、focus、文字入力のショートカット競合、空状態、エラー、非同期中制御を各工程で検証する。実機確認とheadlessテストを区別し、できなかった確認は明記する。実施結果と今後の工程は[工程表](roadmap.md)を参照する。
