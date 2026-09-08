#[cfg(any(target_os = "android", target_arch = "wasm32"))]
fn main() {}

#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
fn main() {
    main::maverick_main()
}
