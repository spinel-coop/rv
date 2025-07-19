use miette::miette;
use rv_gem_specification_yaml::parse;
use std::env;

fn main() -> miette::Result<()> {
    let args: Vec<String> = env::args().collect();

    let yaml_content = if args.len() > 1 {
        // Read from file if provided
        let filename = &args[1];
        std::fs::read_to_string(filename)
            .map_err(|e| miette!("Failed to read file '{}': {}", filename, e))?
    } else {
        // Use example YAML if no file provided
        include_str!("../tests/fixtures/simple_spec.yaml").to_string()
    };

    // Create a named source for better error reporting

    println!("Parsing YAML specification...\n");

    let spec = parse(&yaml_content)?;
    {
        println!("✅ Successfully parsed gem specification!");
        println!("\n📋 Specification Details:");
        println!("=========================");
        println!("{spec:#?}");
    }

    Ok(())
}
