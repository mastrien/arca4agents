use arca::persistence::{Authors, Database, Policy, SessionState};
use arca::{dates, AddOptions, BoardOptions, EditOptions, ListOptions, Ping, Target};
use clap::{Args, Parser, Subcommand, ValueEnum};
use colored::Colorize;
use std::process::ExitCode;

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

    /// Due date (AAAA-MM-DD, hoje, amanhã, +3d) or "none" to clear
    #[arg(long)]
    due: Option<String>,

    /// Project name or "none" to remove from its project
    #[arg(short, long)]
    project: Option<String>,
}

/// Which task: by position (as shown by `list`) or by --id
#[derive(Args)]
struct TargetArgs {
    /// Task position
    #[arg(required_unless_present = "id", conflicts_with = "id")]
    position: Option<usize>,

    /// Task id instead of position
    #[arg(long)]
    id: Option<i64>,

    /// Count positions inside this project's list (`list --project`)
    #[arg(short, long)]
    project: Option<String>,
}

impl TargetArgs {
    fn target(&self) -> Target {
        match (self.id, self.position) {
            (Some(id), _) => Target::Id(id),
            (None, Some(position)) => Target::Position(position),
            (None, None) => unreachable!("clap requires position or --id"),
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
enum PolicyArg {
    /// Agents can only read
    None,
    /// Agents manage only the tasks they created
    Own,
    /// Agents manage every task of the project
    All,
}

impl From<PolicyArg> for Policy {
    fn from(value: PolicyArg) -> Self {
        match value {
            PolicyArg::None => Policy::None,
            PolicyArg::Own => Policy::Own,
            PolicyArg::All => Policy::All,
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
enum StateArg {
    Working,
    Waiting,
    Ended,
}

impl From<StateArg> for SessionState {
    fn from(value: StateArg) -> Self {
        match value {
            StateArg::Working => SessionState::Working,
            StateArg::Waiting => SessionState::Waiting,
            StateArg::Ended => SessionState::Ended,
        }
    }
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

        /// Only tasks of this project
        #[arg(short, long)]
        project: Option<String>,

        /// Also show tasks created by agents
        #[arg(long, conflicts_with = "only_agents")]
        agents: bool,

        /// Show only tasks created by agents
        #[arg(long)]
        only_agents: bool,
    },
    /// Register a new task
    Add {
        /// Task title
        #[arg(required=true, num_args = 1)]
        title: String,

        /// Is task daily?
        #[arg(short, long)]
        daily: bool,

        /// Put it on the shelf instead of the cart
        #[arg(short, long)]
        shelf: bool,

        /// Due date, just for reference (AAAA-MM-DD, hoje, amanhã, +3d)
        #[arg(long)]
        due: Option<String>,

        /// Project name
        #[arg(short, long)]
        project: Option<String>,
    },
    /// Mark task as done
    Done {
        #[command(flatten)]
        target: TargetArgs,
    },
    /// Edit task's metadata
    Edit {
        /// Task id to edit
        #[arg(required=true)]
        id: i64,
        /// Task's fields
        #[command(flatten)]
        fields: EditArgs,
    },
    /// Delete task
    Delete {
        /// Task id
        #[arg(required=true)]
        id: i64,
    },
    /// Take task from shelf
    Take {
        #[command(flatten)]
        target: TargetArgs,
    },
    /// Put a task back on the shelf
    PutBack {
        #[command(flatten)]
        target: TargetArgs,
    },
    /// (Legacy) Archive done tasks now; the day rollover is automatic
    Check,
    /// Check all tasks you've already done
    History,
    /// Manage projects and their linked folders
    Project {
        #[command(subcommand)]
        action: ProjectCommands,
    },
    /// Record agent activity (meant for hooks; reads the hook JSON from stdin, never prints)
    Ping {
        /// Session state; if omitted, derived from the hook's `hook_event_name`
        #[arg(long, value_enum)]
        state: Option<StateArg>,

        /// Agent name (default: $ARCA_AGENT)
        #[arg(long)]
        agent: Option<String>,

        /// Session id (default: `session_id` from stdin)
        #[arg(long)]
        session: Option<String>,

        /// Working directory (default: `cwd` from stdin, then the current one)
        #[arg(long)]
        cwd: Option<String>,

        /// Link the session to the task it is working on
        #[arg(long)]
        task: Option<i64>,
    },
    /// List changes proposed by agents
    Suggestions {
        /// Include approved and rejected ones
        #[arg(long)]
        all: bool,
    },
    /// Apply a change proposed by an agent
    Approve { id: i64 },
    /// Discard a change proposed by an agent
    Reject { id: i64 },
    /// Board view: Shelf | Cart | Done today, plus agent activity
    Board {
        /// Only tasks of this project
        #[arg(short, long)]
        project: Option<String>,

        /// Also show tasks created by agents
        #[arg(long, conflicts_with = "only_agents")]
        agents: bool,

        /// Show only tasks created by agents
        #[arg(long)]
        only_agents: bool,

        /// Terminal width (default: detected)
        #[arg(long)]
        width: Option<usize>,

        /// Minutes without a signal before "working" becomes "possibly stopped"
        #[arg(long)]
        window: Option<i64>,
    },
    /// Run the MCP server for AI agents (stdio)
    Mcp {
        /// Agent name used as `created_by` (default: $ARCA_AGENT)
        #[arg(long)]
        agent: Option<String>,
    },
    /// Show agent sessions per project
    Agents {
        /// Include sessions that ended more than 24h ago
        #[arg(long)]
        all: bool,

        /// Minutes without a signal before "working" becomes "possibly stopped"
        #[arg(long)]
        window: Option<i64>,
    },
    /// Read or change settings
    Config {
        #[command(subcommand)]
        action: ConfigCommands,
    },
    /// Run the sync server for the home network (shows the pairing QR code)
    Serve {
        /// Port to listen on
        #[arg(long, default_value_t = arca_core::net::DEFAULT_PORT)]
        port: u16,

        /// Address to listen on
        #[arg(long, default_value = "0.0.0.0")]
        bind: String,

        /// Don't print the QR code
        #[arg(long)]
        no_qr: bool,
    },
    /// Sync with paired servers
    Sync {
        #[command(subcommand)]
        action: SyncCommands,
    },
    /// Agents' usage quota: show the latest readings or collect new ones
    Usage {
        #[command(subcommand)]
        action: Option<UsageCommands>,
    },
}

#[derive(Subcommand)]
enum UsageCommands {
    /// Claude Code statusline: records `rate_limits` from the stdin JSON and prints the line
    Statusline,
    /// Ask Codex for its limits (runs `codex app-server`) and record them
    Codex,
}

#[derive(Subcommand)]
enum SyncCommands {
    /// Pair with a server using its pairing code (the QR code JSON, or @file)
    Pair { code: String },
    /// Sync now with every paired server
    Now,
    /// List paired servers
    Peers,
    /// Forget a paired server (by name or device id)
    Unpair { who: String },
    /// Change this server's token (every paired device must pair again)
    RotateToken,
}

#[derive(Subcommand)]
enum ProjectCommands {
    /// Create a project
    Add {
        name: String,

        /// What agents may do with the project's tasks
        #[arg(long, value_enum, default_value_t = PolicyArg::Own)]
        policy: PolicyArg,
    },
    /// List projects with folders and task counts
    List,
    /// Rename a project
    Rename { name: String, new_name: String },
    /// Delete a project (its tasks stay, without project)
    Delete { name: String },
    /// Show or change the "active now" window of a project, in minutes ("none" = global)
    Window { name: String, minutes: Option<String> },
    /// Change the agent policy of a project
    Policy {
        name: String,
        #[arg(value_enum)]
        policy: PolicyArg,
    },
    /// Link a folder to a project (default: current folder)
    Link { name: String, folder: Option<String> },
    /// Unlink a folder (default: current folder)
    Unlink { folder: Option<String> },
    /// Show which project a folder belongs to (default: current folder)
    Which { folder: Option<String> },
}

#[derive(Subcommand)]
enum ConfigCommands {
    /// Minutes an agent counts as "working" after its last signal (default 10)
    ActiveWindow { minutes: Option<i64> },
}

fn main() -> ExitCode {
    let args = Cli::parse();

    if let Commands::Ping { state, agent, session, cwd, task } = &args.action {
        // Roda dentro de hooks: nunca pode falhar o hook nem escrever no stdout.
        let options = Ping {
            state: state.map(Into::into),
            agent: agent.as_deref(),
            session: session.as_deref(),
            cwd: cwd.as_deref(),
            task: *task,
        };
        silent_ping(options);
        return ExitCode::SUCCESS;
    }

    if let Commands::Usage { action: Some(UsageCommands::Statusline) } = &args.action {
        // Roda o tempo todo dentro do Claude Code: nunca falha, só imprime a linha.
        println!("{}", silent_statusline());
        return ExitCode::SUCCESS;
    }

    match run(args.action) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{} {:#}", "error:".red().bold(), error);
            ExitCode::FAILURE
        }
    }
}

/// `--agents` / `--only-agents` → quais autores mostrar.
fn authors(agents: bool, only_agents: bool) -> Authors {
    if only_agents { Authors::Agents } else if agents { Authors::All } else { Authors::User }
}

fn open_database() -> anyhow::Result<Database> {
    let db = Database::init(get_db_path()?)?;
    db.rollover_if_needed(dates::today())?;
    Ok(db)
}

fn run(action: Commands) -> anyhow::Result<()> {
    let db = open_database()?;

    match action {
        Commands::List {shelf, cart, id, project, agents, only_agents} => {
            arca::list(&db, ListOptions { shelf, cart, id, project: project.as_deref(), authors: authors(agents, only_agents) })
        }
        Commands::Add {title, daily, shelf, due, project} => arca::add(&db, AddOptions {
            title: &title, daily, shelf, due: due.as_deref(), project: project.as_deref(),
        }),
        Commands::Done {target} => arca::done(&db, target.target(), target.project.as_deref()),
        Commands::Edit {id, fields} => arca::edit(&db, id, EditOptions {
            title: fields.title.as_deref(),
            daily: fields.daily,
            due: fields.due.as_deref(),
            project: fields.project.as_deref(),
        }),
        Commands::Delete {id} => arca::delete(&db, id),
        Commands::Take {target} => arca::take(&db, target.target(), target.project.as_deref()),
        Commands::PutBack {target} => arca::putback(&db, target.target(), target.project.as_deref()),
        Commands::Check => arca::check(&db),
        Commands::History => arca::history(&db),
        Commands::Project {action} => match action {
            ProjectCommands::Add {name, policy} => arca::project_add(&db, &name, policy.into()),
            ProjectCommands::List => arca::project_list(&db),
            ProjectCommands::Rename {name, new_name} => arca::project_rename(&db, &name, &new_name),
            ProjectCommands::Delete {name} => arca::project_delete(&db, &name),
            ProjectCommands::Window {name, minutes} => arca::project_window(&db, &name, minutes.as_deref()),
            ProjectCommands::Policy {name, policy} => arca::project_policy(&db, &name, policy.into()),
            ProjectCommands::Link {name, folder} => arca::project_link(&db, &name, folder.as_deref()),
            ProjectCommands::Unlink {folder} => arca::project_unlink(&db, folder.as_deref()),
            ProjectCommands::Which {folder} => arca::project_which(&db, folder.as_deref()),
        },
        Commands::Board {project, agents, only_agents, width, window} => arca::board(&db, BoardOptions {
            project: project.as_deref(), authors: authors(agents, only_agents), width, window,
        }),
        Commands::Mcp {agent} => {
            let agent = arca::agent_name(agent.as_deref()).unwrap_or_else(|| "unknown".to_string());
            arca::mcp::serve(db, agent)
        }
        Commands::Suggestions {all} => arca::list_suggestions(&db, all),
        Commands::Approve {id} => arca::approve(&db, id),
        Commands::Reject {id} => arca::reject(&db, id),
        Commands::Agents {all, window} => arca::agents(&db, all, window),
        Commands::Config {action} => match action {
            ConfigCommands::ActiveWindow {minutes} => arca::config_active_window(&db, minutes),
        },
        Commands::Serve {port, bind, no_qr} => arca::serve(db, &bind, port, !no_qr),
        Commands::Sync {action} => match action {
            SyncCommands::Pair {code} => arca::sync_pair(&db, &code),
            SyncCommands::Now => arca::sync_now(&db),
            SyncCommands::Peers => arca::sync_peers(&db),
            SyncCommands::Unpair {who} => arca::sync_unpair(&db, &who),
            SyncCommands::RotateToken => arca::sync_rotate_token(&db),
        },
        Commands::Usage {action} => match action {
            None => arca::usage_show(&db),
            Some(UsageCommands::Codex) => arca::usage_codex(&db),
            Some(UsageCommands::Statusline) => unreachable!("handled in main"),
        },
        Commands::Ping {..} => unreachable!("handled in main"),
    }
}

/// Grava a cota da statusline engolindo erros e panics (vão para `ping-errors.log`).
fn silent_statusline() -> String {
    use std::io::Read;
    std::panic::set_hook(Box::new(|_| {}));
    let result = std::panic::catch_unwind(|| -> anyhow::Result<String> {
        let mut raw = String::new();
        std::io::stdin().read_to_string(&mut raw)?;
        let input: serde_json::Value = serde_json::from_str(&raw)?;
        let db = Database::init(get_db_path()?)?;
        arca::usage_statusline(&db, &input)
    });
    match result {
        Ok(Ok(line)) => line,
        Ok(Err(error)) => {
            log_ping_error(&format!("statusline: {error:#}"));
            String::new()
        }
        Err(_) => {
            log_ping_error("statusline: panic");
            String::new()
        }
    }
}

/// Executa o ping engolindo erros e panics; falhas vão para `ping-errors.log` ao lado do banco.
fn silent_ping(options: Ping) {
    std::panic::set_hook(Box::new(|_| {}));
    let result = std::panic::catch_unwind(|| -> anyhow::Result<()> {
        let db = Database::init(get_db_path()?)?;
        // A virada do dia não pode impedir o registro da sessão.
        let _ = db.rollover_if_needed(dates::today());
        arca::ping(&db, options)
    });

    let message = match result {
        Ok(Ok(())) => return,
        Ok(Err(error)) => format!("{error:#}"),
        Err(_) => "panic".to_string(),
    };
    log_ping_error(&message);
}

fn log_ping_error(message: &str) {
    use std::io::Write;
    let Ok(db_path) = get_db_path() else { return };
    let log_path = db_path.with_file_name("ping-errors.log");
    // Mantém o arquivo pequeno: recomeça quando passa de 64 KiB.
    let too_big = std::fs::metadata(&log_path).is_ok_and(|m| m.len() > 64 * 1024);
    let file = std::fs::OpenOptions::new().create(true).append(!too_big).write(true).truncate(too_big).open(&log_path);
    if let Ok(mut file) = file {
        let _ = writeln!(file, "{} {}", dates::now().format(dates::DATETIME_FORMAT), message);
    }
}
