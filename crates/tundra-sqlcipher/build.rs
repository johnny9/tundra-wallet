use bitcoin_hashes::{Hash, sha256};
use std::{env, fs, path::Path};

fn main() {
    // Reviewed generated files must change together with their source provenance.
    for (name, expected) in [
        (
            "sqlite3.c",
            "8640c653acadf665cce6331646f60b5b74a4690746f2c4a2d8f688a0570a0c0c",
        ),
        (
            "sqlite3.h",
            "8a9d1bff44d75174ca6dea3ea9bac50a6104d86facb566647b8bb839375b7b3a",
        ),
    ] {
        let path = Path::new("vendor").join(name);
        let actual = sha256::Hash::hash(&fs::read(&path).expect("SQLCipher source is missing"));
        assert_eq!(
            actual.to_string(),
            expected,
            "SQLCipher source hash changed"
        );
        println!("cargo:rerun-if-changed={}", path.display());
    }
    let mut build = cc::Build::new();
    build
        .file("vendor/sqlite3.c")
        .define("SQLITE_HAS_CODEC", None)
        .define("SQLITE_EXTRA_INIT", "sqlcipher_extra_init")
        .define("SQLITE_EXTRA_SHUTDOWN", "sqlcipher_extra_shutdown")
        .define("SQLITE_THREADSAFE", "1")
        .define("SQLITE_TEMP_STORE", "3")
        .define("SQLITE_DEFAULT_FOREIGN_KEYS", "1")
        .define("SQLITE_ENABLE_API_ARMOR", None)
        .define("SQLITE_OMIT_LOAD_EXTENSION", None)
        .warnings(false);
    if env::var("CARGO_CFG_TARGET_VENDOR").as_deref() == Ok("apple") {
        build.define("SQLCIPHER_CRYPTO_CC", None);
        println!("cargo:rustc-link-lib=framework=Security");
        // The provider reports its version through CFBundle/CFString APIs.
        println!("cargo:rustc-link-lib=framework=CoreFoundation");
    } else {
        build
            .define("SQLCIPHER_CRYPTO_OPENSSL", None)
            .include(env::var("DEP_OPENSSL_INCLUDE").expect("Vendored OpenSSL headers missing"));
    }
    build.compile("tundra_sqlcipher");
}
