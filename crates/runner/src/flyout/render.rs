//! Direct2D and DirectWrite drawing for the quick flyout.

use super::model::{ButtonStyle, Element, Layout, RectF, TextRole};
use anyhow::{Context, Result};
use windows::core::w;
use windows::Win32::Foundation::{D2DERR_RECREATE_TARGET, HWND, RECT};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_ALPHA_MODE_IGNORE, D2D1_COLOR_F, D2D1_PIXEL_FORMAT, D2D_RECT_F, D2D_SIZE_U,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1CreateFactory, ID2D1Factory, ID2D1HwndRenderTarget, ID2D1SolidColorBrush, ID2D1StrokeStyle,
    D2D1_DRAW_TEXT_OPTIONS_CLIP, D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_FEATURE_LEVEL_DEFAULT,
    D2D1_HWND_RENDER_TARGET_PROPERTIES, D2D1_PRESENT_OPTIONS_NONE, D2D1_RENDER_TARGET_PROPERTIES,
    D2D1_RENDER_TARGET_TYPE_DEFAULT, D2D1_RENDER_TARGET_USAGE_NONE, D2D1_ROUNDED_RECT,
};
use windows::Win32::Graphics::DirectWrite::{
    DWriteCreateFactory, IDWriteFactory, IDWriteFontCollection, IDWriteTextFormat,
    DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL,
    DWRITE_FONT_WEIGHT, DWRITE_FONT_WEIGHT_NORMAL, DWRITE_FONT_WEIGHT_SEMI_BOLD,
    DWRITE_MEASURING_MODE_NATURAL, DWRITE_PARAGRAPH_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT,
    DWRITE_TEXT_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_LEADING, DWRITE_TEXT_ALIGNMENT_TRAILING,
    DWRITE_TRIMMING, DWRITE_TRIMMING_GRANULARITY_CHARACTER, DWRITE_WORD_WRAPPING_NO_WRAP,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Gdi::{
    GetSysColor, COLOR_GRAYTEXT, COLOR_HIGHLIGHT, COLOR_HIGHLIGHTTEXT, COLOR_WINDOW,
    COLOR_WINDOWTEXT, SYS_COLOR_INDEX,
};
use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};
use windows::Win32::UI::Accessibility::{HCF_HIGHCONTRASTON, HIGHCONTRASTW};
use windows::Win32::UI::WindowsAndMessaging::{
    GetClientRect, SystemParametersInfoW, SPI_GETHIGHCONTRAST, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
};

const CORNER_RADIUS: f32 = 4.0;
const ROW_TEXT_INSET: f32 = 12.0;

/// Colors for one theme; high contrast uses the system colors unchanged.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub background: D2D1_COLOR_F,
    pub hover: D2D1_COLOR_F,
    pub pressed: D2D1_COLOR_F,
    pub selected: D2D1_COLOR_F,
    pub text: D2D1_COLOR_F,
    pub muted: D2D1_COLOR_F,
    pub accent: D2D1_COLOR_F,
    pub accent_text: D2D1_COLOR_F,
    pub danger: D2D1_COLOR_F,
    pub divider: D2D1_COLOR_F,
    pub focus: D2D1_COLOR_F,
    pub high_contrast: bool,
}

const fn rgb(value: u32) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: ((value >> 16) & 0xFF) as f32 / 255.0,
        g: ((value >> 8) & 0xFF) as f32 / 255.0,
        b: (value & 0xFF) as f32 / 255.0,
        a: 1.0,
    }
}

fn system_color(index: SYS_COLOR_INDEX) -> D2D1_COLOR_F {
    let colorref = unsafe { GetSysColor(index) };
    D2D1_COLOR_F {
        r: (colorref & 0xFF) as f32 / 255.0,
        g: ((colorref >> 8) & 0xFF) as f32 / 255.0,
        b: ((colorref >> 16) & 0xFF) as f32 / 255.0,
        a: 1.0,
    }
}

impl Palette {
    const DARK: Palette = Palette {
        background: rgb(0x202020),
        hover: rgb(0x2D2D2D),
        pressed: rgb(0x262626),
        selected: rgb(0x1F3647),
        text: rgb(0xFFFFFF),
        muted: rgb(0xB5B5B5),
        accent: rgb(0x60CDFF),
        accent_text: rgb(0x000000),
        danger: rgb(0xFF99A4),
        divider: rgb(0x3A3A3A),
        focus: rgb(0xFFFFFF),
        high_contrast: false,
    };

    const LIGHT: Palette = Palette {
        background: rgb(0xF3F3F3),
        hover: rgb(0xE6E6E6),
        pressed: rgb(0xDADADA),
        selected: rgb(0xD6E8F7),
        text: rgb(0x1A1A1A),
        muted: rgb(0x5C5C5C),
        accent: rgb(0x005FB8),
        accent_text: rgb(0xFFFFFF),
        danger: rgb(0xC42B1C),
        divider: rgb(0xD1D1D1),
        focus: rgb(0x000000),
        high_contrast: false,
    };

    /// Palette for the current high-contrast and taskbar theme settings.
    pub fn current() -> Self {
        if high_contrast_enabled() {
            let text = system_color(COLOR_WINDOWTEXT);
            let highlight = system_color(COLOR_HIGHLIGHT);
            let window = system_color(COLOR_WINDOW);
            return Palette {
                background: window,
                hover: highlight,
                pressed: highlight,
                selected: highlight,
                text,
                muted: system_color(COLOR_GRAYTEXT),
                accent: highlight,
                accent_text: system_color(COLOR_HIGHLIGHTTEXT),
                danger: text,
                divider: text,
                focus: text,
                high_contrast: true,
            };
        }
        if system_uses_light_theme() {
            Self::LIGHT
        } else {
            Self::DARK
        }
    }
}

fn high_contrast_enabled() -> bool {
    let mut settings = HIGHCONTRASTW {
        cbSize: std::mem::size_of::<HIGHCONTRASTW>() as u32,
        ..Default::default()
    };
    let queried = unsafe {
        SystemParametersInfoW(
            SPI_GETHIGHCONTRAST,
            settings.cbSize,
            Some(&mut settings as *mut HIGHCONTRASTW as *mut _),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    };
    queried.is_ok() && (settings.dwFlags.0 & HCF_HIGHCONTRASTON.0) != 0
}

/// The flyout sits on the taskbar, so it follows the system (taskbar)
/// theme rather than the app theme.
fn system_uses_light_theme() -> bool {
    let mut value = 0u32;
    let mut size = std::mem::size_of::<u32>() as u32;
    let result = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize"),
            w!("SystemUsesLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut value as *mut u32 as *mut _),
            Some(&mut size as *mut u32),
        )
    };
    result.is_ok() && value != 0
}

struct TextFormats {
    title: IDWriteTextFormat,
    heading: IDWriteTextFormat,
    body: IDWriteTextFormat,
    caption: IDWriteTextFormat,
    caption_trailing: IDWriteTextFormat,
    button_center: IDWriteTextFormat,
}

impl TextFormats {
    fn new(factory: &IDWriteFactory) -> Result<Self> {
        let make = |size: f32, weight: DWRITE_FONT_WEIGHT, alignment: DWRITE_TEXT_ALIGNMENT| {
            create_format(factory, size, weight, alignment)
        };
        Ok(Self {
            title: make(
                16.0,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_TEXT_ALIGNMENT_LEADING,
            )?,
            heading: make(
                12.0,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_TEXT_ALIGNMENT_LEADING,
            )?,
            body: make(
                13.0,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_TEXT_ALIGNMENT_LEADING,
            )?,
            caption: make(
                12.0,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_TEXT_ALIGNMENT_LEADING,
            )?,
            caption_trailing: make(
                12.0,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_TEXT_ALIGNMENT_TRAILING,
            )?,
            button_center: make(
                13.0,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_TEXT_ALIGNMENT_CENTER,
            )?,
        })
    }

    fn for_role(&self, role: TextRole) -> &IDWriteTextFormat {
        match role {
            TextRole::Title => &self.title,
            TextRole::Heading => &self.heading,
            TextRole::Body => &self.body,
            TextRole::Caption => &self.caption,
        }
    }
}

fn create_format(
    factory: &IDWriteFactory,
    size: f32,
    weight: DWRITE_FONT_WEIGHT,
    alignment: DWRITE_TEXT_ALIGNMENT,
) -> Result<IDWriteTextFormat> {
    unsafe {
        let format = factory
            .CreateTextFormat(
                w!("Segoe UI"),
                None::<&IDWriteFontCollection>,
                weight,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                size,
                w!("en-us"),
            )
            .context("cannot create a DirectWrite text format")?;
        format.SetTextAlignment(alignment)?;
        format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
        format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
        let sign = factory.CreateEllipsisTrimmingSign(&format)?;
        let trimming = DWRITE_TRIMMING {
            granularity: DWRITE_TRIMMING_GRANULARITY_CHARACTER,
            delimiter: 0,
            delimiterCount: 0,
        };
        format.SetTrimming(&trimming, &sign)?;
        Ok(format)
    }
}

/// Interaction state the renderer needs for one frame.
#[derive(Debug, Clone, Copy, Default)]
pub struct Interaction {
    pub hover: Option<usize>,
    pub pressed: Option<usize>,
    pub focus: Option<usize>,
    pub show_focus: bool,
}

/// Device-independent Direct2D and DirectWrite objects plus the lazily
/// created, device-dependent HWND render target.
pub struct Renderer {
    factory: ID2D1Factory,
    formats: TextFormats,
    target: Option<(ID2D1HwndRenderTarget, ID2D1SolidColorBrush)>,
}

fn rect_f(rect: &RectF) -> D2D_RECT_F {
    D2D_RECT_F {
        left: rect.left,
        top: rect.top,
        right: rect.right,
        bottom: rect.bottom,
    }
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}

impl Renderer {
    pub fn new() -> Result<Self> {
        unsafe {
            let factory: ID2D1Factory = D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)
                .context("cannot create the Direct2D factory")?;
            let write: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)
                .context("cannot create the DirectWrite factory")?;
            Ok(Self {
                formats: TextFormats::new(&write)?,
                factory,
                target: None,
            })
        }
    }

    fn client_size(hwnd: HWND) -> Result<D2D_SIZE_U> {
        let mut rect = RECT::default();
        unsafe { GetClientRect(hwnd, &mut rect) }.context("cannot read the flyout size")?;
        Ok(D2D_SIZE_U {
            width: (rect.right - rect.left).max(1) as u32,
            height: (rect.bottom - rect.top).max(1) as u32,
        })
    }

    fn ensure_target(
        &mut self,
        hwnd: HWND,
        dpi: f32,
    ) -> Result<&(ID2D1HwndRenderTarget, ID2D1SolidColorBrush)> {
        if self.target.is_none() {
            let properties = D2D1_RENDER_TARGET_PROPERTIES {
                r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_IGNORE,
                },
                dpiX: dpi,
                dpiY: dpi,
                usage: D2D1_RENDER_TARGET_USAGE_NONE,
                minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
            };
            let hwnd_properties = D2D1_HWND_RENDER_TARGET_PROPERTIES {
                hwnd,
                pixelSize: Self::client_size(hwnd)?,
                presentOptions: D2D1_PRESENT_OPTIONS_NONE,
            };
            unsafe {
                let target = self
                    .factory
                    .CreateHwndRenderTarget(&properties, &hwnd_properties)
                    .context("cannot create the flyout render target")?;
                let brush = target
                    .CreateSolidColorBrush(&rgb(0), None)
                    .context("cannot create a Direct2D brush")?;
                self.target = Some((target, brush));
            }
        }
        self.target
            .as_ref()
            .context("the flyout render target is unavailable")
    }

    /// Match the render target to a new client size and DPI.
    pub fn resize(&mut self, hwnd: HWND, dpi: f32) {
        if let Some((target, _)) = &self.target {
            let resized = Self::client_size(hwnd)
                .and_then(|size| unsafe { target.Resize(&size) }.map_err(Into::into));
            unsafe { target.SetDpi(dpi, dpi) };
            if resized.is_err() {
                self.target = None;
            }
        }
    }

    pub fn render(
        &mut self,
        hwnd: HWND,
        dpi: f32,
        layout: &Layout,
        palette: &Palette,
        interaction: Interaction,
    ) -> Result<()> {
        let (target, brush) = {
            let (target, brush) = self.ensure_target(hwnd, dpi)?;
            (target.clone(), brush.clone())
        };
        let formats = &self.formats;
        unsafe {
            target.BeginDraw();
            target.Clear(Some(&palette.background as *const D2D1_COLOR_F));
            for (index, element) in layout.elements.iter().enumerate() {
                draw_element(
                    &target,
                    &brush,
                    formats,
                    palette,
                    element,
                    index,
                    interaction,
                );
            }
            match target.EndDraw(None, None) {
                Err(error) if error.code() == D2DERR_RECREATE_TARGET => {
                    self.target = None;
                    Ok(())
                }
                other => other.context("Direct2D could not draw the flyout"),
            }
        }
    }
}

unsafe fn draw_text(
    target: &ID2D1HwndRenderTarget,
    brush: &ID2D1SolidColorBrush,
    format: &IDWriteTextFormat,
    text: &str,
    rect: &RectF,
    color: &D2D1_COLOR_F,
) {
    brush.SetColor(color);
    target.DrawText(
        &wide(text),
        format,
        &rect_f(rect),
        brush,
        D2D1_DRAW_TEXT_OPTIONS_CLIP,
        DWRITE_MEASURING_MODE_NATURAL,
    );
}

unsafe fn draw_element(
    target: &ID2D1HwndRenderTarget,
    brush: &ID2D1SolidColorBrush,
    formats: &TextFormats,
    palette: &Palette,
    element: &Element,
    index: usize,
    interaction: Interaction,
) {
    match element {
        Element::Text {
            rect,
            text,
            role,
            muted,
        } => {
            let color = if *muted {
                &palette.muted
            } else {
                &palette.text
            };
            draw_text(target, brush, formats.for_role(*role), text, rect, color);
        }
        Element::Divider { rect } => {
            brush.SetColor(&palette.divider);
            target.FillRectangle(&rect_f(rect), brush);
        }
        Element::Button {
            rect,
            label,
            detail,
            style,
            enabled,
            ..
        } => {
            let hovered = interaction.hover == Some(index);
            let pressed = interaction.pressed == Some(index);
            let fill = match style {
                ButtonStyle::Accent => Some(palette.accent),
                ButtonStyle::RowSelected => Some(palette.selected),
                _ if pressed => Some(palette.pressed),
                _ if hovered => Some(palette.hover),
                _ => None,
            };
            let shape = D2D1_ROUNDED_RECT {
                rect: rect_f(rect),
                radiusX: CORNER_RADIUS,
                radiusY: CORNER_RADIUS,
            };
            if let Some(fill) = fill {
                brush.SetColor(&fill);
                target.FillRoundedRectangle(&shape, brush);
            }
            if palette.high_contrast || matches!(style, ButtonStyle::Subtle | ButtonStyle::Danger) {
                brush.SetColor(&palette.divider);
                target.DrawRoundedRectangle(&shape, brush, 1.0, None::<&ID2D1StrokeStyle>);
            }
            if interaction.show_focus && interaction.focus == Some(index) {
                let ring = D2D1_ROUNDED_RECT {
                    rect: rect_f(&rect.inset(-2.0, -2.0)),
                    radiusX: CORNER_RADIUS + 2.0,
                    radiusY: CORNER_RADIUS + 2.0,
                };
                brush.SetColor(&palette.focus);
                target.DrawRoundedRectangle(&ring, brush, 2.0, None::<&ID2D1StrokeStyle>);
            }

            let highlighted_text = palette.high_contrast && (hovered || pressed);
            let mut color = match style {
                ButtonStyle::Accent => palette.accent_text,
                ButtonStyle::Danger => palette.danger,
                _ if highlighted_text => palette.accent_text,
                _ => palette.text,
            };
            if !enabled && !matches!(style, ButtonStyle::RowSelected) {
                color = palette.muted;
            }
            match style {
                ButtonStyle::Row | ButtonStyle::RowSelected => {
                    let text_rect = rect.inset(ROW_TEXT_INSET, 0.0);
                    draw_text(target, brush, &formats.body, label, &text_rect, &color);
                    if let Some(detail) = detail {
                        draw_text(
                            target,
                            brush,
                            &formats.caption_trailing,
                            detail,
                            &text_rect,
                            &palette.muted,
                        );
                    }
                }
                _ => {
                    let text_rect = rect.inset(6.0, 0.0);
                    draw_text(
                        target,
                        brush,
                        &formats.button_center,
                        label,
                        &text_rect,
                        &color,
                    );
                }
            }
        }
    }
}
