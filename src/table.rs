use crate::model::RepoStats;
use chrono::{Datelike, Duration, Local, NaiveDate};
use colored::Colorize;
use comfy_table::{Attribute, Cell, Color, Table, presets};
use std::collections::HashMap;
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
            r.commits_amount.to_string(),
        ]);
    }

    table.add_row(vec![
        "summary".to_string(),
        format!("{}", stats.iter().map(|r| r.added.saturating_sub(r.deleted)).sum::<usize>()),
        format!("+{}", stats.iter().map(|r| r.added).sum::<usize>()),
        format!("-{}", stats.iter().map(|r| r.deleted).sum::<usize>()),
        stats.iter().map(|r| r.commits_amount).sum::<usize>().to_string(),
    ]);

    println!("{table}");

    if breakdown {
        print_breakdown(stats);
    }
}

fn print_breakdown(stats: &[RepoStats]) {
    for repo in stats {
        if !repo.commits.is_empty() {
            println!("\n{}", project_name(&repo.path).green());
            for c in &repo.commits {
                if c.added != 0 || c.deleted != 0 {
                    println!(
                        "- {} {} (+{} / -{})",
                        &c.hash[..7].to_string().purple(),
                        c.message,
                        c.added.to_string().green(),
                        c.deleted.to_string().red()
                    );
                }
            }
        }
    }
}

fn project_name(path: &Path) -> String {
    path.file_name()
        .map_or_else(|| path.display().to_string(), |s| s.to_string_lossy().into_owned())
}

/// Prints a GitHub-style contribution graph for the current year.
///
/// # Panics
///
/// Panics if January 1st cannot be constructed for the current year
/// (this should never happen for valid calendar years).
pub fn print_grid(stats: &[RepoStats], year: Option<i32>) {
    let year = year.unwrap_or_else(|| Local::now().year());
    println!("\n{}", format!("Contribution Activity Grid - {year}").green());

    // Collect all commits by date for the selected year
    let mut commits_by_date: HashMap<NaiveDate, usize> = HashMap::new();

    for repo in stats {
        for commit in &repo.commits {
            if let Some(date_str) = commit.date.split('T').next()
                && let Ok(date) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
            {
                // Only include commits from the specified year
                if date.year() == year {
                    *commits_by_date.entry(date).or_insert(0) += 1;
                }
            }
        }
    }

    // Calculate start and end dates for the grid
    let jan_1 =
        NaiveDate::from_ymd_opt(year, 1, 1).unwrap_or_else(|| panic!("Invalid year: {year}"));

    // If it's the current year, end at today, otherwise end at Dec 31
    let today = Local::now().naive_local().date();
    let dec_31 = NaiveDate::from_ymd_opt(year, 12, 31).unwrap();

    let end_date = if year == today.year() { today } else { dec_31 };

    // Find the Sunday on or before January 1st to align the grid
    let days_from_sunday = jan_1.weekday().num_days_from_sunday();
    let start_date = jan_1 - Duration::days(i64::from(days_from_sunday));

    // Calculate number of weeks to display using integer arithmetic to avoid casts
    let days_until_end = (end_date - start_date).num_days();
    let days_until_end_u64 =
        u64::try_from(days_until_end.max(0)).expect("Amount of days until end should fit in u64");
    let weeks_to_display = days_until_end_u64.div_ceil(7);
    let weeks_to_display =
        usize::try_from(weeks_to_display).expect("Amount of weeks should fit in usize");

    // Track month changes
    let mut month_starts = vec![0]; // First week is always a start
    let mut prev_month = start_date.month();

    for week in 1..weeks_to_display {
        let date = start_date + Duration::weeks(usize_to_i64(week));
        if date.month() != prev_month {
            month_starts.push(week);
            prev_month = date.month();
        }
    }

    // Print month labels
    print!("     ");
    for i in 0..month_starts.len() {
        let week_idx = month_starts[i];
        let date = start_date + Duration::weeks(usize_to_i64(week_idx));
        let month_name = month_abbr(date.month());

        // Calculate width: weeks until next month (or end) * 2 (emoji width) + gaps
        let next_start =
            if i + 1 < month_starts.len() { month_starts[i + 1] } else { weeks_to_display };
        let weeks = next_start - week_idx;

        print!("{:<width$}", month_name, width = weeks * 2 + 2);
    }
    println!();

    // Print contribution grid
    let days = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

    for (day_idx, day_name) in days.iter().enumerate() {
        // Only show labels for Mon, Wed, Fri
        if day_idx == 1 || day_idx == 3 || day_idx == 5 {
            print!("{day_name:>3}  ");
        } else {
            print!("     ");
        }

        let mut prev_month = start_date.month();

        for week in 0..weeks_to_display {
            let date = start_date
                + Duration::weeks(usize_to_i64(week))
                + Duration::days(usize_to_i64(day_idx));

            // Add gap at month boundary
            if week > 0 && date.month() != prev_month {
                print!("  ");
                prev_month = date.month();
            }

            if date.year() != year || date > end_date {
                print!("  ");
                continue;
            }

            let count = commits_by_date.get(&date).copied().unwrap_or(0);
            print!("{}", get_activity_char(count));
        }
        println!();
    }

    // Print legend
    println!(
        "\n  Less {} {} {} {} {} More",
        get_activity_char(0),
        get_activity_char(1),
        get_activity_char(3),
        get_activity_char(6),
        get_activity_char(10)
    );
}

const fn get_activity_char(count: usize) -> &'static str {
    match count {
        0 => "⬜",
        1 => "🟩",
        2..=4 => "🟨",
        5..=9 => "🟧",
        _ => "🟥",
    }
}

const fn month_abbr(month: u32) -> &'static str {
    match month {
        1 => "Jan",
        2 => "Feb",
        3 => "Mar",
        4 => "Apr",
        5 => "May",
        6 => "Jun",
        7 => "Jul",
        8 => "Aug",
        9 => "Sep",
        10 => "Oct",
        11 => "Nov",
        12 => "Dec",
        _ => "???",
    }
}

#[inline]
fn usize_to_i64(u: usize) -> i64 {
    i64::try_from(u).expect("value should fit into i64")
}
