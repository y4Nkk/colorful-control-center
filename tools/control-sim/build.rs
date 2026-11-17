fn main() {
    println!("cargo:rustc-check-cfg=cfg(control_sim)");
    println!("cargo:rustc-cfg=control_sim");
}
