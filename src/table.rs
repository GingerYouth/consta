use crate::model::RepoStats;
use comfy_table::{Attribute, Cell, Color, Table, presets};
use std::path::Path;

pub fn print_summary(stats: &[RepoStats], breakdown: bool) {
    let mut table = Table::new();
    table.load_preset(presets::ASCII_BORDERS_ONLY);
    table.set_header(vec![
        Cell::new("project").add_attribute(Attribute::Bold).fg(Color::Green),
        Cell::new("LoC").add_attribute(Attribute::Bold).fg(Color::Green),
        Cell::new("added").add_attribute(Attribute::Bold).fg(Color::Green),
        Cell::new("deleted").add_attribute(Attribute::Bold).fg(Color::Green),
        Cell::new("commits").add_attribute(Attribute::Bold).fg(Color::Green),
    ]);

    for r in stats {
        table.add_row(vec![
            project_name(&r.path),
            format!("{}", r.added.saturating_sub(r.deleted)),
            format!("+{}", r.added.to_string()),
            format!("-{}", r.deleted.to_string()),
            r.commits.to_string(),
        ]);
    }

    table.add_row(vec![
        "summary".to_string(),
        format!("{}", stats.iter().map(|r| r.added.saturating_sub(r.deleted)).sum::<usize>()),
        format!("+{}", stats.iter().map(|r| r.added).sum::<usize>()),
        format!("-{}", stats.iter().map(|r| r.deleted).sum::<usize>()),
        stats.iter().map(|r| r.commits).sum::<usize>().to_string(),
    ]);

    println!("{table}");

    if breakdown {
        print_breakdown(stats);
    }
}

fn print_breakdown(stats: &[RepoStats]) {
    for repo in stats {
        println!("\n{}", project_name(&repo.path));
        for c in &repo.entries {
            println!("- {} {} (+{} / -{})", &c.hash[..7], c.message, c.added, c.deleted);
        }
    }
}

fn project_name(path: &Path) -> String {
    path.file_name()
        .map_or_else(|| path.display().to_string(), |s| s.to_string_lossy().into_owned())
}
