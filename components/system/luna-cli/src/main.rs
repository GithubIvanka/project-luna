fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() >= 2 && args[0] == "app" && ["run", "list"].contains(&args[1].as_str()) {
        match luna_cli::run_live(&args) {
            Ok((output, code)) => {
                print!("{output}");
                std::process::exit(code);
            }
            Err(error) => {
                eprintln!("luna: {error}");
                std::process::exit(1);
            }
        }
    }
    match luna_cli::run(&args) {
        Ok(output) => print!("{output}"),
        Err(error) => {
            eprintln!("luna: {error}");
            std::process::exit(1);
        }
    }
}
