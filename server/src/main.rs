use std::{
    fs,
    io::{self, BufRead, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

fn main() {
    let mode = std::env::args().nth(1).unwrap_or_else(|| "mcp".to_string());
    if mode != "mcp" {
        eprintln!("usage: canmored mcp");
        std::process::exit(2);
    }

    if let Err(error) = run_mcp() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run_mcp() -> Result<(), String> {
    let store = Store::open(std::env::current_dir().map_err(|error| error.to_string())?)?;
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = line.map_err(|error| error.to_string())?;
        if line.trim().is_empty() {
            continue;
        }
        if let Some(response) = handle_rpc(&store, &line) {
            writeln!(stdout, "{}", response).map_err(|error| error.to_string())?;
            stdout.flush().map_err(|error| error.to_string())?;
        }
    }

    Ok(())
}

fn handle_rpc(store: &Store, line: &str) -> Option<String> {
    let request: Value = match serde_json::from_str(line) {
        Ok(value) => value,
        Err(error) => return Some(rpc_error(Value::Null, -32700, error.to_string()).to_string()),
    };
    let id = request.get("id").cloned();
    let method = request.get("method").and_then(Value::as_str).unwrap_or("");
    if id.is_none() {
        return None;
    }
    let id = id.unwrap_or(Value::Null);

    let result = match method {
        "notifications/initialized" => Ok(json!({})),
        "initialize" => Ok(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "codex-canmore", "version": env!("CARGO_PKG_VERSION") }
        })),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tools() })),
        "tools/call" => {
            let params = request.get("params").cloned().unwrap_or_else(|| json!({}));
            let name = params.get("name").and_then(Value::as_str).unwrap_or("");
            let args = params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            match call_tool(store, name, args) {
                Ok(value) => Ok(json!({
                    "content": [{ "type": "text", "text": serde_json::to_string(&value).unwrap() }],
                    "isError": false
                })),
                Err(error) => Ok(json!({
                    "content": [{ "type": "text", "text": error }],
                    "isError": true
                })),
            }
        }
        _ => Err(format!("unsupported method {method}")),
    };

    match result {
        Ok(result) => Some(json!({ "jsonrpc": "2.0", "id": id, "result": result }).to_string()),
        Err(error) => Some(rpc_error(id, -32601, error).to_string()),
    }
}

fn rpc_error(id: Value, code: i64, message: String) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

fn call_tool(store: &Store, name: &str, args: Value) -> Result<Value, String> {
    match name {
        "canmore_medium_recipe" => Ok(recipe(args.get("medium").and_then(Value::as_str))),
        "canmore_medium_create" => {
            let input: SurfaceInput =
                serde_json::from_value(args).map_err(|error| error.to_string())?;
            Ok(serde_json::to_value(store.create_surface(input)?).unwrap())
        }
        "canmore_medium_update" => {
            let input: SurfacePatch =
                serde_json::from_value(args).map_err(|error| error.to_string())?;
            Ok(serde_json::to_value(store.update_surface(input)?).unwrap())
        }
        "canmore_medium_event" => {
            let input: EventInput =
                serde_json::from_value(args).map_err(|error| error.to_string())?;
            Ok(serde_json::to_value(store.record_event(input)?).unwrap())
        }
        "canmore_medium_read" => {
            let id = required_string(&args, "id")?;
            let include_spec = args
                .get("include_spec")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let include_events = args
                .get("include_events")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            Ok(store.read_surface(&id, include_spec, include_events)?)
        }
        "canmore_medium_list" => {
            let limit = args
                .get("limit")
                .and_then(Value::as_u64)
                .unwrap_or(20)
                .min(100) as usize;
            Ok(json!({ "surfaces": store.list_surfaces(limit)? }))
        }
        "canmore_medium_promote" => {
            let id = required_string(&args, "id")?;
            let note = args.get("note").and_then(Value::as_str).map(str::to_string);
            Ok(serde_json::to_value(store.promote_surface(&id, note)?).unwrap())
        }
        "canmore_image_asset_register" => {
            let input: AssetInput =
                serde_json::from_value(args).map_err(|error| error.to_string())?;
            Ok(serde_json::to_value(store.register_asset(input)?).unwrap())
        }
        _ => Err(format!("unknown tool {name}")),
    }
}

fn required_string(value: &Value, key: &str) -> Result<String, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .ok_or_else(|| format!("missing {key}"))
}

fn required_non_empty(value: String, key: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(format!("{key} is required"))
    } else {
        Ok(trimmed.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Surface {
    id: String,
    title: String,
    purpose: String,
    medium: String,
    ephemeral: bool,
    promotion: String,
    cards: Vec<Card>,
    events: Vec<SurfaceEvent>,
    created_at: String,
    updated_at: String,
    promoted_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Card {
    id: Option<String>,
    #[serde(rename = "type")]
    kind: String,
    title: String,
    body: String,
    value: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SurfaceEvent {
    id: String,
    kind: String,
    target: Option<String>,
    value: Option<Value>,
    note: Option<String>,
    created_at: String,
}

#[derive(Debug, Deserialize)]
struct SurfaceInput {
    title: String,
    purpose: String,
    medium: String,
    #[serde(default = "default_ephemeral")]
    ephemeral: bool,
    #[serde(default)]
    promotion: Option<String>,
    #[serde(default)]
    cards: Vec<Card>,
}

#[derive(Debug, Deserialize)]
struct SurfacePatch {
    id: String,
    title: Option<String>,
    purpose: Option<String>,
    medium: Option<String>,
    ephemeral: Option<bool>,
    promotion: Option<String>,
    cards: Option<Vec<Card>>,
}

#[derive(Debug, Deserialize)]
struct EventInput {
    id: String,
    kind: String,
    target: Option<String>,
    value: Option<Value>,
    note: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AssetInput {
    surface_id: String,
    path: String,
    note: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct AssetRecord {
    id: String,
    surface_id: String,
    file_name: String,
    stored_path: String,
    byte_len: u64,
    mime_type: String,
    sha256: String,
    note: Option<String>,
    created_at: String,
}

fn default_ephemeral() -> bool {
    true
}

struct Store {
    root: PathBuf,
}

impl Store {
    fn open(workspace: PathBuf) -> Result<Self, String> {
        let root = workspace.join(".canmore-medium");
        fs::create_dir_all(root.join("surfaces")).map_err(|error| error.to_string())?;
        fs::create_dir_all(root.join("assets")).map_err(|error| error.to_string())?;
        Ok(Self { root })
    }

    fn create_surface(&self, input: SurfaceInput) -> Result<Value, String> {
        let title = required_non_empty(input.title, "title")?;
        let purpose = required_non_empty(input.purpose, "purpose")?;
        let medium = required_non_empty(input.medium, "medium")?;
        let now = now_stamp();
        let surface = Surface {
            id: new_id("surface"),
            title,
            purpose,
            medium,
            ephemeral: input.ephemeral,
            promotion: input
                .promotion
                .unwrap_or_else(|| "Promote only if this becomes project material.".to_string()),
            cards: input.cards,
            events: Vec::new(),
            created_at: now.clone(),
            updated_at: now,
            promoted_at: None,
        };
        self.write_surface(&surface)?;
        Ok(self.surface_response(&surface, false, false))
    }

    fn update_surface(&self, input: SurfacePatch) -> Result<Value, String> {
        let mut surface = self.read_surface_file(&input.id)?;
        if let Some(title) = input.title {
            surface.title = required_non_empty(title, "title")?;
        }
        if let Some(purpose) = input.purpose {
            surface.purpose = required_non_empty(purpose, "purpose")?;
        }
        if let Some(medium) = input.medium {
            surface.medium = required_non_empty(medium, "medium")?;
        }
        if let Some(ephemeral) = input.ephemeral {
            surface.ephemeral = ephemeral;
        }
        if let Some(promotion) = input.promotion {
            surface.promotion = promotion;
        }
        if let Some(cards) = input.cards {
            surface.cards = cards;
        }
        surface.updated_at = now_stamp();
        self.write_surface(&surface)?;
        Ok(self.surface_response(&surface, false, false))
    }

    fn record_event(&self, input: EventInput) -> Result<Value, String> {
        let mut surface = self.read_surface_file(&input.id)?;
        if input.kind.trim().is_empty() {
            return Err("event kind is required".into());
        }
        let event = SurfaceEvent {
            id: new_id("event"),
            kind: input.kind,
            target: input.target,
            value: input.value,
            note: input.note,
            created_at: now_stamp(),
        };
        surface.events.push(event.clone());
        surface.updated_at = now_stamp();
        self.write_surface(&surface)?;
        Ok(json!({
            "surface_id": surface.id,
            "event": event,
            "event_count": surface.events.len()
        }))
    }

    fn promote_surface(&self, id: &str, note: Option<String>) -> Result<Value, String> {
        let mut surface = self.read_surface_file(id)?;
        surface.ephemeral = false;
        surface.promoted_at = Some(now_stamp());
        if let Some(note) = note {
            surface.promotion = note;
        }
        surface.updated_at = now_stamp();
        self.write_surface(&surface)?;
        Ok(self.surface_response(&surface, true, true))
    }

    fn read_surface(
        &self,
        id: &str,
        include_spec: bool,
        include_events: bool,
    ) -> Result<Value, String> {
        let surface = self.read_surface_file(id)?;
        Ok(self.surface_response(&surface, include_spec, include_events))
    }

    fn list_surfaces(&self, limit: usize) -> Result<Vec<Value>, String> {
        let mut surfaces = Vec::new();
        let dir = self.root.join("surfaces");
        for entry in fs::read_dir(dir).map_err(|error| error.to_string())? {
            let entry = entry.map_err(|error| error.to_string())?;
            if entry.path().extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            let text = fs::read_to_string(entry.path()).map_err(|error| error.to_string())?;
            let surface: Surface =
                serde_json::from_str(&text).map_err(|error| error.to_string())?;
            surfaces.push(self.surface_response(&surface, false, false));
        }
        surfaces.sort_by(|a, b| {
            b.get("updated_at")
                .and_then(Value::as_str)
                .cmp(&a.get("updated_at").and_then(Value::as_str))
        });
        surfaces.truncate(limit);
        Ok(surfaces)
    }

    fn register_asset(&self, input: AssetInput) -> Result<AssetRecord, String> {
        let _surface = self.read_surface_file(&input.surface_id)?;
        let source = PathBuf::from(&input.path);
        if !source.is_file() {
            return Err("asset path must be an existing file".into());
        }
        let bytes = fs::read(&source).map_err(|error| error.to_string())?;
        let Some((ext, mime_type)) = image_signature(&bytes) else {
            return Err("asset must be an image file".into());
        };
        let sha256 = sha256_hex(&bytes);
        let id = new_id("asset");
        let file_name = format!("{id}.{ext}");
        let stored_path = Path::new(".canmore-medium")
            .join("assets")
            .join(&file_name)
            .to_string_lossy()
            .replace('\\', "/");
        let target = self.root.join("assets").join(&file_name);
        fs::write(&target, &bytes).map_err(|error| error.to_string())?;
        let byte_len = bytes.len() as u64;
        let record = AssetRecord {
            id,
            surface_id: input.surface_id,
            file_name,
            stored_path,
            byte_len,
            mime_type: mime_type.to_string(),
            sha256,
            note: input.note,
            created_at: now_stamp(),
        };
        let mut line = serde_json::to_string(&record).map_err(|error| error.to_string())?;
        line.push('\n');
        fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.root.join("assets.jsonl"))
            .and_then(|mut file| file.write_all(line.as_bytes()))
            .map_err(|error| error.to_string())?;
        Ok(record)
    }

    fn surface_response(
        &self,
        surface: &Surface,
        include_spec: bool,
        include_events: bool,
    ) -> Value {
        let mut value = json!({
            "id": surface.id,
            "title": surface.title,
            "purpose": surface.purpose,
            "medium": surface.medium,
            "ephemeral": surface.ephemeral,
            "promoted": surface.promoted_at.is_some(),
            "card_count": surface.cards.len(),
            "event_count": surface.events.len(),
            "updated_at": surface.updated_at,
            "viewer_path": format!(".canmore-medium/surfaces/{}.html", surface.id)
        });
        if include_spec {
            value["cards"] = serde_json::to_value(&surface.cards).unwrap();
            value["promotion"] = json!(surface.promotion);
        }
        if include_events {
            value["events"] = serde_json::to_value(&surface.events).unwrap();
        }
        value
    }

    fn read_surface_file(&self, id: &str) -> Result<Surface, String> {
        validate_id(id)?;
        let text = fs::read_to_string(self.surface_json_path(id))
            .map_err(|_| "surface not found".to_string())?;
        serde_json::from_str(&text).map_err(|error| error.to_string())
    }

    fn write_surface(&self, surface: &Surface) -> Result<(), String> {
        validate_id(&surface.id)?;
        let json_path = self.surface_json_path(&surface.id);
        let html_path = self.surface_html_path(&surface.id);
        let json_text = serde_json::to_string_pretty(surface).map_err(|error| error.to_string())?;
        fs::write(json_path, json_text).map_err(|error| error.to_string())?;
        fs::write(html_path, render_surface_html(surface)).map_err(|error| error.to_string())?;
        Ok(())
    }

    fn surface_json_path(&self, id: &str) -> PathBuf {
        self.root.join("surfaces").join(format!("{id}.json"))
    }

    fn surface_html_path(&self, id: &str) -> PathBuf {
        self.root.join("surfaces").join(format!("{id}.html"))
    }
}

fn recipe(medium: Option<&str>) -> Value {
    let medium = medium.unwrap_or("decision-board");
    json!({
        "kind": "medium",
        "medium": medium,
        "ephemeral": true,
        "template": {
            "title": "Choose the medium",
            "purpose": "Make the next step visible instead of explaining it harder.",
            "medium": medium,
            "cards": [
                { "type": "diagram", "title": "System picture", "body": "Show the moving parts." },
                { "type": "chart", "title": "Control", "body": "Expose the variable worth tuning.", "value": 55 },
                { "type": "matrix", "title": "Decision", "body": "Compare options directly." },
                { "type": "image_gen", "title": "Asset direction", "body": "Generate externally with built-in image_gen, then register outputs." }
            ],
            "promotion": "Promote only if this surface becomes project material."
        }
    })
}

fn tools() -> Vec<Value> {
    vec![
        tool(
            "canmore_medium_recipe",
            "Return a compact template for an on-demand visual medium surface.",
            json!({
                "type": "object",
                "properties": { "medium": { "type": "string" } }
            }),
        ),
        tool(
            "canmore_medium_create",
            "Create an ephemeral medium surface.",
            json!({
                "type": "object",
                "required": ["title", "purpose", "medium"],
                "properties": {
                    "title": { "type": "string" },
                    "purpose": { "type": "string" },
                    "medium": { "type": "string" },
                    "ephemeral": { "type": "boolean" },
                    "promotion": { "type": "string" },
                    "cards": { "type": "array" }
                }
            }),
        ),
        tool(
            "canmore_medium_update",
            "Replace medium surface fields without changing its identity.",
            json!({
                "type": "object",
                "required": ["id"],
                "properties": {
                    "id": { "type": "string" },
                    "title": { "type": "string" },
                    "purpose": { "type": "string" },
                    "medium": { "type": "string" },
                    "ephemeral": { "type": "boolean" },
                    "promotion": { "type": "string" },
                    "cards": { "type": "array" }
                }
            }),
        ),
        tool(
            "canmore_medium_event",
            "Record a structured interaction or user feedback event from a surface.",
            json!({
                "type": "object",
                "required": ["id", "kind"],
                "properties": {
                    "id": { "type": "string" },
                    "kind": { "type": "string" },
                    "target": { "type": "string" },
                    "value": {},
                    "note": { "type": "string" }
                }
            }),
        ),
        tool(
            "canmore_medium_read",
            "Read compact surface state, optionally including cards or events.",
            json!({
                "type": "object",
                "required": ["id"],
                "properties": {
                    "id": { "type": "string" },
                    "include_spec": { "type": "boolean" },
                    "include_events": { "type": "boolean" }
                }
            }),
        ),
        tool(
            "canmore_medium_list",
            "List recent medium surfaces compactly.",
            json!({
                "type": "object",
                "properties": { "limit": { "type": "integer", "minimum": 1, "maximum": 100 } }
            }),
        ),
        tool(
            "canmore_medium_promote",
            "Mark a surface as durable project material.",
            json!({
                "type": "object",
                "required": ["id"],
                "properties": {
                    "id": { "type": "string" },
                    "note": { "type": "string" }
                }
            }),
        ),
        tool(
            "canmore_image_asset_register",
            "Register a local output produced by built-in image_gen.",
            json!({
                "type": "object",
                "required": ["surface_id", "path"],
                "properties": {
                    "surface_id": { "type": "string" },
                    "path": { "type": "string" },
                    "note": { "type": "string" }
                }
            }),
        ),
    ]
}

fn tool(name: &str, description: &str, input_schema: Value) -> Value {
    json!({ "name": name, "description": description, "inputSchema": input_schema })
}

fn render_surface_html(surface: &Surface) -> String {
    let cards = surface
        .cards
        .iter()
        .map(|card| {
            let class = class_token(&card.kind);
            let meter = card.value.map_or(String::new(), |value| {
                format!(
                    "<div class=\"meter\"><span style=\"width:{}%\"></span></div>",
                    value.clamp(0, 100)
                )
            });
            format!(
                "<article class=\"card card-{}\"><span>{}</span><h2>{}</h2><p>{}</p>{}</article>",
                class,
                escape(&card.kind),
                escape(&card.title),
                escape(&card.body),
                meter
            )
        })
        .collect::<Vec<_>>()
        .join("");
    let events = surface
        .events
        .iter()
        .rev()
        .take(12)
        .map(|event| {
            let target = event.target.as_deref().unwrap_or("surface");
            format!("<span>{}: {}</span>", escape(&event.kind), escape(target))
        })
        .collect::<Vec<_>>()
        .join("");
    let event_body = if events.is_empty() {
        "<em>No feedback events recorded yet.</em>".to_string()
    } else {
        events
    };
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{}</title>
  <style>
    :root {{ color-scheme: light dark; font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", system-ui, sans-serif; }}
    body {{ margin: 0; background: #101613; color: #e5fff6; }}
    main {{ max-width: 1060px; margin: 0 auto; padding: 30px; }}
    header, .card, .events, .promotion {{ border: 1px solid #26342f; border-radius: 8px; background: #151d1a; }}
    header {{ display: grid; grid-template-columns: minmax(0, 1fr) auto; gap: 18px; padding: 18px; }}
    h1 {{ margin: 4px 0 8px; font-size: clamp(26px, 4vw, 44px); line-height: 1; }}
    p {{ color: #9eb2aa; line-height: 1.5; }}
    .eyebrow, .card span, .events strong, .promotion strong {{ color: #34d399; font-size: 11px; font-weight: 700; text-transform: uppercase; }}
    .badge {{ align-self: start; padding: 5px 9px; border: 1px solid #36584b; border-radius: 999px; color: #bdf8df; font-size: 12px; }}
    .medium-board {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(210px, 1fr)); gap: 12px; margin: 14px 0; }}
    .card {{ min-height: 136px; padding: 15px; }}
    .card h2 {{ margin: 10px 0 8px; font-size: 17px; }}
    .card-diagram {{ grid-column: span 2; }}
    .card-matrix {{ background: linear-gradient(135deg, #151d1a 0%, #17231f 100%); }}
    .card-chart .meter {{ height: 10px; }}
    .card-image-gen {{ border-color: #456354; }}
    .meter {{ height: 7px; margin-top: 14px; overflow: hidden; border-radius: 999px; background: #26342f; }}
    .meter span {{ display: block; height: 100%; background: #34d399; }}
    .events, .promotion {{ display: grid; gap: 10px; margin-top: 12px; padding: 14px; }}
    .events div {{ display: flex; flex-wrap: wrap; gap: 8px; }}
    .events span {{ padding: 5px 8px; border: 1px solid #36584b; border-radius: 999px; color: #d9fff0; font-size: 12px; }}
    .events em {{ color: #9eb2aa; font-style: normal; }}
    @media (max-width: 620px) {{ .card-diagram {{ grid-column: span 1; }} main {{ padding: 18px; }} }}
  </style>
</head>
<body>
  <main data-medium="{}">
    <header>
      <div><span class="eyebrow">Canmore medium</span><h1>{}</h1><p>{}</p></div>
      <strong class="badge">{}</strong>
    </header>
    <section class="medium-board">{}</section>
    <section class="events"><strong>Feedback events</strong><div>{}</div></section>
    <section class="promotion"><strong>Promotion</strong><p>{}</p></section>
  </main>
</body>
</html>"#,
        escape(&surface.title),
        escape(&surface.medium),
        escape(&surface.title),
        escape(&surface.purpose),
        escape(&surface.medium),
        cards,
        event_body,
        escape(&surface.promotion)
    )
}

fn validate_id(id: &str) -> Result<(), String> {
    let ok = id
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-');
    if ok && !id.is_empty() {
        Ok(())
    } else {
        Err("invalid id".into())
    }
}

fn new_id(prefix: &str) -> String {
    let count = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    format!("{prefix}_{}_{}", unix_millis(), count)
}

fn now_stamp() -> String {
    unix_millis().to_string()
}

fn unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn class_token(input: &str) -> String {
    let token = input
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    if token.is_empty() {
        "surface".to_string()
    } else {
        token
    }
}

fn image_signature(bytes: &[u8]) -> Option<(&'static str, &'static str)> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]) {
        return Some(("png", "image/png"));
    }
    if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        return Some(("jpg", "image/jpeg"));
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Some(("gif", "image/gif"));
    }
    if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        return Some(("webp", "image/webp"));
    }
    None
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        output.push_str(&format!("{byte:02x}"));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surface_lifecycle_is_compact_until_full_read() {
        let temp = tempfile_dir();
        let store = Store::open(temp.clone()).unwrap();
        let created = store
            .create_surface(SurfaceInput {
                title: "Decision".into(),
                purpose: "Use a visual medium.".into(),
                medium: "matrix".into(),
                ephemeral: true,
                promotion: None,
                cards: vec![Card {
                    id: None,
                    kind: "matrix".into(),
                    title: "Options".into(),
                    body: "Compare directly.".into(),
                    value: None,
                }],
            })
            .unwrap();
        let id = created["id"].as_str().unwrap().to_string();
        assert_eq!(created["card_count"], 1);
        assert!(created.get("cards").is_none());

        store
            .record_event(EventInput {
                id: id.clone(),
                kind: "selected variant".into(),
                target: Some("Options".into()),
                value: None,
                note: None,
            })
            .unwrap();
        let compact = store.read_surface(&id, false, false).unwrap();
        let full = store.read_surface(&id, true, true).unwrap();

        assert_eq!(compact["event_count"], 1);
        assert!(compact.get("events").is_none());
        assert_eq!(full["cards"].as_array().unwrap().len(), 1);
        assert_eq!(full["events"].as_array().unwrap().len(), 1);
        assert!(
            temp.join(".canmore-medium")
                .join("surfaces")
                .join(format!("{id}.html"))
                .is_file()
        );
    }

    #[test]
    fn recipe_names_the_medium_layer() {
        let recipe = recipe(Some("diagram"));
        assert_eq!(recipe["kind"], "medium");
        assert_eq!(recipe["ephemeral"], true);
        assert_eq!(recipe["template"]["medium"], "diagram");
        assert!(recipe.to_string().contains("Promote only if"));
    }

    #[test]
    fn notifications_do_not_emit_rpc_responses() {
        let store = Store::open(tempfile_dir()).unwrap();
        let response = handle_rpc(
            &store,
            r#"{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}"#,
        );
        assert!(response.is_none());
    }

    #[test]
    fn surfaces_reject_blank_identity_fields() {
        let store = Store::open(tempfile_dir()).unwrap();
        let error = store
            .create_surface(SurfaceInput {
                title: "Decision".into(),
                purpose: "Purpose".into(),
                medium: " ".into(),
                ephemeral: true,
                promotion: None,
                cards: Vec::new(),
            })
            .unwrap_err();
        assert_eq!(error, "medium is required");
    }

    #[test]
    fn image_asset_registration_checks_bytes_and_records_hash() {
        let temp = tempfile_dir();
        let store = Store::open(temp.clone()).unwrap();
        let created = store
            .create_surface(SurfaceInput {
                title: "Assets".into(),
                purpose: "Register generated files.".into(),
                medium: "image-board".into(),
                ephemeral: true,
                promotion: None,
                cards: Vec::new(),
            })
            .unwrap();
        let id = created["id"].as_str().unwrap().to_string();
        let fake = temp.join("fake.png");
        fs::write(&fake, b"not an image").unwrap();
        let fake_error = store
            .register_asset(AssetInput {
                surface_id: id.clone(),
                path: fake.to_string_lossy().to_string(),
                note: None,
            })
            .unwrap_err();
        assert_eq!(fake_error, "asset must be an image file");

        let real = temp.join("real.png");
        fs::write(&real, b"\x89PNG\r\n\x1a\n").unwrap();
        let record = store
            .register_asset(AssetInput {
                surface_id: id,
                path: real.to_string_lossy().to_string(),
                note: Some("built-in image_gen output".into()),
            })
            .unwrap();
        assert_eq!(record.mime_type, "image/png");
        assert_eq!(record.sha256.len(), 64);
        assert!(temp.join(record.stored_path).is_file());
    }

    fn tempfile_dir() -> PathBuf {
        let path = std::env::temp_dir().join(format!("canmore_test_{}", unix_millis()));
        fs::create_dir_all(&path).unwrap();
        path
    }
}
