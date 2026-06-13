# Zed-HexPeek
Zed-HexPeek, an extension for [Zed](https://zed.dev) editor to peek various forms of a number literal.

![zed-hexpeek](https://github.com/user-attachments/assets/e602ea53-487e-4bd7-9cb5-4d78b748f53b)


## Setup
1. Install [the extension](https://zed.dev/extensions/hexpeek) from the Zed extension marketplace
2. Enable the zed-hexpeek language server by adding the following to your `.zed/settings.json` (or `~/.config/zed/settings.json` for all workspaces)
```json
{
  "languages": {
    "C++": { // the language you want to enable the extension for
      "language_servers": ["hexpeek-language-server"]
    }
  }
}
```

## Known Issues
1. This extension provides hover capabilities through a bundled HexPeek language server. Since Zed's language server extension must declare the languages the language server supports.
```toml
[language_servers.hexpeek-language-server]
name = "HexPeek Language Server"
languages = ["Astro", "C", "C#", "C++", "CSS", "Clojure", "Coffeescript", "Dart", "Diff", "ERB", "Elixir", "Erlang", "F#", "GLSL", "Git Commit", "Gleam", "Go Mod", "Go Work", "Go", "Groovy", "HEEX", "HTML", "JSDoc", "JSON", "JSONC", "Java", "JavaScript", "Lua", "Makefile", "Markdown", "Markdown-Inline", "Objective-C", "Objective-C++", "PHP", "Perl", "Plain Text", "Proto", "Python", "R", "Regex", "Ruby", "Rust", "SQL", "Scala", "Shell Script", "Svelte", "Swift", "TSX", "TypeScript", "XML", "YAML", "Zed Keybind Context"]
```
And this extension supports the above languages. If you are using a language which is not one of them, please add your language name manually.

## License
Apache 2.0
