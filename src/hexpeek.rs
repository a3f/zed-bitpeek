use zed_extension_api::{self as zed};

const SERVER_SCRIPT: &'static str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/language_server/dist/server.cjs"
);

struct HexPeekExtension;

impl zed::Extension for HexPeekExtension {
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
}

zed::register_extension!(HexPeekExtension);
