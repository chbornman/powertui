use std::fs;
use std::io::stdout;
use std::process::Command;
use std::time::Duration;

use color_eyre::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Gauge, List, ListItem, ListState, Paragraph},
};

#[derive(Clone, Copy, PartialEq)]
enum Profile {
    PowerSaver,
    Balanced,
    Performance,
}

impl Profile {
    fn all() -> [Profile; 3] {
        [Profile::PowerSaver, Profile::Balanced, Profile::Performance]
    }

    fn name(&self) -> &'static str {
        match self {
            Profile::PowerSaver => "Power Saver",
            Profile::Balanced => "Balanced",
            Profile::Performance => "Performance",
        }
    }

    fn governor(&self) -> &'static str {
        match self {
            Profile::PowerSaver => "powersave",
            Profile::Balanced => "schedutil",
            Profile::Performance => "performance",
        }
    }

    fn from_governor(gov: &str) -> Option<Profile> {
        match gov.trim() {
            "powersave" => Some(Profile::PowerSaver),
            "schedutil" => Some(Profile::Balanced),
            "performance" => Some(Profile::Performance),
            _ => None,
        }
    }
}

struct BatteryInfo {
    capacity: u8,
    status: String,
    health: Option<u8>,
    time_remaining: Option<String>,
}

struct App {
    battery: Option<BatteryInfo>,
    current_profile: Option<Profile>,
    selected: usize,
    list_state: ListState,
    message: Option<String>,
}

impl App {
    fn new() -> Self {
        let mut app = Self {
            battery: None,
            current_profile: None,
            selected: 0,
            list_state: ListState::default(),
            message: None,
        };
        app.list_state.select(Some(0));
        app.refresh();
        app
    }

    fn refresh(&mut self) {
        self.battery = read_battery_info();
        self.current_profile = read_current_governor();

        // Set selection to current profile
        if let Some(current) = self.current_profile {
            for (i, profile) in Profile::all().iter().enumerate() {
                if *profile == current {
                    self.selected = i;
                    self.list_state.select(Some(i));
                    break;
                }
            }
        }
    }

    fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
            self.list_state.select(Some(self.selected));
        }
    }

    fn move_down(&mut self) {
        if self.selected < Profile::all().len() - 1 {
            self.selected += 1;
            self.list_state.select(Some(self.selected));
        }
    }

    fn select_profile(&mut self) {
        let profile = Profile::all()[self.selected];
        match set_governor(profile.governor()) {
            Ok(()) => {
                self.current_profile = Some(profile);
                self.message = Some(format!("Switched to {}", profile.name()));
            }
            Err(e) => {
                self.message = Some(format!("Error: {}", e));
            }
        }
    }
}

fn read_battery_info() -> Option<BatteryInfo> {
    let base = "/sys/class/power_supply";

    // Find battery (usually BAT0 or macsmc-battery on Asahi)
    let battery_path = fs::read_dir(base).ok()?.find_map(|entry| {
        let entry = entry.ok()?;
        let type_path = entry.path().join("type");
        let bat_type = fs::read_to_string(type_path).ok()?;
        if bat_type.trim() == "Battery" {
            Some(entry.path())
        } else {
            None
        }
    })?;

    let capacity = fs::read_to_string(battery_path.join("capacity"))
        .ok()?
        .trim()
        .parse()
        .ok()?;

    let status = fs::read_to_string(battery_path.join("status"))
        .ok()
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "Unknown".to_string());

    // Calculate health from energy_full vs energy_full_design
    let health = (|| {
        let full: f64 = fs::read_to_string(battery_path.join("energy_full"))
            .ok()?
            .trim()
            .parse()
            .ok()?;
        let design: f64 = fs::read_to_string(battery_path.join("energy_full_design"))
            .ok()?
            .trim()
            .parse()
            .ok()?;
        Some(((full / design) * 100.0) as u8)
    })();

    // Calculate time remaining
    let time_remaining = (|| {
        let power_now: f64 = fs::read_to_string(battery_path.join("power_now"))
            .ok()?
            .trim()
            .parse()
            .ok()?;

        if power_now <= 0.0 {
            return None;
        }

        let energy: f64 = if status == "Charging" {
            let full: f64 = fs::read_to_string(battery_path.join("energy_full"))
                .ok()?
                .trim()
                .parse()
                .ok()?;
            let now: f64 = fs::read_to_string(battery_path.join("energy_now"))
                .ok()?
                .trim()
                .parse()
                .ok()?;
            full - now
        } else {
            fs::read_to_string(battery_path.join("energy_now"))
                .ok()?
                .trim()
                .parse()
                .ok()?
        };

        let hours = energy / power_now;
        let h = hours as u32;
        let m = ((hours - h as f64) * 60.0) as u32;

        if status == "Charging" {
            Some(format!("{}h {}m until full", h, m))
        } else {
            Some(format!("{}h {}m remaining", h, m))
        }
    })();

    Some(BatteryInfo {
        capacity,
        status,
        health,
        time_remaining,
    })
}

fn read_current_governor() -> Option<Profile> {
    let gov = fs::read_to_string("/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor").ok()?;
    Profile::from_governor(&gov)
}

fn set_governor(governor: &str) -> Result<(), String> {
    let output = Command::new("sudo")
        .args(["-n", "cpupower", "frequency-set", "-g", governor])
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        Ok(())
    } else {
        Err("Need passwordless sudo for cpupower".to_string())
    }
}

fn main() -> Result<()> {
    color_eyre::install()?;

    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;

    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    let mut app = App::new();

    loop {
        terminal.draw(|f| ui(f, &mut app))?;

        if event::poll(Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => break,
                        KeyCode::Char('j') | KeyCode::Down => app.move_down(),
                        KeyCode::Char('k') | KeyCode::Up => app.move_up(),
                        KeyCode::Enter | KeyCode::Char(' ') => app.select_profile(),
                        KeyCode::Char('r') => app.refresh(),
                        _ => {}
                    }
                }
            }
        }
    }

    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}

fn ui(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(5), // Battery
            Constraint::Length(5), // Profiles
            Constraint::Length(2), // Help/message
        ])
        .split(f.area());

    // Battery widget
    let battery_block = Block::default()
        .title(" Battery ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    if let Some(ref bat) = app.battery {
        let color = match bat.capacity {
            0..=20 => Color::Red,
            21..=50 => Color::Yellow,
            _ => Color::Green,
        };

        let label = format!(
            "{}%  {}{}",
            bat.capacity,
            bat.status,
            bat.time_remaining
                .as_ref()
                .map(|t| format!("  ({})", t))
                .unwrap_or_default()
        );

        let health_str = bat
            .health
            .map(|h| format!("  Health: {}%", h))
            .unwrap_or_default();

        let gauge = Gauge::default()
            .block(battery_block)
            .gauge_style(Style::default().fg(color))
            .ratio(bat.capacity as f64 / 100.0)
            .label(format!("{}{}", label, health_str));

        f.render_widget(gauge, chunks[0]);
    } else {
        let no_battery = Paragraph::new("No battery found")
            .block(battery_block)
            .style(Style::default().fg(Color::DarkGray));
        f.render_widget(no_battery, chunks[0]);
    }

    // Profile list
    let profiles: Vec<ListItem> = Profile::all()
        .iter()
        .map(|p| {
            let is_current = app.current_profile == Some(*p);
            let marker = if is_current { " ● " } else { "   " };
            let text = format!("{}{} ({})", marker, p.name(), p.governor());
            let style = if is_current {
                Style::default().fg(Color::Green)
            } else {
                Style::default()
            };
            ListItem::new(text).style(style)
        })
        .collect();

    let profiles_block = Block::default()
        .title(" Power Profile ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let list = List::new(profiles)
        .block(profiles_block)
        .highlight_style(Style::default().bg(Color::DarkGray).fg(Color::White))
        .highlight_symbol("▶ ");

    f.render_stateful_widget(list, chunks[1], &mut app.list_state);

    // Help/message line
    let help_text = if let Some(ref msg) = app.message {
        msg.clone()
    } else {
        "j/k navigate  Enter select  r refresh  q quit".to_string()
    };

    let help = Paragraph::new(help_text)
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);

    f.render_widget(help, chunks[2]);
}
