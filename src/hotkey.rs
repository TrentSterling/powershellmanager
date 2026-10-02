//! A thread-owned hotkey registration with cleanup on every exit path.
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS,
};
use windows::Win32::UI::WindowsAndMessaging::{PeekMessageW, MSG, PM_REMOVE, WM_HOTKEY};

#[cfg(test)]
mod tests;

pub(crate) trait HotkeyApi {
    fn register(&self, id: i32, modifiers: HOT_KEY_MODIFIERS, key: u32) -> Result<(), String>;
    fn unregister(&self, id: i32) -> Result<(), String>;
    fn receive(&self) -> Option<i32>;
}

pub(crate) struct NativeHotkeys;

impl HotkeyApi for NativeHotkeys {
    fn register(&self, id: i32, modifiers: HOT_KEY_MODIFIERS, key: u32) -> Result<(), String> {
        unsafe { RegisterHotKey(None, id, modifiers, key) }.map_err(|error| error.to_string())
    }

    fn unregister(&self, id: i32) -> Result<(), String> {
        unsafe { UnregisterHotKey(None, id) }.map_err(|error| error.to_string())
    }

    fn receive(&self) -> Option<i32> {
        let mut message = MSG::default();
        if unsafe { PeekMessageW(&mut message, None, WM_HOTKEY, WM_HOTKEY, PM_REMOVE) }.as_bool() {
            Some(message.wParam.0 as i32)
        } else {
            None
        }
    }
}

pub(crate) struct Binding<'a> {
    api: &'a dyn HotkeyApi,
    id: i32,
    // Thread registrations must be released on the thread that owns them.
    _thread: std::marker::PhantomData<std::rc::Rc<()>>,
}

impl<'a> Binding<'a> {
    pub(crate) fn register(
        api: &'a dyn HotkeyApi,
        id: i32,
        modifiers: HOT_KEY_MODIFIERS,
        key: u32,
    ) -> Result<Self, String> {
        api.register(id, modifiers, key)?;
        Ok(Self {
            api,
            id,
            _thread: std::marker::PhantomData,
        })
    }

    /// Coalesce repeated presses while leaving menu commands in their own queue.
    pub(crate) fn poll(&self) -> bool {
        let mut pending = false;
        for _ in 0..64 {
            let Some(id) = self.api.receive() else {
                break;
            };
            pending |= id == self.id;
        }
        pending
    }
}

impl Drop for Binding<'_> {
    fn drop(&mut self) {
        if let Err(error) = self.api.unregister(self.id) {
            log::warn!("Could not release hotkey {}: {error}", self.id);
        }
    }
}
