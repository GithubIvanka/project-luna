fn main() {
    if let Err(error) = luna_user_session::handoff_current_identity() {
        eprintln!("luna-user-session: authentication handoff failed: {error}");
        std::process::exit(1);
    }
}
