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


追加した主な依存（2026-11-19 の更新で一覧へ反映）:

- `lopdf`（MIT）: 追加画面でPDFから DOI・arXiv ID を探すために使います。PDFの読み取りだけで、ファイルを書き換えません。依存として `aes` / `cbc` / `md-5` / `sha2`（RustCrypto、MIT OR Apache-2.0）、`weezl`、`rangemap`、`stringprep` なども入ります。
- `rfd`（MIT）: PDF添付のファイル選択に使います。
- BSD-3-Clause を含むもの: `alloc-no-stdlib`、`alloc-stdlib`、`brotli-decompressor`（BSD-3-Clause/MIT）、`encoding_rs`（`(Apache-2.0 OR MIT) AND BSD-3-Clause`）。バイナリを配布する際は、BSD-3-Clause の著作権表示・条件・免責を同梱物に含めてください。
- Unicode-3.0 を含むもの: `unicode-normalization`、`unicode-properties`（`(MIT OR Apache-2.0) AND Unicode-3.0`）。配布時は Unicode のライセンス通知を含めてください。
- 一覧は外部の依存だけを載せ、本プロジェクトのクレート（`paper-*`）は含みません。

依存一覧は配布物のライセンス本文・著作権通知を集めたものではありません。バイナリ配布を整備する際は、対象 OS・有効機能・同梱素材に対応する第三者ライセンス本文・通知を一緒に用意してください。現時点では配布パッケージは提供していません。

Crossref / arXiv から取得する書誌情報・概要・リンク先の文書に、本プロジェクトのライセンスを適用するものではありません。API の仕様・利用条件は[取得仕様の公式参照先](metadata-lookup-and-keys.md#providers-and-mapping)を参照してください。

## 配布物の第三者通知

リポジトリ直下の [THIRD-PARTY-NOTICES.md](../THIRD-PARTY-NOTICES.md) は、Windows（x86_64）と macOS（arm64・x86_64）向けの依存のライセンス本文を集めた通知で、配布 zip に同梱します。`cargo-about` で生成します（設定は `about.toml`、書式は `about.hbs`）。依存を変更したら再生成してください。Slint のクレートは Royalty-free 2.0 として `LICENSES/` の本文に従うため、この通知には含めていません。

```sh
cargo install cargo-about --locked --features cli
cargo about generate about.hbs --locked -o THIRD-PARTY-NOTICES.md
```
