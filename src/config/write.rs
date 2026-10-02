#[derive(Clone, Copy)]
pub(crate) enum ConfigEdit<'a> {
    Theme(&'a str),
    StatusIndicators(super::StatusIndicatorStyle),
    Sound(bool),
    ToastDelivery(super::ToastDelivery),
    SpacesMode(super::SpacesSidebarMode),
    SidebarPosition(super::SidebarPositionConfig),
}

impl ConfigEdit<'_> {
    pub(crate) fn description(self) -> &'static str {
        match self {
            Self::Theme(_) => "theme",
            Self::StatusIndicators(_) => "status indicators",
            Self::Sound(_) => "sound setting",
            Self::ToastDelivery(_) => "toast setting",
            Self::SpacesMode(_) | Self::SidebarPosition(_) => "sidebar setting",
        }
    }

    pub(crate) fn apply(self, content: &str) -> String {
        match self {
            Self::Theme(name) => {
                let content =
                    super::upsert_section_value(content, "theme", "name", &format!("\"{name}\""));
                super::upsert_section_bool(&content, "theme", "auto_switch", false)
            }
            Self::StatusIndicators(style) => super::upsert_section_value(
                content,
                "ui",
                "status_indicators",
                &format!("\"{}\"", style.as_str()),
            ),
            Self::Sound(enabled) => {
                super::upsert_section_bool(content, "ui.sound", "enabled", enabled)
            }
            Self::ToastDelivery(delivery) => {
                let value = match delivery {
                    super::ToastDelivery::Off => "\"off\"",
                    super::ToastDelivery::Herdr => "\"herdr\"",
                    super::ToastDelivery::Terminal => "\"terminal\"",
                    super::ToastDelivery::System => "\"system\"",
                };
                let content = super::upsert_section_value(content, "ui.toast", "delivery", value);
                super::remove_section_key(&content, "ui.toast", "enabled")
            }
            Self::SpacesMode(mode) => super::upsert_section_value(
                content,
                "ui.sidebar.spaces",
                "mode",
                &format!("\"{}\"", mode.as_str()),
            ),
            Self::SidebarPosition(position) => super::upsert_section_value(
                content,
                "ui",
                "sidebar_position",
                &format!("\"{}\"", position.as_str()),
            ),
        }
    }
}

pub(crate) fn update_file_at(
    path: &std::path::Path,
    description: &str,
    update: impl FnOnce(&str) -> String,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create config directory: {error}"))?;
    }
    let content = match super::io::read_optional_config(path) {
        Ok(Some(content)) => content,
        Ok(None) => String::new(),
        Err(error) => {
            return Err(format!(
                "failed to read config before saving {description}: {error}"
            ));
        }
    };
    std::fs::write(path, update(&content))
        .map_err(|error| format!("failed to save {description}: {error}"))
}

pub(crate) fn write_edit(edit: ConfigEdit<'_>) -> Result<(), String> {
    update_file_at(&super::config_path(), edit.description(), |content| {
        edit.apply(content)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_file_at_does_not_move_a_leading_bom_into_the_file() {
        let dir = std::env::temp_dir().join(format!("herdr-config-bom-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        std::fs::write(
            &path,
            b"\xEF\xBB\xBF[terminal]\ndefault_shell = \"pwsh.exe\"\n",
        )
        .unwrap();

        update_file_at(&path, "onboarding setting", |content| {
            crate::config::upsert_top_level_bool(content, "onboarding", false)
        })
        .unwrap();

        let written = std::fs::read_to_string(&path).unwrap();
        let _ = std::fs::remove_dir_all(dir);

        assert!(
            !written.contains('\u{feff}'),
            "unexpected BOM in {written:?}"
        );
        assert!(
            toml::from_str::<toml::Value>(&written).is_ok(),
            "written config is not valid TOML: {written:?}"
        );
    }

    fn spaces_mode_after(content: &str, mode: super::super::SpacesSidebarMode) -> String {
        let written = ConfigEdit::SpacesMode(mode).apply(content);
        let parsed: super::super::Config = toml::from_str(&written)
            .unwrap_or_else(|error| panic!("{error}\n--- written ---\n{written}"));
        assert_eq!(parsed.ui.sidebar.spaces.mode, mode, "{written}");
        written
    }

    #[test]
    fn spaces_mode_edit_round_trips_through_the_config_parser() {
        use super::super::SpacesSidebarMode::{Hidden, Shown};

        // No sidebar table at all.
        let written = spaces_mode_after("[ui]\nsidebar_width = 30\n", Hidden);
        assert!(written.contains("sidebar_width = 30"), "{written}");

        // An existing Spaces table keeps its rows; a second edit replaces
        // the value instead of adding another key.
        let rows = "[ui.sidebar.spaces]\nrows = [[\"state_icon\", \"workspace\"]]\nrow_gap = 1\n";
        let hidden = spaces_mode_after(rows, Hidden);
        let shown = spaces_mode_after(&hidden, Shown);
        assert_eq!(shown.matches("mode =").count(), 1, "{shown}");
        let parsed: super::super::Config = toml::from_str(&shown).unwrap();
        assert_eq!(parsed.ui.sidebar.spaces.row_gap, 1);
        assert_eq!(parsed.ui.sidebar.spaces.rows.len(), 1);

        // A plugin-managed agents block, as sidebar plugins write it.
        let plugin = concat!(
            "[ui]\n\n",
            "# >>> plugin sidebar block\n",
            "[ui.sidebar.agents]\n",
            "rows = [[{ token = \"$logo\", fg = \"#e9e9f0\" }, \"agent\"]]\n\n",
            "[ui.sidebar.agents.rows_by_agent]\n",
            "claude = [[\"agent\"]]\n",
            "# <<< plugin sidebar block\n",
        );
        let written = spaces_mode_after(plugin, Hidden);
        assert!(written.contains("# <<< plugin sidebar block"), "{written}");
        let parsed: super::super::Config = toml::from_str(&written).unwrap();
        assert_eq!(parsed.ui.sidebar.agents.rows_by_agent.len(), 1);
    }

    #[test]
    fn sidebar_position_edit_round_trips_through_the_config_parser() {
        use super::super::SidebarPositionConfig::{Left, Right};

        let position_after = |content: &str, position| {
            let written = ConfigEdit::SidebarPosition(position).apply(content);
            let parsed: super::super::Config = toml::from_str(&written)
                .unwrap_or_else(|error| panic!("{error}\n--- written ---\n{written}"));
            assert_eq!(parsed.ui.sidebar_position, position, "{written}");
            written
        };

        position_after("", Right);
        let plugin_tagged =
            "[ui]\nagent_panel_sort = \"spaces\" # plugin\n\n[ui.toast]\ndelivery = \"herdr\"\n";
        let right = position_after(plugin_tagged, Right);
        assert!(right.contains("# plugin"), "{right}");
        let left = position_after(&right, Left);
        assert_eq!(left.matches("sidebar_position").count(), 1, "{left}");
    }
}
