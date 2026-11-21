//! Canal LAN: descubrimiento del móvil **sin internet**.
//!
//! # Por qué existe
//!
//! El canal "estable" (`mdtmfnc.duckdns.org`) depende de internet y de un
//! tercero (DuckDNS) y de otro tercero (Cloudflare) para el quick tunnel.
//! MEDIDO (2026-11-21): el canónico daba `000` porque el A record apuntaba a
//! la IP del sandbox en vez de a la del PC de casa, y el quick tunnel es
//! efímero por diseño.
//!
//! Pero el móvil y el PC de casa están en la **misma red**, y el daemon ya
//! escucha en `0.0.0.0` (puertos 3000/3030/8090/9090) ⇒ el PC puede llamar
//! al móvil **directamente por IP**, sin internet y sin ningún tercero.
//!
//! Lo que faltaba era (a) **descubrir** esa IP sin configurarla a mano (la IP
//! por DHCP cambia) y (b) saber si el móvil está vivo.  Eso es exactamente lo
//! que hace este módulo: un **beacon UDP** en difusión que el PC/sandbox
//! escucha, y que también deja la información en `state/lan.json`.
//!
//! # Límites (honestos)
//!
//! * El beacon UDP en difusión no cruza routers: sólo funciona en la misma
//!   subred (o con un router que reenvíe difusión).  Para salir a internet
//!   sigue haciendo falta el túnel.
//! * Se anuncia **sólo** dirección y puerto.  Ni la API key, ni el token de
//!   sesión, ni ningún secreto salen por aquí.

use serde_json::json;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Puerto UDP del beacon (fuera de los puertos HTTP del cockpit).
pub const BEACON_PORT: u16 = 9099;

/// Segundos entre emisiones del beacon.
pub const BEACON_INTERVAL_S: u64 = 15;

/// Identificador del tipo de paquete: permite descartar ruido en la red.
pub const BEACON_KIND: &str = "megadaemon-beacon/v1";

/// Epoch actual en segundos.
pub fn now_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Todas las IPv4 "arriba" y no-loopback del dispositivo.
///
/// Se usa `getifaddrs` (libc) para no depender de internet: el móvil puede
/// tener `wlan0` (red de casa), `rmnet_data0` (datos móviles) y `ap0`
/// (hotspot) al mismo tiempo, y cada uno es una ruta de entrada distinta.
#[cfg(unix)]
pub fn local_ipv4_all() -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut ifap: *mut libc::ifaddrs = std::ptr::null_mut();
    unsafe {
        if libc::getifaddrs(&mut ifap) != 0 {
            return Vec::new();
        }
        let mut cur = ifap;
        while !cur.is_null() {
            let ifa = &*cur;
            if !ifa.ifa_addr.is_null() {
                let sa = unsafe { &*ifa.ifa_addr };
                if sa.sa_family as i32 == libc::AF_INET {
                    let flags = ifa.ifa_flags as i32;
                    let up = flags & libc::IFF_UP != 0;
                    let loopback = flags & libc::IFF_LOOPBACK != 0;
                    if up && !loopback {
                        let sin = unsafe { &*(ifa.ifa_addr as *const libc::sockaddr_in) };
                        // `s_addr` está en orden de red en memoria.
                        let ip = Ipv4Addr::from(sin.sin_addr.s_addr.to_ne_bytes());
                        let s = ip.to_string();
                        if !out.contains(&s) {
                            out.push(s);
                        }
                    }
                }
            }
            cur = ifa.ifa_next;
        }
        libc::freeifaddrs(ifap);
    }
    out
}

/// Igual que [`local_ipv4_all`] pero sin `getifaddrs` (respaldo).
///
/// Truco UDP: abrir un socket y "conectar" a una IP pública **no envía nada**
/// (UDP no tiene handshake), sólo hace que el kernel elija la ruta por
/// defecto y devuelva la IP local.  Funciona sin internet siempre que haya
/// una ruta por defecto.
pub fn local_ipv4_udp_trick() -> Option<String> {
    let s = UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect("8.8.8.8:80").ok()?;
    let addr = s.local_addr().ok()?;
    match addr.ip() {
        IpAddr::V4(v4) if !v4.is_loopback() => Some(v4.to_string()),
        _ => None,
    }
}

/// La IP "principal": la primera IPv4 no-loopback.  Si hay varias, se
/// prefiere la que el kernel considera ruta por defecto (el truco UDP).
pub fn primary_ipv4() -> Option<String> {
    let all = local_ipv4_all();
    if let Some(pref) = local_ipv4_udp_trick() {
        if all.contains(&pref) {
            return Some(pref);
        }
        if all.is_empty() {
            return Some(pref);
        }
    }
    all.first().cloned()
}

/// URL HTTP directa al móvil por LAN.
pub fn lan_url(ip: &str, port: u16) -> String {
    format!("http://{ip}:{port}")
}

/// Contenido del beacon (JSON).  **Sin secretos**: sólo identidad, IPs y
/// puerto.  La API key y el token de sesión NUNCA salen por aquí.
pub fn beacon_payload(instance_id: &str, http_port: u16) -> serde_json::Value {
    let ips = local_ipv4_all();
    let primary = primary_ipv4();
    json!({
        "kind": BEACON_KIND,
        "id": instance_id,
        "ips": ips,
        "ip": primary,
        // `port` es el puerto CANÓNICO (fijo entre arranques): el que el
        // descubrimiento debe usar.  `primary_port` es el efímero que
        // eligió el SO, que cambia en cada reinicio: informativo, nunca
        // para construir una URL.
        "port": http_port,
        "url": primary.as_deref().map(|ip| lan_url(ip, http_port)),
        "ts": now_epoch(),
    })
}

/// Un beacon recibido.
#[derive(Debug, Clone)]
pub struct Beacon {
    /// Identificador de la instancia que emite.
    pub id: String,
    /// IPs declaradas por el emisor.
    pub ips: Vec<String>,
    /// Puerto HTTP del cockpit.
    pub port: u16,
    /// URL directa (si el emisor la declaró).
    pub url: Option<String>,
    /// Epoch de emisión.
    pub ts: u64,
}

impl Beacon {
    /// Convierte un JSON de beacon en estructura, validando el `kind`.
    pub fn from_json(v: &serde_json::Value) -> Option<Beacon> {
        if v.get("kind").and_then(|k| k.as_str()) != Some(BEACON_KIND) {
            return None;
        }
        let id = v.get("id").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let ips = v
            .get("ips")
            .and_then(|x| x.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|s| s.as_str().map(|x| x.to_string()))
                    .collect::<Vec<String>>()
            })
            .unwrap_or_default();
        let port = v.get("port").and_then(|p| p.as_u64()).unwrap_or(9090) as u16;
        let url = v
            .get("url")
            .and_then(|u| u.as_str())
            .map(|s| s.to_string());
        Some(Beacon {
            id,
            ips,
            port,
            url,
            ts: v.get("ts").and_then(|t| t.as_u64()).unwrap_or(0),
        })
    }
}

/// Escucha beacons durante `timeout` y devuelve el primero visto.
///
/// Pensado para el lado PC/sandbox: `md-lan-listen` o el script de Python.
pub fn listen_beacons(timeout: Duration) -> Option<Beacon> {
    let sock = UdpSocket::bind(("0.0.0.0", BEACON_PORT)).ok()?;
    sock.set_read_timeout(Some(Duration::from_millis(500))).ok()?;
    let deadline = std::time::Instant::now() + timeout;
    let mut buf = [0u8; 4096];
    while std::time::Instant::now() < deadline {
        match sock.recv_from(&mut buf) {
            Ok((n, _from)) => {
                if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&buf[..n]) {
                    if let Some(b) = Beacon::from_json(&v) {
                        return Some(b);
                    }
                }
            }
            Err(_) => continue,
        }
    }
    None
}

/// Envía el beacon una vez a la dirección de difusión y a las de cada
/// interfaz.  Devuelve cuántos paquetes se han enviado.
pub fn send_beacon_once(sock: &UdpSocket, payload: &str) -> usize {
    let mut n = 0;
    let targets = [
        SocketAddr::from((Ipv4Addr::BROADCAST, BEACON_PORT)),
        SocketAddr::from((Ipv4Addr::new(255, 255, 255, 255), BEACON_PORT)),
    ];
    for t in targets {
        if sock.send_to(payload.as_bytes(), t).is_ok() {
            n += 1;
        }
    }
    n
}

/// Arranca el emisor del beacon en segundo plano.
///
/// * Escribe `state/lan.json` (para el PC y para el canal).
/// * Emite por difusión cada [`BEACON_INTERVAL_S`] segundos.
/// * No falla si no hay red: es best-effort y no debe tumbar el daemon.
pub fn spawn_beacon(instance_id: String, http_port: u16, state_dir: Option<String>) {
    let _ = std::thread::Builder::new()
        .name("md-beacon".into())
        .spawn(move || {
            let sock = match UdpSocket::bind("0.0.0.0:0") {
                Ok(s) => s,
                Err(_) => return,
            };
            let _ = sock.set_broadcast(true);
            loop {
                let payload = beacon_payload(&instance_id, http_port).to_string();
                let _ = send_beacon_once(&sock, &payload);
                // Estado local: el PC puede leerlo si tiene acceso al disco
                // (o lo publica el canal).
                if let Some(dir) = state_dir.as_deref() {
                    let p = std::path::Path::new(dir).join("lan.json");
                    if let Some(parent) = p.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    let _ = std::fs::write(&p, &payload);
                }
                std::thread::sleep(Duration::from_secs(BEACON_INTERVAL_S));
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_beacon_no_lleva_ningun_secreto() {
        let p = beacon_payload("inst-1", 9090);
        let texto = p.to_string();
        assert_eq!(p["kind"], json!(BEACON_KIND));
        assert_eq!(p["id"], json!("inst-1"));
        assert_eq!(p["port"], json!(9090));
        for prohibido in ["api_key", "token", "secret", "password", "duckdns_token"] {
            assert!(
                !texto.to_lowercase().contains(prohibido),
                "el beacon no puede llevar '{prohibido}': {texto}"
            );
        }
    }

    #[test]
    fn el_beacon_es_reversible() {
        let p = beacon_payload("inst-2", 3030);
        let b = Beacon::from_json(&p).expect("debe parsear");
        assert_eq!(b.id, "inst-2");
        assert_eq!(b.port, 3030);
        assert!(b.ts > 0);
    }

    #[test]
    fn un_json_ajeno_no_es_un_beacon() {
        assert!(Beacon::from_json(&json!({"kind": "otro", "id": "x"})).is_none());
        assert!(Beacon::from_json(&json!({})).is_none());
    }

    #[test]
    fn el_loopback_no_se_anuncia() {
        for ip in local_ipv4_all() {
            assert_ne!(ip, "127.0.0.1", "no se anuncia el loopback");
            assert!(!ip.starts_with("127."), "no se anuncia loopback v4: {ip}");
        }
    }

    #[test]
    fn la_url_lan_tiene_forma_correcta() {
        assert_eq!(lan_url("192.168.1.124", 9090), "http://192.168.1.124:9090");
    }

    #[test]
    fn el_beacon_es_json_valido() {
        let p = beacon_payload("inst-3", 9090);
        let _: serde_json::Value = serde_json::from_str(&p.to_string()).expect("JSON válido");
    }
}
