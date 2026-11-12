"""スライド以外のノーツのはみ出し規則。"""
from mglib import *

work, bpm = args()
b = Batch(bpm)
b.case("TAP 左に 1 レーンだけ掛かる", [(0, "TAP", "NONE", -1, 2, 0)])
b.case("TAP 右に 1 レーンだけ掛かる", [(0, "TAP", "NONE", 15, 2, 0)])
b.case("TAP 左外（境界に接する）", [(0, "TAP", "NONE", -4, 4, 0)])
b.case("DAMAGE 完全に外", [(0, "DAMAGE", "NONE", -8, 4, 0)])
b.case("EXTAP 完全に外", [(0, "EXTAP", "NONE", -8, 4, 0)])
b.case("FLICK 右外", [(0, "FLICK", "NONE", 20, 4, 0)])
b.case("HOLD 完全に外", [(0, "HOLD", "BEGIN", -8, 4, 0), (1, "HOLD", "END", -8, 4, 960)])
b.case("HOLD 一部掛かる", [(0, "HOLD", "BEGIN", -2, 4, 0), (1, "HOLD", "END", -2, 4, 960)])
b.case("AIR-CRUSH 完全に外", [(0, "AIR_CRUSH", "BEGIN", -8, 4, 0, 80), (1, "AIR_CRUSH", "END", -8, 4, 960, 80)])
b.case("外の TAP に AIR", [(0, "TAP", "NONE", -8, 4, 0), air(-8, 0, d=1)])
b.write(work)
