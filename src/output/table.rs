use tabled::{Table, Tabled};
use colored::Colorize;
use crate::models::InventoryItem;

#[derive(Tabled)]
struct TableRow {
    #[tabled(rename = "Source")]
    source: String,

    #[tabled(rename = "Name")]
    name: String,

    #[tabled(rename = "Category")]
    category: String,

    #[tabled(rename = "Version")]
    version: String,

    #[tabled(rename = "Size")]
    size: String,

    #[tabled(rename = "Status")]
    status: String,

    #[tabled(rename = "Last Used")]
    last_used: String,
}

pub fn render_inventory_table(items: &[InventoryItem]) -> String {
    let rows: Vec<TableRow> = items
        .iter()
        .map(|item| {
            TableRow {
                source: format!("{} {}", item.source.icon(), item.source.label()),
                name: item.name.bold().to_string(),
                category: format!("{}", item.category),
                version: item.version.clone().unwrap_or_else(|| "-".to_string()),
                size: item.formatted_size(),
                status: item.status().label(),
                last_used: item.formatted_last_accessed(),
            }
        })
        .collect();

    let mut table = Table::new(rows);
    table.with(tabled::settings::Style::modern());
    table.to_string()
}
