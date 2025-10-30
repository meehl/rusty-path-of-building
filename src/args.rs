use clap::Parser;
use clap::ValueEnum;
use directories::BaseDirs;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    #[arg(value_enum)]
    pub game: Game,

    pub build_path: Option<String>,
}

#[derive(Debug, Clone, ValueEnum)]
pub enum Game {
    #[value(name = "poe1")]
    Poe1,
    #[value(name = "poe2")]
    Poe2,
}

static GAME_CONFIG: std::sync::OnceLock<Game> = std::sync::OnceLock::new();

impl Game {
    pub fn init(game: Game) {
        GAME_CONFIG.set(game).expect("Game should be uninitialized");
    }

    pub fn current() -> &'static Game {
        GAME_CONFIG.get().expect("Game should be initialized")
    }

    pub fn data_dir() -> PathBuf {
        let directory_name = match Self::current() {
            Game::Poe1 => "RustyPathOfBuilding1",
            Game::Poe2 => "RustyPathOfBuilding2",
        };
        BaseDirs::new().unwrap().data_dir().join(directory_name)
    }

    pub fn script_dir() -> PathBuf {
        Game::data_dir()
    }

    pub fn user_data_dir() -> PathBuf {
        Game::data_dir().join("userdata")
    }
}
