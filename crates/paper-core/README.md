# paper-core

文献管理のドメインモデルと、保存・外部取得に依存しない処理を提供します。

| モジュール | 役割 |
| --- | --- |
| `model` | 文献、書誌情報、利用者メタデータ、識別子、候補、版、履歴 |
| `service` | 更新判定、同一性判定、引用キー提案、BibTeX 出力 |
| `ports` | 保存、メタデータ取得、添付ストレージのインターフェース |
| `command` / `query` | 状態変更と検索・読込の要求 |
| `error` | コア処理のエラー |

書誌情報の候補 `MetadataCandidate` は `model/bibliography.rs`、利用者情報 `UserMetadata` は `model/user_metadata.rs` にあります。保存・HTTP・画面の実装はそれぞれのアダプターに分離しています。ポートやモデルの存在は、その機能が画面や保存層で実装済みであることを意味しません。

設計・実装範囲は[ルート README](../../README.md)、契約は [同一性判定](../../docs/identity-assessment.md)、[候補と版](../../docs/metadata-candidates.md)、[取得・引用キー](../../docs/metadata-lookup-and-keys.md)、[識別子・出力](../../docs/identifiers-and-export.md)を参照してください。

## 検証

```sh
cargo test -p paper-core --locked
```

## ライセンス

[MIT](../../LICENSE-MIT) OR [Apache-2.0](../../LICENSE-APACHE)。
