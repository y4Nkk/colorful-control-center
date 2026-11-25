use serde::{Deserialize, Serialize};
use tauri::{LogicalPosition, LogicalSize, PhysicalSize};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Rect {
    pub x: i32,
    pub y: i32,
    pub size: PhysicalSize<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct SavedPlacement {
    pub display: String,
    pub offset_x: f64,
    pub offset_y: f64,
    pub width: f64,
    pub height: f64,
}

impl SavedPlacement {
    pub fn valid(&self) -> bool {
        !self.display.is_empty()
            && self.offset_x.is_finite()
            && self.offset_y.is_finite()
            && self.width.is_finite()
            && self.height.is_finite()
            && self.width > 0.0
            && self.height > 0.0
    }

    pub fn matches(&self, display: &str) -> bool {
        self.display.eq_ignore_ascii_case(display)
    }
}

pub(super) fn fit(
    work: Rect,
    scale: f64,
    frame: PhysicalSize<i32>,
    default: LogicalSize<f64>,
    minimum: LogicalSize<f64>,
    saved: Option<&SavedPlacement>,
    restore_offset: bool,
) -> Rect {
    let size = saved.map_or(default, |saved| LogicalSize::new(saved.width, saved.height));
    let wanted = size.to_physical::<i32>(scale);
    let minimum = minimum.to_physical::<i32>(scale);
    let width = outer_extent(wanted.width, minimum.width, frame.width, work.size.width);
    let height = outer_extent(
        wanted.height,
        minimum.height,
        frame.height,
        work.size.height,
    );
    let remaining = PhysicalSize::new(work.size.width - width, work.size.height - height);
    let offset = saved.filter(|_| restore_offset).map(|saved| {
        LogicalPosition::new(saved.offset_x, saved.offset_y).to_physical::<i32>(scale)
    });
    Rect {
        x: work.x
            + offset.map_or(remaining.width / 2, |offset| {
                offset.x.clamp(0, remaining.width)
            }),
        y: work.y
            + offset.map_or(remaining.height / 2, |offset| {
                offset.y.clamp(0, remaining.height)
            }),
        size: PhysicalSize::new(width, height),
    }
}

fn outer_extent(wanted: i32, minimum: i32, frame: i32, available: i32) -> i32 {
    wanted
        .max(minimum)
        .saturating_add(frame)
        .clamp(1, available)
}
