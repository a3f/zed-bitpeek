use zed_extension_api::{self as zed, LanguageServerId, Result, settings::LspSettings};

struct BitPeekExtension;

impl zed::Extension for BitPeekExtension {
    fn new() -> Self {
        Self {}
    }

    fn language_server_command(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        let mut command = worktree.which(language_server_id.as_ref());

        if let Ok(lsp_settings) = LspSettings::for_worktree(language_server_id.as_ref(), worktree) {
            if let Some(command_settings) = lsp_settings.binary {
                if command_settings.path.is_some() {
                    command = command_settings.path;
                }
            }
        };

        Ok(zed::Command {
            args: vec![],
            command: command
                .ok_or_else(|| format!("{} not found!", language_server_id.as_ref()))?,
            env: Default::default(),
        })
    }

    fn language_server_initialization_options(
        &mut self,
        language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> zed::Result<Option<zed::serde_json::Value>> {
        Ok(
            zed::settings::LspSettings::for_worktree(language_server_id.as_ref(), worktree)?
                .initialization_options,
        )
    }

    fn language_server_workspace_configuration(
        &mut self,
        language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> zed::Result<Option<zed::serde_json::Value>> {
        Ok(
            zed::settings::LspSettings::for_worktree(language_server_id.as_ref(), worktree)?
                .settings,
        )
    }
}

zed::register_extension!(BitPeekExtension);
