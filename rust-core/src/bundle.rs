//! **Build "meta"**: un artefacto único con TODO dentro.
//!
//! POR QUÉ
//! --------
//! El usuario pidió (2026-11-25) que "el propio MegaDaemon actualice TODO y
//! sea autocontenido": que no haya que ir a por el binario, el supervisor,
//! el keeper, los scripts de Antigravity y la Web UI por separado.
//!
//! Hay dos capas, y son complementarias:
//!
//! 1. **Autocontenido** (módulo [`crate::stack`]): el binario lleva DENTRO
//!    el stack y lo reconcilia solo.  No necesita red: si el supervisor se
//!    desincroniza, el daemon lo repara.
//! 2. **Bundle meta** (este módulo): un `.tar.gz` con el binario, los
//!    scripts, la Web UI y un `manifest.json` con el SHA-256 de cada
//!    fichero.  El daemon lo descarga, lo **verifica fichero a fichero** y
//!    lo instala.  Sirve para instalar desde cero y para actualizar TODO de
//!    una vez.
//!
//! El bundle NO sustituye al stack autocontenido: es la vía de entrada
//! (instalación / actualización completa), el stack es la vía de
//! autorreparación (offline, sin red).
//!
//! Autor: sesión root de MegaDaemon (2026-11-25).

use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Nombre del asset en la release.
pub const BUNDLE_ASSET: &str = "megadaemon-movil.tar.gz";

/// URL del bundle en la última release.
pub fn url_bundle() -> String {
    format!(
        "{}/download/{}",
        crate::self_update::url_release(),
        BUNDLE_ASSET
    )
}

/// URL del SHA-256 del bundle (si la release lo publica).
pub fn url_sha() -> String {
    format!("{}.sha256", url_bundle())
}

/// Un miembro del bundle: de dónde viene, dónde va y su hash.
#[derive(Debug, Clone)]
pub struct Miembro {
    /// Ruta dentro del bundle.
    pub origen: String,
    /// Destino: `prefijo/…`, `servicio/…` o una ruta absoluta.
    pub destino: String,
    /// Permisos.
    pub modo: u32,
    /// SHA-256 del contenido.
    pub sha256: String,
}

/// Resuelve el destino de un miembro.
///
/// * `prefijo/web/cockpit_v2.html` → `<prefijo>/web/cockpit_v2.html`
/// * `servicio/99-megadaemon.sh`   → `<padre del prefijo>/service.d/99-megadaemon.sh`
/// * `/data/x/y`                   → tal cual
pub fn resolver_destino(destino: &str, prefijo: &str) -> PathBuf {
    if let Some(r) = destino.strip_prefix("prefijo/") {
        return Path::new(prefijo).join(r);
    }
    if let Some(r) = destino.strip_prefix("servicio/") {
        return Path::new(prefijo)
            .parent()
            .unwrap_or_else(|| Path::new("/data/adb"))
            .join("service.d")
            .join(r);
    }
    PathBuf::from(destino)
}

/// El manifiesto del bundle (`manifest.json`).
#[derive(Debug, Clone, Default)]
pub struct Manifiesto {
    /// Build del binario.
    pub build: String,
    /// Miembros declarados.
    pub miembros: Vec<Miembro>,
}

impl Manifiesto {
    /// Lee el manifiesto desde el JSON del bundle.
    pub fn desde_json(v: &Value) -> Result<Manifiesto, String> {
        let build = v
            .get("build")
            .and_then(|x| x.as_str())
            .unwrap_or("?")
            .to_string();
        let lista = v
            .get("ficheros")
            .and_then(|x| x.as_array())
            .ok_or("el manifiesto no tiene la lista de ficheros")?;
        let mut miembros = Vec::new();
        for m in lista {
            let origen = m
                .get("origen")
                .and_then(|x| x.as_str())
                .ok_or("miembro sin 'origen'")?
                .to_string();
            let destino = m
                .get("destino")
                .and_then(|x| x.as_str())
                .ok_or("miembro sin 'destino'")?
                .to_string();
            let modo = m.get("modo").and_then(|x| x.as_u64()).unwrap_or(0o644) as u32;
            let sha256 = m
                .get("sha256")
                .and_then(|x| x.as_str())
                .ok_or("miembro sin 'sha256'")?
                .to_string();
            miembros.push(Miembro {
                origen,
                destino,
                modo,
                sha256,
            });
        }
        Ok(Manifiesto { build, miembros })
    }
}

/// Extrae un `.tar.gz` en memoria: ruta → bytes.
pub fn extraer_tar_gz(datos: &[u8]) -> Result<BTreeMap<String, Vec<u8>>, String> {
    use std::io::Read;
    let dec = flate2::read::GzDecoder::new(datos);
    let mut ar = tar::Archive::new(dec);
    let mut salida = BTreeMap::new();
    let entradas = ar
        .entries()
        .map_err(|e| format!("no se pudo leer el tar: {e}"))?;
    for entrada in entradas {
        let mut e = entrada.map_err(|e| format!("entrada ilegible: {e}"))?;
        let ruta = e
            .path()
            .map_err(|e| format!("ruta ilegible: {e}"))?
            .to_string_lossy()
            .to_string();
        // No se admiten rutas con ".." (path traversal).
        if ruta.contains("..") || ruta.starts_with('/') {
            continue;
        }
        let mut buf = Vec::new();
        e.read_to_end(&mut buf)
            .map_err(|e| format!("no se pudo leer {ruta}: {e}"))?;
        salida.insert(ruta, buf);
    }
    if salida.is_empty() {
        return Err("el bundle esta vacio".to_string());
    }
    Ok(salida)
}

/// Veredicto de la instalación del bundle.
#[derive(Debug, Clone, Default)]
pub struct Informe {
    /// Ficheros instalados (o reescritos).
    pub instalados: Vec<String>,
    /// Ficheros que ya estaban al día.
    pub correctos: Vec<String>,
    /// (ruta, motivo).
    pub fallidos: Vec<(String, String)>,
    /// Build del bundle.
    pub build: String,
}

impl Informe {
    /// ¿Todo bien?
    pub fn ok(&self) -> bool {
        self.fallidos.is_empty()
    }

    /// A JSON.
    pub fn a_json(&self) -> Value {
        json!({
            "build": self.build,
            "instalados": self.instalados,
            "correctos": self.correctos.len(),
            "fallidos": self.fallidos,
            "ok": self.ok(),
        })
    }
}

/// **Descarga, verifica e instala el bundle meta.**
///
/// La verificación es **fichero a fichero** contra el `manifest.json` del
/// propio bundle: si un solo SHA no coincide, no se instala NADA (para que
/// no quede el móvil a medias).
pub fn instalar() -> Result<Informe, String> {
    let cfg = crate::stack::Config::del_entorno();
    let url = url_bundle();
    let datos = crate::self_update::descargar_bytes(&url)
        .map_err(|e| format!("no se pudo descargar {url}: {e}"))?;
    instalar_desde_datos(&datos, &cfg)
}

/// Instala el bundle a partir de sus bytes (testeable sin red).
pub fn instalar_desde_datos(datos: &[u8], cfg: &crate::stack::Config) -> Result<Informe, String> {
    let ficheros = extraer_tar_gz(datos)?;
    let man_bytes = ficheros
        .get("manifest.json")
        .ok_or("el bundle no trae manifest.json")?;
    let man_json: Value = serde_json::from_slice(man_bytes)
        .map_err(|e| format!("el manifest.json no es valido: {e}"))?;
    let man = Manifiesto::desde_json(&man_json)?;

    // 1) Verificar TODOS los hashes ANTES de tocar nada.
    for m in &man.miembros {
        let datos = ficheros.get(&m.origen).ok_or_else(|| {
            format!(
                "el manifiesto declara {} pero no esta en el bundle",
                m.origen
            )
        })?;
        let hash = crate::self_update::sha256_hex(datos);
        if hash != m.sha256 {
            return Err(format!(
                "SHA-256 de {} no coincide (manifiesto {}, obtenido {}): no se instala nada",
                m.origen, m.sha256, hash
            ));
        }
    }

    // 2) Instalar.
    let mut inf = Informe {
        build: man.build.clone(),
        ..Informe::default()
    };
    for m in &man.miembros {
        let Some(datos) = ficheros.get(&m.origen) else {
            continue;
        };
        let ruta = resolver_destino(&m.destino, &cfg.prefijo);
        let actual = crate::stack::hash_de(&ruta);
        if actual.as_deref() == Some(m.sha256.as_str()) {
            inf.correctos.push(ruta.display().to_string());
            continue;
        }
        match crate::stack::escribir_atomico_bytes(&ruta, datos, m.modo) {
            Ok(()) => inf.instalados.push(ruta.display().to_string()),
            Err(e) => inf.fallidos.push((ruta.display().to_string(), e)),
        }
    }
    Ok(inf)
}

/// Descarga el bundle y devuelve su SHA-256 (sin instalarlo).
pub fn sha256_bundle() -> Result<String, String> {
    let datos = crate::self_update::descargar_bytes(&url_bundle())?;
    Ok(crate::self_update::sha256_hex(&datos))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn bundle_de_prueba() -> Vec<u8> {
        let mut ar = tar::Builder::new(Vec::new());
        let contenido = "#!/system/bin/sh\n# supervisor\n";
        let mut cab = tar::Header::new_gnu();
        cab.set_size(contenido.len() as u64);
        cab.set_mode(0o755);
        cab.set_cksum();
        ar.append_data(&mut cab, "deploy/99-megadaemon.sh", contenido.as_bytes())
            .unwrap();
        let sha = crate::self_update::sha256_hex(contenido.as_bytes());
        let manifest = json!({
            "build": "testbuild",
            "ficheros": [{
                "origen": "deploy/99-megadaemon.sh",
                "destino": "prefijo/supervisor.sh",
                "modo": 0o755,
                "sha256": sha,
            }],
        });
        let m = serde_json::to_vec(&manifest).unwrap();
        let mut cab2 = tar::Header::new_gnu();
        cab2.set_size(m.len() as u64);
        cab2.set_mode(0o644);
        cab2.set_cksum();
        ar.append_data(&mut cab2, "manifest.json", &m).unwrap();
        let tar = ar.into_inner().unwrap();
        let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        enc.write_all(&tar).unwrap();
        enc.finish().unwrap()
    }

    #[test]
    fn el_destino_se_resuelve() {
        assert_eq!(
            resolver_destino("prefijo/web/x.html", "/data/adb/megadaemon"),
            PathBuf::from("/data/adb/megadaemon/web/x.html")
        );
        assert_eq!(
            resolver_destino("servicio/99-megadaemon.sh", "/data/adb/megadaemon"),
            PathBuf::from("/data/adb/service.d/99-megadaemon.sh")
        );
        assert_eq!(
            resolver_destino("/data/x/y", "/data/adb/megadaemon"),
            PathBuf::from("/data/x/y")
        );
    }

    #[test]
    fn extrae_el_bundle() {
        let datos = bundle_de_prueba();
        let ficheros = extraer_tar_gz(&datos).expect("extraccion");
        assert!(ficheros.contains_key("manifest.json"));
        assert!(ficheros.contains_key("deploy/99-megadaemon.sh"));
    }

    #[test]
    fn instala_y_verifica() {
        let base = std::env::temp_dir().join(format!(
            "megadaemon-bundle-test-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&base).unwrap();
        let cfg = crate::stack::Config {
            prefijo: base.to_string_lossy().to_string(),
            android: true,
        };
        let datos = bundle_de_prueba();
        let inf = instalar_desde_datos(&datos, &cfg).expect("instalacion");
        assert!(inf.ok(), "{:?}", inf.fallidos);
        assert_eq!(inf.instalados.len(), 1);
        assert_eq!(inf.build, "testbuild");
        // Idempotente.
        let inf2 = instalar_desde_datos(&datos, &cfg).expect("instalacion 2");
        assert!(inf2.instalados.is_empty());
        assert_eq!(inf2.correctos.len(), 1);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn un_hash_roto_impide_instalar_nada() {
        let base = std::env::temp_dir().join(format!(
            "megadaemon-bundle-roto-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&base).unwrap();
        let cfg = crate::stack::Config {
            prefijo: base.to_string_lossy().to_string(),
            android: true,
        };
        // Bundle con el hash del manifiesto corrupto.
        let datos = bundle_de_prueba();
        let mut ficheros = extraer_tar_gz(&datos).unwrap();
        let mut man: Value = serde_json::from_slice(&ficheros["manifest.json"]).unwrap();
        man["ficheros"][0]["sha256"] =
            json!("0000000000000000000000000000000000000000000000000000000000000000");
        ficheros.insert(
            "manifest.json".to_string(),
            serde_json::to_vec(&man).unwrap(),
        );
        // Reempaquetar.
        let mut ar = tar::Builder::new(Vec::new());
        for (ruta, datos) in &ficheros {
            let mut cab = tar::Header::new_gnu();
            cab.set_size(datos.len() as u64);
            cab.set_mode(0o644);
            cab.set_cksum();
            ar.append_data(&mut cab, ruta, datos.as_slice()).unwrap();
        }
        let tar = ar.into_inner().unwrap();
        let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        enc.write_all(&tar).unwrap();
        let gz = enc.finish().unwrap();

        let e = instalar_desde_datos(&gz, &cfg).expect_err("debe fallar");
        assert!(e.contains("no coincide"), "{e}");
        // No debe haber instalado nada.
        assert!(!base.join("supervisor.sh").exists());
        let _ = std::fs::remove_dir_all(&base);
    }
}
