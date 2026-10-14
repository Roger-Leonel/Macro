//! Converte um `.mrf` em `.kmacro`: `cargo run -p korvon-core --example mrf2kmacro -- entrada.mrf saida.kmacro`
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 2 {
        eprintln!("uso: mrf2kmacro <entrada.mrf> <saida.kmacro>");
        std::process::exit(2);
    }
    match korvon_core::mrf::import_file(std::path::Path::new(&args[0])) {
        Ok((m, warnings)) => {
            for w in &warnings {
                eprintln!("aviso: {w}");
            }
            m.save(std::path::Path::new(&args[1])).expect("falha ao salvar");
            println!("{} ações importadas", m.actions.len());
        }
        Err(e) => {
            eprintln!("erro: {e}");
            std::process::exit(1);
        }
    }
}
