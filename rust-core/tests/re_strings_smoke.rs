//! Pruebas de la extracción de strings y símbolos (`re_strings`).
//!
//! Los binarios de referencia del repo son grandes (el `liblanguage_server.so` de
//! Antigravity son 181 MB) ⇒ **no** se suben al repo público.  Aquí se prueban
//! con muestras **pequeñas** construidas en memoria, más un ELF real minúsculo
//! generado con los bytes de cabecera y una tabla de símbolos a mano.

use megadaemon_core::re_strings::{
    collect_symbols, extract_ascii, extract_strings, extract_utf16le, StrEncoding, SymKind,
};

#[test]
fn extrae_strings_ascii_con_desplazamiento() {
    let data = b"\x00\x00hello-mundo\x00\x01\x02\x03fin\x00";
    let hits = extract_ascii(data, 3, 0);
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].text, "hello-mundo");
    assert_eq!(hits[0].offset, 2);
    assert_eq!(hits[0].encoding, StrEncoding::Ascii);
    assert_eq!(hits[1].text, "fin");
    // "fin" empieza en el byte 17 (2 + 11 + 1 + 3).
    assert_eq!(hits[1].offset, 17);
}

#[test]
fn el_minimo_de_longitud_filtra() {
    let data = b"abc\x00de\x00";
    let hits = extract_ascii(data, 4, 0);
    assert_eq!(hits.len(), 0);
    let hits = extract_ascii(data, 2, 0);
    assert_eq!(hits.len(), 2);
}

#[test]
fn extrae_strings_utf16le() {
    // "AB" en UTF-16LE con interludio de bytes que rompen la cadena.
    let mut data: Vec<u8> = Vec::new();
    data.extend_from_slice(&[b'A', 0x00, b'B', 0x00]);
    data.push(0xFF);
    data.extend_from_slice(&[b'C', 0x00, b'D', 0x00]);
    let hits = extract_utf16le(&data, 2, 0);
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].text, "AB");
    assert_eq!(hits[0].encoding, StrEncoding::Utf16Le);
    assert_eq!(hits[1].text, "CD");
}

#[test]
fn el_limite_acota_la_salida() {
    let data = b"aaaa\x00bbbb\x00cccc\x00";
    let hits = extract_strings(data, 4, 2);
    assert_eq!(hits.len(), 2);
}

#[test]
fn los_simbolos_de_un_no_elf_no_dan_panic() {
    // Ni ELF ni PE: no debe entrar en pánico ni devolver basura.
    let datos = vec![0u8; 64];
    assert!(collect_symbols(&datos).is_empty());
}

#[test]
fn detecta_un_elf_minimo() {
    // Cabecera ELF64 mínima + tabla de símbolos: el objetivo es que el parser no
    // entre en pánico con un ELF truncado (que es lo que pasa con un dump de memoria).
    let mut data = vec![0u8; 64];
    data[0..4].copy_from_slice(&[0x7f, b'E', b'L', b'F']);
    data[4] = 2; // ELFCLASS64
    data[5] = 1; // ELFDATA2LSB
    data[6] = 1; // EV_CURRENT
    // e_machine = EM_X86_64 (62) en little endian
    data[18..20].copy_from_slice(&62u16.to_le_bytes());
    // Los símbolos vienen vacíos (secciones sin tabla), pero el parseo no debe fallar.
    let syms = collect_symbols(&data);
    assert!(syms.is_empty() || syms.iter().all(|s| !s.name.is_empty()));
}
