// Derived from herdr src/ui/sidebar/tokens.rs (Apache-2.0).

use crate::config::{
    AgentSidebarToken, AgentsSidebarConfig, SidebarLabelToken, SidebarTokenLayout,
    SidebarTokenStyle, SpaceSidebarToken, SpacesSidebarConfig,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedToken {
    pub kind: ResolvedTokenKind,
    pub style: SidebarTokenStyle,
    /// The text drawn before this token, overriding the default.
    pub separator: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ResolvedTokenKind {
    StateIcon,
    StateText(String),
    Machine(String),
    Workspace(String),
    Tab(String),
    Pane(String),
    Agent(String),
    TerminalTitle(String),
    Branch(String),
    GitStatus { ahead: usize, behind: usize },
    AgentCount(String),
    Label(String),
    Custom(String),
}

impl ResolvedTokenKind {
    /// The same kind with its text replaced (`state_icon` and
    /// `git_status` carry none and stay as they are).
    fn with_text(self, text: String) -> Self {
        match self {
            Self::StateText(_) => Self::StateText(text),
            Self::Machine(_) => Self::Machine(text),
            Self::Workspace(_) => Self::Workspace(text),
            Self::Tab(_) => Self::Tab(text),
            Self::Pane(_) => Self::Pane(text),
            Self::Agent(_) => Self::Agent(text),
            Self::TerminalTitle(_) => Self::TerminalTitle(text),
            Self::Branch(_) => Self::Branch(text),
            Self::AgentCount(_) => Self::AgentCount(text),
            Self::Label(_) => Self::Label(text),
            Self::Custom(_) => Self::Custom(text),
            other => other,
        }
    }

    fn text_value(&self) -> Option<&str> {
        match self {
            Self::StateText(value)
            | Self::Machine(value)
            | Self::Workspace(value)
            | Self::Tab(value)
            | Self::Pane(value)
            | Self::Agent(value)
            | Self::TerminalTitle(value)
            | Self::Branch(value)
            | Self::AgentCount(value)
            | Self::Label(value)
            | Self::Custom(value) => Some(value),
            Self::StateIcon | Self::GitStatus { .. } => None,
        }
    }
}

impl ResolvedToken {
    fn new(kind: ResolvedTokenKind, style: SidebarTokenStyle) -> Self {
        Self {
            kind,
            style,
            separator: None,
        }
    }

    /// Apply a styled token's `format` (after the rules matched the raw
    /// value) and `separator`.
    fn laid_out(mut self, layout: Option<&SidebarTokenLayout>) -> Self {
        let Some(layout) = layout else {
            return self;
        };
        if layout.format.is_some()
            && let Some(value) = self.kind.text_value().map(str::to_owned)
        {
            self.kind = self.kind.with_text(layout.apply(&value));
        }
        self.separator = layout.separator.clone();
        self
    }
}

pub(crate) struct AgentTokenContext<'a> {
    pub(crate) machine: Option<&'a str>,
    pub(crate) workspace: &'a str,
    pub(crate) tab: Option<&'a str>,
    pub(crate) pane: Option<&'a str>,
    pub(crate) agent_label: Option<&'a str>,
    pub(crate) terminal_title: Option<&'a str>,
    pub(crate) terminal_title_stripped: Option<&'a str>,
    pub(crate) canonical_agent: Option<crate::chrome::detect::Agent>,
    pub(crate) tokens: &'a std::collections::HashMap<String, String>,
    /// The labels of the agent's terminal.
    pub(crate) labels: &'a [(String, String)],
}

/// A `label` token's shown text and the raw value its rules match.
fn label_kind(label: &SidebarLabelToken, values: &[&str]) -> Option<(ResolvedTokenKind, String)> {
    let raw = label.pick(values)?;
    let text = label.display(raw)?;
    Some((ResolvedTokenKind::Label(text), raw.to_owned()))
}

fn label_values<'a>(labels: &[&'a [(String, String)]], name: &str) -> Vec<&'a str> {
    labels
        .iter()
        .filter_map(|terminal| {
            terminal
                .iter()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.as_str())
        })
        .collect()
}

pub(crate) fn agent_rows(
    config: &AgentsSidebarConfig,
    context: AgentTokenContext<'_>,
    state_text: &str,
) -> Vec<Vec<ResolvedToken>> {
    config
        .rows_for_agent(context.canonical_agent)
        .iter()
        .filter_map(|row| {
            let resolved = row
                .iter()
                .filter_map(|configured| {
                    let (token, style) = configured.parts();
                    let kind = match token {
                        AgentSidebarToken::StateIcon => Some(ResolvedTokenKind::StateIcon),
                        AgentSidebarToken::StateText => {
                            Some(ResolvedTokenKind::StateText(state_text.to_string()))
                        }
                        AgentSidebarToken::Machine => context
                            .machine
                            .map(|value| ResolvedTokenKind::Machine(value.to_string())),
                        AgentSidebarToken::Workspace => {
                            Some(ResolvedTokenKind::Workspace(context.workspace.to_string()))
                        }
                        AgentSidebarToken::Tab => context
                            .tab
                            .map(|value| ResolvedTokenKind::Tab(value.to_string())),
                        AgentSidebarToken::Pane => context
                            .pane
                            .map(|value| ResolvedTokenKind::Pane(value.to_string())),
                        AgentSidebarToken::Agent => context
                            .agent_label
                            .map(|value| ResolvedTokenKind::Agent(value.to_string())),
                        AgentSidebarToken::TerminalTitle => context
                            .terminal_title
                            .map(|value| ResolvedTokenKind::TerminalTitle(value.to_string())),
                        AgentSidebarToken::TerminalTitleStripped => context
                            .terminal_title_stripped
                            .map(|value| ResolvedTokenKind::TerminalTitle(value.to_string())),
                        AgentSidebarToken::Label(label) => {
                            let (kind, raw) =
                                label_kind(label, &label_values(&[context.labels], &label.name))?;
                            let style = configured.style_for_value(&raw)?;
                            return Some(
                                ResolvedToken::new(kind, style).laid_out(configured.layout()),
                            );
                        }
                        AgentSidebarToken::Custom(name) => context
                            .tokens
                            .get(name)
                            .cloned()
                            .map(ResolvedTokenKind::Custom),
                        AgentSidebarToken::Styled { .. } => None,
                    }?;
                    let style = kind
                        .text_value()
                        .map_or(Some(style), |value| configured.style_for_value(value))?;
                    Some(ResolvedToken::new(kind, style).laid_out(configured.layout()))
                })
                .collect::<Vec<_>>();
            (!resolved.is_empty()).then_some(resolved)
        })
        .collect()
}

pub(crate) struct SpaceTokenContext<'a> {
    pub(crate) workspace: &'a str,
    pub(crate) branch: Option<&'a str>,
    pub(crate) state_text: &'a str,
    pub(crate) ahead_behind: Option<(usize, usize)>,
    /// Every agent of the space, idle included.
    pub(crate) agent_count: usize,
    /// The labels of each terminal of the space, in display order.
    pub(crate) labels: Vec<&'a [(String, String)]>,
    pub(crate) tokens: &'a std::collections::HashMap<String, String>,
    pub(crate) suppress_git_details: bool,
}

pub(crate) fn space_rows(
    config: &SpacesSidebarConfig,
    context: SpaceTokenContext<'_>,
) -> Vec<Vec<ResolvedToken>> {
    config
        .rows
        .iter()
        .filter_map(|row| {
            let resolved = row
                .iter()
                .filter_map(|configured| {
                    let (token, style) = configured.parts();
                    let kind = match token {
                        SpaceSidebarToken::StateIcon => Some(ResolvedTokenKind::StateIcon),
                        SpaceSidebarToken::StateText => {
                            Some(ResolvedTokenKind::StateText(context.state_text.to_string()))
                        }
                        SpaceSidebarToken::Workspace => {
                            Some(ResolvedTokenKind::Workspace(context.workspace.to_string()))
                        }
                        SpaceSidebarToken::Branch if !context.suppress_git_details => context
                            .branch
                            .map(|branch| ResolvedTokenKind::Branch(branch.to_string())),
                        SpaceSidebarToken::Branch => None,
                        SpaceSidebarToken::GitStatus if !context.suppress_git_details => context
                            .ahead_behind
                            .filter(|(ahead, behind)| *ahead > 0 || *behind > 0)
                            .map(|(ahead, behind)| ResolvedTokenKind::GitStatus { ahead, behind }),
                        SpaceSidebarToken::GitStatus => None,
                        SpaceSidebarToken::AgentCount => (context.agent_count > 0).then(|| {
                            ResolvedTokenKind::AgentCount(context.agent_count.to_string())
                        }),
                        SpaceSidebarToken::Label(label) => {
                            let (kind, raw) =
                                label_kind(label, &label_values(&context.labels, &label.name))?;
                            let style = configured.style_for_value(&raw)?;
                            return Some(
                                ResolvedToken::new(kind, style).laid_out(configured.layout()),
                            );
                        }
                        SpaceSidebarToken::Custom(name) => context
                            .tokens
                            .get(name)
                            .cloned()
                            .map(ResolvedTokenKind::Custom),
                        SpaceSidebarToken::Styled { .. } => None,
                    }?;
                    let style = kind
                        .text_value()
                        .map_or(Some(style), |value| configured.style_for_value(value))?;
                    Some(ResolvedToken::new(kind, style).laid_out(configured.layout()))
                })
                .collect::<Vec<_>>();
            (!resolved.is_empty()).then_some(resolved)
        })
        .collect()
}

pub(crate) fn separator<'a>(previous: &ResolvedToken, current: &'a ResolvedToken) -> &'a str {
    if let Some(separator) = &current.separator {
        return separator;
    }
    if matches!(previous.kind, ResolvedTokenKind::StateIcon)
        || matches!(current.kind, ResolvedTokenKind::GitStatus { .. })
    {
        " "
    } else {
        " · "
    }
}
