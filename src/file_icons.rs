//! Which vscode-icons icon a file or folder shows, by its name, as VS Code's vscode-icons theme
//! picks them. The icons themselves are in `file_icon_files`.

use crate::file_icon_files::FILES;

/// The SVG of icon `name` (e.g. "file_type_rust"), and the URI egui caches its texture under.
pub fn svg(name: &str) -> Option<(&'static str, &'static [u8])> {
    let index = FILES.binary_search_by(|(n, _)| n.cmp(&name)).ok()?;
    Some(FILES[index])
}

/// The icon for a file named `name`.
pub fn for_file(name: &str) -> &'static str {
    let lower = name.to_ascii_lowercase();
    match by_file_name(&lower).or_else(|| by_extension(&lower)) {
        Some(kind) => kind,
        None => "default_file",
    }
}

/// The icon for a folder named `name`, open or closed.
pub fn for_folder(name: &str, open: bool) -> &'static str {
    let kind = by_folder_name(&name.to_ascii_lowercase());
    match (kind, open) {
        (Some((closed, _)), false) => closed,
        (Some((_, opened)), true) => opened,
        (None, false) => "default_folder",
        (None, true) => "default_folder_opened",
    }
}

fn by_file_name(name: &str) -> Option<&'static str> {
    // Tool configs go by their stem, whatever extension the project picked.
    let stem = name.split('.').find(|s| !s.is_empty()).unwrap_or(name);
    let icon = match name {
        "cargo.toml" | "cargo.lock" => "file_type_cargo",
        "package.json" | "package-lock.json" | ".npmrc" => "file_type_npm",
        "pnpm-lock.yaml" | "pnpm-workspace.yaml" => "file_type_pnpm",
        "yarn.lock" | ".yarnrc" | ".yarnrc.yml" => "file_type_yarn",
        ".gitignore" | ".gitattributes" | ".gitmodules" | ".gitkeep" => "file_type_git",
        "dockerfile" | "containerfile" | ".dockerignore" | "compose.yaml" | "compose.yml" => {
            "file_type_docker"
        }
        "docker-compose.yml" | "docker-compose.yaml" => "file_type_docker",
        "license" | "license.md" | "license.txt" | "licence" | "licence.md" => "file_type_license",
        ".editorconfig" => "file_type_editorconfig",
        ".nvmrc" | ".node-version" => "file_type_node",
        "bun.lockb" | "bun.lock" | "bunfig.toml" => "file_type_bun",
        "deno.json" | "deno.jsonc" | "deno.lock" => "file_type_deno",
        ".babelrc" => "file_type_babel",
        _ if name.starts_with(".env") => "file_type_dotenv",
        _ if name.starts_with(".eslintrc") || stem == "eslint" => "file_type_eslint",
        _ if name.starts_with(".prettierrc") || stem == "prettier" => "file_type_prettier",
        _ if name.starts_with("tsconfig") && name.ends_with(".json") => "file_type_tsconfig",
        _ if name.starts_with("jsconfig") && name.ends_with(".json") => "file_type_jsconfig",
        _ if name.ends_with(".d.ts") => "file_type_typescriptdef",
        _ => match stem {
            "vite" => "file_type_vite",
            "vitest" => "file_type_vitest",
            "tailwind" => "file_type_tailwind",
            "babel" => "file_type_babel",
            "webpack" => "file_type_webpack",
            "rollup" => "file_type_rollup",
            "jest" => "file_type_jest",
            _ => return None,
        },
    };
    Some(icon)
}

fn by_extension(name: &str) -> Option<&'static str> {
    let (_, ext) = name.rsplit_once('.')?;
    let icon = match ext {
        "rs" => "file_type_rust",
        "ts" | "mts" | "cts" => "file_type_typescript",
        "tsx" => "file_type_reactts",
        "jsx" => "file_type_reactjs",
        "js" | "mjs" | "cjs" => "file_type_js",
        "json" | "jsonc" | "json5" => "file_type_json",
        "css" => "file_type_css",
        "scss" => "file_type_scss",
        "sass" => "file_type_sass",
        "less" => "file_type_less",
        "html" | "htm" => "file_type_html",
        "md" | "markdown" => "file_type_markdown",
        "mdx" => "file_type_mdx",
        "py" | "pyi" => "file_type_python",
        "go" => "file_type_go",
        "toml" => "file_type_toml",
        "yaml" | "yml" => "file_type_yaml",
        "ini" | "cfg" | "conf" => "file_type_ini",
        "sh" | "bash" | "zsh" | "fish" => "file_type_shell",
        "swift" => "file_type_swift",
        "rb" => "file_type_ruby",
        "php" => "file_type_php",
        "java" => "file_type_java",
        "kt" | "kts" => "file_type_kotlin",
        "c" | "h" => "file_type_c",
        "cpp" | "cc" | "cxx" | "hpp" | "hh" => "file_type_cpp",
        "cs" => "file_type_csharp",
        "astro" => "file_type_astro",
        "vue" => "file_type_vue",
        "svelte" => "file_type_svelte",
        "liquid" => "file_type_liquid",
        "svg" => "file_type_svg",
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "ico" | "icns" | "bmp" | "avif" => "file_type_image",
        "xml" | "plist" => "file_type_xml",
        "sql" => "file_type_sql",
        "txt" => "file_type_text",
        "log" => "file_type_log",
        "graphql" | "gql" => "file_type_graphql",
        "zip" | "gz" | "tgz" | "tar" | "7z" | "rar" => "file_type_zip",
        "pdf" => "file_type_pdf",
        "ttf" | "otf" | "woff" | "woff2" => "file_type_font",
        "lua" => "file_type_lua",
        "ex" | "exs" => "file_type_elixir",
        "zig" => "file_type_zig",
        "prisma" => "file_type_prisma",
        _ => return None,
    };
    Some(icon)
}

/// Closed and open icons of folders that have their own.
fn by_folder_name(name: &str) -> Option<(&'static str, &'static str)> {
    let kind = match name {
        "src" | "source" => "src",
        "dist" | "out" | "build" | "target" => "dist",
        "docs" | "doc" => "docs",
        "test" | "tests" | "__tests__" | "spec" | "specs" => "test",
        "script" | "scripts" => "script",
        "assets" | "asset" => "asset",
        "images" | "image" | "img" | "icons" => "images",
        "public" | "static" | "www" => "public",
        "components" | "component" => "component",
        "styles" | "style" | "css" => "style",
        "config" | "configs" | ".config" => "config",
        ".git" => "git",
        ".github" => "github",
        ".vscode" => "vscode",
        "node_modules" => "node",
        ".claude" => "claude",
        "macos" => "macos",
        "ios" => "ios",
        "android" => "android",
        "windows" => "windows",
        "linux" => "linux",
        "api" | "apis" => "api",
        "app" | "apps" => "app",
        "controllers" | "controller" => "controller",
        "models" | "model" => "model",
        "views" | "view" | "pages" | "routes" => "view",
        "hooks" | "hook" => "hook",
        "helpers" | "helper" | "utils" | "util" => "helper",
        "tools" | "tool" => "tools",
        "db" | "database" | "migrations" => "db",
        "locales" | "locale" | "i18n" | "lang" => "locale",
        "themes" | "theme" => "theme",
        "tmp" | "temp" => "temp",
        "logs" | "log" => "log",
        "packages" | "package" | "crates" => "package",
        "plugins" | "plugin" | "extensions" => "plugin",
        "server" => "server",
        "client" => "client",
        "e2e" => "e2e",
        "coverage" => "coverage",
        "fonts" | "font" => "fonts",
        "middleware" | "middlewares" => "middleware",
        "mocks" | "mock" | "__mocks__" | "fixtures" => "mock",
        "services" | "service" => "services",
        "shared" | "common" => "shared",
        "templates" | "template" => "template",
        "redux" | "store" => "redux",
        "private" => "private",
        "modules" | "module" => "module",
        "include" | "includes" => "include",
        "docker" | ".docker" => "docker",
        _ => return None,
    };
    let (closed, open) = FOLDERS.iter().find(|(k, _, _)| *k == kind).map(|(_, c, o)| (*c, *o))?;
    Some((closed, open))
}

/// Folder kinds with their closed and open icon names, spelled out so they're `'static`.
const FOLDERS: &[(&str, &str, &str)] = &[
    ("src", "folder_type_src", "folder_type_src_opened"),
    ("dist", "folder_type_dist", "folder_type_dist_opened"),
    ("docs", "folder_type_docs", "folder_type_docs_opened"),
    ("test", "folder_type_test", "folder_type_test_opened"),
    ("script", "folder_type_script", "folder_type_script_opened"),
    ("asset", "folder_type_asset", "folder_type_asset_opened"),
    ("images", "folder_type_images", "folder_type_images_opened"),
    ("public", "folder_type_public", "folder_type_public_opened"),
    ("component", "folder_type_component", "folder_type_component_opened"),
    ("style", "folder_type_style", "folder_type_style_opened"),
    ("config", "folder_type_config", "folder_type_config_opened"),
    ("git", "folder_type_git", "folder_type_git_opened"),
    ("github", "folder_type_github", "folder_type_github_opened"),
    ("vscode", "folder_type_vscode", "folder_type_vscode_opened"),
    ("node", "folder_type_node", "folder_type_node_opened"),
    ("claude", "folder_type_claude", "folder_type_claude_opened"),
    ("macos", "folder_type_macos", "folder_type_macos_opened"),
    ("ios", "folder_type_ios", "folder_type_ios_opened"),
    ("android", "folder_type_android", "folder_type_android_opened"),
    ("windows", "folder_type_windows", "folder_type_windows_opened"),
    ("linux", "folder_type_linux", "folder_type_linux_opened"),
    ("api", "folder_type_api", "folder_type_api_opened"),
    ("app", "folder_type_app", "folder_type_app_opened"),
    ("controller", "folder_type_controller", "folder_type_controller_opened"),
    ("model", "folder_type_model", "folder_type_model_opened"),
    ("view", "folder_type_view", "folder_type_view_opened"),
    ("hook", "folder_type_hook", "folder_type_hook_opened"),
    ("helper", "folder_type_helper", "folder_type_helper_opened"),
    ("tools", "folder_type_tools", "folder_type_tools_opened"),
    ("db", "folder_type_db", "folder_type_db_opened"),
    ("locale", "folder_type_locale", "folder_type_locale_opened"),
    ("theme", "folder_type_theme", "folder_type_theme_opened"),
    ("temp", "folder_type_temp", "folder_type_temp_opened"),
    ("log", "folder_type_log", "folder_type_log_opened"),
    ("package", "folder_type_package", "folder_type_package_opened"),
    ("plugin", "folder_type_plugin", "folder_type_plugin_opened"),
    ("server", "folder_type_server", "folder_type_server_opened"),
    ("client", "folder_type_client", "folder_type_client_opened"),
    ("e2e", "folder_type_e2e", "folder_type_e2e_opened"),
    ("coverage", "folder_type_coverage", "folder_type_coverage_opened"),
    ("fonts", "folder_type_fonts", "folder_type_fonts_opened"),
    ("middleware", "folder_type_middleware", "folder_type_middleware_opened"),
    ("mock", "folder_type_mock", "folder_type_mock_opened"),
    ("services", "folder_type_services", "folder_type_services_opened"),
    ("shared", "folder_type_shared", "folder_type_shared_opened"),
    ("template", "folder_type_template", "folder_type_template_opened"),
    ("redux", "folder_type_redux", "folder_type_redux_opened"),
    ("private", "folder_type_private", "folder_type_private_opened"),
    ("module", "folder_type_module", "folder_type_module_opened"),
    ("include", "folder_type_include", "folder_type_include_opened"),
    ("docker", "folder_type_docker", "folder_type_docker_opened"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_icons_by_name_then_extension() {
        assert_eq!(for_file("main.rs"), "file_type_rust");
        assert_eq!(for_file("Cargo.toml"), "file_type_cargo");
        assert_eq!(for_file("rustfmt.toml"), "file_type_toml");
        assert_eq!(for_file("vite.config.ts"), "file_type_vite");
        assert_eq!(for_file("index.d.ts"), "file_type_typescriptdef");
        assert_eq!(for_file(".env.local"), "file_type_dotenv");
        assert_eq!(for_file("Makefile"), "default_file");
        assert_eq!(for_folder("src", true), "folder_type_src_opened");
        assert_eq!(for_folder("plans", false), "default_folder");
    }

    /// Every icon the mapping can name is bundled, and the table stays sorted for lookups.
    #[test]
    fn every_named_icon_is_bundled() {
        assert!(FILES.windows(2).all(|w| w[0].0 < w[1].0), "file_icon_files is sorted");
        let source = include_str!("file_icons.rs");
        let named = source.split('"').filter(|s| {
            let prefix = ["file_type_", "folder_type_"].into_iter().find(|p| s.starts_with(p));
            prefix.is_some_and(|p| s.len() > p.len())
        });
        for name in named.chain(["default_file", "default_folder", "default_folder_opened"]) {
            assert!(svg(name).is_some(), "{name} is not in assets/icons/vscode");
        }
    }
}
