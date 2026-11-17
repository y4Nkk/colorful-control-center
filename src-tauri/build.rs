fn main() {
    println!("cargo:rustc-check-cfg=cfg(control_sim)");
    // 默认资源只链接 bin；清单单独链接所有目标，lib 测试同样依赖 comctl32 v6。
    let windows = tauri_build::WindowsAttributes::new_without_app_manifest();
    tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows))
        .expect("构建 Tauri 资源失败");
    println!("cargo:rerun-if-changed=windows.rc");
    println!("cargo:rerun-if-changed=windows.manifest.xml");
    embed_resource::compile_for_everything("windows.rc", embed_resource::NONE)
        .manifest_required()
        .expect("编译 Windows 清单失败");
}
