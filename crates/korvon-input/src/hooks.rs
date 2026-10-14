use korvon_core::{Action, MouseButton, PressState};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;
use windows::Win32::Foundation::{HINSTANCE, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;

/// Evento capturado, com timestamp monotônico em ms desde o início dos hooks.
#[derive(Debug, Clone)]
pub struct RawEvent {
    pub t_ms: u64,
    pub action: Action,
    /// Posição do cursor (eventos de mouse).
    pub pos: Option<(i32, i32)>,
}

static SENDER: Mutex<Option<Sender<RawEvent>>> = Mutex::new(None);
static START: OnceLock<Instant> = OnceLock::new();

const LLKHF_INJECTED: u32 = 0x10;
const LLMHF_INJECTED: u32 = 0x01;

fn emit(action: Action, pos: Option<(i32, i32)>) {
    let t_ms = START.get().map(|s| s.elapsed().as_millis() as u64).unwrap_or(0);
    if let Ok(guard) = SENDER.lock() {
        if let Some(tx) = guard.as_ref() {
            let _ = tx.send(RawEvent { t_ms, action, pos });
        }
    }
}

unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let info = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
        if info.flags.0 & LLKHF_INJECTED == 0 {
            let state = match wparam.0 as u32 {
                WM_KEYDOWN | WM_SYSKEYDOWN => Some(PressState::Down),
                WM_KEYUP | WM_SYSKEYUP => Some(PressState::Up),
                _ => None,
            };
            if let Some(state) = state {
                emit(Action::Key { vk: info.vkCode as u16, state }, None);
            }
        }
    }
    CallNextHookEx(HHOOK::default(), code, wparam, lparam)
}

unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let info = &*(lparam.0 as *const MSLLHOOKSTRUCT);
        if info.flags & LLMHF_INJECTED == 0 {
            let hi = (info.mouseData >> 16) as u16;
            let button = |b, s| Some(Action::MouseButton { button: b, state: s });
            let action = match wparam.0 as u32 {
                WM_MOUSEMOVE => Some(Action::MouseMove { x: info.pt.x, y: info.pt.y, ms: 0 }),
                WM_LBUTTONDOWN => button(MouseButton::Left, PressState::Down),
                WM_LBUTTONUP => button(MouseButton::Left, PressState::Up),
                WM_RBUTTONDOWN => button(MouseButton::Right, PressState::Down),
                WM_RBUTTONUP => button(MouseButton::Right, PressState::Up),
                WM_MBUTTONDOWN => button(MouseButton::Middle, PressState::Down),
                WM_MBUTTONUP => button(MouseButton::Middle, PressState::Up),
                WM_XBUTTONDOWN | WM_XBUTTONUP => {
                    let b = if hi == 1 { MouseButton::X1 } else { MouseButton::X2 };
                    let s = if wparam.0 as u32 == WM_XBUTTONDOWN {
                        PressState::Down
                    } else {
                        PressState::Up
                    };
                    button(b, s)
                }
                WM_MOUSEWHEEL => Some(Action::Wheel { delta: hi as i16 as i32, horizontal: false }),
                WM_MOUSEHWHEEL => Some(Action::Wheel { delta: hi as i16 as i32, horizontal: true }),
                _ => None,
            };
            if let Some(a) = action {
                emit(a, Some((info.pt.x, info.pt.y)));
            }
        }
    }
    CallNextHookEx(HHOOK::default(), code, wparam, lparam)
}

/// Instala os hooks globais numa thread dedicada (com laço de mensagens) e
/// devolve o receptor de eventos. Só deve ser chamada uma vez por processo.
pub fn start_hooks() -> Result<Receiver<RawEvent>, String> {
    let (tx, rx) = channel();
    let (ready_tx, ready_rx) = channel::<Result<(), String>>();
    START.get_or_init(Instant::now);
    *SENDER.lock().map_err(|e| e.to_string())? = Some(tx);

    std::thread::spawn(move || unsafe {
        let hmod = match GetModuleHandleW(None) {
            Ok(h) => HINSTANCE(h.0),
            Err(e) => {
                let _ = ready_tx.send(Err(e.to_string()));
                return;
            }
        };
        let kb = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), hmod, 0);
        let ms = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), hmod, 0);
        match (kb, ms) {
            (Ok(_), Ok(_)) => {
                let _ = ready_tx.send(Ok(()));
            }
            (a, b) => {
                let msg = a.err().or(b.err()).map(|e| e.to_string()).unwrap_or_default();
                let _ = ready_tx.send(Err(msg));
                return;
            }
        }
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    });

    ready_rx
        .recv()
        .map_err(|e| e.to_string())?
        .map(|_| rx)
        .map_err(|e| format!("falha ao instalar hooks de teclado/mouse: {e}"))
}
