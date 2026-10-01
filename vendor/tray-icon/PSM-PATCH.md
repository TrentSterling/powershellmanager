# PowerShell Manager Windows tray patch

Source: tray-icon 0.19.3 from the crates.io package already pinned in Cargo.lock.
Upstream: https://github.com/tauri-apps/tray-icon
Licenses: upstream Apache-2.0 OR MIT; license files are preserved here.

The Windows implementation supplied zero for `NOTIFYICONDATAW.cbSize` in its
add, modify-icon, modify-tooltip and delete calls. All four constructors now set
the structure's byte size, as required by the Win32 contract:
https://learn.microsoft.com/en-us/windows/win32/api/shellapi/ns-shellapi-notifyicondataw

Failed icon registration also destroys its newly created hidden window before
returning the original error. `WM_DESTROY` releases that window's owned data.

The original and patched versions both pass the owned-icon audit on this Windows
11 desktop. The sandbox rejects Explorer registration for both versions. This
patch follows the documented API contract; it is not claimed to remove the
sandbox restriction. Upstream's unused `Accel` helper is retained with a specific
dead-code annotation so a path dependency does not introduce build warnings.

Verification: `cargo test native_audit -- --ignored --nocapture --test-threads=1`.
The tray audit creates, themes, rebuilds and removes its own icons. It never
injects global desktop input or changes another application's resources.
