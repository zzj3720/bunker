// 启动与服务端: the instance at a glance (mods enabled, Chinese coverage, the last snapshot, the server's health), the
// server's live log with its warnings and errors counted, and what is checked and done before the game starts.

///|
fn kpi(parent : @view.View, label : String, value : () -> String, sub : () -> String, ink : () -> Int64) -> Unit {
  card(parent, fn(c) {
    c.grow(Is(1.0)).basis(Is(Some(0.0))).gap(Is(6.0)) |> ignore
    text(c, label, size=11.5, ink=c_text2) |> ignore
    text_by(c, value, size=20.0, weight=700, ink_by=ink) |> ignore
    text_by(c, sub, size=11.5, ink=c_muted).cuts(Is(true)) |> ignore
  })
  |> ignore
}

///|
fn state_words(st : String) -> String {
  match st {
    "running" => "运行中"
    "starting" => "启动中"
    "stopping" => "正在停止"
    "failed" => "启动失败"
    "external" => "外部运行中"
    _ => "未启动"
  }
}

///|
fn play_page(v : @view.View, s : @model.Store) -> Unit {
  v.padding(Is((22.0, 18.0, 22.0, 18.0))).gap(Is(16.0)) |> ignore
  row(v, 12.0, fn(g) {
    g.lines_up(Is(None)) |> ignore
    kpi(g, "已启用 Mod", fn() {
      let all = s.mods.get()
      "\{all.filter(fn(m) { m.enabled }).length()} / \{all.length()}"
    }, fn() {
      let on = s.mods.get().filter(fn(m) { m.enabled })
      "服务端 \{on.filter(fn(m) { m.server }).length()} · 客户端 \{on.filter(fn(m) { m.client }).length()}（成对追踪）"
    }, fn() { c_text })
    kpi(g, "中文本地化覆盖", fn() {
      let content = s.mods.get().filter(fn(m) { m.loc != "na" })
      if content.is_empty() {
        "—"
      } else {
        let full = content.filter(fn(m) { m.loc == "full" }).length()
        "\{full * 100 / content.length()}%"
      }
    }, fn() {
      let content = s.mods.get().filter(fn(m) { m.loc != "na" })
      let full = content.filter(fn(m) { m.loc == "full" }).length()
      if content.is_empty() {
        "没有带文本的内容类 Mod"
      } else {
        "\{content.length()} 个内容类 Mod，\{full} 个含完整 ch 词条"
      }
    }, fn() {
      let content = s.mods.get().filter(fn(m) { m.loc != "na" })
      if content.iter().all(fn(m) { m.loc == "full" }) {
        c_green
      } else {
        c_amber
      }
    })
    kpi(g, "最近存档快照", fn() {
      match s.snapshots.get().get(0) {
        Some(x) => s.moment(x.created_at)
        None => "暂无"
      }
    }, fn() {
      match s.snapshots.get().get(0) {
        Some(x) => "\{kind_words(x.kind)} · 共 \{s.snapshots.get().length()} 份"
        None => "装 Mod 前会自动备份"
      }
    }, fn() { c_text })
    kpi(g, "服务端健康状态", fn() { state_words(s.server.get().state) }, fn() {
      let st = s.server.get()
      let took = if st.ready_at > 0.0 && st.started_at > 0.0 {
        let tenths = ((st.ready_at - st.started_at) / 100.0).to_int()
        "耗时 \{tenths / 10}.\{tenths % 10}s 启动 · "
      } else {
        ""
      }
      if st.message != "" {
        st.message
      } else {
        "\{took}\{st.errors} 报错 \{st.warns} 警告"
      }
    }, fn() { state_color(s.server.get().state) })
  })
  |> ignore
  row(v, 14.0, fn(two) {
    two.lines_up(Is(None)).grow(Is(1.0)) |> ignore
    console_card(two, s)
    checks_card(two, s)
  })
  |> ignore
}

///|
fn level_ink(level : String) -> Int64 {
  match level {
    "ok" => c_green
    "warn" => c_amber
    "error" => c_red
    _ => c_console_text
  }
}

///|
/// The console shows the newest lines that fit.
let console_lines : Int = 18

///|
fn console_card(two : @view.View, s : @model.Store) -> Unit {
  card(two, fn(c) {
    c.grow(Is(1.55)).basis(Is(Some(0.0))).gap(Is(12.0)) |> ignore
    row(c, 6.0, fn(h) {
      text(h, "服务端实时日志（SPT.Server.exe）", size=14.0, weight=700).grow(Is(1.0)) |> ignore
      // (the log's filter: @sf's segmented choice, the counts in its words)
      let filters = ["all", "warn", "error"]
      @sf.seg_node(h, fn() { ["全部", "警告 \{s.server.get().warns}", "报错 \{s.server.get().errors}"] }, fn() {
        filters.search(s.log_filter.get()).unwrap_or(0)
      }, fn(i) { s.log_filter.set(filters[i]) }, said="日志筛选")
      |> ignore
      button(h, "复制日志", fn() {
        let all = s.log.untracked().map(fn(l) { "[\{l.t}] \{l.text}" }).join("\n")
        @hostapi.clipboard_set(all)
        s.say("已复制 \{s.log.untracked().length()} 行日志")
      }, small=true)
      |> ignore
    })
    |> ignore
    c.box(fn(con) {
      con
      .grow(Is(1.0))
      .min_height(Is(Some(330.0)))
      .ground(Is(c_console))
      .edge(Is((1.0, c_border)))
      .radius(Is(6.0))
      .padding(Is((12.0, 10.0, 12.0, 10.0)))
      .clips(Is(true))
      .gap(Is(3.0))
      |> ignore
      let shown = fn() {
        let f = s.log_filter.get()
        let lines = s.log.get().filter(fn(l) { f == "all" || l.level == f })
        if lines.length() > console_lines {
          lines[lines.length() - console_lines:].to_owned()
        } else {
          lines
        }
      }
      con.each(shown, fn(l) { l.n }, fn(r, line) {
        r.row(Is(true)).gap(Is(8.0)).height(Is(Some(17.0))) |> ignore
        text_by(r, fn() { "[\{line.get().t}]" }, size=11.5, ink=c_muted) |> ignore
        text_by(r, fn() { line.get().text }, size=11.5, ink_by=fn() { level_ink(line.get().level) })
        .grow(Is(1.0))
        .cuts(Is(true))
        |> ignore
      })
      con.show(fn() { s.log.get().is_empty() }, fn(e) {
        text_by(e, fn() {
          match s.server.get().state {
            "external" => "6969 端口上已有不是 Bunker 启动的服务端在运行：Bunker 不读取它的日志，也不会去停它。"
            _ => "服务端未启动。点右上角「一键启动游戏」会先拉起服务端并在这里显示实时日志。"
          }
        }, size=12.0, ink=c_muted)
        |> ignore
      })
    })
    |> ignore
  })
  |> ignore
}

///|
fn check_row(parent : @view.View, title : String, sub : () -> String, right : (@view.View) -> Unit) -> Unit {
  parent.box(fn(r) {
    r
    .row(Is(true))
    .lines_up(Is(Some(Middle)))
    .gap(Is(10.0))
    .padding(Is((12.0, 10.0, 12.0, 10.0)))
    .ground(Is(c_elevated))
    .edge(Is((1.0, c_border)))
    .radius(Is(6.0))
    |> ignore
    r.box(fn(t) {
      t.grow(Is(1.0)).shrink(Is(1.0)).gap(Is(3.0)) |> ignore
      text(t, title, size=13.0, weight=700) |> ignore
      text_by(t, sub, size=11.5, ink=c_text2, wrap=true) |> ignore
    })
    |> ignore
    right(r)
  })
  |> ignore
}

///|
fn checks_card(two : @view.View, s : @model.Store) -> Unit {
  card(two, fn(c) {
    c.grow(Is(1.0)).basis(Is(Some(0.0))).gap(Is(10.0)) |> ignore
    text(c, "启动前检查与快捷操作", size=14.0, weight=700) |> ignore
    check_row(c, "依赖与版本校验", fn() {
      let mods = s.mods.get()
      let bad = mods.filter(fn(m) { m.compat == "mismatch" })
      let v = match s.current_inst() {
        Some(i) => i.spt_version
        None => ""
      }
      if !bad.is_empty() {
        "不兼容 SPT \{v}：\{bad.map(fn(m) { m.name }).join("、")}"
      } else {
        let known = mods.filter(fn(m) { m.compat == "ok" }).length()
        "\{mods.length()} 个 Mod：\{known} 个声明兼容 SPT \{v}，其余未声明版本要求"
      }
    }, fn(r) {
      tag_by(r, fn() {
        if s.mods.get().iter().any(fn(m) { m.compat == "mismatch" }) {
          "需处理"
        } else {
          "通过"
        }
      }, fn() { if s.mods.get().iter().any(fn(m) { m.compat == "mismatch" }) { Red } else { Green } })
      |> ignore
    })
    check_row(c, "启动前清理临时缓存", fn() { "清理 BepInEx 缓存与 EFT 临时文件" }, fn(r) {
      switch(r, "启动前清理临时缓存", fn() { s.clean_cache.get() }, fn(on) { s.set_setting("clean_cache", on) }) |> ignore
    })
    check_row(c, "启动后最小化窗口", fn() { "节省前台资源，退出游戏后自动恢复" }, fn(r) {
      switch(r, "启动后最小化窗口", fn() { s.minimize.get() }, fn(on) { s.set_setting("minimize", on) }) |> ignore
    })
    check_row(c, "一键备份当前角色存档", fn() { "当前保留 \{s.snapshots.get().length()} 份历史快照（可随时回滚）" }, fn(r) {
      button(r, "立即备份", fn() { s.snapshot("手动备份") }, small=true, disabled=fn() { s.current.get() == "" }) |> ignore
    })
    check_row(c, "服务端进程", fn() {
      let st = s.server.get()
      match st.state {
        "running" | "starting" => "PID \{st.pid} · 127.0.0.1:\{st.port}"
        "external" => "端口被其它程序的服务端占用，Bunker 不会去动它"
        _ => "未运行"
      }
    }, fn(r) {
      button(r, "启动", fn() { s.start_server() }, small=true, disabled=fn() {
        let st = s.server.get().state
        s.current.get() == "" || st == "running" || st == "starting" || st == "stopping" || st == "external"
      })
      |> ignore
      button(r, "停止", fn() { s.stop_server() }, small=true, tone=Danger, disabled=fn() {
        let st = s.server.get().state
        st != "running" && st != "starting"
      })
      |> ignore
    })
  })
  |> ignore
}

///|
fn kind_words(k : String) -> String {
  match k {
    "before_install" => "装 Mod 前自动备份"
    "before_restore" => "回滚前自动备份"
    _ => "手动备份"
  }
}
