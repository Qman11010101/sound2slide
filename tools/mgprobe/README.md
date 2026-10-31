# mgprobe

Margrete の譜面規則（主に 48 レーン拡張フィールドのはみ出し規則）を実測するための開発用プラグインです。
配布物には含めません。sound2slide のワークスペースにも含めていない独立したクレートです。
調べた結果は [`docs/exfield-48lanes.md`](../../docs/exfield-48lanes.md) にあります。

## 何をするか

Margrete の「拡張機能」メニューから `mgprobe` を実行すると、環境変数 `MGPROBE_DIR` のディレクトリにある
`case.txt` を読んでノーツを配置し、譜面の全ノーツを `dump.txt` に書き出します。
配置したあと譜面出力（F5）を行い、出力が通るか、校閲ウィンドウにどのノーツが並ぶかを見ます。

`case.txt` の書式（`#` 以降はコメント）:

```text
MODE load                  # load: 既存ノーツを全削除して配置 / dump: 書き出しのみ
BPM 120                    # tick 0 の BPM
BEAT 4 4                   # 小節 0 の拍子（省略時 4/4）
EV BPM 4000 360            # イベント追加（BPM / TIL / NSM）。前回 load で追加したものは次の load で消える
0 SLIDE BEGIN 0 4 3840     # <深さ> <種別> <ロング属性> <x> <幅> <tick> [高さ] [option] [variation] [direction] [ex]
1 SLIDE END -8 4 4800      # 深さ n は直前の深さ n-1 のノーツの子になる
2 AIR NONE -8 4 4800 80 0 0 2
```

種別は `TAP EXTAP FLICK DAMAGE HOLD SLIDE AIR AIR_HOLD AIR_SLIDE AIR_CRUSH CLICK`、
ロング属性は `NONE BEGIN STEP CONTROL CURVE END END_NO_ACT` です。
`case.txt` は手で書かずに、[`cases/`](cases) のスクリプトで生成します。

## Linux（Claude Code のクラウド環境など）で動かす

前提: リポジトリ外のセットアップで、cargo-xwin・Wine・Xvfb・xdotool・ImageMagick・日本語ロケール
（`ja_JP.UTF-8`）と CJK フォントを入れておくこと。

```bash
# ビルド（.cargo/config.toml で x86_64-pc-windows-msvc が既定になっている）
cd tools/mgprobe && cargo xwin build --release

# Margrete を取得して展開し、プラグインを置く
curl -O https://umgr.inonote.jp/files/Margrete_v1.9.3.zip   # 配布ページ: https://umgr.inonote.jp/docs/releases
python3 -m zipfile -e Margrete_v1.9.3.zip margrete
cp target/x86_64-pc-windows-msvc/release/mgprobe.dll mgprobe.ini margrete/Margrete_v1.9.3/plugins/

# 自動操作
export MARGRETE_DIR=$PWD/margrete/Margrete_v1.9.3 MGPROBE_WORK=$PWD/work
mkdir -p work && cp base48.mgxc work/
. scripts/mg.sh
mg_start "$(winpath "$MGPROBE_WORK/base48.mgxc")"   # Xvfb と Margrete を起動
mg_setup                                            # ウィンドウを広げて mgprobe を有効化（一覧の 2 行目）
python3 cases/slides.py "$MGPROBE_WORK" 120         # case.txt を生成（idx.txt に小節とケースの対応）
probe                                               # 配置
export_ugc result                                   # 譜面出力。EXPORTED / NOT EXPORTED を表示
proof_list result                                   # 校閲一覧のスクリーンショット（result_head.png / result_tail.png）
```

ケースはそれぞれ小節 3, 5, 7, … に置かれます（`idx.txt` 参照）。校閲一覧に出た小節が NG のケースです。
4/4 以外の拍子では 1 小節の長さが変わるので、一覧の小節番号ではなく tick（ケース k は tick = k × 3840）で対応を取ってください。

- `base48.mgxc`: フィールド拡張を有効、難易度 ULTIMA、レベル 16 にしただけの空の譜面です。
- `mg.sh` の座標は、Xvfb 1600x1000 上で Margrete を 1580x980 にした状態を前提にしています。
- `mg_setup` は「拡張機能の管理」一覧の 2 行目（配布版では MargretePluginSample.dll の次）を有効にします。
  ほかのプラグインも置いている場合は `mg_setup 3` のように行番号を渡してください。
- 起動直後に最初に開いた「拡張機能の管理」ではチェック操作が効かないことがあるため、`mg_setup` は mgprobe が動くまで繰り返します。
- `.ini` は ASCII だけで書いています。BOM なし UTF-8 の日本語は Margrete で文字化けしました。

## ケーススクリプト

| スクリプト | 内容 |
|---|---|
| `cases/notes.py` | スライド以外のノーツ |
| `cases/slides.py <dir> <BPM>` | スライドの許容時間・丸め。BPM を変えると tick 数を換算して同じ ms で試す |
| `cases/events.py` | BPM 変化・スクロール速度イベント・終点の AIR の種類 |
| `cases/signature.py <dir> <BPM> <分子> <分母>` | 拍子と tick の長さ |
| `cases/beyond_field.py` | 48 レーンのさらに外（出力は通るので `out.ugc` を見る） |
