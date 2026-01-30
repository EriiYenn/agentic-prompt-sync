# Hey Weston 👋

> [!NOTE]
> First, thank you for building and maintaining this repository — it’s been genuinely useful for distributing rules and skills across multiple projects based on different needs.
> Second, I am not a Rust expert, so please bear with me if I made any mistakes. I tried to follow the existing code style and conventions.

While using the tool, I found a couple of areas where additional functionality would be extremely valuable:

## 🪝 Hooks (Cursor, Claude, ...)

The Hooks documentations for the designated IDEs enforce the scripts running the Hooks to be marked as executable with `chmod +x`.

I extended the already existing pipeline to support this functionality with new **Asset Types**:

- `cursor_hooks`
- `claude_hooks`

**Implementation Details:**

- Added `cursor_hooks` and `claude_hooks` as first-class `AssetKind`s, mirroring the directory-copy behavior of `cursor_rules`.
- Implemented a post-installation step that recursively applies `chmod +x` to all `.sh` files in the destination directory, ensuring hooks work immediately out of the box.
- Added a validation layer (`src/hooks.rs`) that verifies the expected project structure:
  - For Cursor: Checks for `.cursor/hooks.json` and ensures referenced scripts exist.
  - For Claude: Checks for `.claude/settings.json`, parses the `hooks` section, and validates script paths.
- Updated the catalog generation to enumerate individual hook scripts.

## 🤖 Copilot Rule convert

...
