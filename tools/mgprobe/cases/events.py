"""BPM 変化・スクロール速度イベント・終点に付けるノーツの種類。BPM 120 で実行する。"""
from mglib import *

work, _ = args()
b = Batch(120)
for D in (140, 150):
    b.case(f"途中で外 {D} tick・区間中央で BPM120→360", dip(D), [("BPM", 241 + D // 2, 360), ("BPM", 1920, 120)])
    b.case(f"途中で外 {D} tick・区間中央で BPM360→120", dip(D), [("BPM", 0, 360), ("BPM", 241 + D // 2, 120)])
for kind in ("TIL", "NSM"):
    for D in (94, 96):
        for sp in (0.25, 4.0):
            b.case(f"途中で外 {D} tick + {kind} {sp}", dip(D), [(kind, 200, sp), (kind, 400, 1.0)])
for dirv, name in ((3, "AIR-DOWN"), (7, "AIR 左上"), (10, "AIR 右下")):
    b.case(f"末尾 240 tick 外 + {name}", tail(240) + [air(-8, 960, dirv)])
for T in (240, 360):
    b.case(f"末尾 {T} tick 外 + AIR + AIR-HOLD", tail(T) + [air(-8, 960), (3, "AIR_HOLD", "BEGIN", -8, 4, 960, 80), (4, "AIR_HOLD", "END", -8, 4, 1680, 80)])
b.write(work)
