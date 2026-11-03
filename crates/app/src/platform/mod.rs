pub mod menu;
pub mod window;

pub use menu::init_app_menu;
pub use window::default_window_options;

#[cfg(target_os = "macos")]
#[allow(deprecated, unexpected_cfgs)]
pub fn set_dock_icon() {
    use cocoa::appkit::NSApplication;
    use cocoa::base::nil;
    use objc::{class, msg_send, sel, sel_impl};

    unsafe {
        let icon_bytes = include_bytes!("../../resources/app_icon.png");
        let ns_data: cocoa::base::id = msg_send![
            class!(NSData),
            dataWithBytes: icon_bytes.as_ptr() as *const std::ffi::c_void
            length: icon_bytes.len()
        ];
        if !ns_data.is_null() {
            let ns_image: cocoa::base::id = msg_send![class!(NSImage), alloc];
            let ns_image: cocoa::base::id = msg_send![ns_image, initWithData: ns_data];
            if !ns_image.is_null() {
                let app = NSApplication::sharedApplication(nil);
                let _: () = msg_send![app, setApplicationIconImage: ns_image];
                let _: () = msg_send![ns_image, release];
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn set_dock_icon() {}
