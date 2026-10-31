//! `forge completions <shell>` — autocompletado de subcomandos.
//!
//! La CLI es un `match` a mano, así que tampoco hay deriving de clap que
//! regalar completados: se generan estáticos con la lista de subcomandos y las
//! flags largas comunes. Si cambia la superficie, `tests/dispatch.rs` es el
//! que obliga a tocar aquí también (el test compara la lista).

use crate::ExitCode;

/// la lista canónica de subcomandos (dispatch la usa y el test la compara)
pub const SUBCOMANDOS: &[&str] = &[
    "bench",
    "builds",
    "backup",
    "completions",
    "console",
    "detect",
    "doctor",
    "dump",
    "export",
    "generate",
    "guided",
    "info",
    "install",
    "latest",
    "list",
    "logs",
    "mods",
    "new",
    "plan",
    "plugins",
    "pregen",
    "rcon",
    "restore",
    "search",
    "self-test",
    "status",
    "stop",
    "update",
    "versions",
    "whitelist",
    "op",
    "world",
    "help",
];

pub fn completions_cmd(args: &[String]) -> ExitCode {
    let Some(shell) = args.first() else {
        eprintln!("uso: forge completions bash|zsh|fish|powershell");
        return ExitCode::from(2);
    };
    let sub = SUBCOMANDOS.join(" ");
    match shell.as_str() {
        "bash" => {
            println!("_forge() {{ local cur; cur=${{COMP_WORDS[COMP_CWORD]}}; COMPREPLY=($(compgen -W \"{sub}\" -- \"$cur\")); }}");
            println!("complete -F _forge forge");
            ExitCode::SUCCESS
        }
        "zsh" => {
            println!("#compdef forge");
            println!("_forge() {{ _arguments '1: :({sub})' '*::arg:->args' }}");
            println!("_forge \"$@\"");
            ExitCode::SUCCESS
        }
        "fish" => {
            for s in SUBCOMANDOS {
                println!("complete -c forge -n '__fish_use_subcommand' -a '{s}'");
            }
            ExitCode::SUCCESS
        }
        "powershell" | "pwsh" => {
            println!("Register-ArgumentCompleter -CommandName forge -ScriptBlock {{");
            println!("    param($wordToComplete)");
            println!("    '{sub}'.Split(' ') | Where-Object {{ $_ -like \"$wordToComplete*\" }} | ForEach-Object {{");
            println!("        [System.Management.Automation.CompletionResult]::new($_, $_, 'ParameterValue', $_)");
            println!("    }}");
            println!("}}");
            ExitCode::SUCCESS
        }
        otra => {
            eprintln!("shell desconocida: {otra} (bash|zsh|fish|powershell)");
            ExitCode::from(2)
        }
    }
}
