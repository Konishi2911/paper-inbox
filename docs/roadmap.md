# 実装工程と現状監査

更新日: 2026-11-15。初回監査HEAD: `8b64301`。以後のStep 6修復、Step 7/8検証、Step 9〜13の分離と画面Enum化を反映した。機能が存在すること、コミット済みであること、工程の完了条件が検証済みであることは区別する。

## 実装・検証の更新

- `6762f22`: 生成Enumに全テストを追従し、import不足・旧整数操作・fmt不一致を修復。
- `26f69bb`: カードをpreferred-heightと暗黙サイズへ変更し、狭い幅の長文・出力チェックの見切れを修正。実ポインターのSplitter追従、最小/最大幅、縮小・開閉、操作不可条件を試験。
- `ad32671`: 引き継いだStep 8を完成。正常reply時の著者表示/mode、保存失敗・取消・競合・新規遷移・削除復元、著者行番号を修正。既存未コミット2ファイルをこの工程で継承してコミット。
- Step 9: PaperEditor分離。MainWindowのRust接続、親の判断、双方向編集値とdirty、生成AuthorRow/Enumを維持。
- `1cdcbcb`: Step 10、AuthorEditorへ著者編集を分離。3形式と行番号・順序を維持。
- `1e18e14`: Step 11、PaperListへ検索・一覧・pageを分離。検索語共有、pageリセット、親への通知を追加試験。
- `c497d5b`: Step 12、取得・候補・提案・出力・各Dialogを分離。候補の二段の双方向bindと取得入力を実要素から試験。
- Step 13: DetailViewを唯一の可変表示選択へ移行。旧boolは読取専用派生値。出力の戻り先と、読込失敗時のdraft保持を結合試験。
- Workspaceはcore/storage/metadata/desktopの4クレート。schema 4のSQL契約は変更していない。

検証: workspaceのcheck/test、全ターゲットclippy（警告をエラー扱い）、fmt、diffチェック。`editor_window_contract`、`layout_window_contract`、`pane_layout_contract`、`paper_list_window_contract`、`detail_view_window_contract` とreply単体試験で実操作/状態を確認。公開APIの通信テストは通常どおり明示実行専用としてignored。

WSLg/winit-softwareの実描画でもワーカーと単一イベントループによる登録→閲覧→編集→保存エラー→取消、著者表示、縮小・サイドバー、Splitterを確認し、Binding Loopがないことと日本語/長文の描画を確認した。環境由来のxdg desktop settings通知バス接続警告は残り、機能の不具合と区別する。

初回監査で存在したテストのコンパイル失敗・fmt不一致は修復済み。初回監査の未コミットUI変更はStep 8へ継承した。以後は未着手の工程を完了扱いにしない。

## Step 1〜13: UI構造

Step 1〜13を実装・検証済みとする。長期的なUI/a11y試験は引き続き各工程で行う。

| Step | 工程 | 状態・根拠 | 依存と完了条件 |
| --- | --- | --- | --- |
| 1 | Theme | 完了 `489b8d0` | 色トークンの参照、描画の整合を確認 |
| 2 | PaperCard抽出 | 完了 `4c16a23` | 親へのopened/selection callbackと選択・出力の独立性を試験 |
| 3 | カード密度/hover | 完了 `a8b0428`、高さ追従を修正 | 長文、多数著者、通常/出力モードで欠けず操作可能 |
| 4 | Sidebar抽出 | 完了 `776f369` | 子にDB/全体遷移を入れず既存操作・折り畳みを保持 |
| 5 | 操作条件 | 完了 `d66f7fa`、確認中操作を回帰試験 | 削除/破棄/About/出力確認中と処理中の無効操作防止を試験 |
| 6 | 操作Enum | 完了、テスト追従 `6762f22` | 生成Enumを全呼出側で利用し、著者の実行順/行番号を試験 |
| 7 | Splitter | 完了、操作試験 `26f69bb` | 6。ドラッグ追従、縮小/開閉、最小幅、Binding Loopなしを確認 |
| 8 | 閲覧/編集分離 | 完了 `ad32671` | 6/7。著者表示、選択/新規/切替時のmode、保存成功・失敗・再読込失敗・取消の契約を試験 |
| 9 | PaperEditor抽出 | 完了 | 8完了。親API/双方向bind/dirty/削除復元/確認を維持し外観変更を混ぜない |
| 10 | AuthorEditor抽出 | 完了 `1cdcbcb` | 9。個人/団体/Literal、追加削除上下、実行行番号と順序を試験 |
| 11 | PaperList抽出 | 完了 `1e18e14` | 2/7/10。検索/sort/page、幅、出力選択を維持 |
| 12 | 機能画面抽出 | 完了 `c497d5b` | 9〜11。Import/Metadata/Proposal/Export/各Dialogを必要単位で順次分離 |
| 13 | 排他画面Enum | 完了 | 12。DetailView等を生成型で統一、独立boolと確認/処理中状態を維持 |

## Step 14〜23: 操作・機能

工程番号は引き継ぎの識別子として維持するが、将来の優先順位は実用性・不足APIに応じて相談する。アクセシビリティとテストは全工程で行い、23まで延期しない。

| Step | 工程 | 現状 | 依存と完了条件 |
| --- | --- | --- | --- |
| 14 | 一般の複数選択 | 未着手。現状はBibTeX用チェックのみ | 13。通常/Ctrl/Shift/Ctrl+A、閲覧IDと集合、ページ/検索/sort変更時の規則を決定・試験 |
| 15 | Shortcuts | 一部のみ（検索Enter、About Escape） | 14。N/F/E/S/C/Escape/上下のfocus規則、入力の通常編集との非衝突 |
| 16 | 仮想化・連続取得 | 未着手。現状20件page | 11/14。描画とDB取得を別評価。既存page廃止は性能・コストを判断後 |
| 17 | タグ/コレクション | モデルのみ、保存/UI未実装 | 保存API/移行を設計。文献複製なし、階層/付与/検索連携を試験 |
| 18 | DOI/URL/PDFを開く | 未着手（Slint謝辞リンクは対象外） | 詳細UI、PDFは添付実装。scheme/path検証とOS既定利用、shell連結なし |
| 19 | 差分比較UI | 現在/提案/理由/出典と承認拒否は既存。diff強化は未着手 | 12。変更項目/更新時刻/手動解決、色以外の識別、古い提案防止を維持 |
| 20 | Context menu | 未着手 | 14/18/19。単一/複数/削除済みで適用可能な操作を限定 |
| 21 | UI設定永続化 | 未着手。ConfigはCLI DBパスのみ | 保存方式/互換性を決定。窓/幅/折り畳み/sort/themeの復元と既定値 |
| 22 | Dark mode | 未着手。固定Themeのみ | 1/21。標準widgetと独自配色の整合、OS/手動設定優先度を決定 |
| 23 | A11y/UI回帰 | accessible-label、headless入力試験あり。検証を回復済み | 毎工程実施。focus/keyboard/縮小/長文/空/失敗/dirty/非同期を検証 |

## 次の一工程

**Step 14: 一般の複数文献選択**。実装前に、Ctrl/Shiftのanchor、全選択の対象（表示pageか検索全件か）、page/検索/sort変更時の非表示選択の維持・解除を決定する。閲覧IDと選択集合を分け、保存済みBibTeX出力とdirty保護を維持する。

Step 15以降のshortcut、仮想化、タグ・DB/同期設計を同時に混ぜない。
