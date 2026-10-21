fn main() {
    // 只编译 KNArchives（它 import 的 TSPMessages/TSA/TSD 等会自动处理）
    prost_build::Config::new()
        .compile_protos(&["proto/KNArchives.proto"], &["proto"])
        .expect("protoc 编译 proto 失败");
    println!("cargo:rerun-if-changed=proto");
}
