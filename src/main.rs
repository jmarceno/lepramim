use clap::Parser;
use lexaloud::cli::{Cli, Commands};

fn main() {
    lexaloud::init_tracing();
    let cli = Cli::parse();

    let is_app = matches!(
        cli.command,
        None | Some(Commands::App) | Some(Commands::Tray)
    );

    if is_app {
        let code = lexaloud::ui::run(cli.control, cli.overlay);
        std::process::exit(code);
    }

    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let code = rt.block_on(lexaloud::cli::run_async(cli));
    std::process::exit(code);
}
