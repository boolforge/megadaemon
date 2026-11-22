//! Pure-Rust DEX / Binary SDK & Packer Origin Fingerprinter (`aho-corasick`).
//!
//! Single-pass multi-pattern DFA scanner for Android SDKs, cross-platform engines,
//! dynamic loaders, crypto/USB/NFC APIs, and commercial packers.

use serde_json::{json, Value};

pub fn scan_dex_origins(data: &[u8]) -> Value {
    const SIGNATURES: &[(&str, &str)] = &[
        ("Lcom/google/android/gms/", "Google Play Services (GMS)"),
        ("Lcom/google/firebase/", "Firebase SDK"),
        ("Landroidx/", "AndroidX Jetpack"),
        ("Lokhttp3/", "OkHttp3 HTTP Client"),
        ("Lretrofit2/", "Retrofit2 REST Client"),
        ("Lkotlin/", "Kotlin Stdlib"),
        ("Lkotlinx/coroutines/", "Kotlin Coroutines"),
        ("Lio/flutter/", "Flutter Engine"),
        ("Lcom/facebook/react/", "React Native"),
        ("Lorg/apache/cordova/", "Apache Cordova"),
        ("Lcom/unity3d/player/", "Unity3D Player"),
        ("Lcom/secneo/apkwrapper/", "Bangcle/SecNeo Packer"),
        ("Lcom/tencent/StubShell/", "Tencent Legu Packer"),
        ("Lcom/qihoo/util/", "Qihoo 360 Packer"),
        ("Ldalvik/system/DexClassLoader;", "Dynamic DexClassLoader"),
        ("Ljavax/crypto/Cipher;", "Java Crypto Cipher"),
        ("Landroid/hardware/usb/", "Android USB Host API"),
        ("Landroid/nfc/tech/IsoDep;", "Android NFC IsoDep (Smartcard)"),
    ];
    let patterns: Vec<&str> = SIGNATURES.iter().map(|(p, _)| *p).collect();
    let ac = match aho_corasick::AhoCorasick::new(&patterns) {
        Ok(a) => a,
        Err(e) => return json!({ "ok": false, "error": e.to_string() }),
    };
    let mut counts = vec![0usize; SIGNATURES.len()];
    for mat in ac.find_iter(data) {
        counts[mat.pattern().as_usize()] += 1;
    }
    let mut findings = Vec::new();
    for (idx, &cnt) in counts.iter().enumerate() {
        if cnt > 0 {
            findings.push(json!({
                "signature": SIGNATURES[idx].0,
                "label": SIGNATURES[idx].1,
                "hits": cnt,
            }));
        }
    }
    json!({
        "ok": true,
        "engine": "aho-corasick",
        "bytes_scanned": data.len(),
        "matched_families": findings.len(),
        "findings": findings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scans_dex_origins_with_aho_corasick() {
        let blob = b"dex\n035\0Lkotlin/Unit;Lokhttp3/OkHttpClient;Landroid/nfc/tech/IsoDep;";
        let res = scan_dex_origins(blob);
        assert_eq!(res["ok"], true);
        assert_eq!(res["matched_families"], 3);
    }
}
