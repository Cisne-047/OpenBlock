//! Cliente RCON (protocolo de Valve), portado de `src/rcon.ts`.
//!
//! El **códec** está en `forge-core` y tiene paridad byte a byte; esto es el
//! socket, que vive en el CLI porque toca la red. El doctor lo necesita para el
//! apagado limpio del boot test y `apply` para lanzar la pregeneración de Chunky.
//!
//! El TS es asíncrono con callbacks y una cola de `pending`; aquí es **síncrono y
//! bloqueante** con timeouts de socket, que para un CLI es más simple y no obliga
//! a traer un runtime asíncrono. Los timeouts son los mismos del TS: 8 s para
//! conectar y 15 s por respuesta.
//!
//! Ojo con un detalle del original que se reproduce: `send` resuelve con el
//! **primer** paquete que llega, no con el que tenga el id que se mandó. El
//! protocolo permite que un servidor conteste a `AUTH` con un `RESPONSE_VALUE`
//! vacío antes del `AUTH_RESPONSE`; Paper no lo hace, y el self-test del TS hace un
//! apagado limpio por RCON contra Paper de verdad, así que funciona. Se porta igual
//! y se anota.

use forge_core::rcon::{
    decode_packet, encode_packet, DecodedPacket, SERVERDATA_AUTH, SERVERDATA_EXECCOMMAND,
};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

/// Timeout de conexión, como el `timeoutMs = 8000` del TS.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(8);
/// Timeout por respuesta, como el `15000` del TS.
pub const RESPONSE_TIMEOUT: Duration = Duration::from_secs(15);

/// Error del cliente, con los mensajes del TS donde los hay.
#[derive(Debug)]
pub enum Error {
    /// `RCON timeout de conexión`
    Conexion(String),
    /// `RCON: contraseña rechazada`
    PasswordRechazada,
    /// `RCON: sin respuesta`
    SinRespuesta,
    /// El servidor mandó algo que no se puede decodificar.
    Protocolo(String),
    /// Fallo de escritura.
    Escritura(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Conexion(e) => write!(f, "RCON timeout de conexión: {e}"),
            Self::PasswordRechazada => write!(f, "RCON: contraseña rechazada"),
            Self::SinRespuesta => write!(f, "RCON: sin respuesta"),
            Self::Protocolo(e) => write!(f, "RCON: respuesta ilegible: {e}"),
            Self::Escritura(e) => write!(f, "RCON: no se pudo enviar: {e}"),
        }
    }
}

impl std::error::Error for Error {}

/// Cliente RCON conectado (`RconClient`).
#[derive(Debug)]
pub struct RconClient {
    socket: TcpStream,
    /// Bytes que han llegado pero aún no forman un paquete completo.
    buffer: Vec<u8>,
    /// Id del siguiente paquete; empieza en 1 como el TS.
    next_id: i32,
}

impl RconClient {
    /// Conecta y autentica (`connect`).
    pub fn connect(host: &str, port: u16, password: &str) -> Result<Self, Error> {
        let destino = format!("{host}:{port}");
        let direcciones = std::net::ToSocketAddrs::to_socket_addrs(&destino)
            .map_err(|e| Error::Conexion(e.to_string()))?;
        let mut ultimo = None;
        let mut socket = None;
        for dir in direcciones {
            match TcpStream::connect_timeout(&dir, CONNECT_TIMEOUT) {
                Ok(s) => {
                    socket = Some(s);
                    break;
                }
                Err(e) => ultimo = Some(e.to_string()),
            }
        }
        let socket = socket
            .ok_or_else(|| Error::Conexion(ultimo.unwrap_or_else(|| "sin direcciones".into())))?;
        socket.set_read_timeout(Some(RESPONSE_TIMEOUT)).ok();
        socket.set_write_timeout(Some(RESPONSE_TIMEOUT)).ok();

        let mut cliente = Self {
            socket,
            buffer: Vec::new(),
            next_id: 1,
        };
        let res = cliente.send(SERVERDATA_AUTH, password)?;
        if res.id == -1 {
            return Err(Error::PasswordRechazada);
        }
        Ok(cliente)
    }

    /// Envía un paquete y devuelve la primera respuesta (`send`).
    fn send(&mut self, kind: i32, payload: &str) -> Result<DecodedPacket, Error> {
        let id = self.next_id;
        self.next_id += 1;
        let paquete = encode_packet(id, kind, payload);
        self.socket
            .write_all(&paquete)
            .map_err(|e| Error::Escritura(e.to_string()))?;
        self.leer_paquete()
    }

    /// Lee hasta tener un paquete completo (`onData` con su acumulador).
    ///
    /// El TS acumula y va sacando paquetes del buffer; aquí se lee lo que haga
    /// falta hasta completar uno, que es lo mismo pero en bloqueante. El buffer se
    /// conserva entre llamadas: si llegan dos paquetes juntos, el segundo no se
    /// pierde.
    fn leer_paquete(&mut self) -> Result<DecodedPacket, Error> {
        loop {
            // ¿ya hay uno completo en el buffer?
            if self.buffer.len() >= 4 {
                let len = i32::from_le_bytes(self.buffer[0..4].try_into().expect("4 bytes"));
                if len >= 10 && self.buffer.len() >= 4 + len as usize {
                    let resto = self.buffer.split_off(4 + len as usize);
                    let completo = std::mem::replace(&mut self.buffer, resto);
                    return decode_packet(&completo)
                        .ok_or_else(|| Error::Protocolo("frame inválido".into()));
                }
            }
            let mut trozo = [0u8; 4096];
            match self.socket.read(&mut trozo) {
                Ok(0) => return Err(Error::SinRespuesta),
                Ok(n) => self.buffer.extend_from_slice(&trozo[..n]),
                Err(e)
                    if e.kind() == std::io::ErrorKind::WouldBlock
                        || e.kind() == std::io::ErrorKind::TimedOut =>
                {
                    return Err(Error::SinRespuesta)
                }
                Err(e) => return Err(Error::Protocolo(e.to_string())),
            }
        }
    }

    /// Ejecuta un comando y devuelve su salida recortada (`command`).
    pub fn command(&mut self, cmd: &str) -> Result<String, Error> {
        let res = self.send(SERVERDATA_EXECCOMMAND, cmd)?;
        Ok(res.payload.trim().to_string())
    }

    /// Cierra la conexión (`end`).
    pub fn end(self) {
        // un `shutdown` explícito para que el server vea el cierre sin esperar
        let _ = self.socket.shutdown(std::net::Shutdown::Both);
    }
}

/// Conecta, ejecuta `fn` y cierra siempre (`withRcon`).
///
/// El `finally` del TS: si `fn` falla, la conexión se cierra igual.
pub fn with_rcon<T>(
    host: &str,
    port: u16,
    password: &str,
    f: impl FnOnce(&mut RconClient) -> Result<T, Error>,
) -> Result<T, Error> {
    let mut rcon = RconClient::connect(host, port, password)?;
    let resultado = f(&mut rcon);
    rcon.end();
    resultado
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_core::rcon::{SERVERDATA_AUTH_RESPONSE, SERVERDATA_RESPONSE_VALUE};
    use std::net::TcpListener;

    /// Servidor RCON de mentira: acepta una conexión y contesta lo que se le diga.
    ///
    /// Existe porque el cliente es justo lo que no se puede probar con un golden
    /// —necesita un socket—, y arrancar un Minecraft de verdad para probar el
    /// apagado por RCON sería un test de 40 segundos. El protocolo es simple y un
    /// fake de 30 líneas cubre el handshake, el framing y el recorte.
    fn servidor_falso(
        password_ok: bool,
        respuesta_comando: &'static str,
    ) -> (u16, std::thread::JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let puerto = listener.local_addr().expect("addr").port();
        let hilo = std::thread::spawn(move || {
            let (mut sock, _) = listener.accept().expect("accept");
            let mut recibidos = Vec::new();
            for _ in 0..2 {
                // leer un paquete completo: cabecera de 4 + cuerpo
                let mut cabecera = [0u8; 4];
                sock.read_exact(&mut cabecera).expect("cabecera");
                let len = i32::from_le_bytes(cabecera) as usize;
                let mut cuerpo = vec![0u8; len];
                sock.read_exact(&mut cuerpo).expect("cuerpo");
                let id = i32::from_le_bytes(cuerpo[0..4].try_into().unwrap());
                let tipo = i32::from_le_bytes(cuerpo[4..8].try_into().unwrap());
                let payload = String::from_utf8_lossy(&cuerpo[8..len - 2]).to_string();
                recibidos.push(format!("{tipo}:{payload}"));

                if tipo == SERVERDATA_AUTH {
                    // el protocolo contesta a AUTH con AUTH_RESPONSE; id -1 = mal
                    let id_resp = if password_ok { id } else { -1 };
                    let p = encode_packet(id_resp, SERVERDATA_AUTH_RESPONSE, "");
                    sock.write_all(&p).expect("escribir auth");
                } else {
                    let p = encode_packet(id, SERVERDATA_RESPONSE_VALUE, respuesta_comando);
                    sock.write_all(&p).expect("escribir respuesta");
                }
            }
            recibidos
        });
        (puerto, hilo)
    }

    #[test]
    fn handshake_y_comando_contra_un_servidor_de_mentira() {
        let (puerto, hilo) = servidor_falso(true, "TPS: 20.0");
        let salida = with_rcon("127.0.0.1", puerto, "secreto123", |r| {
            r.command("spark tps")
        })
        .expect("va");
        assert_eq!(salida, "TPS: 20.0");
        // y se enviaron los dos paquetes que toca: AUTH con la contraseña y el comando
        let recibidos = hilo.join().expect("el hilo no debe paniquear");
        assert_eq!(
            recibidos,
            vec!["3:secreto123".to_string(), "2:spark tps".to_string()]
        );
    }

    #[test]
    fn la_contrasena_rechazada_se_detecta() {
        // id -1 en la respuesta a AUTH es "contraseña rechazada", igual que el TS
        let (puerto, hilo) = servidor_falso(false, "");
        let e = RconClient::connect("127.0.0.1", puerto, "mala").expect_err("debe fallar");
        assert!(
            matches!(e, Error::PasswordRechazada),
            "error inesperado: {e}"
        );
        let _ = hilo.join();
    }

    #[test]
    fn el_payload_se_recorta_como_en_el_ts() {
        // `command` hace `.trim()` del payload: los saltos de línea del server no salen
        let (puerto, hilo) = servidor_falso(true, "  \n  hola  \n  ");
        let salida = with_rcon("127.0.0.1", puerto, "x", |r| r.command("list")).expect("va");
        assert_eq!(salida, "hola");
        let _ = hilo.join();
    }

    #[test]
    fn un_puerto_sin_nadie_escuchando_da_error_de_conexion() {
        // Se reserva un puerto y se cierra: nadie escucha, y el error tiene que ser
        // de conexión, no un panic.
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let puerto = listener.local_addr().unwrap().port();
        drop(listener);
        let e = RconClient::connect("127.0.0.1", puerto, "x").expect_err("debe fallar");
        assert!(matches!(e, Error::Conexion(_)), "error inesperado: {e}");
    }
}
