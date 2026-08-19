pub mod app;
pub mod ui;

use std::io::{self, stdout};
use std::time::Duration;
use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use crate::models::Category;
use app::{App, InputMode, SortOrder};

/// Launches the interactive fullscreen TUI dashboard instantly with live streaming
pub fn run_tui() -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new_streaming();
    let res = run_event_loop(&mut terminal, &mut app);

    // Always restore terminal state cleanly
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("TUI runtime error: {:?}", err);
    }

    Ok(())
}

fn run_event_loop(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> Result<()> {
    loop {
        // 1. Drain background streaming events & tick spinner animations
        app.tick_events();

        // 2. Draw immediate-mode frame
        terminal.draw(|f| ui::draw(f, app))?;

        // 3. Poll for keyboard input with a snappy 30ms timeout (smooth ~33 FPS)
        if event::poll(Duration::from_millis(30))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match app.input_mode {
                        InputMode::Searching => match key.code {
                            KeyCode::Esc | KeyCode::Enter => {
                                app.input_mode = InputMode::Normal;
                            }
                            KeyCode::Backspace => {
                                app.search_query.pop();
                                app.reapply_filters();
                            }
                            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                app.search_query.clear();
                                app.reapply_filters();
                            }
                            KeyCode::Char(c) => {
                                app.search_query.push(c);
                                app.reapply_filters();
                            }
                            _ => {}
                        },
                        InputMode::FilterMenu => match key.code {
                            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') | KeyCode::Char('f') => {
                                app.input_mode = InputMode::Normal;
                            }
                            KeyCode::Char('o') => app.toggle_orphans(),
                            KeyCode::Char('c') => app.toggle_caches(),
                            KeyCode::Char('t') => app.toggle_top_level(),
                            KeyCode::Char('e') => {
                                app.input_mode = InputMode::EcosystemMenu;
                            }
                            KeyCode::Char('d') => {
                                app.input_mode = InputMode::CategoryMenu;
                            }
                            KeyCode::Char('i') => app.cycle_inactivity(),
                            KeyCode::Char('a') => app.clear_filters(),
                            _ => {}
                        },
                        InputMode::EcosystemMenu => match key.code {
                            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') | KeyCode::Char('f') => {
                                app.input_mode = InputMode::FilterMenu;
                            }
                            KeyCode::Char('r') => app.toggle_ecosystem("ruby"),
                            KeyCode::Char('u') => app.toggle_ecosystem("rust"),
                            KeyCode::Char('j') => app.toggle_ecosystem("js"),
                            KeyCode::Char('p') => app.toggle_ecosystem("python"),
                            KeyCode::Char('g') => app.toggle_ecosystem("go"),
                            KeyCode::Char('b') => app.toggle_ecosystem("brew"),
                            KeyCode::Char('m') => app.toggle_ecosystem("mise"),
                            KeyCode::Char('a') => app.toggle_ecosystem("apps"),
                            KeyCode::Char('c') => app.toggle_ecosystem("cache"),
                            KeyCode::Char('d') | KeyCode::Char('k') => app.toggle_ecosystem("docker"),
                            KeyCode::Char('x') => app.clear_ecosystems(),
                            _ => {}
                        },
                        InputMode::CategoryMenu => match key.code {
                            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') | KeyCode::Char('f') => {
                                app.input_mode = InputMode::FilterMenu;
                            }
                            KeyCode::Char('r') => app.toggle_category(Category::LanguagesAndRuntimes),
                            KeyCode::Char('d') => app.toggle_category(Category::DatabasesAndStorage),
                            KeyCode::Char('c') => app.toggle_category(Category::CliDeveloperTools),
                            KeyCode::Char('i') => app.toggle_category(Category::InfrastructureAndCloud),
                            KeyCode::Char('p') => app.toggle_category(Category::BuildAndPackageManagers),
                            KeyCode::Char('g') => app.toggle_category(Category::GuiAppsAndMedia),
                            KeyCode::Char('b') => app.toggle_category(Category::BuildArtifactsAndCaches),
                            KeyCode::Char('s') => app.toggle_category(Category::SystemAndLibraries),
                            KeyCode::Char('x') => app.clear_categories(),
                            _ => {}
                        },
                        InputMode::SortMenu => match key.code {
                            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') | KeyCode::Char('s') => {
                                app.input_mode = InputMode::Normal;
                            }
                            KeyCode::Char('1') => {
                                app.set_sort(SortOrder::SizeDesc);
                                app.input_mode = InputMode::Normal;
                            }
                            KeyCode::Char('2') => {
                                app.set_sort(SortOrder::NameAsc);
                                app.input_mode = InputMode::Normal;
                            }
                            KeyCode::Char('3') => {
                                app.set_sort(SortOrder::LastUsedDesc);
                                app.input_mode = InputMode::Normal;
                            }
                            KeyCode::Char('4') => {
                                app.set_sort(SortOrder::SourceAsc);
                                app.input_mode = InputMode::Normal;
                            }
                            KeyCode::Char('5') => {
                                app.set_sort(SortOrder::CategoryAsc);
                                app.input_mode = InputMode::Normal;
                            }
                            _ => {}
                        },
                        InputMode::MarkMenu => match key.code {
                            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') | KeyCode::Char('m') => {
                                app.input_mode = InputMode::Normal;
                            }
                            KeyCode::Char('a') => {
                                app.mark_all_visible();
                                app.input_mode = InputMode::Normal;
                            }
                            KeyCode::Char('o') => {
                                app.mark_visible_orphans();
                                app.input_mode = InputMode::Normal;
                            }
                            KeyCode::Char('c') => {
                                app.mark_visible_caches();
                                app.input_mode = InputMode::Normal;
                            }
                            KeyCode::Char('x') => {
                                app.clear_marks();
                                app.input_mode = InputMode::Normal;
                            }
                            _ => {}
                        },
                        InputMode::HistoryView => match key.code {
                            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') | KeyCode::Char('h') => {
                                app.input_mode = InputMode::Normal;
                            }
                            KeyCode::Down | KeyCode::Char('j') => app.next(),
                            KeyCode::Up | KeyCode::Char('k') => app.previous(),
                            KeyCode::PageDown => app.page_down(),
                            KeyCode::PageUp => app.page_up(),
                            _ => {}
                        },
                        InputMode::HelpModal => match key.code {
                            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') | KeyCode::Enter => {
                                app.input_mode = InputMode::Normal;
                            }
                            _ => {}
                        },
                        InputMode::Normal => match key.code {
                            KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                            KeyCode::Char('f') => {
                                app.input_mode = InputMode::FilterMenu;
                            }
                            KeyCode::Char('s') => {
                                app.input_mode = InputMode::SortMenu;
                            }
                            KeyCode::Char('m') => {
                                app.input_mode = InputMode::MarkMenu;
                            }
                            KeyCode::Char('h') => {
                                app.open_history();
                            }
                            KeyCode::Char('/') => {
                                app.input_mode = InputMode::Searching;
                            }
                            KeyCode::Char('?') => {
                                app.input_mode = InputMode::HelpModal;
                            }
                            KeyCode::Down | KeyCode::Char('j') => app.next(),
                            KeyCode::Up | KeyCode::Char('k') => app.previous(),
                            KeyCode::PageDown => app.page_down(),
                            KeyCode::PageUp => app.page_up(),
                            KeyCode::Char(' ') => app.toggle_mark(),
                            KeyCode::Char('a') => app.clear_filters(),
                            KeyCode::Char('x') | KeyCode::Enter => {
                                let _ = app.generate_scripts();
                            }
                            _ => {}
                        },
                    }
                }
            }
        }
    }
}
