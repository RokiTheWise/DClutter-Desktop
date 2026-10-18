//  DClutter — swipe-based file triage
//  Copyright 2026 Dexter Jethro Enriquez
//  Licensed under the Apache License, Version 2.0.

//! How many fingers are on the touchpad, read from the touchpad itself.
//!
//! The WebView only sees two-finger swipes as scrolling: it never learns
//! when fingers touch or lift. macOS swipe handling depends on exactly
//! that (a fresh gesture per touch, ignoring momentum once fingers lift,
//! "let go" to file on the shelf), so DClutter reads the precision
//! touchpad's own reports through Windows Raw Input and passes the finger
//! count on.

#[cfg(windows)]
pub use windows_impl::watch;

/// The HID usages a precision touchpad reports contacts with
/// (Digitizer usage page).
#[cfg_attr(not(windows), allow(dead_code))]
mod usage {
    pub const DIGITIZER_PAGE: u16 = 0x0D;
    pub const TOUCH_PAD: u16 = 0x05;
    pub const TIP_SWITCH: u16 = 0x42;
    pub const CONTACT_ID: u16 = 0x51;
    pub const CONTACT_COUNT: u16 = 0x54;
}

/// Turns a touchpad's reports into "fingers down" counts.
///
/// A precision touchpad describes each frame as a contact count plus one
/// entry per contact (its id and whether it's touching). A frame can fit
/// in one report ("parallel" mode) or be spread over several ("hybrid"
/// mode), where only the first report carries the count and the rest say
/// zero. Entries past the frame's count are unused slots.
#[derive(Debug, Default)]
pub struct FrameAssembler {
    remaining: usize,
    touching: Vec<u32>,
    fingers: usize,
}

impl FrameAssembler {
    /// Feeds one report: its contact count, as the touchpad sent it, and
    /// each contact slot's (id, touching). Returns the new number of
    /// fingers down when a frame completes with a different count.
    pub fn report(&mut self, count: u32, slots: &[(u32, bool)]) -> Option<usize> {
        if count > 0 {
            // A new frame.
            self.remaining = count as usize;
            self.touching.clear();
        } else if self.remaining == 0 {
            // Not part of any frame we know about.
            return None;
        }
        let used = self.remaining.min(slots.len());
        for &(id, touching) in &slots[..used] {
            if touching && !self.touching.contains(&id) {
                self.touching.push(id);
            }
        }
        self.remaining -= used;
        if self.remaining > 0 {
            return None;
        }
        let fingers = self.touching.len();
        (fingers != self.fingers).then(|| {
            self.fingers = fingers;
            fingers
        })
    }
}

#[cfg(windows)]
mod windows_impl {
    use std::collections::HashMap;
    use std::ffi::c_void;
    use std::sync::{Mutex, OnceLock};

    use windows_sys::Win32::Devices::HumanInterfaceDevice::{
        HIDP_CAPS, HIDP_STATUS_SUCCESS, HIDP_VALUE_CAPS, HidP_GetCaps, HidP_GetUsageValue, HidP_GetUsages,
        HidP_GetValueCaps, HidP_Input,
    };
    use windows_sys::Win32::Foundation::{HANDLE, HWND, LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::UI::Input::{
        GetRawInputData, GetRawInputDeviceInfoW, HRAWINPUT, RAWINPUT, RAWINPUTDEVICE, RAWINPUTHEADER,
        RID_INPUT, RIDEV_INPUTSINK, RIDI_PREPARSEDDATA, RIM_TYPEHID, RegisterRawInputDevices,
    };
    use windows_sys::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, WM_INPUT};

    use super::{FrameAssembler, usage};

    type Callback = Box<dyn Fn(usize) + Send + Sync>;

    /// What one touchpad's reports look like, worked out once from its
    /// HID description.
    struct Device {
        preparsed: Vec<u8>,
        /// The link collection holding the contact count.
        count_link: u16,
        /// One link collection per contact slot, in report order.
        contact_links: Vec<u16>,
        frames: FrameAssembler,
    }

    struct Watcher {
        window: HWND,
        on_change: Callback,
        devices: HashMap<isize, Option<Device>>,
    }

    // SAFETY: the window handle is only used on the window's own thread,
    // from inside its window procedure; Send is needed for the static.
    unsafe impl Send for Watcher {}

    static WATCHER: OnceLock<Mutex<Watcher>> = OnceLock::new();
    const SUBCLASS_ID: usize = 0xDC1;

    /// Starts reporting the number of fingers on any precision touchpad to
    /// `on_change`, from `window`'s thread. Call once, on that thread.
    /// Touchpads that don't describe their contacts report nothing, and
    /// the caller should carry on without.
    pub fn watch(window: isize, on_change: impl Fn(usize) + Send + Sync + 'static) -> Result<(), String> {
        let window = window as HWND;
        let device = RAWINPUTDEVICE {
            usUsagePage: usage::DIGITIZER_PAGE,
            usUsage: usage::TOUCH_PAD,
            // Delivered even while another window has focus, so the
            // foreground check below decides, not Windows' routing.
            dwFlags: RIDEV_INPUTSINK,
            hwndTarget: window,
        };
        WATCHER
            .set(Mutex::new(Watcher { window, on_change: Box::new(on_change), devices: HashMap::new() }))
            .map_err(|_| "The touchpad is already being watched".to_string())?;
        // SAFETY: `device` is valid for the call, and the subclass is
        // installed on the calling thread, which owns `window`.
        unsafe {
            if RegisterRawInputDevices(&device, 1, std::mem::size_of::<RAWINPUTDEVICE>() as u32) == 0 {
                return Err(std::io::Error::last_os_error().to_string());
            }
            if SetWindowSubclass(window, Some(window_proc), SUBCLASS_ID, 0) == 0 {
                return Err("Couldn't listen to the window's input".into());
            }
        }
        Ok(())
    }

    unsafe extern "system" fn window_proc(
        window: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
        _id: usize,
        _data: usize,
    ) -> LRESULT {
        if message == WM_INPUT {
            // SAFETY: lparam is the HRAWINPUT Windows sent with WM_INPUT.
            unsafe { handle_input(lparam as HRAWINPUT) };
        }
        // SAFETY: passes the message on unchanged, as subclasses must.
        unsafe { DefSubclassProc(window, message, wparam, lparam) }
    }

    unsafe fn handle_input(input: HRAWINPUT) {
        let Some(watcher) = WATCHER.get() else { return };
        let Ok(mut watcher) = watcher.lock() else { return };
        // Fingers on the touchpad while another app is in front aren't ours.
        // SAFETY: no arguments.
        if unsafe { GetForegroundWindow() } != watcher.window {
            return;
        }

        // SAFETY: the buffer is sized by the first call, and RAWINPUT's
        // header is read only after the data has been copied into it.
        let buffer = unsafe {
            let header = std::mem::size_of::<RAWINPUTHEADER>() as u32;
            let mut size = 0u32;
            GetRawInputData(input, RID_INPUT, std::ptr::null_mut(), &mut size, header);
            // u64 elements keep the buffer aligned for RAWINPUT.
            let mut buffer = vec![0u64; (size as usize).div_ceil(8)];
            if GetRawInputData(input, RID_INPUT, buffer.as_mut_ptr() as *mut c_void, &mut size, header) != size {
                return;
            }
            buffer
        };
        let raw = buffer.as_ptr() as *const RAWINPUT;
        // SAFETY: `raw` points at a complete RAWINPUT copied above.
        let (kind, device) = unsafe { ((*raw).header.dwType, (*raw).header.hDevice) };
        if kind != RIM_TYPEHID {
            return;
        }
        // SAFETY: dwType says the union holds HID data; the reports follow
        // it inside the same buffer, dwCount of them, dwSizeHid bytes each.
        let reports = unsafe {
            let hid = &(*raw).data.hid;
            let start = hid.bRawData.as_ptr();
            let size = hid.dwSizeHid as usize;
            (0..hid.dwCount as usize)
                .map(|i| std::slice::from_raw_parts(start.add(i * size), size).to_vec())
                .collect::<Vec<_>>()
        };

        let watcher = &mut *watcher;
        let entry = watcher.devices.entry(device as isize).or_insert_with(|| unsafe { describe(device) });
        let Some(device) = entry else { return };
        for mut report in reports {
            let (count, slots) = unsafe { read_report(device, &mut report) };
            if let Some(fingers) = device.frames.report(count, &slots) {
                (watcher.on_change)(fingers);
            }
        }
    }

    /// Learns where a touchpad puts its contact count and contact slots.
    /// `None` for a device that doesn't describe contacts.
    unsafe fn describe(device: HANDLE) -> Option<Device> {
        // SAFETY: the buffer is sized by the first call.
        let preparsed = unsafe {
            let mut size = 0u32;
            GetRawInputDeviceInfoW(device, RIDI_PREPARSEDDATA, std::ptr::null_mut(), &mut size);
            let mut buffer = vec![0u8; size as usize];
            if GetRawInputDeviceInfoW(device, RIDI_PREPARSEDDATA, buffer.as_mut_ptr() as *mut c_void, &mut size)
                != size
            {
                return None;
            }
            buffer
        };
        let data = preparsed.as_ptr() as isize;

        // SAFETY: the caps arrays are sized from the device's own counts.
        let caps = unsafe {
            let mut caps: HIDP_CAPS = std::mem::zeroed();
            if HidP_GetCaps(data, &mut caps) != HIDP_STATUS_SUCCESS {
                return None;
            }
            let mut len = caps.NumberInputValueCaps;
            let mut values: Vec<HIDP_VALUE_CAPS> = vec![std::mem::zeroed(); len as usize];
            if HidP_GetValueCaps(HidP_Input, values.as_mut_ptr(), &mut len, data) != HIDP_STATUS_SUCCESS {
                return None;
            }
            values.truncate(len as usize);
            values
        };

        let mut count_link = None;
        let mut contact_links = Vec::new();
        for cap in &caps {
            if cap.UsagePage != usage::DIGITIZER_PAGE || cap.IsRange {
                continue;
            }
            // SAFETY: IsRange is false, so NotRange is the live variant.
            let used = unsafe { cap.Anonymous.NotRange.Usage };
            if used == usage::CONTACT_COUNT {
                count_link = Some(cap.LinkCollection);
            } else if used == usage::CONTACT_ID && !contact_links.contains(&cap.LinkCollection) {
                contact_links.push(cap.LinkCollection);
            }
        }
        if contact_links.is_empty() {
            return None;
        }
        Some(Device { preparsed, count_link: count_link?, contact_links, frames: FrameAssembler::default() })
    }

    /// One report's contact count and its slots' (id, touching).
    unsafe fn read_report(device: &Device, report: &mut [u8]) -> (u32, Vec<(u32, bool)>) {
        let data = device.preparsed.as_ptr() as isize;
        let len = report.len() as u32;
        let ptr = report.as_mut_ptr();
        // SAFETY: `report` is a whole input report from this device, and
        // the usage list is sized for every usage a slot can hold.
        unsafe {
            let mut count = 0u32;
            HidP_GetUsageValue(HidP_Input, usage::DIGITIZER_PAGE, device.count_link, usage::CONTACT_COUNT, &mut count, data, ptr, len);
            let slots = device
                .contact_links
                .iter()
                .map(|&link| {
                    let mut id = 0u32;
                    HidP_GetUsageValue(HidP_Input, usage::DIGITIZER_PAGE, link, usage::CONTACT_ID, &mut id, data, ptr, len);
                    let mut usages = [0u16; 16];
                    let mut used = usages.len() as u32;
                    let ok = HidP_GetUsages(HidP_Input, usage::DIGITIZER_PAGE, link, usages.as_mut_ptr(), &mut used, data, ptr, len)
                        == HIDP_STATUS_SUCCESS;
                    let touching = ok && usages[..used as usize].contains(&usage::TIP_SWITCH);
                    (id, touching)
                })
                .collect();
            (count, slots)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::FrameAssembler;

    #[test]
    fn counts_fingers_in_a_single_report_frame() {
        let mut frames = FrameAssembler::default();
        // Two fingers down; the third slot is unused.
        assert_eq!(frames.report(2, &[(0, true), (1, true), (0, false)]), Some(2));
        // Same again: no change to report.
        assert_eq!(frames.report(2, &[(0, true), (1, true), (0, false)]), None);
    }

    #[test]
    fn assembles_a_frame_spread_over_several_reports() {
        let mut frames = FrameAssembler::default();
        // Hybrid mode: three contacts, one slot per report, count only first.
        assert_eq!(frames.report(3, &[(4, true)]), None);
        assert_eq!(frames.report(0, &[(5, true)]), None);
        assert_eq!(frames.report(0, &[(6, true)]), Some(3));
    }

    #[test]
    fn reports_the_lift_when_every_contact_lets_go() {
        let mut frames = FrameAssembler::default();
        frames.report(2, &[(0, true), (1, true)]);
        assert_eq!(frames.report(2, &[(0, true), (1, false)]), Some(1));
        assert_eq!(frames.report(1, &[(0, false), (9, false)]), Some(0));
    }

    #[test]
    fn ignores_unused_slots_past_the_count() {
        let mut frames = FrameAssembler::default();
        // Slot 2 holds stale data from an earlier frame.
        assert_eq!(frames.report(1, &[(3, true), (7, true)]), Some(1));
    }

    #[test]
    fn ignores_stray_reports_outside_a_frame() {
        let mut frames = FrameAssembler::default();
        assert_eq!(frames.report(0, &[(1, true)]), None);
    }
}
