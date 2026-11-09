mod common;
mod repl;
mod run;

fn main() {
    // The VM is the canonical backend and therefore the REPL default;
    // `ZIRC_BACKEND=interp` selects the reference interpreter.
    let backend = run::resolve_backend(None).unwrap_or_default();
    repl::start_repl_with_backend(backend);
}
