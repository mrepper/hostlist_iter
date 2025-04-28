use std::io::{self, BufWriter, Write};
use std::process;

use clap::{Parser, Subcommand};

use hostlist_iter::{Hostlist, Result, collapse_hosts};

#[derive(Parser)]
#[clap(author, version)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Parse the hostlist and print it out again
    Parse {
        /// hostlists to parse
        #[clap(required = true, num_args = 1..)]
        hostlists: Vec<String>,
    },

    /// List individual hosts in each hostlist
    List {
        /// hostlists to expand
        #[clap(required = true, num_args = 1..)]
        hostlists: Vec<String>,
    },

    /// Collapse a list of individual hosts into a hostlist
    Collapse {
        /// host names to collapse
        #[clap(required = true, num_args = 1..)]
        hosts: Vec<String>,
    },

    /// Count hosts in each hostlist
    Count {
        /// hostlists to count
        #[clap(required = true, num_args = 1..)]
        hostlists: Vec<String>,
    },
}

fn main_real() -> Result<()> {
    let stdout = io::stdout();
    let mut stdout = BufWriter::new(stdout.lock());

    // Closure that makes printing-with-EPIPE-handling more succinct.
    let mut write_line = |line: &str| {
        if let Err(e) = writeln!(stdout, "{line}") {
            if e.kind() == io::ErrorKind::BrokenPipe {
                process::exit(0);
            }
            eprintln!("Error writing to stdout: {e}");
            process::exit(1);
        }
    };

    let cli = Cli::parse();

    // Match on the subcommand
    match cli.command {
        Commands::Parse { hostlists } => {
            for h in hostlists {
                let hostlist = Hostlist::new(&h)?;
                write_line(&hostlist.to_string());
            }
        }
        Commands::List { hostlists } => {
            for h in hostlists {
                let hostlist = Hostlist::new(&h)?;
                for host in hostlist {
                    write_line(&host);
                }
            }
        }
        Commands::Collapse { hosts } => {
            let hostlist = collapse_hosts(hosts)?;
            write_line(&hostlist);
        }
        Commands::Count { hostlists } => {
            for h in hostlists {
                let hostlist = Hostlist::new(&h)?;
                write_line(&format!("{}", hostlist.len()));
            }
        }
    }

    if let Err(e) = stdout.flush() {
        if e.kind() == io::ErrorKind::BrokenPipe {
            process::exit(0);
        }
        eprintln!("Error flushing stdout: {e}");
        process::exit(1);
    }

    Ok(())
}

fn main() {
    // Run the real main function and print any errors using their Display trait
    if let Err(err) = main_real() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}
