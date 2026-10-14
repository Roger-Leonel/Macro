use korvon_core::{wildcard_match, Action, InputSink, MouseButton, PressState};
use std::mem::size_of;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::core::PWSTR;
use windows::Win32::Foundation::{CloseHandle, BOOL, HWND, LPARAM};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetSystemMetrics, GetWindowTextW, GetWindowThreadProcessId, IsIconic,
    IsWindowVisible, SetForegroundWindow, SetWindowPos, ShowWindow, SM_CXVIRTUALSCREEN,
    SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SWP_NOACTIVATE, SWP_NOZORDER,
    SW_RESTORE,
};

/// Reproduz ações reais no sistema via `SendInput`.
#[derive(Default)]
pub struct SendInputSink;

fn send(inputs: &[INPUT]) -> Result<(), String> {
    if inputs.is_empty() {
        return Ok(());
    }
    let sent = unsafe { SendInput(inputs, size_of::<INPUT>() as i32) };
    if sent as usize == inputs.len() {
        Ok(())
    } else {
        Err("SendInput foi bloqueado (janela elevada/UIPI?)".into())
    }
}

fn mouse(dx: i32, dy: i32, data: u32, flags: MOUSE_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT { dx, dy, mouseData: data, dwFlags: flags, time: 0, dwExtraInfo: crate::SELF_MARK },
        },
    }
}

fn key(vk: u16, scan: u16, flags: KEYBD_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(vk),
                wScan: scan,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: crate::SELF_MARK,
            },
        },
    }
}

fn is_extended(vk: u16) -> bool {
    matches!(vk, 0x21..=0x28 | 0x2D | 0x2E | 0x5B..=0x5D | 0x6F | 0x90 | 0xA3 | 0xA5)
}

fn normalize(v: i32, origin: i32, size: i32) -> i32 {
    let span = (size - 1).max(1) as i64;
    (((v - origin) as i64 * 65535 + span / 2) / span).clamp(0, 65535) as i32
}

impl InputSink for SendInputSink {
    fn perform(&mut self, action: &Action) -> Result<(), String> {
        match action {
            Action::MouseMove { x, y, .. } => {
                let (vx, vy, vw, vh) = unsafe {
                    (
                        GetSystemMetrics(SM_XVIRTUALSCREEN),
                        GetSystemMetrics(SM_YVIRTUALSCREEN),
                        GetSystemMetrics(SM_CXVIRTUALSCREEN),
                        GetSystemMetrics(SM_CYVIRTUALSCREEN),
                    )
                };
                send(&[mouse(
                    normalize(*x, vx, vw),
                    normalize(*y, vy, vh),
                    0,
                    MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK,
                )])
            }
            Action::MouseButton { button, state } => {
                let down = *state == PressState::Down;
                let (flags, data) = match button {
                    MouseButton::Left => (if down { MOUSEEVENTF_LEFTDOWN } else { MOUSEEVENTF_LEFTUP }, 0),
                    MouseButton::Right => (if down { MOUSEEVENTF_RIGHTDOWN } else { MOUSEEVENTF_RIGHTUP }, 0),
                    MouseButton::Middle => (if down { MOUSEEVENTF_MIDDLEDOWN } else { MOUSEEVENTF_MIDDLEUP }, 0),
                    MouseButton::X1 => (if down { MOUSEEVENTF_XDOWN } else { MOUSEEVENTF_XUP }, 1),
                    MouseButton::X2 => (if down { MOUSEEVENTF_XDOWN } else { MOUSEEVENTF_XUP }, 2),
                };
                send(&[mouse(0, 0, data, flags)])
            }
            Action::Wheel { delta, horizontal } => send(&[mouse(
                0,
                0,
                *delta as u32,
                if *horizontal { MOUSEEVENTF_HWHEEL } else { MOUSEEVENTF_WHEEL },
            )]),
            Action::Key { vk, state } => {
                let scan = unsafe { MapVirtualKeyW(*vk as u32, MAPVK_VK_TO_VSC) } as u16;
                let mut flags = KEYBD_EVENT_FLAGS(0);
                if *state == PressState::Up {
                    flags |= KEYEVENTF_KEYUP;
                }
                if is_extended(*vk) {
                    flags |= KEYEVENTF_EXTENDEDKEY;
                }
                send(&[key(*vk, scan, flags)])
            }
            Action::Text { text } => {
                let mut inputs = Vec::new();
                for unit in text.encode_utf16() {
                    inputs.push(key(0, unit, KEYEVENTF_UNICODE));
                    inputs.push(key(0, unit, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP));
                }
                send(&inputs)
            }
            Action::FocusWindow { exe, title, rect } => focus_window(exe, title, *rect),
            // Controle de fluxo e esperas são tratados pelo motor, nunca chegam aqui.
            Action::Wait { .. }
            | Action::Label { .. }
            | Action::Goto { .. }
            | Action::Repeat { .. }
            | Action::SetVar { .. }
            | Action::If { .. }
            | Action::Click { .. }
            | Action::KeyPress { .. }
            | Action::Chars { .. } => Ok(()),
        }
    }
}

struct Search<'a> {
    exe: &'a str,
    title: &'a str,
    found: Option<HWND>,
}

fn window_title(hwnd: HWND) -> String {
    let mut buf = [0u16; 512];
    let n = unsafe { GetWindowTextW(hwnd, &mut buf) };
    String::from_utf16_lossy(&buf[..n.max(0) as usize])
}

fn window_exe(hwnd: HWND) -> Option<String> {
    unsafe {
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len);
        let _ = CloseHandle(h);
        ok.ok()?;
        let path = String::from_utf16_lossy(&buf[..len as usize]);
        path.rsplit(['\\', '/']).next().map(str::to_string)
    }
}

unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let s = &mut *(lparam.0 as *mut Search);
    if !IsWindowVisible(hwnd).as_bool() {
        return BOOL(1);
    }
    let title = window_title(hwnd);
    if title.is_empty() || !(s.title.is_empty() || wildcard_match(s.title, &title)) {
        return BOOL(1);
    }
    if !s.exe.is_empty() {
        match window_exe(hwnd) {
            Some(e) if wildcard_match(s.exe, &e) => {}
            _ => return BOOL(1),
        }
    }
    s.found = Some(hwnd);
    BOOL(0) // para a enumeração
}

/// Procura (por até 3 s) a janela e a traz para frente; posiciona se `rect` for dado.
fn focus_window(exe: &str, title: &str, rect: Option<[i32; 4]>) -> Result<(), String> {
    for _ in 0..30 {
        let mut s = Search { exe, title, found: None };
        // EnumWindows devolve erro quando o callback interrompe; o resultado é irrelevante.
        let _ = unsafe { EnumWindows(Some(enum_proc), LPARAM(&mut s as *mut _ as isize)) };
        if let Some(hwnd) = s.found {
            unsafe {
                if IsIconic(hwnd).as_bool() {
                    let _ = ShowWindow(hwnd, SW_RESTORE);
                }
                if let Some([x, y, w, h]) = rect {
                    let _ = SetWindowPos(hwnd, None, x, y, w, h, SWP_NOZORDER | SWP_NOACTIVATE);
                }
                let _ = SetForegroundWindow(hwnd);
            }
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    Err(format!("janela não encontrada: {exe} | {title}"))
}
