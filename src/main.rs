//! Binary entry point for {{crate_name}}.

#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(missing_docs)]

use {{crate_name}}::{add, divide, Config};

/// Main entry point.
fn main() {
    // Example usage
    let config = Config::new().with_verbose(true);

    if config.verbose {
        eprintln!("Running {{crate_name}} with verbose mode enabled");
    }

    // Demonstrate add function
    let sum = add(2, 3);
    eprintln!("2 + 3 = {sum}");

    // Demonstrate divide function with error handling
    match divide(10, 2) {
        Ok(result) => eprintln!("10 / 2 = {result}"),
        Err(e) => eprintln!("Error: {e}"),
    }

    // Demonstrate error case
    match divide(10, 0) {
        Ok(result) => eprintln!("10 / 0 = {result}"),
        Err(e) => eprintln!("Expected error: {e}"),
    }
}
