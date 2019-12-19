// build-pass
// compile-flags:--test

#![crate_name = "sysin"]

/// Hello
///
/// ```
/// use sysin::{bar, func};
/// bar();
/// ```
#[cfg(doctest)]
pub fn bar () {
    println!("hello");
}

pub fn func() {}