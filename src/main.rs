use std::path::PathBuf;

fn main() {
    verinox::run(&PathBuf::from("assets/default_config.windows.toml")).unwrap();
}
