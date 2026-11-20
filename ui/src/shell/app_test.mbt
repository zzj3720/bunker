// Bunker's window under a Tester, its core a stand-in answering as the real one does (its answers shaped by the
// core's own tests and the real 4.1.6 install's names): the instance popover stepping in to add one, the character
// popover, the mods table with its filters and switches, the install popover's two steps from a picked archive,
// snapshots and the config editor; each request the UI sends checked by what it asked.

///|
/// The stand-in core: what was asked, and its answers posted on the "core" channel as the real core posts them.
struct Fake {
  tester : Ref[@ui.Tester?]
  asked : Array[Json]
  mods : Ref[String]
}

///|
let inst_json : String =
  #|{"instances":[
  #|{"root":"C:\\Games\\SPT-4.1.6","name":"SPT 4.1.6 主力档","layout":"spt_runtime","server_rel":"SPT_Runtime","spt_version":"4.1.6","eft_version":"0.16.9.5","bepinex_version":"5.4.23.5","server_mods":21,"client_mods":24,"profiles":1,"mods":0,"port":6969,"has_server":true,"has_game":true,"current":true,"problems":[]},
  #|{"root":"C:\\Games\\SPT-4.0","name":"SPT 4.0 旧档","layout":"spt","server_rel":"SPT","spt_version":"4.0.13","eft_version":"0.16.9.0","bepinex_version":"5.4.23.2","server_mods":11,"client_mods":19,"profiles":2,"mods":0,"port":6969,"has_server":true,"has_game":true,"current":false,"problems":[]}
  #|],"current":"C:\\Games\\SPT-4.1.6"}

///|
let mods_json : String =
  #|{"mods":[
  #|{"id":"u-manimalicebreaker","name":"ManimalIcebreaker","version":"1.2.4","author":"","server":true,"client":true,"enabled":true,"managed":false,"roots":["SPT_Runtime/user/mods/ManimalIcebreaker","BepInEx/plugins/ManimalIcebreaker"],"files":18,"loc":{"status":"full","ch":412,"other":412,"langs":["ch","en"]},"source_kind":"","source_url":"","compat":"unknown","spt_range":""},
  #|{"id":"u-wtt-contentbackport","name":"WTT-ContentBackport","version":"2.1.0","author":"","server":true,"client":true,"enabled":true,"managed":false,"roots":["SPT_Runtime/user/mods/WTT-ContentBackport","BepInEx/plugins/WTT-ContentBackportClient"],"files":240,"loc":{"status":"full","ch":3366,"other":3366,"langs":["ch","en"]},"source_kind":"","source_url":"","compat":"unknown","spt_range":""},
  #|{"id":"sain","name":"SAIN","version":"4.2.0","author":"Solarint","server":true,"client":true,"enabled":true,"managed":true,"roots":["SPT_Runtime/user/mods/SAIN","BepInEx/plugins/SAIN"],"files":31,"loc":{"status":"na","ch":0,"other":0,"langs":[]},"source_kind":"github","source_url":"https://github.com/Solarint/SAIN","compat":"ok","spt_range":"~4.1"},
  #|{"id":"svm","name":"SVM","version":"1.9.2","author":"","server":true,"client":false,"enabled":false,"managed":true,"roots":["SPT_Runtime/user/mods/SVM"],"files":12,"loc":{"status":"na","ch":0,"other":0,"langs":[]},"source_kind":"archive","source_url":"","compat":"unknown","spt_range":""},
  #|{"id":"u-tyfon-uifixes","name":"Tyfon.UIFixes","version":"6.0.4","author":"","server":false,"client":true,"enabled":true,"managed":false,"roots":["BepInEx/plugins/Tyfon.UIFixes.dll"],"files":1,"loc":{"status":"na","ch":0,"other":0,"langs":[]},"source_kind":"","source_url":"","compat":"unknown","spt_range":""}
  #|]}

///|
let plan_json : String =
  #|{"id":"p20261120-083000-000","instance":"C:\\Games\\SPT-4.1.6","archive":"Tyfon-UIFixes-6.0.4.zip","name":"Tyfon-UIFixes","version":"6.0.4","mod_id":"tyfon-uifixes","target":"SPT 4.1","replaces":"","adapted":true,"mappings":[
  #|{"from":"BepInEx/plugins/Tyfon.UIFixes.dll","to":"BepInEx/plugins/Tyfon.UIFixes.dll","side":"client","files":1},
  #|{"from":"SPT/user/mods/UIFixes","to":"SPT_Runtime/user/mods/UIFixes","side":"server","files":2}],
  #|"conflicts":[],"skipped":["README.md"],"warnings":[],"files":3}

///|
fn answer(f : Fake, req : Json) -> Result[String, String] {
  let op = @model.jstr(req, "op")
  match op {
    "settings.get" => Ok("{\"clean_cache\":true}")
    "instances.list" => Ok(inst_json)
    "instances.scan" =>
      Ok(
        "[{\"root\":\"D:\\\\SPT\\\\SPT-4.1-Test\",\"name\":\"\",\"layout\":\"spt_runtime\",\"spt_version\":\"4.1.6\",\"has_server\":true,\"has_game\":true,\"bepinex_version\":\"5.4.23.5\",\"client_mods\":3,\"server_rel\":\"SPT_Runtime\",\"problems\":[]}]",
      )
    "instances.probe" =>
      Ok(
        "{\"root\":\"C:\\\\Games\\\\SPT-Test-4.1\",\"layout\":\"spt_runtime\",\"server_rel\":\"SPT_Runtime\",\"spt_version\":\"4.1.6\",\"bepinex_version\":\"5.4.23.5\",\"client_mods\":12,\"has_server\":true,\"has_game\":true,\"problems\":[]}",
      )
    "instances.add" | "instances.select" | "settings.set" | "mods.cancel_plan" => Ok("{}")
    "mods.list" => Ok(f.mods.val)
    "mods.set_enabled" => Ok("{\"id\":\"x\"}")
    "mods.uninstall" => Ok("{\"removed\":12}")
    "mods.plan_bytes" | "mods.plan_url" => Ok(plan_json)
    "mods.install" =>
      Ok("{\"manifest\":{\"id\":\"tyfon-uifixes\"},\"snapshot\":\"20261120-083001-000\",\"replaced_to\":\"\"}")
    "mods.check_updates" =>
      Ok("{\"updates\":[{\"id\":\"sain\",\"name\":\"SAIN\",\"current\":\"4.2.0\",\"latest\":\"4.2.1\"}],\"failed\":[]}")
    "profiles.list" =>
      Ok(
        "{\"characters\":[{\"profile_id\":\"6afeab5dc8ca1f0ed4b321be\",\"username\":\"bear\",\"nickname\":\"BEAR_Main\",\"side\":\"Bear\",\"edition\":\"Edge Of Darkness\",\"level\":34,\"roubles\":14200000,\"started\":true},{\"profile_id\":\"a1\",\"username\":\"alt\",\"nickname\":\"USEC_Alt\",\"side\":\"Usec\",\"edition\":\"Standard\",\"level\":12,\"roubles\":850000,\"started\":true}]}",
      )
    "snapshots.list" =>
      Ok(
        "[{\"id\":\"20261120-061000-000\",\"created_at\":1795155000000,\"note\":\"安装 SAIN 之前\",\"kind\":\"before_install\",\"files\":[\"6afeab5dc8ca1f0ed4b321be.json\"],\"bytes\":1300000,\"characters\":[\"BEAR_Main Lv.34\"]}]",
      )
    "snapshots.create" =>
      Ok("{\"id\":\"20261120-083002-000\",\"files\":[\"6afeab5dc8ca1f0ed4b321be.json\"],\"kind\":\"manual\"}")
    "config.list" =>
      Ok(
        "[{\"path\":\"SPT_Runtime/user/mods/Skwizzy.LootingBots/Config/config.json\",\"kind\":\"json\",\"owner\":\"Skwizzy.LootingBots\",\"bytes\":2048},{\"path\":\"BepInEx/config/com.mpstark.dynamicmaps.cfg\",\"kind\":\"cfg\",\"owner\":\"BepInEx\",\"bytes\":900}]",
      )
    "config.read" => Ok("{\"text\":\"{\\n  \\\"enabled\\\": true\\n}\"}")
    "config.check" => Ok("{\"ok\":true}")
    "config.save" => Ok("{\"backup\":\"x.json.bak\"}")
    "server.status" =>
      Ok("{\"state\":\"stopped\",\"pid\":0,\"port\":6969,\"started_at\":0,\"ready_at\":0,\"warns\":0,\"errors\":0}")
    "server.log" => Ok("[]")
    "server.start" =>
      Ok("{\"state\":\"starting\",\"pid\":4242,\"port\":6969,\"started_at\":1795160000000,\"ready_at\":0,\"warns\":0,\"errors\":0}")
    "game.launch" => Ok("{\"pid\":777,\"token\":\"6afeab5dc8ca1f0ed4b321be\",\"cleaned\":{\"files\":3}}")
    _ => Err("未知操作：\{op}")
  }
}

///|
fn Fake::call(self : Fake, id : Int64, json : String) -> Unit {
  let req = @json.parse(json) catch { _ => Json::null() }
  self.asked.push(req)
  let reply = match answer(self, req) {
    Ok(data) => "{\"id\":\{id},\"ok\":true,\"data\":\{data}}"
    Err(e) => "{\"id\":\{id},\"ok\":false,\"error\":\"\{e}\"}"
  }
  match self.tester.val {
    Some(t) => t.post("core", reply)
    None => ()
  }
}

///|
fn Fake::ops(self : Fake) -> Array[String] {
  self.asked.map(fn(r) { @model.jstr(r, "op") })
}

///|
fn Fake::last(self : Fake, op : String) -> Json {
  let found = self.asked.filter(fn(r) { @model.jstr(r, "op") == op })
  match found.last() {
    Some(r) => r
    None => Json::null()
  }
}

///|
/// Every word the window shows now, one after another.
fn said(t : @ui.Tester) -> String {
  t.words_said().map(fn(w) { w.0 }).join("|")
}

///|
/// The window 1360 by 840 with its stand-in core, started and its first answers taken.
fn window() -> (@ui.Tester, @model.Store, Fake) {
  let fake = { tester: Ref(None), asked: [], mods: Ref(mods_json) }
  let store = @model.Store::new({
    call: fn(id, j) { fake.call(id, j) },
    call_data: fn(id, j, _) { fake.call(id, j) },
  })
  @ui.on_message_said("core", fn(json, _) { store.receive(json) })
  let t = @ui.Tester::tree(fn(root) { @shell.app(root, store) }, w=1360.0, h=840.0, dark=true)
  fake.tester.val = Some(t)
  store.start()
  t.run_for(100.0)
  (t, store, fake)
}

///|
test "the window opens on the current instance: its name, its mods counted in pairs, its characters and its server" {
  let (t, s, f) = window()
  let words = said(t)
  assert_true(words.contains("SPT 4.1.6 主力档"))
  assert_true(words.contains("启动与服务端"))
  assert_true(words.contains("4 / 5"), msg="enabled of all")
  assert_true(words.contains("服务端 3 · 客户端 4（成对追踪）"))
  assert_true(words.contains("BEAR_Main (Lv.34)"))
  assert_true(words.contains("EoD · 14.2M ₽"))
  assert_true(words.contains("未启动"))
  assert_true(words.contains("EFT 0.16.9.5"))
  assert_eq(s.current.get(), "C:\\Games\\SPT-4.1.6")
  let ops = f.ops()
  for op in ["instances.list", "mods.list", "profiles.list", "snapshots.list", "config.list", "server.status"] {
    assert_true(ops.contains(op), msg=op)
  }
  assert_eq(@model.jstr(f.last("mods.list"), "instance"), "C:\\Games\\SPT-4.1.6")
}

///|
test "the instance capsule's popover lists the instances; 添加实例 steps into importing in the same popover, never a second one over it" {
  let (t, s, f) = window()
  t.click(Button, label="切换 SPT 实例")
  t.run_for(300.0)
  assert_eq(t.all(Dialog).length(), 1)
  let words = said(t)
  assert_true(words.contains("SPT 4.0 旧档"))
  assert_true(words.contains("C:\\Games\\SPT-4.1.6 · 21 Mods · SPT_Runtime"))
  t.click(Button, label="+ 添加实例")
  t.run_for(300.0)
  // (the same one popover, now importing: the list gone from it)
  assert_eq(t.all(Dialog).length(), 1)
  assert_eq(s.inst_step.get(), 2)
  let words = said(t)
  assert_true(words.contains("导入已有 SPT 目录"))
  assert_false(words.contains("SPT 4.0 旧档"))
  assert_true(words.contains("D:\\SPT\\SPT-4.1-Test"), msg="what was found where SPT usually is")
  assert_true(f.ops().contains("instances.scan"))
  // a path typed: checked as it is typed, what it holds said
  t.click(TextField, label="SPT 目录")
  t.type_text("C:\\Games\\SPT-Test-4.1")
  t.run_for(100.0)
  let words = said(t)
  assert_true(words.contains("SPT_Runtime/SPT.Server.exe (4.1.6)"), msg=words)
  assert_true(words.contains("BepInEx 5.4.23.5 · 12 个已装客户端 Mod"))
  t.click(Button, label="确认导入")
  t.run_for(300.0)
  assert_eq(@model.jstr(f.last("instances.add"), "path"), "C:\\Games\\SPT-Test-4.1")
  assert_false(s.inst_open.get())
  // choosing another instance reads its mods anew
  t.click(Button, label="切换 SPT 实例")
  t.run_for(300.0)
  t.click(Button, label="SPT 4.0 旧档")
  t.run_for(300.0)
  assert_eq(@model.jstr(f.last("instances.select"), "path"), "C:\\Games\\SPT-4.0")
}

///|
test "the character capsule's popover chooses who the game starts as" {
  let (t, s, f) = window()
  t.click(Button, label="切换角色")
  t.run_for(300.0)
  assert_true(said(t).contains("USEC_Alt (Lv.12)"))
  t.click(Button, label="USEC_Alt")
  t.run_for(300.0)
  assert_eq(s.character.get(), "alt")
  assert_false(s.role_open.get())
  t.click(Button, label="▶ 一键启动游戏")
  t.run_for(100.0)
  let req = f.last("game.launch")
  assert_eq(@model.jstr(req, "username"), "alt")
  assert_true(@model.jbool(req, "clean_cache"), msg="启动前清理临时缓存 on by default")
  assert_eq(s.game.get(), "running")
}

///|
test "the mods table: pairs and their tags, filters, a switch turning one off, an update found by checking" {
  let (t, s, f) = window()
  t.click(Button, label="◫  Mod 管理")
  t.run_for(300.0)
  let words = said(t)
  assert_true(words.contains("WTT-ContentBackport"))
  assert_true(words.contains("3,366 条中文词条"))
  assert_true(words.contains("已安全禁用（12 个文件暂存于 .bunker/disabled/svm，可一键恢复）"))
  assert_true(words.contains("全部 (5)"))
  assert_true(words.contains("已禁用 (1)"))
  t.click(Button, label="已禁用 (1)")
  t.run_for(300.0)
  assert_eq(s.mods_shown().map(fn(m) { m.id }), ["svm"])
  t.click(Button, label="全部 (5)")
  t.run_for(300.0)
  // the first row's switch: WTT's (sorted as the core gives them, the first is ManimalIcebreaker)
  assert_eq(t.all(Switch).filter(fn(x) { x.label.has_prefix("启用 ") }).length(), 5)
  t.click(Switch, label="启用 ManimalIcebreaker")
  t.run_for(100.0)
  let req = f.last("mods.set_enabled")
  assert_eq(@model.jstr(req, "id"), "u-manimalicebreaker")
  assert_false(@model.jbool(req, "enabled"))
  t.click(Button, label="检查更新")
  t.run_for(300.0)
  assert_true(said(t).contains("发现新版本 v4.2.1（来自 GitHub Releases）"))
  assert_true(said(t).contains("可更新 (1)"))
}

///|
test "installing: an archive picked, its plan in the second step of the same popover, confirmed" {
  let (t, s, f) = window()
  t.click(Button, label="◫  Mod 管理")
  t.run_for(300.0)
  t.click(Button, label="+ 安装 Mod")
  t.run_for(300.0)
  assert_eq(t.all(Dialog).length(), 1)
  assert_true(said(t).contains("步骤 1 / 2 · 选择来源"))
  t.picks([("Tyfon-UIFixes-6.0.4.zip", b"PK\x03\x04 a zip")])
  t.click(Button, label="选择压缩包")
  t.run_for(300.0)
  assert_eq(@model.jstr(f.last("mods.plan_bytes"), "name"), "Tyfon-UIFixes-6.0.4.zip")
  assert_eq(t.all(Dialog).length(), 1)
  let words = said(t)
  assert_true(words.contains("步骤 2 / 2 · 路径预览"))
  assert_true(words.contains("Tyfon-UIFixes-6.0.4.zip"))
  assert_true(words.contains("已自动适配 SPT 4.1"))
  assert_true(words.contains("SPT/user/mods/UIFixes"))
  assert_true(words.contains("SPT_Runtime/user/mods/UIFixes"))
  assert_true(words.contains("✓ 无文件冲突"))
  // back to the first step and forward again: the plan let go, then made anew
  t.click(Button, label="上一步")
  t.run_for(300.0)
  assert_eq(s.install_step.get(), 1)
  assert_true(f.ops().contains("mods.cancel_plan"))
  t.click(Button, label="选择压缩包")
  t.run_for(300.0)
  t.click(Button, label="确认安装")
  t.run_for(300.0)
  let req = f.last("mods.install")
  assert_eq(@model.jstr(req, "plan"), "p20261120-083000-000")
  assert_false(@model.jbool(req, "overwrite"))
  assert_false(s.install_open.get())
  assert_true(said(t).contains("已安装 Tyfon-UIFixes（装前已自动快照存档）"))
}

///|
test "snapshots: one taken with its note; the server running, rolling back refused" {
  let (t, s, f) = window()
  t.click(Button, label="◷  存档快照")
  t.run_for(300.0)
  assert_true(said(t).contains("安装 SAIN 之前"))
  assert_true(said(t).contains("BEAR_Main Lv.34"))
  t.click(TextField, label="快照备注")
  t.type_text("打 Labs 之前")
  t.click(Button, label="立即备份")
  t.run_for(100.0)
  assert_eq(@model.jstr(f.last("snapshots.create"), "note"), "打 Labs 之前")
  // rolling back asks twice
  t.click(Button, label="回滚到此")
  t.run_for(100.0)
  assert_true(said(t).contains("确认回滚"))
  assert_false(f.ops().contains("snapshots.restore"))
  s.server.set({ ..s.server.get(), state: "running" })
  t.run_for(100.0)
  assert_true(t.get(Button, label="确认回滚").disabled)
}

///|
test "config: a file opened from its mod's row, edited, checked and saved" {
  let (t, s, f) = window()
  t.click(Button, label="⚙  配置编辑")
  t.run_for(300.0)
  assert_true(said(t).contains("config.json"))
  assert_true(said(t).contains("com.mpstark.dynamicmaps.cfg"))
  t.click(Button, label="SPT_Runtime/user/mods/Skwizzy.LootingBots/Config/config.json")
  t.run_for(100.0)
  assert_eq(s.config_path.get(), "SPT_Runtime/user/mods/Skwizzy.LootingBots/Config/config.json")
  t.click(TextField, label="配置内容")
  assert_eq(t.field_words(), "{\n  \"enabled\": true\n}")
  t.type_text(" ")
  t.run_for(100.0)
  assert_true(f.ops().contains("config.check"))
  assert_true(said(t).contains("✓ 语法正确"))
  t.click(Button, label="校验并保存")
  t.run_for(100.0)
  assert_eq(@model.jstr(f.last("config.save"), "path"), "SPT_Runtime/user/mods/Skwizzy.LootingBots/Config/config.json")
  assert_true(said(t).contains("已保存（原文件备份为 x.json.bak）"))
}
