use std::io::{self, IsTerminal};

use monkey::repl;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    ctrlc::set_handler(|| {
        eprintln!("\nGoodbye!");
        std::process::exit(0);
    })?;

    // Get current user name
    let user = std::env::var("USER").unwrap_or_else(|_| "friend".to_owned());
    println!("Hello, {}! This is the Monkey programming language!", user);
    println!("Feel free to type in commands");

    let stdin = io::stdin();
    let stdout = io::stdout();
    if stdin.is_terminal() && stdout.is_terminal() {
        repl::start_interactive()?;
    } else {
        let mut input = stdin.lock();
        let mut output = stdout.lock();
        repl::start(&mut input, &mut output);
    }
    Ok(())
}
