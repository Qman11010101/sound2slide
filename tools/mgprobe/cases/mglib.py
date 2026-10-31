"""mgprobe の case.txt を組み立てるヘルパー。

ケース k は tick = k*3840 に置くため、Margrete の校閲一覧では小節 2k+1 に現れる。
"""
import sys


class Batch:
    def __init__(self, bpm=120, beat=(4, 4)):
        self.out = ["MODE load", f"BPM {bpm}", f"BEAT {beat[0]} {beat[1]}"]
        self.idx = []

    def case(self, desc, lines, events=()):
        k = len(self.idx) + 1
        t = k * 3840
        self.idx.append(f"bar {k * 2 + 1}: {desc}")
        for kind, dt, v in events:
            self.out.append(f"EV {kind} {t + dt} {v}")
        for d, ty, at, x, w, dt, *rest in lines:
            self.out.append(" ".join(map(str, [d, ty, at, x, w, t + dt, *rest])))

    def write(self, work_dir):
        open(f"{work_dir}/case.txt", "w").write("\n".join(self.out) + "\n")
        open(f"{work_dir}/idx.txt", "w").write("\n".join(self.idx) + "\n")
        print("\n".join(self.idx))


def S(at, x, dt, w=4, d=1):
    return (d, "SLIDE", at, x, w, dt)


def air(x, dt, dirv=2, w=4, d=2):
    """AIR（dir: 2=上 3=下 7=左上 8=右上 9=左下 10=右下）"""
    return (d, "AIR", "NONE", x, w, dt, 80, 0, 0, dirv)


def dip(D, a=240):
    """中 → 1 tick で外 (x=-8) → D tick 後に 1 tick で中。外の tick は D+1 個。"""
    return [S("BEGIN", 0, 0, d=0), S("CONTROL", -3, a), S("CONTROL", -8, a + 1),
            S("CONTROL", -8, a + 1 + D), S("CONTROL", -3, a + 2 + D), S("END", 0, a + 240 + D)]


def tail(T, e=960):
    """終点直前の T tick（終点を含め T+1 個の tick）が外。AIR は呼び出し側で足す。"""
    return [S("BEGIN", 0, 0, d=0), S("CONTROL", -3, e - T - 1), S("CONTROL", -8, e - T), S("END", -8, e)]


def mirror(lines):
    """幅 4 のノーツを中央 16 レーンを軸に左右反転する。"""
    return [(d, ty, at, 12 - x, w, dt, *r) for (d, ty, at, x, w, dt, *r) in lines]


def args():
    work = sys.argv[1]
    bpm = float(sys.argv[2]) if len(sys.argv) > 2 else 120
    return work, bpm
