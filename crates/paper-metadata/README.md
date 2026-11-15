# paper-metadata

`paper-core::ports::MetadataResolver` を実装する、Crossref / arXiv 用の書誌情報取得アダプターです。

- DOI → Crossref REST API
- arXiv ID → arXiv 公式 Atom API

送信先は固定の HTTPS エンドポイントです。応答の識別子、サイズ、タイムアウトを検査し、省略された項目を削除提案として扱いません。出版社版の DOI を arXiv プレプリント自身の識別子として取り込むこともありません。バックグラウンド取得・自動再試行・PDF 取得は行いません。

デスクトップでは専用ワーカーから `resolve_blocking` を呼びます。非同期ポートは同じ通信処理を別スレッドへ移し、呼出側の実行器をブロックしません。取得項目、通信間隔、出典・鮮度の扱いは[取得仕様](../../docs/metadata-lookup-and-keys.md)を参照してください。

## 検証

```sh
cargo test -p paper-metadata --locked
```

通常テストは固定データとローカル HTTP サーバーで実行します。外部サービスの確認は明示的に行います。

```sh
cargo test -p paper-metadata --test live_contract --locked -- --ignored
```

## ライセンス

[MIT](../../LICENSE-MIT) OR [Apache-2.0](../../LICENSE-APACHE)。依存コードや取得先の条件は[第三者ライセンス](../../docs/licenses.md)を参照してください。
