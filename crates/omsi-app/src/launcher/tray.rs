//! The launcher in the system tray while a game runs (the Windows 11 concept): its window
//! leaves the taskbar, and the tray icon's menu brings it back, opens the game's log, stops
//! the game or quits the launcher (the game keeps running). Windows has the tray; elsewhere
//! there is none (`Tray::new` gives `None`) and the launcher's window is minimized instead.

use std::sync::mpsc::Receiver;

/// What the tray icon's menu asked for.
#[cfg_attr(not(windows), allow(dead_code))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayCmd {
    /// "Show launcher", or a double click on the icon.
    Show,
    OpenLog,
    StopGame,
    Quit,
}

/// The icon in the tray, while it lives (dropped: the icon is gone when the drop returns).
pub struct Tray {
    rx: Receiver<TrayCmd>,
    #[cfg(windows)]
    window: imp::Window,
}

impl Tray {
    /// Put the icon into the tray: `tip` is its tooltip, `head` the menu's first line (the
    /// game that runs). `None` where there is no tray.
    pub fn new(tip: &str, head: &str) -> Option<Tray> {
        #[cfg(windows)]
        {
            let (tx, rx) = std::sync::mpsc::channel();
            let window = imp::start(tip, head, tx)?;
            Some(Tray { rx, window })
        }
        #[cfg(not(windows))]
        {
            let _ = (tip, head);
            None
        }
    }

    /// The next thing the menu asked for, if any.
    pub fn take(&self) -> Option<TrayCmd> {
        self.rx.try_recv().ok()
    }

    /// A new first line for the menu (shown the next time it opens).
    pub fn set_head(&self, head: &str) {
        #[cfg(windows)]
        imp::set_head(&self.window, head);
        #[cfg(not(windows))]
        let _ = head;
    }
}

#[cfg(windows)]
impl Drop for Tray {
    fn drop(&mut self) {
        imp::stop(&self.window);
    }
}

/// The menu's lines, in order, with what each asks for (`None`: a divider).
#[cfg_attr(not(windows), allow(dead_code))]
pub const MENU: [(Option<TrayCmd>, &str); 5] = [
    (Some(TrayCmd::Show), "Show launcher"),
    (Some(TrayCmd::OpenLog), "Open log"),
    (None, ""),
    (Some(TrayCmd::StopGame), "Stop game"),
    (Some(TrayCmd::Quit), "Quit launcher"),
];

#[cfg(windows)]
mod imp {
    use super::{TrayCmd, MENU};
    use std::cell::RefCell;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::mpsc::{Receiver, Sender};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use windows::core::{w, PCWSTR};
    use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::Shell::{
        Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_SETVERSION,
        NOTIFYICONDATAW, NOTIFYICONDATAW_0, NOTIFYICON_VERSION,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DestroyWindow,
        DispatchMessageW, GetCursorPos, GetMessageW, GetSystemMetrics, LoadIconW, LoadImageW,
        PostMessageW, PostQuitMessage, RegisterClassW, RegisterWindowMessageW, SetForegroundWindow,
        TrackPopupMenu, TranslateMessage, HICON, IDI_APPLICATION, IMAGE_ICON, LR_SHARED, MF_GRAYED,
        MF_SEPARATOR, MF_STRING, MSG, SM_CXSMICON, SM_CYSMICON, TPM_BOTTOMALIGN, TPM_RETURNCMD,
        TPM_RIGHTBUTTON, WINDOW_EX_STYLE, WINDOW_STYLE, WM_APP, WM_CLOSE, WM_CONTEXTMENU,
        WM_DESTROY, WM_LBUTTONDBLCLK, WM_NULL, WM_RBUTTONUP, WM_USER, WNDCLASSW,
    };

    /// The message the icon sends its window.
    const WM_TRAY: u32 = WM_APP + 1;
    /// What the icon says when Enter or Space is pressed on it after the keyboard reached it
    /// (`NIN_KEYSELECT` in the system's headers; sent once the icon has asked for the newer
    /// behaviour with `NIM_SETVERSION`).
    const NIN_KEYSELECT: u32 = WM_USER + 1;
    /// How long the launcher waits for the icon's thread (to start, and to take the icon away).
    const WAIT: Duration = Duration::from_secs(2);

    /// The message Explorer sends every top window when the taskbar is made anew (Explorer
    /// restarted): the icon must be added again then.
    static TASKBAR_CREATED: AtomicU32 = AtomicU32::new(0);

    /// The icon's hidden window (its handle as a number: it is used from the launcher's
    /// thread only to close it), the menu's first line, and the end of its thread.
    pub struct Window {
        hwnd: isize,
        head: Arc<Mutex<Vec<u16>>>,
        done: Receiver<()>,
    }

    struct Own {
        tx: Sender<TrayCmd>,
        head: Arc<Mutex<Vec<u16>>>,
        icon: NOTIFYICONDATAW,
        /// The icon took the newer behaviour: its menu comes with `WM_CONTEXTMENU` (by mouse
        /// or keyboard), no longer with the right button's `WM_RBUTTONUP`.
        versioned: bool,
    }

    thread_local! {
        static OWN: RefCell<Option<Own>> = const { RefCell::new(None) };
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// The icon on a thread of its own, with the window and the message loop it needs.
    pub fn start(tip: &str, head: &str, tx: Sender<TrayCmd>) -> Option<Window> {
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        // (never sent on: it is dropped when the thread ends, which is what `stop` waits for)
        let (done_tx, done) = std::sync::mpsc::channel::<()>();
        let tip = tip.to_string();
        let head = Arc::new(Mutex::new(wide(head)));
        let shared = head.clone();
        std::thread::Builder::new()
            .name("launcher tray".into())
            .spawn(move || {
                let _done = done_tx;
                let opened = unsafe { open(&tip) };
                let sent = ready_tx.send(opened.map(|(h, ..)| h.0 as isize)).is_ok();
                let Some((hwnd, icon, versioned)) = opened else {
                    return;
                };
                if !sent {
                    // (the launcher gave up waiting: nobody owns this icon, so it goes now)
                    unsafe {
                        let _ = DestroyWindow(hwnd);
                    }
                    return;
                }
                OWN.with(|o| {
                    *o.borrow_mut() = Some(Own {
                        tx,
                        head: shared,
                        icon,
                        versioned,
                    })
                });
                let mut msg = MSG::default();
                unsafe {
                    while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                        let _ = TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }
                }
            })
            .ok()?;
        let hwnd = ready_rx.recv_timeout(WAIT).ok().flatten()?;
        Some(Window { hwnd, head, done })
    }

    pub fn set_head(w: &Window, head: &str) {
        if let Ok(mut h) = w.head.lock() {
            *h = wide(head);
        }
    }

    /// The icon goes: its window is closed on its own thread, and the launcher waits until
    /// that thread has ended (the icon is then out of the tray, also when the launcher quits
    /// right after).
    pub fn stop(w: &Window) {
        unsafe {
            let _ = PostMessageW(Some(HWND(w.hwnd as *mut _)), WM_CLOSE, WPARAM(0), LPARAM(0));
        }
        let _ = w.done.recv_timeout(WAIT);
    }

    /// The icon's window and the icon in the tray: the window, the icon's data (to add it
    /// again) and whether it took the newer behaviour.
    unsafe fn open(tip: &str) -> Option<(HWND, NOTIFYICONDATAW, bool)> {
        let module = GetModuleHandleW(None).ok()?;
        let instance = HINSTANCE(module.0);
        let class = w!("DepotLauncherTray");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(proc),
            hInstance: instance,
            lpszClassName: class,
            ..Default::default()
        };
        // (registered once a process: a second icon finds the class there)
        RegisterClassW(&wc);
        TASKBAR_CREATED.store(
            RegisterWindowMessageW(w!("TaskbarCreated")),
            Ordering::Relaxed,
        );
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class,
            w!("Project Depot"),
            WINDOW_STYLE(0),
            0,
            0,
            0,
            0,
            None,
            None,
            Some(instance),
            None,
        )
        .ok()?;
        // the program's own icon (the executable's first icon resource), small; shared, so
        // the system keeps it and there is nothing to free
        let (cx, cy) = (GetSystemMetrics(SM_CXSMICON), GetSystemMetrics(SM_CYSMICON));
        let icon = LoadImageW(
            Some(instance),
            PCWSTR(1 as *const u16),
            IMAGE_ICON,
            cx,
            cy,
            LR_SHARED,
        )
        .map(|h| HICON(h.0))
        .or_else(|_| LoadIconW(None, IDI_APPLICATION))
        .unwrap_or_default();
        let mut nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: 1,
            uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
            uCallbackMessage: WM_TRAY,
            hIcon: icon,
            ..Default::default()
        };
        for (d, s) in nid.szTip.iter_mut().zip(tip.encode_utf16().take(127)) {
            *d = s;
        }
        if !Shell_NotifyIconW(NIM_ADD, &nid).as_bool() {
            let _ = DestroyWindow(hwnd);
            return None;
        }
        let versioned = set_version(&nid);
        Some((hwnd, nid, versioned))
    }

    /// Ask for the newer icon behaviour (keyboard selection, the menu by `WM_CONTEXTMENU`).
    unsafe fn set_version(nid: &NOTIFYICONDATAW) -> bool {
        let v = NOTIFYICONDATAW {
            Anonymous: NOTIFYICONDATAW_0 {
                uVersion: NOTIFYICON_VERSION,
            },
            ..*nid
        };
        Shell_NotifyIconW(NIM_SETVERSION, &v).as_bool()
    }

    fn send(cmd: TrayCmd) {
        OWN.with(|o| {
            if let Some(o) = o.borrow().as_ref() {
                let _ = o.tx.send(cmd);
            }
        });
    }

    unsafe fn menu(hwnd: HWND) {
        let Ok(m) = CreatePopupMenu() else { return };
        let head = OWN
            .with(|o| {
                o.borrow()
                    .as_ref()
                    .and_then(|o| o.head.lock().ok().map(|h| h.clone()))
            })
            .unwrap_or_else(|| wide("Project Depot"));
        let _ = AppendMenuW(m, MF_STRING | MF_GRAYED, 0, PCWSTR(head.as_ptr()));
        let _ = AppendMenuW(m, MF_SEPARATOR, 0, PCWSTR::null());
        let labels: Vec<Vec<u16>> = MENU
            .iter()
            .map(|x| wide(omsi_ui::tr(x.1).as_ref()))
            .collect();
        for (k, (cmd, _)) in MENU.iter().enumerate() {
            if cmd.is_some() {
                let _ = AppendMenuW(m, MF_STRING, k + 1, PCWSTR(labels[k].as_ptr()));
            } else {
                let _ = AppendMenuW(m, MF_SEPARATOR, 0, PCWSTR::null());
            }
        }
        let mut at = POINT::default();
        let _ = GetCursorPos(&mut at);
        // (the menu closes when clicked away from only if its window is in front)
        let _ = SetForegroundWindow(hwnd);
        let chosen = TrackPopupMenu(
            m,
            TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_BOTTOMALIGN,
            at.x,
            at.y,
            None,
            hwnd,
            None,
        )
        .0 as usize;
        let _ = PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0));
        let _ = DestroyMenu(m);
        if let Some(cmd) = chosen
            .checked_sub(1)
            .and_then(|k| MENU.get(k))
            .and_then(|x| x.0)
        {
            send(cmd);
        }
    }

    unsafe extern "system" fn proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match msg {
            WM_TRAY => {
                let versioned = OWN.with(|o| o.borrow().as_ref().is_some_and(|o| o.versioned));
                match (lparam.0 as u32) & 0xffff {
                    // (a double click, or Enter or Space on the icon reached by the keyboard)
                    WM_LBUTTONDBLCLK | NIN_KEYSELECT => send(TrayCmd::Show),
                    WM_CONTEXTMENU => menu(hwnd),
                    WM_RBUTTONUP if !versioned => menu(hwnd),
                    _ => {}
                }
                LRESULT(0)
            }
            WM_CLOSE => {
                let _ = DestroyWindow(hwnd);
                LRESULT(0)
            }
            WM_DESTROY => {
                let nid = NOTIFYICONDATAW {
                    cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                    hWnd: hwnd,
                    uID: 1,
                    ..Default::default()
                };
                let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
                PostQuitMessage(0);
                LRESULT(0)
            }
            m if m != 0 && m == TASKBAR_CREATED.load(Ordering::Relaxed) => {
                OWN.with(|o| {
                    if let Some(o) = o.borrow_mut().as_mut() {
                        if Shell_NotifyIconW(NIM_ADD, &o.icon).as_bool() {
                            o.versioned = set_version(&o.icon);
                        }
                    }
                });
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}
