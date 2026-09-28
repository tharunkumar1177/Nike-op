//! Flyout state projection, layout, hit testing, and placement.
//!
//! Everything here is independent of Win32 and Direct2D so it can be tested
//! without creating a window. Layout coordinates are device-independent
//! pixels (1/96 inch).

pub const FLYOUT_WIDTH: f32 = 360.0;
pub const MAX_PROFILE_ROWS: usize = 6;
pub const MAX_PROCESS_ROWS: usize = 6;

const PADDING: f32 = 16.0;
const GAP: f32 = 8.0;
const TITLE_HEIGHT: f32 = 24.0;
const CAPTION_HEIGHT: f32 = 18.0;
const HEADING_HEIGHT: f32 = 22.0;
const BODY_HEIGHT: f32 = 20.0;
const ROW_HEIGHT: f32 = 40.0;
const BUTTON_HEIGHT: f32 = 32.0;
const END_BUTTON_WIDTH: f32 = 64.0;
const END_ALL_BUTTON_WIDTH: f32 = 76.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RectF {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl RectF {
    pub fn new(left: f32, top: f32, width: f32, height: f32) -> Self {
        Self {
            left,
            top,
            right: left + width,
            bottom: top + height,
        }
    }

    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.left && x < self.right && y >= self.top && y < self.bottom
    }

    pub fn inset(&self, dx: f32, dy: f32) -> Self {
        Self {
            left: self.left + dx,
            top: self.top + dy,
            right: self.right - dx,
            bottom: self.bottom - dy,
        }
    }
}

/// A process instance as the flyout knows it; PID plus creation time, so a
/// recycled PID never matches a stale row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProcessKey {
    pub pid: u32,
    pub creation_time: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessRow {
    pub key: ProcessKey,
    pub image_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlyoutAction {
    Activate(String),
    Deactivate,
    Terminate(ProcessKey),
    TerminateAll,
    OpenSettings,
    ExitRunner,
}

/// Projection of Runner's current in-memory state.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlyoutModel {
    pub profiles: Vec<String>,
    pub active_profile: Option<String>,
    pub processes: Vec<ProcessRow>,
    pub termination_available: bool,
    /// A termination request is in flight.
    pub busy: bool,
    pub status: Option<String>,
}

impl FlyoutModel {
    fn can_terminate(&self) -> bool {
        self.termination_available && !self.busy && self.active_profile.is_some()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextRole {
    Title,
    Heading,
    Body,
    Caption,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonStyle {
    /// Full-width list row.
    Row,
    /// Full-width list row for the active item.
    RowSelected,
    Accent,
    Subtle,
    Danger,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Element {
    Text {
        rect: RectF,
        text: String,
        role: TextRole,
        muted: bool,
    },
    Button {
        rect: RectF,
        label: String,
        detail: Option<String>,
        style: ButtonStyle,
        action: FlyoutAction,
        enabled: bool,
    },
    Divider {
        rect: RectF,
    },
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Layout {
    pub elements: Vec<Element>,
    pub height: f32,
}

impl Layout {
    fn is_enabled_button(&self, index: usize) -> bool {
        matches!(
            self.elements.get(index),
            Some(Element::Button { enabled: true, .. })
        )
    }

    /// Enabled button under a point, in DIPs.
    pub fn hit_test(&self, x: f32, y: f32) -> Option<usize> {
        self.elements
            .iter()
            .enumerate()
            .find_map(|(index, element)| match element {
                Element::Button {
                    rect,
                    enabled: true,
                    ..
                } if rect.contains(x, y) => Some(index),
                _ => None,
            })
    }

    pub fn action(&self, index: usize) -> Option<&FlyoutAction> {
        match self.elements.get(index) {
            Some(Element::Button {
                action,
                enabled: true,
                ..
            }) => Some(action),
            _ => None,
        }
    }

    /// Next enabled button in reading order, wrapping at either end.
    pub fn next_focus(&self, current: Option<usize>, forward: bool) -> Option<usize> {
        let focusable: Vec<usize> = (0..self.elements.len())
            .filter(|&index| self.is_enabled_button(index))
            .collect();
        if focusable.is_empty() {
            return None;
        }
        let position = current.and_then(|index| focusable.iter().position(|&f| f == index));
        let next = match (position, forward) {
            (None, true) => 0,
            (None, false) => focusable.len() - 1,
            (Some(p), true) => (p + 1) % focusable.len(),
            (Some(p), false) => (p + focusable.len() - 1) % focusable.len(),
        };
        Some(focusable[next])
    }
}

struct Builder {
    elements: Vec<Element>,
    y: f32,
}

impl Builder {
    fn content_width() -> f32 {
        FLYOUT_WIDTH - 2.0 * PADDING
    }

    fn text(&mut self, text: impl Into<String>, role: TextRole, muted: bool, height: f32) {
        self.elements.push(Element::Text {
            rect: RectF::new(PADDING, self.y, Self::content_width(), height),
            text: text.into(),
            role,
            muted,
        });
        self.y += height;
    }

    fn divider(&mut self) {
        self.y += GAP;
        self.elements.push(Element::Divider {
            rect: RectF::new(PADDING, self.y, Self::content_width(), 1.0),
        });
        self.y += 1.0 + GAP;
    }

    fn button(
        &mut self,
        rect: RectF,
        label: impl Into<String>,
        detail: Option<String>,
        style: ButtonStyle,
        action: FlyoutAction,
        enabled: bool,
    ) {
        self.elements.push(Element::Button {
            rect,
            label: label.into(),
            detail,
            style,
            action,
            enabled,
        });
    }
}

/// Lay out the flyout for a model at the fixed flyout width.
pub fn layout(model: &FlyoutModel) -> Layout {
    let width = Builder::content_width();
    let mut b = Builder {
        elements: Vec::new(),
        y: PADDING,
    };

    b.text("Edge Optimizer", TextRole::Title, false, TITLE_HEIGHT);
    let summary = match &model.active_profile {
        Some(name) => format!("Active profile: {name}"),
        None => "No profile active".to_string(),
    };
    b.text(summary, TextRole::Caption, true, CAPTION_HEIGHT);
    b.divider();

    b.text("Profiles", TextRole::Heading, true, HEADING_HEIGHT);
    if model.profiles.is_empty() {
        b.text(
            "No profiles yet. Create one in Settings.",
            TextRole::Body,
            true,
            BODY_HEIGHT,
        );
    }
    for name in model.profiles.iter().take(MAX_PROFILE_ROWS) {
        let is_active = model
            .active_profile
            .as_deref()
            .is_some_and(|active| active.eq_ignore_ascii_case(name));
        let rect = RectF::new(PADDING, b.y, width, ROW_HEIGHT);
        if is_active {
            b.button(
                rect,
                name.clone(),
                Some("Active".into()),
                ButtonStyle::RowSelected,
                FlyoutAction::Activate(name.clone()),
                false,
            );
        } else {
            b.button(
                rect,
                name.clone(),
                None,
                ButtonStyle::Row,
                FlyoutAction::Activate(name.clone()),
                true,
            );
        }
        b.y += ROW_HEIGHT + 2.0;
    }
    if model.profiles.len() > MAX_PROFILE_ROWS {
        let hidden = model.profiles.len() - MAX_PROFILE_ROWS;
        b.text(
            format!("{hidden} more in Settings"),
            TextRole::Caption,
            true,
            CAPTION_HEIGHT,
        );
    }
    if let Some(active) = &model.active_profile {
        b.y += 4.0;
        let rect = RectF::new(PADDING, b.y, width, BUTTON_HEIGHT);
        b.button(
            rect,
            format!("Deactivate {active}"),
            None,
            ButtonStyle::Subtle,
            FlyoutAction::Deactivate,
            true,
        );
        b.y += BUTTON_HEIGHT;
    }

    if let Some(active) = &model.active_profile {
        b.divider();
        let heading_top = b.y;
        let heading_width = if model.processes.is_empty() {
            width
        } else {
            width - END_ALL_BUTTON_WIDTH - GAP
        };
        b.elements.push(Element::Text {
            rect: RectF::new(PADDING, heading_top, heading_width, BUTTON_HEIGHT),
            text: format!("Running apps from {active}"),
            role: TextRole::Heading,
            muted: true,
        });
        if !model.processes.is_empty() {
            b.button(
                RectF::new(
                    PADDING + width - END_ALL_BUTTON_WIDTH,
                    heading_top,
                    END_ALL_BUTTON_WIDTH,
                    BUTTON_HEIGHT,
                ),
                "End all",
                None,
                ButtonStyle::Danger,
                FlyoutAction::TerminateAll,
                model.can_terminate(),
            );
        }
        b.y += BUTTON_HEIGHT + 4.0;

        if !model.termination_available {
            b.text(
                "The engine service is unavailable, so apps can't be closed.",
                TextRole::Caption,
                true,
                CAPTION_HEIGHT,
            );
        }
        if model.processes.is_empty() {
            b.text(
                "None of this profile's apps are running.",
                TextRole::Body,
                true,
                BODY_HEIGHT,
            );
        }
        for row in model.processes.iter().take(MAX_PROCESS_ROWS) {
            let label_width = width - END_BUTTON_WIDTH - GAP;
            b.elements.push(Element::Text {
                rect: RectF::new(PADDING, b.y, label_width, 22.0),
                text: row.image_name.clone(),
                role: TextRole::Body,
                muted: false,
            });
            b.elements.push(Element::Text {
                rect: RectF::new(PADDING, b.y + 20.0, label_width, 16.0),
                text: format!("PID {}", row.key.pid),
                role: TextRole::Caption,
                muted: true,
            });
            b.button(
                RectF::new(
                    PADDING + width - END_BUTTON_WIDTH,
                    b.y + 4.0,
                    END_BUTTON_WIDTH,
                    BUTTON_HEIGHT,
                ),
                "End",
                Some(format!("{} (PID {})", row.image_name, row.key.pid)),
                ButtonStyle::Danger,
                FlyoutAction::Terminate(row.key),
                model.can_terminate(),
            );
            b.y += ROW_HEIGHT;
        }
        if model.processes.len() > MAX_PROCESS_ROWS {
            let hidden = model.processes.len() - MAX_PROCESS_ROWS;
            b.text(
                format!("{hidden} more running"),
                TextRole::Caption,
                true,
                CAPTION_HEIGHT,
            );
        }
    }

    b.divider();
    if let Some(status) = &model.status {
        b.text(status.clone(), TextRole::Caption, false, CAPTION_HEIGHT);
        b.y += GAP;
    }
    let half = (width - GAP) / 2.0;
    let footer = b.y;
    b.button(
        RectF::new(PADDING, footer, half, BUTTON_HEIGHT),
        "Open Settings",
        None,
        ButtonStyle::Accent,
        FlyoutAction::OpenSettings,
        true,
    );
    b.button(
        RectF::new(PADDING + half + GAP, footer, half, BUTTON_HEIGHT),
        "Exit",
        None,
        ButtonStyle::Subtle,
        FlyoutAction::ExitRunner,
        true,
    );
    b.y += BUTTON_HEIGHT + PADDING;

    Layout {
        elements: b.elements,
        height: b.y.ceil(),
    }
}

/// Re-check an action from a possibly stale click against the current model.
pub fn validate_action(model: &FlyoutModel, action: &FlyoutAction) -> Option<FlyoutAction> {
    let valid = match action {
        FlyoutAction::Activate(name) => {
            model
                .profiles
                .iter()
                .any(|profile| profile.eq_ignore_ascii_case(name))
                && !model
                    .active_profile
                    .as_deref()
                    .is_some_and(|active| active.eq_ignore_ascii_case(name))
        }
        FlyoutAction::Deactivate => model.active_profile.is_some(),
        FlyoutAction::Terminate(key) => {
            model.can_terminate() && model.processes.iter().any(|row| row.key == *key)
        }
        FlyoutAction::TerminateAll => model.can_terminate() && !model.processes.is_empty(),
        FlyoutAction::OpenSettings | FlyoutAction::ExitRunner => true,
    };
    valid.then(|| action.clone())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RectI {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

fn clamp_start(preferred: i32, low: i32, high: i32) -> i32 {
    preferred.min(high).max(low)
}

/// Top-left corner for a flyout of `size` pixels anchored at the tray click,
/// against the taskbar edge implied by the work area, inside the work area.
pub fn place(
    anchor: (i32, i32),
    monitor: RectI,
    work: RectI,
    size: (i32, i32),
    margin: i32,
) -> (i32, i32) {
    let (width, height) = size;
    let (x, y) = anchor;
    let centered_x = clamp_start(
        x - width / 2,
        work.left + margin,
        work.right - width - margin,
    );
    let centered_y = clamp_start(
        y - height / 2,
        work.top + margin,
        work.bottom - height - margin,
    );
    if work.top > monitor.top {
        (centered_x, work.top + margin)
    } else if work.left > monitor.left {
        (work.left + margin, centered_y)
    } else if work.right < monitor.right {
        (work.right - width - margin, centered_y)
    } else {
        (centered_x, work.bottom - height - margin)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model() -> FlyoutModel {
        FlyoutModel {
            profiles: vec!["Valorant".into(), "Racing".into()],
            active_profile: Some("valorant".into()),
            processes: vec![ProcessRow {
                key: ProcessKey {
                    pid: 42,
                    creation_time: 7,
                },
                image_name: "discord.exe".into(),
            }],
            termination_available: true,
            busy: false,
            status: None,
        }
    }

    fn actions(layout: &Layout) -> Vec<(FlyoutAction, bool)> {
        layout
            .elements
            .iter()
            .filter_map(|element| match element {
                Element::Button {
                    action, enabled, ..
                } => Some((action.clone(), *enabled)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn active_profile_layout_offers_deactivate_and_per_process_termination() {
        // Verifies the projection exposes activation, deactivation, per-PID and bulk termination, and footer actions.
        let layout = layout(&model());
        let key = ProcessKey {
            pid: 42,
            creation_time: 7,
        };
        assert_eq!(
            actions(&layout),
            [
                (FlyoutAction::Activate("Valorant".into()), false),
                (FlyoutAction::Activate("Racing".into()), true),
                (FlyoutAction::Deactivate, true),
                (FlyoutAction::TerminateAll, true),
                (FlyoutAction::Terminate(key), true),
                (FlyoutAction::OpenSettings, true),
                (FlyoutAction::ExitRunner, true),
            ]
        );
        assert!(layout.height > 0.0);
    }

    #[test]
    fn termination_controls_are_disabled_while_busy_or_unavailable() {
        // Verifies no termination can be requested while one is in flight or EngineSvc is unreachable.
        for (available, busy) in [(false, false), (true, true)] {
            let mut state = model();
            state.termination_available = available;
            state.busy = busy;
            let enabled: Vec<bool> = actions(&layout(&state))
                .into_iter()
                .filter(|(action, _)| {
                    matches!(
                        action,
                        FlyoutAction::Terminate(_) | FlyoutAction::TerminateAll
                    )
                })
                .map(|(_, enabled)| enabled)
                .collect();
            assert_eq!(enabled, [false, false]);
        }
    }

    #[test]
    fn inactive_layout_has_no_process_section_and_caps_profile_rows() {
        // Verifies an inactive flyout lists at most the row limit and offers no termination.
        let state = FlyoutModel {
            profiles: (0..9).map(|i| format!("Profile {i}")).collect(),
            ..FlyoutModel::default()
        };
        let buttons = actions(&layout(&state));
        assert_eq!(buttons.len(), MAX_PROFILE_ROWS + 2);
        assert!(!buttons.iter().any(|(action, _)| matches!(
            action,
            FlyoutAction::Deactivate | FlyoutAction::Terminate(_) | FlyoutAction::TerminateAll
        )));
    }

    #[test]
    fn hit_test_and_focus_skip_disabled_buttons() {
        // Verifies pointer and keyboard navigation reach only enabled buttons and wrap in both directions.
        let layout = layout(&model());
        let Element::Button { rect, .. } = &layout.elements[4] else {
            panic!("expected the active profile row");
        };
        assert_eq!(layout.hit_test(rect.left + 1.0, rect.top + 1.0), None);
        let Element::Button { rect, .. } = &layout.elements[5] else {
            panic!("expected the inactive profile row");
        };
        assert_eq!(layout.hit_test(rect.left + 1.0, rect.top + 1.0), Some(5));
        assert_eq!(layout.hit_test(-5.0, -5.0), None);

        let first = layout.next_focus(None, true).unwrap();
        assert_eq!(
            layout.action(first),
            Some(&FlyoutAction::Activate("Racing".into()))
        );
        let last = layout.next_focus(None, false).unwrap();
        assert_eq!(layout.action(last), Some(&FlyoutAction::ExitRunner));
        assert_eq!(layout.next_focus(Some(last), true), Some(first));
        assert_eq!(layout.next_focus(Some(first), false), Some(last));
    }

    #[test]
    fn stale_actions_are_resolved_against_current_state() {
        // Verifies clicks on rows that changed since rendering are dropped instead of executed.
        let state = model();
        let live = ProcessKey {
            pid: 42,
            creation_time: 7,
        };
        let recycled = ProcessKey {
            pid: 42,
            creation_time: 8,
        };
        assert_eq!(
            validate_action(&state, &FlyoutAction::Terminate(live)),
            Some(FlyoutAction::Terminate(live))
        );
        assert_eq!(
            validate_action(&state, &FlyoutAction::Terminate(recycled)),
            None
        );
        assert_eq!(
            validate_action(&state, &FlyoutAction::Activate("VALORANT".into())),
            None
        );
        assert_eq!(
            validate_action(&state, &FlyoutAction::Activate("Deleted".into())),
            None
        );
        let idle = FlyoutModel {
            active_profile: None,
            ..model()
        };
        assert_eq!(validate_action(&idle, &FlyoutAction::Deactivate), None);
        assert_eq!(validate_action(&idle, &FlyoutAction::TerminateAll), None);
        assert_eq!(
            validate_action(&idle, &FlyoutAction::OpenSettings),
            Some(FlyoutAction::OpenSettings)
        );
    }

    #[test]
    fn placement_follows_the_taskbar_edge_and_stays_in_the_work_area() {
        // Verifies bottom, top, left, and right taskbars and clamping near screen corners.
        let monitor = RectI {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1080,
        };
        let size = (360, 400);
        let bottom = RectI {
            bottom: 1032,
            ..monitor
        };
        assert_eq!(place((1900, 1050), monitor, bottom, size, 12), (1548, 620));
        let top = RectI { top: 48, ..monitor };
        assert_eq!(place((100, 20), monitor, top, size, 12), (12, 60));
        let left = RectI {
            left: 48,
            ..monitor
        };
        assert_eq!(place((20, 1000), monitor, left, size, 12), (60, 668));
        let right = RectI {
            right: 1872,
            ..monitor
        };
        assert_eq!(place((1900, 500), monitor, right, size, 12), (1500, 300));
        assert_eq!(place((1900, 1070), monitor, monitor, size, 12), (1548, 668));
    }
}
