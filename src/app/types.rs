#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Screen {
    Login,
    Transition,
    Dashboard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum View {
    Home,
    Settings,
    Channels,
    TwitchLookup,
    Templates,
    AiAlerts,
}
