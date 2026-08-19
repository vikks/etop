use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, Wrap},
    Frame,
};

use crate::models::{format_bytes, Category, PackageStatus};
use super::app::{App, InputMode};

pub fn draw(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header stats
            Constraint::Min(10),   // Main table + detail inspector
            Constraint::Length(3), // Footer modal menu / search bar
        ])
        .split(f.area());

    draw_header(f, app, chunks[0]);

    if app.input_mode == InputMode::HistoryView {
        draw_history_main(f, app, chunks[1]);
    } else {
        draw_main(f, app, chunks[1]);
    }

    draw_footer(f, app, chunks[2]);

    if app.input_mode == InputMode::HelpModal {
        draw_help_modal(f, f.area());
    }
}

fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    if app.input_mode == InputMode::HistoryView {
        let spans = vec![
            Span::styled(" 🏛️ etop History & Tombstone Archive ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw(" | Archived Removals: "),
            Span::styled(app.tombstones.len().to_string(), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw(" | Press [Esc/h] to return to Live Inventory"),
        ];
        let block = Block::default().borders(Borders::ALL).style(Style::default().fg(Color::Cyan));
        f.render_widget(Paragraph::new(Line::from(spans)).block(block), area);
        return;
    }

    let total_count = app.items.len();
    let visible_count = app.filtered_indices.len();
    let marked_count = app.marked_for_prune.len();

    let marked_reclaim: u64 = app.items
        .iter()
        .filter(|i| app.marked_for_prune.contains(&i.id))
        .map(|i| i.size_bytes)
        .sum();

    let orphan_count = app.items.iter().filter(|i| i.is_orphan()).count();
    let orphan_reclaim: u64 = app.items.iter().filter(|i| i.is_orphan()).map(|i| i.size_bytes).sum();

    let mut header_spans = vec![
        Span::styled(" ⚡ etop ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
    ];

    if app.is_loading {
        header_spans.push(Span::styled(format!(" {} ", app.spinner()), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
        header_spans.push(Span::styled("Auditing macOS toolchains...", Style::default().fg(Color::Yellow)));
        header_spans.push(Span::raw(" | Items: "));
        header_spans.push(Span::styled(format!("{}/{}", visible_count, total_count), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)));
    } else {
        header_spans.push(Span::raw(" | Items: "));
        header_spans.push(Span::styled(format!("{}/{}", visible_count, total_count), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)));
        header_spans.push(Span::raw(" | Orphans: "));
        header_spans.push(Span::styled(format!("{} ({})", orphan_count, format_bytes(orphan_reclaim)), Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)));
        header_spans.push(Span::raw(" | Marked: "));
        header_spans.push(Span::styled(format!("{} ({})", marked_count, format_bytes(marked_reclaim)), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)));
        header_spans.push(Span::raw(" | Sort: "));
        header_spans.push(Span::styled(app.active_sort.label(), Style::default().fg(Color::LightBlue)));
    }

    if !app.active_ecosystems.is_empty() {
        header_spans.push(Span::raw(" | Eco: "));
        header_spans.push(Span::styled(app.ecosystems_summary(), Style::default().fg(Color::LightCyan).add_modifier(Modifier::BOLD)));
    }

    if !app.active_categories.is_empty() {
        header_spans.push(Span::raw(" | Cat: "));
        header_spans.push(Span::styled(app.categories_summary(), Style::default().fg(Color::Yellow)));
    }

    if let Some(days) = app.min_inactivity_days {
        header_spans.push(Span::raw(" | Inactive: "));
        header_spans.push(Span::styled(format!(">{}d", days), Style::default().fg(Color::LightYellow)));
    }

    if app.only_orphans {
        header_spans.push(Span::styled(" [ORPHANS]", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)));
    }

    if app.only_caches {
        header_spans.push(Span::styled(" [CACHES]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
    }

    if app.only_top_level {
        header_spans.push(Span::styled(" [TOP-LEVEL]", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)));
    }

    if !app.search_query.is_empty() {
        header_spans.push(Span::raw(" | 🔍 \""));
        header_spans.push(Span::styled(&app.search_query, Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD)));
        header_spans.push(Span::raw("\""));
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .style(Style::default().fg(Color::DarkGray));

    let paragraph = Paragraph::new(Line::from(header_spans)).block(block);
    f.render_widget(paragraph, area);
}

fn draw_main(f: &mut Frame, app: &App, area: Rect) {
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(60), // Table
            Constraint::Percentage(40), // Inspector Detail
        ])
        .split(area);

    draw_table(f, app, main_chunks[0]);
    draw_inspector(f, app, main_chunks[1]);
}

fn draw_table(f: &mut Frame, app: &App, area: Rect) {
    let header_cells = ["Mark", "Source", "Name", "Category", "Size", "Status", "Last Used"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)));
    let header = Row::new(header_cells).height(1).bottom_margin(1);

    let visible_rows = area.height.saturating_sub(4) as usize;
    let start_idx = if app.selected_index >= visible_rows {
        app.selected_index - visible_rows + 1
    } else {
        0
    };
    let end_idx = (start_idx + visible_rows).min(app.filtered_indices.len());

    let rows = (start_idx..end_idx).map(|i| {
        let item_idx = app.filtered_indices[i];
        let item = &app.items[item_idx];
        let is_selected = i == app.selected_index;
        let is_marked = app.marked_for_prune.contains(&item.id);

        let mark_str = if is_marked { "[✓]" } else { "[ ]" };
        let mark_color = if is_marked { Color::Green } else { Color::DarkGray };

        let (status_str, status_color) = match item.status() {
            PackageStatus::Orphan => ("Orphan (0 refs)".to_string(), Color::Magenta),
            PackageStatus::TopLevel => ("Top-Level".to_string(), Color::Cyan),
            PackageStatus::Dependency { required_by_count } => (format!("Dep ({} refs)", required_by_count), Color::Red),
            PackageStatus::BuildCache => ("Cache".to_string(), Color::Yellow),
            PackageStatus::ActiveRuntime => ("Active Runtime".to_string(), Color::Green),
        };

        let cells = vec![
            Cell::from(mark_str).style(Style::default().fg(mark_color).add_modifier(if is_marked { Modifier::BOLD } else { Modifier::empty() })),
            Cell::from(format!("{} {}", item.source.icon(), item.source.label())),
            Cell::from(item.name.clone()).style(Style::default().add_modifier(Modifier::BOLD)),
            Cell::from(item.category.as_str()),
            Cell::from(item.formatted_size()),
            Cell::from(status_str).style(Style::default().fg(status_color).add_modifier(Modifier::BOLD)),
            Cell::from(item.formatted_last_accessed()),
        ];

        let mut row = Row::new(cells).height(1);
        if is_selected {
            row = row.style(Style::default().bg(Color::Rgb(30, 45, 65)).fg(Color::White).add_modifier(Modifier::BOLD));
        }
        row
    });

    let table_title = if app.is_loading {
        format!(" Installed Packages & Artifacts {} Streaming live discoveries... ", app.spinner())
    } else {
        " Installed Packages & Artifacts ".to_string()
    };

    let table = Table::new(
        rows,
        [
            Constraint::Length(5),  // Mark
            Constraint::Length(14), // Source
            Constraint::Length(20), // Name
            Constraint::Length(22), // Category
            Constraint::Length(10), // Size
            Constraint::Length(16), // Status
            Constraint::Length(10), // Last Used
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(Span::styled(table_title, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)))
            .style(Style::default().fg(Color::DarkGray)),
    );

    f.render_widget(table, area);
}

fn draw_inspector(f: &mut Frame, app: &App, area: Rect) {
    let content = if app.items.is_empty() && app.is_loading {
        vec![
            Line::from(vec![
                Span::styled(format!(" {} ", app.spinner()), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::styled("Background Systems Audit in Progress", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            ]),
            Line::raw("=================================================="),
            Line::from(vec![
                Span::styled("  Current Stage: ", Style::default().fg(Color::Yellow)),
                Span::styled(app.current_stage, Style::default().fg(Color::White)),
            ]),
            Line::raw(""),
            Line::from(vec![
                Span::styled(format!("  {} ", app.spinner()), Style::default().fg(Color::Cyan)),
                Span::raw("Homebrew Formulae & Casks (brew info --json)"),
            ]),
            Line::from(vec![
                Span::styled(format!("  {} ", app.spinner()), Style::default().fg(Color::Cyan)),
                Span::raw("Mise Runtimes (Ruby, Node, Python, Go, Rust)"),
            ]),
            Line::from(vec![
                Span::styled(format!("  {} ", app.spinner()), Style::default().fg(Color::Cyan)),
                Span::raw("Cargo Binaries (~/.cargo/.crates2.json)"),
            ]),
            Line::from(vec![
                Span::styled(format!("  {} ", app.spinner()), Style::default().fg(Color::Cyan)),
                Span::raw("Ruby Gems across installed runtimes"),
            ]),
            Line::from(vec![
                Span::styled(format!("  {} ", app.spinner()), Style::default().fg(Color::Cyan)),
                Span::raw("NPM / Bun / Yarn Global Packages"),
            ]),
            Line::from(vec![
                Span::styled(format!("  {} ", app.spinner()), Style::default().fg(Color::Cyan)),
                Span::raw("Python UV & Pip Isolated Tools"),
            ]),
            Line::from(vec![
                Span::styled(format!("  {} ", app.spinner()), Style::default().fg(Color::Cyan)),
                Span::raw("macOS /Applications Bundle Info.plist"),
            ]),
            Line::from(vec![
                Span::styled(format!("  {} ", app.spinner()), Style::default().fg(Color::Cyan)),
                Span::raw("Compiler Build Caches & Docker Dangling Layers"),
            ]),
            Line::from(vec![
                Span::styled(format!("  {} ", app.spinner()), Style::default().fg(Color::Cyan)),
                Span::raw("Parallel APFS Inode Sizing & POSIX Access Times"),
            ]),
            Line::raw("=================================================="),
            Line::from(Span::styled("⚡ Controls are fully responsive during loading!", Style::default().fg(Color::Green))),
        ]
    } else if let Some(item) = app.current_selected_item() {
        let (status_str, status_color) = match item.status() {
            PackageStatus::Orphan => ("Orphan Dependency (Unreferenced by any active package)".to_string(), Color::Magenta),
            PackageStatus::TopLevel => ("Top-Level Explicit Install".to_string(), Color::Cyan),
            PackageStatus::Dependency { required_by_count } => {
                (format!("Shared Dependency (Required by {} packages)", required_by_count), Color::Red)
            }
            PackageStatus::BuildCache => ("Build Artifact Cache / Dangling Layer".to_string(), Color::Yellow),
            PackageStatus::ActiveRuntime => ("Active Toolchain Language Runtime".to_string(), Color::Green),
        };

        let mut lines = vec![
            Line::from(vec![
                Span::styled("📦 Package: ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::styled(&item.name, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                Span::raw(" ("),
                Span::styled(item.version.as_deref().unwrap_or("unknown"), Style::default().fg(Color::Yellow)),
                Span::raw(")"),
            ]),
            Line::from(vec![
                Span::styled("🏷️  Category: ", Style::default().fg(Color::Cyan)),
                Span::raw(format!("{}", item.category)),
            ]),
            Line::from(vec![
                Span::styled("🔗 Status: ", Style::default().fg(Color::Cyan)),
                Span::styled(status_str, Style::default().fg(status_color).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(vec![
                Span::styled("💾 Real Disk Size: ", Style::default().fg(Color::Cyan)),
                Span::styled(item.formatted_size(), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(vec![
                Span::styled("📂 Install Path: ", Style::default().fg(Color::Cyan)),
                Span::styled(item.install_path.display().to_string(), Style::default().fg(Color::DarkGray)),
            ]),
        ];

        if let Some(days) = item.inactivity_days() {
            lines.push(Line::from(vec![
                Span::styled("⏱️  Last Accessed: ", Style::default().fg(Color::Cyan)),
                Span::styled(format!("{} days ago", days), Style::default().fg(Color::White)),
            ]));
        }

        if !item.required_by.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("⛓️  Required By: ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                Span::styled(item.required_by.join(", "), Style::default().fg(Color::LightRed)),
            ]));
        }

        let meta = item.discover_associated_metadata();

        if !meta.configs.is_empty() {
            lines.push(Line::raw(""));
            lines.push(Line::from(vec![
                Span::styled("⚙️  Preserved Configs: ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::styled(format!("({} found)", meta.configs.len()), Style::default().fg(Color::DarkGray)),
            ]));
            for p in &meta.configs {
                lines.push(Line::from(vec![
                    Span::raw("  • "),
                    Span::styled(p.display().to_string(), Style::default().fg(Color::White)),
                ]));
            }
        }

        if !meta.data_dirs.is_empty() {
            lines.push(Line::raw(""));
            lines.push(Line::from(vec![
                Span::styled("💾 Data & State Dirs: ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::styled(format!("({} found)", meta.data_dirs.len()), Style::default().fg(Color::DarkGray)),
            ]));
            for p in &meta.data_dirs {
                lines.push(Line::from(vec![
                    Span::raw("  • "),
                    Span::styled(p.display().to_string(), Style::default().fg(Color::White)),
                ]));
            }
        }

        if !meta.logs.is_empty() {
            lines.push(Line::raw(""));
            lines.push(Line::from(vec![
                Span::styled("📜 Associated Logs: ", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
                Span::styled(format!("({} found)", meta.logs.len()), Style::default().fg(Color::DarkGray)),
            ]));
            for p in &meta.logs {
                lines.push(Line::from(vec![
                    Span::raw("  • "),
                    Span::styled(p.display().to_string(), Style::default().fg(Color::White)),
                ]));
            }
        }

        if !meta.caches.is_empty() {
            lines.push(Line::raw(""));
            lines.push(Line::from(vec![
                Span::styled("⚡ Associated Caches: ", Style::default().fg(Color::LightYellow).add_modifier(Modifier::BOLD)),
                Span::styled(format!("({} found)", meta.caches.len()), Style::default().fg(Color::DarkGray)),
            ]));
            for p in &meta.caches {
                lines.push(Line::from(vec![
                    Span::raw("  • "),
                    Span::styled(p.display().to_string(), Style::default().fg(Color::White)),
                ]));
            }
        }

        if !meta.env_vars.is_empty() {
            lines.push(Line::raw(""));
            lines.push(Line::from(vec![
                Span::styled("🌐 Environment Variables: ", Style::default().fg(Color::LightCyan).add_modifier(Modifier::BOLD)),
                Span::styled(format!("({} active)", meta.env_vars.len()), Style::default().fg(Color::DarkGray)),
            ]));
            for (k, v) in &meta.env_vars {
                let display_v = if v.len() > 40 { format!("{}...", &v[..37]) } else { v.clone() };
                lines.push(Line::from(vec![
                    Span::styled(format!("  {}=", k), Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD)),
                    Span::styled(display_v, Style::default().fg(Color::White)),
                ]));
            }
        }

        lines.push(Line::raw(""));

        if let Some(uninstall) = &item.uninstall_cmd {
            lines.push(Line::from(vec![
                Span::styled("✂️  Uninstall Command:\n", Style::default().fg(Color::LightYellow).add_modifier(Modifier::BOLD)),
                Span::styled(format!("  {}", uninstall), Style::default().fg(Color::White)),
            ]));
        }

        if let Some(reinstall) = &item.reinstall_cmd {
            lines.push(Line::raw(""));
            lines.push(Line::from(vec![
                Span::styled("🔄 Reinstall Command:\n", Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD)),
                Span::styled(format!("  {}", reinstall), Style::default().fg(Color::White)),
            ]));
        }

        lines
    } else {
        vec![Line::raw("No package matches the current filters")]
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(" Package Detail Inspector ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)))
        .style(Style::default().fg(Color::DarkGray));

    let paragraph = Paragraph::new(content).block(block).wrap(Wrap { trim: true });
    f.render_widget(paragraph, area);
}

fn draw_history_main(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(55), // Tombstones Table
            Constraint::Percentage(45), // Tombstone Inspector
        ])
        .split(area);

    // Left Table: Archived Tombstones
    let header_cells = ["Removed At", "Source", "Package Name", "Reclaimed", "Configs"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)));
    let header = Row::new(header_cells).height(1).bottom_margin(1);

    let rows = app.tombstones.iter().enumerate().map(|(i, t)| {
        let is_selected = i == app.selected_tombstone_idx;
        let cells = vec![
            Cell::from(t.removed_at.format("%Y-%m-%d %H:%M").to_string()),
            Cell::from(format!("{} {}", t.source.icon(), t.source.label())),
            Cell::from(t.name.clone()).style(Style::default().add_modifier(Modifier::BOLD)),
            Cell::from(format_bytes(t.size_bytes)),
            Cell::from(format!("{} files", t.associated_metadata.configs.len())),
        ];
        let mut row = Row::new(cells).height(1);
        if is_selected {
            row = row.style(Style::default().bg(Color::Rgb(30, 45, 65)).fg(Color::White).add_modifier(Modifier::BOLD));
        }
        row
    });

    let table = Table::new(
        rows,
        [
            Constraint::Length(18), // Removed At
            Constraint::Length(14), // Source
            Constraint::Length(22), // Package Name
            Constraint::Length(12), // Reclaimed Size
            Constraint::Length(12), // Configs count
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(Span::styled(" Archived Package Tombstones & History ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)))
            .style(Style::default().fg(Color::DarkGray)),
    );

    f.render_widget(table, chunks[0]);

    // Right: Selected Tombstone Inspector
    let content = if let Some(t) = app.tombstones.get(app.selected_tombstone_idx) {
        let mut lines = vec![
            Line::from(vec![
                Span::styled("🏛️ Archived Tombstone: ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::styled(&t.name, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                Span::raw(" ("),
                Span::styled(t.version.as_deref().unwrap_or("unknown"), Style::default().fg(Color::Yellow)),
                Span::raw(")"),
            ]),
            Line::from(vec![
                Span::styled("🕒 Removed At: ", Style::default().fg(Color::Cyan)),
                Span::styled(t.removed_at.format("%Y-%m-%d %H:%M:%S UTC").to_string(), Style::default().fg(Color::LightCyan)),
            ]),
            Line::from(vec![
                Span::styled("🏷️  Category: ", Style::default().fg(Color::Cyan)),
                Span::raw(format!("{}", t.category)),
            ]),
            Line::from(vec![
                Span::styled("💾 Reclaimed Size: ", Style::default().fg(Color::Cyan)),
                Span::styled(format_bytes(t.size_bytes), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(vec![
                Span::styled("📂 Original Install Path: ", Style::default().fg(Color::Cyan)),
                Span::styled(t.install_path.display().to_string(), Style::default().fg(Color::DarkGray)),
            ]),
        ];

        if !t.associated_metadata.configs.is_empty() {
            lines.push(Line::raw(""));
            lines.push(Line::from(vec![
                Span::styled("⚙️  Preserved Config Files: ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::styled(format!("({} preserved)", t.associated_metadata.configs.len()), Style::default().fg(Color::DarkGray)),
            ]));
            for p in &t.associated_metadata.configs {
                lines.push(Line::from(vec![
                    Span::raw("  • "),
                    Span::styled(p.display().to_string(), Style::default().fg(Color::White)),
                ]));
            }
        }

        if !t.associated_metadata.data_dirs.is_empty() {
            lines.push(Line::raw(""));
            lines.push(Line::from(vec![
                Span::styled("💾 Preserved Data Dirs: ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::styled(format!("({} preserved)", t.associated_metadata.data_dirs.len()), Style::default().fg(Color::DarkGray)),
            ]));
            for p in &t.associated_metadata.data_dirs {
                lines.push(Line::from(vec![
                    Span::raw("  • "),
                    Span::styled(p.display().to_string(), Style::default().fg(Color::White)),
                ]));
            }
        }

        if !t.associated_metadata.logs.is_empty() {
            lines.push(Line::raw(""));
            lines.push(Line::from(vec![
                Span::styled("📜 Associated Log File Paths: ", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
            ]));
            for p in &t.associated_metadata.logs {
                lines.push(Line::from(vec![
                    Span::raw("  • "),
                    Span::styled(p.display().to_string(), Style::default().fg(Color::DarkGray)),
                ]));
            }
        }

        if !t.associated_metadata.env_vars.is_empty() {
            lines.push(Line::raw(""));
            lines.push(Line::from(vec![
                Span::styled("🌐 Associated Environment Variables: ", Style::default().fg(Color::LightCyan).add_modifier(Modifier::BOLD)),
            ]));
            for (k, v) in &t.associated_metadata.env_vars {
                lines.push(Line::from(vec![
                    Span::styled(format!("  {}=", k), Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD)),
                    Span::styled(v.clone(), Style::default().fg(Color::White)),
                ]));
            }
        }

        if let Some(reinstall) = &t.reinstall_cmd {
            lines.push(Line::raw(""));
            lines.push(Line::from(vec![
                Span::styled("🔄 Inverse Reinstall Command:\n", Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD)),
                Span::styled(format!("  {}", reinstall), Style::default().fg(Color::White)),
            ]));
        }

        lines
    } else {
        vec![Line::raw("No package tombstone selected. Run a cleanup to archive packages.")]
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(" Forensic Tombstone Inspector ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)))
        .style(Style::default().fg(Color::DarkGray));

    let paragraph = Paragraph::new(content).block(block).wrap(Wrap { trim: true });
    f.render_widget(paragraph, chunks[1]);
}

fn draw_footer(f: &mut Frame, app: &App, area: Rect) {
    if app.input_mode == InputMode::HistoryView {
        let history_footer = Line::from(vec![
            Span::styled(" [j/k/↑/↓] ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw("Navigate  "),
            Span::styled("[Esc / Enter / h] ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw("Return to Live Inventory"),
        ]);
        let block = Block::default().borders(Borders::ALL).style(Style::default().fg(Color::Cyan));
        f.render_widget(Paragraph::new(history_footer).block(block), area);
        return;
    }

    match app.input_mode {
        InputMode::Searching => {
            let search_line = Line::from(vec![
                Span::styled(" 🔍 SEARCH: ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::styled(&app.search_query, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                Span::styled("█", Style::default().fg(Color::Cyan)),
                Span::styled("  (Press Enter/Esc to confirm)", Style::default().fg(Color::DarkGray)),
            ]);
            let block = Block::default().borders(Borders::ALL).style(Style::default().fg(Color::Yellow));
            f.render_widget(Paragraph::new(search_line).block(block), area);
        }
        InputMode::FilterMenu => {
            let orphan_badge = if app.only_orphans { Span::styled("[ON]", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)) } else { Span::styled("[off]", Style::default().fg(Color::DarkGray)) };
            let cache_badge = if app.only_caches { Span::styled("[ON]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)) } else { Span::styled("[off]", Style::default().fg(Color::DarkGray)) };
            let top_badge = if app.only_top_level { Span::styled("[ON]", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)) } else { Span::styled("[off]", Style::default().fg(Color::DarkGray)) };
            let eco_str = app.ecosystems_summary();
            let cat_str = app.categories_summary();
            let inact_str = app.min_inactivity_days.map(|d| format!(">{}d", d)).unwrap_or_else(|| "ALL".to_string());

            let filter_line = Line::from(vec![
                Span::styled(" 🎛️ FILTER: ", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
                Span::styled("[o] ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::raw("Orphans "),
                orphan_badge,
                Span::raw("  "),
                Span::styled("[c] ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::raw("Caches "),
                cache_badge,
                Span::raw("  "),
                Span::styled("[t] ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::raw("Top-Level "),
                top_badge,
                Span::raw("  "),
                Span::styled("[e] ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::styled(format!("Eco:{} -> ", eco_str), Style::default().fg(Color::LightCyan)),
                Span::styled("[d] ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                Span::styled(format!("Domain:{} -> ", cat_str), Style::default().fg(Color::LightGreen)),
                Span::styled("[i] ", Style::default().fg(Color::LightYellow).add_modifier(Modifier::BOLD)),
                Span::styled(format!("Inact:{} ", inact_str), Style::default().fg(Color::LightYellow)),
                Span::styled("[a] ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                Span::raw("Reset  "),
                Span::styled("[Esc/Enter] ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                Span::raw("Done"),
            ]);
            let block = Block::default().borders(Borders::ALL).style(Style::default().fg(Color::Magenta));
            f.render_widget(Paragraph::new(filter_line).block(block), area);
        }
        InputMode::EcosystemMenu => {
            let has = |key: &str| app.active_ecosystems.contains(key);
            let eco_item = |key: &str, label: &str, hotkey: &str| -> Vec<Span> {
                let badge = if has(key) {
                    Span::styled("[✓]", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
                } else {
                    Span::styled("[ ]", Style::default().fg(Color::DarkGray))
                };
                vec![
                    Span::styled(format!("[{}] ", hotkey), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                    Span::raw(format!("{} ", label)),
                    badge,
                    Span::raw("  "),
                ]
            };

            let mut spans = vec![
                Span::styled(" 🌐 ECOSYSTEMS: ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            ];
            spans.extend(eco_item("ruby", "Ruby", "r"));
            spans.extend(eco_item("rust", "Rust", "u"));
            spans.extend(eco_item("js", "JS", "j"));
            spans.extend(eco_item("python", "Py", "p"));
            spans.extend(eco_item("go", "Go", "g"));
            spans.extend(eco_item("brew", "Brew", "b"));
            spans.extend(eco_item("mise", "Mise", "m"));
            spans.extend(eco_item("apps", "Apps", "a"));
            spans.extend(eco_item("cache", "Cache", "c"));
            spans.extend(eco_item("docker", "Docker", "k"));
            spans.push(Span::styled("[x] ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)));
            spans.push(Span::raw("All  "));
            spans.push(Span::styled("[Esc/Enter] ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)));
            spans.push(Span::raw("Back"));

            let block = Block::default().borders(Borders::ALL).style(Style::default().fg(Color::Cyan));
            f.render_widget(Paragraph::new(Line::from(spans)).block(block), area);
        }
        InputMode::CategoryMenu => {
            let has = |cat: &Category| app.active_categories.contains(cat);
            let cat_item = |cat: Category, label: &str, hotkey: &str| -> Vec<Span> {
                let badge = if has(&cat) {
                    Span::styled("[✓]", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
                } else {
                    Span::styled("[ ]", Style::default().fg(Color::DarkGray))
                };
                vec![
                    Span::styled(format!("[{}] ", hotkey), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                    Span::raw(format!("{} ", label)),
                    badge,
                    Span::raw("  "),
                ]
            };

            let mut spans = vec![
                Span::styled(" 🗂️ DOMAINS: ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            ];
            spans.extend(cat_item(Category::LanguagesAndRuntimes, "Runtimes", "r"));
            spans.extend(cat_item(Category::DatabasesAndStorage, "DB", "d"));
            spans.extend(cat_item(Category::CliDeveloperTools, "CLI", "c"));
            spans.extend(cat_item(Category::InfrastructureAndCloud, "Infra", "i"));
            spans.extend(cat_item(Category::BuildAndPackageManagers, "PkgMgr", "p"));
            spans.extend(cat_item(Category::GuiAppsAndMedia, "GUI", "g"));
            spans.extend(cat_item(Category::BuildArtifactsAndCaches, "Cache", "b"));
            spans.extend(cat_item(Category::SystemAndLibraries, "System", "s"));
            spans.push(Span::styled("[x] ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)));
            spans.push(Span::raw("All  "));
            spans.push(Span::styled("[Esc/Enter] ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)));
            spans.push(Span::raw("Back"));

            let block = Block::default().borders(Borders::ALL).style(Style::default().fg(Color::Green));
            f.render_widget(Paragraph::new(Line::from(spans)).block(block), area);
        }
        InputMode::SortMenu => {
            let sort_line = Line::from(vec![
                Span::styled(" 📊 SORT: ", Style::default().fg(Color::LightBlue).add_modifier(Modifier::BOLD)),
                Span::styled("[1] ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::raw("Disk Size  "),
                Span::styled("[2] ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::raw("Name (A-Z)  "),
                Span::styled("[3] ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::raw("Inactivity  "),
                Span::styled("[4] ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::raw("Source Eco  "),
                Span::styled("[5] ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::raw("Category  "),
                Span::styled("[Esc/Enter] ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                Span::raw("Done"),
            ]);
            let block = Block::default().borders(Borders::ALL).style(Style::default().fg(Color::LightBlue));
            f.render_widget(Paragraph::new(sort_line).block(block), area);
        }
        InputMode::MarkMenu => {
            let mark_line = Line::from(vec![
                Span::styled(" 🏷️ MARK: ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                Span::styled("[a] ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::raw("Mark All Visible  "),
                Span::styled("[o] ", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
                Span::raw("Mark Orphans  "),
                Span::styled("[c] ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::raw("Mark Caches  "),
                Span::styled("[x] ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                Span::raw("Clear All Marks  "),
                Span::styled("[Esc/Enter] ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                Span::raw("Done"),
            ]);
            let block = Block::default().borders(Borders::ALL).style(Style::default().fg(Color::Green));
            f.render_widget(Paragraph::new(mark_line).block(block), area);
        }
        InputMode::HistoryView => {}
        InputMode::Normal | InputMode::HelpModal => {
            let mut spans = vec![
                Span::styled(" [f] ", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
                Span::raw("Filter  "),
                Span::styled("[s] ", Style::default().fg(Color::LightBlue).add_modifier(Modifier::BOLD)),
                Span::raw("Sort  "),
                Span::styled("[m] ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                Span::raw("Mark  "),
                Span::styled("[h] ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::raw("History  "),
                Span::styled("[/] ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::raw("Search  "),
                Span::styled("[Space] ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                Span::raw("Toggle  "),
                Span::styled("[x] ", Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD)),
                Span::styled("Export Cleanup  ", Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD)),
                Span::styled("[?] ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::raw("Help  "),
                Span::styled("[q] ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                Span::raw("Quit"),
            ];

            if let Some((msg, is_success)) = &app.status_message {
                let color = if *is_success { Color::LightGreen } else { Color::LightRed };
                spans.push(Span::raw(" | "));
                spans.push(Span::styled(format!("📢 {}", msg), Style::default().fg(color)));
            }

            let block = Block::default().borders(Borders::ALL).style(Style::default().fg(Color::DarkGray));
            f.render_widget(Paragraph::new(Line::from(spans)).block(block), area);
        }
    }
}

fn draw_help_modal(f: &mut Frame, area: Rect) {
    let modal_area = centered_rect(70, 85, area);

    f.render_widget(Clear, modal_area);

    let help_text = vec![
        Line::from(Span::styled("etop: Ecosystem Top Navigation System", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
        Line::raw("=================================================================="),
        Line::from(vec![
            Span::styled("  [f] Filter Menu ->  ", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
            Span::raw("Compose multi-select queries with active badges [✓]:"),
        ]),
        Line::from(vec![
            Span::styled("      o               ", Style::default().fg(Color::Yellow)),
            Span::raw("Toggle Orphan Dependencies [ON/OFF]"),
        ]),
        Line::from(vec![
            Span::styled("      c               ", Style::default().fg(Color::Yellow)),
            Span::raw("Toggle Build Caches & Dangling Layers [ON/OFF]"),
        ]),
        Line::from(vec![
            Span::styled("      t               ", Style::default().fg(Color::Yellow)),
            Span::raw("Toggle Top-Level Explicit Installs [ON/OFF]"),
        ]),
        Line::from(vec![
            Span::styled("      e               ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw("Open Ecosystem Sub-Menu (Select Multiple: Ruby, Rust, JS, Python, Go, etc.)"),
        ]),
        Line::from(vec![
            Span::styled("      d               ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::raw("Open Domain Sub-Menu (Select Multiple: CLI, DB, Runtimes, GUI, etc.)"),
        ]),
        Line::from(vec![
            Span::styled("      i               ", Style::default().fg(Color::LightYellow)),
            Span::raw("Cycle Inactivity threshold (>30d, >90d, >180d, ALL)"),
        ]),
        Line::from(vec![
            Span::styled("      a               ", Style::default().fg(Color::White)),
            Span::raw("Reset / Clear all active filters"),
        ]),
        Line::raw("------------------------------------------------------------------"),
        Line::from(vec![
            Span::styled("  [s] Sort Menu   ->  ", Style::default().fg(Color::LightBlue).add_modifier(Modifier::BOLD)),
            Span::raw("Choose Sort Order ([1] Size, [2] Name, [3] Inactivity, etc.)"),
        ]),
        Line::from(vec![
            Span::styled("  [m] Mark Menu   ->  ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::raw("Batch Marking ([a] Visible, [o] Orphans, [c] Caches, [x] Clear)"),
        ]),
        Line::from(vec![
            Span::styled("  [h] History     ->  ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw("View uninstalled package tombstones, configs & history"),
        ]),
        Line::from(vec![
            Span::styled("  [/] Search          ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw("Live search by package name, ID, or description"),
        ]),
        Line::from(vec![
            Span::styled("  j / k / ↑ / ↓       ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::raw("Navigate package rows"),
        ]),
        Line::from(vec![
            Span::styled("  Space               ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::raw("Toggle single item checkbox [✓]"),
        ]),
        Line::from(vec![
            Span::styled("  x / Enter           ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::raw("Export deterministic cleanup.sh & rollback_reinstall.sh"),
        ]),
        Line::from(vec![
            Span::styled("  ?                   ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw("Toggle this Help Modal"),
        ]),
        Line::from(vec![
            Span::styled("  q / Esc             ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            Span::raw("Exit sub-menu or Quit application"),
        ]),
        Line::raw("=================================================================="),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(" Modal Hotkey Reference ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)))
        .style(Style::default().bg(Color::Rgb(20, 25, 35)).fg(Color::White));

    let paragraph = Paragraph::new(help_text)
        .block(block)
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, modal_area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
