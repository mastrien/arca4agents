use arca::persistence::Database;
use clap::{Parser, Subcommand, Args};
use std::io::{self};

use arca::db_path_solver::get_db_path;

/// Arca's Task Manager
#[derive(Parser)]
#[command(name = "arca", version, about = "Arca's Task Manager")]
struct Cli {
    /// Command to execute
    #[command(subcommand)]
    action: Commands,
}

#[derive(Args)]
#[group(required=true, multiple=true)]
struct EditArgs {
    /// Edit task's title
    #[arg(short, long)]
    title: Option<String>,
    
    /// Edit task's daily
    #[arg(short, long)]
    daily: Option<bool>,
}

#[derive(Subcommand)]
enum Commands {
    /// List your tasks
    List {
        /// Show tasks on shelf
        #[arg(short, long)]
        shelf: bool,

        /// Show tasks on cart
        #[arg(short, long)]
        cart: bool,

        /// Show ids
        #[arg(long)]
        id: bool,
    },
    /// Register a new task
    Add {
        /// Task title
        #[arg(required=true, num_args = 1)]
        title: String,

        /// Is task daily?
        #[arg(short, long)]
        daily: bool,
    },
    /// Mark task as done
    Done {
        /// Task position
        #[arg(required=true)]
        position: u8,
    },
    /// Edit task's metadata
    Edit {
        /// Task id to edit
        #[arg(required=true)]
        id: u32,
        /// Task's fields
        #[command(flatten)]
        fields: EditArgs,
    },
    /// Delete task
    Delete {
        /// Task id
        #[arg(required=true)]
        id: u32,
    },
    /// Take task from shelf
    Take {
        /// Task position in shelf
        #[arg(required=true)]
        position_in_shelf: u8,
    },
    /// Put a task back on the shelf
    PutBack {
        /// Task position in cart
        #[arg(required=true)]
        position_in_cart: u8,
    },
    /// Checks the day and clear the cart
    Check,
    /// Check all tasks you've already done
    History,
}

fn main() -> io::Result<()> {

    let args = Cli::parse();

    let db = Database::init(get_db_path().unwrap()).unwrap();
    let conn = db.conn();

    conn.pragma_update(None, "journal_mode", "WAL").unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();

    match args.action {
        Commands::List {shelf, cart, id} => arca::list(db, Some(shelf), Some(cart), Some(id)),
        Commands::Add {title, daily} => arca::add(db, title, daily),
        Commands::Done {position} => arca::done(db, position),
        Commands::Edit {id, fields} => arca::edit(db, id, fields.title.as_deref(), fields.daily),
        Commands::Delete {id} => arca::delete(db, id),
        Commands::Take {position_in_shelf} => arca::take(db, position_in_shelf),
        Commands::PutBack {position_in_cart} => arca::putback(db, position_in_cart),
        Commands::Check => arca::check(db),
        Commands::History => arca::history(db),
        // _ => println!("Are you sure this command exists?")
    }

    Ok(())
}
