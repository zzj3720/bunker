// What the UI shows, kept as signals, and what it asks the core: each request sent with an id and its answer taken
// as it comes on the "core" channel (`Store::receive`), with the core's own events (a server's log line, its state,
// the game's). Requests are sent from handlers (a press, an answer), never while the tree is built.

///|
/// How requests reach the core: the app's (corelink, the Rust core linked in) or a test's.
pub(all) struct Core {
  call : (Int64, String) -> Unit
  call_data : (Int64, String, Bytes) -> Unit
}

///|
#derive(Eq, Debug)
pub(all) enum Page {
  Play
  Mods
  Snapshots
  Config
}

///|
/// A notice at the window's foot: what was done or what went wrong.
#derive(Eq, Debug)
pub(all) struct Notice {
  n : Int
  text : String
  bad : Bool
}

///|
pub struct Store {
  core : Core
  priv next_id : Ref[Int64]
  priv pending : Map[Int64, (Bool, Json) -> Unit]
  priv offset_min : Int
  // the instances
  instances : @signals.Signal[Array[Inst]]
  current : @signals.Signal[String]
  // the current instance's
  mods : @signals.Signal[Array[ModItem]]
  mods_loaded : @signals.Signal[Bool]
  snapshots : @signals.Signal[Array[Snap]]
  characters : @signals.Signal[Array[Character]]
  character : @signals.Signal[String]
  server : @signals.Signal[ServerStatus]
  log : @signals.Signal[Array[LogLine]]
  log_filter : @signals.Signal[String]
  configs : @signals.Signal[Array[ConfigFile]]
  config_path : @signals.Signal[String]
  config_text : @signals.Signal[String]
  config_check : @signals.Signal[String]
  game : @signals.Signal[String]
  // settings (kept by the core)
  clean_cache : @signals.Signal[Bool]
  minimize : @signals.Signal[Bool]
  // where people are
  page : @signals.Signal[Page]
  mods_filter : @signals.Signal[String]
  mods_query : @signals.Signal[String]
  inst_open : @signals.Signal[Bool]
  inst_step : @signals.Signal[Int]
  probe : @signals.Signal[Inst?]
  probe_path : @signals.Signal[String]
  found : @signals.Signal[Array[Inst]]
  role_open : @signals.Signal[Bool]
  install_open : @signals.Signal[Bool]
  install_step : @signals.Signal[Int]
  plan : @signals.Signal[Plan?]
  /// what is under way (`installing`, `planning`, `starting`…), ``: nothing
  busy : @signals.Signal[String]
  notice : @signals.Signal[Notice?]
}

///|
pub fn Store::new(core : Core, offset_min? : Int = 0) -> Store {
  {
    core,
    next_id: Ref(1L),
    pending: Map([]),
    offset_min,
    instances: @signals.signal([]),
    current: @signals.signal(""),
    mods: @signals.signal([]),
    mods_loaded: @signals.signal(false),
    snapshots: @signals.signal([]),
    characters: @signals.signal([]),
    character: @signals.signal(""),
    server: @signals.signal(server_unknown),
    log: @signals.signal([]),
    log_filter: @signals.signal("all"),
    configs: @signals.signal([]),
    config_path: @signals.signal(""),
    config_text: @signals.signal(""),
    config_check: @signals.signal(""),
    game: @signals.signal("idle"),
    clean_cache: @signals.signal(true),
    minimize: @signals.signal(false),
    page: @signals.signal(Play),
    mods_filter: @signals.signal("all"),
    mods_query: @signals.signal(""),
    inst_open: @signals.signal(false),
    inst_step: @signals.signal(1),
    probe: @signals.signal(None),
    probe_path: @signals.signal(""),
    found: @signals.signal([]),
    role_open: @signals.signal(false),
    install_open: @signals.signal(false),
    install_step: @signals.signal(1),
    plan: @signals.signal(None),
    busy: @signals.signal(""),
    notice: @signals.signal(None),
  }
}

///|
pub fn Store::moment(self : Store, ms : Double) -> String {
  moment_text(ms, self.offset_min)
}

///|
fn obj(pairs : Array[(String, Json)]) -> Json {
  let m : Map[String, Json] = Map([])
  for p in pairs {
    m[p.0] = p.1
  }
  Json::object(m)
}

///|
/// Sends `op` with `args`; `done` given whether it went well and its answer (the data, or the error's words).
pub fn Store::ask(
  self : Store,
  op : String,
  args : Array[(String, Json)],
  done : (Bool, Json) -> Unit,
  data? : Bytes,
) -> Unit {
  let id = self.next_id.val
  self.next_id.val = id + 1L
  self.pending[id] = done
  let req = obj([("op", Json::string(op)), ..args])
  match data {
    Some(d) => (self.core.call_data)(id, req.stringify(), d)
    None => (self.core.call)(id, req.stringify())
  }
}

///|
/// As `ask`, an error said in the notice.
fn Store::run(self : Store, op : String, args : Array[(String, Json)], ok : (Json) -> Unit, data? : Bytes) -> Unit {
  self.ask(op, args, data?, fn(good, v) {
    if good {
      ok(v)
    } else {
      self.say(
        match v {
          String(s) => s
          _ => "操作失败"
        },
        bad=true,
      )
    }
  })
}

///|
pub fn Store::say(self : Store, text : String, bad? : Bool = false) -> Unit {
  let n = match self.notice.untracked() {
    Some(x) => x.n + 1
    None => 1
  }
  self.notice.set(Some({ n, text, bad }))
}

///|
fn Store::inst_arg(self : Store) -> (String, Json) {
  ("instance", Json::string(self.current.untracked()))
}

///|
/// A message from the core: a request's answer, or one of its events. Whether it was the core's to take.
pub fn Store::receive(self : Store, text : String) -> Bool {
  let j = @json.parse(text) catch { _ => return false }
  match jobj(j, "id") {
    Number(n, ..) => {
      let id = n.to_int64()
      match self.pending.get(id) {
        Some(done) => {
          self.pending.remove(id)
          if jbool(j, "ok") {
            done(true, jobj(j, "data"))
          } else {
            done(false, Json::string(jstr(j, "error")))
          }
        }
        None => ()
      }
      return true
    }
    _ => ()
  }
  let root = self.current.untracked()
  match jstr(j, "ev") {
    "log" =>
      if jstr(j, "instance") == root {
        let line = LogLine::from_json(jobj(j, "line"))
        let old = self.log.untracked()
        let kept = if old.length() >= log_keep { old[old.length() - log_keep + 1:].to_owned() } else { old.copy() }
        kept.push(line)
        self.log.set(kept)
        let s = self.server.untracked()
        self.server.set({ ..s, warns: jint(j, "warns"), errors: jint(j, "errors") })
      }
    "server" => {
      let st = jobj(j, "status")
      if jstr(st, "instance") == root {
        let s = ServerStatus::from_json(st)
        self.server.set(s)
        if s.state == "running" {
          self.load_characters()
        }
      }
    }
    "game" =>
      if jstr(j, "instance") == root {
        self.game.set(jstr(j, "state"))
        if jstr(j, "state") == "exited" {
          self.say("游戏已退出")
        }
      }
    _ => return false
  }
  true
}

///|
/// The log lines kept for the console.
pub let log_keep : Int = 600

// ---- what is asked ----

///|
/// Everything read anew: the instances, then the current one's mods, characters, server, snapshots and configs.
pub fn Store::start(self : Store) -> Unit {
  self.run("settings.get", [], fn(v) {
    match jobj(v, "clean_cache") {
      True => self.clean_cache.set(true)
      False => self.clean_cache.set(false)
      _ => ()
    }
    self.minimize.set(jbool(v, "minimize"))
  })
  self.load_instances(then=fn() { self.load_current() })
}

///|
pub fn Store::load_instances(self : Store, then? : () -> Unit = fn() {  }) -> Unit {
  self.run("instances.list", [], fn(v) {
    let list = jarr(v, "instances").map(Inst::from_json)
    self.instances.set(list)
    self.current.set(jstr(v, "current"))
    then()
  })
}

///|
/// The current instance's own: read anew (after it is chosen, or something in it changed).
pub fn Store::load_current(self : Store) -> Unit {
  if self.current.untracked() == "" {
    return
  }
  self.load_mods()
  self.load_characters()
  self.load_snapshots()
  self.load_configs()
  self.run("server.status", [self.inst_arg()], fn(v) { self.server.set(ServerStatus::from_json(v)) })
  self.run("server.log", [self.inst_arg()], fn(v) {
    match v {
      Array(a) => self.log.set(a.map(LogLine::from_json))
      _ => ()
    }
  })
}

///|
pub fn Store::load_mods(self : Store) -> Unit {
  self.run("mods.list", [self.inst_arg()], fn(v) {
    let updates = self.mods.untracked().filter(fn(m) { m.update != "" })
    let list = jarr(v, "mods").map(fn(j) {
      let m = ModItem::from_json(j)
      match updates.search_by(fn(u) { u.id == m.id }) {
        Some(i) => { ..m, update: updates[i].update }
        None => m
      }
    })
    self.mods.set(list)
    self.mods_loaded.set(true)
  })
}

///|
pub fn Store::load_characters(self : Store) -> Unit {
  self.run("profiles.list", [self.inst_arg()], fn(v) {
    let list = jarr(v, "characters").map(Character::from_json)
    self.characters.set(list)
    let now = self.character.untracked()
    if !list.iter().any(fn(c) { c.username == now }) {
      self.character.set(
        match list.get(0) {
          Some(c) => c.username
          None => ""
        },
      )
    }
  })
}

///|
pub fn Store::load_snapshots(self : Store) -> Unit {
  self.run("snapshots.list", [self.inst_arg()], fn(v) {
    match v {
      Array(a) => self.snapshots.set(a.map(Snap::from_json))
      _ => ()
    }
  })
}

///|
pub fn Store::load_configs(self : Store) -> Unit {
  self.run("config.list", [self.inst_arg()], fn(v) {
    match v {
      Array(a) => self.configs.set(a.map(ConfigFile::from_json))
      _ => ()
    }
  })
}

///|
pub fn Store::select_instance(self : Store, root : String) -> Unit {
  self.inst_open.set(false)
  if root == self.current.untracked() {
    return
  }
  self.run("instances.select", [("path", Json::string(root))], fn(_) {
    self.current.set(root)
    self.mods.set([])
    self.mods_loaded.set(false)
    self.log.set([])
    self.server.set(server_unknown)
    self.config_path.set("")
    self.load_instances()
    self.load_current()
  })
}

///|
/// The add-instance step: SPT installs found where they usually are, and the path typed checked as it changes.
pub fn Store::open_import(self : Store) -> Unit {
  self.inst_step.set(2)
  self.probe.set(None)
  self.run("instances.scan", [], fn(v) {
    match v {
      Array(a) => self.found.set(a.map(Inst::from_json))
      _ => ()
    }
  })
}

///|
pub fn Store::probe_path(self : Store, path : String) -> Unit {
  self.probe_path.set(path)
  if path.trim(chars=" \t\r\n").is_empty() {
    self.probe.set(None)
    return
  }
  self.ask("instances.probe", [("path", Json::string(path))], fn(good, v) {
    // (the answer for the path still typed only)
    if good && self.probe_path.untracked() == path {
      self.probe.set(Some(Inst::from_json(v)))
    }
  })
}

///|
pub fn Store::add_instance(self : Store, path : String) -> Unit {
  self.run("instances.add", [("path", Json::string(path))], fn(_) {
    self.inst_open.set(false)
    self.inst_step.set(1)
    self.probe.set(None)
    self.say("已导入实例：\{path}")
    self.mods.set([])
    self.mods_loaded.set(false)
    self.log.set([])
    self.load_instances(then=fn() { self.load_current() })
  })
}

///|
pub fn Store::set_setting(self : Store, key : String, on : Bool) -> Unit {
  match key {
    "clean_cache" => self.clean_cache.set(on)
    _ => self.minimize.set(on)
  }
  self.run("settings.set", [("values", obj([(key, Json::boolean(on))]))], fn(_) {  })
}

///|
pub fn Store::set_mod_enabled(self : Store, id : String, on : Bool) -> Unit {
  // (shown at once; read back as the core answers, put back if it refused)
  self.mods.set(self.mods.untracked().map(fn(m) { if m.id == id { { ..m, enabled: on } } else { m } }))
  self.ask("mods.set_enabled", [self.inst_arg(), ("id", Json::string(id)), ("enabled", Json::boolean(on))], fn(
    good,
    v,
  ) {
    if !good {
      self.say(
        match v {
          String(s) => s
          _ => "操作失败"
        },
        bad=true,
      )
    } else {
      self.say(if on { "已启用" } else { "已安全禁用（文件暂存于 .bunker/disabled，可一键恢复）" })
    }
    self.load_mods()
  })
}

///|
pub fn Store::uninstall(self : Store, id : String) -> Unit {
  let name = match self.mods.untracked().search_by(fn(m) { m.id == id }) {
    Some(i) => self.mods.untracked()[i].name
    None => id
  }
  self.busy.set("uninstalling")
  self.ask("mods.uninstall", [self.inst_arg(), ("id", Json::string(id))], fn(good, v) {
    self.busy.set("")
    if good {
      self.say("已卸载 \{name}（\{jint(v, "removed")} 个文件，零残留）")
    } else {
      self.say(
        match v {
          String(s) => s
          _ => "卸载失败"
        },
        bad=true,
      )
    }
    self.load_mods()
    self.load_configs()
  })
}

///|
fn Store::planned(self : Store, good : Bool, v : Json) -> Unit {
  self.busy.set("")
  if good {
    self.plan.set(Some(Plan::from_json(v)))
    self.install_step.set(2)
  } else {
    self.say(
      match v {
        String(s) => s
        _ => "无法读取压缩包"
      },
      bad=true,
    )
  }
}

///|
/// A file let go on the window or picked: its plan asked (step 2).
pub fn Store::plan_bytes(self : Store, name : String, data : Bytes) -> Unit {
  self.install_open.set(true)
  self.busy.set("planning")
  self.ask("mods.plan_bytes", [self.inst_arg(), ("name", Json::string(name))], data~, fn(good, v) {
    self.planned(good, v)
  })
}

///|
pub fn Store::plan_url(self : Store, url : String) -> Unit {
  if url.trim(chars=" \t\r\n").is_empty() {
    return
  }
  self.busy.set("planning")
  self.ask("mods.plan_url", [self.inst_arg(), ("url", Json::string(url.trim(chars=" \t\r\n").to_owned()))], fn(
    good,
    v,
  ) {
    self.planned(good, v)
  })
}

///|
pub fn Store::install(self : Store, overwrite : Bool) -> Unit {
  guard self.plan.untracked() is Some(p) else { return }
  self.busy.set("installing")
  self.ask("mods.install", [self.inst_arg(), ("plan", Json::string(p.id)), ("overwrite", Json::boolean(overwrite))], fn(
    good,
    v,
  ) {
    self.busy.set("")
    if good {
      self.install_open.set(false)
      self.install_step.set(1)
      self.plan.set(None)
      self.say("已安装 \{p.name}（装前已自动快照存档）")
      self.load_mods()
      self.load_snapshots()
      self.load_configs()
    } else {
      self.say(
        match v {
          String(s) => s
          _ => "安装失败"
        },
        bad=true,
      )
    }
  })
}

///|
/// Back to step 1 (or closed): the plan's unpacked files let go.
pub fn Store::drop_plan(self : Store) -> Unit {
  match self.plan.untracked() {
    Some(p) => self.ask("mods.cancel_plan", [self.inst_arg(), ("plan", Json::string(p.id))], fn(_, _) {  })
    None => ()
  }
  self.plan.set(None)
  self.install_step.set(1)
}

///|
pub fn Store::check_updates(self : Store) -> Unit {
  self.busy.set("checking")
  self.ask("mods.check_updates", [self.inst_arg()], fn(good, v) {
    self.busy.set("")
    if !good {
      self.say(
        match v {
          String(s) => s
          _ => "检查更新失败"
        },
        bad=true,
      )
      return
    }
    let ups = jarr(v, "updates")
    self.mods.set(
      self.mods
      .untracked()
      .map(fn(m) {
        match ups.search_by(fn(u) { jstr(u, "id") == m.id }) {
          Some(i) => { ..m, update: jstr(ups[i], "latest") }
          None => m
        }
      }),
    )
    let checked = self.mods.untracked().filter(fn(m) { m.source_url != "" }).length()
    self.say(
      if ups.is_empty() {
        "已检查 \{checked} 个可追踪来源的 Mod：都是最新版"
      } else {
        "\{ups.length()} 个 Mod 有新版本"
      },
    )
  })
}

///|
pub fn Store::update_mod(self : Store, id : String) -> Unit {
  match self.mods.untracked().search_by(fn(m) { m.id == id }) {
    Some(i) => {
      self.install_open.set(true)
      self.plan_url(self.mods.untracked()[i].source_url)
    }
    None => ()
  }
}

///|
pub fn Store::snapshot(self : Store, note : String) -> Unit {
  self.run("snapshots.create", [self.inst_arg(), ("note", Json::string(note))], fn(v) {
    self.say("已备份存档（\{jarr(v, "files").length()} 个存档文件）")
    self.load_snapshots()
  })
}

///|
pub fn Store::restore(self : Store, id : String) -> Unit {
  self.run("snapshots.restore", [self.inst_arg(), ("id", Json::string(id))], fn(_) {
    self.say("已回滚到 \{id}（回滚前的存档已自动备份）")
    self.load_snapshots()
    self.load_characters()
  })
}

///|
pub fn Store::delete_snapshot(self : Store, id : String) -> Unit {
  self.run("snapshots.delete", [self.inst_arg(), ("id", Json::string(id))], fn(_) { self.load_snapshots() })
}

///|
pub fn Store::open_config(self : Store, path : String) -> Unit {
  self.config_path.set(path)
  self.config_check.set("")
  self.run("config.read", [self.inst_arg(), ("path", Json::string(path))], fn(v) {
    if self.config_path.untracked() == path {
      self.config_text.set(jstr(v, "text"))
    }
  })
}

///|
pub fn Store::check_config(self : Store, text : String) -> Unit {
  let path = self.config_path.untracked()
  self.run("config.check", [("path", Json::string(path)), ("text", Json::string(text))], fn(v) {
    self.config_check.set(if jbool(v, "ok") { "ok" } else { jstr(v, "error") })
  })
}

///|
pub fn Store::save_config(self : Store, text : String) -> Unit {
  let path = self.config_path.untracked()
  self.ask("config.save", [self.inst_arg(), ("path", Json::string(path)), ("text", Json::string(text))], fn(good, v) {
    if good {
      self.config_check.set("ok")
      self.config_text.set(text)
      self.say("已保存（原文件备份为 \{jstr(v, "backup")}）")
    } else {
      let e = match v {
        String(s) => s
        _ => "保存失败"
      }
      self.config_check.set(e)
      self.say(e, bad=true)
    }
  })
}

///|
pub fn Store::start_server(self : Store) -> Unit {
  self.log.set([])
  self.run("server.start", [self.inst_arg()], fn(v) { self.server.set(ServerStatus::from_json(v)) })
}

///|
pub fn Store::stop_server(self : Store) -> Unit {
  self.run("server.stop", [self.inst_arg()], fn(v) { self.server.set(ServerStatus::from_json(v)) })
}

///|
pub fn Store::restart_server(self : Store) -> Unit {
  self.log.set([])
  self.run("server.restart", [self.inst_arg()], fn(v) { self.server.set(ServerStatus::from_json(v)) })
}

///|
pub fn Store::launch(self : Store) -> Unit {
  let who = self.character.untracked()
  if who == "" {
    self.say("这个实例还没有角色：先在启动器里创建一个", bad=true)
    return
  }
  self.game.set("launching")
  self.ask("game.launch", [self.inst_arg(), ("username", Json::string(who)), ("clean_cache", Json::boolean(self.clean_cache.untracked()))], fn(
    good,
    v,
  ) {
    if good {
      self.game.set("running")
      let cleaned = jobj(v, "cleaned")
      let c = jint(cleaned, "files")
      self.say(if c > 0 { "游戏已启动（已清理 \{c} 个缓存文件）" } else { "游戏已启动" })
    } else {
      self.game.set("idle")
      self.say(
        match v {
          String(s) => s
          _ => "启动失败"
        },
        bad=true,
      )
    }
  })
}

// ---- read for the pages ----

///|
pub fn Store::current_inst(self : Store) -> Inst? {
  let root = self.current.get()
  let list = self.instances.get()
  match list.search_by(fn(i) { i.root == root }) {
    Some(i) => Some(list[i])
    None => None
  }
}

///|
pub fn Store::current_character(self : Store) -> Character? {
  let who = self.character.get()
  let list = self.characters.get()
  match list.search_by(fn(c) { c.username == who }) {
    Some(i) => Some(list[i])
    None => None
  }
}

///|
/// The mods as the search and the filter show them.
pub fn Store::mods_shown(self : Store) -> Array[ModItem] {
  let q = self.mods_query.get().to_lower()
  let f = self.mods_filter.get()
  self.mods
  .get()
  .filter(fn(m) {
    let by_filter = match f {
      "on" => m.enabled
      "off" => !m.enabled
      "update" => m.update != ""
      _ => true
    }
    by_filter &&
    (q == "" ||
    m.name.to_lower().contains(q) ||
    m.author.to_lower().contains(q) ||
    m.roots.iter().any(fn(r) { r.to_lower().contains(q) }))
  })
}
