use std::path::PathBuf;

fn main() {
    let _result = verinox::run(&PathBuf::from("assets/default_config.windows.toml")).unwrap();
}
