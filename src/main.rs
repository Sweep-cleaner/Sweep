//! `sweep` binary entry point.

fn main() -> std::process::ExitCode {
    // Console code page first: everything the CLI prints comes after this.
    sweep::platform::prepare_process();
    // The CLI handles its own error reporting and exit codes.
    sweep::cli::main()
}
