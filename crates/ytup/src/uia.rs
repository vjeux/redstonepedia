//! Windows UI Automation over the owner's own Chrome: the accessibility API that screen readers
//! use. It reads what a page shows and presses its controls the way a person would; it reads no
//! cookie and no stored credential, and it needs no DevTools port (Chrome refuses one on the
//! default profile).

use anyhow::{Result, anyhow, bail};
use std::path::Path;
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx};
use windows::Win32::System::Variant::VARIANT;
use windows::Win32::UI::Accessibility::*;
use windows::core::BSTR;

pub struct Uia {
    a: IUIAutomation,
}

/// One element of a page, with the properties read in the same round trip.
pub struct Item {
    pub el: IUIAutomationElement,
    pub role: &'static str,
    pub name: String,
    pub aid: String,
    pub value: String,
    pub rect: [i32; 4],
    pub enabled: bool,
}

pub fn role_name(id: i32) -> &'static str {
    const NAMES: [&str; 39] = [
        "button", "calendar", "checkbox", "combobox", "edit", "link", "image", "listitem", "list", "menu", "menubar", "menuitem", "progressbar", "radio",
        "scrollbar", "slider", "spinner", "statusbar", "tab", "tabitem", "text", "toolbar", "tooltip", "tree", "treeitem", "custom", "group", "thumb",
        "datagrid", "dataitem", "document", "splitbutton", "window", "pane", "header", "headeritem", "table", "titlebar", "separator",
    ];
    usize::try_from(id - 50000).ok().and_then(|i| NAMES.get(i).copied()).unwrap_or("?")
}

impl Uia {
    pub fn new() -> Result<Uia> {
        // SAFETY: plain COM initialisation and object creation.
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let a: IUIAutomation = CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)?;
            Ok(Uia { a })
        }
    }

    /// The top-level Chrome windows: (element, title, window handle).
    pub fn chrome_windows(&self) -> Result<Vec<(IUIAutomationElement, String, HWND)>> {
        // SAFETY: UI Automation calls on elements this process owns references to.
        unsafe {
            let root = self.a.GetRootElement()?;
            let cond = self.a.CreatePropertyCondition(UIA_ClassNamePropertyId, &VARIANT::from("Chrome_WidgetWin_1"))?;
            let all = root.FindAll(TreeScope_Children, &cond)?;
            let mut out = Vec::new();
            for i in 0..all.Length()? {
                let e = all.GetElement(i)?;
                let title = e.CurrentName().map(|b| b.to_string()).unwrap_or_default();
                if title.contains("Google Chrome") {
                    let hwnd = e.CurrentNativeWindowHandle().unwrap_or_default();
                    out.push((e, title, hwnd));
                }
            }
            Ok(out)
        }
    }

    /// The Chrome window whose title contains `title` (any Chrome window when empty).
    pub fn window(&self, title: &str) -> Result<(IUIAutomationElement, String, HWND)> {
        let wins = self.chrome_windows()?;
        let n = wins.len();
        wins.into_iter().find(|(_, t, _)| t.contains(title)).ok_or_else(|| anyhow!("no Chrome window with {title:?} in its title ({n} Chrome windows)"))
    }

    /// Every element under `root`, in tree order, with role, name, automation id, value and box.
    pub fn all(&self, root: &IUIAutomationElement) -> Result<Vec<Item>> {
        // SAFETY: UI Automation calls; the cache request makes this one cross-process round trip.
        unsafe {
            let cr = self.a.CreateCacheRequest()?;
            for p in [UIA_NamePropertyId, UIA_ControlTypePropertyId, UIA_AutomationIdPropertyId, UIA_BoundingRectanglePropertyId, UIA_ValueValuePropertyId, UIA_IsEnabledPropertyId] {
                cr.AddProperty(p)?;
            }
            let cond = self.a.CreateTrueCondition()?;
            let arr = root.FindAllBuildCache(TreeScope_Descendants, &cond, &cr)?;
            let mut out = Vec::new();
            for i in 0..arr.Length()? {
                let el = arr.GetElement(i)?;
                let role = role_name(el.CachedControlType().map(|c| c.0).unwrap_or(0));
                let name = el.CachedName().map(|b| b.to_string()).unwrap_or_default();
                let aid = el.CachedAutomationId().map(|b| b.to_string()).unwrap_or_default();
                let value = el.GetCachedPropertyValue(UIA_ValueValuePropertyId).ok().map(|v| v.to_string()).unwrap_or_default();
                let enabled = el.CachedIsEnabled().map(|b| b.as_bool()).unwrap_or(true);
                let r: RECT = el.CachedBoundingRectangle().unwrap_or_default();
                out.push(Item { el, role, name, aid, value, rect: [r.left, r.top, r.right, r.bottom], enabled });
            }
            Ok(out)
        }
    }
}

/// Press an element: Invoke (buttons, links), else Toggle (checkboxes), else Select (radio,
/// list items), else the accessible default action.
pub fn press(el: &IUIAutomationElement) -> Result<&'static str> {
    // SAFETY: pattern queries and calls on a live element.
    unsafe {
        if let Ok(p) = el.GetCurrentPatternAs::<IUIAutomationInvokePattern>(UIA_InvokePatternId) {
            p.Invoke()?;
            return Ok("invoke");
        }
        if let Ok(p) = el.GetCurrentPatternAs::<IUIAutomationTogglePattern>(UIA_TogglePatternId) {
            p.Toggle()?;
            return Ok("toggle");
        }
        if let Ok(p) = el.GetCurrentPatternAs::<IUIAutomationSelectionItemPattern>(UIA_SelectionItemPatternId) {
            p.Select()?;
            return Ok("select");
        }
        if let Ok(p) = el.GetCurrentPatternAs::<IUIAutomationLegacyIAccessiblePattern>(UIA_LegacyIAccessiblePatternId) {
            p.DoDefaultAction()?;
            return Ok("default-action");
        }
    }
    bail!("the element has no invoke, toggle, select or default action")
}

/// The toggle state of a checkbox (`None` when it is not one).
pub fn toggled(el: &IUIAutomationElement) -> Option<bool> {
    // SAFETY: pattern query and property read on a live element.
    unsafe {
        let p = el.GetCurrentPatternAs::<IUIAutomationTogglePattern>(UIA_TogglePatternId).ok()?;
        p.CurrentToggleState().ok().map(|s| s == ToggleState_On)
    }
}

/// Close a window (its WindowPattern, like the title bar's X).
pub fn close(win: &IUIAutomationElement) -> Result<()> {
    // SAFETY: pattern query and call on a live element.
    unsafe {
        let p = win.GetCurrentPatternAs::<IUIAutomationWindowPattern>(UIA_WindowPatternId)?;
        p.Close()?;
    }
    Ok(())
}

/// Put text into an edit field (ValuePattern, as an on-screen keyboard would).
pub fn set_value(el: &IUIAutomationElement, text: &str) -> Result<()> {
    // SAFETY: pattern query and call on a live element.
    unsafe {
        let _ = el.SetFocus();
        let p = el.GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId)?;
        p.SetValue(&BSTR::from(text))?;
    }
    Ok(())
}

/// A picture of a window (PrintWindow with full content: works while the window is covered).
pub fn shot(hwnd: HWND, out: &Path) -> Result<()> {
    use windows::Win32::Graphics::Gdi::*;
    use windows::Win32::Storage::Xps::{PRINT_WINDOW_FLAGS, PrintWindow};
    use windows::Win32::UI::WindowsAndMessaging::GetWindowRect;
    // SAFETY: GDI calls on a device context and bitmap created and released here.
    let (w, h, bgra) = unsafe {
        let mut r = RECT::default();
        GetWindowRect(hwnd, &mut r)?;
        let (w, h) = (r.right - r.left, r.bottom - r.top);
        if w <= 0 || h <= 0 {
            bail!("the window has no area ({w}x{h}): minimized?");
        }
        let screen = GetDC(None);
        let dc = CreateCompatibleDC(Some(screen));
        let bmp = CreateCompatibleBitmap(screen, w, h);
        let old = SelectObject(dc, bmp.into());
        let _ = PrintWindow(hwnd, dc, PRINT_WINDOW_FLAGS(2));
        let mut bi = BITMAPINFO::default();
        bi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bi.bmiHeader.biWidth = w;
        bi.bmiHeader.biHeight = -h;
        bi.bmiHeader.biPlanes = 1;
        bi.bmiHeader.biBitCount = 32;
        bi.bmiHeader.biCompression = BI_RGB.0;
        let mut buf = vec![0u8; (w * h * 4) as usize];
        GetDIBits(dc, bmp, 0, h as u32, Some(buf.as_mut_ptr().cast()), &mut bi, DIB_RGB_COLORS);
        SelectObject(dc, old);
        let _ = DeleteObject(bmp.into());
        let _ = DeleteDC(dc);
        ReleaseDC(None, screen);
        (w as u32, h as u32, buf)
    };
    let rgb: Vec<u8> = bgra.chunks_exact(4).flat_map(|p| [p[2], p[1], p[0]]).collect();
    crate::png::write_rgb(out, w, h, &rgb)
}
