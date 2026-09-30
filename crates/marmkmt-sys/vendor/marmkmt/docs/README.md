# marmkmt ドキュメント

このディレクトリには、実装済みの marmkmt C ABI 1 の利用方法と仕様をまとめています。
将来案や実装計画ではなく、`include/marmkmt.h` と現在の実装が提供する契約を記載します。

- [導入・プラグイン実装ガイド](getting-started.md) — ビルド、リンク、`mg_plugin_init`、最小実装
- [C ABI リファレンス](abi-reference.md) — 対象環境、型、API テーブル、データ構造
- [実行時規約](runtime-contract.md) — 文字列、ハンドル、スレッド、Undo、エラー処理

公開 API の宣言と各関数の引数は [`include/marmkmt.h`](../include/marmkmt.h) が正本です。
本ドキュメントと公開ヘッダーが食い違う場合は、公開ヘッダーを優先してください。

## 対応範囲

| 項目 | 対応内容 |
|---|---|
| OS / アーキテクチャ | Windows x64 |
| Margrete Plugin SDK | v2 (`MP_SDK_VERSION == 2`) |
| SDK commit | `b8d0c87125e090880a75f6f0ff57a773510299a8` |
| marmkmt C ABI | 1 |
| 成果物 | スタティックライブラリ `marmkmt.lib` |

marmkmt は bridge DLL、言語別ラッパー、プラグインテンプレートを提供しません。最終成果物は、
利用側のコード、言語別ラッパー、プラグイン実装、および marmkmt をリンクした単一の Margrete
プラグイン DLL です。1 DLL は 1 種類の command を公開します。

