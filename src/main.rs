mod espanso_yaml;

use dirs::config_dir;
use espanso_yaml::{EspansoYaml, YamlPairs};
use home;
use iced::theme::Theme;
use iced::widget::{
    button, column, container, row, scrollable, text, text_input, Button, Column, Container,
    Scrollable, Space,
};
use iced::{
    alignment, font, window, Alignment, Application, Command, Element, Length, Renderer, Settings,
};
use iced_aw::{modal, Card};
use once_cell::sync::Lazy;
use rfd::FileDialog;
use serde_yaml::{from_reader, to_writer};
use std::path::PathBuf;
use std::process::Command as p_cmd;
use walkdir::WalkDir;

static SCROLLABLE_ID: Lazy<scrollable::Id> = Lazy::new(scrollable::Id::unique);

pub fn main() -> iced::Result {
    EGUI::run(Settings {
        window: window::Settings {
            size: (1024, 768),
            ..window::Settings::default()
        },
        ..Settings::default()
    })
}

#[derive(Debug)]
enum EGUI {
    Loading,
    Loaded(State),
}

#[derive(Debug, Default)]
struct State {
    espanso_loc: String,
    selected_nav: String,
    selected_file: PathBuf,
    original_file: EspansoYaml,
    edited_file: EspansoYaml,
    match_files: Vec<String>,
    show_modal: bool,
    modal_title: String,
    modal_description: String,
    nav_queue: String,
}

impl State {
    fn new() -> Self {
        if valid_espanso_dir(get_default_espanso_dir()) {
            State {
                espanso_loc: get_default_espanso_dir(),
                selected_nav: "eg-Settings".to_string(),
                selected_file: PathBuf::new(),
                original_file: EspansoYaml::default(),
                edited_file: EspansoYaml::default(),
                match_files: {
                    let default_path = PathBuf::from(get_default_espanso_dir());
                    get_all_match_file_stems(default_path.join("match"))
                },
                show_modal: false,
                modal_title: String::new(),
                modal_description: String::new(),
                nav_queue: String::new(),
            }
        } else {
            State {
                espanso_loc: String::new(),
                selected_nav: "eg-Settings".to_string(),
                selected_file: PathBuf::new(),
                original_file: EspansoYaml::default(),
                edited_file: EspansoYaml::default(),
                match_files: Vec::new(),
                show_modal: false,
                modal_title: String::new(),
                modal_description: String::new(),
                nav_queue: String::new(),
            }
        }
    }
}

#[derive(Debug, Clone)]
enum Message {
    AddPairPressed,
    InputChanged(String),
    YamlInputChanged(String, usize, String),
    BrowsePressed,
    SettingsSavePressed,
    NavigateTo(String),
    ResetPressed,
    SaveFilePressed,
    ModalCancelPressed,
    ModalOkPressed,
    CloseModal,
    ShowModal(String, String, String),
    Loaded(Result<(), String>),
    FontLoaded(Result<(), font::Error>),
}

impl Application for EGUI {
    type Message = Message;
    type Executor = iced::executor::Default;
    type Flags = ();
    type Theme = Theme;

    fn new(_: Self::Flags) -> (Self, Command<Self::Message>) {
        // (EGUI::Loaded(State::new()), Command::none())
        (
            EGUI::Loading,
            Command::batch(vec![
                font::load(iced_aw::graphics::icons::ICON_FONT_BYTES).map(Message::FontLoaded),
                Command::perform(load(), Message::Loaded),
            ]),
        )
    }

    fn title(&self) -> String {
        String::from("espansoGUI")
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match self {
            EGUI::Loading => {
                if let Message::Loaded(_) = message {
                    *self = EGUI::Loaded(State::new())
                }
            }
            EGUI::Loaded(state) => match message {
                Message::ShowModal(title, description, destination) => {
                    state.modal_title = title;
                    state.modal_description = description;
                    state.nav_queue = destination;
                    state.show_modal = true;
                }
                Message::ModalOkPressed => {
                    state.show_modal = false;
                    if !state.nav_queue.is_empty() {
                        println!("Nav queue: {}", state.nav_queue);
                        let destination = state.nav_queue.clone();
                        let _ = self.update(Message::NavigateTo(destination));
                    }
                }
                Message::CloseModal => {
                    state.show_modal = false;
                }
                Message::ModalCancelPressed => {
                    state.show_modal = false;
                }
                Message::AddPairPressed => {
                    state.edited_file.matches.push(YamlPairs::default());
                    return scrollable::snap_to(
                        SCROLLABLE_ID.clone(),
                        scrollable::RelativeOffset::END,
                    );
                }
                Message::InputChanged(value) => {
                    state.espanso_loc = value;
                }
                Message::YamlInputChanged(new_str, i, trig_repl) => {
                    if trig_repl == "trigger" {
                        state.edited_file.matches.get_mut(i).unwrap().trigger = new_str;
                    } else {
                        state.edited_file.matches.get_mut(i).unwrap().replace = new_str;
                    }
                }
                Message::NavigateTo(value) => {
                    println!("Got to navigate.");
                    state.selected_nav = value.clone();
                    let espanso_loc = state.espanso_loc.clone();
                    match value.as_str() {
                        "eg-Preferences" => {
                            state.selected_file = if espanso_loc.ends_with("/") {
                                PathBuf::from(espanso_loc + "config/default.yml")
                            } else {
                                PathBuf::from(espanso_loc + "/config/default.yml")
                            };
                        }
                        "eg-Settings" => state.selected_file = PathBuf::new(),
                        _ => {
                            state.selected_file = if espanso_loc.ends_with("/") {
                                PathBuf::from(espanso_loc + "match/" + &state.selected_nav + ".yml")
                            } else {
                                PathBuf::from(
                                    espanso_loc + "/match/" + &state.selected_nav + ".yml",
                                )
                            };
                            state.original_file = read_to_triggers(state.selected_file.clone());
                            state.edited_file = state.original_file.clone();
                        }
                    }
                }
                Message::BrowsePressed => {
                    let default_path_mac: PathBuf = ["Library", "Application Support", "espanso"]
                        .iter()
                        .collect();
                    let mut default_espanso_path = PathBuf::new();
                    match home::home_dir() {
                        Some(path) => {
                            default_espanso_path = path;
                            default_espanso_path = default_espanso_path.join(default_path_mac);
                        }
                        None => println!("User directory not found"),
                    }
                    let selected_folder = FileDialog::new()
                        .set_directory(default_espanso_path)
                        .pick_folder();

                    if selected_folder.is_some() {
                        let espanso_dir = selected_folder.unwrap();
                        if valid_espanso_dir(espanso_dir.display().to_string()) {
                            state.espanso_loc = espanso_dir.into_os_string().into_string().unwrap();
                        } else {
                            // TODO: Show invalid directory
                        }
                    }
                }
                Message::SettingsSavePressed => {
                    if valid_espanso_dir(state.espanso_loc.clone()) {
                        state.match_files = get_all_match_file_stems(
                            PathBuf::from(state.espanso_loc.clone()).join("match"),
                        )
                    }
                }
                Message::ResetPressed => {
                    state.edited_file = state.original_file.clone();
                }
                Message::SaveFilePressed => {
                    write_from_triggers(state.selected_file.clone(), state.edited_file.clone());
                    state.original_file = state.edited_file.clone();
                }
                _ => {}
            },
        }
        Command::none()
    }

    fn view(&self) -> Element<Message> {
        let nav_col = column![
            text("Files").size(20),
            column![text("JA.yaml").size(16)].padding([0, 0, 0, 10]),
            text("Preferences").size(20),
            text("Settings").size(20)
        ]
        .spacing(12)
        .padding(20)
        .align_items(Alignment::Start);

        match self {
            EGUI::Loading => container(row![
                nav_col,
                column![
                    Space::new(Length::Fill, Length::Fill),
                    text("Loading...")
                        .horizontal_alignment(alignment::Horizontal::Center)
                        .size(50),
                    Space::new(Length::Fill, Length::Fill),
                ]
                .align_items(Alignment::Center)
                .width(Length::Fill)
                .height(Length::Fill),
            ])
            .width(Length::Fill)
            .height(Length::Fill)
            .into(),
            EGUI::Loaded(State {
                espanso_loc,
                selected_nav,
                original_file,
                edited_file,
                match_files,
                show_modal,
                modal_title,
                modal_description: modal_message,
                ..
            }) => {
                let unsaved_changes = edited_file.matches != original_file.matches;
                let mut nav_col = column![text("Files").size(20),]
                    .spacing(12)
                    .padding(20)
                    .align_items(Alignment::Start);
                let mut yml_files_col: Column<'_, Message, Renderer> =
                    Column::new().spacing(8).padding([0, 0, 0, 10]);
                for yml_file in match_files {
                    yml_files_col =
                        yml_files_col.push(nav_button(yml_file, yml_file, unsaved_changes));
                }
                nav_col = nav_col.push(yml_files_col);
                nav_col =
                    nav_col.push(nav_button("Preferences", "eg-Preferences", unsaved_changes));
                nav_col = nav_col.push(nav_button("Settings", "eg-Settings", unsaved_changes));

                let settings_col = column![
                    row![text("Settings").size(25)].padding([0, 0, 20, 0]),
                    column![
                        text("espanso is not running").size(20),
                        row![
                            text("Location").size(20),
                            Space::new(10, 0),
                            text_input("", espanso_loc)
                                .on_input(Message::InputChanged)
                                .size(20),
                            Space::new(10, 0),
                            button("Browse").on_press(Message::BrowsePressed),
                        ]
                        .align_items(Alignment::Center),
                    ]
                    .spacing(15)
                    .padding([0, 0, 0, 20]),
                    Space::new(Length::Fill, Length::Fill),
                    row![
                        Space::new(Length::Fill, 0),
                        button("Save").on_press(Message::SettingsSavePressed)
                    ],
                ]
                .padding([20, 20, 20, 40])
                .width(Length::Fill)
                .align_items(Alignment::Start);

                let mut all_trigger_replace_rows: Column<'_, Message, Renderer> =
                    Column::new().spacing(8).padding([0, 0, 0, 10]);
                if !selected_nav.is_empty() && selected_nav != "eg-Settings" {
                    all_trigger_replace_rows = all_trigger_replace_rows.push(
                        row![
                            button("+ Add").on_press(Message::AddPairPressed),
                            text(format!("Items: {}", original_file.matches.len())),
                            Space::new(Length::Fill, 0),
                            button("Reset").on_press_maybe(
                                match original_file.matches == edited_file.matches {
                                    true => None,
                                    false => Some(Message::ResetPressed),
                                }
                            ),
                            button("Save").on_press_maybe(
                                match original_file.matches == edited_file.matches {
                                    true => None,
                                    false => Some(Message::SaveFilePressed),
                                }
                            ),
                        ]
                        .align_items(Alignment::Center)
                        .spacing(10),
                    );

                    for i in 0..edited_file.matches.len() {
                        all_trigger_replace_rows = all_trigger_replace_rows.push(
                            Container::new(
                                row![column![
                                    row![
                                        text("Trigger:").size(20).width(90),
                                        text_input(
                                            &edited_file.matches[i].trigger,
                                            &edited_file.matches[i].trigger
                                        )
                                        .on_input(move |s| {
                                            Message::YamlInputChanged(s, i, "trigger".to_string())
                                        })
                                        .size(20)
                                    ],
                                    row![
                                        text("Replace:").size(20).width(75),
                                        text_input(
                                            &edited_file.matches[i].replace,
                                            &edited_file.matches[i].replace
                                        )
                                        .on_input(move |s| {
                                            Message::YamlInputChanged(s, i, "replace".to_string())
                                        })
                                        .size(20)
                                    ]
                                    .spacing(10)
                                    .align_items(Alignment::Center)
                                ]
                                .spacing(8)]
                                .spacing(10)
                                .padding(20),
                            )
                            .style(style::gray_background),
                        );
                    }
                }

                // let mut scrollable = Scrollable::new(scrollable_state);
                // scrollable.push(all_trigger_replace_rows.padding([20, 20, 20, 40]));
                // Scrollable::new(all_trigger_replace_rows.padding([20, 20, 20, 40]));
                // scrollable.scroll_to(scrollable::Scroll::End);
                let open_file_col =
                    column![
                        Scrollable::new(all_trigger_replace_rows.padding([20, 20, 20, 40]))
                            .id(SCROLLABLE_ID.clone())
                    ]
                    .width(Length::Fill)
                    .align_items(Alignment::Start);

                let main_row = row![
                    nav_col,
                    match selected_nav.as_str() {
                        "eg-Settings" => settings_col,
                        // "eg-Preferences" => preferences_col,
                        _ => open_file_col,
                    }
                ];

                let underlay = Container::new(main_row)
                    .width(iced::Length::Fill)
                    .height(iced::Length::Fill);

                let overlay: Option<Card<'_, Message, Renderer>> = if show_modal.clone() {
                    Some(
                        Card::new(text(modal_title), text(modal_message))
                            .foot(
                                row![
                                    button("Cancel")
                                        .width(Length::Fill)
                                        .on_press(Message::ModalCancelPressed),
                                    button("Ok")
                                        .width(Length::Fill)
                                        .on_press(Message::ModalOkPressed),
                                ]
                                .spacing(10)
                                .padding(5)
                                .width(Length::Fill),
                            )
                            .max_width(300.0)
                            .on_close(Message::CloseModal),
                    )
                } else {
                    None
                };

                modal(underlay, overlay)
                    .backdrop(Message::CloseModal)
                    .on_esc(Message::CloseModal)
                    .into()
            }
        }
    }
}

async fn load() -> Result<(), String> {
    Ok(())
}

fn read_to_triggers(file_path: PathBuf) -> EspansoYaml {
    let f = std::fs::File::open(file_path).expect("Could not open file.");
    from_reader(f).expect("Could not read values.")
}

fn write_from_triggers(file_path: PathBuf, edited_file: EspansoYaml) {
    let f = std::fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .create(true)
        .open(file_path)
        .expect("Couldn't open file");
    to_writer(f, &edited_file).unwrap();
    // println!("{:?}", edited_file);
}

fn get_default_espanso_dir() -> String {
    // TODO: Return to normal after testing
    return "/Users/ricky/Downloads/espanso".to_string();
    // Get result of 'espanso path' command if possible
    let espanso_path_cmd = p_cmd::new("espanso")
        .arg("path")
        .output()
        .expect("failed to get path from espanso");
    let espanso_path_cmd_output =
        String::from_utf8(espanso_path_cmd.stdout).expect("Couldn't get espanso path");
    let espanso_path_array: Vec<&str> = espanso_path_cmd_output.split("\n").collect();
    if !espanso_path_array.is_empty() {
        if !espanso_path_array[0].is_empty() {
            if espanso_path_array[0].starts_with("Config:") {
                return espanso_path_array[0][8..].to_string();
            }
        }
    }

    // If that was unsuccessful, get the default path
    if let Some(config_dir) = config_dir() {
        let default_path = config_dir.join("espanso");
        return default_path.display().to_string();
    }

    String::new()
}

fn valid_espanso_dir(selected_dir: String) -> bool {
    // Check if expected directories and files exist to verify it is valid
    let selected_dir: PathBuf = PathBuf::from(selected_dir);
    let config_dir = selected_dir.join("config");
    let match_dir = selected_dir.join("match");
    let config_exists: bool = config_dir.is_dir();
    let match_exists: bool = match_dir.is_dir();
    let config_yml_exists: bool = selected_dir.join("config/default.yml").is_file();
    if config_exists && match_exists && config_yml_exists {
        true
    } else {
        false
    }
}

fn get_all_match_file_stems(match_dir: PathBuf) -> Vec<String> {
    let mut match_file_stems = Vec::new();
    // Walk the directory and get all .yml file names
    for entry in WalkDir::new(match_dir)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.path().is_file() {
            if let Some(extension) = entry.path().extension() {
                if extension == "yml" {
                    match_file_stems.push(
                        entry
                            .path()
                            .file_stem()
                            .unwrap()
                            .to_string_lossy()
                            .into_owned(),
                    );
                }
            }
        }
    }

    match_file_stems
}

// TODO: Could remove 'a here and make nav_to a String
fn nav_button<'a>(
    text: &'a str,
    destination: &'a str,
    unsaved_changes: bool,
) -> Button<'a, Message> {
    button(text).on_press({
        if unsaved_changes {
            Message::ShowModal(
                "Unsaved Changes".to_string(),
                "Leaving this file with erase any unsaved changes.".to_string(),
                destination.to_string(),
            )
        } else {
            Message::NavigateTo(destination.to_string())
        }
    })
}

// fn navigate_after_modal(destination: String) -> impl Future<Output = Message> {
//     async move { Message::NavigateTo(destination) }
// }

mod style {
    use iced::widget::container;
    use iced::Theme;

    pub fn gray_background(theme: &Theme) -> container::Appearance {
        let palette = theme.extended_palette();

        container::Appearance {
            background: Some(palette.background.weak.color.into()),
            ..Default::default()
        }
    }
}
