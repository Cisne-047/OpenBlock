//! Códec RCON (protocolo de Valve), portado de `src/rcon.ts`.
//!
//! OpenBlock lo usa para automatizar la pregeneración con Chunky y consultar
//! spark sin tocar la consola. Solo va el **códec** a propósito: el cliente TCP
//! necesita un socket y por tanto vive en el borde, como el resto de la E/S. El
//! formato sí es puro, y es justo donde la paridad byte a byte importa más: un
//! byte mal puesto en un protocolo binario no da un error legible, da un server
//! que no responde.
//!
//! Packet: `[len:i32le][id:i32le][type:i32le][payload ascii][0x00 0x00]`

/// Tipo del paquete de autenticación.
pub const SERVERDATA_AUTH: i32 = 3;
/// Respuesta a la autenticación.
pub const SERVERDATA_AUTH_RESPONSE: i32 = 2;
/// Ejecutar un comando.
pub const SERVERDATA_EXECCOMMAND: i32 = 2;
/// Respuesta a un comando.
pub const SERVERDATA_RESPONSE_VALUE: i32 = 0;

/// Paquete decodificado (`DecodedPacket`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedPacket {
    pub id: i32,
    pub kind: i32,
    pub payload: String,
}

/// El TS llama `type` al campo, pero es palabra reservada en Rust; se expone
/// como `kind` y así se serializa.
pub use DecodedPacket as Packet;

/// Codifica un paquete RCON (`encodePacket`).
///
/// Reproduce un quirk del TS: `Buffer.alloc(4 + 4 + Buffer.byteLength(payload) + 2)`
/// mide el payload en **UTF-8**, pero `write(payload, 8, "ascii")` escribe
/// **latin1**, que ocupa menos. Los bytes que sobran quedan a cero (el `alloc` los
/// rellena), así que la longitud declarada es la de UTF-8 y el payload real puede
/// ser más corto, con relleno de ceros hasta esa longitud y luego los dos ceros
/// finales.
///
/// Con payload ASCII —que es el 100% de los comandos reales: `chunky radius N`,
/// `spark tps`— los dos cómputos coinciden y el quirk no se nota. Con acentos sí,
/// y un RCON con la longitud mal no responde: da igual que el caso no se dé en la
/// práctica, porque el puerto tiene que ser byte a byte o no es un puerto.
pub fn encode_packet(id: i32, kind: i32, payload: &str) -> Vec<u8> {
    // el TS mide en UTF-8 para el alloc...
    let declarado = payload.len();
    // ...y escribe latin1 (el byte bajo de cada carácter)
    let latin1: Vec<u8> = payload.chars().map(|c| (c as u32 & 0xFF) as u8).collect();

    let cuerpo = 4 + 4 + declarado + 2;
    let mut out = vec![0u8; 4 + cuerpo]; // relleno a cero, como Buffer.alloc
    out[0..4].copy_from_slice(&(cuerpo as i32).to_le_bytes());
    out[4..8].copy_from_slice(&id.to_le_bytes());
    out[8..12].copy_from_slice(&kind.to_le_bytes());
    // el payload latin1, dejando ceros hasta la longitud declarada
    out[12..12 + latin1.len().min(declarado)]
        .copy_from_slice(&latin1[..latin1.len().min(declarado)]);
    out
}

/// Decodifica un paquete RCON (`decodePacket`).
///
/// Devuelve `None` si el buffer es demasiado corto o si la longitud declarada no
/// cuadra con lo que hay. El TS lee sin comprobar y deja que `Buffer` reviente
/// con un `RangeError`; aquí es un `Option`, porque quien llama —el cliente TCP—
/// está leyendo de un socket y un `panic` en ese punto tumbaría el proceso.
pub fn decode_packet(buf: &[u8]) -> Option<DecodedPacket> {
    if buf.len() < 12 {
        return None;
    }
    let len = i32::from_le_bytes(buf[0..4].try_into().ok()?);
    if len < 10 || buf.len() < 4 + len as usize {
        return None;
    }
    let id = i32::from_le_bytes(buf[4..8].try_into().ok()?);
    let kind = i32::from_le_bytes(buf[8..12].try_into().ok()?);
    // `subarray(12, 4 + len - 2)`: deja fuera los dos ceros finales
    let fin = 4 + len as usize - 2;
    let payload = buf.get(12..fin).unwrap_or(&[]);
    // Latin-1: `toString("ascii")` de Node con un buffer también va por esa ruta,
    // así que cada byte es un carácter, no una secuencia UTF-8
    let payload = payload.iter().map(|b| *b as char).collect();
    Some(DecodedPacket { id, kind, payload })
}

/// Cuántos bytes ocupa el paquete completo, para el framing del cliente.
pub fn packet_len(buf: &[u8]) -> Option<usize> {
    if buf.len() < 4 {
        return None;
    }
    let len = i32::from_le_bytes(buf[0..4].try_into().ok()?);
    if len < 10 {
        return None;
    }
    Some(4 + len as usize)
}
