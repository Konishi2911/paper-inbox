# ライセンス

## 本プロジェクト

独自コードは **MIT OR Apache-2.0** のデュアルライセンスです。「OR」は利用者がどちらかを選べることを意味します。著作権表記は `Copyright (c) 2026 Paper Inbox contributors` です。

- [MIT License 本文](../LICENSE-MIT)
- [Apache License 2.0 本文](../LICENSE-APACHE)
- [ライセンス案内](../LICENSE)

ワークスペースの `license` に SPDX 式 `MIT OR Apache-2.0` を設定し、全クレートが継承します。この指定は本プロジェクト独自コードに適用し、依存コードの条件を変更しません。

## Slint

ロックされている Slint 1.18.1 は、GPL-3.0-only、Slint Royalty-free 2.0、商用ライセンスを選択肢として提供しています。本デスクトップアプリでの Slint 利用には **Slint Royalty-free 2.0** を選択し、メニューから開ける「このアプリについて」に公式 `AboutSlint` ウィジェットを表示します。

- [同梱した Royalty-free 2.0 本文](../LICENSES/LicenseRef-Slint-Royalty-free-2.0.md)
- [使用バージョンの公式ライセンス説明](https://github.com/slint-ui/slint/blob/v1.18.1/LICENSE.md)

この条件は Slint の利用・配布に関するもので、Paper Inbox 独自コードのデュアルライセンスとは別です。Royalty-free 2.0 は組込み機器や Slint 単体の再配布を対象にしていません。上記本文に利用範囲・謝辞・制限の条件があります。

## その他の依存と取得データ

[依存クレートのライセンス宣言](dependency-licenses.md)に、Linux 向け Cargo 依存のバージョンと宣言を記録しています。ライセンスは MIT / Apache-2.0 以外も含み、Slint が内包する第三者素材にもそれぞれの条件があります。

依存一覧は配布物のライセンス本文・著作権通知を集めたものではありません。バイナリ配布を整備する際は、対象 OS・有効機能・同梱素材に対応する第三者ライセンス本文・通知を一緒に用意してください。現時点では配布パッケージは提供していません。

Crossref / arXiv から取得する書誌情報・概要・リンク先の文書に、本プロジェクトのライセンスを適用するものではありません。API の仕様・利用条件は[取得仕様の公式参照先](metadata-lookup-and-keys.md#providers-and-mapping)を参照してください。
