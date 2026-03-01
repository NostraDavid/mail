use iced::{
    Element, Task,
    widget::{button, column, container, row, scrollable, text, text_input},
};
use mail_engine::{
    DEFAULT_GOOGLE_CLIENT_ID, Engine, LoginResult, MailMessage, Provider, ProviderCredentials,
    SavedOAuthSettings,
};
use tracing_subscriber::EnvFilter;

fn main() -> iced::Result {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(false)
        .compact()
        .try_init()
        .ok();

    iced::application("mail", update, view).run_with(|| {
        let mut state = MailApp::default();
        state.google_client_id = DEFAULT_GOOGLE_CLIENT_ID.to_owned();

        (
            state,
            Task::batch(vec![
                Task::perform(load_saved_settings(), Message::SettingsLoaded),
                Task::perform(restore_google_session(), Message::RestoreSessionDone),
            ]),
        )
    })
}

#[derive(Debug, Clone)]
enum Message {
    SettingsLoaded(Result<SavedOAuthSettings, String>),
    ToggleGoogleSetup,
    SelectFolder(MailFolder),
    SelectMessage(usize),
    GoogleClientIdChanged(String),
    GoogleClientSecretChanged(String),
    SaveGoogleSettings,
    SaveDone(Result<String, String>),
    LoginGoogle,
    LoginDone(Result<LoginResult, String>),
    RestoreSessionDone(Result<Option<LoginResult>, String>),
}

#[derive(Debug, Default)]
enum UiState {
    #[default]
    Idle,
    Working(String),
    Loaded,
    Error(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum MailFolder {
    #[default]
    Inbox,
    Starred,
    Sent,
    Drafts,
    Spam,
    Trash,
}

impl MailFolder {
    fn label(self) -> &'static str {
        match self {
            MailFolder::Inbox => "Inbox",
            MailFolder::Starred => "Starred",
            MailFolder::Sent => "Sent",
            MailFolder::Drafts => "Drafts",
            MailFolder::Spam => "Spam",
            MailFolder::Trash => "Trash",
        }
    }

    fn all() -> &'static [MailFolder] {
        const FOLDERS: [MailFolder; 6] = [
            MailFolder::Inbox,
            MailFolder::Starred,
            MailFolder::Sent,
            MailFolder::Drafts,
            MailFolder::Spam,
            MailFolder::Trash,
        ];
        &FOLDERS
    }
}

#[derive(Debug, Default)]
struct MailApp {
    state: UiState,
    status_note: Option<String>,
    account_label: Option<String>,
    messages: Vec<MailMessage>,
    selected_folder: MailFolder,
    selected_message: Option<usize>,
    show_google_setup: bool,
    google_client_id: String,
    google_client_secret: String,
}

fn update(state: &mut MailApp, message: Message) -> Task<Message> {
    match message {
        Message::SettingsLoaded(Ok(settings)) => {
            if let Some(google) = settings.google {
                state.google_client_id = google.client_id;
                state.google_client_secret = google.client_secret.unwrap_or_default();
            }
            if state.status_note.is_none() {
                state.status_note = Some("Lokale OAuth-instellingen geladen.".to_owned());
            }
            Task::none()
        }
        Message::SettingsLoaded(Err(error)) => {
            state.state = UiState::Error(error);
            Task::none()
        }
        Message::ToggleGoogleSetup => {
            state.show_google_setup = !state.show_google_setup;
            Task::none()
        }
        Message::SelectFolder(folder) => {
            state.selected_folder = folder;
            if folder == MailFolder::Inbox {
                state.selected_message = state.first_message_index();
            } else {
                state.selected_message = None;
            }
            Task::none()
        }
        Message::SelectMessage(index) => {
            if index < state.messages.len() {
                state.selected_message = Some(index);
            }
            Task::none()
        }
        Message::GoogleClientIdChanged(value) => {
            state.google_client_id = value;
            Task::none()
        }
        Message::GoogleClientSecretChanged(value) => {
            state.google_client_secret = value;
            Task::none()
        }
        Message::SaveGoogleSettings => {
            state.state = UiState::Working("Google-instellingen opslaan...".to_owned());
            Task::perform(
                save_settings(
                    Provider::Google,
                    state.google_client_id.clone(),
                    state.google_client_secret.clone(),
                ),
                Message::SaveDone,
            )
        }
        Message::SaveDone(Ok(status)) => {
            state.state = UiState::Idle;
            state.status_note = Some(status);
            Task::none()
        }
        Message::SaveDone(Err(error)) => {
            state.state = UiState::Error(error);
            Task::none()
        }
        Message::LoginGoogle => {
            state.state = UiState::Working("Login met Google...".to_owned());
            Task::perform(
                login_and_fetch(
                    Provider::Google,
                    state.google_client_id.clone(),
                    state.google_client_secret.clone(),
                ),
                Message::LoginDone,
            )
        }
        Message::LoginDone(Ok(result)) => {
            state.state = UiState::Loaded;
            state.status_note = Some("Inbox opgehaald.".to_owned());
            state.account_label = Some(format!("{}: {}", result.provider.label(), result.account));
            state.messages = result.messages;
            state.selected_folder = MailFolder::Inbox;
            state.selected_message = state.first_message_index();
            Task::none()
        }
        Message::LoginDone(Err(error)) => {
            state.state = UiState::Error(error);
            Task::none()
        }
        Message::RestoreSessionDone(Ok(Some(result))) => {
            state.state = UiState::Loaded;
            state.status_note = Some("Sessie hersteld.".to_owned());
            state.account_label = Some(format!("{}: {}", result.provider.label(), result.account));
            state.messages = result.messages;
            state.selected_folder = MailFolder::Inbox;
            state.selected_message = state.first_message_index();
            Task::none()
        }
        Message::RestoreSessionDone(Ok(None)) => Task::none(),
        Message::RestoreSessionDone(Err(error)) => {
            state.status_note = Some(format!(
                "Sessie kon niet automatisch worden hersteld: {error}"
            ));
            Task::none()
        }
    }
}

fn view(state: &MailApp) -> Element<'_, Message> {
    let is_working = matches!(state.state, UiState::Working(_));

    let status_line = match &state.state {
        UiState::Idle => "Klaar om in te loggen".to_owned(),
        UiState::Working(text) => text.clone(),
        UiState::Loaded => "Klaar".to_owned(),
        UiState::Error(error) => format!("Fout: {error}"),
    };

    let mut google_save_btn = button("Opslaan");
    let mut google_login_btn = button("Login met Google").style(iced::widget::button::primary);
    let mut google_toggle_btn = if state.show_google_setup {
        button("Google instellingen verbergen")
    } else {
        button("Google instellingen")
    };

    if !is_working {
        google_toggle_btn = google_toggle_btn.on_press(Message::ToggleGoogleSetup);
        google_login_btn = google_login_btn.on_press(Message::LoginGoogle);
        if !state.google_client_id.trim().is_empty() {
            google_save_btn = google_save_btn.on_press(Message::SaveGoogleSettings);
        }
    }

    let mut header = column![
        text("mail"),
        text(status_line),
        row![google_login_btn, google_toggle_btn].spacing(10),
    ]
    .spacing(8);

    if let Some(note) = &state.status_note {
        header = header.push(text(note));
    }
    if let Some(account) = &state.account_label {
        header = header.push(
            container(text(format!("Ingelogd als {account}")))
                .padding(8)
                .style(iced::widget::container::rounded_box),
        );
    }

    let mut content = column![header].spacing(12).padding(12);

    if state.show_google_setup {
        content = content.push(
            container(
                column![
                    text("Google OAuth instellingen"),
                    text_input("Google Client ID", &state.google_client_id)
                        .on_input(Message::GoogleClientIdChanged),
                    text_input("Google Client Secret", &state.google_client_secret)
                        .on_input(Message::GoogleClientSecretChanged),
                    google_save_btn,
                ]
                .spacing(8),
            )
            .padding(10)
            .style(iced::widget::container::rounded_box),
        );
    }

    let folder_pane = folder_pane(state, is_working);
    let list_pane = message_list_pane(state, is_working);
    let detail_pane = message_detail_pane(state);

    content = content.push(
        row![folder_pane, list_pane, detail_pane]
            .spacing(10)
            .height(iced::Fill),
    );

    container(content).width(iced::Fill).height(iced::Fill).into()
}

fn folder_pane(state: &MailApp, is_working: bool) -> Element<'_, Message> {
    let mut content = column![text("Mailboxen")].spacing(6);

    for folder in MailFolder::all() {
        let is_selected = *folder == state.selected_folder;
        let mut item = button(folder.label());
        item = if is_selected {
            item.style(iced::widget::button::primary)
        } else {
            item.style(iced::widget::button::secondary)
        };
        if !is_working {
            item = item.on_press(Message::SelectFolder(*folder));
        }
        content = content.push(item);
    }

    container(scrollable(content))
        .padding(10)
        .style(iced::widget::container::rounded_box)
        .width(iced::Length::FillPortion(1))
        .height(iced::Fill)
        .into()
}

fn message_list_pane(state: &MailApp, is_working: bool) -> Element<'_, Message> {
    let mut content = column![text(format!("{} berichten", state.selected_folder.label()))].spacing(6);

    if state.selected_folder != MailFolder::Inbox {
        content = content.push(text("Deze map is nog niet gekoppeld."));
    } else if state.messages.is_empty() {
        content = content.push(text("Nog geen berichten geladen."));
    } else {
        for (index, item) in state.messages.iter().enumerate() {
            let is_selected = Some(index) == state.selected_message;
            let mut row_btn = button(
                column![
                    text(&item.subject),
                    text(format!("{} | {}", item.from, item.date)).size(13),
                ]
                .spacing(3),
            );
            row_btn = if is_selected {
                row_btn.style(iced::widget::button::primary)
            } else {
                row_btn.style(iced::widget::button::secondary)
            };
            if !is_working {
                row_btn = row_btn.on_press(Message::SelectMessage(index));
            }
            content = content.push(row_btn.width(iced::Fill));
        }
    }

    container(scrollable(content))
        .padding(10)
        .style(iced::widget::container::rounded_box)
        .width(iced::Length::FillPortion(2))
        .height(iced::Fill)
        .into()
}

fn message_detail_pane(state: &MailApp) -> Element<'_, Message> {
    let content = if let Some(message) = state.selected_mail_message() {
        column![
            text(&message.subject).size(24),
            text(format!("Van: {}", message.from)),
            text(format!("Datum: {}", message.date)),
            text(""),
            text(&message.body),
        ]
        .spacing(8)
    } else if state.selected_folder == MailFolder::Inbox {
        column![text("Selecteer een email om te lezen.")]
    } else {
        column![text("Selecteer Inbox om berichten te lezen.")]
    };

    container(scrollable(content))
        .padding(12)
        .style(iced::widget::container::rounded_box)
        .width(iced::Length::FillPortion(4))
        .height(iced::Fill)
        .into()
}

impl MailApp {
    fn first_message_index(&self) -> Option<usize> {
        if self.messages.is_empty() {
            None
        } else {
            Some(0)
        }
    }

    fn selected_mail_message(&self) -> Option<&MailMessage> {
        if self.selected_folder != MailFolder::Inbox {
            return None;
        }
        self.selected_message
            .and_then(|index| self.messages.get(index))
    }
}

async fn load_saved_settings() -> Result<SavedOAuthSettings, String> {
    let engine = Engine::new("mail");
    engine
        .load_oauth_settings()
        .await
        .map_err(|error| format!("{error:#}"))
}

async fn save_settings(
    provider: Provider,
    client_id: String,
    client_secret: String,
) -> Result<String, String> {
    let engine = Engine::new("mail");
    let credentials = ProviderCredentials {
        client_id,
        client_secret: normalize_secret(client_secret),
    };

    engine
        .save_provider_credentials(provider, credentials)
        .await
        .map_err(|error| format!("{error:#}"))?;

    Ok(format!("{}-instellingen opgeslagen.", provider.label()))
}

async fn login_and_fetch(
    provider: Provider,
    client_id: String,
    client_secret: String,
) -> Result<LoginResult, String> {
    let engine = Engine::new("mail");
    let client_id = client_id.trim().to_owned();

    if !client_id.is_empty() {
        engine
            .save_provider_credentials(
                provider,
                ProviderCredentials {
                    client_id,
                    client_secret: normalize_secret(client_secret),
                },
            )
            .await
            .map_err(|error| format!("{error:#}"))?;
    }

    engine
        .login_and_fetch(provider)
        .await
        .map_err(|error| format!("{error:#}"))
}

async fn restore_google_session() -> Result<Option<LoginResult>, String> {
    let engine = Engine::new("mail");
    engine
        .try_restore_session(Provider::Google)
        .await
        .map_err(|error| format!("{error:#}"))
}

fn normalize_secret(secret: String) -> Option<String> {
    let trimmed = secret.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}
