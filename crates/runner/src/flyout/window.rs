//! Runner-owned flyout window.
//!
//! The window lives on Runner's UI thread and is pumped by Runner's message
//! loop. Its state sits in a `RefCell` referenced from `GWLP_USERDATA`; no
//! borrow is held across a Win32 call that can re-enter the window procedure.

use super::model::{self, FlyoutAction, FlyoutModel, Layout, RectI, FLYOUT_WIDTH};
use super::render::{Interaction, Palette, Renderer};
use anyhow::{Context, Result};
use std::cell::RefCell;
use std::time::{Duration, Instant};
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
    DWM_WINDOW_CORNER_PREFERENCE,
};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, InvalidateRect, MonitorFromPoint, ValidateRect, HMONITOR, MONITORINFO,
    MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::WM_MOUSELEAVE;
use windows::Win32::UI::HiDpi::{
    GetDpiForMonitor, SetThreadDpiAwarenessContext, DPI_AWARENESS_CONTEXT,
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, MDT_EFFECTIVE_DPI,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, TrackMouseEvent, TME_LEAVE, TRACKMOUSEEVENT, VK_DOWN, VK_ESCAPE, VK_RETURN,
    VK_SHIFT, VK_SPACE, VK_TAB, VK_UP,
};
use windows::Win32::UI::WindowsAndMessaging::*;

const CLASS_NAME: PCWSTR = w!("EdgeOptimizerQuickFlyout");
const TIMER_ID: usize = 1;
const TICK_MILLISECONDS: u32 = 1000;
const SCREEN_MARGIN: i32 = 12;

/// Something the flyout reports back to Runner's loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlyoutEvent {
    Action(FlyoutAction),
    Hidden,
    /// Periodic tick while visible; Runner refreshes tracked processes.
    Tick,
}

/// Runs the enclosed calls with per-monitor-v2 DPI awareness, so monitor,
/// cursor, and window coordinates are physical pixels.
struct DpiScope(DPI_AWARENESS_CONTEXT);

impl DpiScope {
    fn enter() -> Self {
        Self(unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) })
    }
}

impl Drop for DpiScope {
    fn drop(&mut self) {
        if self.0 .0 != 0 {
            unsafe {
                SetThreadDpiAwarenessContext(self.0);
            }
        }
    }
}

/// Cursor position in physical pixels, for anchoring the flyout at a tray click.
pub fn cursor_position() -> (i32, i32) {
    let _scope = DpiScope::enter();
    let mut point = POINT::default();
    if unsafe { GetCursorPos(&mut point) }.is_ok() {
        (point.x, point.y)
    } else {
        (0, 0)
    }
}

struct WindowState {
    model: FlyoutModel,
    layout: Layout,
    palette: Palette,
    renderer: Renderer,
    dpi: u32,
    anchor: (i32, i32),
    hover: Option<usize>,
    pressed: Option<usize>,
    focus: Option<usize>,
    keyboard_focus: bool,
    tracking_mouse: bool,
    visible: bool,
    hidden_at: Option<Instant>,
    events: Vec<FlyoutEvent>,
    render_failed: bool,
}

/// What the window procedure does after its state borrow is released.
enum After {
    Result(LRESULT),
    Default,
    Invalidate,
    Hide,
    Reposition(RECT),
}

impl WindowState {
    fn scale(&self) -> f32 {
        self.dpi as f32 / 96.0
    }

    fn to_dip(&self, lparam: LPARAM) -> (f32, f32) {
        let x = (lparam.0 & 0xFFFF) as u16 as i16 as f32;
        let y = ((lparam.0 >> 16) & 0xFFFF) as u16 as i16 as f32;
        (x / self.scale(), y / self.scale())
    }

    fn pixel_size(&self) -> (i32, i32) {
        let scale = self.scale();
        (
            (FLYOUT_WIDTH * scale).ceil() as i32,
            (self.layout.height * scale).ceil() as i32,
        )
    }

    fn relayout(&mut self) {
        self.layout = model::layout(&self.model);
        let count = self.layout.elements.len();
        for slot in [&mut self.hover, &mut self.pressed, &mut self.focus] {
            if slot.is_some_and(|index| index >= count) {
                *slot = None;
            }
        }
        if self
            .focus
            .is_some_and(|index| self.layout.action(index).is_none())
        {
            self.focus = self.layout.next_focus(None, true);
        }
    }

    fn activate(&mut self, index: Option<usize>) -> After {
        match index.and_then(|index| self.layout.action(index)).cloned() {
            Some(action) => {
                self.events.push(FlyoutEvent::Action(action));
                After::Invalidate
            }
            None => After::Result(LRESULT(0)),
        }
    }

    fn handle(&mut self, hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> After {
        match message {
            WM_PAINT => {
                let interaction = Interaction {
                    hover: self.hover,
                    pressed: self.pressed,
                    focus: self.focus,
                    show_focus: self.keyboard_focus,
                };
                let dpi = self.dpi as f32;
                if let Err(error) =
                    self.renderer
                        .render(hwnd, dpi, &self.layout, &self.palette, interaction)
                {
                    if !self.render_failed {
                        tracing::warn!("flyout rendering failed: {:#}", error);
                        self.render_failed = true;
                    }
                }
                unsafe {
                    let _ = ValidateRect(hwnd, None);
                }
                After::Result(LRESULT(0))
            }
            WM_ERASEBKGND => After::Result(LRESULT(1)),
            WM_SIZE => {
                let dpi = self.dpi as f32;
                self.renderer.resize(hwnd, dpi);
                After::Result(LRESULT(0))
            }
            WM_DPICHANGED => {
                self.dpi = u32::from((wparam.0 & 0xFFFF) as u16).max(96);
                let suggested = unsafe { *(lparam.0 as *const RECT) };
                After::Reposition(suggested)
            }
            WM_MOUSEMOVE => {
                if !self.tracking_mouse {
                    let mut track = TRACKMOUSEEVENT {
                        cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                        dwFlags: TME_LEAVE,
                        hwndTrack: hwnd,
                        dwHoverTime: 0,
                    };
                    self.tracking_mouse = unsafe { TrackMouseEvent(&mut track) }.is_ok();
                }
                let (x, y) = self.to_dip(lparam);
                let hover = self.layout.hit_test(x, y);
                if hover != self.hover {
                    self.hover = hover;
                    After::Invalidate
                } else {
                    After::Result(LRESULT(0))
                }
            }
            WM_MOUSELEAVE => {
                self.tracking_mouse = false;
                self.hover = None;
                self.pressed = None;
                After::Invalidate
            }
            WM_LBUTTONDOWN => {
                let (x, y) = self.to_dip(lparam);
                self.pressed = self.layout.hit_test(x, y);
                self.keyboard_focus = false;
                After::Invalidate
            }
            WM_LBUTTONUP => {
                let (x, y) = self.to_dip(lparam);
                let released = self.layout.hit_test(x, y);
                let pressed = self.pressed.take();
                if released.is_some() && released == pressed {
                    self.focus = released;
                    self.activate(released)
                } else {
                    After::Invalidate
                }
            }
            WM_KEYDOWN => {
                let key = (wparam.0 & 0xFFFF) as u16;
                if key == VK_ESCAPE.0 {
                    return After::Hide;
                }
                if key == VK_RETURN.0 || key == VK_SPACE.0 {
                    return self.activate(self.focus);
                }
                let forward = if key == VK_TAB.0 {
                    Some(unsafe { GetKeyState(i32::from(VK_SHIFT.0)) } >= 0)
                } else if key == VK_DOWN.0 {
                    Some(true)
                } else if key == VK_UP.0 {
                    Some(false)
                } else {
                    None
                };
                match forward {
                    Some(forward) => {
                        self.keyboard_focus = true;
                        self.focus = self.layout.next_focus(self.focus, forward);
                        After::Invalidate
                    }
                    None => After::Default,
                }
            }
            WM_ACTIVATE => {
                if (wparam.0 & 0xFFFF) as u32 == WA_INACTIVE && self.visible {
                    After::Hide
                } else {
                    After::Default
                }
            }
            WM_TIMER if wparam.0 == TIMER_ID => {
                self.events.push(FlyoutEvent::Tick);
                After::Result(LRESULT(0))
            }
            _ => After::Default,
        }
    }
}

/// The single flyout instance owned by Runner.
pub struct FlyoutWindow {
    hwnd: HWND,
    state: Box<RefCell<WindowState>>,
}

extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        let pointer = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const RefCell<WindowState>;
        if pointer.is_null() {
            return DefWindowProcW(hwnd, message, wparam, lparam);
        }
        let cell = &*pointer;
        let after = match cell.try_borrow_mut() {
            Ok(mut state) => state.handle(hwnd, message, wparam, lparam),
            Err(_) if message == WM_PAINT => {
                let _ = ValidateRect(hwnd, None);
                let _ = InvalidateRect(hwnd, None, false);
                return LRESULT(0);
            }
            Err(_) => return DefWindowProcW(hwnd, message, wparam, lparam),
        };
        match after {
            After::Result(result) => result,
            After::Default => DefWindowProcW(hwnd, message, wparam, lparam),
            After::Invalidate => {
                let _ = InvalidateRect(hwnd, None, false);
                LRESULT(0)
            }
            After::Hide => {
                hide_window(hwnd, cell);
                LRESULT(0)
            }
            After::Reposition(rect) => {
                let _ = SetWindowPos(
                    hwnd,
                    HWND_TOPMOST,
                    rect.left,
                    rect.top,
                    rect.right - rect.left,
                    rect.bottom - rect.top,
                    SWP_NOACTIVATE,
                );
                if let Ok(mut state) = cell.try_borrow_mut() {
                    let dpi = state.dpi as f32;
                    state.renderer.resize(hwnd, dpi);
                }
                let _ = InvalidateRect(hwnd, None, false);
                LRESULT(0)
            }
        }
    }
}

fn hide_window(hwnd: HWND, cell: &RefCell<WindowState>) {
    {
        let Ok(mut state) = cell.try_borrow_mut() else {
            return;
        };
        if !state.visible {
            return;
        }
        state.visible = false;
        state.hidden_at = Some(Instant::now());
        state.hover = None;
        state.pressed = None;
        state.tracking_mouse = false;
        state.events.push(FlyoutEvent::Hidden);
    }
    unsafe {
        let _ = KillTimer(hwnd, TIMER_ID);
        let _ = ShowWindow(hwnd, SW_HIDE);
    }
}

fn monitor_rects(monitor: HMONITOR) -> Option<(RectI, RectI)> {
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if !unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() {
        return None;
    }
    let convert = |rect: RECT| RectI {
        left: rect.left,
        top: rect.top,
        right: rect.right,
        bottom: rect.bottom,
    };
    Some((convert(info.rcMonitor), convert(info.rcWork)))
}

impl FlyoutWindow {
    pub fn new() -> Result<Self> {
        let renderer = Renderer::new()?;
        let _scope = DpiScope::enter();
        unsafe {
            let instance = GetModuleHandleW(None).context("cannot resolve the Runner module")?;
            let class = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                style: CS_DROPSHADOW,
                lpfnWndProc: Some(window_proc),
                hInstance: instance.into(),
                hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
                lpszClassName: CLASS_NAME,
                ..Default::default()
            };
            RegisterClassExW(&class);

            let hwnd = CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
                CLASS_NAME,
                w!("Edge Optimizer quick flyout"),
                WS_POPUP,
                0,
                0,
                FLYOUT_WIDTH as i32,
                100,
                None,
                None,
                instance,
                None,
            );
            if hwnd.0 == 0 {
                anyhow::bail!(
                    "cannot create the flyout window: {}",
                    windows::core::Error::from_win32()
                );
            }
            let corner = DWMWCP_ROUND;
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_WINDOW_CORNER_PREFERENCE,
                &corner as *const DWM_WINDOW_CORNER_PREFERENCE as *const _,
                std::mem::size_of::<DWM_WINDOW_CORNER_PREFERENCE>() as u32,
            );

            let initial = FlyoutModel::default();
            let state = Box::new(RefCell::new(WindowState {
                layout: model::layout(&initial),
                model: initial,
                palette: Palette::current(),
                renderer,
                dpi: 96,
                anchor: (0, 0),
                hover: None,
                pressed: None,
                focus: None,
                keyboard_focus: false,
                tracking_mouse: false,
                visible: false,
                hidden_at: None,
                events: Vec::new(),
                render_failed: false,
            }));
            SetWindowLongPtrW(
                hwnd,
                GWLP_USERDATA,
                &*state as *const RefCell<WindowState> as isize,
            );
            Ok(Self { hwnd, state })
        }
    }

    pub fn is_visible(&self) -> bool {
        self.state.borrow().visible
    }

    /// Whether the flyout was dismissed within `window`; a tray click that
    /// dismissed it by stealing activation must not immediately reopen it.
    pub fn hidden_within(&self, window: Duration) -> bool {
        self.state
            .borrow()
            .hidden_at
            .is_some_and(|at| at.elapsed() < window)
    }

    pub fn has_events(&self) -> bool {
        !self.state.borrow().events.is_empty()
    }

    pub fn take_events(&self) -> Vec<FlyoutEvent> {
        std::mem::take(&mut self.state.borrow_mut().events)
    }

    /// Show the flyout for `model`, anchored at a physical screen point.
    pub fn show_at(&self, anchor: (i32, i32), model: FlyoutModel) {
        let _scope = DpiScope::enter();
        let monitor = unsafe {
            MonitorFromPoint(
                POINT {
                    x: anchor.0,
                    y: anchor.1,
                },
                MONITOR_DEFAULTTONEAREST,
            )
        };
        let (mut dpi_x, mut dpi_y) = (96u32, 96u32);
        if unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) }.is_err()
        {
            dpi_x = 96;
        }
        {
            let mut state = self.state.borrow_mut();
            state.dpi = dpi_x.max(96);
            state.anchor = anchor;
            state.palette = Palette::current();
            state.model = model;
            state.hover = None;
            state.pressed = None;
            state.focus = None;
            state.keyboard_focus = false;
            state.render_failed = false;
            state.visible = true;
            state.relayout();
        }
        self.apply_bounds(monitor, true);
        unsafe {
            let _ = SetForegroundWindow(self.hwnd);
            SetTimer(self.hwnd, TIMER_ID, TICK_MILLISECONDS, None);
        }
    }

    /// Replace the projected state; resizes and repaints when visible.
    pub fn set_model(&self, model: FlyoutModel) {
        let visible = {
            let mut state = self.state.borrow_mut();
            if state.model == model {
                return;
            }
            state.model = model;
            state.relayout();
            state.visible
        };
        if visible {
            let _scope = DpiScope::enter();
            let anchor = self.state.borrow().anchor;
            let monitor = unsafe {
                MonitorFromPoint(
                    POINT {
                        x: anchor.0,
                        y: anchor.1,
                    },
                    MONITOR_DEFAULTTONEAREST,
                )
            };
            self.apply_bounds(monitor, false);
            unsafe {
                let _ = InvalidateRect(self.hwnd, None, false);
            }
        }
    }

    pub fn hide(&self) {
        hide_window(self.hwnd, &self.state);
    }

    fn apply_bounds(&self, monitor: HMONITOR, show: bool) {
        let (anchor, size, margin) = {
            let state = self.state.borrow();
            let margin = (SCREEN_MARGIN as f32 * state.scale()).round() as i32;
            (state.anchor, state.pixel_size(), margin)
        };
        let Some((monitor_rect, work)) = monitor_rects(monitor) else {
            return;
        };
        let (x, y) = model::place(anchor, monitor_rect, work, size, margin);
        let mut flags = SWP_NOACTIVATE;
        if show {
            flags |= SWP_SHOWWINDOW;
        }
        unsafe {
            let _ = SetWindowPos(self.hwnd, HWND_TOPMOST, x, y, size.0, size.1, flags);
            let _ = InvalidateRect(self.hwnd, None, false);
        }
    }
}

impl Drop for FlyoutWindow {
    fn drop(&mut self) {
        unsafe {
            let _ = KillTimer(self.hwnd, TIMER_ID);
            SetWindowLongPtrW(self.hwnd, GWLP_USERDATA, 0);
            let _ = DestroyWindow(self.hwnd);
        }
    }
}
