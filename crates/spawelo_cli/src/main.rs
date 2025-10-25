use std::error::Error;
use std::io::Write as _;

use clap::Parser;
use clio::{Input, Output};
use eloelo_model::history::HistoryEntry;
use serde::Deserialize;
use serde_yaml;
use spawelo;

/// CLI for calculating ELO
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// File containing matches data (history)
    #[clap(long, short, value_parser, default_value = "-")]
    input: Input,

    /// File containing options
    #[clap(long, value_parser)]
    options_file: Option<Input>,

    /// File to write output
    #[clap(long, short, value_parser, default_value = "-")]
    output: Output,
}

#[derive(Deserialize)]
struct HistoryStorage {
    entries: Vec<HistoryEntry>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = Args::parse();
    let options = match args.options_file {
        Some(f) => serde_yaml::from_reader(f)?,
        None => Default::default(),
    };
    let history: HistoryStorage = serde_yaml::from_reader(args.input)?;

    let mut elo = spawelo::ml_elo(&history.entries, &options)
        .into_iter()
        .map(|v| (v.0, v.1 as i64))
        .collect::<Vec<_>>();
    elo.sort_by_key(|(_, v)| -*v);

    if elo.is_empty() {
        return Ok(());
    }

    let name_col_width = elo.iter().map(|v| v.0.as_str().len()).max().unwrap();
    for (player, rank) in elo {
        writeln!(args.output, "{player:>name_col_width$} {rank}")?;
    }
    Ok(())
}
