//! W2 · Tests del inventario del dispositivo (`device_inventory`).
//!
//! El módulo se usa desde la libreria (`use megadaemon_core::...`).
//! MEDIDO el 2026-11-21: antes se copiaba con `#[path = "../src/x.rs"]`; dentro de
//! un arnés `crate::` es la raíz del ARNÉS, así que un módulo copiado que usa
//! `crate::otro_modulo` no resolvía (E0433) y **el target de test no compilaba**
//! => la suite entera moría sin ejecutar ni un test.

use serde_json::Value;

use megadaemon_core::device_inventory;

use megadaemon_core::device_inventory::{
    collect, manifest_ndjson, parse_accounts, parse_adb_keys, parse_cpuinfo, parse_df,
    parse_meminfo, parse_partitions, parse_pm_packages, parse_pm_versions, parse_props,
    parse_wifi_saved, risky_permissions, sha16, sha256_hex, FileDigest, Options, Source,
};

/// Doble de test: devuelve contenidos canned y digests calculados de verdad.
struct FakeSource {
    files: Vec<(String, String)>,
}

impl FakeSource {
    fn new(files: &[(&str, &str)]) -> Self {
        FakeSource {
            files: files.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
        }
    }
}

impl Source for FakeSource {
    fn shell(&self, cmd: &str) -> Option<String> {
        if cmd.starts_with("uname") {
            Some("Linux localhost 5.4.147 #1 SMP PREEMPT".to_string())
        } else if cmd.starts_with("id -Z") {
            Some("u:r:magisk:s0".to_string())
        } else {
            None
        }
    }
    fn file(&self, name: &str) -> Option<String> {
        self.files.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone())
    }
    fn digest(&self, name: &str) -> Option<FileDigest> {
        self.file(name).map(|content| FileDigest {
            sha256: sha256_hex(content.as_bytes()),
            bytes: content.len() as u64,
        })
    }
}

const GETPROP: &str = "\
[ro.product.model]: [SM-G998B]
[ro.product.device]: [p3s]
[ro.soc.model]: [Exynos 2100]
[ro.board.platform]: [s5e9840]
[ro.build.version.release]: [14]
[ro.build.version.sdk]: [34]
[ro.build.version.security_patch]: [2024-06-01]
[ro.csc.sales_code]: [AMO]
[ro.csc.country_code]: [SPAIN]
[ro.build.fingerprint]: [samsung/r0qxxx/r0q:14/UP1A.231005.007/...]
[ro.boot.verifiedbootstate]: [green]
[ro.boot.vbmeta.digest]: [abc123]
[ro.hardware.egl]: [mali]
[ro.magisk.version]: [27.2-kitsune]
";

const CPUINFO: &str = "\
Hardware\t: Samsung Exynos 2100
processor\t: 0
model name\t: ARMv8 Processor rev 4 (v8l)
Features\t: fp asimd evtstrm aes pmull sha1 sha2 crc32 atomics
processor\t: 1
processor\t: 2
processor\t: 3
";

const MEMINFO: &str = "\
MemTotal:       10575472 kB
MemFree:          229000 kB
MemAvailable:    1300000 kB
SwapTotal:       3400000 kB
SwapFree:        2100000 kB
";

const PARTITIONS: &str = "\
major minor  #blocks  name
   8        0  1048576 sda
   8        1     65536 sda1
 259        0  120000000 sda34
";

const PM_PACKAGES: &str = "\
package:/system/app/SystemUI/SystemUI.apk=com.android.systemui
package:/data/app/~~abc==/com.ejemplo-1/base.apk=com.ejemplo
package:/data/app/~~xyz==/com.privado-1/base.apk=com.privado
";

const DUMPSYS_PACKAGES: &str = "\
  Package [com.android.systemui] (31):
    versionCode=31 minSdk=23 targetSdk=34
    versionName=14
    signatures=PackageSignatures{4b06399 version:3, signatures:[302e4f92], past signatures:[]}
    install permissions:
      android.permission.INTERNET: granted=true
    User 0: ceDataInode=1 installed=true hidden=false enabled=0 instant=false
  Package [com.ejemplo] (42):
    versionCode=42 minSdk=26 targetSdk=34
    versionName=2.1
    signatures=PackageSignatures{1617ace version:2, signatures:[6052b3ed], past signatures:[]}
    install permissions:
      android.permission.CAMERA: granted=true
    runtime permissions:
      android.permission.READ_SMS: granted=false, flags=[ USER_SENSITIVE]
    User 0: ceDataInode=2 installed=true hidden=false enabled=0 instant=false
  Package [com.privado] (7):
    versionCode=7 minSdk=24 targetSdk=33
    versionName=0.9
    signatures=PackageSignatures{99aa11b version:3, signatures:[aabbccdd], past signatures:[]}
    User 0: ceDataInode=3 installed=true hidden=false enabled=2 instant=false
";

const PM_VERSIONS: &str = "\
package:/system/app/SystemUI/SystemUI.apk=com.android.systemui versionCode:31
package:/data/app/~~abc==/com.ejemplo-1/base.apk=com.ejemplo versionCode:42
package:/data/app/~~xyz==/com.privado-1/base.apk=com.privado versionCode:7
";

const ACCOUNTS: &str = "\
Account {name=usuario@ejemplo.com, type=com.google}
Account {name=usuario@ejemplo.com, type=com.google}
Account {name=sesionSamsung, type=com.samsung.account}
";

const WIFI: &str = "\
<NetworkList>
<Network>
<SSID>CasaDePrueba</SSID>
<BSSID>aa:bb:cc:dd:ee:ff</BSSID>
</Network>
<Network>
<SSID>Oficina</SSID>
<BSSID>11:22:33:44:55:66</BSSID>
</Network>
</NetworkList>
";

const ADB_KEYS: &str = "\
# comment
AAAAB3NzaC1yc2EAADBAAClaveDePruebanueva mavis@samsung
";

fn fake() -> FakeSource {
    FakeSource::new(&[
        ("getprop_all", GETPROP),
        ("cpuinfo", CPUINFO),
        ("meminfo", MEMINFO),
        ("partitions", PARTITIONS),
        ("pm_packages", PM_PACKAGES),
        ("pm_system", "package:com.android.systemui"),
        ("pm_third_party", "package:com.ejemplo\npackage:com.privado"),
        ("pm_disabled", "package:com.privado"),
        ("pm_versions", PM_VERSIONS),
        ("accounts", ACCOUNTS),
        ("wifi_config_store", WIFI),
        ("adb_keys", ADB_KEYS),
        ("df", "Filesystem 1K-blocks Used Available Use% Mounted on\n/dev/block/sda34 225000000 219000000 5700000 98% /data\n"),
        ("du_media", "44346358 /data/media/0/Download\n18043172 /data/media/0/DCIM"),
        ("du_data", "90000000 /data/media\n5000000 /data/adb"),
        ("gpu", "12\n220000000 220000000 220000000"),
        ("npu", "crw-r--r-- 1 root root 10, 234 /dev/vertex10"),
        ("screen", "Physical size: 1440x3088\nPhysical density: 500 dpi"),
        ("sensors", "0x01000000 | Acme Accelerometer | Acme"),
        ("cameras", "Number of camera devices: 4"),
        ("modem", "[gsm.version.baseband]: [S5E9840]"),
        ("boot_props", "[ro.boot.verifiedbootstate]: [green]\n[ro.boot.vbmeta.digest]: [abc123]"),
        ("block_by_name", "lrwxrwxrwx 1 root root 16 boot -> /dev/block/sda13\nlrwxrwxrwx 1 root root 15 efs -> /dev/block/sda1"),
        ("tee_devices", "crw-r--r-- 1 root root 10, 234 /dev/vertex10"),
        ("selinux", "Enforcing"),
        ("magisk", "27.2-kitsune-4\n28001\n---\n/data/adb/modules\n---\nsu: /system/bin/su"),
        ("users", "UserInfo{0:Owner:c13} running\nUserInfo{10:Work profile:1030}"),
        ("swaps", "/dev/block/sda34 partition 3400000 1300000"),
        ("net_ifaces", "2: eth0: <BROADCAST> inet 192.168.1.124/24"),
        ("net_dns_proxy", "[net.dns1]: [192.168.1.1]"),
        ("net_vpn", "VPN not connected"),
        ("partitions_extra", ""),
        ("dumpsys_packages", DUMPSYS_PACKAGES),
    ])
}

#[test]
fn el_inventario_tiene_version_y_todas_las_secciones() {
    let inv = collect(&fake(), &Options::default());
    assert_eq!(inv["version"], device_inventory::INVENTORY_VERSION);
    for sec in [
        "hardware",
        "software",
        "boot_y_particiones",
        "apps",
        "red",
        "cuentas",
        "espacio",
        "referencias",
    ] {
        assert!(!inv[sec].is_null(), "falta la sección {sec}");
    }
}

#[test]
fn el_hardware_recoge_soc_cpu_gpu_npu_memoria_y_pantalla() {
    let inv = collect(&fake(), &Options::default());
    let hw = &inv["hardware"];
    assert_eq!(hw["modelo"], "SM-G998B");
    assert_eq!(hw["soc_modelo"], "Exynos 2100");
    assert_eq!(hw["soc_plataforma"], "s5e9840");
    assert_eq!(hw["cpu"]["nucleos_logicos"], 4);
    assert_eq!(hw["cpu"]["hardware"], "Samsung Exynos 2100");
    assert_eq!(hw["gpu"]["egl"], "mali");
    assert_eq!(hw["npu"]["dispositivos"]["/dev/vertex10"]["modo"], "crw-r--r--");
    assert_eq!(hw["memoria"]["ram_total_kb"], 10575472u64);
    assert_eq!(hw["memoria"]["swap_total_kb"], 3400000u64);
    assert_eq!(hw["pantalla"]["tamano"][0], "Physical size: 1440x3088");
}

#[test]
fn el_software_recoge_parche_csc_kernel_selinux_y_magisk() {
    let inv = collect(&fake(), &Options::default());
    let sw = &inv["software"];
    assert_eq!(sw["rom"]["parche_seguridad"], "2024-06-01");
    assert_eq!(sw["rom"]["csc"], "AMO");
    assert_eq!(sw["rom"]["sdk"], "34");
    assert_eq!(sw["selinux"]["modo"], "Enforcing");
    assert_eq!(sw["magisk"]["version"], "27.2-kitsune-4");
    assert_eq!(sw["kernel"]["uname"], "Linux localhost 5.4.147 #1 SMP PREEMPT");
    assert_eq!(sw["perfiles_usuario"]["work_profile"], true);
}

#[test]
fn el_boot_recoge_particiones_avb_y_tee() {
    let inv = collect(&fake(), &Options::default());
    let bp = &inv["boot_y_particiones"];
    assert_eq!(bp["tabla_particiones"]["total"], 3);
    assert_eq!(bp["by_name"]["boot"], "/dev/block/sda13");
    assert_eq!(bp["by_name"]["efs"], "/dev/block/sda1");
    assert_eq!(bp["avb"]["verified_boot_state"], "green");
    assert_eq!(bp["avb"]["vbmeta_digest"], "abc123");
    assert!(bp["tee"]["dispositivos"]["/dev/vertex10"].is_object());
}

#[test]
fn las_apps_llevan_riesgo_estado_y_sistema() {
    let inv = collect(&fake(), &Options::default());
    let apps = &inv["apps"];
    assert_eq!(apps["total"], 3);
    assert_eq!(apps["de_sistema"], 1);
    assert_eq!(apps["deshabilitadas"], 1);
    let lista = apps["paquetes"].as_array().unwrap();
    let ejemplo = lista.iter().find(|a| a["paquete"] == "com.ejemplo").unwrap();
    assert_eq!(ejemplo["version_code"], 42);
    assert_eq!(ejemplo["version_name"], "2.1");
    assert_eq!(ejemplo["firma_sha256_corto"], "6052b3ed");
    assert_eq!(ejemplo["target_sdk"], 34);
    assert_eq!(ejemplo["es_sistema"], false);
    assert_eq!(ejemplo["habilitada"], true);
    let riesgos = ejemplo["permisos_riesgo"].as_array().unwrap();
    assert!(riesgos.contains(&Value::from("android.permission.CAMERA")));
    let privado = lista.iter().find(|a| a["paquete"] == "com.privado").unwrap();
    assert_eq!(privado["habilitada"], false);
    let sistema = lista.iter().find(|a| a["paquete"] == "com.android.systemui").unwrap();
    assert_eq!(sistema["es_sistema"], true);
}

#[test]
fn las_cuentas_solo_presencia_y_estado_nunca_tokens() {
    let inv = collect(&fake(), &Options::default());
    let c = &inv["cuentas"];
    assert_eq!(c["tokens_included"], false);
    assert_eq!(c["total_cuentas"], 3);
    // El nombre en claro NO puede aparecer en el JSON.
    let texto = serde_json::to_string(&inv).unwrap();
    assert!(!texto.contains("usuario@ejemplo.com"), "¡cuenta en claro en el inventario!");
    assert!(!texto.contains("sesionSamsung"), "¡cuenta en claro en el inventario!");
    assert!(texto.contains("nombres_sha16"));
}

#[test]
fn la_red_oculta_ssid_bssid_y_claves_adb() {
    let inv = collect(&fake(), &Options::default());
    let texto = serde_json::to_string(&inv).unwrap();
    assert!(!texto.contains("CasaDePrueba"), "¡SSID en claro!");
    assert!(!texto.contains("aa:bb:cc:dd:ee:ff"), "¡BSSID en claro!");
    assert!(!texto.contains("AAAAB3NzaC1yc2EAADBAAClaveDePruebanueva"), "¡clave ADB en claro!");
    let red = &inv["red"];
    assert_eq!(red["redes_guardadas"]["total"], 2);
    assert_eq!(red["claves_adb_autorizadas"]["total"], 1);
    assert_eq!(red["claves_adb_autorizadas"]["claves_autorizadas"][0]["nombre"], "mavis@samsung");
}

#[test]
fn cada_referencia_lleva_sha256_y_el_manifiesto_una_linea_por_fichero() {
    let src = fake();
    let inv = collect(&src, &Options::default());
    let refs = inv["referencias"].as_array().unwrap();
    assert!(refs.len() >= 20, "faltan referencias: {}", refs.len());
    for r in refs {
        let sha = r["sha256"].as_str().unwrap();
        assert_eq!(sha.len(), 64, "sha256 con longitud rara: {sha}");
        assert!(sha.chars().all(|c| c.is_ascii_hexdigit()));
        assert!(r["bytes"].as_u64().unwrap() > 0 || r["id"] == "partitions_extra");
    }
    let nd = manifest_ndjson(&src);
    let lineas: Vec<&str> = nd.lines().collect();
    assert_eq!(lineas.len(), refs.len());
    for l in &lineas {
        let v: Value = serde_json::from_str(l).unwrap();
        assert_eq!(v["sha256"].as_str().unwrap().len(), 64);
    }
}

#[test]
fn las_referencias_privadas_no_se_publican() {
    let inv = collect(&fake(), &Options::default());
    let refs = inv["referencias"].as_array().unwrap();
    let privado = |id: &str| {
        refs.iter()
            .find(|r| r["id"] == id)
            .map(|r| r["visibilidad"] == "privada")
            .unwrap_or(false)
    };
    assert!(privado("adb_keys"));
    assert!(privado("wifi_config_store"));
    assert!(privado("accounts"));
    assert!(!privado("getprop_all"));
}

#[test]
fn los_parsers_unitarios_separen_bien_los_formatos() {
    // `pm list packages -f` usa "package:ruta=paquete" (NO "package=ruta").
    let pkgs = parse_pm_packages(PM_PACKAGES);
    assert_eq!(pkgs.len(), 3);
    assert_eq!(pkgs[0]["paquete"], "com.android.systemui");
    assert_eq!(pkgs[0]["ruta_apk"], "/system/app/SystemUI/SystemUI.apk");

    let v = parse_pm_versions(PM_VERSIONS);
    assert_eq!(v["com.ejemplo"], 42);
    assert_eq!(v["com.privado"], 7);

    let p = parse_props(GETPROP);
    assert_eq!(p["ro.product.model"], "SM-G998B");
    assert_eq!(p["ro.csc.sales_code"], "AMO");
    assert_eq!(p["ro.csc.country_code"], "SPAIN");

    let m = parse_meminfo(MEMINFO);
    assert_eq!(m["ram_total_kb"], 10575472u64);
    assert_eq!(m["swap_usada_kb"], 1300000u64);

    let part = parse_partitions(PARTITIONS);
    assert_eq!(part["total"], 3);

    let df = parse_df("Filesystem 1K-blocks Used Available Use% Mounted on\n/dev/block/sda34 225000000 219000000 5700000 98% /data\n");
    assert_eq!(df["filesystems"][0]["uso_pct"], 98);
    assert_eq!(df["filesystems"][0]["libre_kb"], 5700000u64);

    let acc = parse_accounts(ACCOUNTS);
    assert_eq!(acc["total_cuentas"], 3);
    assert_eq!(acc["tokens_included"], false);

    let cpu = parse_cpuinfo(CPUINFO);
    assert_eq!(cpu["nucleos_logicos"], 4);
    assert_eq!(cpu["model_name"], "ARMv8 Processor rev 4 (v8l)");

    let wifi = parse_wifi_saved(WIFI);
    assert_eq!(wifi["total"], 2);

    let adb = parse_adb_keys(ADB_KEYS);
    assert_eq!(adb["total"], 1);
}

#[test]
fn el_sha256_coincide_con_el_vector_conocido() {
    assert_eq!(
        sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(sha16("abc").len(), 16);
}

#[test]
fn los_permisos_de_riesgo_se_detectan() {
    let r = risky_permissions("com.x\nandroid.permission.CAMERA\nandroid.permission.READ_SMS");
    assert_eq!(r.len(), 2);
    assert!(r.contains(&"android.permission.CAMERA".to_string()));
}

