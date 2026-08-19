use serde::{Deserialize, Serialize};
use std::fmt;

/// Deterministic categorization taxonomy
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Category {
    LanguagesAndRuntimes,
    DatabasesAndStorage,
    CliDeveloperTools,
    InfrastructureAndCloud,
    BuildAndPackageManagers,
    GuiAppsAndMedia,
    BuildArtifactsAndCaches,
    SystemAndLibraries,
    Uncategorized,
}

impl Category {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::LanguagesAndRuntimes     => "Languages & Runtimes",
            Self::DatabasesAndStorage      => "Databases & Storage",
            Self::CliDeveloperTools        => "CLI Developer Tools",
            Self::InfrastructureAndCloud   => "Infrastructure & Cloud",
            Self::BuildAndPackageManagers  => "Build & Package Managers",
            Self::GuiAppsAndMedia          => "GUI Apps & Media",
            Self::BuildArtifactsAndCaches  => "Build Artifacts & Caches",
            Self::SystemAndLibraries       => "System & Libraries",
            Self::Uncategorized            => "Uncategorized",
        }
    }

    pub fn emoji(&self) -> &'static str {
        match self {
            Self::LanguagesAndRuntimes     => "⚡",
            Self::DatabasesAndStorage      => "🗄️",
            Self::CliDeveloperTools        => "🛠️",
            Self::InfrastructureAndCloud   => "☁️",
            Self::BuildAndPackageManagers  => "📦",
            Self::GuiAppsAndMedia          => "🖥️",
            Self::BuildArtifactsAndCaches  => "🧹",
            Self::SystemAndLibraries       => "⚙️",
            Self::Uncategorized            => "❓",
        }
    }
}

impl fmt::Display for Category {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.emoji(), self.as_str())
    }
}
