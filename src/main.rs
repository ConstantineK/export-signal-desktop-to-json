use clap::Parser;
use rusqlite::Result;
use std::env;
use std::path::PathBuf;

extern crate export_signal_desktop as get_signal;
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[clap(short, long, default_value = "not set")]
    config_path: String,
    #[clap(short, long, default_value = "not set")]
    database_path: String,
    #[clap(short, long, default_value = "not set")]
    output_directory: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let mut signal_config_path = args.config_path;
    let mut signal_database_path = args.database_path;
    let mut output_directory = args.output_directory;

    if signal_config_path.clone() == "not set" {
        // Check if the environment variable is set
        match env::var("SIGNAL_CONFIG_PATH") {
            Ok(val) => {
                signal_config_path = val;
            }
            Err(e) => {
                println!("Neither the --config_path nor the environment variable SIGNAL_CONFIG_PATH were set.");
                println!("Error: {}", e);
                std::process::exit(1);
            }
        }
    }

    if signal_database_path.clone() == "not set" {
        // Check if the environment variable is set
        match env::var("SIGNAL_DATABASE_PATH") {
            Ok(val) => {
                signal_database_path = val;
            }
            Err(e) => {
                println!("Neither the --database_path nor the environment variable SIGNAL_DATABASE_PATH were set");
                println!("Error: {}", e);
                std::process::exit(1);
            }
        }
    }

    if output_directory.clone() == "not set" {
        // Check if the environment variable is set
        match env::var("SIGNAL_OUTPUT_DIRECTORY") {
            Ok(val) => {
                output_directory = val;
            }
            Err(e) => {
                println!("Neither the --output-directory nor the environment variable SIGNAL_OUTPUT_DIRECTORY were set");
                println!("Error: {}", e);
                std::process::exit(1);
            }
        }
    }

    // Why is this just in plain text? This seems pretty simple of a defense.
    let signal_key = get_signal::get_signal_key(PathBuf::from(signal_config_path))?;

    let conversations =
        get_signal::get_signal_data_from_sqlite(PathBuf::from(signal_database_path), signal_key)
            .unwrap();

    _ = get_signal::write_conversations_to_json(
        PathBuf::from(output_directory),
        conversations.clone(),
    );

    Ok(())
}
