//! Air-crush trace lines without crush notes ("rainbone"), used for decoration.

/// `MP_OPTIONVALUE_AIRCRUSH_TRACELIKE`: Margrete's "no notes" mode, which leaves only the line.
pub(crate) const LINE_ONLY: i32 = 0;
pub(crate) const DEFAULT_HEIGHT: i32 = 80;
pub(crate) const MAX_HEIGHT: i32 = 1000;

pub(crate) struct LineColor {
    /// Value stored in the head note's variation ID; it does not follow the menu order.
    pub id: i32,
    /// Key shown in Margrete's context menu.
    pub key: char,
    /// Preview color; `None` for the transparent line.
    pub rgb: Option<[u8; 3]>,
    /// `[ja, en, zh-TW, ko]`
    pub names: [&'static str; 4],
}

pub(crate) const COLORS: &[LineColor] = &[
    LineColor {
        id: 0,
        key: '0',
        rgb: Some([255, 64, 255]),
        names: ["通常", "Default", "一般", "기본"],
    },
    LineColor {
        id: 1,
        key: '1',
        rgb: Some([255, 48, 48]),
        names: ["赤", "Red", "紅", "빨강"],
    },
    LineColor {
        id: 2,
        key: '2',
        rgb: Some([255, 160, 48]),
        names: ["橙", "Orange", "橙", "주황"],
    },
    LineColor {
        id: 3,
        key: '3',
        rgb: Some([255, 255, 0]),
        names: ["黄", "Yellow", "黃", "노랑"],
    },
    LineColor {
        id: 12,
        key: '4',
        rgb: Some([200, 255, 48]),
        names: ["草", "Lime", "草綠", "연두"],
    },
    LineColor {
        id: 4,
        key: '5',
        rgb: Some([48, 255, 48]),
        names: ["緑", "Green", "綠", "초록"],
    },
    LineColor {
        id: 5,
        key: '6',
        rgb: Some([48, 255, 255]),
        names: ["水", "Aqua", "水藍", "물빛"],
    },
    LineColor {
        id: 13,
        key: '7',
        rgb: Some([48, 232, 255]),
        names: ["空", "Sky", "天空", "하늘"],
    },
    LineColor {
        id: 14,
        key: '8',
        rgb: Some([48, 176, 255]),
        names: ["天", "Azure", "蒼", "창공"],
    },
    LineColor {
        id: 6,
        key: '9',
        rgb: Some([48, 96, 255]),
        names: ["青", "Blue", "藍", "파랑"],
    },
    LineColor {
        id: 7,
        key: 'A',
        rgb: Some([160, 64, 255]),
        names: ["青紫", "Violet", "藍紫", "청보라"],
    },
    LineColor {
        id: 15,
        key: 'B',
        rgb: Some([224, 48, 255]),
        names: ["赤紫", "Purple", "紅紫", "적보라"],
    },
    LineColor {
        id: 8,
        key: 'C',
        rgb: Some([255, 96, 224]),
        names: ["桃", "Pink", "桃紅", "분홍"],
    },
    LineColor {
        id: 10,
        key: 'D',
        rgb: Some([255, 255, 255]),
        names: ["白", "White", "白", "하양"],
    },
    LineColor {
        id: 11,
        key: 'E',
        rgb: Some([96, 96, 96]),
        names: ["黒", "Black", "黑", "검정"],
    },
    LineColor {
        id: 35,
        key: 'Z',
        rgb: None,
        names: ["透明", "Transparent", "透明", "투명"],
    },
];

pub(crate) fn color(id: i32) -> &'static LineColor {
    COLORS
        .iter()
        .find(|color| color.id == id)
        .unwrap_or(&COLORS[0])
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Output {
    #[default]
    Slide,
    Rainbone {
        color: i32,
        height: i32,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_ids_are_unique_and_unknown_ids_fall_back_to_default() {
        for (index, color) in COLORS.iter().enumerate() {
            assert!(COLORS[index + 1..].iter().all(|other| other.id != color.id));
        }
        assert_eq!(color(3).names[0], "黄");
        assert_eq!(color(13).names[0], "空");
        assert_eq!(color(35).names[0], "透明");
        assert_eq!(color(-1).id, 0);
    }
}
