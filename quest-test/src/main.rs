//! Desktop entry point for quest-test
//!
//! This allows testing compilation on desktop before deploying to Quest.
//! This binary is not used on Android - android_main in lib.rs is the entry point.

#[cfg(not(target_os = "android"))]
fn main() {
    quest_test::run();
}

#[cfg(target_os = "android")]
fn main() {
    // Not used - android_main in lib.rs is the entry point
}
