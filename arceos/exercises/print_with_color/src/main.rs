#![cfg_attr(feature = "axstd", no_std)]
#![cfg_attr(feature = "axstd", no_main)]

#[cfg(feature = "axstd")]
use axstd::println;

#[cfg_attr(feature = "axstd", no_mangle)]
fn main() {
    // Print the greeting in bold green using ANSI escape sequences.
    println!("\u{1b}[32;1m[WithColor]: Hello, Arceos!\u{1b}[0m");
}
