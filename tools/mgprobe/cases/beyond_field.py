"""48 レーンのさらに外。規格チェックは通るが .ugc の x が化ける。出力に成功するので out.ugc を確認する。"""
from mglib import *

work, bpm = args()
b = Batch(bpm)
b.case("DAMAGE x=-24", [(0, "DAMAGE", "NONE", -24, 4, 0)])
b.case("AIR-CRUSH x=-30→40", [(0, "AIR_CRUSH", "BEGIN", -30, 4, 0, 80), (1, "AIR_CRUSH", "END", 40, 4, 480, 80)])
b.case("スライドが 90 tick だけ x=-24", [S("BEGIN", 0, 0, d=0), S("CONTROL", -3, 240), S("CONTROL", -24, 241), S("CONTROL", -24, 331), S("CONTROL", -3, 332), S("END", 0, 600)])
b.write(work)
