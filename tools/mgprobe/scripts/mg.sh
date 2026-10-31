# Linux + Wine + Xvfb 上の Margrete を xdotool で操作するためのシェル関数。`. scripts/mg.sh` で読み込む。
#
# 前提: Xvfb :99 (1600x1000) 上で Margrete を 1580x980 のウィンドウにして操作する。座標はこの前提に依存する。
#   MARGRETE_DIR  Margrete.exe のあるディレクトリ（必須）
#   MGPROBE_WORK  case.txt / dump.txt / out.ugc を置く作業ディレクトリ（必須）
#   MGPROBE_SHOTS スクリーンショットの保存先（省略時は作業ディレクトリ）
: "${MARGRETE_DIR:?MARGRETE_DIR を設定してください}" "${MGPROBE_WORK:?MGPROBE_WORK を設定してください}"
MGPROBE_SHOTS="${MGPROBE_SHOTS:-$MGPROBE_WORK}"
export DISPLAY="${DISPLAY:-:99}" WINEDEBUG=-all

winpath() { echo "Z:$(echo "$1" | tr / '\\')"; }
shot() { import -window root -crop "${2:-1600x1000+0+0}" "$MGPROBE_SHOTS/$1.png"; }
wins() { xdotool search --onlyvisible --name . 2>/dev/null | while read -r w; do echo "$w $(xdotool getwindowname "$w")"; done; }

# Xvfb を起動し、Margrete を日本語ロケールで起動する（引数: 開く .mgxc）
mg_start() {
  pgrep -x Xvfb >/dev/null || { nohup Xvfb :99 -screen 0 1600x1000x24 -nolisten tcp >/dev/null 2>&1 & sleep 1; }
  wineserver -k 2>/dev/null; sleep 2
  (cd "$MARGRETE_DIR" && MGPROBE_DIR="$(winpath "$MGPROBE_WORK")" LANG=ja_JP.UTF-8 LC_ALL=ja_JP.UTF-8 \
    nohup wine Margrete.exe "$@" >"$MGPROBE_WORK/margrete.log" 2>&1 &)
  until xdotool search --onlyvisible --name Margrete >/dev/null 2>&1; do sleep 2; done; sleep 5
}

# ウィンドウを広げ、拡張機能の管理で n 行目（既定 2 = 配布版の MargretePluginSample.dll の次の mgprobe.dll）を有効にする。
# 起動後に最初に開いた管理画面ではチェック操作が効かないことがあるため、
# mgprobe が実際に動くまで（最大 3 回）繰り返す。
mg_setup() {
  local w y saved i
  w=$(xdotool search --onlyvisible --name Margrete | head -1)
  xdotool windowmove "$w" 0 0 windowsize "$w" 1580 980; sleep 1.5
  y=$((310 + (${1:-2} - 1) * 17))
  saved=$(cat "$MGPROBE_WORK/case.txt" 2>/dev/null)
  echo "MODE dump" >"$MGPROBE_WORK/case.txt"
  for i in 1 2 3; do
    xdotool key Escape; sleep 0.3
    xdotool mousemove 489 11 click 1; sleep 1.2; xdotool mousemove 530 61 click 1; sleep 2
    xdotool mousemove 588 "$y" click 1; sleep 0.8
    shot "setup_manager_$i"; xdotool mousemove 885 637 click 1; sleep 1.5
    if probe >/dev/null; then echo "mgprobe enabled"; printf '%s\n' "$saved" >"$MGPROBE_WORK/case.txt"; return 0; fi
  done
  printf '%s\n' "$saved" >"$MGPROBE_WORK/case.txt"; echo "mgprobe を有効にできませんでした"; return 1
}

# mgprobe を実行し、dump.txt が更新されるまで待つ
probe() {
  local before; before=$(stat -c %y "$MGPROBE_WORK/dump.txt" 2>/dev/null)
  for _ in 1 2 3; do
    xdotool key Escape; sleep 0.3; xdotool key Escape; sleep 0.3
    xdotool mousemove 489 11 click 1; sleep 1.2; xdotool mousemove 506 32 click 1
    for _ in 1 2 3 4 5 6; do
      sleep 0.5
      [ "$(stat -c %y "$MGPROBE_WORK/dump.txt" 2>/dev/null)" != "$before" ] && { cat "$MGPROBE_WORK/dump.txt"; return 0; }
    done
  done
  echo "PROBE FAILED"; return 1
}

# 譜面出力（F5）。規格違反があると出力されず、校閲ウィンドウに違反ノーツが並ぶ
export_ugc() {
  rm -f "$MGPROBE_WORK/out.ugc"
  xdotool key Escape; sleep 0.3; xdotool mousemove 800 300 key F5; sleep 2
  if xdotool search --onlyvisible --name '^譜面出力$' >/dev/null 2>&1; then
    xdotool mousemove 837 740 click --repeat 3 --delay 80 1; xdotool key ctrl+a
    xdotool type --delay 10 "$(winpath "$MGPROBE_WORK")\\out.ugc"; xdotool key Return; sleep 2.5
  fi
  [ -f "$MGPROBE_WORK/out.ugc" ] && echo "EXPORTED" || echo "NOT EXPORTED"
  shot "${1:-export}"; xdotool key Return; sleep 0.5
}

# 校閲ウィンドウの一覧を先頭と末尾の 2 枚撮る（行が多いときは一覧が切れるため）
proof_list() {
  xdotool mousemove 300 900 click 1; xdotool key Home; sleep 0.5; shot "${1:-proof}_head" 700x260+0+740
  xdotool key End; sleep 0.5; shot "${1:-proof}_tail" 700x260+0+740
}
