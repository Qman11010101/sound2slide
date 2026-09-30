# 実行時規約

## 文字列

C ABI の文字列は検証済み UTF-8 です。容量と必要量は終端 NUL を含む byte 数です。

- `buffer == NULL` かつ `capacity == 0` はサイズ照会です。
- サイズ照会は `MG_OK` と必要量を返します。空文字列でも必要量は 1 です。
- 容量不足では `MG_ERR_BUFFER_TOO_SMALL` と必要量を返します。
- 埋め込み NUL、不正な UTF-8、照会時と書き込み時で異なる必要量は拒否されます。
- UTF-8 または変換後の UTF-16 code point の途中で切り詰めません。

プラグイン情報と command 名は、marmkmt が UTF-8 から UTF-16 へ変換して Margrete SDK に渡します。

## ハンドルと所有権

`MgContext` は `invoke` の間だけ有効な借用ハンドルです。解放してはいけません。Context 以外の
Host API から返るハンドルは所有ハンドルで、`invoke` が戻る前に `object_release` が必要です。

- `object_add_ref` は所有ハンドルの参照を 1 つ増やします。
- append / delete は呼び出し側の所有権を移動しません。
- delete 成功後の対象ハンドルは `object_release` だけに使用できます。
- 解放済み、別 invocation 由来、または型の異なるハンドルは無効です。
- ハンドルをプラグインの永続状態へ保存してはいけません。

## スレッドと再入

SDK に触れる操作と release はすべて、その `invoke` が実行されたスレッドで行います。ハンドルを
別スレッドへ渡してはいけません。worker thread へ渡せるのはコピー済みの値データだけです。
worker thread を使う場合は `invoke` が戻る前に join してください。

同じ command instance への同時 `invoke` と再入は拒否されます。`invoke` 復帰後に Margrete へ
非同期に作用する機能は ABI 1 にはありません。

## Undo

Undo 記録はネストできません。`undo_begin_recording` に成功した呼び出しは、同じ `invoke` 内で
`undo_commit_recording` または `undo_discard_recording` を必ず呼びます。記録中の Undo buffer は
解放できません。

プラグインが記録を閉じずに `invoke` から戻った場合、marmkmt は記録を破棄します。その状態で
プラグインが `MG_OK` を返していても、実行結果は失敗になります。

## 例外、panic、エラー通知

C++ 例外や unwind 可能な panic を C ABI 境界の外へ出してはいけません。プラグインの callback は
自身の言語ランタイムに適した方法で境界内に捕捉し、`MG_ERR_PANIC` などを返してください。
`destroy_command` は no-fail であり、例外や panic を送出してはいけません。

marmkmt が callback からの例外を捕捉した場合、その command instance を poisoned 状態にします。
poisoned instance は以後実行できません。新しく生成した command へ状態は引き継がれません。

callback の失敗は、原則として marmkmt が Margrete のウィンドウを owner とするダイアログへ表示します。
検索 API の `MG_ERR_NOT_FOUND` は正常な検索不成立であり、自動表示されません。プラグイン固有の
詳細を表示する場合は `report_error` を呼び、続いて失敗結果を `invoke` から返します。

アクセス違反、stack / heap 破壊、abort、プロセス終了は安全には回復できません。プラグイン DLL は
Margrete と同一プロセスで動く信頼済み native code として扱われます。

## ABI 互換性

API テーブルの利用者は受信した `struct_size` の範囲内だけを読みます。提供者は呼び出し側が示した
`struct_size` を超えて書き込みません。ABI 1 の既存フィールドの削除、並べ替え、意味変更、定数値の
再利用は行いません。ABI の将来版で追加する場合は構造体末尾へ追加します。

