"""拍子と tick の長さの関係。

Margrete の tick は拍子に関係なく 4 分音符 = 480 tick（3/4 なら 1 小節 1440 tick、3/8 なら 720 tick）。
BPM が 4 分音符基準なら、1 tick の長さは拍子によらず 60000 / (BPM * 480) ms になる。
使い方: signature.py <作業ディレクトリ> <BPM> <分子> <分母>
"""
import sys

from mglib import *

work, bpm = args()
beat = (int(sys.argv[3]), int(sys.argv[4]))
b = Batch(bpm, beat)
ms = 60000 / (bpm * 480)  # 4 分音符基準の 1 tick
for D in (94, 96):
    b.case(f"{beat[0]}/{beat[1]} 途中で外 {D + 1} tick（4 分音符基準で {(D + 1) * ms:.1f} ms）", dip(D))
for T in (45, 50):
    b.case(f"{beat[0]}/{beat[1]} 末尾 {T + 1} tick 外・AIR なし（4 分音符基準で {(T + 1) * ms:.1f} ms）", tail(T))
b.write(work)
