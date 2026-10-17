//! Scripts de arranque e instalación, portados de `src/generators/scripts.ts`.
//!
//! El server generado se ejecuta igual en Windows (`start.bat`) y Linux
//! (`start.sh`), y el zip del proyecto se instala en otra máquina sin tener
//! OpenBlock encima.

mod ps;
mod sh;

pub use ps::{install_ps1, start_bat};
pub use sh::{install_sh, jvm_args_txt, shell_quote, start_sh, user_jvm_args_txt};
