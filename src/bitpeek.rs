use zed_extension_api::{self as zed};

const SERVER_SCRIPT: &'static str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/language_server/dist/server.cjs"
);

struct BitPeekExtension;

impl zed::Extension for BitPeekExtension {
    fn new() -> Self {
        Self {}
    }

    fn language_server_command(
        &mut self,
        _language_server_id: &zed::LanguageServerId,
        _worktree: &zed::Worktree,
    ) -> zed::Result<zed::Command> {
        Ok(zed::Command {
            command: zed::node_binary_path()?,
            args: vec![SERVER_SCRIPT.into(), "--stdio".into()],
            env: Default::default(),
        })
    }

    fn language_server_initialization_options(
        &mut self,
        language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> zed::Result<Option<zed::serde_json::Value>> {
        Ok(zed::settings::LspSettings::for_worktree(language_server_id.as_ref(), worktree)?
            .initialization_options)
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
