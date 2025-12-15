use clap::Parser;
use clap::ValueEnum;
use directories::BaseDirs;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    #[arg(value_enum)]
    pub game: Game,

    #[arg(
        help = "URL of build to import on startup. Needs to use custom protocol schema, e.g. `pob://pobbin/<id>`"
    )]
    pub import_url: Option<String>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Game {
    #[value(name = "poe1")]
    Poe1,
    #[value(name = "poe2")]
    Poe2,
}

impl Game {
    pub fn data_dir(&self) -> PathBuf {
        let directory_name = match self {
            Game::Poe1 => "RustyPathOfBuilding1",
            Game::Poe2 => "RustyPathOfBuilding2",
        };
        BaseDirs::new().unwrap().data_dir().join(directory_name)
    }

    pub fn script_dir(&self) -> PathBuf {
        self.data_dir()
    }
}
