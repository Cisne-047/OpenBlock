//! RCON: mismos casos que `tests/protocol.test.ts`, más paridad byte a byte de
//! los paquetes que genera el TS.

use forge_core::rcon;
use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("raíz del repo")
}

// ---------- rcon codec (tests/protocol.test.ts:5-17) ----------

#[test]
fn roundtrip_auth() {
    let buf = rcon::encode_packet(7, 3, "pass123");
    let len = i32::from_le_bytes(buf[0..4].try_into().unwrap());
    assert_eq!(len, 4 + 4 + 7 + 2);
    assert_eq!(
        rcon::decode_packet(&buf),
        Some(rcon::DecodedPacket {
            id: 7,
            kind: 3,
            payload: "pass123".to_string()
        })
    );
}

#[test]
fn roundtrip_comando_con_espacios() {
    let buf = rcon::encode_packet(1, 2, "chunky radius 4000");
    assert_eq!(
        rcon::decode_packet(&buf).unwrap().payload,
        "chunky radius 4000"
    );
}

// ---------- casos que el TS no prueba, y que aquí sí ----------

#[test]
fn framing_del_paquete_vacio() {
    // payload vacío: len = 10, el mínimo válido del protocolo
    let buf = rcon::encode_packet(2, 0, "");
    assert_eq!(rcon::packet_len(&buf), Some(14));
    assert_eq!(rcon::decode_packet(&buf).unwrap().payload, "");
}

#[test]
fn el_payload_no_ascii_arrastra_el_quirk_del_ts() {
    // El TS mide el payload en UTF-8 para el `alloc` (11 bytes para "acentué ñ")
    // pero lo escribe en latin1 (9 bytes), así que sobran 2 ceros antes de los 2
    // finales. Lo cazó la paridad: mi primera versión declaraba la longitud de
    // latin1 y el golden del TS decía otra cosa.
    let buf = rcon::encode_packet(1, 2, "acentué ñ");
    // el payload empieza en el byte 12 (tras len+id+type) y va en latin1
    assert_eq!(
        &buf[12..21],
        &[0x61, 0x63, 0x65, 0x6e, 0x74, 0x75, 0xe9, 0x20, 0xf1]
    );
    // y luego el relleno: 2 ceros del alloc + los 2 del protocolo
    assert_eq!(&buf[21..], &[0, 0, 0, 0]);
    // la longitud declarada es la de UTF-8: 4+4+11+2
    assert_eq!(i32::from_le_bytes(buf[0..4].try_into().unwrap()), 21);
    assert_eq!(buf.len(), 4 + 21);
}

#[test]
fn con_ascii_el_quirk_no_se_nota() {
    // que es el caso real: los comandos que OpenBlock manda son ASCII
    let buf = rcon::encode_packet(1, 2, "chunky radius 4000");
    assert_eq!(
        i32::from_le_bytes(buf[0..4].try_into().unwrap()),
        4 + 4 + 18 + 2
    );
    assert_eq!(buf.len(), 4 + 28);
    assert_eq!(
        rcon::decode_packet(&buf).unwrap().payload,
        "chunky radius 4000"
    );
}

#[test]
fn un_buffer_corto_o_incoherente_no_revienta() {
    // El TS leería sin comprobar y lanzaría RangeError; aquí es None, porque
    // quien llama está leyendo de un socket y un panic tumbaría el proceso.
    assert_eq!(rcon::decode_packet(&[]), None);
    assert_eq!(rcon::decode_packet(&[1, 2, 3]), None);
    // longitud declarada mayor que el buffer
    let mut buf = rcon::encode_packet(1, 2, "hola");
    buf[0] = 99;
    assert_eq!(rcon::decode_packet(&buf), None);
    // longitud declarada imposible (< 10)
    let mut buf2 = rcon::encode_packet(1, 2, "hola");
    buf2[0] = 1;
    assert_eq!(rcon::decode_packet(&buf2), None);
}

#[test]
fn ids_en_los_extremos_del_i32() {
    for id in [0i32, 1, -1, i32::MAX, i32::MIN] {
        let buf = rcon::encode_packet(id, 2, "x");
        assert_eq!(rcon::decode_packet(&buf).unwrap().id, id, "id {id}");
    }
}

// ---------- paridad byte a byte ----------

#[test]
fn los_paquetes_son_byte_a_byte_los_del_ts() {
    let p = repo_root().join("rust/crates/forge-core/tests/golden/rcon.json");
    let txt =
        fs::read_to_string(&p).unwrap_or_else(|e| panic!("no se pudo leer {}: {e}", p.display()));
    let g: serde_json::Value = serde_json::from_str(&txt).expect("golden inválido");
    let casos = g["casos"].as_array().expect("casos");
    assert!(!casos.is_empty());

    for caso in casos {
        let id = caso["id"].as_i64().unwrap() as i32;
        let tipo = caso["tipo"].as_i64().unwrap() as i32;
        let payload = caso["payload"].as_str().unwrap();
        let esperado = caso["hex"].as_str().unwrap();

        let mio: String = rcon::encode_packet(id, tipo, payload)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assert_eq!(mio, esperado, "paquete ({id}, {tipo}, {payload:?})");

        // y el roundtrip devuelve lo mismo que se metió (salvo lo no-ASCII)
        let buf = rcon::encode_packet(id, tipo, payload);
        let dec = rcon::decode_packet(&buf).expect("decodifica");
        assert_eq!(dec.id, id);
        assert_eq!(dec.kind, tipo);
    }
}
