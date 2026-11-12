"""スライドのはみ出し許容時間。BPM を変えて実行すると ms 基準であることを確かめられる。"""
from mglib import *

work, bpm = args()
scale = bpm / 120  # BPM 120 での tick 数を、同じ ms になるよう換算
t = lambda ticks: round(ticks * scale)
b = Batch(bpm)
b.case("始点が外（直後に中へ）", [S("BEGIN", -8, 0, d=0), S("CONTROL", -8, 10), S("CONTROL", -3, 11), S("END", 0, 250)])
for D in (94, 96):
    b.case(f"途中で外 {t(D)} tick", dip(t(D)))
    b.case(f"右側 途中で外 {t(D)} tick", mirror(dip(t(D))))
for T in (45, 50):
    b.case(f"末尾 {t(T)} tick 外・AIR なし", tail(t(T)))
for T in (240, 360):
    b.case(f"末尾 {t(T)} tick 外・AIR あり", tail(t(T)) + [air(-8, 960)])
D = t(90)
b.case(f"外 {D} tick → 中 2 tick → 外 {D} tick", [
    S("BEGIN", 0, 0, d=0), S("CONTROL", -3, 240), S("CONTROL", -8, 241), S("CONTROL", -8, 241 + D), S("CONTROL", -3, 242 + D),
    S("CONTROL", -3, 244 + D), S("CONTROL", -8, 245 + D), S("CONTROL", -8, 245 + 2 * D), S("CONTROL", -3, 246 + 2 * D),
    S("END", 0, 480 + 2 * D)])
for L in (360, 400):
    b.case(f"x=0→-4 直線 {t(L)} tick（端の丸め）", [S("BEGIN", 0, 0, d=0), S("END", -4, t(L))])
for L in (80, 120):
    b.case(f"x-2→-4・幅 3→4 {t(L)} tick（x と幅でなく端を丸める）", [S("BEGIN", -2, 0, w=3, d=0), S("END", -4, t(L), w=4)])
b.write(work)
