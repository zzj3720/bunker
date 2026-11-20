// What the core says, as the UI keeps it: values compared by what they hold (signals stop at an equal one).

///|
/// An SPT install Bunker knows (`instances.list`), or one a path names (`instances.probe`).
#derive(Eq, Debug)
pub(all) struct Inst {
  root : String
  name : String
  /// `spt_runtime` (4.1+), `spt` (3.11 / 4.0), `root`
  layout : String
  server_rel : String
  spt_version : String
  eft_version : String
  bepinex_version : String
  server_mods : Int
  client_mods : Int
  profiles : Int
  managed : Int
  port : Int
  ok : Bool
  current : Bool
  problems : Array[String]
}

///|
/// A mod of the current instance, its server and client parts as one (`mods.list`).
#derive(Eq, Debug)
pub(all) struct ModItem {
  id : String
  name : String
  version : String
  author : String
  server : Bool
  client : Bool
  enabled : Bool
  managed : Bool
  roots : Array[String]
  files : Int
  /// `full`, `partial`, `none`, `na`
  loc : String
  loc_ch : Int
  source_kind : String
  source_url : String
  /// `ok`, `mismatch`, `unknown`
  compat : String
  spt_range : String
  /// a newer release's version (`mods.check_updates`), or ``
  update : String
  /// its client assemblies against this SPT: `compatible`, `incompatible`, `unknown`, `` (none)
  dll_status : String
  /// the SPT they were built for (`4.0.1`)
  built_for : String
  bad_dlls : Array[BadDll]
  /// mods turned on that need it, and those it needs
  required_by : Array[ModRef]
  requires : Array[ModRef]
  /// where an update built for this SPT comes from, ``: none known
  fix_url : String
}

///|
/// A client assembly not built for the instance's SPT.
#derive(Eq, Debug)
pub(all) struct BadDll {
  path : String
  /// as SPT's warning names it: `UnityToolkit\UnityToolkit.dll`
  shown : String
  built_for : String
}

///|
#derive(Eq, Debug)
pub(all) struct ModRef {
  id : String
  name : String
}

///|
pub fn ModItem::incompatible(self : ModItem) -> Bool {
  self.enabled && self.dll_status == "incompatible"
}

///|
/// The names of the mods that need it: `ManimalIcebreaker、ManimalInterchange`.
pub fn ModItem::needed_by(self : ModItem) -> String {
  self.required_by.map(fn(r) { r.name }).join("、")
}

///|
#derive(Eq, Debug)
pub(all) struct Snap {
  id : String
  note : String
  /// `manual`, `before_install`, `before_restore`
  kind : String
  created_at : Double
  files : Int
  bytes : Double
  characters : Array[String]
}

///|
/// A character of the instance (its profile files).
#derive(Eq, Debug)
pub(all) struct Character {
  profile_id : String
  username : String
  nickname : String
  side : String
  edition : String
  level : Int
  roubles : Double
  started : Bool
}

///|
#derive(Eq, Debug)
pub(all) struct ServerStatus {
  /// `stopped`, `starting`, `running`, `stopping`, `failed`, `external`
  state : String
  pid : Int
  port : Int
  started_at : Double
  ready_at : Double
  warns : Int
  errors : Int
  message : String
}

///|
pub let server_unknown : ServerStatus = {
  state: "stopped",
  pid: 0,
  port: 6969,
  started_at: 0.0,
  ready_at: 0.0,
  warns: 0,
  errors: 0,
  message: "",
}

///|
#derive(Eq, Debug)
pub(all) struct LogLine {
  n : Int
  t : String
  /// `info`, `ok`, `warn`, `error`
  level : String
  text : String
}

///|
#derive(Eq, Debug)
pub(all) struct ConfigFile {
  path : String
  /// `json`, `jsonc`, `cfg`
  kind : String
  owner : String
  bytes : Double
}

///|
#derive(Eq, Debug)
pub(all) struct Mapping {
  from : String
  to : String
  side : String
  files : Int
}

///|
/// An install's plan: where each entry of the archive goes in the instance (`mods.plan_*`).
#derive(Eq, Debug)
pub(all) struct Plan {
  id : String
  archive : String
  name : String
  version : String
  target : String
  replaces : String
  adapted : Bool
  mappings : Array[Mapping]
  conflicts : Array[String]
  skipped : Array[String]
  warnings : Array[String]
  files : Int
}

// ---- read from the core's JSON ----

///|
fn field(j : Json, k : String) -> Json {
  match j {
    Object(o) =>
      match o.get(k) {
        Some(v) => v
        None => Json::null()
      }
    _ => Json::null()
  }
}

///|
pub fn jstr(j : Json, k : String) -> String {
  match field(j, k) {
    String(s) => s
    _ => ""
  }
}

///|
pub fn jnum(j : Json, k : String) -> Double {
  match field(j, k) {
    Number(n, ..) => n
    _ => 0.0
  }
}

///|
pub fn jint(j : Json, k : String) -> Int {
  jnum(j, k).to_int()
}

///|
pub fn jbool(j : Json, k : String) -> Bool {
  match field(j, k) {
    True => true
    _ => false
  }
}

///|
pub fn jarr(j : Json, k : String) -> Array[Json] {
  match field(j, k) {
    Array(a) => a
    _ => []
  }
}

///|
pub fn jobj(j : Json, k : String) -> Json {
  field(j, k)
}

///|
fn strings(a : Array[Json]) -> Array[String] {
  a.map(fn(x) {
    match x {
      String(s) => s
      _ => ""
    }
  })
}

///|
pub fn Inst::from_json(j : Json) -> Inst {
  {
    root: jstr(j, "root"),
    name: jstr(j, "name"),
    layout: jstr(j, "layout"),
    server_rel: jstr(j, "server_rel"),
    spt_version: jstr(j, "spt_version"),
    eft_version: jstr(j, "eft_version"),
    bepinex_version: jstr(j, "bepinex_version"),
    server_mods: jint(j, "server_mods"),
    client_mods: jint(j, "client_mods"),
    profiles: jint(j, "profiles"),
    managed: jint(j, "mods"),
    port: jint(j, "port"),
    ok: jbool(j, "has_server") && jbool(j, "has_game"),
    current: jbool(j, "current"),
    problems: strings(jarr(j, "problems")),
  }
}

///|
/// The layout said for people: where its server and its mods are.
pub fn Inst::layout_text(self : Inst) -> String {
  match self.layout {
    "spt_runtime" => "SPT_Runtime"
    "spt" => "SPT/"
    _ => "根目录"
  }
}

///|
pub fn ModItem::from_json(j : Json) -> ModItem {
  let loc = jobj(j, "loc")
  {
    id: jstr(j, "id"),
    name: jstr(j, "name"),
    version: jstr(j, "version"),
    author: jstr(j, "author"),
    server: jbool(j, "server"),
    client: jbool(j, "client"),
    enabled: jbool(j, "enabled"),
    managed: jbool(j, "managed"),
    roots: strings(jarr(j, "roots")),
    files: jint(j, "files"),
    loc: jstr(loc, "status"),
    loc_ch: jint(loc, "ch"),
    source_kind: jstr(j, "source_kind"),
    source_url: jstr(j, "source_url"),
    compat: jstr(j, "compat"),
    spt_range: jstr(j, "spt_range"),
    update: "",
    dll_status: jstr(j, "dll_status"),
    built_for: jstr(j, "built_for"),
    bad_dlls: jarr(j, "bad_dlls").map(fn(b) {
      { path: jstr(b, "path"), shown: jstr(b, "shown"), built_for: jstr(b, "built_for") }
    }),
    required_by: jarr(j, "required_by").map(fn(r) { { id: jstr(r, "id"), name: jstr(r, "name") } }),
    requires: jarr(j, "requires").map(fn(r) { { id: jstr(r, "id"), name: jstr(r, "name") } }),
    fix_url: jstr(j, "fix_url"),
  }
}

///|
pub fn Snap::from_json(j : Json) -> Snap {
  {
    id: jstr(j, "id"),
    note: jstr(j, "note"),
    kind: jstr(j, "kind"),
    created_at: jnum(j, "created_at"),
    files: jarr(j, "files").length(),
    bytes: jnum(j, "bytes"),
    characters: strings(jarr(j, "characters")),
  }
}

///|
pub fn Character::from_json(j : Json) -> Character {
  {
    profile_id: jstr(j, "profile_id"),
    username: jstr(j, "username"),
    nickname: jstr(j, "nickname"),
    side: jstr(j, "side"),
    edition: jstr(j, "edition"),
    level: jint(j, "level"),
    roubles: jnum(j, "roubles"),
    started: jbool(j, "started"),
  }
}

///|
/// The name it is shown by: its nickname in game, else its username (a profile not started yet).
pub fn Character::shown(self : Character) -> String {
  if self.nickname == "" {
    self.username
  } else {
    self.nickname
  }
}

///|
pub fn ServerStatus::from_json(j : Json) -> ServerStatus {
  {
    state: jstr(j, "state"),
    pid: jint(j, "pid"),
    port: jint(j, "port"),
    started_at: jnum(j, "started_at"),
    ready_at: jnum(j, "ready_at"),
    warns: jint(j, "warns"),
    errors: jint(j, "errors"),
    message: jstr(j, "message"),
  }
}

///|
pub fn LogLine::from_json(j : Json) -> LogLine {
  { n: jint(j, "n"), t: jstr(j, "t"), level: jstr(j, "level"), text: jstr(j, "text") }
}

///|
pub fn ConfigFile::from_json(j : Json) -> ConfigFile {
  { path: jstr(j, "path"), kind: jstr(j, "kind"), owner: jstr(j, "owner"), bytes: jnum(j, "bytes") }
}

///|
pub fn Plan::from_json(j : Json) -> Plan {
  {
    id: jstr(j, "id"),
    archive: jstr(j, "archive"),
    name: jstr(j, "name"),
    version: jstr(j, "version"),
    target: jstr(j, "target"),
    replaces: jstr(j, "replaces"),
    adapted: jbool(j, "adapted"),
    mappings: jarr(j, "mappings").map(fn(m) {
      { from: jstr(m, "from"), to: jstr(m, "to"), side: jstr(m, "side"), files: jint(m, "files") }
    }),
    conflicts: jarr(j, "conflicts").map(fn(c) { jstr(c, "path") }),
    skipped: strings(jarr(j, "skipped")),
    warnings: strings(jarr(j, "warnings")),
    files: jint(j, "files"),
  }
}

///|
/// Roubles said short: `14.2M ₽`, `850K ₽`.
pub fn roubles_text(r : Double) -> String {
  if r >= 1000000.0 {
    let tenths = (r / 100000.0).to_int()
    "\{tenths / 10}.\{tenths % 10}M ₽"
  } else if r >= 1000.0 {
    "\{(r / 1000.0).to_int()}K ₽"
  } else {
    "\{r.to_int()} ₽"
  }
}

///|
/// A count with thousands' commas: `3,366`.
pub fn count_text(n : Int) -> String {
  let s = n.to_string()
  let out = StringBuilder()
  let len = s.length()
  for i = 0; i < len; i = i + 1 {
    if i > 0 && (len - i) % 3 == 0 {
      out.write_char(',')
    }
    out.write_char(s[i].unsafe_to_char())
  }
  out.to_string()
}

///|
/// A moment said as local time, `11-20 14:09`, `offset_min` minutes east of UTC.
pub fn moment_text(ms : Double, offset_min : Int) -> String {
  let secs = (ms / 1000.0).to_int64() + offset_min.to_int64() * 60L
  let days = if secs >= 0L { secs / 86400L } else { (secs - 86399L) / 86400L }
  let t = secs - days * 86400L
  // (Howard Hinnant's days-to-civil)
  let z = days + 719468L
  let era = (if z >= 0L { z } else { z - 146096L }) / 146097L
  let doe = z - era * 146097L
  let yoe = (doe - doe / 1460L + doe / 36524L - doe / 146096L) / 365L
  let doy = doe - (365L * yoe + yoe / 4L - yoe / 100L)
  let mp = (5L * doy + 2L) / 153L
  let d = doy - (153L * mp + 2L) / 5L + 1L
  let m = if mp < 10L { mp + 3L } else { mp - 9L }
  let two = fn(x : Int64) { if x < 10L { "0\{x}" } else { x.to_string() } }
  "\{two(m)}-\{two(d)} \{two(t / 3600L)}:\{two(t / 60L % 60L)}"
}

///|
/// Bytes said short: `12.4 MB`.
pub fn bytes_text(b : Double) -> String {
  if b >= 1048576.0 {
    let tenths = (b / 104857.6).to_int()
    "\{tenths / 10}.\{tenths % 10} MB"
  } else if b >= 1024.0 {
    "\{(b / 1024.0).to_int()} KB"
  } else {
    "\{b.to_int()} B"
  }
}
