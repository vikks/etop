use std::path::Path;
use clap::{Parser, Subcommand};
use colored::Colorize;
use anyhow::Result;

mod models;
mod collectors;
mod engine;
mod output;
mod cleanup;
mod tui;

use collectors::collect_all_parallel;
use engine::{DependencyGraph, FsAuditor, TaxonomyEngine};
use models::{format_bytes, Category, InventoryItem, PackageStatus};
use output::render_inventory_table;
use cleanup::{HistoryStore, ScriptGenerator};

#[derive(Parser, Debug)]
#[command(name = "etop")]
#[command(about = "Deterministic macOS Developer Ecosystem & Package Top", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Output raw JSON instead of tables
    #[arg(long, global = true)]
    json: bool,

    /// Sort output: size, name, last-used, source, category
    #[arg(long, default_value = "size", global = true)]
    sort: String,

    /// Limit number of items displayed in table (0 for all)
    #[arg(long, short = 'n', default_value_t = 0, global = true)]
    top: usize,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Launch interactive fullscreen TUI package dashboard (Default)
    Tui,
    /// Scan all package managers, audit real disk usage, and print complete inventory
    Scan,
    /// Show detailed package inspection (configs, data dirs, logs, caches, and environment variables)
    Info {
        /// Package name, ID, or substring query to inspect
        package: String,
    },
    /// Filter software inventory by objective toolchain, topology, or metadata criteria
    Filter {
        /// Filter by programming ecosystem/toolchain (e.g. ruby, rust, js, python, go, brew, mise, apps, cache, docker)
        #[arg(long, short = 'e')]
        ecosystem: Option<String>,

        /// Show only unreferenced orphan dependencies
        #[arg(long)]
        orphans: bool,

        /// Show only build artifact caches and dangling container layers
        #[arg(long)]
        caches: bool,

        /// Show only top-level explicitly installed packages
        #[arg(long)]
        top_level: bool,

        /// Filter packages inactive for more than N days
        #[arg(long)]
        older_than: Option<u64>,

        /// Filter by Category (e.g. 'cli', 'database', 'runtime', 'gui', 'cache')
        #[arg(long)]
        category: Option<String>,
    },
    /// Print a high-level summary breakdown of ecosystems, orphans, and caches
    Summary,
    /// Print disk usage grouped by categorized domains
    Categories,
    /// View uninstalled package history, forensic tombstones, and preserved configurations
    History {
        /// Optional package ID or name to inspect detailed forensic metadata
        #[arg(long, short = 'i')]
        inspect: Option<String>,
    },
    /// Generate deterministic dry-run cleanup and rollback shell scripts
    Prune {
        /// Prune unreferenced orphan dependencies
        #[arg(long)]
        orphans: bool,

        /// Prune build caches and dangling Docker layers
        #[arg(long)]
        caches: bool,

        /// Filter pruning target by ecosystem (e.g. ruby, rust, brew, js)
        #[arg(long, short = 'e')]
        ecosystem: Option<String>,

        /// Prune items inactive for more than N days
        #[arg(long)]
        older_than: Option<u64>,

        /// Custom output directory for generated shell scripts (default: current dir)
        #[arg(long, default_value = ".")]
        out: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let command = cli.command.unwrap_or(Commands::Tui);

    // If TUI is launched, immediately paint screen with zero cold-start latency!
    if let Commands::Tui = command {
        return tui::run_tui();
    }

    // If History command is selected, read directly from persistent store
    if let Commands::History { inspect } = &command {
        let store = HistoryStore::new();
        if let Some(query) = inspect {
            if let Some(tombstone) = store.find_tombstone(query)? {
                println!("\n{}", "🏛️  Forensic Package Tombstone Snapshot".bold().cyan());
                println!("===============================================================");
                println!("Package:              {} ({})", tombstone.name.bold(), tombstone.version.as_deref().unwrap_or("unknown").yellow());
                println!("ID:                   {}", tombstone.id.dimmed());
                println!("Source:               {} {}", tombstone.source.icon(), tombstone.source.label());
                println!("Category:             {}", tombstone.category);
                println!("Removed At:           {}", tombstone.removed_at.format("%Y-%m-%d %H:%M:%S UTC").to_string().cyan());
                println!("Original Path:        {}", tombstone.install_path.display());
                println!("Reclaimed Size:       {}", format_bytes(tombstone.size_bytes).green().bold());

                if !tombstone.associated_metadata.configs.is_empty() {
                    println!("\n{}", "⚙️  Preserved Configuration Files:".yellow().bold());
                    for p in &tombstone.associated_metadata.configs {
                        println!("   • {}", p.display());
                    }
                }

                if !tombstone.associated_metadata.data_dirs.is_empty() {
                    println!("\n{}", "💾 Preserved Data Directories:".cyan().bold());
                    for p in &tombstone.associated_metadata.data_dirs {
                        println!("   • {}", p.display());
                    }
                }

                if !tombstone.associated_metadata.logs.is_empty() {
                    println!("\n{}", "📜 Associated Log File Paths:".magenta().bold());
                    for p in &tombstone.associated_metadata.logs {
                        println!("   • {}", p.display());
                    }
                }

                if !tombstone.associated_metadata.env_vars.is_empty() {
                    println!("\n{}", "🌐 Associated Environment Variables:".cyan().bold());
                    for (k, v) in &tombstone.associated_metadata.env_vars {
                        println!("   • {}={}", k.green().bold(), v);
                    }
                }

                if let Some(reinstall) = &tombstone.reinstall_cmd {
                    println!("\n{}", "🔄 Inverse Reinstall Command:".green().bold());
                    println!("   {}", reinstall);
                }
                println!("===============================================================\n");
            } else {
                println!("\n❌ No archived tombstone found matching \"{}\". Check 'etop history' for all records.\n", query);
            }
            return Ok(());
        }

        let all_tombstones = store.list_all_tombstones()?;
        if all_tombstones.is_empty() {
            println!("\n🏛️  No archived package tombstones found yet. When you generate cleanup scripts, full forensic snapshots will be archived here.\n");
            return Ok(());
        }

        println!("\n{}", "🏛️  Archived Package Tombstones & Removal History".bold().cyan());
        println!("=========================================================================================");
        println!("{:<20} {:<14} {:<24} {:<10} {:<12}", "Removed At", "Source", "Package Name", "Reclaimed", "Configs Kept");
        println!("-----------------------------------------------------------------------------------------");
        for t in &all_tombstones {
            println!(
                "{:<20} {:<14} {:<24} {:<10} {:<12}",
                t.removed_at.format("%Y-%m-%d %H:%M").to_string().cyan(),
                format!("{} {}", t.source.icon(), t.source.label()),
                t.name.bold(),
                format_bytes(t.size_bytes).green(),
                format!("{} files", t.associated_metadata.configs.len()).yellow()
            );
        }
        println!("=========================================================================================");
        println!("💡 Run 'etop history --inspect <name>' to view configs, logs, and env vars for any removed tool.\n");
        return Ok(());
    }

    // Otherwise, perform CLI pipeline for non-TUI subcommands
    // 1. Gather all active collectors concurrently in parallel
    let mut all_items = collect_all_parallel();

    // 2. Parallel Filesystem & Metadata Audit (Sizing & Timestamps)
    FsAuditor::audit_all(&mut all_items);

    // 3. Deterministic Taxonomy Categorization
    let taxonomy = TaxonomyEngine::load_or_default();
    for item in all_items.iter_mut() {
        item.category = taxonomy.classify(&item.name, &item.id);
    }

    // 4. Dependency Graph & Orphan Resolution
    let _dep_graph = DependencyGraph::resolve(&mut all_items);

    // 5. Handle specific subcommands
    match command {
        Commands::Tui | Commands::History { .. } => unreachable!(),
        Commands::Scan => {
            let mut items = all_items;
            sort_items(&mut items, &cli.sort);
            print_items_table(&items, cli.json, cli.top);
        }
        Commands::Info { package } => {
            let q = package.to_lowercase();
            let matches: Vec<&InventoryItem> = all_items
                .iter()
                .filter(|item| {
                    let name = item.name.to_lowercase();
                    let id = item.id.to_lowercase();
                    name == q
                        || id == q
                        || name.contains(&q)
                        || id.contains(&q)
                        || (q == "postgres" && (name.contains("postgresql") || id.contains("postgresql")))
                        || (q == "postgresql" && (name.contains("postgres") || id.contains("postgres")))
                        || (q == "golang" && (name.contains("go") || id.contains("go:")))
                        || (q == "nodejs" && (name.contains("node") || id.contains("npm:")))
                        || (q == "python" && (name.contains("py") || id.contains("uv:")))
                })
                .collect();

            if matches.is_empty() {
                println!("\n❌ No installed package found matching \"{}\". Check 'etop scan' to see all installed packages.\n", package);
                return Ok(());
            }

            if cli.json {
                #[derive(serde::Serialize)]
                struct PackageInfoOutput<'a> {
                    item: &'a InventoryItem,
                    associated_metadata: crate::models::AssociatedMetadata,
                }

                let outputs: Vec<_> = matches
                    .iter()
                    .map(|item| PackageInfoOutput {
                        item,
                        associated_metadata: item.discover_associated_metadata(),
                    })
                    .collect();

                println!("{}", serde_json::to_string_pretty(&outputs)?);
                return Ok(());
            }

            if matches.len() > 1 {
                println!(
                    "\n🔍 {} {} {} {}",
                    "Found".bold(),
                    matches.len().to_string().cyan().bold(),
                    "distinct installations matching".bold(),
                    format!("\"{}\":", package).yellow().bold()
                );
            }

            for item in matches {
                print_package_info(item);
            }
        }
        Commands::Filter {
            ecosystem,
            orphans,
            caches,
            top_level,
            older_than,
            category,
        } => {
            let mut filtered: Vec<InventoryItem> = all_items
                .into_iter()
                .filter(|item| {
                    if let Some(eco) = &ecosystem {
                        if !item.matches_ecosystem(eco) {
                            return false;
                        }
                    }
                    if orphans && !item.is_orphan() {
                        return false;
                    }
                    if caches && !item.is_cache() {
                        return false;
                    }
                    if top_level && !item.is_top_level {
                        return false;
                    }
                    if let Some(days) = older_than {
                        if item.inactivity_days().unwrap_or(0) < days as i64 {
                            return false;
                        }
                    }
                    if let Some(cat) = &category {
                        let cat_str = item.category.as_str().to_lowercase();
                        if !cat_str.contains(&cat.to_lowercase()) {
                            return false;
                        }
                    }
                    true
                })
                .collect();

            sort_items(&mut filtered, &cli.sort);
            print_items_table(&filtered, cli.json, cli.top);
        }
        Commands::Summary => {
            let total_count = all_items.len();
            let total_bytes: u64 = all_items.iter().map(|i| i.size_bytes).sum();

            let orphan_items: Vec<_> = all_items.iter().filter(|i| i.is_orphan()).collect();
            let orphan_bytes: u64 = orphan_items.iter().map(|i| i.size_bytes).sum();

            let cache_items: Vec<_> = all_items.iter().filter(|i| i.is_cache()).collect();
            let cache_bytes: u64 = cache_items.iter().map(|i| i.size_bytes).sum();

            let top_level_count = all_items.iter().filter(|i| i.is_top_level && !i.is_cache()).count();
            let dep_count = all_items.iter().filter(|i| matches!(i.status(), PackageStatus::Dependency { .. })).count();

            println!("\n{}", "📊 macOS Software Inventory Summary".bold().cyan());
            println!("==================================================");
            println!("Total Audited Items:      {}", total_count.to_string().bold());
            println!("📦 Top-Level Tools:        {}", top_level_count.to_string().cyan().bold());
            println!("🧹 Orphan Dependencies:   {} ({})", orphan_items.len().to_string().magenta().bold(), format_bytes(orphan_bytes).magenta().bold());
            println!("💾 Build Caches & Docker:  {} ({})", cache_items.len().to_string().yellow().bold(), format_bytes(cache_bytes).yellow().bold());
            println!("⛓️  Active Dependencies:    {}", dep_count.to_string().blue());
            println!("💾 Total Disk Usage:       {}", format_bytes(total_bytes).green().bold());
            println!("==================================================\n");

            // Ecosystem breakdown
            let mut eco_counts: std::collections::HashMap<&'static str, (usize, u64)> = std::collections::HashMap::new();
            for item in &all_items {
                let label = item.source.label();
                let entry = eco_counts.entry(label).or_insert((0, 0));
                entry.0 += 1;
                entry.1 += item.size_bytes;
            }

            let mut eco_list: Vec<(&'static str, usize, u64)> = eco_counts.into_iter().map(|(eco, (cnt, sz))| (eco, cnt, sz)).collect();
            eco_list.sort_by(|a, b| b.2.cmp(&a.2));

            println!("{}", "📦 Disk Usage by Ecosystem / Toolchain".bold().cyan());
            println!("--------------------------------------------------");
            for (label, count, size) in eco_list {
                println!("{:<24} {:>4} items   {:>10}", label, count.to_string().bold(), format_bytes(size).green().bold());
            }
            println!("--------------------------------------------------\n");
        }
        Commands::Categories => {
            let total_bytes: u64 = all_items.iter().map(|i| i.size_bytes).sum();
            let mut cat_map: std::collections::HashMap<Category, (usize, u64)> = std::collections::HashMap::new();
            for item in &all_items {
                let entry = cat_map.entry(item.category.clone()).or_insert((0, 0));
                entry.0 += 1;
                entry.1 += item.size_bytes;
            }

            let mut cat_list: Vec<(Category, usize, u64)> = cat_map.into_iter().map(|(c, (cnt, sz))| (c, cnt, sz)).collect();
            cat_list.sort_by(|a, b| b.2.cmp(&a.2));

            println!("\n{}", "🗂️  Software Inventory by Category".bold().cyan());
            println!("===============================================================");
            for (cat, count, size) in cat_list {
                println!(
                    "{:<32} {:>5} items   {:>10}",
                    format!("{}", cat),
                    count.to_string().bold(),
                    format_bytes(size).green().bold()
                );
            }
            println!("===============================================================");
            println!("Total: {:>36} items   {:>10}\n", all_items.len().to_string().bold(), format_bytes(total_bytes).green().bold());
        }
        Commands::Prune {
            orphans,
            caches,
            ecosystem,
            older_than,
            out,
        } => {
            let prune_items: Vec<InventoryItem> = all_items
                .into_iter()
                .filter(|item| {
                    if item.uninstall_cmd.is_none() {
                        return false;
                    }
                    if let Some(eco) = &ecosystem {
                        if !item.matches_ecosystem(eco) {
                            return false;
                        }
                    }
                    if let Some(days) = older_than {
                        if item.inactivity_days().unwrap_or(0) < days as i64 {
                            return false;
                        }
                    }

                    // If explicit flags given, match them
                    if orphans && item.is_orphan() {
                        return true;
                    }
                    if caches && item.is_cache() {
                        return true;
                    }

                    // Default prune targets: both orphans and caches if no specific flag given
                    if !orphans && !caches && older_than.is_none() && ecosystem.is_none() {
                        return item.is_orphan() || item.is_cache();
                    }

                    false
                })
                .collect();

            if prune_items.is_empty() {
                println!("\n✨ {}", "No items matching criteria for pruning. Your system is lean!".green().bold());
                return Ok(());
            }

            let prune_bytes: u64 = prune_items.iter().map(|i| i.size_bytes).sum();
            let output_dir = Path::new(&out);

            let (cleanup_file, rollback_file, manifest_file) = ScriptGenerator::generate_and_save(&prune_items, output_dir)?;

            println!("\n{}", "🧹 Deterministic Cleanup Plan Generated (--dry-run)".bold().green());
            println!("===============================================================");
            println!("Candidate packages to prune: {}", prune_items.len().to_string().cyan().bold());
            println!("Estimated disk reclaim:      {}", format_bytes(prune_bytes).green().bold());
            println!("---------------------------------------------------------------");
            println!("📝 Runnable Cleanup Script:  {}", cleanup_file.display().to_string().bold());
            println!("🔄 Reversible Rollback:      {}", rollback_file.display().to_string().bold());
            println!("🏛️  Forensic Tombstones:     {}", manifest_file.display().to_string().yellow().bold());
            println!("===============================================================");
            println!("\n👉 To review and execute the cleanup plan:");
            println!("   bash {}", cleanup_file.display());
            println!("\n👉 To restore/reinstall any pruned packages later:");
            println!("   bash {}\n", rollback_file.display());
        }
    }

    Ok(())
}

fn print_package_info(item: &InventoryItem) {
    let status_str = match item.status() {
        PackageStatus::Orphan => "🧹 Orphan Dependency (0 references)".magenta().bold(),
        PackageStatus::TopLevel => "📦 Top-Level Explicit Install".cyan().bold(),
        PackageStatus::Dependency { required_by_count } => {
            format!("⛓️ Shared Dependency (Required by {} packages)", required_by_count).red().bold()
        }
        PackageStatus::BuildCache => "💾 Build Artifact Cache / Dangling Layer".yellow().bold(),
        PackageStatus::ActiveRuntime => "⚡ Active Toolchain Language Runtime".green().bold(),
    };

    println!("\n{}", "📦 Package Detail Inspector".bold().cyan());
    println!("===============================================================");
    println!("Package:              {} ({})", item.name.bold(), item.version.as_deref().unwrap_or("unknown").yellow());
    println!("ID:                   {}", item.id.dimmed());
    println!("Source:               {} {}", item.source.icon(), item.source.label());
    println!("Category:             {}", item.category);
    println!("Status:               {}", status_str);
    println!("Real Disk Size:       {}", item.formatted_size().green().bold());
    println!("Install Path:         {}", item.install_path.display());

    if let Some(days) = item.inactivity_days() {
        println!("Last Accessed:        {} ({} days ago)", item.formatted_last_accessed().bold(), days);
    }

    if !item.required_by.is_empty() {
        println!("Required By:          {}", item.required_by.join(", ").red().bold());
    }

    let meta = item.discover_associated_metadata();

    if !meta.configs.is_empty() {
        println!("\n{}", format!("⚙️  Preserved Configs ({} found):", meta.configs.len()).yellow().bold());
        for p in &meta.configs {
            println!("   • {}", p.display());
        }
    }

    if !meta.data_dirs.is_empty() {
        println!("\n{}", format!("💾 Data & State Dirs ({} found):", meta.data_dirs.len()).cyan().bold());
        for p in &meta.data_dirs {
            println!("   • {}", p.display());
        }
    }

    if !meta.logs.is_empty() {
        println!("\n{}", format!("📜 Associated Log Files ({} found):", meta.logs.len()).magenta().bold());
        for p in &meta.logs {
            println!("   • {}", p.display());
        }
    }

    if !meta.caches.is_empty() {
        println!("\n{}", format!("⚡ Associated Caches ({} found):", meta.caches.len()).yellow().bold());
        for p in &meta.caches {
            println!("   • {}", p.display());
        }
    }

    if !meta.env_vars.is_empty() {
        println!("\n{}", format!("🌐 Associated Environment Variables ({} active):", meta.env_vars.len()).cyan().bold());
        for (k, v) in &meta.env_vars {
            println!("   • {}={}", k.green().bold(), v);
        }
    }

    if let Some(uninstall) = &item.uninstall_cmd {
        println!("\n{}", "✂️  Uninstall Command:".yellow().bold());
        println!("   {}", uninstall);
    }

    if let Some(reinstall) = &item.reinstall_cmd {
        println!("\n{}", "🔄 Reinstall Command:".green().bold());
        println!("   {}", reinstall);
    }

    println!("===============================================================\n");
}

fn sort_items(items: &mut [InventoryItem], sort_criteria: &str) {
    match sort_criteria.to_lowercase().as_str() {
        "name" => items.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase())),
        "source" => items.sort_by(|a, b| a.source.cmp(&b.source)),
        "category" => items.sort_by(|a, b| a.category.cmp(&b.category)),
        "last-used" | "inactivity" => items.sort_by(|a, b| {
            let days_a = a.inactivity_days().unwrap_or(0);
            let days_b = b.inactivity_days().unwrap_or(0);
            days_b.cmp(&days_a).then_with(|| b.size_bytes.cmp(&a.size_bytes))
        }),
        _ => items.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes)), // Default sort by size descending
    }
}

fn print_items_table(items: &[InventoryItem], json: bool, top: usize) {
    let display_items = if top > 0 {
        &items[..top.min(items.len())]
    } else {
        &items[..]
    };

    if json {
        if let Ok(json_str) = serde_json::to_string_pretty(display_items) {
            println!("{}", json_str);
        }
        return;
    }

    let total_bytes: u64 = items.iter().map(|i| i.size_bytes).sum();
    let orphan_bytes: u64 = items.iter().filter(|i| i.is_orphan()).map(|i| i.size_bytes).sum();

    println!("\n🔍 {}", "Discovered Software Inventory:".bold().underline());
    println!("{}", render_inventory_table(display_items));

    if top > 0 && top < items.len() {
        println!(
            "Showing top {} of {} packages | Total Disk Footprint: {} (Orphan Footprint: {})\n",
            top.to_string().cyan().bold(),
            items.len().to_string().bold(),
            format_bytes(total_bytes).green().bold(),
            format_bytes(orphan_bytes).magenta().bold()
        );
    } else {
        println!(
            "Total packages: {} | Total Disk Footprint: {} (Orphan Footprint: {})\n",
            items.len().to_string().cyan().bold(),
            format_bytes(total_bytes).green().bold(),
            format_bytes(orphan_bytes).magenta().bold()
        );
    }
}
