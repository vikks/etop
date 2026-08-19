use rayon::prelude::*;
use crate::models::InventoryItem;

pub mod brew;
pub mod caches;
pub mod cargo;
pub mod gem;
pub mod go;
pub mod js;
pub mod macos_apps;
pub mod mise;
pub mod python;
pub mod trait_def;

pub use brew::BrewCollector;
pub use caches::CacheCollector;
pub use cargo::CargoCollector;
pub use gem::GemCollector;
pub use go::GoCollector;
pub use js::JsCollector;
pub use macos_apps::MacAppsCollector;
pub use mise::MiseCollector;
pub use python::PythonCollector;
pub use trait_def::Collector;

/// Returns all available polyglot collectors configured for this system
pub fn all_collectors() -> Vec<Box<dyn Collector>> {
    vec![
        Box::new(BrewCollector::new()),
        Box::new(MiseCollector::new()),
        Box::new(CargoCollector::new()),
        Box::new(GemCollector::new()),
        Box::new(JsCollector::new()),
        Box::new(PythonCollector::new()),
        Box::new(GoCollector::new()),
        Box::new(MacAppsCollector::new()),
        Box::new(CacheCollector::new()),
    ]
}

/// Runs all available collectors concurrently using Rayon parallelism
pub fn collect_all_parallel() -> Vec<InventoryItem> {
    let collectors = all_collectors();
    collectors
        .par_iter()
        .filter(|c| c.is_available())
        .flat_map(|c| match c.collect() {
            Ok(items) => items,
            Err(e) => {
                eprintln!("⚠️  Collector [{}] error: {}", c.name(), e);
                Vec::new()
            }
        })
        .collect()
}
