use iced::{
    Element, Task,
    widget::{button, column, container, text},
};
use mail_engine::Engine;
use tracing_subscriber::EnvFilter;

fn main() -> iced::Result {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(false)
        .compact()
        .try_init()
        .ok();

    iced::application("mail", update, view).run()
}

#[derive(Debug, Clone)]
enum Message {
    StartEngine,
    EngineStarted(Result<(), String>),
}

#[derive(Debug, Default)]
enum EngineState {
    #[default]
    Idle,
    Starting,
    Running,
    Error(String),
}

#[derive(Debug, Default)]
struct MailApp {
    engine_state: EngineState,
}

fn update(state: &mut MailApp, message: Message) -> Task<Message> {
    match message {
        Message::StartEngine => {
            state.engine_state = EngineState::Starting;
            Task::perform(start_engine(), Message::EngineStarted)
        }
        Message::EngineStarted(Ok(())) => {
            state.engine_state = EngineState::Running;
            Task::none()
        }
        Message::EngineStarted(Err(error)) => {
            state.engine_state = EngineState::Error(error);
            Task::none()
        }
    }
}

fn view(state: &MailApp) -> Element<'_, Message> {
    let status = match &state.engine_state {
        EngineState::Idle => "Engine status: idle".to_owned(),
        EngineState::Starting => "Engine status: starting...".to_owned(),
        EngineState::Running => "Engine status: running".to_owned(),
        EngineState::Error(error) => format!("Engine status: error ({error})"),
    };

    let mut start_button = button("Start engine");
    if !matches!(state.engine_state, EngineState::Starting) {
        start_button = start_button.on_press(Message::StartEngine);
    }

    container(
        column![text("mail"), text(status), start_button]
            .spacing(16)
            .padding(24),
    )
    .center_x(iced::Fill)
    .center_y(iced::Fill)
    .into()
}

async fn start_engine() -> Result<(), String> {
    let engine = Engine::new("mail");
    engine.start().await.map_err(|error| error.to_string())
}
