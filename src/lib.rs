//! A library for writing tmux plugins in Rust. To use tmux plugins
//! you will need to use a fork of tmux that has plugin support.
//!
//! # Basic usage
//!
//! Use one of the macros exported by this library to define
//! a plugin. For instance, to define a [format plugin](./macro.format_plugin.html):
//!
//! ```rust
//! // This plugin does the same thing as the built in "window_width".
//! use tmux_plugin::format_plugin;
//! use std::ffi::CString;
//!
//! format_plugin!(b"my_window_width\0", |format_tree| {
//!     CString::new(
//!         format!("{}", unsafe { *(*format_tree).w }.sx)
//!     ).unwrap()
//! });
//! # fn main() {}
//! ```
//!
//! Then compile your crate [as a dynamic library](https://doc.rust-lang.org/cargo/reference/manifest.html#building-dynamic-or-static-libraries), by adding this to your `[lib]` section in your `Cargo.toml`:
//! ```toml
//! [lib]
//! name = "..." # Your plugin's name
//! crate-type = ["cdylib"]
//! ```

pub mod tmux;
mod tmux_bindings;

#[doc(hidden)]
pub use libc;

/// Defines a new format variable.
///
/// This macro takes two arguments: The name of the variable (as a null-terminated byte string),
/// and a function to calculate that variable's value.
/// This function will be passed a tmux [`format_tree`](./tmux/struct.format_tree.html) object,
/// and should return a type that implements [`AsRef`](https://doc.rust-lang.org/std/convert/trait.AsRef.html)`<`[`CStr`](https://doc.rust-lang.org/std/ffi/struct.CStr.html)`>`,
/// such as [`CStr`](https://doc.rust-lang.org/std/ffi/struct.CStr.html)
/// or [`CString`](https://doc.rust-lang.org/std/ffi/struct.CString.html).
///
/// For example:
///
/// ```rust
/// // This plugin does the same thing as the builtin "window_width".
/// use tmux_plugin::format_plugin;
/// use std::ffi::CString;
///
/// format_plugin!(b"my_window_width\0", |format_tree| {
///     CString::new(
///         format!("{}", unsafe { *(*format_tree).w }.sx)
///     ).unwrap()
/// });
/// # fn main() {}
/// ```
#[macro_export]
macro_rules! format_plugin {
    ($name:expr, |$ft:ident| $body:block) => {
        mod tmux_format_plugin {
            use super::*;
            use $crate::tmux;

            $crate::__plugin!(
                format,
                tmux::format_plugin {
                    name: $name as *const u8 as *const ::libc::c_char,
                    cb: Some(plugin_format_cb),
                }
            );
            use std::convert::AsRef;
            fn format_plugin_body(
                $ft: *mut tmux::format_tree,
                fe: *mut tmux::format_entry,
            ) -> impl ::std::convert::AsRef<::std::ffi::CStr> {
                $body
            }

            pub unsafe extern "C" fn plugin_format_cb(
                $ft: *mut tmux::format_tree,
                fe: *mut tmux::format_entry,
            ) {
                let return_str = format_plugin_body($ft, fe);
                let dup = $crate::libc::strdup(return_str.as_ref().as_ptr());
                (*fe).value = dup;
            }
        }
    };
}

/// Defines a new format function.
///
/// This macro takes two arguments: The name of the function (as a null-terminated byte string),
/// and the function body itself. The function recives a `&`[`CStr`](https://doc.rust-lang.org/std/ffi/struct.CStr.html).
/// as an argument, and should return a type that implements [`AsRef`](https://doc.rust-lang.org/std/convert/trait.AsRef.html)`<`[`CStr`](https://doc.rust-lang.org/std/ffi/struct.CStr.html)`>`,
/// such as [`CStr`](https://doc.rust-lang.org/std/ffi/struct.CStr.html)
/// or [`CString`](https://doc.rust-lang.org/std/ffi/struct.CString.html).
///
/// This function will then be usable in tmux format strings by surrounding it with a pair of colons.
/// For instance, if you define a format function that reverses it's input named `reverse`, you can use it as `#{:reverse:session_name}` to display the builtin tmux `session_name` variable backwards.
///
/// For example:
///
/// ```rust
/// use tmux_plugin::format_function_plugin;
/// use std::ffi::CString;
///
/// format_function_plugin!(b"trim\0", |arg| {
///     match arg.to_str() {
///       Ok(string) => {
///         CString::new(string.trim().to_owned())
///             .expect("Does not contain null bytes, as the source was a valid CString")
///       },
///       Err(_) => CString::new("Invalid UTF-8 in input").unwrap(),
///     }
/// });
/// # fn main() {}
/// ```
#[macro_export]
macro_rules! format_function_plugin {
    ($name:expr, |$arg:ident| $body:block) => {
        mod tmux_format_function_plugin {
            use super::*;
            use $crate::tmux;

            $crate::__plugin!(
                function,
                tmux::function_plugin {
                    name: $name as *const u8 as *const $crate::libc::c_char,
                    cb: Some(plugin_format_function_cb),
                }
            );

            use std::convert::AsRef;
            fn format_function_plugin_body(
                $arg: &::std::ffi::CStr,
            ) -> impl ::std::convert::AsRef<::std::ffi::CStr> {
                $body
            }

            pub unsafe extern "C" fn plugin_format_function_cb(
                $arg: *const $crate::libc::c_char,
            ) -> *mut $crate::libc::c_char {
                let argument = ::std::ffi::CStr::from_ptr($arg);
                let return_str = format_function_plugin_body(argument);
                $crate::libc::strdup(return_str.as_ref().as_ptr())
            }
        }
    };
}

/// Defines a new notification callback.
///
/// This macro has two variants: If passed just a callback function, that function
/// is registered as a callback for all hook events. If passed a null-terminated hook name
/// and a callback function, that function will be registered as a callback for that specific
/// hook event. The callback function itself will in either case recive an argument of type
/// [`*mut`](https://doc.rust-lang.org/std/primitive.pointer.html)` `[`notify_entry`](./tmux/struct.notify_entry.html).
///
/// For example:
///
///
/// ```rust
/// use tmux_plugin::notification_plugin;
/// use std::ffi::{CStr, CString};
///
/// // Enforce that window names are lower case.
/// notification_plugin!(b"window-renamed\0", |notify_entry| {
///     let window = unsafe { (*notify_entry).window };
///     let window_name = unsafe { CStr::from_ptr((*window).name) };
///     let lowercase_name = window_name
///         .to_string_lossy()
///         .into_owned()
///         .to_lowercase();
///     let c_string = CString::new(lowercase_name)
///         .expect("Does not contain null bytes, as the source was a valid C str");
///     unsafe {
///         // Free the old name, and duplicate the new name so
///         // that tmux can later free it safely.
///         libc::free((*window).name as *mut _);
///         (*window).name = libc::strdup(c_string.as_ptr())
///     }
/// });
/// # fn main() {}
/// ```
#[macro_export]
macro_rules! notification_plugin {
    (|$arg:ident| $body:block) => {
        notification_plugin!(::std::ptr::null(), |$arg| $body);
    };
    ($name:expr, |$arg:ident| $body:block) => {
        mod tmux_notification_plugin {
            use super::*;
            use $crate::tmux;

            $crate::__plugin!(
                notify,
                tmux::notification_plugin {
                    event: $name as *const u8 as *const $crate::libc::c_char,
                    cb: Some(notify_cb),
                }
            );

            fn notify_plugin_body($arg: *mut tmux::notify_entry) {
                $body
            }

            pub unsafe extern "C" fn notify_cb($arg: *mut tmux::notify_entry) {
                notify_plugin_body($arg)
            }
        }
    };
}

#[macro_export]
macro_rules! cmd_plugin {
    ($name:expr, $alias:expr, $usage:expr, $argsmin:expr, $argsmax:expr, |$self:ident| $body:block) => {
        cmd_plugin!(
            $name,
            $alias,
            $usage,
            $argsmin,
            $argsmax,
            |$self: ident, _args| $body
        );
    };
    ($name:expr, $alias:expr, $usage:expr, $argsmin:expr, $argsmax:expr, |$self:ident, $args:ident| $body:block) => {
        mod tmux_cmd_plugin {
            use super::*;
            use $crate::tmux;

            $crate::__plugin!(
                cmd,
                tmux::cmd_entry {
                    name: $name as *const u8 as *const $crate::libc::c_char,
                    alias: $alias as *const u8 as *const $crate::libc::c_char,
                    args: tmux::cmd_entry__bindgen_ty_1 {
                        template: b"" as *const u8 as *const $crate::libc::c_char,
                        lower: $argsmin,
                        upper: $argsmax,
                    },
                    usage: $usage as *const u8 as *const $crate::libc::c_char,
                    source: tmux::cmd_entry_flag {
                        flag: 0,
                        type_: 0 as tmux::cmd_find_type,
                        flags: 0,
                    },
                    target: tmux::cmd_entry_flag {
                        flag: 0,
                        type_: 0,
                        flags: 0,
                    },
                    flags: 0,
                    exec: Some(cmd_exec),
                }
            );

            fn cmd_plugin_body<'a>(
                $self: *mut tmux::cmd,
                $args: impl Iterator<Item = &'a CStr>,
            ) -> tmux::cmd_retval {
                $body
            }

            pub unsafe extern "C" fn cmd_exec(
                $self: *mut tmux::cmd,
                _item: *mut tmux::cmdq_item,
            ) -> tmux::cmd_retval {
                let args = *(*$self).args;
                let argv: &[*mut i8] = std::slice::from_raw_parts(args.argv, args.argc as usize);
                let argv = argv
                    .iter()
                    .map(|arg| unsafe { ::std::ffi::CStr::from_ptr(*arg) });
                cmd_plugin_body($self, argv)
            }
        }
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __plugin {
    (format, $body:expr) => {
        $crate::__plugin!(format, tmux::FORMAT_PLUGIN, $body);
    };
    (function, $body:expr) => {
        $crate::__plugin!(function, tmux::FORMAT_FUNCTION_PLUGIN, $body);
    };
    (notify, $body:expr) => {
        $crate::__plugin!(notify, tmux::NOTIFICATION_PLUGIN, $body);
    };
    (cmd, $body:expr) => {
        $crate::__plugin!(cmd, tmux::CMD_PLUGIN, $body);
    };
    ($field:ident, $type:expr, $body:expr) => {
        #[repr(transparent)]
        pub struct Plugin(tmux::plugin);
        unsafe impl Sync for Plugin {}

        #[allow(non_upper_case_globals)]
        #[no_mangle]
        static plugin: Plugin = Plugin(tmux::plugin {
            type_: $type as $crate::libc::c_int,
            __bindgen_anon_1: { tmux::plugin_inner { $field: $body } },
        });
    };
}
