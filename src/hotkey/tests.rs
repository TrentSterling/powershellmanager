use super::*;
use std::cell::RefCell;
use std::collections::VecDeque;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, MOD_WIN,
};

type Registration = (i32, HOT_KEY_MODIFIERS, u32);

#[derive(Default)]
struct Api {
    register_error: bool,
    unregister_error: bool,
    registered: RefCell<Vec<Registration>>,
    released: RefCell<Vec<i32>>,
    messages: RefCell<VecDeque<i32>>,
}

impl HotkeyApi for Api {
    fn register(&self, id: i32, modifiers: HOT_KEY_MODIFIERS, key: u32) -> Result<(), String> {
        self.registered.borrow_mut().push((id, modifiers, key));
        if self.register_error {
            Err("key is already owned".into())
        } else {
            Ok(())
        }
    }
    fn unregister(&self, id: i32) -> Result<(), String> {
        self.released.borrow_mut().push(id);
        if self.unregister_error {
            Err("release failed".into())
        } else {
            Ok(())
        }
    }
    fn receive(&self) -> Option<i32> {
        self.messages.borrow_mut().pop_front()
    }
}

#[test]
fn owned_binding_coalesces_its_messages_ignores_other_ids_and_releases_once() {
    let api = Api {
        messages: RefCell::new([2, 1, 1, 99, 1].into_iter().collect()),
        ..Default::default()
    };
    let binding = Binding::register(&api, 1, MOD_ALT | MOD_CONTROL | MOD_NOREPEAT, 0x47).unwrap();
    assert_eq!(
        *api.registered.borrow(),
        [(1, MOD_ALT | MOD_CONTROL | MOD_NOREPEAT, 0x47)]
    );
    assert!(binding.poll());
    assert!(!binding.poll());
    *api.messages.borrow_mut() = [2, 99].into_iter().collect();
    assert!(!binding.poll());
    *api.messages.borrow_mut() = (0..100).map(|_| 99).chain(std::iter::once(1)).collect();
    assert!(!binding.poll());
    assert_eq!(api.messages.borrow().len(), 37);
    assert!(binding.poll());
    assert!(api.released.borrow().is_empty());
    drop(binding);
    assert_eq!(*api.released.borrow(), [1]);
}

#[test]
fn failed_registration_does_not_release_someone_elses_binding() {
    let api = Api {
        register_error: true,
        ..Default::default()
    };
    let error = Binding::register(&api, 1, MOD_CONTROL, 0x47).err().unwrap();
    assert_eq!(error, "key is already owned");
    assert!(api.released.borrow().is_empty());
}

#[test]
fn cleanup_survives_release_errors_and_unwinding() {
    crate::action_tests::init_logging();
    let api = Api {
        unregister_error: true,
        ..Default::default()
    };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _owned = Binding::register(&api, 1, MOD_CONTROL, 0x47).unwrap();
        panic!("isolated hotkey worker failure");
    }));
    assert!(result.is_err());
    assert_eq!(*api.released.borrow(), [1]);
}

#[test]
#[ignore = "Registers only a private F24 chord and posts to its own thread queue; no desktop input"]
fn native_audit_private_hotkey_receives_owned_messages_and_releases_after_collision() {
    use windows::Win32::{
        Foundation::{LPARAM, WPARAM},
        System::Threading::GetCurrentThreadId,
        UI::WindowsAndMessaging::PostThreadMessageW,
    };
    const ID: i32 = 0x7000;
    const KEY: u32 = 0x87; // F24, with every modifier; never send a physical key press.
    let modifiers = MOD_ALT | MOD_CONTROL | MOD_SHIFT | MOD_WIN | MOD_NOREPEAT;
    let binding =
        Binding::register(&NativeHotkeys, ID, modifiers, KEY).expect("private owned audit chord");
    assert!(!binding.poll());
    let thread = unsafe { GetCurrentThreadId() };
    unsafe {
        PostThreadMessageW(thread, WM_HOTKEY, WPARAM((ID + 1) as usize), LPARAM(0)).unwrap();
        PostThreadMessageW(thread, WM_HOTKEY, WPARAM(ID as usize), LPARAM(0)).unwrap();
        PostThreadMessageW(thread, WM_HOTKEY, WPARAM(ID as usize), LPARAM(0)).unwrap();
    }
    assert!(binding.poll());
    assert!(!binding.poll());
    assert!(std::thread::spawn(
        move || Binding::register(&NativeHotkeys, ID, modifiers, KEY).is_err()
    )
    .join()
    .unwrap());
    drop(binding);
    let available = std::thread::spawn(move || {
        let retry = Binding::register(&NativeHotkeys, ID, modifiers, KEY)
            .expect("owned chord was released");
        drop(retry);
        assert!(NativeHotkeys.unregister(ID).is_err());
    });
    available.join().unwrap();
    println!("NATIVE HOTKEY PASS: private F24 chord, owned-thread messages, collision refusal and release; no desktop input");
}
