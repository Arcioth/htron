use std::path::Path;

fn main() {
    let c_src = Path::new("c_src");

    let c_files = [
        c_src.join("util.c"),
        c_src.join("hw.c"),
        c_src.join("control.c"),
        c_src.join("daeboard.c"),
        c_src.join("fan.c"),
        c_src.join("cmds.c"),
        c_src.join("modes.c"),
        c_src.join("profile.c"),
        c_src.join("settings.c"),
        c_src.join("telem.c"),
        c_src.join("display/display.c"),
        c_src.join("display/display_kde.c"),
        c_src.join("display/display_hypr.c"),
    ];

    let mut build = cc::Build::new();
    build
        .std("c11")
        .define("_GNU_SOURCE", None)
        .include(c_src)
        .flag_if_supported("-Wno-unused-parameter")
        .flag_if_supported("-Wno-sign-compare")
        .file("src/c_bridge.c");

    for f in &c_files {
        println!("cargo:rerun-if-changed={}", f.display());
        build.file(f);
    }
    println!("cargo:rerun-if-changed=src/c_bridge.c");

    build.compile("ctron_core");
    println!("cargo:rustc-link-lib=pthread");
}
