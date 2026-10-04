#![allow(missing_docs)]

use std::{env, fs, path::PathBuf, process::Command};

fn main() {
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("none") {
        return;
    }
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("cargo sets OUT_DIR"));
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../ksrc");
    let clang = std::env::var("CC").unwrap_or_else(|_| "clang".into());
    let mut objects = Vec::new();
    for source in ["a.c", "z.c"] {
        let object = out.join(source.replace(".c", ".o"));
        let status = Command::new(&clang)
            .args([
                "--target=aarch64-none-elf",
                "-mcpu=cortex-a72",
                "-O2",
                "-ffreestanding",
                "-fno-builtin",
                "-fno-stack-protector",
                "-fno-unwind-tables",
                "-fno-asynchronous-unwind-tables",
                "-DKSYS",
                "-DKHEAP=22",
                "-Dmain=k_main",
                "-I",
                root.to_str().expect("ksrc path is UTF-8"),
                "-c",
            ])
            .arg(root.join(source))
            .args(["-o", object.to_str().expect("object path is UTF-8")])
            .status()
            .expect("run clang");
        assert!(status.success(), "clang failed for {source}");
        objects.push(object);
        println!("cargo::rerun-if-changed={}", root.join(source).display());
    }
    for header in fs::read_dir(&root).expect("read ksrc") {
        let path = header.expect("read ksrc entry").path();
        if path.extension().is_some_and(|ext| ext == "h") {
            println!("cargo::rerun-if-changed={}", path.display());
        }
    }
    let archive = out.join("libk.a");
    let ar = std::env::var("AR").unwrap_or_else(|_| rust_llvm_ar());
    let status = Command::new(ar)
        .arg("crus")
        .arg(&archive)
        .args(&objects)
        .status()
        .expect("run llvm-ar");
    assert!(status.success(), "llvm-ar failed");
    println!("cargo::rustc-link-search=native={}", out.display());
    println!("cargo::rustc-link-lib=static=k");
}

fn rust_llvm_ar() -> String {
    let sysroot = Command::new("rustc")
        .args(["--print", "sysroot"])
        .output()
        .expect("find rust sysroot");
    let host = Command::new("rustc")
        .args(["-vV"])
        .output()
        .expect("find rust host")
        .stdout;
    let host = String::from_utf8(host)
        .expect("rustc host is UTF-8")
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .expect("rustc reports host")
        .to_owned();
    PathBuf::from(
        String::from_utf8(sysroot.stdout)
            .expect("sysroot is UTF-8")
            .trim(),
    )
    .join("lib/rustlib")
    .join(host)
    .join("bin/llvm-ar")
    .to_string_lossy()
    .into_owned()
}
