//! `args` is used to parse the arguments passed to the program on launch.
//!
//! Normally these are the arguments passed after the `rusty-path-of-building`
//! command from a CLI.

use clap::Parser;
use clap::ValueEnum;
use directories::BaseDirs;
use std::path::PathBuf;

/// CLI arguments passed to the application on launch.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Used to determine which PoB to start. (PoE1 or PoE2)
    #[arg(value_enum)]
    pub game: Game,

    /// Specify a build to load on start using a URL. (Optional)
    #[arg(
        help = "URL of build to import on startup. Needs to use custom protocol schema, e.g. `pob://pobbin/<id>`"
    )]
    pub import_url: Option<String>,
}

/// Enum representing which game (PoE1 or PoE2) the application needs to launch.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Game {
    /// Path of Exile 1
    #[value(name = "poe1")]
    Poe1,
    /// Path of Exile 2
    #[value(name = "poe2")]
    Poe2,
}

impl Game {
    /// Returns the path to the user’s data directory based on which `Game` option
    /// was used to start the application.
    pub fn data_dir(&self) -> PathBuf {
        let directory_name = match self {
            Game::Poe1 => "RustyPathOfBuilding1",
            Game::Poe2 => "RustyPathOfBuilding2",
        };
        BaseDirs::new().unwrap().data_dir().join(directory_name)
    }

    /// Returns the path to the user's data directory. Calls [`Self::data_dir`].
    pub fn script_dir(&self) -> PathBuf {
        self.data_dir()
    }
}
