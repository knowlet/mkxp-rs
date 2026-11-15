use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::error::{Error, Result};

/// Subset of `mkxp.json` that the runtime honors. Unknown keys are ignored.
#[derive(Clone, Debug)]
pub struct Config {
    pub rgss_version: i32,
    pub debug_mode: bool,
    pub display_fps: bool,
    pub print_fps: bool,
    pub win_resizable: bool,
    pub fullscreen: bool,
    pub fixed_aspect_ratio: bool,
    pub smooth_scaling: i32,
    pub vsync: bool,
    pub def_screen_w: i32,
    pub def_screen_h: i32,
    pub window_title: String,
    pub fixed_framerate: i32,
    pub frame_skip: bool,
    pub sync_to_refresh_rate: bool,
    pub game_folder: String,
    pub any_alt_toggle_fs: bool,
    pub enable_reset: bool,
    pub enable_settings: bool,
    pub allow_symlinks: bool,
    pub path_cache: bool,
    pub data_path_org: String,
    pub data_path_app: String,
    pub icon_path: String,
    pub exec_name: String,
    pub midi_sound_font: String,
    pub midi_chorus: bool,
    pub midi_reverb: bool,
    pub se_source_count: i32,
    pub bgm_track_count: i32,
    pub custom_script: String,
    pub use_script_names: bool,
    pub preload_scripts: Vec<String>,
    pub rtps: Vec<String>,
    pub font_scale: f64,
    pub font_kerning: bool,
    pub solid_fonts: Vec<String>,
    pub binding_names: BTreeMap<String, String>,
    pub editor_debug: bool,
    pub editor_battle_test: bool,
    pub game_title: String,
    pub game_scripts: String,
    pub win_console: bool,
    pub manual_folder_select: bool,
    pub warnings: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        let mut binding_names = BTreeMap::new();
        for key in ["a", "b", "c", "x", "y", "z", "l", "r"] {
            binding_names.insert(key.to_string(), key.to_ascii_uppercase());
        }
        Self {
            rgss_version: 0,
            debug_mode: false,
            display_fps: false,
            print_fps: false,
            win_resizable: true,
            fullscreen: false,
            fixed_aspect_ratio: true,
            smooth_scaling: 0,
            vsync: false,
            def_screen_w: 0,
            def_screen_h: 0,
            window_title: String::new(),
            fixed_framerate: 0,
            frame_skip: false,
            sync_to_refresh_rate: false,
            game_folder: String::new(),
            any_alt_toggle_fs: false,
            enable_reset: true,
            enable_settings: true,
            allow_symlinks: true,
            path_cache: true,
            data_path_org: String::new(),
            data_path_app: String::new(),
            icon_path: String::new(),
            exec_name: "Game".to_string(),
            midi_sound_font: String::new(),
            midi_chorus: false,
            midi_reverb: false,
            se_source_count: 6,
            bgm_track_count: 1,
            custom_script: String::new(),
            use_script_names: true,
            preload_scripts: Vec::new(),
            rtps: Vec::new(),
            font_scale: 0.0,
            font_kerning: true,
            solid_fonts: Vec::new(),
            binding_names,
            editor_debug: false,
            editor_battle_test: false,
            game_title: String::new(),
            game_scripts: String::new(),
            win_console: false,
            manual_folder_select: false,
            warnings: Vec::new(),
        }
    }
}

impl Config {
    /// Read `mkxp.json` and `<execName>.ini` the way `Config::read` does.
    /// A missing or unreadable config file keeps the defaults and records a warning.
    pub fn load_dir(dir: &Path, args: &[String]) -> Self {
        let mut config = Self::default();
        config.apply_args(args);
        let path = dir.join("mkxp.json");
        match fs::read_to_string(&path) {
            Ok(text) => match parse_json5(&text) {
                Ok(value) => {
                    if let Err(err) = config.apply_json(&value) {
                        config.warnings.push(err.message);
                    }
                }
                Err(err) => config.warnings.push(format!(
                    "Failed to parse {}: {}",
                    path.display(),
                    err.message
                )),
            },
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => config
                .warnings
                .push(format!("Failed to read {}: {err}", path.display())),
        }
        let game_dir = game_directory(dir, &config.game_folder);
        config.read_game_ini(&game_dir);
        config.clamp_runtime();
        config.apply_screen_defaults();
        config.win_console = env_flag("MKXPZ_WINDOWS_CONSOLE", config.editor_debug);
        config.manual_folder_select = env_flag("MKXPZ_FOLDER_SELECT", false);
        config
    }

    pub fn from_json5(text: &str) -> Result<Self> {
        let mut config = Self::default();
        config.apply_json(&parse_json5(text)?)?;
        config.clamp_runtime();
        config.apply_screen_defaults();
        Ok(config)
    }

    pub fn screen_size(&self) -> (i32, i32) {
        (self.def_screen_w, self.def_screen_h)
    }

    fn apply_args(&mut self, args: &[String]) {
        if let Some(first) = args.first() {
            if first == "debug" || first == "test" {
                self.editor_debug = true;
            } else if first == "btest" {
                self.editor_battle_test = true;
            }
        }
    }

    fn apply_json(&mut self, value: &Json) -> Result<()> {
        let obj = value
            .as_object()
            .ok_or_else(|| Error::new("mkxp.json must be an object"))?;
        let get = |key: &str| obj.iter().find(|(k, _)| k == key).map(|(_, v)| v);
        if let Some(v) = get("rgssVersion").and_then(Json::as_i32) {
            self.rgss_version = v;
        }
        assign_bool(&mut self.debug_mode, get("debugMode"));
        assign_bool(&mut self.display_fps, get("displayFPS"));
        assign_bool(&mut self.print_fps, get("printFPS"));
        assign_bool(&mut self.win_resizable, get("winResizable"));
        assign_bool(&mut self.fullscreen, get("fullscreen"));
        assign_bool(&mut self.fixed_aspect_ratio, get("fixedAspectRatio"));
        if let Some(v) = get("smoothScaling").and_then(Json::as_i32) {
            self.smooth_scaling = v;
        }
        assign_bool(&mut self.vsync, get("vsync"));
        if let Some(v) = get("defScreenW").and_then(Json::as_i32) {
            self.def_screen_w = v;
        }
        if let Some(v) = get("defScreenH").and_then(Json::as_i32) {
            self.def_screen_h = v;
        }
        if let Some(v) = get("windowTitle").and_then(Json::as_str) {
            self.window_title = v.to_string();
        }
        if let Some(v) = get("fixedFramerate").and_then(Json::as_i32) {
            self.fixed_framerate = v;
        }
        assign_bool(&mut self.frame_skip, get("frameSkip"));
        assign_bool(&mut self.sync_to_refresh_rate, get("syncToRefreshrate"));
        if let Some(v) = get("gameFolder").and_then(Json::as_str) {
            self.game_folder = v.to_string();
        }
        assign_bool(&mut self.any_alt_toggle_fs, get("anyAltToggleFS"));
        assign_bool(&mut self.enable_reset, get("enableReset"));
        assign_bool(&mut self.enable_settings, get("enableSettings"));
        assign_bool(&mut self.allow_symlinks, get("allowSymlinks"));
        assign_bool(&mut self.path_cache, get("pathCache"));
        if let Some(v) = get("dataPathOrg").and_then(Json::as_str) {
            self.data_path_org = v.to_string();
        }
        if let Some(v) = get("dataPathApp").and_then(Json::as_str) {
            self.data_path_app = v.to_string();
        }
        if let Some(v) = get("iconPath").and_then(Json::as_str) {
            self.icon_path = v.to_string();
        }
        if let Some(v) = get("execName").and_then(Json::as_str) {
            self.exec_name = v.to_string();
        }
        if let Some(v) = get("midiSoundFont").and_then(Json::as_str) {
            self.midi_sound_font = v.to_string();
        }
        assign_bool(&mut self.midi_chorus, get("midiChorus"));
        assign_bool(&mut self.midi_reverb, get("midiReverb"));
        if let Some(v) = get("SESourceCount").and_then(Json::as_i32) {
            self.se_source_count = v;
        }
        if let Some(v) = get("BGMTrackCount").and_then(Json::as_i32) {
            self.bgm_track_count = v;
        }
        if let Some(v) = get("customScript").and_then(Json::as_str) {
            self.custom_script = v.to_string();
        }
        assign_bool(&mut self.use_script_names, get("useScriptNames"));
        if let Some(v) = get("fontScale").and_then(Json::as_f64) {
            self.font_scale = v;
        }
        assign_bool(&mut self.font_kerning, get("fontKerning"));
        if let Some(list) = get("preloadScript").and_then(Json::as_string_list) {
            self.preload_scripts = list;
        }
        if let Some(list) = get("RTP").and_then(Json::as_string_list) {
            self.rtps = list;
        }
        if let Some(list) = get("solidFonts").and_then(Json::as_string_list) {
            self.solid_fonts = list.into_iter().map(|s| s.to_ascii_lowercase()).collect();
        }
        if let Some(Json::Object(entries)) = get("bindingNames") {
            for (key, value) in entries {
                if let Some(text) = value.as_str() {
                    self.binding_names.insert(key.clone(), text.to_string());
                }
            }
        }
        Ok(())
    }

    fn read_game_ini(&mut self, dir: &Path) {
        if !self.custom_script.is_empty() {
            self.game_title = self.custom_script.clone();
            if self.rgss_version == 0 {
                self.rgss_version = 1;
            }
            return;
        }
        let ini_path = dir.join(format!("{}.ini", self.exec_name));
        match fs::read(&ini_path) {
            Ok(bytes) => {
                // Game.ini titles are often legacy bytes. Decode the file
                // without dropping ASCII keys such as Scripts when Title is
                // not UTF-8. Invalid bytes become Latin-1 characters, which
                // is what a Western RPG Maker title uses for bytes such as 0xE9.
                let text = decode_ini_bytes(&bytes);
                let ini = parse_ini(&text);
                if let Some(title) = ini.get("game.title") {
                    self.game_title = title.clone();
                }
                if let Some(scripts) = ini.get("game.scripts") {
                    self.game_scripts = scripts.replace('\\', "/");
                }
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                self.warnings
                    .push(format!("Could not read {}", ini_path.display()));
            }
            Err(err) => self
                .warnings
                .push(format!("Could not read {}: {err}", ini_path.display())),
        }
        if self.game_title.is_empty() {
            self.game_title = "mkxp-z".to_string();
        }
        if self.rgss_version == 0 {
            self.rgss_version = guess_version(&self.game_scripts);
        }
    }

    fn clamp_runtime(&mut self) {
        self.rgss_version = self.rgss_version.clamp(0, 3);
        self.se_source_count = self.se_source_count.clamp(1, 64);
        self.bgm_track_count = self.bgm_track_count.clamp(1, 16);
    }

    fn apply_screen_defaults(&mut self) {
        if self.def_screen_w <= 0 {
            self.def_screen_w = if self.rgss_version == 1 { 640 } else { 544 };
        }
        if self.def_screen_h <= 0 {
            self.def_screen_h = if self.rgss_version == 1 { 480 } else { 416 };
        }
    }

    pub fn font_is_solid(&self, name: &str) -> bool {
        let lower = name.to_ascii_lowercase();
        self.solid_fonts.iter().any(|font| font == &lower)
    }
}

fn game_directory(config_dir: &Path, game_folder: &str) -> std::path::PathBuf {
    if game_folder.is_empty() {
        return config_dir.to_path_buf();
    }
    let folder = Path::new(game_folder);
    if folder.is_absolute() {
        folder.to_path_buf()
    } else {
        config_dir.join(folder)
    }
}

fn decode_ini_bytes(bytes: &[u8]) -> String {
    let mut out = String::new();
    let mut index = 0;
    while index < bytes.len() {
        match std::str::from_utf8(&bytes[index..]) {
            Ok(text) => {
                out.push_str(text);
                break;
            }
            Err(err) => {
                let valid = err.valid_up_to();
                out.push_str(std::str::from_utf8(&bytes[index..index + valid]).unwrap());
                index += valid;
                if index < bytes.len() {
                    out.push(bytes[index] as char);
                    index += 1;
                }
            }
        }
    }
    out
}

fn guess_version(scripts: &str) -> i32 {
    if scripts.ends_with(".rvdata2") {
        3
    } else if scripts.ends_with(".rvdata") {
        2
    } else {
        1
    }
}

fn assign_bool(slot: &mut bool, value: Option<&Json>) {
    if let Some(Json::Bool(v)) = value {
        *slot = *v;
    }
}

fn env_flag(name: &str, default: bool) -> bool {
    match std::env::var(name).as_deref() {
        Ok("0") => false,
        Ok("1") => true,
        _ => default,
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Json {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

impl Json {
    fn as_object(&self) -> Option<&[(String, Json)]> {
        match self {
            Json::Object(v) => Some(v),
            _ => None,
        }
    }
    fn as_str(&self) -> Option<&str> {
        match self {
            Json::String(v) => Some(v),
            _ => None,
        }
    }
    fn as_i32(&self) -> Option<i32> {
        match self {
            Json::Number(v)
                if v.fract() == 0.0 && (i32::MIN as f64..=i32::MAX as f64).contains(v) =>
            {
                Some(*v as i32)
            }
            _ => None,
        }
    }
    fn as_f64(&self) -> Option<f64> {
        match self {
            Json::Number(v) => Some(*v),
            _ => None,
        }
    }
    fn as_string_list(&self) -> Option<Vec<String>> {
        match self {
            Json::Array(items) => Some(
                items
                    .iter()
                    .filter_map(Json::as_str)
                    .map(str::to_string)
                    .collect(),
            ),
            Json::String(v) => Some(vec![v.clone()]),
            _ => None,
        }
    }
}

fn parse_json5(text: &str) -> Result<Json> {
    let mut parser = Parser {
        bytes: text.as_bytes(),
        index: 0,
    };
    parser.skip()?;
    if parser.eof() {
        return Ok(Json::Object(Vec::new()));
    }
    let value = parser.parse_value()?;
    parser.skip()?;
    if !parser.eof() {
        return Err(Error::new("trailing data in mkxp.json"));
    }
    Ok(value)
}

struct Parser<'a> {
    bytes: &'a [u8],
    index: usize,
}

impl Parser<'_> {
    fn eof(&self) -> bool {
        self.index >= self.bytes.len()
    }
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.index).copied()
    }
    fn bump(&mut self) -> Option<u8> {
        let byte = self.peek()?;
        self.index += 1;
        Some(byte)
    }
    fn skip(&mut self) -> Result<()> {
        loop {
            while matches!(self.peek(), Some(b' ' | b'\t' | b'\r' | b'\n')) {
                self.index += 1;
            }
            if self.peek() == Some(b'/') && self.bytes.get(self.index + 1) == Some(&b'/') {
                self.index += 2;
                while let Some(byte) = self.peek() {
                    self.index += 1;
                    if byte == b'\n' {
                        break;
                    }
                }
                continue;
            }
            if self.peek() == Some(b'/') && self.bytes.get(self.index + 1) == Some(&b'*') {
                self.index += 2;
                loop {
                    if self.eof() {
                        return Err(Error::new("unterminated block comment"));
                    }
                    if self.peek() == Some(b'*') && self.bytes.get(self.index + 1) == Some(&b'/') {
                        self.index += 2;
                        break;
                    }
                    self.index += 1;
                }
                continue;
            }
            break;
        }
        Ok(())
    }
    fn parse_value(&mut self) -> Result<Json> {
        self.skip()?;
        match self.peek() {
            Some(b'{') => self.parse_object(),
            Some(b'[') => self.parse_array(),
            Some(b'"') => Ok(Json::String(self.parse_string(b'"')?)),
            Some(b'\'') => Ok(Json::String(self.parse_string(b'\'')?)),
            Some(b't') => self.parse_ident("true", Json::Bool(true)),
            Some(b'f') => self.parse_ident("false", Json::Bool(false)),
            Some(b'n') => self.parse_ident("null", Json::Null),
            Some(b'+' | b'-' | b'.' | b'0'..=b'9') => self.parse_number(),
            Some(other) => Err(Error::new(format!(
                "unexpected '{}' in mkxp.json",
                other as char
            ))),
            None => Err(Error::new("unexpected end of mkxp.json")),
        }
    }
    fn parse_ident(&mut self, text: &str, value: Json) -> Result<Json> {
        let end = self.index + text.len();
        if self.bytes.get(self.index..end) == Some(text.as_bytes()) {
            self.index = end;
            Ok(value)
        } else {
            Err(Error::new(format!("expected {text}")))
        }
    }
    fn parse_object(&mut self) -> Result<Json> {
        self.bump();
        let mut entries = Vec::new();
        loop {
            self.skip()?;
            if self.peek() == Some(b'}') {
                self.bump();
                break;
            }
            let key = match self.peek() {
                Some(b'"') => self.parse_string(b'"')?,
                Some(b'\'') => self.parse_string(b'\'')?,
                Some(b'A'..=b'Z' | b'a'..=b'z' | b'_') => self.parse_bare_key()?,
                _ => return Err(Error::new("expected an object key")),
            };
            self.skip()?;
            if self.bump() != Some(b':') {
                return Err(Error::new("expected ':' after an object key"));
            }
            let value = self.parse_value()?;
            entries.push((key, value));
            self.skip()?;
            match self.bump() {
                Some(b',') => continue,
                Some(b'}') => break,
                _ => return Err(Error::new("expected ',' or '}' in object")),
            }
        }
        Ok(Json::Object(entries))
    }
    fn parse_array(&mut self) -> Result<Json> {
        self.bump();
        let mut items = Vec::new();
        loop {
            self.skip()?;
            if self.peek() == Some(b']') {
                self.bump();
                break;
            }
            items.push(self.parse_value()?);
            self.skip()?;
            match self.bump() {
                Some(b',') => continue,
                Some(b']') => break,
                _ => return Err(Error::new("expected ',' or ']' in array")),
            }
        }
        Ok(Json::Array(items))
    }
    fn parse_bare_key(&mut self) -> Result<String> {
        let start = self.index;
        while matches!(
            self.peek(),
            Some(b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_')
        ) {
            self.index += 1;
        }
        Ok(String::from_utf8_lossy(&self.bytes[start..self.index]).into_owned())
    }
    fn parse_string(&mut self, quote: u8) -> Result<String> {
        self.bump();
        let mut out = String::new();
        loop {
            let byte = self
                .peek()
                .ok_or_else(|| Error::new("unterminated string"))?;
            if byte == quote {
                self.bump();
                return Ok(out);
            }
            if byte == b'\\' {
                self.bump();
                let escaped = self
                    .bump()
                    .ok_or_else(|| Error::new("truncated string escape"))?;
                match escaped {
                    b'"' | b'\'' | b'\\' | b'/' => out.push(escaped as char),
                    b'n' => out.push('\n'),
                    b'r' => out.push('\r'),
                    b't' => out.push('\t'),
                    b'b' => out.push('\u{0008}'),
                    b'f' => out.push('\u{000c}'),
                    b'v' => out.push('\u{000b}'),
                    b'0' => out.push('\0'),
                    b'\n' | b'\r' => {
                        if escaped == b'\r' && self.peek() == Some(b'\n') {
                            self.bump();
                        }
                    }
                    b'x' => {
                        let mut hex = [0u8; 2];
                        for slot in &mut hex {
                            *slot = self
                                .bump()
                                .ok_or_else(|| Error::new("truncated hex escape"))?;
                        }
                        let text =
                            std::str::from_utf8(&hex).map_err(|_| Error::new("bad hex escape"))?;
                        let code = u32::from_str_radix(text, 16)
                            .map_err(|_| Error::new("bad hex escape"))?;
                        out.push(char::from_u32(code).unwrap_or('\u{FFFD}'));
                    }
                    b'u' => {
                        if self.peek() == Some(b'{') {
                            self.bump();
                            let mut hex = Vec::new();
                            loop {
                                let digit = self
                                    .bump()
                                    .ok_or_else(|| Error::new("truncated unicode escape"))?;
                                if digit == b'}' {
                                    break;
                                }
                                if !digit.is_ascii_hexdigit() || hex.len() == 6 {
                                    return Err(Error::new("bad unicode escape"));
                                }
                                hex.push(digit);
                            }
                            if hex.is_empty() {
                                return Err(Error::new("bad unicode escape"));
                            }
                            let text = std::str::from_utf8(&hex)
                                .map_err(|_| Error::new("bad unicode escape"))?;
                            let code = u32::from_str_radix(text, 16)
                                .map_err(|_| Error::new("bad unicode escape"))?;
                            out.push(
                                char::from_u32(code)
                                    .ok_or_else(|| Error::new("bad unicode escape"))?,
                            );
                            continue;
                        }
                        let mut hex = [0u8; 4];
                        for slot in &mut hex {
                            *slot = self
                                .bump()
                                .ok_or_else(|| Error::new("truncated unicode escape"))?;
                        }
                        let text = std::str::from_utf8(&hex)
                            .map_err(|_| Error::new("bad unicode escape"))?;
                        let code = u32::from_str_radix(text, 16)
                            .map_err(|_| Error::new("bad unicode escape"))?;
                        // JSON represents non-BMP characters as two UTF-16
                        // escapes. Decode the pair before converting to a Rust
                        // scalar; lone surrogates keep replacement semantics.
                        let low = self
                            .bytes
                            .get(self.index..self.index + 6)
                            .filter(|tail| tail.starts_with(b"\\u"))
                            .and_then(|tail| std::str::from_utf8(&tail[2..]).ok())
                            .and_then(|hex| u32::from_str_radix(hex, 16).ok());
                        let scalar = if (0xD800..=0xDBFF).contains(&code) {
                            if let Some(low) = low.filter(|low| (0xDC00..=0xDFFF).contains(low)) {
                                self.index += 6;
                                0x10000 + ((code - 0xD800) << 10) + low - 0xDC00
                            } else {
                                code
                            }
                        } else {
                            code
                        };
                        out.push(char::from_u32(scalar).unwrap_or('\u{FFFD}'));
                    }
                    _ => return Err(Error::new("unknown string escape")),
                }
                continue;
            }
            let width = match byte {
                0x00..=0x7F => 1,
                0xC2..=0xDF => 2,
                0xE0..=0xEF => 3,
                0xF0..=0xF4 => 4,
                _ => return Err(Error::new("invalid utf-8 in string")),
            };
            let end = self.index + width;
            let slice = self
                .bytes
                .get(self.index..end)
                .ok_or_else(|| Error::new("invalid utf-8 in string"))?;
            let text =
                std::str::from_utf8(slice).map_err(|_| Error::new("invalid utf-8 in string"))?;
            let ch = text
                .chars()
                .next()
                .ok_or_else(|| Error::new("invalid utf-8 in string"))?;
            self.index = end;
            out.push(ch);
        }
    }
    fn parse_number(&mut self) -> Result<Json> {
        let start = self.index;
        if matches!(self.peek(), Some(b'+' | b'-')) {
            self.index += 1;
        }
        if self.peek() == Some(b'0') && matches!(self.bytes.get(self.index + 1), Some(b'x' | b'X'))
        {
            self.index += 2;
            let hex_start = self.index;
            while matches!(self.peek(), Some(b'0'..=b'9' | b'a'..=b'f' | b'A'..=b'F')) {
                self.index += 1;
            }
            let text = std::str::from_utf8(&self.bytes[hex_start..self.index])
                .map_err(|_| Error::new("bad number"))?;
            let value = i64::from_str_radix(text, 16).map_err(|_| Error::new("bad number"))?;
            let sign = if self.bytes[start] == b'-' { -1 } else { 1 };
            return Ok(Json::Number((value * sign) as f64));
        }
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.index += 1;
        }
        if self.peek() == Some(b'.') {
            self.index += 1;
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.index += 1;
            }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.index += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.index += 1;
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.index += 1;
            }
        }
        let text = std::str::from_utf8(&self.bytes[start..self.index])
            .map_err(|_| Error::new("bad number"))?;
        let value = text.parse::<f64>().map_err(|_| Error::new("bad number"))?;
        Ok(Json::Number(value))
    }
}

fn parse_ini(text: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let mut section = String::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = line[1..line.len() - 1].trim().to_ascii_lowercase();
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            let name = format!("{}.{}", section, key.trim().to_ascii_lowercase());
            out.insert(name, value.trim().to_string());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json5_accepts_signed_and_leading_decimal_numbers() {
        for (literal, expected) in [
            (".5", 0.5),
            ("+.5", 0.5),
            ("-.5", -0.5),
            ("+0.5", 0.5),
            ("-0.5", -0.5),
            ("+1", 1.0),
            ("1.", 1.0),
            ("+1.", 1.0),
            (".5e+2", 50.0),
            ("+.5E-1", 0.05),
        ] {
            let config = Config::from_json5(&format!("{{fontScale: {literal}}}"))
                .unwrap_or_else(|err| panic!("{literal}: {err}"));
            assert_eq!(config.font_scale, expected, "{literal}");
        }
    }

    #[test]
    fn json5_accepts_signed_hex_numbers() {
        for (literal, expected) in [("0x280", 640.0), ("+0X280", 640.0), ("-0x280", -640.0)] {
            let config = Config::from_json5(&format!("{{fontScale: {literal}}}"))
                .unwrap_or_else(|err| panic!("{literal}: {err}"));
            assert_eq!(config.font_scale, expected, "{literal}");
        }
        let config = Config::from_json5("{defScreenW: +0x280}").unwrap();
        assert_eq!(config.def_screen_w, 640);
    }

    #[test]
    fn json5_rejects_incomplete_signed_numbers() {
        for literal in [
            "+", "-", ".", "+.", "-.", "+0x", "-0X", "+1e", "+1e-", "++1", "+-1",
        ] {
            assert!(
                Config::from_json5(&format!("{{fontScale: {literal}}}")).is_err(),
                "{literal}"
            );
        }
    }

    #[test]
    fn json5_surrogate_pairs_match_literal_unicode() {
        let escaped =
            Config::from_json5(r#"{windowTitle: "\uD83D\uDE00", RTP: ["\uD840\uDC00"]}"#).unwrap();
        let literal = Config::from_json5(r#"{windowTitle: "😀", RTP: ["𠀀"]}"#).unwrap();
        assert_eq!(escaped.window_title, literal.window_title);
        assert_eq!(escaped.rtps, literal.rtps);
    }

    #[test]
    fn json5_surrogate_boundaries_and_invalid_escapes() {
        for (escaped, expected) in [
            (r"\uD800\uDC00", "\u{10000}"),
            (r"\uDBFF\uDFFF", "\u{10FFFF}"),
            (r"\uD83D\uDE00\uD83D\uDE00", "😀😀"),
            (r"\u00E9\u52C7", "é勇"),
            (r"\uD800", "�"),
            (r"\uDC00", "�"),
            (r"\uD800\u0041", "�A"),
            (r"\uD800x", "�x"),
        ] {
            let config = Config::from_json5(&format!("{{windowTitle: '{escaped}'}}")).unwrap();
            assert_eq!(config.window_title, expected, "{escaped}");
        }
        let braced = Config::from_json5(r#"{windowTitle: "\u{1F600}"}"#).unwrap();
        assert_eq!(braced.window_title, "😀");
        assert!(Config::from_json5(r#"{windowTitle: "\u{}"}"#).is_err());
        assert!(Config::from_json5(r#"{windowTitle: "\q"}"#).is_err());
        for escaped in [r"\uD800\uZZZZ", r"\uD800\uDC", r"\uZZZZ"] {
            assert!(Config::from_json5(&format!("{{windowTitle: '{escaped}'}}")).is_err());
        }
    }

    #[test]
    fn json5_comments_and_trailing_commas() {
        let config = Config::from_json5(
            r#"{
                // version guess is overridden
                rgssVersion: 3,
                RTP: ["Standard",],
                "windowTitle": "Demo",
            }"#,
        )
        .unwrap();
        assert_eq!(config.rgss_version, 3);
        assert_eq!(config.rtps, ["Standard"]);
        assert_eq!(config.window_title, "Demo");
        assert_eq!(config.screen_size(), (544, 416));
        let utf8 = Config::from_json5("{ RTP: [\"Pokémon\"], windowTitle: \"勇者\" }").unwrap();
        assert_eq!(utf8.rtps, ["Pokémon"]);
        assert_eq!(utf8.window_title, "勇者");
    }

    #[test]
    fn game_ini_selects_the_rgss_version() {
        let dir = std::env::temp_dir().join(format!("mkxpz-config-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("Game.ini"),
            "[Game]\nTitle=Harbor\nScripts=Data\\Scripts.rvdata2\n",
        )
        .unwrap();
        let config = Config::load_dir(&dir, &["debug".to_string()]);
        assert!(config.editor_debug);
        assert_eq!(config.game_title, "Harbor");
        assert_eq!(config.rgss_version, 3);
        assert_eq!(config.game_scripts, "Data/Scripts.rvdata2");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn game_folder_reads_the_ini_inside_the_selected_directory() {
        let dir = std::env::temp_dir().join(format!("mkxpz-gamefolder-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let game = dir.join("Game");
        fs::create_dir_all(&game).unwrap();
        fs::write(dir.join("mkxp.json"), "{ gameFolder: \"Game\" }\n").unwrap();
        fs::write(
            dir.join("Game.ini"),
            "[Game]\nTitle=Wrong\nScripts=Data\\Scripts.rxdata\n",
        )
        .unwrap();
        fs::write(
            game.join("Game.ini"),
            "[Game]\nTitle=Nested\nScripts=Data\\Scripts.rvdata2\n",
        )
        .unwrap();
        let config = Config::load_dir(&dir, &[]);
        assert_eq!(config.game_title, "Nested");
        assert_eq!(config.game_scripts, "Data/Scripts.rvdata2");
        assert_eq!(config.rgss_version, 3);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn legacy_ini_title_keeps_the_scripts_path() {
        let dir = std::env::temp_dir().join(format!("mkxpz-ini-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let mut bytes = b"[Game]\nTitle=Caf".to_vec();
        bytes.push(0xE9);
        bytes.extend_from_slice(b"\nScripts=Data/Scripts.rvdata2\n");
        fs::write(dir.join("Game.ini"), bytes).unwrap();
        let config = Config::load_dir(&dir, &[]);
        assert_eq!(config.game_title, "Café");
        assert_eq!(config.game_scripts, "Data/Scripts.rvdata2");
        assert_eq!(config.rgss_version, 3);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn repository_mkxp_json_parses() {
        let text = fs::read_to_string("mkxp.json").unwrap();
        let config = Config::from_json5(&text).unwrap();
        assert_eq!(config.rgss_version, 0);
        assert!(config.win_resizable);
    }
}
