//! Las conversiones de JavaScript que el port necesita reproducir.
//!
//! El TS lee JSON con `as`, así que un campo que falta no es un error: es
//! `undefined`, y meterlo en una plantilla escribe literalmente `"undefined"`.
//! Eso sale en las URLs que se descargan y en los nombres de fichero, así que se
//! reproduce en vez de arreglarse.

use serde_json::Value;

/// `String(v)`, incluido el `"undefined"` de un campo que no está.
pub fn string(v: Option<&Value>) -> String {
    match v {
        None => "undefined".to_string(),
        Some(Value::Null) => "null".to_string(),
        Some(Value::Bool(b)) => b.to_string(),
        Some(Value::Number(n)) => num(n),
        Some(Value::String(s)) => s.clone(),
        // `String([])` es "", `String([1,2])` es "1,2"; los objetos son "[object Object]"
        Some(Value::Array(a)) => a
            .iter()
            .map(|e| match e {
                Value::Null => String::new(),
                otra => string(Some(otra)),
            })
            .collect::<Vec<_>>()
            .join(","),
        Some(Value::Object(_)) => "[object Object]".to_string(),
    }
}

/// Un número como lo escribiría JS en una plantilla.
///
/// `1.0` es `"1"` en JS y `"1.0"` en `serde_json`, y los ids de build son
/// enteros, así que el caso que importa es ese: los enteros salen enteros.
pub fn num(n: &serde_json::Number) -> String {
    if let Some(i) = n.as_i64() {
        return i.to_string();
    }
    if let Some(u) = n.as_u64() {
        return u.to_string();
    }
    match n.as_f64() {
        Some(f) if f.fract() == 0.0 && f.is_finite() && f.abs() < 1e21 => format!("{}", f as i64),
        Some(f) => format!("{f}"),
        None => n.to_string(),
    }
}

/// `Number(v)`, que es lo que el TS hace al ordenar builds. Un valor que no es
/// número da `NaN`, y en JS un comparador que devuelve `NaN` no reordena nada:
/// eso importa para que el orden de los casos raros sea el mismo.
pub fn number(v: Option<&Value>) -> f64 {
    match v {
        None => f64::NAN,
        Some(Value::Null) => 0.0,
        Some(Value::Bool(b)) => {
            if *b {
                1.0
            } else {
                0.0
            }
        }
        Some(Value::Number(n)) => n.as_f64().unwrap_or(f64::NAN),
        Some(Value::String(s)) => {
            let t = s.trim();
            if t.is_empty() {
                0.0
            } else {
                t.parse().unwrap_or(f64::NAN)
            }
        }
        // `Number([])` es 0 y `Number([5])` es 5; con más de un elemento, NaN
        Some(Value::Array(a)) => match a.len() {
            0 => 0.0,
            1 => number(a.first()),
            _ => f64::NAN,
        },
        Some(Value::Object(_)) => f64::NAN,
    }
}

#[cfg(test)]
mod tests {
    use super::{num, string};
    use serde_json::{json, Value};

    #[test]
    fn un_campo_que_no_esta_es_undefined() {
        let v = json!({ "otro": 1 });
        assert_eq!(string(v.get("version")), "undefined");
        assert_eq!(string(v.get("otro")), "1");
        assert_eq!(string(Some(&Value::Null)), "null");
    }

    #[test]
    fn los_objetos_y_las_listas_como_en_js() {
        assert_eq!(string(Some(&json!([1, 2]))), "1,2");
        assert_eq!(string(Some(&json!([1, null, 2]))), "1,,2");
        assert_eq!(string(Some(&json!({}))), "[object Object]");
        assert_eq!(string(Some(&json!(true))), "true");
    }

    #[test]
    fn los_enteros_no_llevan_decimales() {
        assert_eq!(num(&json!(47).as_number().unwrap().clone()), "47");
        assert_eq!(num(&json!(47.0).as_number().unwrap().clone()), "47");
        assert_eq!(num(&json!(-3).as_number().unwrap().clone()), "-3");
    }
}
