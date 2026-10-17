//! `encodeURIComponent` de JavaScript, para construir las URLs igual que el TS.
//!
//! No vale `percent-encoding` ni `url::form_urlencoded`: la primera no existe en
//! el árbol de dependencias, y la segunda codifica el espacio como `+`. La regla
//! de JS es corta y conviene tenerla explícita: **no** se escapan
//! `A-Za-z0-9` ni `-_.!~*'()`; todo lo demás va a `%XX` en hexadecimal
//! **mayúscula**, byte a byte del UTF-8.

/// El conjunto que JS deja pasar tal cual.
fn no_se_escapa(b: u8) -> bool {
    b.is_ascii_alphanumeric()
        || matches!(
            b,
            b'-' | b'_' | b'.' | b'!' | b'~' | b'*' | b'\'' | b'(' | b')'
        )
}

/// Equivalente de `encodeURIComponent`. Con `&str` no puede haber sustitutos
/// sueltos, que es el único caso en el que el TS lanza `URIError`.
pub fn encode_uri_component(s: &str) -> String {
    let mut salida = String::with_capacity(s.len());
    for b in s.bytes() {
        if no_se_escapa(b) {
            salida.push(b as char);
        } else {
            salida.push('%');
            salida.push_str(&format!("{b:02X}"));
        }
    }
    salida
}

#[cfg(test)]
mod tests {
    use super::encode_uri_component;

    #[test]
    fn deja_pasar_lo_que_deja_pasar_js() {
        // el mismo caso que el golden: nada de esto se escapa
        assert_eq!(encode_uri_component("a'b(c)!*~-_."), "a'b(c)!*~-_.");
        assert_eq!(encode_uri_component("1.20.1"), "1.20.1");
        assert_eq!(encode_uri_component(""), "");
    }

    #[test]
    fn escapa_lo_demas_en_mayusculas() {
        assert_eq!(encode_uri_component("a b"), "a%20b");
        assert_eq!(encode_uri_component("1.20.1+build"), "1.20.1%2Bbuild");
        assert_eq!(encode_uri_component("100%"), "100%25");
        // multibyte: se escapa byte a byte
        assert_eq!(encode_uri_component("ñ/é"), "%C3%B1%2F%C3%A9");
        assert_eq!(encode_uri_component("日本"), "%E6%97%A5%E6%9C%AC");
    }
}
